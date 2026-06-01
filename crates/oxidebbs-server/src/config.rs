#![allow(dead_code)]

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use serde::Deserialize;
use thiserror::Error;

use oxidebbs_core::menu::{Menu, MenuAction, MenuEntry, ScreenAsset};

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
    pub flow: FlowConfig,
    #[serde(default)]
    pub screens: HashMap<String, ScreenConfig>,
    #[serde(default)]
    pub menus: HashMap<String, MenuConfig>,
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
    pub ascii: Option<String>,
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

        self.ascii.as_deref().or(self.text.as_deref())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalCapabilities {
    pub supports_ansi: bool,
    pub width: u16,
}

impl TerminalCapabilities {
    pub fn ansi_80() -> Self {
        Self {
            supports_ansi: true,
            width: 80,
        }
    }

    pub fn ansi_40() -> Self {
        Self {
            supports_ansi: true,
            width: 40,
        }
    }

    pub fn plain_text() -> Self {
        Self {
            supports_ansi: false,
            width: 80,
        }
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
        self.validate_flow()?;
        self.validate_screens()?;
        self.validate_menus()?;
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

    fn validate_screens(&self) -> Result<(), ConfigError> {
        for (name, screen) in &self.screens {
            if screen.ansi.is_none()
                && screen.ansi_40.is_none()
                && screen.ascii.is_none()
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
    "dosbox".into()
}
fn default_door_time_limit() -> u32 {
    30
}
fn default_network_name() -> String {
    "OxideNet".into()
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
        assert_eq!(config.paths.screens, PathBuf::from("./assets/screens"));
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
    }
}
