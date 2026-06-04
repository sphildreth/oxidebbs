//! Adapter to bridge oxidebbs-telnet's Transport trait to oxidebbs-transfer's ByteTransport trait.

use std::future::Future;
use std::pin::Pin;

use crate::{ByteTransport, TransferError, TransferRead};

/// Adapter that wraps a telnet Transport implementation to provide the ByteTransport interface.
///
/// This allows file transfer protocols (XMODEM-CRC, ZMODEM) to work over any
/// transport layer (TCP/telnet, serial, loopback) that implements the Transport trait.
pub struct TransportAdapter<T> {
    transport: T,
}

impl<T> TransportAdapter<T> {
    /// Create a new adapter wrapping the given transport.
    pub fn new(transport: T) -> Self {
        Self { transport }
    }

    /// Get a reference to the underlying transport.
    pub fn transport(&self) -> &T {
        &self.transport
    }

    /// Get a mutable reference to the underlying transport.
    pub fn transport_mut(&mut self) -> &mut T {
        &mut self.transport
    }

    /// Consume the adapter and return the underlying transport.
    pub fn into_inner(self) -> T {
        self.transport
    }
}

impl<T> ByteTransport for TransportAdapter<T>
where
    T: oxidebbs_telnet::Transport + Send,
{
    fn read_byte(
        &mut self,
        timeout_secs: u64,
    ) -> Pin<Box<dyn Future<Output = Result<TransferRead, TransferError>> + Send + '_>> {
        Box::pin(async move {
            let timeout_duration = tokio::time::Duration::from_secs(timeout_secs);

            match tokio::time::timeout(timeout_duration, self.transport.read_byte()).await {
                Ok(Ok(Some(byte))) => Ok(TransferRead::Byte(byte)),
                Ok(Ok(None)) => Ok(TransferRead::Closed),
                Ok(Err(_)) => Err(TransferError::Transport),
                Err(_) => Ok(TransferRead::TimedOut),
            }
        })
    }

    fn write_all<'a>(
        &'a mut self,
        buf: &'a [u8],
    ) -> Pin<Box<dyn Future<Output = Result<(), TransferError>> + Send + 'a>> {
        Box::pin(async move {
            self.transport
                .write_all(buf)
                .await
                .map_err(|_| TransferError::Transport)
        })
    }

    fn flush(&mut self) -> Pin<Box<dyn Future<Output = Result<(), TransferError>> + Send + '_>> {
        Box::pin(async move {
            // Transport trait doesn't have a separate flush method,
            // write_all already flushes in TcpTransport implementation
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxidebbs_telnet::LoopbackTransport;

    #[tokio::test]
    async fn adapter_reads_byte_from_transport() {
        let (transport, handle) = LoopbackTransport::new();

        handle.write_bytes(b"ABC").expect("write test data");

        let mut adapter = TransportAdapter::new(transport);

        let byte1 = adapter.read_byte(1).await.expect("read byte 1");
        assert_eq!(byte1, TransferRead::Byte(b'A'));

        let byte2 = adapter.read_byte(1).await.expect("read byte 2");
        assert_eq!(byte2, TransferRead::Byte(b'B'));

        let byte3 = adapter.read_byte(1).await.expect("read byte 3");
        assert_eq!(byte3, TransferRead::Byte(b'C'));
    }

    #[tokio::test]
    async fn adapter_writes_to_transport() {
        let (transport, mut handle) = LoopbackTransport::new();

        let mut adapter = TransportAdapter::new(transport);

        adapter.write_all(b"Hello").await.expect("write");

        let output = handle.read_output_bytes();
        assert_eq!(output, b"Hello");
    }

    #[tokio::test]
    async fn adapter_detects_closed_transport() {
        let (transport, handle) = LoopbackTransport::new();

        drop(handle); // Close the transport

        let mut adapter = TransportAdapter::new(transport);

        let result = adapter.read_byte(1).await.expect("read from closed");
        assert_eq!(result, TransferRead::Closed);
    }

    #[tokio::test]
    async fn adapter_times_out_when_no_data() {
        let (transport, _handle) = LoopbackTransport::new();

        let mut adapter = TransportAdapter::new(transport);

        // Read with 1 second timeout, no data available
        let result = adapter.read_byte(1).await.expect("timeout read");
        assert_eq!(result, TransferRead::TimedOut);
    }
}
