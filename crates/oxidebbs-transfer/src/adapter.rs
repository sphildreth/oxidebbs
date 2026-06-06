//! Adapter to bridge oxidebbs-telnet's Transport trait to oxidebbs-transfer's ByteTransport trait.

use std::future::Future;
use std::pin::Pin;

use crate::{ByteTransport, TransferError, TransferRead};
use oxidebbs_telnet::telnet::{DO, DONT, IAC, SB, SE, WILL, WONT};

/// Adapter that wraps a telnet Transport implementation to provide the ByteTransport interface.
///
/// This allows file transfer protocols (XMODEM-CRC, ZMODEM) to work over any
/// transport layer (TCP/telnet, serial, loopback) that implements the Transport trait.
pub struct TransportAdapter<T> {
    transport: T,
    telnet_iac: bool,
}

impl<T> TransportAdapter<T> {
    /// Create a new raw adapter wrapping the given transport.
    pub fn new(transport: T) -> Self {
        Self::new_raw(transport)
    }

    /// Create a raw byte adapter for serial and already-unescaped transports.
    pub fn new_raw(transport: T) -> Self {
        Self {
            transport,
            telnet_iac: false,
        }
    }

    /// Create a telnet byte adapter that unescapes inbound IAC bytes and
    /// doubles outbound IAC bytes for binary file-transfer payloads.
    pub fn new_telnet(transport: T) -> Self {
        Self {
            transport,
            telnet_iac: true,
        }
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

            loop {
                let byte = match tokio::time::timeout(timeout_duration, self.transport.read_byte())
                    .await
                {
                    Ok(Ok(Some(byte))) => byte,
                    Ok(Ok(None)) => return Ok(TransferRead::Closed),
                    Ok(Err(_)) => return Err(TransferError::Transport),
                    Err(_) => return Ok(TransferRead::TimedOut),
                };

                if !self.telnet_iac || byte != IAC {
                    return Ok(TransferRead::Byte(byte));
                }

                let command = match tokio::time::timeout(
                    timeout_duration,
                    self.transport.read_byte(),
                )
                .await
                {
                    Ok(Ok(Some(byte))) => byte,
                    Ok(Ok(None)) => return Ok(TransferRead::Closed),
                    Ok(Err(_)) => return Err(TransferError::Transport),
                    Err(_) => return Ok(TransferRead::TimedOut),
                };

                match command {
                    IAC => return Ok(TransferRead::Byte(IAC)),
                    DO | DONT | WILL | WONT => {
                        let option =
                            tokio::time::timeout(timeout_duration, self.transport.read_byte())
                                .await;
                        match option {
                            Ok(Ok(Some(_))) => {}
                            Ok(Ok(None)) => return Ok(TransferRead::Closed),
                            Ok(Err(_)) => return Err(TransferError::Transport),
                            Err(_) => return Ok(TransferRead::TimedOut),
                        }
                    }
                    SB => loop {
                        let next =
                            tokio::time::timeout(timeout_duration, self.transport.read_byte())
                                .await;
                        let next = match next {
                            Ok(Ok(Some(byte))) => byte,
                            Ok(Ok(None)) => return Ok(TransferRead::Closed),
                            Ok(Err(_)) => return Err(TransferError::Transport),
                            Err(_) => return Ok(TransferRead::TimedOut),
                        };
                        if next != IAC {
                            continue;
                        }
                        let after_iac =
                            tokio::time::timeout(timeout_duration, self.transport.read_byte())
                                .await;
                        match after_iac {
                            Ok(Ok(Some(SE))) => break,
                            Ok(Ok(Some(_))) => {}
                            Ok(Ok(None)) => return Ok(TransferRead::Closed),
                            Ok(Err(_)) => return Err(TransferError::Transport),
                            Err(_) => return Ok(TransferRead::TimedOut),
                        }
                    },
                    _ => {}
                }
            }
        })
    }

    fn write_all<'a>(
        &'a mut self,
        buf: &'a [u8],
    ) -> Pin<Box<dyn Future<Output = Result<(), TransferError>> + Send + 'a>> {
        Box::pin(async move {
            if self.telnet_iac && buf.contains(&IAC) {
                let mut escaped = Vec::with_capacity(buf.len() + 1);
                for &byte in buf {
                    escaped.push(byte);
                    if byte == IAC {
                        escaped.push(IAC);
                    }
                }
                self.transport
                    .write_all(&escaped)
                    .await
                    .map_err(|_| TransferError::Transport)
            } else {
                self.transport
                    .write_all(buf)
                    .await
                    .map_err(|_| TransferError::Transport)
            }
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
    async fn telnet_adapter_unescapes_doubled_iac_data() {
        let (transport, handle) = LoopbackTransport::new();

        handle
            .write_bytes(&[b'A', IAC, IAC, b'B'])
            .expect("write telnet escaped data");

        let mut adapter = TransportAdapter::new_telnet(transport);

        assert_eq!(
            adapter.read_byte(1).await.expect("read A"),
            TransferRead::Byte(b'A')
        );
        assert_eq!(
            adapter.read_byte(1).await.expect("read IAC"),
            TransferRead::Byte(IAC)
        );
        assert_eq!(
            adapter.read_byte(1).await.expect("read B"),
            TransferRead::Byte(b'B')
        );
    }

    #[tokio::test]
    async fn telnet_adapter_escapes_outbound_iac_data() {
        let (transport, mut handle) = LoopbackTransport::new();

        let mut adapter = TransportAdapter::new_telnet(transport);

        adapter.write_all(&[b'A', IAC, b'B']).await.expect("write");

        let output = handle.read_output_bytes();
        assert_eq!(output, [b'A', IAC, IAC, b'B']);
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
