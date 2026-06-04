use thiserror::Error;

#[derive(Debug, Error)]
pub enum BinkpError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("protocol error: {0}")]
    Protocol(String),

    #[error("connection refused")]
    ConnectionRefused,

    #[error("TLS error: {0}")]
    Tls(String),

    #[error("TLS required but not available")]
    TlsRequired,

    #[error("TLS handshake failed: {0}")]
    TlsHandshake(String),
}
