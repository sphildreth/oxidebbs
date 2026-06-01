//! ANSI/CP437 terminal rendering helpers.

pub mod ansi_parser;

use crate::ansi_parser::strip_ansi;
use std::error::Error;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

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

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct TerminalCapabilities {
    pub supports_ansi: bool,
    pub width: u16,
}

impl TerminalCapabilities {
    pub fn ansi_80() -> Self {
        Self {
            supports_ansi: true,
            width: 80,
        }
    }

    pub fn ansi_40() -> Self {
        Self {
            supports_ansi: true,
            width: 40,
        }
    }

    pub fn plain_text() -> Self {
        Self {
            supports_ansi: false,
            width: 80,
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum LoadedScreen {
    Ansi(Vec<u8>),
    PlainText(String),
}

#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub struct ScreenAsset {
    pub ansi: Option<String>,
    pub ansi_40: Option<String>,
    pub ascii: Option<String>,
    pub text: Option<String>,
    pub pause: bool,
}

impl ScreenAsset {
    pub fn resolve_for_terminal(
        &self,
        capabilities: TerminalCapabilities,
    ) -> Option<(&str, ScreenLoadMode)> {
        if capabilities.supports_ansi {
            if capabilities.width <= 40
                && let Some(asset) = self.ansi_40.as_deref()
            {
                return Some((asset, ScreenLoadMode::Ansi));
            }

            if let Some(asset) = self.ansi.as_deref() {
                return Some((asset, ScreenLoadMode::Ansi));
            }

            if let Some(asset) = self.ansi_40.as_deref() {
                return Some((asset, ScreenLoadMode::Ansi));
            }
        } else {
            if let Some(asset) = self.ascii.as_deref() {
                return Some((asset, ScreenLoadMode::Text));
            }

            if let Some(asset) = self.text.as_deref() {
                return Some((asset, ScreenLoadMode::Text));
            }

            if capabilities.width <= 40
                && let Some(asset) = self.ansi_40.as_deref()
            {
                return Some((asset, ScreenLoadMode::AnsiFallbackToPlain));
            }

            if let Some(asset) = self.ansi.as_deref() {
                return Some((asset, ScreenLoadMode::AnsiFallbackToPlain));
            }

            if let Some(asset) = self.ansi_40.as_deref() {
                return Some((asset, ScreenLoadMode::AnsiFallbackToPlain));
            }
        }

        None
    }

    pub fn load<P: AsRef<Path>>(
        &self,
        screens_root: P,
        capabilities: TerminalCapabilities,
    ) -> Result<LoadedScreen, ScreenLoadError> {
        let (asset_name, mode) = self
            .resolve_for_terminal(capabilities)
            .ok_or(ScreenLoadError::AssetMissing)?;

        let asset_path = screens_root.as_ref().join(asset_name);
        let bytes = fs::read(&asset_path).map_err(|source| ScreenLoadError::ReadFailed {
            path: asset_path.clone(),
            source,
        })?;

        let content = match mode {
            ScreenLoadMode::Ansi => LoadedScreen::Ansi(bytes),
            ScreenLoadMode::Text => {
                LoadedScreen::PlainText(String::from_utf8_lossy(&bytes).to_string())
            }
            ScreenLoadMode::AnsiFallbackToPlain => {
                LoadedScreen::PlainText(render_plain_text(&bytes))
            }
        };

        Ok(content)
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ScreenLoadMode {
    Ansi,
    Text,
    AnsiFallbackToPlain,
}

#[derive(Debug)]
pub enum ScreenLoadError {
    AssetMissing,
    ReadFailed { path: PathBuf, source: io::Error },
}

impl fmt::Display for ScreenLoadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AssetMissing => {
                formatter.write_str("no screen variant is configured for this terminal profile")
            }
            Self::ReadFailed { path, source } => {
                write!(
                    formatter,
                    "failed to read terminal asset file {}: {source}",
                    path.display()
                )
            }
        }
    }
}

impl Error for ScreenLoadError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::AssetMissing => None,
            Self::ReadFailed { source, .. } => Some(source),
        }
    }
}

pub fn render_plain_text(input: &[u8]) -> String {
    decode_cp437(&strip_ansi(input))
}

pub fn render_menu_line(key: &str, label: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }

    let key = key.chars().next().unwrap_or('?');
    let prefix = format!("[{key}] ");

    if width <= prefix.len() {
        return prefix.chars().take(width).collect();
    }

    let label_max = width - prefix.len();
    let label = if label.len() <= label_max {
        label.to_string()
    } else if label_max <= 3 {
        label.chars().take(label_max).collect()
    } else {
        format!("{}...", truncate_to_chars(label, label_max - 3))
    };

    format!("{prefix}{label}")
}

pub fn render_status_bar(left: &str, right: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }

    let separator = " | ";
    if width <= separator.len() {
        return truncate_to_chars(left, width);
    }

    let mut left = truncate_to_chars(left, width - separator.len());
    let right = truncate_to_chars(right, width.saturating_sub(separator.len() + left.len()));
    if left.is_empty() {
        return right;
    }
    if right.is_empty() {
        return left;
    }

    let line = format!("{left}{separator}{right}");
    if line.len() > width {
        left = truncate_to_chars(&left, width.saturating_sub(separator.len() + right.len()));
        let line = format!("{left}{separator}{right}");
        if line.len() > width {
            return truncate_to_chars(&line, width);
        }
        return line;
    }

    line
}

pub fn render_pager(input: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return Vec::new();
    }
    if input.is_empty() {
        return vec![String::new()];
    }

    let mut lines = Vec::new();
    for line in input.split('\n') {
        if line.is_empty() {
            lines.push(String::new());
            continue;
        }

        for chunk in line.as_bytes().chunks(width) {
            lines.push(String::from_utf8_lossy(chunk).to_string());
        }
    }

    lines
}

fn truncate_to_chars(input: &str, max_chars: usize) -> String {
    if max_chars == 0 {
        return String::new();
    }

    input.chars().take(max_chars).collect()
}

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
    use std::env;
    use std::fs;
    use std::path::Path;
    use std::process;
    use std::time::{SystemTime, UNIX_EPOCH};

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

    #[test]
    fn selects_ansi_40_asset_for_narrow_ansi_capability() {
        let screens_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/screens");
        let asset = ScreenAsset {
            ansi: Some("login/login.ans".into()),
            ansi_40: Some("login/login-40.ans".into()),
            ..Default::default()
        };

        let selected = asset
            .resolve_for_terminal(TerminalCapabilities::ansi_40())
            .expect("terminal should resolve a screen");
        assert_eq!(selected.0, "login/login-40.ans");

        let loaded = asset
            .load(&screens_root, TerminalCapabilities::ansi_40())
            .expect("load 40-column ansi");

        let expected = fs::read(screens_root.join("login/login-40.ans")).expect("read fixture");
        assert_eq!(loaded, LoadedScreen::Ansi(expected));
    }

    #[test]
    fn falls_back_to_plain_text_when_plain_terminal_lacks_plain_assets() {
        let unique = format!(
            "oxidebbs-term-assets-{}-{}",
            process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time valid")
                .as_nanos()
        );
        let screens_root = env::temp_dir().join(unique);
        fs::create_dir_all(&screens_root).expect("create fixture dir");

        fs::write(
            screens_root.join("plain-fallback.ans"),
            b"\x1b[1mOxide \xc9",
        )
        .expect("write fixture");
        let asset = ScreenAsset {
            ansi: Some("plain-fallback.ans".into()),
            ..Default::default()
        };

        let loaded = asset
            .load(&screens_root, TerminalCapabilities::plain_text())
            .expect("load fallback screen");
        assert_eq!(loaded, LoadedScreen::PlainText("Oxide ╔".into()));

        fs::remove_dir_all(&screens_root).expect("cleanup fixture");
    }

    #[test]
    fn renders_menu_line_within_width_limits() {
        let short = render_menu_line("L", "Logon", 80);
        assert_eq!(short, "[L] Logon");
        assert!(short.len() <= 80);

        let narrow = render_menu_line("L", "This is a very long menu label for 40 columns", 40);
        assert!(narrow.len() <= 40);
        assert!(narrow.starts_with("[L] "));
        assert!(narrow.len() <= 40);
    }

    #[test]
    fn renders_status_bar_within_width_limits() {
        let status = render_status_bar("Nodes: 12", "Users: 42", 40);
        assert_eq!(status, "Nodes: 12 | Users: 42");
        assert!(status.len() <= 40);

        let narrow_status =
            render_status_bar("Very long left status", "Very long right status", 20);
        assert!(narrow_status.len() <= 20);
    }

    #[test]
    fn wraps_pager_content_within_width_limits() {
        let lines = render_pager(
            "This is a sentence that is too long for a narrow screen.",
            20,
        );
        assert_eq!(
            lines,
            vec![
                "This is a sentence t",
                "hat is too long for ",
                "a narrow screen."
            ]
        );
        assert!(lines.iter().all(|line| line.len() <= 20));
    }
}
