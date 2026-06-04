use thiserror::Error;

#[derive(Debug, Error)]
pub enum BinkpError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("protocol error: {0}")]
    Protocol(String),

    #[error("connection refused")]
    ConnectionRefused,
}
