//! Minimal ZMODEM framing primitives.
//!
//! The caller file-transfer subsystem uses these helpers as the byte-level
//! foundation for ZMODEM handshakes. Full send/receive state machines live above
//! frame parsing so they can share security, accounting, and transport policy
//! with XMODEM-CRC.

use crate::TransferError;
use crate::crc::crc16_xmodem;

const ZPAD: u8 = b'*';
const ZDLE: u8 = 0x18;
const ZBIN: u8 = b'A';
const ZHEX: u8 = b'B';

/// ZMODEM binary header frame types used by the caller transfer stack.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZmodemFrameKind {
    /// Receiver initialization request.
    Zrqinit = 0,
    /// Receiver initialization response.
    Zrinit = 1,
    /// Send-init request.
    Zsinit = 2,
    /// ACK for position.
    Zack = 3,
    /// File metadata header.
    Zfile = 4,
    /// Skip file.
    Zskip = 5,
    /// Negative ACK.
    Znak = 6,
    /// Abort.
    Zabort = 7,
    /// File position.
    Zrpos = 9,
    /// Data position.
    Zdata = 10,
    /// End of file.
    Zeof = 11,
    /// Finished.
    Zfin = 8,
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
            _ => Err(TransferError::ProtocolError),
        }
    }
}

/// ZMODEM binary header with four position/flag bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ZmodemHeader {
    pub kind: ZmodemFrameKind,
    pub position: u32,
}

/// Encode a binary ZMODEM header with CRC-16 validation.
#[must_use]
pub fn encode_binary_header(header: ZmodemHeader) -> Vec<u8> {
    let mut payload = Vec::with_capacity(5);
    payload.push(header.kind as u8);
    payload.extend_from_slice(&header.position.to_le_bytes());
    let crc = crc16_xmodem(&payload);

    let mut frame = Vec::with_capacity(9);
    frame.extend_from_slice(&[ZPAD, ZDLE, ZBIN]);
    frame.extend_from_slice(&payload);
    frame.extend_from_slice(&crc.to_be_bytes());
    frame
}

/// Decode a binary ZMODEM header.
///
/// # Errors
///
/// Returns [`TransferError::ProtocolError`] when the header marker, frame kind,
/// length, or CRC is invalid.
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
    let position = u32::from_le_bytes([payload[1], payload[2], payload[3], payload[4]]);
    Ok(ZmodemHeader { kind, position })
}

/// Encode a ZMODEM hex header, useful for readable handshake fixtures.
#[must_use]
pub fn encode_hex_header(header: ZmodemHeader) -> Vec<u8> {
    let binary = encode_binary_header(header);
    let payload = &binary[3..];
    let mut frame = Vec::with_capacity(3 + payload.len() * 2 + 2);
    frame.extend_from_slice(&[ZPAD, ZPAD, ZDLE, ZHEX]);
    for byte in payload {
        frame.extend_from_slice(format!("{byte:02x}").as_bytes());
    }
    frame.extend_from_slice(b"\r\n");
    frame
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn binary_header_round_trips() {
        let header = ZmodemHeader {
            kind: ZmodemFrameKind::Zfile,
            position: 42,
        };

        let encoded = encode_binary_header(header);
        let decoded = decode_binary_header(&encoded).expect("decode");

        assert_eq!(decoded, header);
    }

    #[test]
    fn binary_header_rejects_bad_crc() {
        let mut encoded = encode_binary_header(ZmodemHeader {
            kind: ZmodemFrameKind::Zrinit,
            position: 0,
        });
        encoded[9] ^= 0xFF;

        let error = decode_binary_header(&encoded).expect_err("bad crc");

        assert_eq!(error, TransferError::ProtocolError);
    }

    #[test]
    fn hex_header_uses_zmodem_prefix_and_line_end() {
        let encoded = encode_hex_header(ZmodemHeader {
            kind: ZmodemFrameKind::Zrqinit,
            position: 0,
        });

        assert!(encoded.starts_with(b"**\x18B"));
        assert!(encoded.ends_with(b"\r\n"));
    }
}
