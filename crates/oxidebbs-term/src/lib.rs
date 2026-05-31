//! ANSI/CP437 terminal rendering helpers.

pub mod ansi_parser;

use std::error::Error;
use std::fmt;

pub const CLEAR_SCREEN_AND_HOME: &[u8] = &[0x1b, b'[', b'2', b'J', 0x1b, b'[', b'H'];
pub const RESET_ATTRIBUTES: &[u8] = &[0x1b, b'[', b'0', b'm'];

const CP437_HIGH: [char; 128] = [
    '\u{00c7}', '\u{00fc}', '\u{00e9}', '\u{00e2}', '\u{00e4}', '\u{00e0}', '\u{00e5}', '\u{00e7}',
    '\u{00ea}', '\u{00eb}', '\u{00e8}', '\u{00ef}', '\u{00ee}', '\u{00ec}', '\u{00c4}', '\u{00c5}',
    '\u{00c9}', '\u{00e6}', '\u{00c6}', '\u{00f4}', '\u{00f6}', '\u{00f2}', '\u{00fb}', '\u{00f9}',
    '\u{00ff}', '\u{00d6}', '\u{00dc}', '\u{00a2}', '\u{00a3}', '\u{00a5}', '\u{20a7}', '\u{0192}',
    '\u{00e1}', '\u{00ed}', '\u{00f3}', '\u{00fa}', '\u{00f1}', '\u{00d1}', '\u{00aa}', '\u{00ba}',
    '\u{00bf}', '\u{2310}', '\u{00ac}', '\u{00bd}', '\u{00bc}', '\u{00a1}', '\u{00ab}', '\u{00bb}',
    '\u{2591}', '\u{2592}', '\u{2593}', '\u{2502}', '\u{2524}', '\u{2561}', '\u{2562}', '\u{2556}',
    '\u{2555}', '\u{2563}', '\u{2551}', '\u{2557}', '\u{255d}', '\u{255c}', '\u{255b}', '\u{2510}',
    '\u{2514}', '\u{2534}', '\u{252c}', '\u{251c}', '\u{2500}', '\u{253c}', '\u{255e}', '\u{255f}',
    '\u{255a}', '\u{2554}', '\u{2569}', '\u{2566}', '\u{2560}', '\u{2550}', '\u{256c}', '\u{2567}',
    '\u{2568}', '\u{2564}', '\u{2565}', '\u{2559}', '\u{2558}', '\u{2552}', '\u{2553}', '\u{256b}',
    '\u{256a}', '\u{2518}', '\u{250c}', '\u{2588}', '\u{2584}', '\u{258c}', '\u{2590}', '\u{2580}',
    '\u{03b1}', '\u{00df}', '\u{0393}', '\u{03c0}', '\u{03a3}', '\u{03c3}', '\u{00b5}', '\u{03c4}',
    '\u{03a6}', '\u{0398}', '\u{03a9}', '\u{03b4}', '\u{221e}', '\u{03c6}', '\u{03b5}', '\u{2229}',
    '\u{2261}', '\u{00b1}', '\u{2265}', '\u{2264}', '\u{2320}', '\u{2321}', '\u{00f7}', '\u{2248}',
    '\u{00b0}', '\u{2219}', '\u{00b7}', '\u{221a}', '\u{207f}', '\u{00b2}', '\u{25a0}', '\u{00a0}',
];

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Cp437EncodeError {
    character: char,
    byte_index: usize,
}

impl Cp437EncodeError {
    pub fn new(character: char, byte_index: usize) -> Self {
        Self {
            character,
            byte_index,
        }
    }

    pub fn character(&self) -> char {
        self.character
    }

    pub fn byte_index(&self) -> usize {
        self.byte_index
    }
}

impl fmt::Display for Cp437EncodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "character {:?} at byte index {} is not representable in CP437",
            self.character, self.byte_index
        )
    }
}

impl Error for Cp437EncodeError {}

#[derive(Debug, Default, Clone, Eq, PartialEq)]
pub struct AnsiBuffer {
    bytes: Vec<u8>,
}

impl AnsiBuffer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear_screen_and_home(&mut self) -> &mut Self {
        self.bytes.extend_from_slice(CLEAR_SCREEN_AND_HOME);
        self
    }

    pub fn reset_attributes(&mut self) -> &mut Self {
        self.bytes.extend_from_slice(RESET_ATTRIBUTES);
        self
    }

    pub fn raw_bytes(&mut self, bytes: &[u8]) -> &mut Self {
        self.bytes.extend_from_slice(bytes);
        self
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }
}

pub fn decode_cp437(bytes: &[u8]) -> String {
    bytes.iter().copied().map(cp437_byte_to_char).collect()
}

pub fn encode_cp437(input: &str) -> Result<Vec<u8>, Cp437EncodeError> {
    let mut bytes = Vec::with_capacity(input.len());
    for (byte_index, character) in input.char_indices() {
        let byte = char_to_cp437_byte(character)
            .ok_or_else(|| Cp437EncodeError::new(character, byte_index))?;
        bytes.push(byte);
    }
    Ok(bytes)
}

pub fn cp437_byte_to_char(byte: u8) -> char {
    if byte < 0x80 {
        char::from(byte)
    } else {
        CP437_HIGH[usize::from(byte - 0x80)]
    }
}

pub fn char_to_cp437_byte(character: char) -> Option<u8> {
    if character.is_ascii() {
        return Some(character as u8);
    }

    CP437_HIGH
        .iter()
        .position(|mapped| *mapped == character)
        .and_then(|index| u8::try_from(index).ok())
        .map(|index| index + 0x80)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_cp437_box_drawing_bytes() {
        assert_eq!(
            decode_cp437(&[0xc9, 0xcd, 0xbb, b'\r', b'\n']),
            "\u{2554}\u{2550}\u{2557}\r\n"
        );
    }

    #[test]
    fn encodes_cp437_box_drawing_characters() {
        let encoded = encode_cp437("\u{255a}\u{2550}\u{255d}").expect("encode CP437");

        assert_eq!(encoded, [0xc8, 0xcd, 0xbc]);
    }

    #[test]
    fn reports_unrepresentable_cp437_character() {
        let error = encode_cp437("BBS \u{1f680}").expect_err("rocket is not CP437");

        assert_eq!(error.character(), '\u{1f680}');
        assert_eq!(error.byte_index(), 4);
    }

    #[test]
    fn ansi_buffer_emits_stable_escape_bytes() {
        let mut buffer = AnsiBuffer::new();
        buffer
            .clear_screen_and_home()
            .raw_bytes(b"OxideBBS")
            .reset_attributes();

        assert_eq!(
            buffer.as_bytes(),
            &[
                0x1b, b'[', b'2', b'J', 0x1b, b'[', b'H', b'O', b'x', b'i', b'd', b'e', b'B', b'B',
                b'S', 0x1b, b'[', b'0', b'm',
            ]
        );
    }
}
