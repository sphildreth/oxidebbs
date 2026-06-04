use std::io;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum FtnError {
    #[error("parse error: {0}")]
    Parse(String),

    #[error("io error: {0}")]
    Io(#[from] io::Error),

    #[error("protocol error: {0}")]
    Protocol(String),
}
