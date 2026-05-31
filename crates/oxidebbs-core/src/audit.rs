use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEvent {
    pub id: String,
    pub created_at: String,
    pub event_type: String,
    pub user_id: Option<String>,
    pub node_number: Option<u16>,
    pub details: String,
}
