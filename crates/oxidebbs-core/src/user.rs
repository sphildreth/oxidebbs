use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UserStatus {
    Active,
    Inactive,
    Locked,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: String,
    pub alias: String,
    pub real_name: String,
    pub email: Option<String>,
    pub password_hash: String,
    pub security_level: i32,
    pub is_sysop: bool,
    pub created_at: String,
    pub last_login_at: Option<String>,
    pub total_calls: i64,
    pub time_bank_minutes: i64,
    pub status: UserStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateUser {
    pub alias: String,
    pub real_name: String,
    pub email: Option<String>,
    pub password_hash: String,
}
