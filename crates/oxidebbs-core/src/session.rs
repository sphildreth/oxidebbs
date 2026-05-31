use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DisconnectReason {
    UserLogoff,
    IdleTimeout,
    ServerShutdown,
    TransportError,
    KickedBySysop,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub node_number: u16,
    pub user_id: Option<String>,
    pub transport: String,
    pub remote_address: String,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub disconnect_reason: Option<DisconnectReason>,
}
