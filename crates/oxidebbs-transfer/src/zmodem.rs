use std::io::Write;

use crate::crc::crc16_xmodem;
use crate::{ByteTransport, TransferError, TransferRead};

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
const HEADER_TIMEOUT_SECS: u64 = 10;
const DATA_TIMEOUT_SECS: u64 = 10;
const DEFAULT_FRAME_RETRIES: u8 = 10;
const ACK_INTERVAL_BYTES: usize = 32 * 1024;
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZmodemFile {
    pub filename: String,
    pub payload: Vec<u8>,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ZmodemTransferStats {
    pub files: usize,
    pub bytes: u64,
    pub retries: u32,
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
    let end_byte = subpacket.frame_end.to_byte();
    let mut payload = subpacket.data.clone();
    payload.push(end_byte);

    let mut frame = zdle_escape(&subpacket.data, true);
    frame.push(ZDLE);
    frame.push(end_byte);
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

pub fn decode_data_subpacket(
    frame: &[u8],
    use_crc32: bool,
) -> Result<ZmodemDataSubpacket, TransferError> {
    let crc_len = if use_crc32 { 4 } else { 2 };
    let mut data = Vec::new();
    let mut index = 0;
    let frame_end = loop {
        if index >= frame.len() {
            return Err(TransferError::ProtocolError);
        }
        let byte = frame[index];
        index += 1;
        if byte != ZDLE {
            data.push(byte);
            continue;
        }
        if index >= frame.len() {
            return Err(TransferError::ProtocolError);
        }
        let escaped = frame[index];
        index += 1;
        if let Some(frame_end) = FrameEnd::from_byte(escaped) {
            break frame_end;
        }
        data.push(escaped ^ 0x40);
    };
    let crc_bytes = zdle_unescape(&frame[index..])?;
    if crc_bytes.len() != crc_len {
        return Err(TransferError::ProtocolError);
    }
    let mut payload = data.clone();
    payload.push(frame_end.to_byte());
    if use_crc32 {
        let received_crc =
            u32::from_le_bytes([crc_bytes[0], crc_bytes[1], crc_bytes[2], crc_bytes[3]]);
        if crc32_iso_hdlc(&payload) != received_crc {
            return Err(TransferError::ProtocolError);
        }
    } else {
        let received_crc = u16::from_be_bytes([crc_bytes[0], crc_bytes[1]]);
        if crc16_xmodem(&payload) != received_crc {
            return Err(TransferError::ProtocolError);
        }
    }
    Ok(ZmodemDataSubpacket { data, frame_end })
}

/// Send one file to a caller using OxideBBS' owned ZMODEM sender.
///
/// # Errors
///
/// Returns a transfer error on malformed handshakes, retry exhaustion, caller
/// cancellation, or underlying transport failures.
pub async fn send_zmodem_file<T: ByteTransport + ?Sized>(
    transport: &mut T,
    filename: &str,
    payload: &[u8],
) -> Result<ZmodemTransferStats, TransferError> {
    send_zmodem_files(
        transport,
        &[ZmodemFile {
            filename: filename.to_string(),
            payload: payload.to_vec(),
        }],
    )
    .await
}

/// Send a batch of files to a caller using ZMODEM.
///
/// # Errors
///
/// Returns a transfer error on malformed handshakes, retry exhaustion, caller
/// cancellation, or underlying transport failures.
pub async fn send_zmodem_files<T: ByteTransport + ?Sized>(
    transport: &mut T,
    files: &[ZmodemFile],
) -> Result<ZmodemTransferStats, TransferError> {
    wait_for_receiver_ready(transport).await?;
    let mut stats = ZmodemTransferStats::default();

    for file in files {
        send_zfile_header(transport, file).await?;
        let mut skipped = false;
        let position = loop {
            match read_required_header(transport).await? {
                header if header.kind == ZmodemFrameKind::Zrpos => {
                    break header.position() as usize;
                }
                header if header.kind == ZmodemFrameKind::Zskip => {
                    stats.files += 1;
                    skipped = true;
                    break 0;
                }
                header
                    if header.kind == ZmodemFrameKind::Zrinit
                        || header.kind == ZmodemFrameKind::Zrqinit =>
                {
                    continue;
                }
                header
                    if header.kind == ZmodemFrameKind::Zabort
                        || header.kind == ZmodemFrameKind::Zcan =>
                {
                    return Err(TransferError::Canceled);
                }
                _ => return Err(TransferError::ProtocolError),
            }
        };
        if skipped {
            continue;
        }

        let file_stats = send_zmodem_payload(transport, &file.payload, position).await?;
        stats.files += 1;
        stats.bytes = stats.bytes.saturating_add(file.payload.len() as u64);
        stats.retries = stats.retries.saturating_add(file_stats.retries);
    }

    finish_send_session(transport).await?;
    Ok(stats)
}

/// Receive one uploaded file from a caller using ZMODEM.
///
/// # Errors
///
/// Returns a transfer error on malformed handshakes, retry exhaustion, caller
/// cancellation, quota denial, or underlying transport failures.
pub async fn receive_zmodem_file<T: ByteTransport + ?Sized>(
    transport: &mut T,
    max_upload_bytes: Option<u64>,
) -> Result<ZmodemFile, TransferError> {
    let (mut files, _) = receive_zmodem_files(transport, max_upload_bytes).await?;
    if files.is_empty() {
        return Err(TransferError::ProtocolError);
    }
    Ok(files.remove(0))
}

/// Receive a ZMODEM upload batch from a caller.
///
/// # Errors
///
/// Returns a transfer error on malformed handshakes, retry exhaustion, caller
/// cancellation, quota denial, or underlying transport failures.
pub async fn receive_zmodem_files<T: ByteTransport + ?Sized>(
    transport: &mut T,
    max_upload_bytes: Option<u64>,
) -> Result<(Vec<ZmodemFile>, ZmodemTransferStats), TransferError> {
    let mut files = Vec::new();
    let mut stats = ZmodemTransferStats::default();
    write_header(
        transport,
        ZmodemHeader::from_flags(ZmodemFrameKind::Zrinit, zrinit_flags(), 0, 0, 0),
    )
    .await?;

    loop {
        let header = match read_header(transport, HEADER_TIMEOUT_SECS).await? {
            Some(header) => header,
            None => {
                write_header(
                    transport,
                    ZmodemHeader::from_flags(ZmodemFrameKind::Zrinit, zrinit_flags(), 0, 0, 0),
                )
                .await?;
                continue;
            }
        };

        match header.kind {
            ZmodemFrameKind::Zrqinit => {
                write_header(
                    transport,
                    ZmodemHeader::from_flags(ZmodemFrameKind::Zrinit, zrinit_flags(), 0, 0, 0),
                )
                .await?;
            }
            ZmodemFrameKind::Zfile => {
                let metadata_packet =
                    read_data_subpacket_from_transport(transport, true, 4096).await?;
                let metadata = ZfileMetadata::parse(&metadata_packet.data)?;
                let declared_size = metadata.size.ok_or(TransferError::ProtocolError)?;
                if let Some(max) = max_upload_bytes
                    && declared_size > max
                {
                    write_header(transport, ZmodemHeader::new(ZmodemFrameKind::Zferr, 0)).await?;
                    return Err(TransferError::QuotaDenied);
                }
                write_header(transport, ZmodemHeader::new(ZmodemFrameKind::Zrpos, 0)).await?;
                let (payload, retries) =
                    receive_zmodem_payload(transport, declared_size, max_upload_bytes).await?;
                let payload_len = payload.len() as u64;
                files.push(ZmodemFile {
                    filename: metadata.pathname,
                    payload,
                });
                stats.files += 1;
                stats.bytes = stats.bytes.saturating_add(payload_len);
                stats.retries = stats.retries.saturating_add(retries);
                write_header(
                    transport,
                    ZmodemHeader::from_flags(ZmodemFrameKind::Zrinit, zrinit_flags(), 0, 0, 0),
                )
                .await?;
            }
            ZmodemFrameKind::Zfin => {
                write_header(transport, ZmodemHeader::new(ZmodemFrameKind::Zfin, 0)).await?;
                transport.write_all(b"OO").await?;
                transport.flush().await?;
                return Ok((files, stats));
            }
            ZmodemFrameKind::Zabort | ZmodemFrameKind::Zcan => {
                return Err(TransferError::Canceled);
            }
            ZmodemFrameKind::Zcommand => {
                write_header(transport, ZmodemHeader::new(ZmodemFrameKind::Zferr, 0)).await?;
                return Err(TransferError::Unsupported);
            }
            ZmodemFrameKind::Zfreecnt => {
                let free = max_upload_bytes
                    .unwrap_or(u32::MAX as u64)
                    .min(u32::MAX as u64) as u32;
                write_header(transport, ZmodemHeader::new(ZmodemFrameKind::Zack, free)).await?;
            }
            _ => {
                write_header(transport, ZmodemHeader::new(ZmodemFrameKind::Znak, 0)).await?;
            }
        }
    }
}

async fn wait_for_receiver_ready<T: ByteTransport + ?Sized>(
    transport: &mut T,
) -> Result<(), TransferError> {
    for _ in 0..=DEFAULT_FRAME_RETRIES {
        write_header(transport, ZmodemHeader::new(ZmodemFrameKind::Zrqinit, 0)).await?;
        match read_header(transport, HEADER_TIMEOUT_SECS).await? {
            Some(header) if header.kind == ZmodemFrameKind::Zrinit => return Ok(()),
            Some(header)
                if header.kind == ZmodemFrameKind::Zabort
                    || header.kind == ZmodemFrameKind::Zcan =>
            {
                return Err(TransferError::Canceled);
            }
            Some(_) | None => {}
        }
    }
    Err(TransferError::Timeout)
}

async fn send_zfile_header<T: ByteTransport + ?Sized>(
    transport: &mut T,
    file: &ZmodemFile,
) -> Result<(), TransferError> {
    let metadata = ZfileMetadata {
        pathname: file.filename.clone(),
        size: Some(file.payload.len() as u64),
        mtime: Some(0),
        mode: Some(0o100644),
    };
    write_header(
        transport,
        ZmodemHeader {
            kind: ZmodemFrameKind::Zfile,
            flags: zfile_flags(),
        },
    )
    .await?;
    let subpacket = ZmodemDataSubpacket {
        data: metadata.encode(),
        frame_end: FrameEnd::Zcrcw,
    };
    transport
        .write_all(&encode_data_subpacket(&subpacket, true))
        .await?;
    transport.flush().await
}

async fn send_zmodem_payload<T: ByteTransport + ?Sized>(
    transport: &mut T,
    payload: &[u8],
    mut position: usize,
) -> Result<ZmodemTransferStats, TransferError> {
    let mut retries = 0_u32;
    'resend: loop {
        if position > payload.len() {
            return Err(TransferError::ProtocolError);
        }
        write_header(
            transport,
            ZmodemHeader::new(ZmodemFrameKind::Zdata, position as u32),
        )
        .await?;
        let mut offset = position;
        let mut bytes_since_ack = 0_usize;
        while offset < payload.len() {
            let end = (offset + MAX_SUBPACKET_BYTES).min(payload.len());
            let chunk = &payload[offset..end];
            offset = end;
            bytes_since_ack += chunk.len();
            let frame_end = if offset == payload.len() {
                FrameEnd::Zcrce
            } else if bytes_since_ack >= ACK_INTERVAL_BYTES {
                bytes_since_ack = 0;
                FrameEnd::Zcrcw
            } else {
                FrameEnd::Zcrcg
            };
            let subpacket = ZmodemDataSubpacket {
                data: chunk.to_vec(),
                frame_end,
            };
            transport
                .write_all(&encode_data_subpacket(&subpacket, true))
                .await?;
            transport.flush().await?;
            if frame_end == FrameEnd::Zcrcw {
                match read_required_header(transport).await? {
                    header if header.kind == ZmodemFrameKind::Zack => {}
                    header if header.kind == ZmodemFrameKind::Zrpos => {
                        retries = retries.saturating_add(1);
                        if retries > u32::from(DEFAULT_FRAME_RETRIES) {
                            return Err(TransferError::ProtocolError);
                        }
                        position = header.position() as usize;
                        continue 'resend;
                    }
                    header
                        if header.kind == ZmodemFrameKind::Zabort
                            || header.kind == ZmodemFrameKind::Zcan =>
                    {
                        return Err(TransferError::Canceled);
                    }
                    _ => return Err(TransferError::ProtocolError),
                }
            }
        }

        write_header(
            transport,
            ZmodemHeader::new(ZmodemFrameKind::Zeof, payload.len() as u32),
        )
        .await?;
        match read_required_header(transport).await? {
            header if header.kind == ZmodemFrameKind::Zrinit => {
                return Ok(ZmodemTransferStats {
                    files: 1,
                    bytes: payload.len() as u64,
                    retries,
                });
            }
            header if header.kind == ZmodemFrameKind::Zrpos => {
                retries = retries.saturating_add(1);
                if retries > u32::from(DEFAULT_FRAME_RETRIES) {
                    return Err(TransferError::ProtocolError);
                }
                position = header.position() as usize;
            }
            header
                if header.kind == ZmodemFrameKind::Zabort
                    || header.kind == ZmodemFrameKind::Zcan =>
            {
                return Err(TransferError::Canceled);
            }
            _ => return Err(TransferError::ProtocolError),
        }
    }
}

async fn receive_zmodem_payload<T: ByteTransport + ?Sized>(
    transport: &mut T,
    declared_size: u64,
    max_upload_bytes: Option<u64>,
) -> Result<(Vec<u8>, u32), TransferError> {
    let mut payload = Vec::new();
    let mut retries = 0_u32;

    loop {
        let header = read_required_header(transport).await?;
        match header.kind {
            ZmodemFrameKind::Zdata if header.position() as usize == payload.len() => loop {
                match read_data_subpacket_from_transport(transport, true, MAX_SUBPACKET_BYTES).await
                {
                    Ok(subpacket) => {
                        payload.extend_from_slice(&subpacket.data);
                        if let Some(max) = max_upload_bytes
                            && payload.len() as u64 > max
                        {
                            write_header(
                                transport,
                                ZmodemHeader::new(ZmodemFrameKind::Zferr, payload.len() as u32),
                            )
                            .await?;
                            return Err(TransferError::QuotaDenied);
                        }
                        match subpacket.frame_end {
                            FrameEnd::Zcrce => break,
                            FrameEnd::Zcrcg => {}
                            FrameEnd::Zcrcq | FrameEnd::Zcrcw => {
                                write_header(
                                    transport,
                                    ZmodemHeader::new(ZmodemFrameKind::Zack, payload.len() as u32),
                                )
                                .await?;
                            }
                        }
                    }
                    Err(TransferError::ProtocolError) => {
                        retries = retries.saturating_add(1);
                        if retries > u32::from(DEFAULT_FRAME_RETRIES) {
                            return Err(TransferError::ProtocolError);
                        }
                        write_header(
                            transport,
                            ZmodemHeader::new(ZmodemFrameKind::Zrpos, payload.len() as u32),
                        )
                        .await?;
                        break;
                    }
                    Err(error) => return Err(error),
                }
            },
            ZmodemFrameKind::Zeof if u64::from(header.position()) == payload.len() as u64 => {
                if payload.len() as u64 != declared_size {
                    return Err(TransferError::ProtocolError);
                }
                return Ok((payload, retries));
            }
            ZmodemFrameKind::Zabort | ZmodemFrameKind::Zcan => {
                return Err(TransferError::Canceled);
            }
            _ => {
                retries = retries.saturating_add(1);
                if retries > u32::from(DEFAULT_FRAME_RETRIES) {
                    return Err(TransferError::ProtocolError);
                }
                write_header(
                    transport,
                    ZmodemHeader::new(ZmodemFrameKind::Zrpos, payload.len() as u32),
                )
                .await?;
            }
        }
    }
}

async fn finish_send_session<T: ByteTransport + ?Sized>(
    transport: &mut T,
) -> Result<(), TransferError> {
    for _ in 0..=DEFAULT_FRAME_RETRIES {
        write_header(transport, ZmodemHeader::new(ZmodemFrameKind::Zfin, 0)).await?;
        match read_header(transport, HEADER_TIMEOUT_SECS).await? {
            Some(header) if header.kind == ZmodemFrameKind::Zfin => {
                let _ = transport.write_all(b"OO").await;
                let _ = transport.flush().await;
                return Ok(());
            }
            Some(header)
                if header.kind == ZmodemFrameKind::Zabort
                    || header.kind == ZmodemFrameKind::Zcan =>
            {
                return Err(TransferError::Canceled);
            }
            Some(_) | None => {}
        }
    }
    Err(TransferError::Timeout)
}

async fn write_header<T: ByteTransport + ?Sized>(
    transport: &mut T,
    header: ZmodemHeader,
) -> Result<(), TransferError> {
    let encoded = match header.kind {
        ZmodemFrameKind::Zrqinit | ZmodemFrameKind::Zrinit | ZmodemFrameKind::Zfin => {
            encode_hex_header(header)
        }
        _ => encode_binary32_header(header),
    };
    transport.write_all(&encoded).await?;
    transport.flush().await
}

async fn read_required_header<T: ByteTransport + ?Sized>(
    transport: &mut T,
) -> Result<ZmodemHeader, TransferError> {
    read_header(transport, HEADER_TIMEOUT_SECS)
        .await?
        .ok_or(TransferError::Timeout)
}

async fn read_header<T: ByteTransport + ?Sized>(
    transport: &mut T,
    timeout_secs: u64,
) -> Result<Option<ZmodemHeader>, TransferError> {
    loop {
        match read_raw_byte(transport, timeout_secs).await? {
            Some(ZPAD) => break,
            Some(_) => {}
            None => return Ok(None),
        }
    }

    let mut frame = vec![ZPAD];
    let next = read_required_raw_byte(transport, timeout_secs).await?;
    let marker = if next == ZPAD {
        frame.push(next);
        read_required_raw_byte(transport, timeout_secs).await?
    } else {
        next
    };
    if marker != ZDLE {
        return Err(TransferError::ProtocolError);
    }
    frame.push(marker);
    let encoding = read_required_raw_byte(transport, timeout_secs).await?;
    frame.push(encoding);

    match encoding {
        ZBIN => {
            let mut rest = read_raw_exact(transport, 7, timeout_secs).await?;
            frame.append(&mut rest);
            decode_binary_header(&frame).map(Some)
        }
        ZBIN32 => {
            let mut rest = read_raw_exact(transport, 9, timeout_secs).await?;
            frame.append(&mut rest);
            decode_binary32_header(&frame).map(Some)
        }
        ZHEX => {
            let mut rest = read_raw_exact(transport, 16, timeout_secs).await?;
            frame.append(&mut rest);
            Ok(Some(decode_hex_header(&frame)?))
        }
        _ => Err(TransferError::ProtocolError),
    }
}

async fn read_data_subpacket_from_transport<T: ByteTransport + ?Sized>(
    transport: &mut T,
    use_crc32: bool,
    max_data_len: usize,
) -> Result<ZmodemDataSubpacket, TransferError> {
    let mut data = Vec::new();
    loop {
        let byte = read_required_raw_byte(transport, DATA_TIMEOUT_SECS).await?;
        match byte {
            XON | XOFF => {}
            ZDLE => {
                let escaped = read_required_raw_byte(transport, DATA_TIMEOUT_SECS).await?;
                if let Some(frame_end) = FrameEnd::from_byte(escaped) {
                    let crc_len = if use_crc32 { 4 } else { 2 };
                    let crc_bytes =
                        read_decoded_exact(transport, crc_len, DATA_TIMEOUT_SECS).await?;
                    let mut payload = data.clone();
                    payload.push(escaped);
                    if use_crc32 {
                        let received_crc = u32::from_le_bytes([
                            crc_bytes[0],
                            crc_bytes[1],
                            crc_bytes[2],
                            crc_bytes[3],
                        ]);
                        if crc32_iso_hdlc(&payload) != received_crc {
                            return Err(TransferError::ProtocolError);
                        }
                    } else {
                        let received_crc = u16::from_be_bytes([crc_bytes[0], crc_bytes[1]]);
                        if crc16_xmodem(&payload) != received_crc {
                            return Err(TransferError::ProtocolError);
                        }
                    }
                    return Ok(ZmodemDataSubpacket { data, frame_end });
                }
                data.push(escaped ^ 0x40);
            }
            other => data.push(other),
        }

        if data.len() > max_data_len {
            return Err(TransferError::ProtocolError);
        }
    }
}

async fn read_decoded_exact<T: ByteTransport + ?Sized>(
    transport: &mut T,
    len: usize,
    timeout_secs: u64,
) -> Result<Vec<u8>, TransferError> {
    let mut bytes = Vec::with_capacity(len);
    while bytes.len() < len {
        let byte = read_required_raw_byte(transport, timeout_secs).await?;
        if byte == ZDLE {
            let escaped = read_required_raw_byte(transport, timeout_secs).await?;
            bytes.push(escaped ^ 0x40);
        } else {
            bytes.push(byte);
        }
    }
    Ok(bytes)
}

async fn read_raw_exact<T: ByteTransport + ?Sized>(
    transport: &mut T,
    len: usize,
    timeout_secs: u64,
) -> Result<Vec<u8>, TransferError> {
    let mut bytes = Vec::with_capacity(len);
    while bytes.len() < len {
        bytes.push(read_required_raw_byte(transport, timeout_secs).await?);
    }
    Ok(bytes)
}

async fn read_required_raw_byte<T: ByteTransport + ?Sized>(
    transport: &mut T,
    timeout_secs: u64,
) -> Result<u8, TransferError> {
    read_raw_byte(transport, timeout_secs)
        .await?
        .ok_or(TransferError::Timeout)
}

async fn read_raw_byte<T: ByteTransport + ?Sized>(
    transport: &mut T,
    timeout_secs: u64,
) -> Result<Option<u8>, TransferError> {
    match transport.read_byte(timeout_secs).await? {
        TransferRead::Byte(byte) => Ok(Some(byte)),
        TransferRead::TimedOut => Ok(None),
        TransferRead::Closed => Err(TransferError::Transport),
    }
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
    use std::future::Future;
    use std::pin::Pin;
    use tokio::sync::mpsc;

    struct MemoryByteTransport {
        rx: mpsc::UnboundedReceiver<u8>,
        tx: mpsc::UnboundedSender<u8>,
    }

    fn memory_pair() -> (MemoryByteTransport, MemoryByteTransport) {
        let (a_tx, b_rx) = mpsc::unbounded_channel();
        let (b_tx, a_rx) = mpsc::unbounded_channel();
        (
            MemoryByteTransport { rx: a_rx, tx: a_tx },
            MemoryByteTransport { rx: b_rx, tx: b_tx },
        )
    }

    impl crate::ByteTransport for MemoryByteTransport {
        fn read_byte(
            &mut self,
            timeout_secs: u64,
        ) -> Pin<Box<dyn Future<Output = Result<crate::TransferRead, TransferError>> + Send + '_>>
        {
            Box::pin(async move {
                match tokio::time::timeout(
                    std::time::Duration::from_secs(timeout_secs),
                    self.rx.recv(),
                )
                .await
                {
                    Ok(Some(byte)) => Ok(crate::TransferRead::Byte(byte)),
                    Ok(None) => Ok(crate::TransferRead::Closed),
                    Err(_) => Ok(crate::TransferRead::TimedOut),
                }
            })
        }

        fn write_all<'a>(
            &'a mut self,
            buf: &'a [u8],
        ) -> Pin<Box<dyn Future<Output = Result<(), TransferError>> + Send + 'a>> {
            Box::pin(async move {
                for &byte in buf {
                    self.tx.send(byte).map_err(|_| TransferError::Transport)?;
                }
                Ok(())
            })
        }

        fn flush(
            &mut self,
        ) -> Pin<Box<dyn Future<Output = Result<(), TransferError>> + Send + '_>> {
            Box::pin(async { Ok(()) })
        }
    }

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

    #[tokio::test]
    async fn zmodem_loopback_download_round_trips_payload() {
        let (mut sender, mut receiver) = memory_pair();
        let receive_task = tokio::spawn(async move {
            receive_zmodem_file(&mut receiver, Some(4096))
                .await
                .expect("receive zmodem file")
        });

        let stats = send_zmodem_file(&mut sender, "hello.bin", b"hello\xffzmodem")
            .await
            .expect("send zmodem file");
        let file = receive_task.await.expect("receiver task");

        assert_eq!(stats.files, 1);
        assert_eq!(stats.bytes, 12);
        assert_eq!(file.filename, "hello.bin");
        assert_eq!(file.payload, b"hello\xffzmodem");
    }

    #[tokio::test]
    async fn zmodem_loopback_batch_round_trips_multiple_files() {
        let (mut sender, mut receiver) = memory_pair();
        let receive_task = tokio::spawn(async move {
            receive_zmodem_files(&mut receiver, Some(4096))
                .await
                .expect("receive zmodem files")
        });
        let files = vec![
            ZmodemFile {
                filename: "one.txt".to_string(),
                payload: b"one".to_vec(),
            },
            ZmodemFile {
                filename: "two.bin".to_string(),
                payload: vec![0, 1, 2, 0xff],
            },
            ZmodemFile {
                filename: "three.dat".to_string(),
                payload: b"three".to_vec(),
            },
        ];

        let stats = send_zmodem_files(&mut sender, &files)
            .await
            .expect("send zmodem batch");
        let (received, receive_stats) = receive_task.await.expect("receiver task");

        assert_eq!(stats.files, 3);
        assert_eq!(receive_stats.files, 3);
        assert_eq!(received, files);
    }

    #[tokio::test]
    async fn zmodem_sender_reports_receiver_cancel() {
        let (mut sender, mut receiver) = memory_pair();
        let receiver_task = tokio::spawn(async move {
            let header = read_required_header(&mut receiver)
                .await
                .expect("read zrqinit");
            assert_eq!(header.kind, ZmodemFrameKind::Zrqinit);
            write_header(
                &mut receiver,
                ZmodemHeader::from_flags(ZmodemFrameKind::Zrinit, zrinit_flags(), 0, 0, 0),
            )
            .await
            .expect("write zrinit");
            let header = read_required_header(&mut receiver)
                .await
                .expect("read zfile");
            assert_eq!(header.kind, ZmodemFrameKind::Zfile);
            write_header(&mut receiver, ZmodemHeader::new(ZmodemFrameKind::Zcan, 0))
                .await
                .expect("write zcan");
        });

        let error = send_zmodem_file(&mut sender, "cancel.bin", b"cancel")
            .await
            .expect_err("send should be canceled");
        receiver_task.await.expect("receiver task");

        assert_eq!(error, TransferError::Canceled);
    }

    #[tokio::test]
    async fn zmodem_receiver_retries_after_bad_data_crc() {
        let (mut sender, mut receiver) = memory_pair();
        let receiver_task = tokio::spawn(async move {
            receive_zmodem_file(&mut receiver, Some(4096))
                .await
                .expect("receive after retry")
        });

        let header = read_required_header(&mut sender)
            .await
            .expect("read initial zrinit");
        assert_eq!(header.kind, ZmodemFrameKind::Zrinit);

        let metadata = ZfileMetadata {
            pathname: "retry.bin".to_string(),
            size: Some(5),
            mtime: Some(0),
            mode: Some(0o100644),
        };
        write_header(
            &mut sender,
            ZmodemHeader {
                kind: ZmodemFrameKind::Zfile,
                flags: zfile_flags(),
            },
        )
        .await
        .expect("write zfile");
        sender
            .write_all(&encode_data_subpacket(
                &ZmodemDataSubpacket {
                    data: metadata.encode(),
                    frame_end: FrameEnd::Zcrcw,
                },
                true,
            ))
            .await
            .expect("write metadata");
        assert_eq!(
            read_required_header(&mut sender)
                .await
                .expect("read zrpos")
                .kind,
            ZmodemFrameKind::Zrpos
        );

        write_header(&mut sender, ZmodemHeader::new(ZmodemFrameKind::Zdata, 0))
            .await
            .expect("write zdata");
        let mut bad = encode_data_subpacket(
            &ZmodemDataSubpacket {
                data: b"retry".to_vec(),
                frame_end: FrameEnd::Zcrce,
            },
            true,
        );
        let last = bad.last_mut().expect("bad frame has crc");
        *last ^= 0x55;
        sender.write_all(&bad).await.expect("write bad data");
        let retry_header = read_required_header(&mut sender)
            .await
            .expect("read retry zrpos");
        assert_eq!(retry_header.kind, ZmodemFrameKind::Zrpos);
        assert_eq!(retry_header.position(), 0);

        write_header(&mut sender, ZmodemHeader::new(ZmodemFrameKind::Zdata, 0))
            .await
            .expect("write retry zdata");
        sender
            .write_all(&encode_data_subpacket(
                &ZmodemDataSubpacket {
                    data: b"retry".to_vec(),
                    frame_end: FrameEnd::Zcrce,
                },
                true,
            ))
            .await
            .expect("write good data");
        write_header(&mut sender, ZmodemHeader::new(ZmodemFrameKind::Zeof, 5))
            .await
            .expect("write zeof");
        assert_eq!(
            read_required_header(&mut sender)
                .await
                .expect("read post-file zrinit")
                .kind,
            ZmodemFrameKind::Zrinit
        );
        write_header(&mut sender, ZmodemHeader::new(ZmodemFrameKind::Zfin, 0))
            .await
            .expect("write zfin");
        assert_eq!(
            read_required_header(&mut sender)
                .await
                .expect("read finish zfin")
                .kind,
            ZmodemFrameKind::Zfin
        );

        let file = receiver_task.await.expect("receiver task");
        assert_eq!(file.filename, "retry.bin");
        assert_eq!(file.payload, b"retry");
    }
}
