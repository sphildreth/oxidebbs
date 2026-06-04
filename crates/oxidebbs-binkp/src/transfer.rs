use std::io::{Read, Write};

use crate::error::BinkpError;
use crate::frame::{
    BinkpFrame, FrameType, M_EOB, M_ERR, M_FILE, M_GOT, M_NUL, M_SKIP, read_frame, write_frame,
};

const MAX_DATA_FRAME: usize = 0x7FFF;

/// A local file offered through a BinkP session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BinkpOutboundFile {
    pub name: String,
    pub mtime: u64,
    pub bytes: Vec<u8>,
}

impl BinkpOutboundFile {
    /// Build a BinkP outbound file offer.
    ///
    /// # Errors
    ///
    /// Returns a protocol error when the BinkP file name is unsafe for a
    /// session-level file offer.
    pub fn new(name: impl Into<String>, mtime: u64, bytes: Vec<u8>) -> Result<Self, BinkpError> {
        let name = name.into();
        validate_binkp_file_name(&name)?;
        Ok(Self { name, mtime, bytes })
    }
}

/// A file received through a BinkP session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BinkpInboundFile {
    pub name: String,
    pub mtime: u64,
    pub declared_size: u64,
    pub bytes: Vec<u8>,
}

/// Send one BinkP file offer and its data frames.
///
/// # Errors
///
/// Returns protocol errors for unsafe names or oversized metadata, and I/O
/// errors from the stream.
pub fn send_file<W: Write>(writer: &mut W, file: &BinkpOutboundFile) -> Result<(), BinkpError> {
    validate_binkp_file_name(&file.name)?;
    let descriptor = format!("{} {} {} 0", file.name, file.bytes.len(), file.mtime);
    write_frame(
        writer,
        &BinkpFrame::command(M_FILE, descriptor.into_bytes()),
    )?;
    for chunk in file.bytes.chunks(MAX_DATA_FRAME) {
        write_frame(writer, &BinkpFrame::data(chunk.to_vec()))?;
    }
    Ok(())
}

/// Send zero or more BinkP file offers and finish with `M_EOB`.
///
/// An empty batch is valid and sends only the end-of-batch marker, allowing an
/// empty poll to complete cleanly at the stream layer.
///
/// # Errors
///
/// Returns protocol errors for invalid file metadata and I/O errors from the
/// stream.
pub fn send_batch<W: Write>(writer: &mut W, files: &[BinkpOutboundFile]) -> Result<(), BinkpError> {
    for file in files {
        validate_binkp_file_name(&file.name)?;
    }
    for file in files {
        send_file(writer, file)?;
    }
    send_end_of_batch(writer)
}

/// Send the BinkP end-of-batch marker.
///
/// # Errors
///
/// Returns I/O errors from the stream.
pub fn send_end_of_batch<W: Write>(writer: &mut W) -> Result<(), BinkpError> {
    write_frame(writer, &BinkpFrame::command(M_EOB, Vec::new()))
}

/// Receive the next BinkP file, acknowledging it with `M_GOT`.
///
/// Returns `Ok(None)` when the peer sends `M_EOB`.
///
/// # Errors
///
/// Returns protocol errors for malformed offers, unexpected commands, size
/// mismatches, or unsafe filenames, and I/O errors from the stream.
pub fn receive_next_file<S: Read + Write>(
    stream: &mut S,
) -> Result<Option<BinkpInboundFile>, BinkpError> {
    let offer = read_frame(stream)?;
    if offer.frame_type != FrameType::Command {
        return Err(BinkpError::Protocol(
            "BinkP peer sent data before M_FILE".to_string(),
        ));
    }

    match offer.command {
        M_EOB => return Ok(None),
        M_FILE => {}
        M_ERR => {
            return Err(BinkpError::Protocol(
                "BinkP peer reported an error before file transfer".to_string(),
            ));
        }
        command => {
            return Err(BinkpError::Protocol(format!(
                "unexpected BinkP command {command} before file transfer"
            )));
        }
    }

    let descriptor = parse_file_descriptor(&offer.payload)?;
    let expected_len = usize::try_from(descriptor.declared_size).map_err(|_| {
        BinkpError::Protocol("BinkP file size exceeds this platform's capacity".to_string())
    })?;
    let mut bytes = Vec::with_capacity(expected_len);

    while bytes.len() < expected_len {
        let frame = read_frame(stream)?;
        match frame.frame_type {
            FrameType::Data => {
                bytes.extend_from_slice(&frame.payload);
                if bytes.len() > expected_len {
                    return Err(BinkpError::Protocol(format!(
                        "BinkP file {:?} exceeded declared size",
                        descriptor.name
                    )));
                }
            }
            FrameType::Command => match frame.command {
                M_NUL => {}
                M_SKIP => {
                    return Err(BinkpError::Protocol(format!(
                        "BinkP peer skipped file {:?}",
                        descriptor.name
                    )));
                }
                M_EOB => {
                    return Err(BinkpError::Protocol(format!(
                        "BinkP batch ended before file {:?} completed",
                        descriptor.name
                    )));
                }
                M_ERR => {
                    return Err(BinkpError::Protocol(format!(
                        "BinkP peer reported an error while sending {:?}",
                        descriptor.name
                    )));
                }
                command => {
                    return Err(BinkpError::Protocol(format!(
                        "unexpected BinkP command {command} while receiving file"
                    )));
                }
            },
        }
    }

    write_frame(
        stream,
        &BinkpFrame::command(M_GOT, descriptor.name.as_bytes().to_vec()),
    )?;

    Ok(Some(BinkpInboundFile {
        name: descriptor.name,
        mtime: descriptor.mtime,
        declared_size: descriptor.declared_size,
        bytes,
    }))
}

/// Receive BinkP files until the peer sends `M_EOB`.
///
/// This helper owns only the command/data-frame loop. Runtime client/server
/// layers are still responsible for connection lifetime, TLS, spool placement,
/// retry policy, and poll logging.
///
/// # Errors
///
/// Returns protocol errors for malformed file exchange or I/O errors from the
/// stream.
pub fn receive_batch<S: Read + Write>(stream: &mut S) -> Result<Vec<BinkpInboundFile>, BinkpError> {
    let mut files = Vec::new();
    while let Some(file) = receive_next_file(stream)? {
        files.push(file);
    }
    Ok(files)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FileDescriptor {
    name: String,
    declared_size: u64,
    mtime: u64,
}

fn parse_file_descriptor(payload: &[u8]) -> Result<FileDescriptor, BinkpError> {
    let text = String::from_utf8(payload.to_vec())
        .map_err(|_| BinkpError::Protocol("BinkP M_FILE payload is not UTF-8".to_string()))?;
    let mut parts = text.split_whitespace();
    let name = parts
        .next()
        .ok_or_else(|| BinkpError::Protocol("BinkP M_FILE is missing filename".to_string()))?
        .to_string();
    validate_binkp_file_name(&name)?;
    let declared_size = parse_u64_field(parts.next(), "size")?;
    let mtime = parse_u64_field(parts.next(), "mtime")?;
    Ok(FileDescriptor {
        name,
        declared_size,
        mtime,
    })
}

fn parse_u64_field(value: Option<&str>, field: &str) -> Result<u64, BinkpError> {
    let value = value
        .ok_or_else(|| BinkpError::Protocol(format!("BinkP M_FILE is missing {field} field")))?;
    value
        .parse::<u64>()
        .map_err(|_| BinkpError::Protocol(format!("BinkP M_FILE has invalid {field} field")))
}

fn validate_binkp_file_name(name: &str) -> Result<(), BinkpError> {
    if name.is_empty()
        || name.contains('/')
        || name.contains('\\')
        || name.contains('\0')
        || name.chars().any(char::is_whitespace)
    {
        return Err(BinkpError::Protocol(format!(
            "unsafe BinkP file name {name:?}"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{self, Cursor};

    struct ScriptStream {
        reads: Cursor<Vec<u8>>,
        writes: Vec<u8>,
    }

    impl ScriptStream {
        fn new(reads: Vec<u8>) -> Self {
            Self {
                reads: Cursor::new(reads),
                writes: Vec::new(),
            }
        }
    }

    impl Read for ScriptStream {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            self.reads.read(buf)
        }
    }

    impl Write for ScriptStream {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.writes.extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn sends_and_receives_one_file_with_acknowledgement() {
        let file = BinkpOutboundFile::new("00000001.pkt", 1234, b"packet bytes".to_vec())
            .expect("valid file");
        let mut wire = Vec::new();
        send_file(&mut wire, &file).expect("send file");
        send_end_of_batch(&mut wire).expect("send eob");
        let mut stream = ScriptStream::new(wire);

        let received = receive_next_file(&mut stream)
            .expect("receive file")
            .expect("file");
        let end = receive_next_file(&mut stream).expect("receive eob");

        assert_eq!(received.name, "00000001.pkt");
        assert_eq!(received.mtime, 1234);
        assert_eq!(received.declared_size, 12);
        assert_eq!(received.bytes, b"packet bytes");
        assert!(end.is_none());

        let got = read_frame(&mut stream.writes.as_slice()).expect("read got");
        assert_eq!(got.frame_type, FrameType::Command);
        assert_eq!(got.command, M_GOT);
        assert_eq!(got.payload, b"00000001.pkt");
    }

    #[test]
    fn empty_batch_writes_and_receives_eob_only() {
        let mut wire = Vec::new();
        send_batch(&mut wire, &[]).expect("send empty batch");
        let mut stream = ScriptStream::new(wire);

        let files = receive_batch(&mut stream).expect("receive empty batch");

        assert!(files.is_empty());
        assert!(stream.writes.is_empty());
    }

    #[test]
    fn batch_round_trips_multiple_files_and_acknowledges_each() {
        let files = vec![
            BinkpOutboundFile::new("00000001.pkt", 1234, b"first".to_vec()).expect("first"),
            BinkpOutboundFile::new("00000002.pkt", 1235, b"second".to_vec()).expect("second"),
        ];
        let mut wire = Vec::new();
        send_batch(&mut wire, &files).expect("send batch");
        let mut stream = ScriptStream::new(wire);

        let received = receive_batch(&mut stream).expect("receive batch");

        assert_eq!(received.len(), 2);
        assert_eq!(received[0].name, "00000001.pkt");
        assert_eq!(received[0].bytes, b"first");
        assert_eq!(received[1].name, "00000002.pkt");
        assert_eq!(received[1].bytes, b"second");

        let mut acks = Cursor::new(stream.writes);
        let first_ack = read_frame(&mut acks).expect("read first ack");
        assert_eq!(first_ack.command, M_GOT);
        assert_eq!(first_ack.payload, b"00000001.pkt");
        let second_ack = read_frame(&mut acks).expect("read second ack");
        assert_eq!(second_ack.command, M_GOT);
        assert_eq!(second_ack.payload, b"00000002.pkt");
    }

    #[test]
    fn large_batch_file_is_chunked_and_reassembled() {
        let bytes = vec![b'x'; MAX_DATA_FRAME + 5];
        let file = BinkpOutboundFile::new("large.pkt", 1234, bytes.clone()).expect("valid file");
        let mut wire = Vec::new();
        send_batch(&mut wire, std::slice::from_ref(&file)).expect("send batch");

        let mut frames = Cursor::new(wire.clone());
        let offer = read_frame(&mut frames).expect("offer");
        let first_data = read_frame(&mut frames).expect("first data");
        let second_data = read_frame(&mut frames).expect("second data");
        assert_eq!(offer.command, M_FILE);
        assert_eq!(first_data.payload.len(), MAX_DATA_FRAME);
        assert_eq!(second_data.payload.len(), 5);

        let mut stream = ScriptStream::new(wire);
        let received = receive_batch(&mut stream).expect("receive batch");

        assert_eq!(received.len(), 1);
        assert_eq!(
            received[0].declared_size,
            u64::try_from(bytes.len()).unwrap()
        );
        assert_eq!(received[0].bytes, bytes);
    }

    #[test]
    fn batch_validates_all_outbound_names_before_writing() {
        let valid = BinkpOutboundFile::new("good.pkt", 0, Vec::new()).expect("valid file");
        let mut invalid =
            BinkpOutboundFile::new("also-good.pkt", 0, Vec::new()).expect("initially valid file");
        invalid.name = "../bad.pkt".to_string();
        let mut wire = Vec::new();

        let error = send_batch(&mut wire, &[valid, invalid]).expect_err("unsafe batch item");

        assert!(matches!(error, BinkpError::Protocol(_)));
        assert!(wire.is_empty());
    }

    #[test]
    fn rejects_path_like_file_names() {
        let error = BinkpOutboundFile::new("../bad.pkt", 0, Vec::new()).expect_err("unsafe name");

        assert!(matches!(error, BinkpError::Protocol(_)));
    }

    #[test]
    fn rejects_short_file_data() {
        let mut wire = Vec::new();
        write_frame(
            &mut wire,
            &BinkpFrame::command(M_FILE, b"00000001.pkt 12 1234 0".to_vec()),
        )
        .expect("offer");
        write_frame(&mut wire, &BinkpFrame::data(b"short".to_vec())).expect("data");
        write_frame(&mut wire, &BinkpFrame::command(M_EOB, Vec::new())).expect("eob");
        let mut stream = ScriptStream::new(wire);

        let error = receive_next_file(&mut stream).expect_err("short data");

        assert!(matches!(error, BinkpError::Protocol(_)));
    }
}
