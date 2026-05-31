use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AreaKind {
    Local,
    EchoMail,
    NetMail,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageArea {
    pub id: String,
    pub key: String,
    pub name: String,
    pub description: String,
    pub kind: AreaKind,
    pub network_id: Option<String>,
    pub read_security_level: i32,
    pub post_security_level: i32,
    pub moderated: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MessageVisibility {
    Normal,
    Deleted,
    PendingModeration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    pub area_id: String,
    pub author_user_id: String,
    pub to_user_id: Option<String>,
    pub subject: String,
    pub body: String,
    pub created_at: String,
    pub reply_to_id: Option<String>,
    pub network_message_id: Option<String>,
    pub visibility: MessageVisibility,
}
