use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoorDefinition {
    pub id: String,
    pub key: String,
    pub name: String,
    pub runner: String,
    pub working_dir: String,
    pub command: String,
    pub drop_file: String,
    pub exclusive: bool,
    pub time_limit_minutes: u32,
    pub enabled: bool,
    #[serde(default)]
    pub min_security_level: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoorRun {
    pub id: String,
    pub door_id: String,
    pub user_id: String,
    pub node_number: u16,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub exit_code: Option<i32>,
    pub timed_out: bool,
    pub disconnect_forced: bool,
    pub bytes_in: u64,
    pub bytes_out: u64,
}
