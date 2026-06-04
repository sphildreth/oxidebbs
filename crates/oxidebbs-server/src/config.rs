#![allow(dead_code)]

use std::collections::{HashMap, HashSet};
use std::net::IpAddr;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use thiserror::Error;

use oxidebbs_core::menu::{Menu, MenuAction, MenuEntry, ScreenAsset};
pub use oxidebbs_term::TerminalCapabilities;
use oxidebbs_term::{
    BackspaceMode, LineEndingMode, OutputPacing, TerminalCharset, TerminalProfile,
};

pub const DEFAULT_DATABASE_FILE_NAME: &str = "oxidebbs.ddb";

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
    pub auth: AuthConfig,
    #[serde(default)]
    pub audit: AuditConfig,
    #[serde(default)]
    pub logging: LoggingConfig,
    #[serde(default)]
    pub database: DatabaseConfig,
    #[serde(default)]
    pub paths: PathsConfig,
    #[serde(default)]
    pub nodes: NodesConfig,
    #[serde(default)]
    pub sysop: SysopConfig,
    #[serde(default)]
    pub terminal: TerminalConfig,
    #[serde(default)]
    pub flow: FlowConfig,
    #[serde(default)]
    pub screens: HashMap<String, ScreenConfig>,
    #[serde(default)]
    pub menus: HashMap<String, MenuConfig>,
    #[serde(default)]
    pub doors: DoorsConfig,
    #[serde(default)]
    pub network: NetworkConfig,
    #[serde(default)]
    pub ftn: FtnConfig,
    #[serde(default)]
    pub serial: SerialConfig,
    #[serde(default)]
    pub file_transfers: FileTransfersConfig,
    #[serde(default)]
    pub admin_web: AdminWebConfig,
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
pub struct AuthConfig {
    #[serde(default = "default_failed_login_threshold")]
    pub failed_login_threshold: i64,
    #[serde(default = "default_failed_login_window_minutes")]
    pub failed_login_window_minutes: i64,
    #[serde(default = "default_failed_login_lockout_minutes")]
    pub failed_login_lockout_minutes: i64,
    #[serde(default = "default_new_user_security_level")]
    pub new_user_security_level: i32,
    #[serde(default)]
    pub argon2: Argon2Config,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Argon2Config {
    #[serde(default = "default_argon2_memory_cost_kib")]
    pub memory_cost_kib: u32,
    #[serde(default = "default_argon2_iterations")]
    pub iterations: u32,
    #[serde(default = "default_argon2_parallelism")]
    pub parallelism: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AuditConfig {
    #[serde(default = "default_audit_retention_days")]
    pub retention_days: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LoggingConfig {
    #[serde(default = "default_logging_level")]
    pub level: String,
    #[serde(default = "default_true")]
    pub file_enabled: bool,
    #[serde(default = "default_logging_file_name")]
    pub file_name: String,
    #[serde(default = "default_logging_format")]
    pub format: String,
    #[serde(default)]
    pub rotation: LoggingRotationConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LoggingRotationConfig {
    #[serde(default = "default_logging_rotation_strategy")]
    pub strategy: String,
    #[serde(default = "default_logging_rotation_max_size_mb")]
    pub max_size_mb: u64,
    #[serde(default = "default_logging_rotation_max_files")]
    pub max_files: usize,
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
    #[serde(default = "default_screens_path")]
    pub screens: PathBuf,
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
pub struct SysopConfig {
    #[serde(default = "default_true")]
    pub confirm_quit: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TerminalConfig {
    #[serde(default = "default_encoding")]
    pub default_encoding: String,
    #[serde(default = "default_terminal_profile")]
    pub default_profile: String,
    #[serde(default = "default_true")]
    pub manual_profile_selection: bool,
    #[serde(default = "default_true")]
    pub clear_screen_on_connect: bool,
    #[serde(default = "default_welcome_screen")]
    pub welcome_screen: String,
    #[serde(default = "default_logoff_screen")]
    pub logoff_screen: String,
    #[serde(default = "default_terminal_profiles")]
    pub profiles: HashMap<String, TerminalProfileConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TerminalProfileConfig {
    #[serde(default = "default_terminal_profile_name")]
    pub name: String,
    #[serde(default = "default_terminal_profile_width")]
    pub width: u16,
    #[serde(default = "default_terminal_profile_height")]
    pub height: u16,
    #[serde(default)]
    pub supports_ansi: bool,
    #[serde(default)]
    pub supports_color: bool,
    #[serde(default = "default_ascii_charset")]
    pub charset: String,
    #[serde(default = "default_crlf_line_endings")]
    pub line_endings: String,
    #[serde(default = "default_backspace_mode")]
    pub backspace_mode: String,
    #[serde(default)]
    pub output_pacing_bytes_per_second: Option<u32>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FlowConfig {
    #[serde(default = "default_login_screen")]
    pub login_screen: String,
    #[serde(default = "default_login_menu")]
    pub login_menu: String,
    #[serde(default)]
    pub post_login_screens: Vec<String>,
    #[serde(default = "default_main_menu")]
    pub main_menu: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ScreenConfig {
    pub ansi: Option<String>,
    pub ansi_40: Option<String>,
    pub ascii_40: Option<String>,
    pub ascii: Option<String>,
    pub text_40: Option<String>,
    pub text: Option<String>,
    #[serde(default)]
    pub pause: bool,
}

impl ScreenConfig {
    pub fn asset_for(&self, capabilities: TerminalCapabilities) -> Option<&str> {
        if capabilities.supports_ansi {
            if capabilities.width <= 40
                && let Some(asset) = self.ansi_40.as_deref()
            {
                return Some(asset);
            }
            if let Some(asset) = self.ansi.as_deref() {
                return Some(asset);
            }
            if let Some(asset) = self.ansi_40.as_deref() {
                return Some(asset);
            }
        }

        if capabilities.width <= 40 {
            if let Some(asset) = self.ascii_40.as_deref() {
                return Some(asset);
            }
            if let Some(asset) = self.text_40.as_deref() {
                return Some(asset);
            }
        }

        self.ascii.as_deref().or(self.text.as_deref())
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct MenuConfig {
    pub screen: String,
    #[serde(default = "default_menu_prompt")]
    pub prompt: String,
    #[serde(default)]
    pub items: Vec<MenuItemConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MenuItemConfig {
    pub key: String,
    pub label: String,
    pub action: String,
    #[serde(default)]
    pub target: Option<String>,
    #[serde(default)]
    pub min_security_level: i32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DoorsConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_door_runner")]
    pub default_runner: String,
    #[serde(default = "default_door_allowed_runners")]
    pub allowed_runners: Vec<String>,
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
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub min_security_level: i32,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NetworkConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub profiles: HashMap<String, NetworkProfileConfig>,
    #[serde(default)]
    pub links: HashMap<String, NetworkLinkConfig>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NetworkProfileConfig {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_network_adapter")]
    pub adapter: String,
    pub local_address: NetworkLocalAddressConfig,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NetworkLocalAddressConfig {
    pub zone: u16,
    pub net: u16,
    pub node: u16,
    #[serde(default)]
    pub point: u16,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NetworkLinkConfig {
    pub network: String,
    pub address: String,
    pub host: String,
    #[serde(default = "default_binkp_port")]
    pub binkp_port: u16,
    #[serde(default)]
    pub password: String,
    #[serde(default = "default_poll_schedule_minutes")]
    pub poll_schedule_minutes: u32,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_bundle_compression")]
    pub compression: String,
    #[serde(default = "default_transport_security")]
    pub transport_security: String,
    #[serde(default)]
    pub legacy_compatible: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FtnConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_network_name")]
    pub reserved_network_name: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct SerialConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub devices: Vec<SerialDeviceConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SerialDeviceConfig {
    pub name: String,
    pub path: String,
    #[serde(default = "default_serial_baud_rate")]
    pub baud_rate: u32,
    #[serde(default = "default_serial_flow_control")]
    pub flow_control: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct FileTransfersConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_file_transfers_max_upload_bytes")]
    pub max_upload_bytes: i64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdminWebConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_admin_web_bind")]
    pub bind: String,
    #[serde(default = "default_true")]
    pub require_tls: bool,
    #[serde(default = "default_true")]
    pub read_only: bool,
    #[serde(default = "default_admin_web_session_timeout_seconds")]
    pub session_timeout_seconds: u64,
    #[serde(default = "default_admin_web_csrf_token_ttl_seconds")]
    pub csrf_token_ttl_seconds: u64,
    #[serde(default = "default_admin_web_replay_window_seconds")]
    pub replay_window_seconds: u64,
    #[serde(default = "default_admin_web_rate_limit_per_minute")]
    pub rate_limit_per_minute: u32,
}

impl OxideConfig {
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        let contents = std::fs::read_to_string(path).map_err(|source| ConfigError::ReadFailed {
            path: path.to_path_buf(),
            source,
        })?;
        let mut config: OxideConfig =
            toml::from_str(&contents).map_err(|source| ConfigError::ParseFailed {
                path: path.to_path_buf(),
                source,
            })?;
        config.normalize_paths();
        config.validate()?;
        Ok(config)
    }

    pub fn normalize_paths(&mut self) {
        self.database.path = normalize_database_path(&self.database.path);
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
        if self.auth.failed_login_threshold <= 0 {
            return Err(ConfigError::Validation(
                "auth.failed_login_threshold must be greater than 0".into(),
            ));
        }
        if self.auth.failed_login_window_minutes <= 0 {
            return Err(ConfigError::Validation(
                "auth.failed_login_window_minutes must be greater than 0".into(),
            ));
        }
        if self.auth.failed_login_lockout_minutes <= 0 {
            return Err(ConfigError::Validation(
                "auth.failed_login_lockout_minutes must be greater than 0".into(),
            ));
        }
        if !(0..=255).contains(&self.auth.new_user_security_level) {
            return Err(ConfigError::Validation(
                "auth.new_user_security_level must be between 0 and 255".into(),
            ));
        }
        if self.auth.argon2.memory_cost_kib == 0 {
            return Err(ConfigError::Validation(
                "auth.argon2.memory_cost_kib must be greater than 0".into(),
            ));
        }
        if self.auth.argon2.iterations == 0 {
            return Err(ConfigError::Validation(
                "auth.argon2.iterations must be greater than 0".into(),
            ));
        }
        if self.auth.argon2.parallelism == 0 {
            return Err(ConfigError::Validation(
                "auth.argon2.parallelism must be greater than 0".into(),
            ));
        }
        if self.audit.retention_days <= 0 {
            return Err(ConfigError::Validation(
                "audit.retention_days must be greater than 0".into(),
            ));
        }
        validate_logging_level(&self.logging.level).map_err(ConfigError::Validation)?;
        validate_logging_file_name(&self.logging.file_name).map_err(ConfigError::Validation)?;
        validate_logging_format(&self.logging.format).map_err(ConfigError::Validation)?;
        validate_logging_rotation_strategy(&self.logging.rotation.strategy)
            .map_err(ConfigError::Validation)?;
        if self.logging.rotation.max_size_mb == 0 {
            return Err(ConfigError::Validation(
                "logging.rotation.max_size_mb must be greater than 0".into(),
            ));
        }
        if self.logging.rotation.max_files == 0 {
            return Err(ConfigError::Validation(
                "logging.rotation.max_files must be greater than 0".into(),
            ));
        }
        if self.doors.allowed_runners.is_empty() {
            return Err(ConfigError::Validation(
                "doors.allowed_runners must include at least one runner".into(),
            ));
        }
        for door in &self.doors.definitions {
            if door.time_limit_minutes == 0 || door.time_limit_minutes > 240 {
                return Err(ConfigError::Validation(format!(
                    "doors.definitions.{} time_limit_minutes must be between 1 and 240",
                    door.key
                )));
            }
        }
        self.validate_terminal()?;
        self.validate_flow()?;
        self.validate_screens()?;
        self.validate_menus()?;
        self.validate_network()?;
        self.validate_admin_web()?;
        Ok(())
    }

    fn validate_terminal(&self) -> Result<(), ConfigError> {
        if !self
            .terminal
            .profiles
            .contains_key(&self.terminal.default_profile)
        {
            return Err(ConfigError::Validation(format!(
                "terminal.default_profile references missing profile {:?}",
                self.terminal.default_profile
            )));
        }

        for (key, profile) in &self.terminal.profiles {
            validate_config_key("terminal.profiles", key)?;
            if profile.name.trim().is_empty() {
                return Err(ConfigError::Validation(format!(
                    "terminal.profiles.{key}.name must not be blank"
                )));
            }
            if profile.width == 0 {
                return Err(ConfigError::Validation(format!(
                    "terminal.profiles.{key}.width must be greater than 0"
                )));
            }
            if profile.height == 0 {
                return Err(ConfigError::Validation(format!(
                    "terminal.profiles.{key}.height must be greater than 0"
                )));
            }
            validate_terminal_charset(&profile.charset).map_err(ConfigError::Validation)?;
            validate_terminal_line_endings(&profile.line_endings)
                .map_err(ConfigError::Validation)?;
            validate_terminal_backspace_mode(&profile.backspace_mode)
                .map_err(ConfigError::Validation)?;
            if profile.supports_color && !profile.supports_ansi {
                return Err(ConfigError::Validation(format!(
                    "terminal.profiles.{key}.supports_color requires supports_ansi = true"
                )));
            }
            if profile.output_pacing_bytes_per_second == Some(0) {
                return Err(ConfigError::Validation(format!(
                    "terminal.profiles.{key}.output_pacing_bytes_per_second must be greater than 0 when set"
                )));
            }
        }

        Ok(())
    }

    fn validate_flow(&self) -> Result<(), ConfigError> {
        if !self.screens.contains_key(&self.flow.login_screen) {
            return Err(ConfigError::Validation(format!(
                "flow.login_screen references missing screen {:?}",
                self.flow.login_screen
            )));
        }
        if !self.menus.contains_key(&self.flow.login_menu) {
            return Err(ConfigError::Validation(format!(
                "flow.login_menu references missing menu {:?}",
                self.flow.login_menu
            )));
        }
        for screen in &self.flow.post_login_screens {
            if !self.screens.contains_key(screen) {
                return Err(ConfigError::Validation(format!(
                    "flow.post_login_screens references missing screen {screen:?}"
                )));
            }
        }
        if !self.menus.contains_key(&self.flow.main_menu) {
            return Err(ConfigError::Validation(format!(
                "flow.main_menu references missing menu {:?}",
                self.flow.main_menu
            )));
        }
        Ok(())
    }

    fn validate_admin_web(&self) -> Result<(), ConfigError> {
        if self.admin_web.bind.trim().is_empty() {
            return Err(ConfigError::Validation(
                "admin_web.bind must not be blank".into(),
            ));
        }
        let bind = self
            .admin_web
            .bind
            .parse::<std::net::SocketAddr>()
            .map_err(|_| {
                ConfigError::Validation(format!(
                    "admin_web.bind must be an IP socket address, got {:?}",
                    self.admin_web.bind
                ))
            })?;
        if self.admin_web.session_timeout_seconds == 0 {
            return Err(ConfigError::Validation(
                "admin_web.session_timeout_seconds must be greater than 0".into(),
            ));
        }
        if self.admin_web.csrf_token_ttl_seconds == 0 {
            return Err(ConfigError::Validation(
                "admin_web.csrf_token_ttl_seconds must be greater than 0".into(),
            ));
        }
        if self.admin_web.replay_window_seconds == 0 {
            return Err(ConfigError::Validation(
                "admin_web.replay_window_seconds must be greater than 0".into(),
            ));
        }
        if self.admin_web.rate_limit_per_minute == 0 {
            return Err(ConfigError::Validation(
                "admin_web.rate_limit_per_minute must be greater than 0".into(),
            ));
        }
        if self.admin_web.enabled {
            if !self.admin_web.read_only {
                return Err(ConfigError::Validation(
                    "admin_web.read_only must remain true until remote admin mutations are implemented".into(),
                ));
            }
            if !ip_is_loopback(bind.ip()) && !self.admin_web.require_tls {
                return Err(ConfigError::Validation(
                    "admin_web.require_tls must be true for non-loopback binds".into(),
                ));
            }
        }
        Ok(())
    }

    fn validate_screens(&self) -> Result<(), ConfigError> {
        for (name, screen) in &self.screens {
            if screen.ansi.is_none()
                && screen.ansi_40.is_none()
                && screen.ascii_40.is_none()
                && screen.ascii.is_none()
                && screen.text_40.is_none()
                && screen.text.is_none()
            {
                return Err(ConfigError::Validation(format!(
                    "screens.{name} must define at least one asset variant"
                )));
            }
        }
        Ok(())
    }

    fn validate_menus(&self) -> Result<(), ConfigError> {
        for (name, menu) in &self.menus {
            if !self.screens.contains_key(&menu.screen) {
                return Err(ConfigError::Validation(format!(
                    "menus.{name}.screen references missing screen {:?}",
                    menu.screen
                )));
            }
            if menu.items.is_empty() {
                return Err(ConfigError::Validation(format!(
                    "menus.{name} must define at least one item"
                )));
            }

            let mut keys = HashSet::new();
            for item in &menu.items {
                let mut key_chars = item.key.chars();
                let Some(key) = key_chars.next() else {
                    return Err(ConfigError::Validation(format!(
                        "menus.{name} item key {:?} must be exactly one ASCII character",
                        item.key
                    )));
                };
                if key_chars.next().is_some() || !key.is_ascii() {
                    return Err(ConfigError::Validation(format!(
                        "menus.{name} item key {:?} must be exactly one ASCII character",
                        item.key
                    )));
                }

                let normalized = key.to_ascii_uppercase().to_string();
                if !keys.insert(normalized.clone()) {
                    return Err(ConfigError::Validation(format!(
                        "menus.{name} has duplicate key {normalized:?}"
                    )));
                }

                match parse_menu_action(&item.action, item.target.as_deref())? {
                    MenuAction::ShowScreen { screen } => {
                        if !self.screens.contains_key(&screen.asset) {
                            return Err(ConfigError::Validation(format!(
                                "menus.{name} show_screen target references missing screen {:?}",
                                screen.asset
                            )));
                        }
                    }
                    MenuAction::Submenu { menu_id } if !self.menus.contains_key(&menu_id) => {
                        return Err(ConfigError::Validation(format!(
                            "menus.{name} submenu target references missing menu {menu_id:?}"
                        )));
                    }
                    _ => {}
                }
            }
        }
        Ok(())
    }

    fn validate_network(&self) -> Result<(), ConfigError> {
        for (key, profile) in &self.network.profiles {
            validate_config_key("network.profiles", key)?;
            validate_network_adapter(&profile.adapter).map_err(ConfigError::Validation)?;
            if profile.local_address.zone == 0 {
                return Err(ConfigError::Validation(format!(
                    "network.profiles.{key}.local_address.zone must be greater than 0"
                )));
            }
            if profile.local_address.net == 0 {
                return Err(ConfigError::Validation(format!(
                    "network.profiles.{key}.local_address.net must be greater than 0"
                )));
            }
            if profile.local_address.node == 0 {
                return Err(ConfigError::Validation(format!(
                    "network.profiles.{key}.local_address.node must be greater than 0"
                )));
            }
        }

        for (key, link) in &self.network.links {
            validate_config_key("network.links", key)?;
            if link.network.trim().is_empty() {
                return Err(ConfigError::Validation(format!(
                    "network.links.{key}.network must not be blank"
                )));
            }
            let Some(profile) = self.network.profiles.get(&link.network) else {
                return Err(ConfigError::Validation(format!(
                    "network.links.{key}.network references unknown profile {:?}",
                    link.network
                )));
            };
            if link.address.trim().is_empty() {
                return Err(ConfigError::Validation(format!(
                    "network.links.{key}.address must not be blank"
                )));
            }
            if link.host.trim().is_empty() {
                return Err(ConfigError::Validation(format!(
                    "network.links.{key}.host must not be blank"
                )));
            }
            if link.binkp_port == 0 {
                return Err(ConfigError::Validation(format!(
                    "network.links.{key}.binkp_port must be between 1 and 65535"
                )));
            }
            if link.poll_schedule_minutes == 0 {
                return Err(ConfigError::Validation(format!(
                    "network.links.{key}.poll_schedule_minutes must be greater than 0"
                )));
            }

            let compression =
                validate_bundle_compression(&link.compression).map_err(ConfigError::Validation)?;
            let transport_security = validate_transport_security(&link.transport_security)
                .map_err(ConfigError::Validation)?;
            let adapter = profile.adapter.trim().to_ascii_lowercase();

            if transport_security == "plaintext_legacy" && adapter != "legacy-ftn" {
                return Err(ConfigError::Validation(format!(
                    "network.links.{key}.transport_security plaintext_legacy is allowed only for legacy-ftn profiles"
                )));
            }
            if transport_security == "tls_opportunistic" && !link.legacy_compatible {
                return Err(ConfigError::Validation(format!(
                    "network.links.{key}.transport_security tls_opportunistic requires legacy_compatible = true"
                )));
            }
            if link.legacy_compatible && adapter != "legacy-ftn" {
                return Err(ConfigError::Validation(format!(
                    "network.links.{key}.legacy_compatible is allowed only for legacy-ftn profiles"
                )));
            }
            if compression == "arj" && adapter != "legacy-ftn" {
                return Err(ConfigError::Validation(format!(
                    "network.links.{key}.compression arj is allowed only for legacy-ftn profiles"
                )));
            }
        }

        Ok(())
    }

    pub fn core_menu(&self, menu_id: &str) -> Result<Menu, ConfigError> {
        let menu = self.menus.get(menu_id).ok_or_else(|| {
            ConfigError::Validation(format!("menu {menu_id:?} is not configured"))
        })?;

        let entries = menu
            .items
            .iter()
            .map(|item| {
                Ok(MenuEntry {
                    key: item.key.clone(),
                    label: item.label.clone(),
                    action: parse_menu_action(&item.action, item.target.as_deref())?,
                    min_security_level: item.min_security_level,
                })
            })
            .collect::<Result<Vec<_>, ConfigError>>()?;

        let core_menu = Menu {
            id: menu_id.to_string(),
            title: menu_id.to_string(),
            description: Some(menu.prompt.clone()),
            screen: ScreenAsset {
                asset: menu.screen.clone(),
            },
            entries,
            pre_menu_screens: self
                .flow
                .post_login_screens
                .iter()
                .cloned()
                .map(|screen| ScreenAsset { asset: screen })
                .collect(),
        };
        core_menu.validate().map_err(|error| {
            ConfigError::Validation(format!("menu {menu_id:?} failed validation: {error}"))
        })?;
        Ok(core_menu)
    }
}

impl TerminalConfig {
    pub fn default_capabilities(&self) -> Result<TerminalCapabilities, ConfigError> {
        self.capabilities_for_profile(&self.default_profile)
    }

    pub fn capabilities_for_profile(
        &self,
        profile_key: &str,
    ) -> Result<TerminalCapabilities, ConfigError> {
        let profile = self.profiles.get(profile_key).ok_or_else(|| {
            ConfigError::Validation(format!(
                "terminal profile {profile_key:?} is not configured"
            ))
        })?;
        profile.to_capabilities(profile_key)
    }
}

impl TerminalProfileConfig {
    fn to_capabilities(&self, profile_key: &str) -> Result<TerminalCapabilities, ConfigError> {
        Ok(TerminalCapabilities {
            profile: terminal_profile_kind(profile_key),
            supports_ansi: self.supports_ansi,
            supports_color: self.supports_color,
            width: self.width,
            height: self.height,
            charset: terminal_charset(&self.charset).map_err(ConfigError::Validation)?,
            line_endings: terminal_line_endings(&self.line_endings)
                .map_err(ConfigError::Validation)?,
            backspace_mode: terminal_backspace_mode(&self.backspace_mode)
                .map_err(ConfigError::Validation)?,
            output_pacing: self
                .output_pacing_bytes_per_second
                .map(|bytes_per_second| OutputPacing { bytes_per_second }),
        })
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
    "127.0.0.1:2323".into()
}
fn default_failed_login_threshold() -> i64 {
    5
}
fn default_failed_login_window_minutes() -> i64 {
    10
}
fn default_failed_login_lockout_minutes() -> i64 {
    15
}
fn default_new_user_security_level() -> i32 {
    10
}
fn default_argon2_memory_cost_kib() -> u32 {
    19_456
}
fn default_argon2_iterations() -> u32 {
    2
}
fn default_argon2_parallelism() -> u32 {
    1
}
fn default_audit_retention_days() -> i64 {
    365
}
fn default_logging_level() -> String {
    "info".into()
}
fn default_logging_file_name() -> String {
    "oxidebbs-server.log".into()
}
fn default_logging_format() -> String {
    "text".into()
}
fn default_logging_rotation_strategy() -> String {
    "daily".into()
}
fn default_logging_rotation_max_size_mb() -> u64 {
    50
}
fn default_logging_rotation_max_files() -> usize {
    14
}
fn default_max_connections() -> u32 {
    4
}
fn default_idle_timeout() -> u64 {
    900
}
fn default_db_path() -> PathBuf {
    PathBuf::from("./data").join(DEFAULT_DATABASE_FILE_NAME)
}
fn default_ansi_path() -> PathBuf {
    PathBuf::from("./assets/ansi")
}
fn default_screens_path() -> PathBuf {
    PathBuf::from("./assets/screens")
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
fn default_terminal_profile() -> String {
    "plain".into()
}
fn default_terminal_profile_name() -> String {
    "Plain ASCII".into()
}
fn default_terminal_profile_width() -> u16 {
    80
}
fn default_terminal_profile_height() -> u16 {
    25
}
fn default_ascii_charset() -> String {
    "ascii".into()
}
fn default_crlf_line_endings() -> String {
    "crlf".into()
}
fn default_backspace_mode() -> String {
    "backspace_or_delete".into()
}
fn default_terminal_profiles() -> HashMap<String, TerminalProfileConfig> {
    let mut profiles = HashMap::new();
    profiles.insert(
        "ansi80".to_string(),
        TerminalProfileConfig {
            name: "ANSI / CP437 80-column".to_string(),
            width: 80,
            height: 25,
            supports_ansi: true,
            supports_color: true,
            charset: "cp437".to_string(),
            line_endings: "crlf".to_string(),
            backspace_mode: "backspace_or_delete".to_string(),
            output_pacing_bytes_per_second: None,
        },
    );
    profiles.insert(
        "plain".to_string(),
        TerminalProfileConfig {
            name: "Plain ASCII".to_string(),
            width: 80,
            height: 25,
            supports_ansi: false,
            supports_color: false,
            charset: "ascii".to_string(),
            line_endings: "crlf".to_string(),
            backspace_mode: "backspace_or_delete".to_string(),
            output_pacing_bytes_per_second: None,
        },
    );
    profiles.insert(
        "c64".to_string(),
        TerminalProfileConfig {
            name: "C64 / C64 Ultimate 40-column".to_string(),
            width: 40,
            height: 25,
            supports_ansi: false,
            supports_color: false,
            charset: "petscii_ascii_fallback".to_string(),
            line_endings: "crlf".to_string(),
            backspace_mode: "backspace_or_delete".to_string(),
            output_pacing_bytes_per_second: Some(1_200),
        },
    );
    profiles
}
fn default_welcome_screen() -> String {
    "welcome.ans".into()
}
fn default_logoff_screen() -> String {
    "logoff.ans".into()
}
fn default_login_screen() -> String {
    "login".into()
}
fn default_login_menu() -> String {
    "login".into()
}
fn default_main_menu() -> String {
    "main".into()
}
fn default_menu_prompt() -> String {
    "Command? ".into()
}
fn default_door_runner() -> String {
    "dosemu".into()
}
fn default_door_allowed_runners() -> Vec<String> {
    vec!["dosemu".to_string(), "dosemu2".to_string()]
}
fn default_door_time_limit() -> u32 {
    30
}
fn default_network_name() -> String {
    "OxideNet".into()
}
fn default_network_adapter() -> String {
    "legacy-ftn".into()
}
fn default_binkp_port() -> u16 {
    24_554
}
fn default_poll_schedule_minutes() -> u32 {
    60
}
fn default_bundle_compression() -> String {
    "zip".into()
}
fn default_transport_security() -> String {
    "tls_required".into()
}
fn default_serial_baud_rate() -> u32 {
    115_200
}
fn default_serial_flow_control() -> String {
    "rtscts".into()
}
fn default_file_transfers_max_upload_bytes() -> i64 {
    1_048_576
}
fn default_admin_web_bind() -> String {
    "127.0.0.1:8080".into()
}
fn default_admin_web_session_timeout_seconds() -> u64 {
    900
}
fn default_admin_web_csrf_token_ttl_seconds() -> u64 {
    900
}
fn default_admin_web_replay_window_seconds() -> u64 {
    300
}
fn default_admin_web_rate_limit_per_minute() -> u32 {
    30
}

fn ip_is_loopback(ip: IpAddr) -> bool {
    ip.is_loopback()
}

fn validate_config_key(section: &str, key: &str) -> Result<(), ConfigError> {
    if key.trim().is_empty() {
        return Err(ConfigError::Validation(format!(
            "{section} keys must not be blank"
        )));
    }
    if key.chars().any(char::is_whitespace) {
        return Err(ConfigError::Validation(format!(
            "{section}.{key} keys must not contain whitespace"
        )));
    }
    Ok(())
}

fn validate_network_adapter(adapter: &str) -> Result<&'static str, String> {
    match adapter.trim().to_ascii_lowercase().as_str() {
        "legacy-ftn" => Ok("legacy-ftn"),
        "oxidenet" => Ok("oxidenet"),
        other => Err(format!(
            "network profile adapter must be one of legacy-ftn or oxidenet, got {other:?}"
        )),
    }
}

fn validate_bundle_compression(compression: &str) -> Result<&'static str, String> {
    match compression.trim().to_ascii_lowercase().as_str() {
        "none" => Ok("none"),
        "zip" => Ok("zip"),
        "arj" => Ok("arj"),
        other => Err(format!(
            "network link compression must be one of none, zip, or arj, got {other:?}"
        )),
    }
}

fn validate_transport_security(transport_security: &str) -> Result<&'static str, String> {
    match transport_security.trim().to_ascii_lowercase().as_str() {
        "tls_required" => Ok("tls_required"),
        "tls_opportunistic" => Ok("tls_opportunistic"),
        "plaintext_legacy" => Ok("plaintext_legacy"),
        other => Err(format!(
            "network link transport_security must be one of tls_required, tls_opportunistic, or plaintext_legacy, got {other:?}"
        )),
    }
}

fn terminal_profile_kind(profile_key: &str) -> TerminalProfile {
    match profile_key.trim().to_ascii_lowercase().as_str() {
        "ansi80" | "ansi-80" | "ansi_80" => TerminalProfile::Ansi80,
        "ansi40" | "ansi-40" | "ansi_40" => TerminalProfile::Ansi40,
        "c64" | "c64-40" | "petscii40" | "petscii-40" => TerminalProfile::C64,
        _ => TerminalProfile::PlainAscii,
    }
}

fn validate_terminal_charset(charset: &str) -> Result<&'static str, String> {
    match charset.trim().to_ascii_lowercase().as_str() {
        "cp437" => Ok("cp437"),
        "ascii" => Ok("ascii"),
        "petscii_ascii_fallback" | "petscii-ascii-fallback" | "petscii40" => {
            Ok("petscii_ascii_fallback")
        }
        other => Err(format!(
            "terminal charset must be one of cp437, ascii, or petscii_ascii_fallback, got {other:?}"
        )),
    }
}

fn terminal_charset(charset: &str) -> Result<TerminalCharset, String> {
    Ok(match validate_terminal_charset(charset)? {
        "cp437" => TerminalCharset::Cp437,
        "petscii_ascii_fallback" => TerminalCharset::PetsciiAsciiFallback,
        _ => TerminalCharset::Ascii,
    })
}

fn validate_terminal_line_endings(line_endings: &str) -> Result<&'static str, String> {
    match line_endings.trim().to_ascii_lowercase().as_str() {
        "crlf" => Ok("crlf"),
        other => Err(format!("terminal line_endings must be crlf, got {other:?}")),
    }
}

fn terminal_line_endings(line_endings: &str) -> Result<LineEndingMode, String> {
    validate_terminal_line_endings(line_endings)?;
    Ok(LineEndingMode::Crlf)
}

fn validate_terminal_backspace_mode(backspace_mode: &str) -> Result<&'static str, String> {
    match backspace_mode.trim().to_ascii_lowercase().as_str() {
        "backspace_or_delete" | "backspace-or-delete" | "both" => Ok("backspace_or_delete"),
        other => Err(format!(
            "terminal backspace_mode must be backspace_or_delete, got {other:?}"
        )),
    }
}

fn terminal_backspace_mode(backspace_mode: &str) -> Result<BackspaceMode, String> {
    validate_terminal_backspace_mode(backspace_mode)?;
    Ok(BackspaceMode::BackspaceOrDelete)
}

pub(crate) fn validate_logging_level(level: &str) -> Result<(), String> {
    match level.trim().to_ascii_lowercase().as_str() {
        "error" | "warn" | "info" | "debug" | "trace" => Ok(()),
        other => Err(format!(
            "logging.level must be one of error, warn, info, debug, or trace, got {other:?}"
        )),
    }
}

pub(crate) fn validate_logging_format(format: &str) -> Result<(), String> {
    match format.trim().to_ascii_lowercase().as_str() {
        "text" | "json" => Ok(()),
        other => Err(format!(
            "logging.format must be one of text or json, got {other:?}"
        )),
    }
}

pub(crate) fn validate_logging_rotation_strategy(strategy: &str) -> Result<(), String> {
    match strategy.trim().to_ascii_lowercase().as_str() {
        "never" | "daily" | "size" => Ok(()),
        other => Err(format!(
            "logging.rotation.strategy must be one of never, daily, or size, got {other:?}"
        )),
    }
}

fn validate_logging_file_name(file_name: &str) -> Result<(), String> {
    let file_name = file_name.trim();
    if file_name.is_empty() {
        return Err("logging.file_name must not be blank".into());
    }
    if file_name.contains('/') || file_name.contains('\\') || Path::new(file_name).is_absolute() {
        return Err("logging.file_name must be a file name under paths.logs, not a path".into());
    }
    Ok(())
}

pub fn normalize_database_path(path: impl AsRef<Path>) -> PathBuf {
    let path = path.as_ref();
    if path.is_dir() || path_has_trailing_separator(path) {
        path.join(DEFAULT_DATABASE_FILE_NAME)
    } else {
        path.to_path_buf()
    }
}

fn path_has_trailing_separator(path: &Path) -> bool {
    let raw = path.as_os_str().to_string_lossy();
    raw.ends_with('/') || raw.ends_with('\\')
}

fn parse_menu_action(action: &str, target: Option<&str>) -> Result<MenuAction, ConfigError> {
    match action.trim().to_ascii_lowercase().as_str() {
        "doors" => Ok(MenuAction::Doors),
        "messages" => Ok(MenuAction::Messages),
        "logoff" => Ok(MenuAction::Logoff),
        "new_user" | "new-user" | "newuser" => Ok(MenuAction::NewUser),
        "login" | "logon" => Ok(MenuAction::Login),
        "noop" => Ok(MenuAction::Noop),
        "show_screen" | "show-screen" | "show" => {
            let screen = target.ok_or_else(|| {
                ConfigError::Validation("show_screen menu action requires target".into())
            })?;
            Ok(MenuAction::ShowScreen {
                screen: ScreenAsset {
                    asset: screen.to_string(),
                },
            })
        }
        "submenu" => {
            let menu_id = target.ok_or_else(|| {
                ConfigError::Validation("submenu menu action requires target".into())
            })?;
            Ok(MenuAction::Submenu {
                menu_id: menu_id.to_string(),
            })
        }
        other => Err(ConfigError::Validation(format!(
            "unsupported menu action {other:?}"
        ))),
    }
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

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            failed_login_threshold: default_failed_login_threshold(),
            failed_login_window_minutes: default_failed_login_window_minutes(),
            failed_login_lockout_minutes: default_failed_login_lockout_minutes(),
            new_user_security_level: default_new_user_security_level(),
            argon2: Argon2Config::default(),
        }
    }
}

impl Default for Argon2Config {
    fn default() -> Self {
        Self {
            memory_cost_kib: default_argon2_memory_cost_kib(),
            iterations: default_argon2_iterations(),
            parallelism: default_argon2_parallelism(),
        }
    }
}

impl Default for AuditConfig {
    fn default() -> Self {
        Self {
            retention_days: default_audit_retention_days(),
        }
    }
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: default_logging_level(),
            file_enabled: default_true(),
            file_name: default_logging_file_name(),
            format: default_logging_format(),
            rotation: LoggingRotationConfig::default(),
        }
    }
}

impl Default for LoggingRotationConfig {
    fn default() -> Self {
        Self {
            strategy: default_logging_rotation_strategy(),
            max_size_mb: default_logging_rotation_max_size_mb(),
            max_files: default_logging_rotation_max_files(),
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
            screens: default_screens_path(),
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

impl Default for SysopConfig {
    fn default() -> Self {
        Self {
            confirm_quit: default_true(),
        }
    }
}

impl Default for TerminalConfig {
    fn default() -> Self {
        Self {
            default_encoding: default_encoding(),
            default_profile: default_terminal_profile(),
            manual_profile_selection: default_true(),
            clear_screen_on_connect: default_true(),
            welcome_screen: default_welcome_screen(),
            logoff_screen: default_logoff_screen(),
            profiles: default_terminal_profiles(),
        }
    }
}

impl Default for FlowConfig {
    fn default() -> Self {
        Self {
            login_screen: default_login_screen(),
            login_menu: default_login_menu(),
            post_login_screens: Vec::new(),
            main_menu: default_main_menu(),
        }
    }
}

impl Default for DoorsConfig {
    fn default() -> Self {
        Self {
            enabled: default_true(),
            default_runner: default_door_runner(),
            allowed_runners: default_door_allowed_runners(),
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

impl Default for AdminWebConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            bind: default_admin_web_bind(),
            require_tls: default_true(),
            read_only: default_true(),
            session_timeout_seconds: default_admin_web_session_timeout_seconds(),
            csrf_token_ttl_seconds: default_admin_web_csrf_token_ttl_seconds(),
            replay_window_seconds: default_admin_web_replay_window_seconds(),
            rate_limit_per_minute: default_admin_web_rate_limit_per_minute(),
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
        assert!(config.sysop.confirm_quit);
        assert_eq!(config.paths.ansi, PathBuf::from("./assets/ansi"));
        assert_eq!(config.paths.screens, PathBuf::from("./assets/screens"));
    }

    #[test]
    fn parses_sysop_tui_quit_confirmation_setting() {
        let toml = r#"
[board]
name = "Test"

[sysop]
confirm_quit = false
"#;
        let config: OxideConfig = toml::from_str(toml).expect("parse");
        assert!(!config.sysop.confirm_quit);
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
        assert_eq!(config.telnet.bind, "127.0.0.1:2323");
        assert_eq!(config.auth.failed_login_threshold, 5);
        assert_eq!(config.auth.failed_login_window_minutes, 10);
        assert_eq!(config.auth.failed_login_lockout_minutes, 15);
        assert_eq!(config.auth.new_user_security_level, 10);
        assert_eq!(config.auth.argon2.memory_cost_kib, 19_456);
        assert_eq!(config.auth.argon2.iterations, 2);
        assert_eq!(config.auth.argon2.parallelism, 1);
        assert_eq!(config.audit.retention_days, 365);
        assert!(config.sysop.confirm_quit);
        assert_eq!(config.logging.level, "info");
        assert!(config.logging.file_enabled);
        assert_eq!(config.logging.file_name, "oxidebbs-server.log");
        assert_eq!(config.logging.format, "text");
        assert_eq!(config.logging.rotation.strategy, "daily");
        assert_eq!(config.logging.rotation.max_size_mb, 50);
        assert_eq!(config.logging.rotation.max_files, 14);
        assert_eq!(
            config.doors.allowed_runners,
            vec!["dosemu".to_string(), "dosemu2".to_string()]
        );
        assert_eq!(config.database.path, PathBuf::from("./data/oxidebbs.ddb"));
        assert_eq!(config.nodes.count, 4);
        assert_eq!(config.terminal.default_encoding, "cp437");
        assert_eq!(config.terminal.default_profile, "plain");
        assert!(config.terminal.manual_profile_selection);
        assert!(config.terminal.profiles.contains_key("ansi80"));
        assert!(config.terminal.profiles.contains_key("plain"));
        assert!(config.terminal.profiles.contains_key("c64"));
        assert_eq!(
            config
                .terminal
                .capabilities_for_profile("c64")
                .expect("c64 profile"),
            TerminalCapabilities::c64()
        );
        assert!(!config.network.enabled);
        assert!(config.network.profiles.is_empty());
        assert!(config.network.links.is_empty());
        assert!(!config.ftn.enabled);
        assert!(!config.admin_web.enabled);
        assert_eq!(config.admin_web.bind, "127.0.0.1:8080");
        assert!(config.admin_web.require_tls);
        assert!(config.admin_web.read_only);
    }

    #[test]
    fn normalizes_database_directory_path_to_default_database_file() {
        let mut dir = std::env::temp_dir();
        dir.push(format!(
            "oxidebbs-db-path-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system time should be valid")
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).expect("create db dir");

        assert_eq!(
            normalize_database_path(&dir),
            dir.join(DEFAULT_DATABASE_FILE_NAME)
        );
        assert_eq!(
            normalize_database_path(PathBuf::from("data/")),
            PathBuf::from("data").join(DEFAULT_DATABASE_FILE_NAME)
        );

        let explicit_file = dir.join("custom.ddb");
        assert_eq!(normalize_database_path(&explicit_file), explicit_file);

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn rejects_invalid_logging_level() {
        let toml = r#"
[board]
name = "Bad Logging"

[logging]
level = "debgu"
"#;
        let config: OxideConfig = toml::from_str(toml).expect("parse");
        let error = config.validate().expect_err("invalid level rejected");
        assert!(
            error
                .to_string()
                .contains("logging.level must be one of error, warn, info, debug, or trace")
        );
    }

    #[test]
    fn rejects_logging_file_name_paths() {
        let toml = r#"
[board]
name = "Bad Logging File"

[logging]
file_name = "../server.log"
"#;
        let config: OxideConfig = toml::from_str(toml).expect("parse");
        let error = config.validate().expect_err("path file name rejected");
        assert!(
            error
                .to_string()
                .contains("logging.file_name must be a file name")
        );
    }

    #[test]
    fn rejects_invalid_logging_format() {
        let toml = r#"
[board]
name = "Bad Logging Format"

[logging]
format = "yaml"
"#;
        let config: OxideConfig = toml::from_str(toml).expect("parse");
        let error = config.validate().expect_err("invalid format rejected");
        assert!(
            error
                .to_string()
                .contains("logging.format must be one of text or json")
        );
    }

    #[test]
    fn rejects_invalid_logging_rotation() {
        let toml = r#"
[board]
name = "Bad Logging Rotation"

[logging.rotation]
strategy = "weekly"
"#;
        let config: OxideConfig = toml::from_str(toml).expect("parse");
        let error = config.validate().expect_err("invalid rotation rejected");
        assert!(
            error
                .to_string()
                .contains("logging.rotation.strategy must be one of never, daily, or size")
        );
    }

    #[test]
    fn parses_disabled_admin_web_defaults() {
        let toml = r#"
[board]
name = "Admin Web Defaults"

[admin_web]
enabled = false
"#;
        let config: OxideConfig = toml::from_str(toml).expect("parse");

        config
            .validate_admin_web()
            .expect("disabled admin web validates");
        assert!(!config.admin_web.enabled);
        assert_eq!(config.admin_web.session_timeout_seconds, 900);
        assert_eq!(config.admin_web.csrf_token_ttl_seconds, 900);
        assert_eq!(config.admin_web.replay_window_seconds, 300);
        assert_eq!(config.admin_web.rate_limit_per_minute, 30);
    }

    #[test]
    fn rejects_admin_web_non_loopback_without_tls() {
        let toml = r#"
[board]
name = "Bad Admin Web"

[admin_web]
enabled = true
bind = "0.0.0.0:8080"
require_tls = false
"#;
        let config: OxideConfig = toml::from_str(toml).expect("parse");
        let error = config
            .validate_admin_web()
            .expect_err("public admin web without TLS rejected");

        assert!(
            error
                .to_string()
                .contains("admin_web.require_tls must be true")
        );
    }

    #[test]
    fn rejects_admin_web_mutation_mode_until_http_surface_exists() {
        let toml = r#"
[board]
name = "Bad Admin Web"

[admin_web]
enabled = true
read_only = false
"#;
        let config: OxideConfig = toml::from_str(toml).expect("parse");
        let error = config
            .validate_admin_web()
            .expect_err("mutable remote admin rejected");

        assert!(
            error
                .to_string()
                .contains("admin_web.read_only must remain true")
        );
    }

    #[test]
    fn parses_door_definitions() {
        let toml = r#"
[board]
name = "Test"

[doors]
enabled = true
default_runner = "dosemu"

[[doors.definitions]]
key = "lord"
name = "Legend of the Red Dragon"
runner = "dosemu"
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

    #[test]
    fn validates_repository_example_config() {
        let config: OxideConfig =
            toml::from_str(include_str!("../../../config/oxidebbs.example.toml")).expect("parse");

        config.validate().expect("validate example config");
        assert_eq!(config.flow.login_screen, "login");
        assert_eq!(config.flow.login_menu, "login");
        assert_eq!(config.flow.post_login_screens, ["screen1", "screen2"]);
        assert_eq!(config.flow.main_menu, "main");
        assert_eq!(
            config.screens["main_menu"].ansi.as_deref(),
            Some("menus/main/main.ans")
        );
        assert_eq!(config.menus["main"].items[0].key, "D");
    }

    #[test]
    fn parses_multiple_network_profiles_with_independent_addresses() {
        let toml = r#"
[board]
name = "Network BBS"

[network]
enabled = true

[network.profiles.fidonet]
name = "FidoNet"
adapter = "legacy-ftn"

[network.profiles.fidonet.local_address]
zone = 1
net = 105
node = 42
point = 0

[network.profiles.oxidenet]
name = "OxideNet"
adapter = "oxidenet"

[network.profiles.oxidenet.local_address]
zone = 42
net = 1
node = 7
point = 0

[network.links.fidonet_hub]
network = "fidonet"
address = "1:105/0"
host = "fidonet.example.net"
transport_security = "plaintext_legacy"
legacy_compatible = true

[network.links.oxide_hub]
network = "oxidenet"
address = "42:1/0"
host = "hub.oxidebbs.net"
compression = "none"
transport_security = "tls_required"
"#;
        let config: OxideConfig = toml::from_str(toml).expect("parse network config");

        config.validate_network().expect("validate network config");
        assert!(config.network.enabled);
        assert_eq!(config.network.profiles["fidonet"].local_address.zone, 1);
        assert_eq!(config.network.profiles["oxidenet"].local_address.zone, 42);
    }

    #[test]
    fn rejects_network_link_with_unknown_profile_key() {
        let toml = r#"
[board]
name = "Network BBS"

[network.links.unknown_hub]
network = "missing"
address = "1:105/0"
host = "fidonet.example.net"
"#;
        let config: OxideConfig = toml::from_str(toml).expect("parse network config");
        let error = config
            .validate_network()
            .expect_err("unknown network profile rejected");

        assert!(
            error
                .to_string()
                .contains("network.links.unknown_hub.network references unknown profile")
        );
    }

    #[test]
    fn rejects_plaintext_legacy_on_non_legacy_network_profile() {
        let toml = r#"
[board]
name = "Network BBS"

[network.profiles.oxidenet]
adapter = "oxidenet"

[network.profiles.oxidenet.local_address]
zone = 42
net = 1
node = 7

[network.links.oxide_hub]
network = "oxidenet"
address = "42:1/0"
host = "hub.oxidebbs.net"
transport_security = "plaintext_legacy"
"#;
        let config: OxideConfig = toml::from_str(toml).expect("parse network config");
        let error = config
            .validate_network()
            .expect_err("plaintext legacy rejected on OxideNet");

        assert!(
            error
                .to_string()
                .contains("plaintext_legacy is allowed only for legacy-ftn profiles")
        );
    }

    #[test]
    fn rejects_unknown_network_link_fields() {
        let toml = r#"
[board]
name = "Network BBS"

[network.links.bad]
network = "fidonet"
address = "1:105/0"
host = "fidonet.example.net"
unknown_key = true
"#;
        let error = toml::from_str::<OxideConfig>(toml).expect_err("unknown field rejected");

        assert!(error.to_string().contains("unknown field `unknown_key`"));
    }

    #[test]
    fn parses_deprecated_ftn_compatibility_alias() {
        let toml = r#"
[board]
name = "Legacy"

[ftn]
enabled = true
reserved_network_name = "FidoNet"
"#;
        let config: OxideConfig = toml::from_str(toml).expect("parse ftn alias");

        assert!(config.ftn.enabled);
        assert_eq!(config.ftn.reserved_network_name, "FidoNet");
        assert!(!config.network.enabled);
    }

    #[test]
    fn converts_configured_menu_to_core_router() {
        let config: OxideConfig =
            toml::from_str(include_str!("../../../config/oxidebbs.example.toml")).expect("parse");

        let menu = config.core_menu("main").expect("convert main menu");

        assert_eq!(menu.screen.asset, "main_menu");
        assert_eq!(
            menu.route("d"),
            Some(oxidebbs_core::menu::MenuAction::Doors)
        );
        assert_eq!(
            menu.route("M"),
            Some(oxidebbs_core::menu::MenuAction::Messages)
        );
        assert_eq!(menu.route("z"), None);
        assert_eq!(
            menu.pre_menu_screens
                .iter()
                .map(|screen| screen.asset.as_str())
                .collect::<Vec<_>>(),
            vec!["screen1", "screen2"]
        );
    }

    #[test]
    fn converts_login_menu_to_core_router() {
        let config: OxideConfig =
            toml::from_str(include_str!("../../../config/oxidebbs.example.toml")).expect("parse");

        let menu = config.core_menu("login").expect("convert login menu");

        assert_eq!(menu.screen.asset, "login");
        assert_eq!(
            menu.route("L"),
            Some(oxidebbs_core::menu::MenuAction::Login)
        );
        assert_eq!(
            menu.route("n"),
            Some(oxidebbs_core::menu::MenuAction::NewUser)
        );
        assert_eq!(
            menu.route("G"),
            Some(oxidebbs_core::menu::MenuAction::Logoff)
        );
    }

    #[test]
    fn supports_submenu_action_routing() {
        let toml = r#"
[board]
name = "Test"

[flow]
login_screen = "login"
login_menu = "main"
main_menu = "main"

[screens.login]
text = "login.txt"

[screens.main_menu]
text = "main.txt"

[screens.submenu]
text = "submenu.txt"

[menus.main]
screen = "main_menu"

[[menus.main.items]]
key = "S"
label = "Submenu"
action = "submenu"
target = "submenu"

[[menus.main.items]]
key = "L"
label = "Logoff"
action = "logoff"

[menus.submenu]
screen = "submenu"

[[menus.submenu.items]]
key = "L"
label = "Logoff"
action = "logoff"
"#;
        let config: OxideConfig = toml::from_str(toml).expect("parse");

        assert!(config.validate().is_ok());
        let menu = config
            .core_menu("main")
            .expect("convert main menu with submenu");
        assert_eq!(
            menu.route("S"),
            Some(oxidebbs_core::menu::MenuAction::Submenu {
                menu_id: "submenu".to_string()
            })
        );
    }

    #[test]
    fn rejects_duplicate_menu_keys() {
        let toml = r#"
[board]
name = "Test"

[flow]
login_screen = "login"
login_menu = "main"
main_menu = "main"

[screens.login]
text = "login.txt"

[screens.main_menu]
text = "main.txt"

[menus.main]
screen = "main_menu"

[[menus.main.items]]
key = "D"
label = "Doors"
action = "doors"

[[menus.main.items]]
key = "d"
label = "Duplicate"
action = "messages"
"#;
        let config: OxideConfig = toml::from_str(toml).expect("parse");

        assert!(config.validate().is_err());
    }

    #[test]
    fn rejects_unknown_menu_action() {
        let toml = r#"
[board]
name = "Test"

[flow]
login_screen = "login"
login_menu = "main"
main_menu = "main"

[screens.login]
text = "login.txt"

[screens.main_menu]
text = "main.txt"

[menus.main]
screen = "main_menu"

[[menus.main.items]]
key = "X"
label = "Unsafe"
action = "shell"
"#;
        let config: OxideConfig = toml::from_str(toml).expect("parse");

        assert!(config.validate().is_err());
    }

    #[test]
    fn rejects_unknown_default_terminal_profile() {
        let toml = r#"
[board]
name = "Test"

[terminal]
default_profile = "missing"
"#;
        let config: OxideConfig = toml::from_str(toml).expect("parse");
        let error = config
            .validate_terminal()
            .expect_err("unknown terminal profile rejected");

        assert!(
            error
                .to_string()
                .contains("terminal.default_profile references missing profile")
        );
    }

    #[test]
    fn parses_custom_c64_terminal_profile_contract() {
        let toml = r#"
[board]
name = "Test"

[terminal]
default_profile = "c64"

[terminal.profiles.c64]
name = "C64 / C64 Ultimate 40-column"
width = 40
height = 25
supports_ansi = false
supports_color = false
charset = "petscii_ascii_fallback"
line_endings = "crlf"
backspace_mode = "backspace_or_delete"
output_pacing_bytes_per_second = 600
"#;
        let config: OxideConfig = toml::from_str(toml).expect("parse");
        config.validate_terminal().expect("validate terminal");

        let capabilities = config
            .terminal
            .default_capabilities()
            .expect("default capabilities");
        assert_eq!(capabilities.profile, TerminalProfile::C64);
        assert_eq!(capabilities.width, 40);
        assert_eq!(capabilities.height, 25);
        assert!(!capabilities.supports_ansi);
        assert_eq!(
            capabilities.output_pacing,
            Some(OutputPacing {
                bytes_per_second: 600
            })
        );
    }

    #[test]
    fn rejects_non_ascii_menu_key() {
        let toml = r#"
[board]
name = "Test"

[flow]
login_screen = "login"
login_menu = "main"
main_menu = "main"

[screens.login]
text = "login.txt"

[screens.main_menu]
text = "main.txt"

[menus.main]
screen = "main_menu"

[[menus.main.items]]
key = "é"
label = "Unsafe"
action = "messages"
"#;
        let config: OxideConfig = toml::from_str(toml).expect("parse");

        assert!(config.validate().is_err());
    }

    #[test]
    fn selects_screen_asset_for_terminal_capabilities() {
        let config: OxideConfig =
            toml::from_str(include_str!("../../../config/oxidebbs.example.toml")).expect("parse");
        let screen = &config.screens["login"];

        assert_eq!(
            screen.asset_for(TerminalCapabilities::ansi_80()),
            Some("login/login.ans")
        );
        assert_eq!(
            screen.asset_for(TerminalCapabilities::ansi_40()),
            Some("login/login-40.ans")
        );
        assert_eq!(
            screen.asset_for(TerminalCapabilities::plain_text()),
            Some("login/login.asc")
        );
        assert_eq!(
            screen.asset_for(TerminalCapabilities::c64()),
            Some("login/login.asc")
        );
    }
}
