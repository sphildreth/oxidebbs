use super::transport::{Transport, TransportError};
use std::fmt;
use thiserror::Error;

pub const IAC: u8 = 255;
pub const DO: u8 = 253;
pub const DONT: u8 = 254;
pub const WILL: u8 = 251;
pub const WONT: u8 = 252;
pub const SB: u8 = 250;
pub const SE: u8 = 240;

pub const TELOPT_ECHO: u8 = 1;
pub const TELOPT_SUPPRESS_GO_AHEAD: u8 = 3;
pub const TELOPT_NAWS: u8 = 31;
pub const TELOPT_TERMINAL_TYPE: u8 = 24;
pub const TELOPT_TTYPE_IS: u8 = 0;
pub const TELOPT_TTYPE_SEND: u8 = 1;

#[derive(Debug, Error)]
pub enum TelnetError {
    #[error("transport error: {0}")]
    Transport(#[from] TransportError),
    #[error("incomplete IAC sequence")]
    Incomplete,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TelnetCommand {
    Will,
    Wont,
    Do,
    Dont,
    Sb,
    Se,
    Iac,
}

impl From<u8> for TelnetCommand {
    fn from(b: u8) -> Self {
        match b {
            WILL => TelnetCommand::Will,
            WONT => TelnetCommand::Wont,
            DO => TelnetCommand::Do,
            DONT => TelnetCommand::Dont,
            SB => TelnetCommand::Sb,
            SE => TelnetCommand::Se,
            _ => TelnetCommand::Iac, // treat unknown as IAC escape
        }
    }
}

/// Event produced by the incremental telnet byte parser.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TelnetEvent {
    /// A raw data byte destined for the caller.
    Data(u8),
    /// A negotiated window size (NAWS subnegotiation).
    WindowSize { columns: u16, rows: u16 },
    /// A telnet option negotiation with the resulting acceptance.
    Negotiation {
        command: TelnetCommand,
        option: u8,
        accepted: bool,
    },
    /// A TERMINAL-TYPE value reported by the client.
    TerminalType(Vec<u8>),
    /// The server requested the client's terminal type.
    TerminalTypeRequest,
    /// A generic subnegotiation for an option the parser does not special-case.
    Subnegotiation { option: u8, data: Vec<u8> },
}

/// Incremental, async-free telnet protocol parser.
///
/// Feed bytes one at a time via [`TelnetParser::feed`]. When a negotiation
/// requires a response, the reply bytes are appended to the provided buffer.
/// When a full protocol unit is recognized, a [`TelnetEvent`] is returned.
#[derive(Default)]
pub struct TelnetParser {
    state: ParserState,
    sb_option: Option<u8>,
    sb_data: Vec<u8>,
}

#[derive(Default)]
enum ParserState {
    #[default]
    Data,
    Iac,
    WillOpt,
    WontOpt,
    DoOpt,
    DontOpt,
    SbOption,
    SbData,
    SbIac,
}

impl TelnetParser {
    pub fn new() -> Self {
        Self::default()
    }

    /// Feed a single byte. Returns an event when one is completed.
    /// Negotiation reply bytes are appended to `reply`.
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
            ParserState::Iac => match byte {
                IAC => {
                    self.state = ParserState::Data;
                    Some(TelnetEvent::Data(IAC))
                }
                WILL => {
                    self.state = ParserState::WillOpt;
                    None
                }
                WONT => {
                    self.state = ParserState::WontOpt;
                    None
                }
                DO => {
                    self.state = ParserState::DoOpt;
                    None
                }
                DONT => {
                    self.state = ParserState::DontOpt;
                    None
                }
                SB => {
                    self.state = ParserState::SbOption;
                    self.sb_option = None;
                    self.sb_data.clear();
                    None
                }
                SE => {
                    self.state = ParserState::Data;
                    None
                }
                _ => {
                    self.state = ParserState::Data;
                    None
                }
            },
            ParserState::WillOpt => {
                let opt = byte;
                self.state = ParserState::Data;
                reply.extend_from_slice(&[IAC, DO, opt]);
                Some(TelnetEvent::Negotiation {
                    command: TelnetCommand::Will,
                    option: opt,
                    accepted: true,
                })
            }
            ParserState::WontOpt => {
                let opt = byte;
                self.state = ParserState::Data;
                reply.extend_from_slice(&[IAC, DONT, opt]);
                Some(TelnetEvent::Negotiation {
                    command: TelnetCommand::Wont,
                    option: opt,
                    accepted: false,
                })
            }
            ParserState::DoOpt => {
                let opt = byte;
                self.state = ParserState::Data;
                reply.extend_from_slice(&[IAC, WILL, opt]);
                Some(TelnetEvent::Negotiation {
                    command: TelnetCommand::Do,
                    option: opt,
                    accepted: true,
                })
            }
            ParserState::DontOpt => {
                let opt = byte;
                self.state = ParserState::Data;
                reply.extend_from_slice(&[IAC, WONT, opt]);
                Some(TelnetEvent::Negotiation {
                    command: TelnetCommand::Dont,
                    option: opt,
                    accepted: false,
                })
            }
            ParserState::SbOption => {
                self.sb_option = Some(byte);
                self.state = ParserState::SbData;
                None
            }
            ParserState::SbData => {
                if byte == IAC {
                    self.state = ParserState::SbIac;
                } else {
                    self.sb_data.push(byte);
                }
                None
            }
            ParserState::SbIac => match byte {
                SE => {
                    let event = self.finish_subnegotiation();
                    self.state = ParserState::Data;
                    event
                }
                IAC => {
                    self.sb_data.push(IAC);
                    self.state = ParserState::SbData;
                    None
                }
                _ => {
                    self.state = ParserState::Data;
                    None
                }
            },
        }
    }

    fn finish_subnegotiation(&mut self) -> Option<TelnetEvent> {
        let option = self.sb_option?;
        let data = std::mem::take(&mut self.sb_data);
        if option == TELOPT_TERMINAL_TYPE && !data.is_empty() {
            match data[0] {
                TELOPT_TTYPE_IS => {
                    let value = data.into_iter().skip(1).collect();
                    return Some(TelnetEvent::TerminalType(value));
                }
                TELOPT_TTYPE_SEND => {
                    return Some(TelnetEvent::TerminalTypeRequest);
                }
                _ => {}
            }
        }
        if option == TELOPT_NAWS && data.len() >= 4 {
            let columns = u16::from_be_bytes([data[0], data[1]]);
            let rows = u16::from_be_bytes([data[2], data[3]]);
            return Some(TelnetEvent::WindowSize { columns, rows });
        }
        Some(TelnetEvent::Subnegotiation { option, data })
    }
}

pub struct TelnetSession<T: Transport> {
    transport: T,
}

impl<T: Transport> TelnetSession<T> {
    pub fn new(transport: T) -> Self {
        Self { transport }
    }

    // read one byte, handling IAC negotiations and returning raw data bytes.
    pub async fn read_byte(&mut self) -> Result<Option<u8>, TelnetError> {
        loop {
            let opt = self.transport.read_byte().await?;
            let byte = match opt {
                None => return Ok(None),
                Some(b) => b,
            };
            if byte == IAC {
                // need next byte
                let opt_next = self.transport.read_byte().await?;
                let next = match opt_next {
                    None => return Err(TelnetError::Incomplete),
                    Some(b) => b,
                };
                let cmd = TelnetCommand::from(next);
                match cmd {
                    TelnetCommand::Iac => return Ok(Some(IAC)),
                    TelnetCommand::Will => {
                        let opt_code = self.read_option().await?;
                        self.send_will(opt_code).await?;
                        continue;
                    }
                    TelnetCommand::Wont => {
                        let opt_code = self.read_option().await?;
                        self.send_wont(opt_code).await?;
                        continue;
                    }
                    TelnetCommand::Do => {
                        let opt_code = self.read_option().await?;
                        self.send_do(opt_code).await?;
                        continue;
                    }
                    TelnetCommand::Dont => {
                        let opt_code = self.read_option().await?;
                        self.send_dont(opt_code).await?;
                        continue;
                    }
                    TelnetCommand::Sb => {
                        // consume until SE
                        loop {
                            let opt_sb = self.transport.read_byte().await?;
                            let b = match opt_sb {
                                None => return Err(TelnetError::Incomplete),
                                Some(b) => b,
                            };
                            if b == SE {
                                break;
                            }
                        }
                        continue;
                    }
                    TelnetCommand::Se => {
                        // stray SE, ignore
                        continue;
                    }
                }
            } else {
                return Ok(Some(byte));
            }
        }
    }

    async fn read_option(&mut self) -> Result<u8, TelnetError> {
        match self.transport.read_byte().await? {
            None => Err(TelnetError::Incomplete),
            Some(b) => Ok(b),
        }
    }

    async fn send_will(&mut self, opt: u8) -> Result<(), TelnetError> {
        let bytes = [IAC, DO, opt];
        self.transport.write_all(&bytes).await?;
        Ok(())
    }

    async fn send_wont(&mut self, opt: u8) -> Result<(), TelnetError> {
        let bytes = [IAC, DONT, opt];
        self.transport.write_all(&bytes).await?;
        Ok(())
    }

    async fn send_do(&mut self, opt: u8) -> Result<(), TelnetError> {
        let bytes = [IAC, WILL, opt];
        self.transport.write_all(&bytes).await?;
        Ok(())
    }

    async fn send_dont(&mut self, opt: u8) -> Result<(), TelnetError> {
        let bytes = [IAC, WONT, opt];
        self.transport.write_all(&bytes).await?;
        Ok(())
    }

    // forward write
    pub async fn write_all(&mut self, data: &[u8]) -> Result<(), TelnetError> {
        self.transport
            .write_all(data)
            .await
            .map_err(TelnetError::Transport)
    }

    pub async fn hangup(&mut self) -> Result<(), TelnetError> {
        self.transport
            .hangup()
            .await
            .map_err(TelnetError::Transport)
    }
}

impl<T: Transport> fmt::Debug for TelnetSession<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TelnetSession").finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::LoopbackTransport;

    #[tokio::test]
    async fn telnet_negotiates_echo() {
        let (transport, mut client) = LoopbackTransport::new();
        let mut sess = TelnetSession::new(transport);
        // client sends WILL ECHO
        client.write_bytes(&[IAC, WILL, TELOPT_ECHO]).unwrap();

        // Process the negotiation in the background; it blocks awaiting more input.
        let reader = tokio::spawn(async move { sess.read_byte().await });

        // Allow the session to answer with DO ECHO, then verify the reply.
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        let out = client.read_output_bytes();
        assert_eq!(out, vec![IAC, DO, TELOPT_ECHO]);

        // Closing the client signals EOF, unblocking the reader with no data.
        drop(client);
        assert!(reader.await.unwrap().unwrap().is_none());
    }

    #[tokio::test]
    async fn echo_and_passthrough() {
        let (transport, mut client) = LoopbackTransport::new();
        let mut sess = TelnetSession::new(transport);
        // simulate client enabling echo
        client.write_bytes(&[IAC, WILL, TELOPT_ECHO]).unwrap();
        client.read_output_bytes(); // consume response
        // client sends normal data 'A','B'
        client.write_bytes(b"AB").unwrap();
        // server reads
        assert_eq!(sess.read_byte().await.unwrap(), Some(b'A'));
        assert_eq!(sess.read_byte().await.unwrap(), Some(b'B'));
        // After the caller closes, the reader observes EOF rather than echoing.
        drop(client);
        assert!(sess.read_byte().await.unwrap().is_none());
    }

    #[test]
    fn parser_emits_will_negotiation_with_accept_reply() {
        let mut parser = TelnetParser::new();
        let mut reply = Vec::new();
        assert!(parser.feed(IAC, &mut reply).is_none());
        assert!(parser.feed(WILL, &mut reply).is_none());
        let event = parser.feed(TELOPT_ECHO, &mut reply);
        assert_eq!(
            event,
            Some(TelnetEvent::Negotiation {
                command: TelnetCommand::Will,
                option: TELOPT_ECHO,
                accepted: true,
            })
        );
        assert_eq!(reply, vec![IAC, DO, TELOPT_ECHO]);
    }

    #[test]
    fn parser_emits_terminal_type_event() {
        let mut parser = TelnetParser::new();
        let mut reply = Vec::new();
        let mut event = None;
        for &b in &[
            IAC,
            SB,
            TELOPT_TERMINAL_TYPE,
            TELOPT_TTYPE_IS,
            b'S',
            b'y',
            b'n',
            b'c',
            IAC,
            SE,
        ] {
            if let Some(ev) = parser.feed(b, &mut reply) {
                event = Some(ev);
            }
        }
        assert_eq!(event, Some(TelnetEvent::TerminalType(b"Sync".to_vec())));
    }
}
