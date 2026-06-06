use std::fmt;

use thiserror::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::mpsc;

#[derive(Debug, Error)]
pub enum TransportError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("transport closed")]
    Closed,
}

pub trait Transport: Send {
    fn read_byte(
        &mut self,
    ) -> impl std::future::Future<Output = Result<Option<u8>, TransportError>> + Send;
    fn write_all(
        &mut self,
        bytes: &[u8],
    ) -> impl std::future::Future<Output = Result<(), TransportError>> + Send;
    fn hangup(&mut self) -> impl std::future::Future<Output = Result<(), TransportError>> + Send;
}

impl<T: Transport + ?Sized> Transport for &mut T {
    async fn read_byte(&mut self) -> Result<Option<u8>, TransportError> {
        (**self).read_byte().await
    }

    async fn write_all(&mut self, bytes: &[u8]) -> Result<(), TransportError> {
        (**self).write_all(bytes).await
    }

    async fn hangup(&mut self) -> Result<(), TransportError> {
        (**self).hangup().await
    }
}

pub struct TcpTransport {
    reader: tokio::io::ReadHalf<tokio::net::TcpStream>,
    writer: tokio::io::WriteHalf<tokio::net::TcpStream>,
    read_buffer: [u8; 4096],
    read_buffer_len: usize,
    read_buffer_pos: usize,
}

impl TcpTransport {
    pub fn new(stream: tokio::net::TcpStream) -> Self {
        let (reader, writer) = tokio::io::split(stream);
        Self {
            reader,
            writer,
            read_buffer: [0u8; 4096],
            read_buffer_len: 0,
            read_buffer_pos: 0,
        }
    }
}

impl Transport for TcpTransport {
    async fn read_byte(&mut self) -> Result<Option<u8>, TransportError> {
        if self.read_buffer_pos == self.read_buffer_len {
            self.read_buffer_len = self.reader.read(&mut self.read_buffer).await?;
            self.read_buffer_pos = 0;

            if self.read_buffer_len == 0 {
                return Ok(None);
            }
        }

        let byte = self.read_buffer[self.read_buffer_pos];
        self.read_buffer_pos += 1;
        Ok(Some(byte))
    }

    async fn write_all(&mut self, bytes: &[u8]) -> Result<(), TransportError> {
        self.writer.write_all(bytes).await?;
        self.writer.flush().await?;
        Ok(())
    }

    async fn hangup(&mut self) -> Result<(), TransportError> {
        self.writer.shutdown().await?;
        Ok(())
    }
}

pub struct LoopbackTransport {
    rx: mpsc::UnboundedReceiver<u8>,
    tx: mpsc::UnboundedSender<u8>,
}

impl fmt::Debug for LoopbackTransport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LoopbackTransport").finish_non_exhaustive()
    }
}

impl LoopbackTransport {
    pub fn new() -> (Self, LoopbackHandle) {
        let (client_tx, server_rx) = mpsc::unbounded_channel();
        let (server_tx, client_rx) = mpsc::unbounded_channel();
        (
            Self {
                rx: server_rx,
                tx: server_tx,
            },
            LoopbackHandle {
                rx: client_rx,
                tx: client_tx,
            },
        )
    }
}

impl Transport for LoopbackTransport {
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

pub struct LoopbackHandle {
    rx: mpsc::UnboundedReceiver<u8>,
    tx: mpsc::UnboundedSender<u8>,
}

impl LoopbackHandle {
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
    use tokio::io::AsyncWriteExt;

    #[tokio::test]
    async fn tcp_transport_reads_one_byte_at_a_time_from_internal_buffer() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind tcp transport test listener");
        let server_addr = listener.local_addr().expect("listener addr");
        let client_task = tokio::spawn(async move {
            let mut client = tokio::net::TcpStream::connect(server_addr)
                .await
                .expect("connect test client");
            client
                .write_all(b"ABCD")
                .await
                .expect("write transport test payload");
        });

        let (server, _) = listener.accept().await.expect("accept test connection");
        let mut transport = TcpTransport::new(server);

        assert_eq!(transport.read_byte().await.expect("read first"), Some(b'A'));
        assert_eq!(
            transport.read_byte().await.expect("read second"),
            Some(b'B')
        );
        assert_eq!(transport.read_byte().await.expect("read third"), Some(b'C'));
        assert_eq!(
            transport.read_byte().await.expect("read fourth"),
            Some(b'D')
        );
        assert_eq!(transport.read_byte().await.expect("read eof"), None);

        client_task.await.expect("client task");
    }

    #[tokio::test]
    async fn tcp_transport_reads_multiple_frames_from_same_recv_buffer() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind tcp transport test listener");
        let server_addr = listener.local_addr().expect("listener addr");
        let client_task = tokio::spawn(async move {
            let mut client = tokio::net::TcpStream::connect(server_addr)
                .await
                .expect("connect test client");
            client
                .write_all(b"HELLO")
                .await
                .expect("write transport test payload");
        });

        let (server, _) = listener.accept().await.expect("accept test connection");
        let mut transport = TcpTransport::new(server);

        assert_eq!(transport.read_byte().await.expect("read first"), Some(b'H'));
        assert_eq!(
            transport.read_byte().await.expect("read second"),
            Some(b'E')
        );
        assert_eq!(transport.read_byte().await.expect("read third"), Some(b'L'));
        assert_eq!(
            transport.read_byte().await.expect("read fourth"),
            Some(b'L')
        );
        assert_eq!(transport.read_byte().await.expect("read fifth"), Some(b'O'));
        assert_eq!(transport.read_byte().await.expect("read eof"), None);

        client_task.await.expect("client task");
    }

    #[tokio::test]
    async fn loopback_echo_roundtrip() {
        let (mut server, mut client) = LoopbackTransport::new();

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
    async fn loopback_hangup_returns_closed() {
        let (mut server, client) = LoopbackTransport::new();

        server.hangup().await.expect("hangup");
        let result = server.write_all(b"x").await;
        assert!(matches!(result, Err(TransportError::Closed)));

        drop(client);
        let byte = server.read_byte().await.expect("read after close");
        assert_eq!(byte, None);
    }
}
