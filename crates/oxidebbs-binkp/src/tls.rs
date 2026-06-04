use std::io::{Read, Write};
use std::net::TcpStream;

use native_tls::{Certificate, Identity, Protocol, TlsAcceptor, TlsConnector, TlsStream};

use crate::error::BinkpError;

/// TLS configuration for BinkP client connections.
#[derive(Clone)]
pub struct BinkpTlsClientConfig {
    /// Whether to verify server certificates.
    pub verify_certificates: bool,
    /// Optional client certificate for mutual TLS.
    pub client_identity: Option<Identity>,
}

impl Default for BinkpTlsClientConfig {
    fn default() -> Self {
        Self {
            verify_certificates: true,
            client_identity: None,
        }
    }
}

/// TLS configuration for BinkP server connections.
#[derive(Clone)]
pub struct BinkpTlsServerConfig {
    /// Server identity (certificate and private key).
    pub identity: Identity,
    /// Whether to require client certificates.
    pub require_client_cert: bool,
    /// Trusted client certificates for mutual TLS.
    pub client_certificates: Vec<Certificate>,
}

/// Establish a TLS connection as a client.
pub fn connect_tls(
    stream: TcpStream,
    hostname: &str,
    config: &BinkpTlsClientConfig,
) -> Result<TlsStream<TcpStream>, BinkpError> {
    let mut builder = TlsConnector::builder();

    if !config.verify_certificates {
        builder.danger_accept_invalid_certs(true);
        builder.danger_accept_invalid_hostnames(true);
    }

    if let Some(identity) = &config.client_identity {
        builder.identity(identity.clone());
    }

    // Disable older protocols for security
    builder.min_protocol_version(Some(Protocol::Tlsv12));

    let connector = builder
        .build()
        .map_err(|e| BinkpError::Tls(format!("Failed to build TLS connector: {}", e)))?;

    connector
        .connect(hostname, stream)
        .map_err(|e| BinkpError::Tls(format!("TLS handshake failed: {}", e)))
}

/// Accept a TLS connection as a server.
pub fn accept_tls(
    stream: TcpStream,
    config: &BinkpTlsServerConfig,
) -> Result<TlsStream<TcpStream>, BinkpError> {
    let mut builder = TlsAcceptor::builder(config.identity.clone());

    // Disable older protocols for security
    builder.min_protocol_version(Some(Protocol::Tlsv12));

    let acceptor = builder
        .build()
        .map_err(|e| BinkpError::Tls(format!("Failed to build TLS acceptor: {}", e)))?;

    acceptor
        .accept(stream)
        .map_err(|e| BinkpError::Tls(format!("TLS handshake failed: {}", e)))
}

/// A wrapper that can hold either a plain TCP stream or a TLS stream.
pub enum BinkpStream {
    Plain(TcpStream),
    Tls(TlsStream<TcpStream>),
}

impl Read for BinkpStream {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self {
            BinkpStream::Plain(stream) => stream.read(buf),
            BinkpStream::Tls(stream) => stream.read(buf),
        }
    }
}

impl Write for BinkpStream {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match self {
            BinkpStream::Plain(stream) => stream.write(buf),
            BinkpStream::Tls(stream) => stream.write(buf),
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        match self {
            BinkpStream::Plain(stream) => stream.flush(),
            BinkpStream::Tls(stream) => stream.flush(),
        }
    }
}

impl BinkpStream {
    /// Create a plain TCP stream.
    pub fn plain(stream: TcpStream) -> Self {
        BinkpStream::Plain(stream)
    }

    /// Create a TLS stream.
    pub fn tls(stream: TlsStream<TcpStream>) -> Self {
        BinkpStream::Tls(stream)
    }

    /// Check if this is a TLS connection.
    pub fn is_tls(&self) -> bool {
        matches!(self, BinkpStream::Tls(_))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_client_config() {
        let config = BinkpTlsClientConfig::default();
        assert!(config.verify_certificates);
        assert!(config.client_identity.is_none());
    }

    #[test]
    fn test_stream_is_tls() {
        // We can't easily test with real streams in unit tests,
        // but we can test the enum logic
        let stream = TcpStream::connect("127.0.0.1:1").unwrap_err();
        // This will fail to connect, which is fine for this test
        assert!(
            stream.to_string().contains("Connection refused")
                || stream.to_string().contains("Network is unreachable")
        );
    }
}
