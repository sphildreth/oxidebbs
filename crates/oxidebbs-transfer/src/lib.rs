//! File transfer protocols for BBS callers (XMODEM-CRC and ZMODEM).

pub mod adapter;
pub mod crc;
pub mod path;
pub mod xmodem;
pub mod zmodem;

use std::future::Future;
use std::pin::Pin;

use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq, Clone)]
pub enum TransferError {
    #[error("protocol framing error")]
    ProtocolError,
    #[error("transfer timed out")]
    Timeout,
    #[error("transport error")]
    Transport,
    #[error("I/O error: {0}")]
    IoError(String),
    #[error("security policy denied transfer")]
    SecurityDenied,
    #[error("transfer would exceed quota")]
    QuotaDenied,
    #[error("transfer cancelled by user")]
    Canceled,
    #[error("protocol or feature not supported")]
    Unsupported,
    #[error("path is invalid or not allowed")]
    PathInvalid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransferProtocol {
    XmodemCrc,
    Zmodem,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransferDirection {
    SendToCaller,
    ReceiveFromCaller,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransferRead {
    Byte(u8),
    TimedOut,
    Closed,
}

pub trait ByteTransport {
    fn read_byte(
        &mut self,
        timeout_secs: u64,
    ) -> Pin<Box<dyn Future<Output = Result<TransferRead, TransferError>> + Send + '_>>;

    fn write_all<'a>(
        &'a mut self,
        buf: &'a [u8],
    ) -> Pin<Box<dyn Future<Output = Result<(), TransferError>> + Send + 'a>>;

    fn flush(&mut self) -> Pin<Box<dyn Future<Output = Result<(), TransferError>> + Send + '_>>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransferEvent {
    Started,
    FileStarted { filename: String, size: u64 },
    BytesAdvanced { bytes: u64, total: u64 },
    Retry,
    Resumed,
    Skipped,
    Canceled,
    Completed,
    Error(TransferError),
}

pub use adapter::TransportAdapter;
pub use path::{safe_upload_path, sanitize_filename, validate_path_within_base};
