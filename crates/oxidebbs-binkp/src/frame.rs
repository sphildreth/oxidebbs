use std::io::{Read, Write};

use crate::error::BinkpError;

pub const M_NUL: u8 = 0x00;
pub const M_ADR: u8 = 0x01;
pub const M_PWD: u8 = 0x02;
pub const M_FILE: u8 = 0x03;
pub const M_OK: u8 = 0x04;
pub const M_EOB: u8 = 0x05;
pub const M_GOT: u8 = 0x06;
pub const M_ERR: u8 = 0x07;
pub const M_BSY: u8 = 0x08;
pub const M_GET: u8 = 0x09;
pub const M_SKIP: u8 = 0x0A;

const COMMAND_FRAME_FLAG: u16 = 0x8000;
const MAX_PAYLOAD_LEN: usize = 0x7FFF;

/// BinkP frame kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameType {
    Command,
    Data,
}

/// One BinkP protocol frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BinkpFrame {
    pub frame_type: FrameType,
    pub command: u8,
    pub payload: Vec<u8>,
}

impl BinkpFrame {
    /// Build a command frame.
    #[must_use]
    pub fn command(command: u8, payload: impl Into<Vec<u8>>) -> Self {
        Self {
            frame_type: FrameType::Command,
            command,
            payload: payload.into(),
        }
    }

    /// Build a data frame.
    #[must_use]
    pub fn data(payload: impl Into<Vec<u8>>) -> Self {
        Self {
            frame_type: FrameType::Data,
            command: 0,
            payload: payload.into(),
        }
    }
}

/// Encode one BinkP frame.
///
/// # Errors
///
/// Returns a protocol error when the payload exceeds the 15-bit BinkP frame
/// length field.
pub fn encode_frame(frame: &BinkpFrame) -> Result<Vec<u8>, BinkpError> {
    let content_len = match frame.frame_type {
        FrameType::Command => frame.payload.len() + 1,
        FrameType::Data => frame.payload.len(),
    };
    if content_len > MAX_PAYLOAD_LEN {
        return Err(BinkpError::Protocol(
            "BinkP frame payload exceeds 32767 bytes".to_string(),
        ));
    }

    let mut header = content_len as u16;
    if frame.frame_type == FrameType::Command {
        header |= COMMAND_FRAME_FLAG;
    }

    let mut out = Vec::with_capacity(content_len + 2);
    out.extend_from_slice(&header.to_be_bytes());
    if frame.frame_type == FrameType::Command {
        out.push(frame.command);
    }
    out.extend_from_slice(&frame.payload);
    Ok(out)
}

/// Decode one complete BinkP frame from bytes.
///
/// # Errors
///
/// Returns a protocol error when the frame is truncated or a command frame has
/// no command byte.
pub fn decode_frame(bytes: &[u8]) -> Result<BinkpFrame, BinkpError> {
    if bytes.len() < 2 {
        return Err(BinkpError::Protocol("truncated BinkP frame".to_string()));
    }
    let header = u16::from_be_bytes([bytes[0], bytes[1]]);
    let is_command = (header & COMMAND_FRAME_FLAG) != 0;
    let len = usize::from(header & !COMMAND_FRAME_FLAG);
    if bytes.len() != len + 2 {
        return Err(BinkpError::Protocol("truncated BinkP payload".to_string()));
    }

    if is_command {
        if len == 0 {
            return Err(BinkpError::Protocol(
                "BinkP command frame is missing command byte".to_string(),
            ));
        }
        Ok(BinkpFrame::command(bytes[2], bytes[3..].to_vec()))
    } else {
        Ok(BinkpFrame::data(bytes[2..].to_vec()))
    }
}

/// Read one BinkP frame from a stream.
///
/// # Errors
///
/// Returns an I/O error from the underlying reader or a protocol error for
/// malformed command frames.
pub fn read_frame<R: Read>(reader: &mut R) -> Result<BinkpFrame, BinkpError> {
    let mut header_bytes = [0_u8; 2];
    reader.read_exact(&mut header_bytes)?;
    let header = u16::from_be_bytes(header_bytes);
    let len = usize::from(header & !COMMAND_FRAME_FLAG);
    let mut payload = vec![0_u8; len];
    reader.read_exact(&mut payload)?;

    let mut frame = Vec::with_capacity(len + 2);
    frame.extend_from_slice(&header_bytes);
    frame.extend_from_slice(&payload);
    decode_frame(&frame)
}

/// Write one BinkP frame to a stream.
///
/// # Errors
///
/// Returns a protocol error for oversized frames or an I/O error from the
/// underlying writer.
pub fn write_frame<W: Write>(writer: &mut W, frame: &BinkpFrame) -> Result<(), BinkpError> {
    writer.write_all(&encode_frame(frame)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_frame_round_trips() {
        let frame = BinkpFrame::command(M_ADR, b"42:1/100".to_vec());

        let encoded = encode_frame(&frame).expect("encode");
        let decoded = decode_frame(&encoded).expect("decode");

        assert_eq!(decoded, frame);
        assert_eq!(encoded[0] & 0x80, 0x80);
    }

    #[test]
    fn data_frame_round_trips() {
        let frame = BinkpFrame::data(b"packet-bytes".to_vec());

        let encoded = encode_frame(&frame).expect("encode");
        let decoded = decode_frame(&encoded).expect("decode");

        assert_eq!(decoded, frame);
        assert_eq!(encoded[0] & 0x80, 0);
    }

    #[test]
    fn stream_read_and_write_round_trip() {
        let frame = BinkpFrame::command(M_PWD, b"secret".to_vec());
        let mut bytes = Vec::new();

        write_frame(&mut bytes, &frame).expect("write");
        let decoded = read_frame(&mut bytes.as_slice()).expect("read");

        assert_eq!(decoded, frame);
    }

    #[test]
    fn rejects_truncated_frame() {
        let error = decode_frame(&[0x80, 0x02, M_OK]).expect_err("truncated");

        assert!(matches!(error, BinkpError::Protocol(_)));
    }
}
