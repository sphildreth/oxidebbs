#![allow(dead_code)]

use std::path::{Path, PathBuf};

use serde::Deserialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("failed to read config file {path}: {source}")]
    ReadFailed {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("failed to parse config file {path}: {source}")]
    ParseFailed {
        path: PathBuf,
        source: toml::de::Error,
    },

    #[error("validation error: {0}")]
    Validation(String),
}

#[derive(Debug, Clone, Deserialize)]
pub struct OxideConfig {
    pub board: BoardConfig,
    #[serde(default)]
    pub telnet: TelnetConfig,
    #[serde(default)]
    pub database: DatabaseConfig,
    #[serde(default)]
    pub paths: PathsConfig,
    #[serde(default)]
    pub nodes: NodesConfig,
    #[serde(default)]
    pub terminal: TerminalConfig,
    #[serde(default)]
    pub doors: DoorsConfig,
    #[serde(default)]
    pub ftn: FtnConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BoardConfig {
    pub name: String,
    #[serde(default = "default_tagline")]
    pub tagline: String,
    #[serde(default = "default_sysop_name")]
    pub sysop_name: String,
    #[serde(default = "default_timezone")]
    pub timezone: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TelnetConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_bind")]
    pub bind: String,
    #[serde(default = "default_max_connections")]
    pub max_connections: u32,
    #[serde(default = "default_idle_timeout")]
    pub idle_timeout_seconds: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DatabaseConfig {
    #[serde(default = "default_db_path")]
    pub path: PathBuf,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PathsConfig {
    #[serde(default = "default_ansi_path")]
    pub ansi: PathBuf,
    #[serde(default = "default_doors_path")]
    pub doors: PathBuf,
    #[serde(default = "default_runtime_path")]
    pub runtime: PathBuf,
    #[serde(default = "default_logs_path")]
    pub logs: PathBuf,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NodesConfig {
    #[serde(default = "default_node_count")]
    pub count: u16,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TerminalConfig {
    #[serde(default = "default_encoding")]
    pub default_encoding: String,
    #[serde(default = "default_true")]
    pub clear_screen_on_connect: bool,
    #[serde(default = "default_welcome_screen")]
    pub welcome_screen: String,
    #[serde(default = "default_logoff_screen")]
    pub logoff_screen: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DoorsConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_door_runner")]
    pub default_runner: String,
    #[serde(default)]
    pub definitions: Vec<DoorDefConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DoorDefConfig {
    pub key: String,
    pub name: String,
    #[serde(default = "default_door_runner")]
    pub runner: String,
    pub working_dir: String,
    pub command: String,
    pub drop_file: String,
    #[serde(default)]
    pub exclusive: bool,
    #[serde(default = "default_door_time_limit")]
    pub time_limit_minutes: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FtnConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_network_name")]
    pub reserved_network_name: String,
}

impl OxideConfig {
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        let contents = std::fs::read_to_string(path).map_err(|source| ConfigError::ReadFailed {
            path: path.to_path_buf(),
            source,
        })?;
        let config: OxideConfig =
            toml::from_str(&contents).map_err(|source| ConfigError::ParseFailed {
                path: path.to_path_buf(),
                source,
            })?;
        config.validate()?;
        Ok(config)
    }

    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.telnet.max_connections == 0 {
            return Err(ConfigError::Validation(
                "telnet.max_connections must be greater than 0".into(),
            ));
        }
        if self.nodes.count == 0 {
            return Err(ConfigError::Validation(
                "nodes.count must be greater than 0".into(),
            ));
        }
        if self.telnet.idle_timeout_seconds == 0 {
            return Err(ConfigError::Validation(
                "telnet.idle_timeout_seconds must be greater than 0".into(),
            ));
        }
        Ok(())
    }
}

// Defaults

fn default_tagline() -> String {
    "Built for sysops. Driven by code.".into()
}
fn default_sysop_name() -> String {
    "Sysop".into()
}
fn default_timezone() -> String {
    "America/Chicago".into()
}
fn default_true() -> bool {
    true
}
fn default_bind() -> String {
    "0.0.0.0:2323".into()
}
fn default_max_connections() -> u32 {
    4
}
fn default_idle_timeout() -> u64 {
    900
}
fn default_db_path() -> PathBuf {
    PathBuf::from("./data/oxidebbs.ddb")
}
fn default_ansi_path() -> PathBuf {
    PathBuf::from("./assets/ansi")
}
fn default_doors_path() -> PathBuf {
    PathBuf::from("./doors")
}
fn default_runtime_path() -> PathBuf {
    PathBuf::from("./runtime")
}
fn default_logs_path() -> PathBuf {
    PathBuf::from("./logs")
}
fn default_node_count() -> u16 {
    4
}
fn default_encoding() -> String {
    "cp437".into()
}
fn default_welcome_screen() -> String {
    "welcome.ans".into()
}
fn default_logoff_screen() -> String {
    "logoff.ans".into()
}
fn default_door_runner() -> String {
    "dosbox".into()
}
fn default_door_time_limit() -> u32 {
    30
}
fn default_network_name() -> String {
    "OxideNet".into()
}

impl Default for TelnetConfig {
    fn default() -> Self {
        Self {
            enabled: default_true(),
            bind: default_bind(),
            max_connections: default_max_connections(),
            idle_timeout_seconds: default_idle_timeout(),
        }
    }
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            path: default_db_path(),
        }
    }
}

impl Default for PathsConfig {
    fn default() -> Self {
        Self {
            ansi: default_ansi_path(),
            doors: default_doors_path(),
            runtime: default_runtime_path(),
            logs: default_logs_path(),
        }
    }
}

impl Default for NodesConfig {
    fn default() -> Self {
        Self {
            count: default_node_count(),
        }
    }
}

impl Default for TerminalConfig {
    fn default() -> Self {
        Self {
            default_encoding: default_encoding(),
            clear_screen_on_connect: default_true(),
            welcome_screen: default_welcome_screen(),
            logoff_screen: default_logoff_screen(),
        }
    }
}

impl Default for DoorsConfig {
    fn default() -> Self {
        Self {
            enabled: default_true(),
            default_runner: default_door_runner(),
            definitions: Vec::new(),
        }
    }
}

impl Default for FtnConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            reserved_network_name: default_network_name(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_example_config() {
        let toml = r#"
[board]
name = "Test BBS"
tagline = "Testing"
sysop_name = "Admin"
timezone = "UTC"

[telnet]
enabled = true
bind = "127.0.0.1:9999"
max_connections = 2
idle_timeout_seconds = 60

[database]
path = "/tmp/test.ddb"

[nodes]
count = 2
"#;
        let config: OxideConfig = toml::from_str(toml).expect("parse config");
        assert_eq!(config.board.name, "Test BBS");
        assert_eq!(config.telnet.bind, "127.0.0.1:9999");
        assert_eq!(config.telnet.max_connections, 2);
        assert_eq!(config.nodes.count, 2);
        assert_eq!(config.paths.ansi, PathBuf::from("./assets/ansi"));
    }

    #[test]
    fn rejects_zero_max_connections() {
        let toml = r#"
[board]
name = "Test"

[telnet]
max_connections = 0
"#;
        let config: OxideConfig = toml::from_str(toml).expect("parse");
        assert!(config.validate().is_err());
    }

    #[test]
    fn rejects_zero_node_count() {
        let toml = r#"
[board]
name = "Test"

[nodes]
count = 0
"#;
        let config: OxideConfig = toml::from_str(toml).expect("parse");
        assert!(config.validate().is_err());
    }

    #[test]
    fn applies_defaults_for_missing_sections() {
        let toml = r#"
[board]
name = "Minimal"
"#;
        let config: OxideConfig = toml::from_str(toml).expect("parse with defaults");
        assert_eq!(config.telnet.bind, "0.0.0.0:2323");
        assert_eq!(config.database.path, PathBuf::from("./data/oxidebbs.ddb"));
        assert_eq!(config.nodes.count, 4);
        assert_eq!(config.terminal.default_encoding, "cp437");
        assert!(!config.ftn.enabled);
    }

    #[test]
    fn parses_door_definitions() {
        let toml = r#"
[board]
name = "Test"

[doors]
enabled = true
default_runner = "dosbox"

[[doors.definitions]]
key = "lord"
name = "Legend of the Red Dragon"
runner = "dosbox"
working_dir = "./doors/lord"
command = "LORD.EXE"
drop_file = "DORINFO1.DEF"
exclusive = false
time_limit_minutes = 30
"#;
        let config: OxideConfig = toml::from_str(toml).expect("parse");
        assert_eq!(config.doors.definitions.len(), 1);
        assert_eq!(config.doors.definitions[0].key, "lord");
        assert_eq!(config.doors.definitions[0].time_limit_minutes, 30);
    }
}
