use std::sync::Arc;

use crate::transport::TransportError;

use crate::transport::Transport;

type LifecycleHook = Arc<dyn Fn(&str) + Send + Sync>;

pub const IAC: u8 = 0xFF;
pub const SE: u8 = 0xF0;
pub const DONT: u8 = 0xFE;
pub const DO: u8 = 0xFD;
pub const WONT: u8 = 0xFC;
pub const WILL: u8 = 0xFB;
pub const SB: u8 = 0xFA;

pub const TELOPT_BINARY: u8 = 0;
pub const TELOPT_ECHO: u8 = 1;
pub const TELOPT_SUPPRESS_GO_AHEAD: u8 = 3;
pub const TELOPT_TERMINAL_TYPE: u8 = 24;
pub const TELOPT_NAWS: u8 = 31;
pub const TELOPT_TTYPE_SEND: u8 = 1;
pub const TELOPT_TTYPE_IS: u8 = 0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TelnetCommand {
    Will,
    Wont,
    Do,
    Dont,
}

impl TelnetCommand {
    fn as_u8(self) -> u8 {
        match self {
            Self::Will => WILL,
            Self::Wont => WONT,
            Self::Do => DO,
            Self::Dont => DONT,
        }
    }

    fn from_u8(byte: u8) -> Option<Self> {
        match byte {
            WILL => Some(Self::Will),
            WONT => Some(Self::Wont),
            DO => Some(Self::Do),
            DONT => Some(Self::Dont),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TelnetEvent {
    Data(u8),
    Negotiation {
        command: TelnetCommand,
        option: u8,
        accepted: bool,
    },
    TerminalType(Vec<u8>),
    TerminalTypeRequest,
    WindowSize {
        columns: u16,
        rows: u16,
    },
    Subnegotiation {
        option: u8,
        data: Vec<u8>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TelnetOptionPolicy {
    pub accept_echo: bool,
    pub accept_suppress_go_ahead: bool,
    pub accept_terminal_type: bool,
    pub accept_naws: bool,
    pub terminal_type: Vec<u8>,
}

impl Default for TelnetOptionPolicy {
    fn default() -> Self {
        Self {
            accept_echo: true,
            accept_suppress_go_ahead: true,
            accept_terminal_type: true,
            accept_naws: true,
            terminal_type: b"VT100".to_vec(),
        }
    }
}

impl TelnetOptionPolicy {
    fn accepts(&self, option: u8) -> bool {
        match option {
            TELOPT_ECHO => self.accept_echo,
            TELOPT_SUPPRESS_GO_AHEAD => self.accept_suppress_go_ahead,
            TELOPT_TERMINAL_TYPE => self.accept_terminal_type,
            TELOPT_NAWS => self.accept_naws,
            _ => false,
        }
    }
}

#[derive(Default)]
pub struct TelnetLifecycleHooks {
    pub on_connect: Option<LifecycleHook>,
    pub on_disconnect: Option<LifecycleHook>,
}

impl TelnetLifecycleHooks {
    pub fn with_connect_hook<F>(mut self, hook: F) -> Self
    where
        F: Fn(&str) + Send + Sync + 'static,
    {
        self.on_connect = Some(Arc::new(hook));
        self
    }

    pub fn with_disconnect_hook<F>(mut self, hook: F) -> Self
    where
        F: Fn(&str) + Send + Sync + 'static,
    {
        self.on_disconnect = Some(Arc::new(hook));
        self
    }
}

#[derive(Default)]
enum ParserState {
    #[default]
    Data,
    Iac,
    Negotiation,
    SubnegotiationOption,
    SubnegotiationData,
}

#[derive(Default)]
pub struct TelnetParser {
    state: ParserState,
    pending_command: Option<TelnetCommand>,
    pending_subnegotiation_option: Option<u8>,
    subnegotiation_data: Vec<u8>,
    subnegotiation_escape: bool,
    policy: TelnetOptionPolicy,
}

impl TelnetParser {
    pub fn with_policy(policy: TelnetOptionPolicy) -> Self {
        Self {
            policy,
            state: ParserState::Data,
            ..Default::default()
        }
    }

    pub fn policy(&self) -> &TelnetOptionPolicy {
        &self.policy
    }

    pub fn feed(&mut self, byte: u8, reply: &mut Vec<u8>) -> Option<TelnetEvent> {
        match self.state {
            ParserState::Data => {
                if byte == IAC {
                    self.state = ParserState::Iac;
                    None
                } else {
                    Some(TelnetEvent::Data(byte))
                }
            }
            ParserState::Iac => {
                self.state = match byte {
                    IAC => {
                        self.state = ParserState::Data;
                        return Some(TelnetEvent::Data(IAC));
                    }
                    SB => ParserState::SubnegotiationOption,
                    DONT | DO | WILL | WONT => {
                        self.pending_command = TelnetCommand::from_u8(byte);
                        ParserState::Negotiation
                    }
                    _ => {
                        // Ignore unhandled command bytes and continue.
                        ParserState::Data
                    }
                };

                None
            }
            ParserState::Negotiation => {
                let command = self
                    .pending_command
                    .take()
                    .expect("negotiation state without pending command");
                let accepted = self.policy.accepts(byte);
                self.state = ParserState::Data;
                self.negotiate_reply(command, byte, accepted, reply);
                Some(TelnetEvent::Negotiation {
                    command,
                    option: byte,
                    accepted,
                })
            }
            ParserState::SubnegotiationOption => {
                self.pending_subnegotiation_option = Some(byte);
                self.subnegotiation_data.clear();
                self.subnegotiation_escape = false;
                self.state = ParserState::SubnegotiationData;
                None
            }
            ParserState::SubnegotiationData => match self.subnegotiation_escape {
                true => {
                    self.subnegotiation_escape = false;
                    if byte == IAC {
                        self.subnegotiation_data.push(IAC);
                        None
                    } else if byte == SE {
                        self.state = ParserState::Data;
                        self.take_subnegotiation_event(reply)
                    } else {
                        self.subnegotiation_data.push(IAC);
                        self.subnegotiation_data.push(byte);
                        None
                    }
                }
                false => {
                    if byte == IAC {
                        self.subnegotiation_escape = true;
                        None
                    } else {
                        self.subnegotiation_data.push(byte);
                        None
                    }
                }
            },
        }
    }

    fn negotiate_reply(
        &self,
        command: TelnetCommand,
        option: u8,
        accepted: bool,
        reply: &mut Vec<u8>,
    ) {
        let should_reply = accepted;
        let response = match command {
            TelnetCommand::Will if should_reply => TelnetCommand::Do,
            TelnetCommand::Will => TelnetCommand::Dont,
            TelnetCommand::Do if should_reply => TelnetCommand::Will,
            TelnetCommand::Do => TelnetCommand::Wont,
            _ => {
                return;
            }
        };

        reply.extend_from_slice(&[IAC, response.as_u8(), option]);
    }

    fn take_subnegotiation_event(&mut self, reply: &mut Vec<u8>) -> Option<TelnetEvent> {
        let option = match self.pending_subnegotiation_option.take() {
            Some(option) => option,
            None => {
                return None;
            }
        };
        let data = std::mem::take(&mut self.subnegotiation_data);

        Some(match option {
            TELOPT_TERMINAL_TYPE => self.resolve_terminal_type_subnegotiation(data, reply),
            TELOPT_NAWS => self.parse_window_size(data),
            _ => TelnetEvent::Subnegotiation { option, data },
        })
    }

    fn resolve_terminal_type_subnegotiation(
        &self,
        mut data: Vec<u8>,
        reply: &mut Vec<u8>,
    ) -> TelnetEvent {
        if data.is_empty() {
            return TelnetEvent::Subnegotiation {
                option: TELOPT_TERMINAL_TYPE,
                data,
            };
        }

        let command = data.remove(0);
        if command == TELOPT_TTYPE_SEND && self.policy.accept_terminal_type {
            reply.extend_from_slice(&[IAC, SB, TELOPT_TERMINAL_TYPE, TELOPT_TTYPE_IS]);
            reply.extend_from_slice(&self.policy.terminal_type);
            reply.extend_from_slice(&[IAC, SE]);
            TelnetEvent::TerminalTypeRequest
        } else if command == TELOPT_TTYPE_IS {
            TelnetEvent::TerminalType(data)
        } else {
            let mut rebuilt = Vec::with_capacity(1 + data.len());
            rebuilt.push(command);
            rebuilt.extend_from_slice(&data);
            TelnetEvent::Subnegotiation {
                option: TELOPT_TERMINAL_TYPE,
                data: rebuilt,
            }
        }
    }

    fn parse_window_size(&self, data: Vec<u8>) -> TelnetEvent {
        if data.len() == 4 {
            let columns = u16::from_be_bytes([data[0], data[1]]);
            let rows = u16::from_be_bytes([data[2], data[3]]);
            TelnetEvent::WindowSize { columns, rows }
        } else {
            TelnetEvent::Subnegotiation {
                option: TELOPT_NAWS,
                data,
            }
        }
    }
}

pub struct TelnetSession<T: Transport> {
    transport: T,
    parser: TelnetParser,
    session_id: String,
    hooks: TelnetLifecycleHooks,
    connected: bool,
}

impl<T: Transport> TelnetSession<T> {
    pub fn new(transport: T, session_id: impl Into<String>) -> Self {
        Self::with_parser(
            transport,
            session_id,
            TelnetParser::with_policy(TelnetOptionPolicy::default()),
            TelnetLifecycleHooks::default(),
        )
    }

    pub fn with_parser(
        transport: T,
        session_id: impl Into<String>,
        parser: TelnetParser,
        hooks: TelnetLifecycleHooks,
    ) -> Self {
        Self {
            transport,
            parser,
            session_id: session_id.into(),
            hooks,
            connected: false,
        }
    }

    pub async fn read(&mut self) -> Result<Option<TelnetEvent>, TransportError> {
        self.maybe_connect();
        let byte = self.transport.read_byte().await?;
        match byte {
            None => {
                self.maybe_disconnect();
                Ok(None)
            }
            Some(byte) => {
                let mut reply = Vec::new();
                let event = self.parser.feed(byte, &mut reply);
                if !reply.is_empty() {
                    self.transport.write_all(&reply).await?;
                }
                Ok(event)
            }
        }
    }

    fn maybe_connect(&mut self) {
        if self.connected {
            return;
        }
        self.connected = true;
        if let Some(hook) = &self.hooks.on_connect {
            (hook)(self.session_id.as_str());
        }
    }

    fn maybe_disconnect(&mut self) {
        if !self.connected {
            return;
        }
        self.connected = false;
        if let Some(hook) = &self.hooks.on_disconnect {
            (hook)(self.session_id.as_str());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::LoopbackTransport;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[tokio::test]
    async fn parse_escaped_iac_in_data() {
        let mut parser = TelnetParser::with_policy(TelnetOptionPolicy::default());
        let mut out = Vec::new();

        let mut events = Vec::new();
        events.push(parser.feed(b'H', &mut out).expect("data event"));
        assert!(parser.feed(IAC, &mut out).is_none());
        events.push(parser.feed(IAC, &mut out).expect("escaped iac"));
        events.push(parser.feed(b'I', &mut out).expect("data event"));

        assert_eq!(
            events,
            vec![
                TelnetEvent::Data(b'H'),
                TelnetEvent::Data(IAC),
                TelnetEvent::Data(b'I'),
            ]
        );
        assert!(out.is_empty());
    }

    #[tokio::test]
    async fn negotiate_iac_will_do_sequences() {
        let mut parser = TelnetParser::with_policy(TelnetOptionPolicy::default());
        let mut out = Vec::new();

        assert_eq!(parser.feed(IAC, &mut out), None, "enter command state");
        assert_eq!(parser.feed(DO, &mut out), None, "capture negotiation verb");
        let event = parser
            .feed(TELOPT_SUPPRESS_GO_AHEAD, &mut out)
            .expect("negotiation event");
        assert_eq!(
            out,
            vec![IAC, WILL, TELOPT_SUPPRESS_GO_AHEAD],
            "server should accept by default"
        );
        assert_eq!(
            event,
            TelnetEvent::Negotiation {
                command: TelnetCommand::Do,
                option: TELOPT_SUPPRESS_GO_AHEAD,
                accepted: true,
            }
        );

        out.clear();
        assert_eq!(parser.feed(IAC, &mut out), None);
        assert_eq!(parser.feed(WILL, &mut out), None);
        let echo_event = parser
            .feed(TELOPT_ECHO, &mut out)
            .expect("echo negotiation");
        assert_eq!(out, vec![IAC, DO, TELOPT_ECHO]);
        assert_eq!(
            echo_event,
            TelnetEvent::Negotiation {
                command: TelnetCommand::Will,
                option: TELOPT_ECHO,
                accepted: true,
            }
        );
    }

    #[tokio::test]
    async fn terminal_type_subnegotiation_emits_request_and_replies() {
        let mut parser = TelnetParser::with_policy(TelnetOptionPolicy::default());
        let mut out = Vec::new();

        let sequence = [IAC, SB, TELOPT_TERMINAL_TYPE, TELOPT_TTYPE_SEND, IAC, SE];

        let mut events = Vec::new();
        for byte in sequence {
            if let Some(event) = parser.feed(byte, &mut out) {
                events.push(event);
            }
        }

        assert_eq!(
            events,
            vec![TelnetEvent::TerminalTypeRequest],
            "terminal type request should surface as event"
        );
        assert_eq!(
            out,
            vec![
                IAC,
                SB,
                TELOPT_TERMINAL_TYPE,
                TELOPT_TTYPE_IS,
                b'V',
                b'T',
                b'1',
                b'0',
                b'0',
                IAC,
                SE,
            ],
            "server should send terminal type response"
        );
    }

    #[tokio::test]
    async fn naws_subnegotiation_parsed() {
        let mut parser = TelnetParser::with_policy(TelnetOptionPolicy::default());
        let mut out = Vec::new();
        let sequence = [IAC, SB, TELOPT_NAWS, 0x01, 0x2C, 0x00, 0x50, IAC, SE];

        let mut event = None;
        for byte in sequence {
            if let Some(e) = parser.feed(byte, &mut out) {
                event = Some(e);
            }
        }

        assert!(out.is_empty());
        assert_eq!(
            event,
            Some(TelnetEvent::WindowSize {
                columns: 300,
                rows: 80
            })
        );
    }

    #[tokio::test]
    async fn session_hooks_fire_on_connect_and_disconnect() {
        let (server, client) = LoopbackTransport::new();
        let connect_count = Arc::new(AtomicUsize::new(0));
        let disconnect_count = Arc::new(AtomicUsize::new(0));

        let hooks = TelnetLifecycleHooks::default()
            .with_connect_hook({
                let connect_count = connect_count.clone();
                move |_| {
                    connect_count.fetch_add(1, Ordering::SeqCst);
                }
            })
            .with_disconnect_hook({
                let disconnect_count = disconnect_count.clone();
                move |_| {
                    disconnect_count.fetch_add(1, Ordering::SeqCst);
                }
            });

        let mut session =
            TelnetSession::with_parser(server, "node-1", TelnetParser::default(), hooks);
        client.write_bytes(b"X").expect("write test byte");
        let event = session.read().await.expect("session read");
        assert_eq!(event, Some(TelnetEvent::Data(b'X')));

        drop(client);
        let eof = session.read().await.expect("session eof");
        assert_eq!(eof, None);

        assert_eq!(connect_count.load(Ordering::SeqCst), 1);
        assert_eq!(disconnect_count.load(Ordering::SeqCst), 1);
    }
}
