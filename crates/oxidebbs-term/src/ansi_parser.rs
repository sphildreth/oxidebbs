#[derive(Debug, Clone, Eq, PartialEq)]
pub enum AnsiSequence {
    Csi {
        params: Vec<i64>,
        intermediates: Vec<u8>,
        final_byte: u8,
    },
    Osc {
        payload: Vec<u8>,
    },
    Esc {
        byte: u8,
    },
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum ParseEvent {
    Char(u8),
    Sequence(AnsiSequence),
}

#[derive(Debug, Clone, Eq, PartialEq)]
enum ParseState {
    Ground,
    Esc,
    Csi,
    Osc,
}

pub struct AnsiParser {
    state: ParseState,
    params: Vec<i64>,
    current_param: Option<i64>,
    intermediates: Vec<u8>,
    osc_payload: Vec<u8>,
}

impl AnsiParser {
    pub fn new() -> Self {
        Self {
            state: ParseState::Ground,
            params: Vec::new(),
            current_param: None,
            intermediates: Vec::new(),
            osc_payload: Vec::new(),
        }
    }

    pub fn feed(&mut self, byte: u8) -> Option<ParseEvent> {
        match self.state {
            ParseState::Ground => self.feed_ground(byte),
            ParseState::Esc => self.feed_esc(byte),
            ParseState::Csi => self.feed_csi(byte),
            ParseState::Osc => self.feed_osc(byte),
        }
    }

    pub fn feed_all(&mut self, bytes: &[u8]) -> Vec<ParseEvent> {
        bytes.iter().filter_map(|&b| self.feed(b)).collect()
    }

    fn feed_ground(&mut self, byte: u8) -> Option<ParseEvent> {
        if byte == 0x1b {
            self.state = ParseState::Esc;
            None
        } else {
            Some(ParseEvent::Char(byte))
        }
    }

    fn feed_esc(&mut self, byte: u8) -> Option<ParseEvent> {
        match byte {
            b'[' => {
                self.state = ParseState::Csi;
                self.params.clear();
                self.current_param = None;
                self.intermediates.clear();
                None
            }
            b']' => {
                self.state = ParseState::Osc;
                self.osc_payload.clear();
                None
            }
            0x20..=0x2f => {
                self.intermediates.push(byte);
                None
            }
            0x30..=0x7e => {
                self.state = ParseState::Ground;
                Some(ParseEvent::Sequence(AnsiSequence::Esc { byte }))
            }
            _ => {
                self.state = ParseState::Ground;
                None
            }
        }
    }

    fn feed_csi(&mut self, byte: u8) -> Option<ParseEvent> {
        match byte {
            b'0'..=b'9' => {
                let digit = i64::from(byte - b'0');
                self.current_param = Some(self.current_param.unwrap_or(0) * 10 + digit);
                None
            }
            b';' => {
                self.params.push(self.current_param.unwrap_or(0));
                self.current_param = None;
                None
            }
            0x20..=0x2f => {
                self.intermediates.push(byte);
                None
            }
            0x40..=0x7e => {
                self.params.push(self.current_param.unwrap_or(0));
                self.state = ParseState::Ground;
                Some(ParseEvent::Sequence(AnsiSequence::Csi {
                    params: std::mem::take(&mut self.params),
                    intermediates: std::mem::take(&mut self.intermediates),
                    final_byte: byte,
                }))
            }
            _ => {
                self.state = ParseState::Ground;
                self.params.clear();
                self.current_param = None;
                self.intermediates.clear();
                None
            }
        }
    }

    fn feed_osc(&mut self, byte: u8) -> Option<ParseEvent> {
        match byte {
            0x07 => {
                self.state = ParseState::Ground;
                Some(ParseEvent::Sequence(AnsiSequence::Osc {
                    payload: std::mem::take(&mut self.osc_payload),
                }))
            }
            0x1b => {
                self.osc_payload.push(byte);
                None
            }
            _ if self.osc_payload.last() == Some(&0x1b) && byte == b'\\' => {
                self.osc_payload.pop();
                self.state = ParseState::Ground;
                Some(ParseEvent::Sequence(AnsiSequence::Osc {
                    payload: std::mem::take(&mut self.osc_payload),
                }))
            }
            _ => {
                self.osc_payload.push(byte);
                None
            }
        }
    }
}

impl Default for AnsiParser {
    fn default() -> Self {
        Self::new()
    }
}

pub fn strip_ansi(bytes: &[u8]) -> Vec<u8> {
    let mut parser = AnsiParser::new();
    bytes
        .iter()
        .filter_map(|&b| parser.feed(b))
        .filter_map(|event| match event {
            ParseEvent::Char(byte) => Some(byte),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_plain_text() {
        let mut parser = AnsiParser::new();
        let events = parser.feed_all(b"Hello");
        assert_eq!(
            events,
            vec![
                ParseEvent::Char(b'H'),
                ParseEvent::Char(b'e'),
                ParseEvent::Char(b'l'),
                ParseEvent::Char(b'l'),
                ParseEvent::Char(b'o'),
            ]
        );
    }

    #[test]
    fn parses_csi_cursor_position() {
        let mut parser = AnsiParser::new();
        let events = parser.feed_all(b"\x1b[12;40H");
        assert_eq!(
            events,
            vec![ParseEvent::Sequence(AnsiSequence::Csi {
                params: vec![12, 40],
                intermediates: vec![],
                final_byte: b'H',
            })]
        );
    }

    #[test]
    fn parses_csi_sgr_color() {
        let mut parser = AnsiParser::new();
        let events = parser.feed_all(b"\x1b[31;1m");
        assert_eq!(
            events,
            vec![ParseEvent::Sequence(AnsiSequence::Csi {
                params: vec![31, 1],
                intermediates: vec![],
                final_byte: b'm',
            })]
        );
    }

    #[test]
    fn parses_csi_with_default_param() {
        let mut parser = AnsiParser::new();
        let events = parser.feed_all(b"\x1b[;H");
        assert_eq!(
            events,
            vec![ParseEvent::Sequence(AnsiSequence::Csi {
                params: vec![0, 0],
                intermediates: vec![],
                final_byte: b'H',
            })]
        );
    }

    #[test]
    fn parses_esc_sequence() {
        let mut parser = AnsiParser::new();
        let input: &[u8] = &[0x1b, b'7'];
        let events = parser.feed_all(input);
        assert_eq!(
            events,
            vec![ParseEvent::Sequence(AnsiSequence::Esc { byte: b'7' })]
        );
    }

    #[test]
    fn parses_mixed_text_and_ansi() {
        let mut parser = AnsiParser::new();
        let events = parser.feed_all(b"Hi\x1b[1m!\x1b[0m");
        assert_eq!(
            events,
            vec![
                ParseEvent::Char(b'H'),
                ParseEvent::Char(b'i'),
                ParseEvent::Sequence(AnsiSequence::Csi {
                    params: vec![1],
                    intermediates: vec![],
                    final_byte: b'm',
                }),
                ParseEvent::Char(b'!'),
                ParseEvent::Sequence(AnsiSequence::Csi {
                    params: vec![0],
                    intermediates: vec![],
                    final_byte: b'm',
                }),
            ]
        );
    }

    #[test]
    fn strip_ansi_removes_escapes() {
        assert_eq!(strip_ansi(b"Hello"), b"Hello");
        assert_eq!(strip_ansi(b"\x1b[1mBold\x1b[0m"), b"Bold");
        assert_eq!(strip_ansi(b"\x1b[2J\x1b[H"), b"");
    }

    #[test]
    fn parses_osc_with_bel_terminator() {
        let mut parser = AnsiParser::new();
        let events = parser.feed_all(b"\x1b]0;Title\x07");
        assert_eq!(
            events,
            vec![ParseEvent::Sequence(AnsiSequence::Osc {
                payload: b"0;Title".to_vec(),
            })]
        );
    }

    #[test]
    fn parses_csi_no_params() {
        let mut parser = AnsiParser::new();
        let events = parser.feed_all(b"\x1b[c");
        assert_eq!(
            events,
            vec![ParseEvent::Sequence(AnsiSequence::Csi {
                params: vec![0],
                intermediates: vec![],
                final_byte: b'c',
            })]
        );
    }
}
