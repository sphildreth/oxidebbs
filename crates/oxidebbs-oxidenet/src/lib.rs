use thiserror::Error;

pub const OXIDENET_ZONE: i32 = 42;
pub const DEFAULT_HUB_ADDRESS: (&str, i32, i32, i32) = ("42", 1, 1, 0);
pub const DEFAULT_BACKUP_HUB: (&str, i32, i32, i32) = ("42", 1, 2, 0);
pub const INFRA_RANGE_START: i32 = 10;
pub const INFRA_RANGE_END: i32 = 99;
pub const MEMBER_RANGE_START: i32 = 100;
pub const TEST_LAB_START: i32 = 900;

#[derive(Debug, Error)]
pub enum OxideNetError {
    #[error("network error: {0}")]
    Network(String),

    #[error("application already exists for address {0}")]
    DuplicateApplication(String),

    #[error("node not found: {0}")]
    NodeNotFound(String),

    #[error("invalid config package: {0}")]
    InvalidConfigPackage(String),
}

pub const DEFAULT_AREAS: &[&str] = &[
    "OXIDE.GENERAL",
    "OXIDE.SYSOP",
    "OXIDE.NETWORK",
    "OXIDE.TEST",
];

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct Application {
    pub id: String,
    pub sysop_name: String,
    pub board_name: String,
    pub email: String,
    pub description: String,
    pub requested_address: Option<String>,
    pub assigned_address: Option<String>,
    pub status: ApplicationStatus,
    pub created_at: String,
    pub reviewed_at: Option<String>,
    pub reviewed_by: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ApplicationStatus {
    Pending,
    Approved,
    Rejected,
    OnHold,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct OxideNode {
    pub id: String,
    pub address: String,
    pub sysop_name: String,
    pub board_name: String,
    pub host: String,
    pub binkp_port: u16,
    pub password_hash: String,
    pub suspended: bool,
    pub created_at: String,
    pub last_poll_at: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct ConfigPackage {
    pub network_name: String,
    pub local_address: String,
    pub hub_address: String,
    pub binkp_port: u16,
    pub password: String,
    pub areas: Vec<String>,
    pub generated_at: String,
    pub token_hash: String,
}
