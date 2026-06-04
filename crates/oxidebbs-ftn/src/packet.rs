use std::io::{Read, Write};

use crate::error::FtnError;

const PACKET_HEADER_LEN: usize = 58;
const PACKED_MESSAGE_TYPE: u16 = 2;
const PACKET_TERMINATOR: u16 = 0;

/// FTN Type-2/2+ packet header fields used by OxideBBS.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PacketHeader {
    pub orig_node: u16,
    pub orig_net: u16,
    pub orig_zone: u16,
    pub dest_node: u16,
    pub dest_net: u16,
    pub dest_zone: u16,
    pub year: u16,
    pub month: u16,
    pub day: u16,
    pub hour: u16,
    pub minute: u16,
    pub second: u16,
    pub baud: u16,
    pub packet_type: u16,
    pub orig_net2: u16,
    pub dest_net2: u16,
    pub product_code: u8,
    pub password: [u8; 8],
    pub orig_zone2: u16,
    pub dest_zone2: u16,
    pub fill: [u8; 4],
}

impl Default for PacketHeader {
    fn default() -> Self {
        Self {
            orig_node: 1,
            orig_net: 1,
            orig_zone: 1,
            dest_node: 1,
            dest_net: 1,
            dest_zone: 1,
            year: 1986,
            month: 1,
            day: 1,
            hour: 0,
            minute: 0,
            second: 0,
            baud: 0,
            packet_type: 2,
            orig_net2: 1,
            dest_net2: 1,
            product_code: 0,
            password: [0; 8],
            orig_zone2: 1,
            dest_zone2: 1,
            fill: [0; 4],
        }
    }
}

/// Packed message attribute bits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MessageAttribute(pub u16);

impl MessageAttribute {
    /// Private message.
    pub const PRIVATE: Self = Self(0x0001);
    /// Crash message.
    pub const CRASH: Self = Self(0x0002);
    /// Received message.
    pub const RECEIVED: Self = Self(0x0004);
    /// Sent message.
    pub const SENT: Self = Self(0x0008);
    /// File-attached message.
    pub const FILE_ATTACHED: Self = Self(0x0010);
    /// In-transit message.
    pub const IN_TRANSIT: Self = Self(0x0020);
    /// Orphan message.
    pub const ORPHAN: Self = Self(0x0040);
    /// Kill/sent message.
    pub const KILL_SENT: Self = Self(0x0080);
    /// Local message.
    pub const LOCAL: Self = Self(0x0100);
    /// Hold-for-pickup message.
    pub const HOLD: Self = Self(0x0200);
}

/// FTN packet message with raw body bytes preserved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PacketMessage {
    pub to_user: String,
    pub from_user: String,
    pub subject: String,
    pub body: Vec<u8>,
    pub area_tag: String,
    pub attributes: MessageAttribute,
}

/// Parsed FTN packet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FtnPacket {
    pub header: PacketHeader,
    pub messages: Vec<PacketMessage>,
}

/// Reader for FTN Type-2 packet bytes.
pub struct PacketReader;

/// Writer for FTN Type-2+ compatible packet bytes.
pub struct PacketWriter;

impl PacketReader {
    /// Read a full FTN packet from a byte stream.
    ///
    /// # Errors
    ///
    /// Returns an error when the header, packed message structure, or packet
    /// terminator is malformed, or when the underlying reader fails.
    pub fn read<R: Read>(mut reader: R) -> Result<FtnPacket, FtnError> {
        let mut header_bytes = [0_u8; PACKET_HEADER_LEN];
        reader.read_exact(&mut header_bytes)?;
        let header = decode_header(&header_bytes)?;
        if header.packet_type != 2 {
            return Err(FtnError::Protocol(format!(
                "unsupported packet type {}",
                header.packet_type
            )));
        }

        let mut messages = Vec::new();
        loop {
            let message_type = read_u16_le(&mut reader)?;
            if message_type == PACKET_TERMINATOR {
                break;
            }
            if message_type != PACKED_MESSAGE_TYPE {
                return Err(FtnError::Protocol(format!(
                    "unsupported packed message type {message_type}"
                )));
            }
            messages.push(read_message(&mut reader)?);
        }

        Ok(FtnPacket { header, messages })
    }
}

impl PacketWriter {
    /// Write a full FTN packet to a byte stream.
    ///
    /// # Errors
    ///
    /// Returns an error when the underlying writer fails.
    pub fn write<W: Write>(mut writer: W, packet: &FtnPacket) -> Result<(), FtnError> {
        writer.write_all(&encode_header(&packet.header))?;
        for message in &packet.messages {
            write_u16_le(&mut writer, PACKED_MESSAGE_TYPE)?;
            write_message(&mut writer, &packet.header, message)?;
        }
        write_u16_le(&mut writer, PACKET_TERMINATOR)?;
        Ok(())
    }
}

fn read_message<R: Read>(reader: &mut R) -> Result<PacketMessage, FtnError> {
    let orig_node = read_u16_le(reader)?;
    let dest_node = read_u16_le(reader)?;
    let orig_net = read_u16_le(reader)?;
    let dest_net = read_u16_le(reader)?;
    let attributes = MessageAttribute(read_u16_le(reader)?);
    let _cost = read_u16_le(reader)?;
    let _date_time = read_nul_terminated(reader)?;
    let to_user = read_nul_string(reader)?;
    let from_user = read_nul_string(reader)?;
    let subject = read_nul_string(reader)?;
    let body = read_nul_terminated(reader)?;
    let area_tag = parse_area_tag(&body).unwrap_or_default();

    if orig_node == 0 || dest_node == 0 || orig_net == 0 || dest_net == 0 {
        return Err(FtnError::Protocol(
            "packed message contains zero origin or destination address part".to_string(),
        ));
    }

    Ok(PacketMessage {
        to_user,
        from_user,
        subject,
        body,
        area_tag,
        attributes,
    })
}

fn write_message<W: Write>(
    writer: &mut W,
    header: &PacketHeader,
    message: &PacketMessage,
) -> Result<(), FtnError> {
    write_u16_le(writer, header.orig_node)?;
    write_u16_le(writer, header.dest_node)?;
    write_u16_le(writer, header.orig_net)?;
    write_u16_le(writer, header.dest_net)?;
    write_u16_le(writer, message.attributes.0)?;
    write_u16_le(writer, 0)?;
    writer.write_all(b"01 Jan 86  00:00:00\0")?;
    write_nul_string(writer, &message.to_user)?;
    write_nul_string(writer, &message.from_user)?;
    write_nul_string(writer, &message.subject)?;
    if !message.area_tag.is_empty() && parse_area_tag(&message.body).is_none() {
        writer.write_all(format!("AREA:{}\r", message.area_tag).as_bytes())?;
    }
    writer.write_all(&message.body)?;
    writer.write_all(&[0])?;
    Ok(())
}

fn decode_header(bytes: &[u8; PACKET_HEADER_LEN]) -> Result<PacketHeader, FtnError> {
    let header = PacketHeader {
        orig_node: le_at(bytes, 0),
        dest_node: le_at(bytes, 2),
        year: le_at(bytes, 4),
        month: le_at(bytes, 6),
        day: le_at(bytes, 8),
        hour: le_at(bytes, 10),
        minute: le_at(bytes, 12),
        second: le_at(bytes, 14),
        baud: le_at(bytes, 16),
        packet_type: le_at(bytes, 18),
        orig_net: le_at(bytes, 20),
        dest_net: le_at(bytes, 22),
        product_code: bytes[24],
        password: bytes[26..34]
            .try_into()
            .map_err(|_| FtnError::Parse("invalid packet password field".to_string()))?,
        orig_zone: le_at(bytes, 34),
        dest_zone: le_at(bytes, 36),
        orig_net2: le_at(bytes, 38),
        dest_net2: le_at(bytes, 40),
        orig_zone2: le_at(bytes, 50),
        dest_zone2: le_at(bytes, 52),
        fill: bytes[54..58]
            .try_into()
            .map_err(|_| FtnError::Parse("invalid packet fill field".to_string()))?,
    };

    if header.orig_node == 0 || header.dest_node == 0 {
        return Err(FtnError::Protocol(
            "packet header contains zero origin or destination node".to_string(),
        ));
    }
    Ok(header)
}

fn encode_header(header: &PacketHeader) -> [u8; PACKET_HEADER_LEN] {
    let mut bytes = [0_u8; PACKET_HEADER_LEN];
    write_le_at(&mut bytes, 0, header.orig_node);
    write_le_at(&mut bytes, 2, header.dest_node);
    write_le_at(&mut bytes, 4, header.year);
    write_le_at(&mut bytes, 6, header.month);
    write_le_at(&mut bytes, 8, header.day);
    write_le_at(&mut bytes, 10, header.hour);
    write_le_at(&mut bytes, 12, header.minute);
    write_le_at(&mut bytes, 14, header.second);
    write_le_at(&mut bytes, 16, header.baud);
    write_le_at(&mut bytes, 18, header.packet_type);
    write_le_at(&mut bytes, 20, header.orig_net);
    write_le_at(&mut bytes, 22, header.dest_net);
    bytes[24] = header.product_code;
    bytes[26..34].copy_from_slice(&header.password);
    write_le_at(&mut bytes, 34, header.orig_zone);
    write_le_at(&mut bytes, 36, header.dest_zone);
    write_le_at(&mut bytes, 38, header.orig_net2);
    write_le_at(&mut bytes, 40, header.dest_net2);
    write_le_at(&mut bytes, 50, header.orig_zone2);
    write_le_at(&mut bytes, 52, header.dest_zone2);
    bytes[54..58].copy_from_slice(&header.fill);
    bytes
}

fn parse_area_tag(body: &[u8]) -> Option<String> {
    let rest = body.strip_prefix(b"AREA:")?;
    let end = rest
        .iter()
        .position(|byte| *byte == b'\r' || *byte == b'\n')
        .unwrap_or(rest.len());
    if end == 0 {
        return None;
    }
    Some(String::from_utf8_lossy(&rest[..end]).into_owned())
}

fn read_nul_string<R: Read>(reader: &mut R) -> Result<String, FtnError> {
    Ok(String::from_utf8_lossy(&read_nul_terminated(reader)?).into_owned())
}

fn read_nul_terminated<R: Read>(reader: &mut R) -> Result<Vec<u8>, FtnError> {
    let mut bytes = Vec::new();
    let mut byte = [0_u8; 1];
    loop {
        reader.read_exact(&mut byte)?;
        if byte[0] == 0 {
            return Ok(bytes);
        }
        bytes.push(byte[0]);
    }
}

fn write_nul_string<W: Write>(writer: &mut W, value: &str) -> Result<(), FtnError> {
    writer.write_all(value.as_bytes())?;
    writer.write_all(&[0])?;
    Ok(())
}

fn read_u16_le<R: Read>(reader: &mut R) -> Result<u16, FtnError> {
    let mut bytes = [0_u8; 2];
    reader.read_exact(&mut bytes)?;
    Ok(u16::from_le_bytes(bytes))
}

fn write_u16_le<W: Write>(writer: &mut W, value: u16) -> Result<(), FtnError> {
    writer.write_all(&value.to_le_bytes())?;
    Ok(())
}

fn le_at(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([bytes[offset], bytes[offset + 1]])
}

fn write_le_at(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;

    fn packet() -> FtnPacket {
        FtnPacket {
            header: PacketHeader {
                orig_node: 100,
                orig_net: 1,
                orig_zone: 42,
                dest_node: 1,
                dest_net: 1,
                dest_zone: 42,
                orig_net2: 1,
                dest_net2: 1,
                orig_zone2: 42,
                dest_zone2: 42,
                ..PacketHeader::default()
            },
            messages: vec![PacketMessage {
                to_user: "All".to_string(),
                from_user: "Sysop".to_string(),
                subject: "Hello".to_string(),
                body: b"AREA:OXIDE.GENERAL\r\x01MSGID: 42:1/100 abc\rBody".to_vec(),
                area_tag: "OXIDE.GENERAL".to_string(),
                attributes: MessageAttribute::LOCAL,
            }],
        }
    }

    #[test]
    fn packet_writer_and_reader_round_trip() {
        let packet = packet();
        let mut bytes = Vec::new();

        PacketWriter::write(&mut bytes, &packet).expect("write packet");
        let decoded = PacketReader::read(bytes.as_slice()).expect("read packet");

        assert_eq!(decoded, packet);
    }

    #[test]
    fn reader_preserves_non_utf8_body_bytes() {
        let mut packet = packet();
        packet.messages[0].body = b"AREA:BIN\r".iter().copied().chain([0xFF, 0xFE]).collect();
        packet.messages[0].area_tag = "BIN".to_string();
        let mut bytes = Vec::new();

        PacketWriter::write(&mut bytes, &packet).expect("write packet");
        let decoded = PacketReader::read(bytes.as_slice()).expect("read packet");

        assert_eq!(decoded.messages[0].body, b"AREA:BIN\r\xff\xfe");
    }

    #[test]
    fn reader_rejects_unsupported_packet_type() {
        let mut packet = packet();
        packet.header.packet_type = 9;
        let mut bytes = Vec::new();
        PacketWriter::write(&mut bytes, &packet).expect("write packet");

        let error = PacketReader::read(bytes.as_slice()).expect_err("bad type");

        assert!(matches!(error, FtnError::Protocol(_)));
    }

    #[test]
    fn writer_prepends_area_when_missing_from_body() {
        let mut packet = packet();
        packet.messages[0].body = b"Body".to_vec();
        let mut bytes = Vec::new();

        PacketWriter::write(&mut bytes, &packet).expect("write packet");
        let decoded = PacketReader::read(bytes.as_slice()).expect("read packet");

        assert_eq!(
            decoded.messages[0].body,
            b"AREA:OXIDE.GENERAL\rBody".to_vec()
        );
    }
}
