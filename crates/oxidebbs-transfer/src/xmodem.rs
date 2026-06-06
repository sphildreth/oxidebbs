//! XMODEM-CRC sender and receiver.

use crate::crc::crc16_xmodem;
use crate::{ByteTransport, TransferError, TransferRead};

const SOH: u8 = 0x01;
const EOT: u8 = 0x04;
const ACK: u8 = 0x06;
const NAK: u8 = 0x15;
const CAN: u8 = 0x18;
const CRC_REQUEST: u8 = b'C';
const CPMEOF: u8 = 0x1A;
const BLOCK_SIZE: usize = 128;
const DEFAULT_MAX_RETRIES: u8 = 10;
const CONTROL_TIMEOUT_SECS: u64 = 1;
const INITIAL_RECEIVER_TIMEOUT_SECS: u64 = 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SendMode {
    Crc,
    Checksum,
}

/// Send bytes to a caller using XMODEM-CRC.
///
/// The receiver should start the transfer by sending `C`. For terminal clients
/// that expose only classic XMODEM receive and send `NAK`, the sender falls
/// back to the one-byte checksum variant. Payloads are split into 128-byte
/// blocks and padded with CP/M EOF bytes for the final block.
///
/// # Errors
///
/// Returns a transfer error on malformed handshakes, retry exhaustion, caller
/// cancellation, or underlying transport failures.
pub async fn send_xmodem_crc<T: ByteTransport + ?Sized>(
    transport: &mut T,
    payload: &[u8],
) -> Result<(), TransferError> {
    send_xmodem_crc_with_retries(transport, payload, DEFAULT_MAX_RETRIES).await
}

/// Send bytes to a caller with an explicit retry limit.
///
/// # Errors
///
/// Returns a transfer error on malformed handshakes, retry exhaustion, caller
/// cancellation, or underlying transport failures.
pub async fn send_xmodem_crc_with_retries<T: ByteTransport + ?Sized>(
    transport: &mut T,
    payload: &[u8],
    max_retries: u8,
) -> Result<(), TransferError> {
    let send_mode = wait_for_receiver_request(transport, max_retries).await?;

    let mut block_number = 1_u8;
    for chunk in payload.chunks(BLOCK_SIZE) {
        let mut block = [CPMEOF; BLOCK_SIZE];
        block[..chunk.len()].copy_from_slice(chunk);
        send_block_with_retries(transport, block_number, &block, send_mode, max_retries).await?;
        block_number = block_number.wrapping_add(1);
    }

    send_eot_with_retries(transport, max_retries).await
}

/// Receive bytes from a caller using XMODEM-CRC and trim trailing CP/M EOF
/// padding.
///
/// Use [`receive_xmodem_crc_with_size`] when the expected file size is known and
/// binary payloads may legitimately end in `0x1A`.
///
/// # Errors
///
/// Returns a transfer error on malformed frames, retry exhaustion, caller
/// cancellation, or underlying transport failures.
pub async fn receive_xmodem_crc<T: ByteTransport + ?Sized>(
    transport: &mut T,
) -> Result<Vec<u8>, TransferError> {
    let mut received = receive_xmodem_crc_padded(transport).await?;
    while received.last().copied() == Some(CPMEOF) {
        received.pop();
    }
    Ok(received)
}

/// Receive bytes from a caller using XMODEM-CRC and truncate to an expected
/// size.
///
/// # Errors
///
/// Returns a transfer error on malformed frames, retry exhaustion, caller
/// cancellation, underlying transport failures, or short payloads.
pub async fn receive_xmodem_crc_with_size<T: ByteTransport + ?Sized>(
    transport: &mut T,
    expected_size: usize,
) -> Result<Vec<u8>, TransferError> {
    let mut received = receive_xmodem_crc_padded(transport).await?;
    if received.len() < expected_size {
        return Err(TransferError::ProtocolError);
    }
    received.truncate(expected_size);
    Ok(received)
}

async fn wait_for_receiver_request<T: ByteTransport + ?Sized>(
    transport: &mut T,
    max_retries: u8,
) -> Result<SendMode, TransferError> {
    let attempts = INITIAL_RECEIVER_TIMEOUT_SECS.max(u64::from(max_retries) + 1);
    for _ in 0..attempts {
        match read_control_byte_with_timeout(transport, CONTROL_TIMEOUT_SECS).await? {
            Some(CRC_REQUEST) => return Ok(SendMode::Crc),
            Some(NAK) => return Ok(SendMode::Checksum),
            Some(CAN) => return Err(TransferError::Canceled),
            Some(_) | None => {}
        }
    }
    Err(TransferError::Timeout)
}

async fn send_block_with_retries<T: ByteTransport + ?Sized>(
    transport: &mut T,
    block_number: u8,
    block: &[u8; BLOCK_SIZE],
    send_mode: SendMode,
    max_retries: u8,
) -> Result<(), TransferError> {
    let frame = build_block_frame(block_number, block, send_mode);
    for _ in 0..=max_retries {
        transport.write_all(&frame).await?;
        transport.flush().await?;
        match read_control_byte(transport).await? {
            Some(ACK) => return Ok(()),
            Some(NAK) | None => continue,
            Some(CAN) => return Err(TransferError::Canceled),
            Some(_) => return Err(TransferError::ProtocolError),
        }
    }
    Err(TransferError::Timeout)
}

async fn send_eot_with_retries<T: ByteTransport + ?Sized>(
    transport: &mut T,
    max_retries: u8,
) -> Result<(), TransferError> {
    for _ in 0..=max_retries {
        transport.write_all(&[EOT]).await?;
        transport.flush().await?;
        match read_control_byte(transport).await? {
            Some(ACK) => return Ok(()),
            Some(NAK) | None => continue,
            Some(CAN) => return Err(TransferError::Canceled),
            Some(_) => return Err(TransferError::ProtocolError),
        }
    }
    Err(TransferError::Timeout)
}

async fn receive_xmodem_crc_padded<T: ByteTransport + ?Sized>(
    transport: &mut T,
) -> Result<Vec<u8>, TransferError> {
    let mut out = Vec::new();
    let mut expected_block = 1_u8;
    let mut retries = 0_u8;

    transport.write_all(&[CRC_REQUEST]).await?;
    transport.flush().await?;

    loop {
        match read_control_byte(transport).await? {
            Some(SOH) => {
                let block_number = read_required_byte(transport).await?;
                let block_complement = read_required_byte(transport).await?;
                let mut block = [0_u8; BLOCK_SIZE];
                for byte in &mut block {
                    *byte = read_required_byte(transport).await?;
                }
                let crc_high = read_required_byte(transport).await?;
                let crc_low = read_required_byte(transport).await?;
                let received_crc = u16::from_be_bytes([crc_high, crc_low]);
                let valid_number = block_number ^ block_complement == 0xFF;
                let valid_crc = crc16_xmodem(&block) == received_crc;

                if valid_number && valid_crc && block_number == expected_block {
                    out.extend_from_slice(&block);
                    expected_block = expected_block.wrapping_add(1);
                    retries = 0;
                    transport.write_all(&[ACK]).await?;
                    transport.flush().await?;
                } else if valid_number
                    && valid_crc
                    && block_number == expected_block.wrapping_sub(1)
                {
                    transport.write_all(&[ACK]).await?;
                    transport.flush().await?;
                } else {
                    retries = retries.saturating_add(1);
                    if retries > DEFAULT_MAX_RETRIES {
                        return Err(TransferError::ProtocolError);
                    }
                    transport.write_all(&[NAK]).await?;
                    transport.flush().await?;
                }
            }
            Some(EOT) => {
                transport.write_all(&[ACK]).await?;
                transport.flush().await?;
                return Ok(out);
            }
            Some(CAN) => return Err(TransferError::Canceled),
            Some(_) => return Err(TransferError::ProtocolError),
            None => {
                retries = retries.saturating_add(1);
                if retries > DEFAULT_MAX_RETRIES {
                    return Err(TransferError::Timeout);
                }
                transport.write_all(&[CRC_REQUEST]).await?;
                transport.flush().await?;
            }
        }
    }
}

fn build_block_frame(block_number: u8, block: &[u8; BLOCK_SIZE], send_mode: SendMode) -> Vec<u8> {
    let mut frame = Vec::with_capacity(BLOCK_SIZE + 5);
    frame.push(SOH);
    frame.push(block_number);
    frame.push(0xFF - block_number);
    frame.extend_from_slice(block);
    match send_mode {
        SendMode::Crc => frame.extend_from_slice(&crc16_xmodem(block).to_be_bytes()),
        SendMode::Checksum => frame.push(xmodem_checksum(block)),
    }
    frame
}

fn xmodem_checksum(block: &[u8; BLOCK_SIZE]) -> u8 {
    block
        .iter()
        .fold(0_u8, |checksum, byte| checksum.wrapping_add(*byte))
}

async fn read_required_byte<T: ByteTransport + ?Sized>(
    transport: &mut T,
) -> Result<u8, TransferError> {
    read_control_byte(transport)
        .await?
        .ok_or(TransferError::Timeout)
}

async fn read_control_byte<T: ByteTransport + ?Sized>(
    transport: &mut T,
) -> Result<Option<u8>, TransferError> {
    read_control_byte_with_timeout(transport, CONTROL_TIMEOUT_SECS).await
}

async fn read_control_byte_with_timeout<T: ByteTransport + ?Sized>(
    transport: &mut T,
    timeout_secs: u64,
) -> Result<Option<u8>, TransferError> {
    match transport.read_byte(timeout_secs).await? {
        TransferRead::Byte(byte) => Ok(Some(byte)),
        TransferRead::TimedOut => Ok(None),
        TransferRead::Closed => Err(TransferError::Transport),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::future::Future;
    use std::pin::Pin;

    use super::*;

    #[derive(Debug, Default)]
    struct ScriptTransport {
        reads: VecDeque<TransferRead>,
        writes: Vec<u8>,
    }

    impl ScriptTransport {
        fn with_reads(reads: impl IntoIterator<Item = TransferRead>) -> Self {
            Self {
                reads: reads.into_iter().collect(),
                writes: Vec::new(),
            }
        }
    }

    impl ByteTransport for ScriptTransport {
        fn read_byte(
            &mut self,
            _timeout_secs: u64,
        ) -> Pin<Box<dyn Future<Output = Result<TransferRead, TransferError>> + Send + '_>>
        {
            Box::pin(async move { Ok(self.reads.pop_front().unwrap_or(TransferRead::TimedOut)) })
        }

        fn write_all(
            &mut self,
            buf: &[u8],
        ) -> Pin<Box<dyn Future<Output = Result<(), TransferError>> + Send + '_>> {
            let owned = buf.to_vec();
            Box::pin(async move {
                self.writes.extend_from_slice(&owned);
                Ok(())
            })
        }

        fn flush(
            &mut self,
        ) -> Pin<Box<dyn Future<Output = Result<(), TransferError>> + Send + '_>> {
            Box::pin(async { Ok(()) })
        }
    }

    #[tokio::test]
    async fn sender_builds_crc_block_and_finishes_with_eot() {
        let mut transport = ScriptTransport::with_reads([
            TransferRead::Byte(CRC_REQUEST),
            TransferRead::Byte(ACK),
            TransferRead::Byte(ACK),
        ]);

        send_xmodem_crc(&mut transport, b"hello")
            .await
            .expect("send");

        assert_eq!(transport.writes[0], SOH);
        assert_eq!(transport.writes[1], 1);
        assert_eq!(transport.writes[2], 0xFE);
        assert_eq!(&transport.writes[3..8], b"hello");
        assert_eq!(transport.writes[133], EOT);
    }

    #[tokio::test]
    async fn sender_retries_rejected_block() {
        let mut transport = ScriptTransport::with_reads([
            TransferRead::Byte(CRC_REQUEST),
            TransferRead::Byte(NAK),
            TransferRead::Byte(ACK),
            TransferRead::Byte(ACK),
        ]);

        send_xmodem_crc(&mut transport, b"retry")
            .await
            .expect("send");

        assert_eq!(transport.writes.len(), 267);
        assert_eq!(transport.writes[0], SOH);
        assert_eq!(transport.writes[133], SOH);
        assert_eq!(transport.writes[266], EOT);
    }

    #[tokio::test]
    async fn receiver_accepts_valid_block_and_truncates_to_expected_size() {
        let mut block = [CPMEOF; BLOCK_SIZE];
        block[..5].copy_from_slice(b"hello");
        let frame = build_block_frame(1, &block, SendMode::Crc);
        let reads = frame
            .into_iter()
            .chain([EOT])
            .map(TransferRead::Byte)
            .collect::<Vec<_>>();
        let mut transport = ScriptTransport::with_reads(reads);

        let received = receive_xmodem_crc_with_size(&mut transport, 5)
            .await
            .expect("receive");

        assert_eq!(received, b"hello");
        assert_eq!(transport.writes, [CRC_REQUEST, ACK, ACK]);
    }

    #[tokio::test]
    async fn receiver_rejects_bad_crc() {
        let block = [0_u8; BLOCK_SIZE];
        let frame = build_block_frame(1, &block, 0xFFFF);
        let reads = frame
            .into_iter()
            .map(TransferRead::Byte)
            .chain(std::iter::repeat_n(
                TransferRead::TimedOut,
                usize::from(DEFAULT_MAX_RETRIES) + 1,
            ))
            .collect::<Vec<_>>();
        let mut transport = ScriptTransport::with_reads(reads);

        let error = receive_xmodem_crc(&mut transport)
            .await
            .expect_err("bad crc");

        assert_eq!(error, TransferError::Timeout);
        assert!(transport.writes.contains(&NAK));
    }

    #[tokio::test]
    async fn sender_handles_cancel() {
        let mut transport =
            ScriptTransport::with_reads([TransferRead::Byte(CRC_REQUEST), TransferRead::Byte(CAN)]);

        let error = send_xmodem_crc(&mut transport, b"cancel")
            .await
            .expect_err("cancel");

        assert_eq!(error, TransferError::Canceled);
    }
}
