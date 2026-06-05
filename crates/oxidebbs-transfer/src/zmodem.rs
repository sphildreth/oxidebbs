use std::io::Write;

use crate::TransferError;
use crate::crc::crc16_xmodem;

const ZPAD: u8 = b'*';
const ZDLE: u8 = 0x18;
const ZBIN: u8 = b'A';
const ZHEX: u8 = b'B';
const ZBIN32: u8 = b'C';

const ZCRCE: u8 = 0x68;
const ZCRCG: u8 = 0x69;
const ZCRCQ: u8 = 0x6A;
const ZCRCW: u8 = 0x6B;

pub const ZDLEE: u8 = ZDLE ^ 0x40;

const XON: u8 = 0x11;
const XOFF: u8 = 0x13;
const DLE: u8 = 0x10;
const CAN: u8 = 0x18;

const CANFDX: u8 = 0x01;
const CANOVIO: u8 = 0x02;
const CANFC32: u8 = 0x20;
const ESCCTL: u8 = 0x40;

const ZCBIN: u8 = 0x01;

pub const MAX_SUBPACKET_BYTES: usize = 1024;
const CANCEL_SEQUENCE: [u8; 20] = [
    ZPAD, ZPAD, CAN, CAN, CAN, CAN, CAN, CAN, CAN, CAN, 0x08, 0x08, 0x08, 0x08, 0x08, 0x08, 0x08,
    0x08, 0x08, 0x08,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZmodemFrameKind {
    Zrqinit = 0,
    Zrinit = 1,
    Zsinit = 2,
    Zack = 3,
    Zfile = 4,
    Zskip = 5,
    Znak = 6,
    Zabort = 7,
    Zfin = 8,
    Zrpos = 9,
    Zdata = 10,
    Zeof = 11,
    Zferr = 12,
    Zcrc = 13,
    Zchallenge = 14,
    Zcompl = 15,
    Zcan = 16,
    Zfreecnt = 17,
    Zcommand = 18,
    Zstderr = 19,
}

impl TryFrom<u8> for ZmodemFrameKind {
    type Error = TransferError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Zrqinit),
            1 => Ok(Self::Zrinit),
            2 => Ok(Self::Zsinit),
            3 => Ok(Self::Zack),
            4 => Ok(Self::Zfile),
            5 => Ok(Self::Zskip),
            6 => Ok(Self::Znak),
            7 => Ok(Self::Zabort),
            8 => Ok(Self::Zfin),
            9 => Ok(Self::Zrpos),
            10 => Ok(Self::Zdata),
            11 => Ok(Self::Zeof),
            12 => Ok(Self::Zferr),
            13 => Ok(Self::Zcrc),
            14 => Ok(Self::Zchallenge),
            15 => Ok(Self::Zcompl),
            16 => Ok(Self::Zcan),
            17 => Ok(Self::Zfreecnt),
            18 => Ok(Self::Zcommand),
            19 => Ok(Self::Zstderr),
            _ => Err(TransferError::ProtocolError),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ZmodemHeader {
    pub kind: ZmodemFrameKind,
    pub flags: [u8; 4],
}

impl ZmodemHeader {
    pub fn new(kind: ZmodemFrameKind, position: u32) -> Self {
        Self {
            kind,
            flags: position_to_flags(position),
        }
    }

    pub fn position(&self) -> u32 {
        pos_from_flags(&self.flags)
    }

    pub fn from_flags(kind: ZmodemFrameKind, zf0: u8, zf1: u8, zf2: u8, zf3: u8) -> Self {
        Self {
            kind,
            flags: [zf3, zf2, zf1, zf0],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameEnd {
    Zcrce,
    Zcrcg,
    Zcrcq,
    Zcrcw,
}

impl FrameEnd {
    fn to_byte(self) -> u8 {
        match self {
            Self::Zcrce => ZCRCE,
            Self::Zcrcg => ZCRCG,
            Self::Zcrcq => ZCRCQ,
            Self::Zcrcw => ZCRCW,
        }
    }

    pub fn from_byte(byte: u8) -> Option<Self> {
        match byte {
            ZCRCE => Some(Self::Zcrce),
            ZCRCG => Some(Self::Zcrcg),
            ZCRCQ => Some(Self::Zcrcq),
            ZCRCW => Some(Self::Zcrcw),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZmodemDataSubpacket {
    pub data: Vec<u8>,
    pub frame_end: FrameEnd,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZfileMetadata {
    pub pathname: String,
    pub size: Option<u64>,
    pub mtime: Option<u64>,
    pub mode: Option<u32>,
}

impl ZfileMetadata {
    pub fn parse(data: &[u8]) -> Result<Self, TransferError> {
        if data.len() > 4096 {
            return Err(TransferError::ProtocolError);
        }
        let nul_pos = data.iter().position(|&b| b == 0).unwrap_or(data.len());
        let pathname = String::from_utf8_lossy(&data[..nul_pos]).to_string();
        let remainder = if nul_pos + 1 < data.len() {
            &data[nul_pos + 1..]
        } else {
            b""
        };
        let second_nul = remainder
            .iter()
            .position(|&b| b == 0)
            .unwrap_or(remainder.len());
        let fields_str = String::from_utf8_lossy(&remainder[..second_nul]);
        let fields: Vec<&str> = fields_str.split_whitespace().collect();

        let size = fields.first().and_then(|s| s.parse::<u64>().ok());
        let mtime = fields.get(1).and_then(|s| u64::from_str_radix(s, 8).ok());
        let mode = fields.get(2).and_then(|s| u32::from_str_radix(s, 8).ok());

        Ok(Self {
            pathname,
            size,
            mtime,
            mode,
        })
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(self.pathname.as_bytes());
        data.push(0);
        let size_str = self.size.map_or_else(|| "0".to_string(), |s| s.to_string());
        let mtime_str = self
            .mtime
            .map_or_else(|| "0".to_string(), |m| format!("{m:o}"));
        let mode_str = self
            .mode
            .map_or_else(|| "100644".to_string(), |m| format!("{m:o}"));
        let fields = format!("{size_str} {mtime_str} {mode_str}");
        data.extend_from_slice(fields.as_bytes());
        data.push(0);
        data
    }
}

pub fn position_to_flags(pos: u32) -> [u8; 4] {
    pos.to_le_bytes()
}

pub fn pos_from_flags(flags: &[u8; 4]) -> u32 {
    u32::from_le_bytes(*flags)
}

pub fn crc32_iso_hdlc(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for &byte in data {
        let index = ((crc ^ u32::from(byte)) & 0xFF) as usize;
        crc = CRC32_TABLE[index] ^ (crc >> 8);
    }
    crc ^ 0xFFFF_FFFF
}

const CRC32_TABLE: [u32; 256] = {
    let mut table = [0u32; 256];
    let mut i = 0;
    while i < 256 {
        let mut crc = i as u32;
        let mut j = 0;
        while j < 8 {
            if crc & 1 != 0 {
                crc = (crc >> 1) ^ 0xEDB8_8320;
            } else {
                crc >>= 1;
            }
            j += 1;
        }
        table[i] = crc;
        i += 1;
    }
    table
};

fn needs_escape(byte: u8, escctl: bool) -> bool {
    match byte {
        ZDLE | XON | XOFF | DLE => true,
        0x91 | 0x93 | 0x90 => true,
        0x0D | 0x8D => false,
        b if escctl && (b & 0x60) == 0 => true,
        _ => false,
    }
}

pub fn zdle_escape(data: &[u8], escctl: bool) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len());
    let mut prev_at = false;
    for &byte in data {
        let should_escape =
            needs_escape(byte, escctl) || ((byte == 0x0D || byte == 0x8D) && prev_at);
        if should_escape {
            out.push(ZDLE);
            out.push(byte ^ 0x40);
        } else {
            out.push(byte);
        }
        prev_at = byte == 0x40 || byte == 0xC0;
    }
    out
}

pub fn zdle_unescape(data: &[u8]) -> Result<Vec<u8>, TransferError> {
    let mut out = Vec::with_capacity(data.len());
    let mut i = 0;
    while i < data.len() {
        if data[i] == ZDLE {
            i += 1;
            if i >= data.len() {
                return Err(TransferError::ProtocolError);
            }
            out.push(data[i] ^ 0x40);
        } else {
            out.push(data[i]);
        }
        i += 1;
    }
    Ok(out)
}

#[must_use]
pub fn encode_binary_header(header: ZmodemHeader) -> Vec<u8> {
    let mut payload = Vec::with_capacity(5);
    payload.push(header.kind as u8);
    payload.extend_from_slice(&header.flags);
    let crc = crc16_xmodem(&payload);

    let mut frame = Vec::with_capacity(10);
    frame.extend_from_slice(&[ZPAD, ZDLE, ZBIN]);
    frame.extend_from_slice(&payload);
    frame.extend_from_slice(&crc.to_be_bytes());
    frame
}

pub fn decode_binary_header(frame: &[u8]) -> Result<ZmodemHeader, TransferError> {
    if frame.len() != 10 || frame[..3] != [ZPAD, ZDLE, ZBIN] {
        return Err(TransferError::ProtocolError);
    }
    let payload = &frame[3..8];
    let received_crc = u16::from_be_bytes([frame[8], frame[9]]);
    if crc16_xmodem(payload) != received_crc {
        return Err(TransferError::ProtocolError);
    }
    let kind = ZmodemFrameKind::try_from(payload[0])?;
    let flags = [payload[1], payload[2], payload[3], payload[4]];
    Ok(ZmodemHeader { kind, flags })
}

#[must_use]
pub fn encode_binary32_header(header: ZmodemHeader) -> Vec<u8> {
    let mut payload = Vec::with_capacity(5);
    payload.push(header.kind as u8);
    payload.extend_from_slice(&header.flags);
    let crc = crc32_iso_hdlc(&payload);

    let mut frame = Vec::with_capacity(12);
    frame.extend_from_slice(&[ZPAD, ZDLE, ZBIN32]);
    frame.extend_from_slice(&payload);
    frame.extend_from_slice(&crc.to_le_bytes());
    frame
}

pub fn decode_binary32_header(frame: &[u8]) -> Result<ZmodemHeader, TransferError> {
    if frame.len() != 12 || frame[..3] != [ZPAD, ZDLE, ZBIN32] {
        return Err(TransferError::ProtocolError);
    }
    let payload = &frame[3..8];
    let received_crc = u32::from_le_bytes([frame[8], frame[9], frame[10], frame[11]]);
    if crc32_iso_hdlc(payload) != received_crc {
        return Err(TransferError::ProtocolError);
    }
    let kind = ZmodemFrameKind::try_from(payload[0])?;
    let flags = [payload[1], payload[2], payload[3], payload[4]];
    Ok(ZmodemHeader { kind, flags })
}

#[must_use]
pub fn encode_hex_header(header: ZmodemHeader) -> Vec<u8> {
    let mut payload = Vec::with_capacity(5);
    payload.push(header.kind as u8);
    payload.extend_from_slice(&header.flags);
    let crc = crc16_xmodem(&payload);

    let mut frame = Vec::with_capacity(18);
    frame.extend_from_slice(&[ZPAD, ZPAD, ZDLE, ZHEX]);
    for byte in &payload {
        frame.extend_from_slice(format!("{byte:02x}").as_bytes());
    }
    frame.extend_from_slice(format!("{crc:04x}").as_bytes());
    frame.extend_from_slice(b"\r\n");
    frame
}

pub fn decode_hex_header(frame: &[u8]) -> Result<ZmodemHeader, TransferError> {
    if frame.len() < 20
        || frame[0] != ZPAD
        || frame[1] != ZPAD
        || frame[2] != ZDLE
        || frame[3] != ZHEX
    {
        return Err(TransferError::ProtocolError);
    }
    let hex_str = std::str::from_utf8(&frame[4..18]).map_err(|_| TransferError::ProtocolError)?;
    let mut payload = Vec::with_capacity(7);
    for i in (0..14).step_by(2) {
        let byte =
            u8::from_str_radix(&hex_str[i..i + 2], 16).map_err(|_| TransferError::ProtocolError)?;
        payload.push(byte);
    }
    let crc_payload = &payload[..5];
    let received_crc = u16::from_be_bytes([payload[5], payload[6]]);
    if crc16_xmodem(crc_payload) != received_crc {
        return Err(TransferError::ProtocolError);
    }
    let kind = ZmodemFrameKind::try_from(payload[0])?;
    let flags = [payload[1], payload[2], payload[3], payload[4]];
    Ok(ZmodemHeader { kind, flags })
}

pub fn encode_data_subpacket(subpacket: &ZmodemDataSubpacket, use_crc32: bool) -> Vec<u8> {
    let mut payload = subpacket.data.clone();
    payload.push(subpacket.frame_end.to_byte());

    let escaped = zdle_escape(&payload, true);
    let mut frame = escaped;

    if use_crc32 {
        let crc = crc32_iso_hdlc(&payload);
        let crc_bytes = crc.to_le_bytes();
        let escaped_crc = zdle_escape(&crc_bytes, true);
        frame.extend_from_slice(&escaped_crc);
    } else {
        let crc = crc16_xmodem(&payload);
        let crc_bytes = crc.to_be_bytes();
        let escaped_crc = zdle_escape(&crc_bytes, true);
        frame.extend_from_slice(&escaped_crc);
    }

    frame
}

pub fn send_cancel<W: Write>(writer: &mut W) -> Result<(), TransferError> {
    writer
        .write_all(&CANCEL_SEQUENCE)
        .map_err(|_| TransferError::Transport)?;
    writer.flush().map_err(|_| TransferError::Transport)?;
    Ok(())
}

pub fn zrinit_flags() -> u8 {
    CANFDX | CANOVIO | CANFC32 | ESCCTL
}

pub fn zfile_flags() -> [u8; 4] {
    [0, 0, 0, ZCBIN]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn binary_header_round_trips() {
        let header = ZmodemHeader::new(ZmodemFrameKind::Zfile, 42);
        let encoded = encode_binary_header(header);
        let decoded = decode_binary_header(&encoded).expect("decode");
        assert_eq!(decoded, header);
    }

    #[test]
    fn binary_header_rejects_bad_crc() {
        let mut encoded = encode_binary_header(ZmodemHeader::new(ZmodemFrameKind::Zrinit, 0));
        encoded[9] ^= 0xFF;
        assert_eq!(
            decode_binary_header(&encoded).unwrap_err(),
            TransferError::ProtocolError
        );
    }

    #[test]
    fn binary32_header_round_trips() {
        let header = ZmodemHeader::new(ZmodemFrameKind::Zdata, 12345);
        let encoded = encode_binary32_header(header);
        let decoded = decode_binary32_header(&encoded).expect("decode");
        assert_eq!(decoded, header);
    }

    #[test]
    fn binary32_header_rejects_bad_crc() {
        let mut encoded = encode_binary32_header(ZmodemHeader::new(ZmodemFrameKind::Zrinit, 0));
        encoded[10] ^= 0xFF;
        assert_eq!(
            decode_binary32_header(&encoded).unwrap_err(),
            TransferError::ProtocolError
        );
    }

    #[test]
    fn hex_header_round_trips() {
        let header = ZmodemHeader::new(ZmodemFrameKind::Zrqinit, 0);
        let encoded = encode_hex_header(header);
        assert!(encoded.starts_with(b"**\x18B"));
        assert!(encoded.ends_with(b"\r\n"));
        let decoded = decode_hex_header(&encoded).expect("decode");
        assert_eq!(decoded, header);
    }

    #[test]
    fn hex_header_rejects_bad_crc() {
        let mut encoded = encode_hex_header(ZmodemHeader::new(ZmodemFrameKind::Zrinit, 0));
        encoded[17] = b'f';
        assert_eq!(
            decode_hex_header(&encoded).unwrap_err(),
            TransferError::ProtocolError
        );
    }

    #[test]
    fn crc32_check_value() {
        let data = b"123456789";
        assert_eq!(crc32_iso_hdlc(data), 0xCBF4_3926);
    }

    #[test]
    fn zdle_escape_escapes_zdle() {
        let data = [ZDLE];
        let escaped = zdle_escape(&data, false);
        assert_eq!(escaped, vec![ZDLE, ZDLEE]);
    }

    #[test]
    fn zdle_escape_escapes_xon_xoff() {
        let data = [XON, XOFF];
        let escaped = zdle_escape(&data, false);
        assert_eq!(escaped, vec![ZDLE, XON ^ 0x40, ZDLE, XOFF ^ 0x40]);
    }

    #[test]
    fn zdle_escape_escapes_control_chars_with_escctl() {
        let data = [0x01, 0x02, 0x1F];
        let escaped = zdle_escape(&data, true);
        assert_eq!(
            escaped,
            vec![ZDLE, 0x01 ^ 0x40, ZDLE, 0x02 ^ 0x40, ZDLE, 0x1F ^ 0x40]
        );
    }

    #[test]
    fn zdle_escape_does_not_escape_normal_bytes() {
        let data = [b'A', b'B', b'C'];
        let escaped = zdle_escape(&data, false);
        assert_eq!(escaped, vec![b'A', b'B', b'C']);
    }

    #[test]
    fn zdle_unescape_round_trips() {
        let original = [ZDLE, XON, XOFF, b'A', b'B'];
        let escaped = zdle_escape(&original, true);
        let unescaped = zdle_unescape(&escaped).expect("unescape");
        assert_eq!(unescaped, original);
    }

    #[test]
    fn zfile_metadata_round_trips() {
        let meta = ZfileMetadata {
            pathname: "test.txt".to_string(),
            size: Some(1234),
            mtime: Some(1_700_000_000),
            mode: Some(0o100644),
        };
        let encoded = meta.encode();
        let decoded = ZfileMetadata::parse(&encoded).expect("parse");
        assert_eq!(decoded.pathname, "test.txt");
        assert_eq!(decoded.size, Some(1234));
        assert_eq!(decoded.mtime, Some(1_700_000_000));
        assert_eq!(decoded.mode, Some(0o100644));
    }

    #[test]
    fn zfile_metadata_rejects_oversized() {
        let data = vec![b'A'; 5000];
        assert_eq!(
            ZfileMetadata::parse(&data).unwrap_err(),
            TransferError::ProtocolError
        );
    }

    #[test]
    fn position_flags_round_trip() {
        let pos = 0x1234_5678u32;
        let flags = position_to_flags(pos);
        assert_eq!(pos_from_flags(&flags), pos);
    }

    #[test]
    fn data_subpacket_encode_produces_valid_frame() {
        let subpacket = ZmodemDataSubpacket {
            data: vec![1, 2, 3, 4, 5],
            frame_end: FrameEnd::Zcrcg,
        };
        let encoded = encode_data_subpacket(&subpacket, false);
        assert!(!encoded.is_empty());
    }

    #[test]
    fn data_subpacket_encode_crc32() {
        let subpacket = ZmodemDataSubpacket {
            data: vec![1, 2, 3],
            frame_end: FrameEnd::Zcrce,
        };
        let encoded = encode_data_subpacket(&subpacket, true);
        assert!(!encoded.is_empty());
    }

    #[test]
    fn cancel_sequence_has_correct_format() {
        assert_eq!(CANCEL_SEQUENCE[0], ZPAD);
        assert_eq!(CANCEL_SEQUENCE[1], ZPAD);
        for &byte in &CANCEL_SEQUENCE[2..10] {
            assert_eq!(byte, CAN);
        }
        for &byte in &CANCEL_SEQUENCE[10..20] {
            assert_eq!(byte, 0x08);
        }
    }

    #[test]
    fn zrinit_flags_include_required_capabilities() {
        let flags = zrinit_flags();
        assert!(flags & CANFDX != 0);
        assert!(flags & CANFC32 != 0);
        assert!(flags & ESCCTL != 0);
    }

    #[test]
    fn zfile_flags_use_zbin() {
        let flags = zfile_flags();
        assert_eq!(flags[3], ZCBIN);
    }

    #[test]
    fn frame_end_round_trips() {
        assert_eq!(FrameEnd::from_byte(ZCRCE), Some(FrameEnd::Zcrce));
        assert_eq!(FrameEnd::from_byte(ZCRCG), Some(FrameEnd::Zcrcg));
        assert_eq!(FrameEnd::from_byte(ZCRCQ), Some(FrameEnd::Zcrcq));
        assert_eq!(FrameEnd::from_byte(ZCRCW), Some(FrameEnd::Zcrcw));
        assert_eq!(FrameEnd::from_byte(0x00), None);
    }

    #[test]
    fn send_cancel_writes_to_writer() {
        let mut buf = Vec::new();
        send_cancel(&mut buf).expect("send cancel");
        assert_eq!(buf.len(), 20);
        assert_eq!(buf, CANCEL_SEQUENCE);
    }
}
