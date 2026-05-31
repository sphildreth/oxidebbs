use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeStatus {
    Idle,
    Connected,
    LoggingIn,
    InMenu,
    InDoor,
    Uploading,
    Downloading,
    Chatting,
    Voting,
    Disconnected,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    pub id: String,
    pub node_number: u16,
    pub status: NodeStatus,
    pub current_user_id: Option<String>,
    pub current_activity: Option<String>,
    pub connected_at: Option<String>,
    pub last_activity_at: Option<String>,
    pub transport: String,
}
