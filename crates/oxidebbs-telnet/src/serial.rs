use std::fmt;

use tokio::sync::mpsc;

use crate::transport::{Transport, TransportError};

pub struct SerialTransport {
    rx: mpsc::UnboundedReceiver<u8>,
    tx: mpsc::UnboundedSender<u8>,
}

impl fmt::Debug for SerialTransport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SerialTransport").finish_non_exhaustive()
    }
}

impl SerialTransport {
    pub fn new() -> (Self, SerialHandle) {
        let (client_tx, server_rx) = mpsc::unbounded_channel();
        let (server_tx, client_rx) = mpsc::unbounded_channel();
        (
            Self {
                rx: server_rx,
                tx: server_tx,
            },
            SerialHandle {
                rx: client_rx,
                tx: client_tx,
            },
        )
    }
}

impl Transport for SerialTransport {
    async fn read_byte(&mut self) -> Result<Option<u8>, TransportError> {
        Ok(self.rx.recv().await)
    }

    async fn write_all(&mut self, bytes: &[u8]) -> Result<(), TransportError> {
        for &byte in bytes {
            self.tx.send(byte).map_err(|_| TransportError::Closed)?;
        }
        Ok(())
    }

    async fn hangup(&mut self) -> Result<(), TransportError> {
        self.tx = mpsc::unbounded_channel().0;
        Ok(())
    }
}

pub struct SerialHandle {
    rx: mpsc::UnboundedReceiver<u8>,
    tx: mpsc::UnboundedSender<u8>,
}

impl SerialHandle {
    pub async fn read_byte(&mut self) -> Option<u8> {
        self.rx.recv().await
    }

    pub fn write_byte(&self, byte: u8) -> Result<(), TransportError> {
        self.tx.send(byte).map_err(|_| TransportError::Closed)
    }

    pub fn write_bytes(&self, bytes: &[u8]) -> Result<(), TransportError> {
        for &byte in bytes {
            self.tx.send(byte).map_err(|_| TransportError::Closed)?;
        }
        Ok(())
    }

    pub fn read_output_bytes(&mut self) -> Vec<u8> {
        let mut bytes = Vec::new();
        while let Ok(byte) = self.rx.try_recv() {
            bytes.push(byte);
        }
        bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn serial_echo_roundtrip() {
        let (mut server, mut client) = SerialTransport::new();

        client.write_bytes(b"Hi").expect("write to server");
        let first = server.read_byte().await.expect("read first");
        let second = server.read_byte().await.expect("read second");

        assert_eq!(first, Some(b'H'));
        assert_eq!(second, Some(b'i'));

        server.write_all(b"OK").await.expect("write to client");
        let out = client.read_output_bytes();
        assert_eq!(out, b"OK");
    }

    #[tokio::test]
    async fn serial_hangup_returns_closed() {
        let (mut server, client) = SerialTransport::new();

        server.hangup().await.expect("hangup");
        let result = server.write_all(b"x").await;
        assert!(matches!(result, Err(TransportError::Closed)));

        drop(client);
        let byte = server.read_byte().await.expect("read after close");
        assert_eq!(byte, None);
    }
}
