use thiserror::Error;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("user not found: {0}")]
    UserNotFound(String),

    #[error("session not found: {0}")]
    SessionNotFound(String),

    #[error("node {0} is not available")]
    NodeUnavailable(u16),

    #[error("door not found: {0}")]
    DoorNotFound(String),

    #[error("message area not found: {0}")]
    AreaNotFound(String),

    #[error("permission denied: requires security level {required}, user has {actual}")]
    PermissionDenied { required: i32, actual: i32 },

    #[error("invalid alias: {0}")]
    InvalidAlias(String),

    #[error("alias already taken: {0}")]
    AliasTaken(String),

    #[error("invalid configuration: {0}")]
    InvalidConfig(String),
}
