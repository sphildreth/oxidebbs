use std::collections::{BTreeMap, BTreeSet};
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};

use serde::Serialize;

const DEFAULT_BOARD_NAME: &str = "OxideBBS";
const DEFAULT_TAGLINE: &str = "Built for sysops. Driven by code.";
const DEFAULT_SYSOP_NAME: &str = "Sysop";
const DEFAULT_SYSOP_ALIAS: &str = "sysop";
const DEFAULT_TIMEZONE: &str = "America/Chicago";
const DEFAULT_TELNET_BIND: &str = "127.0.0.1:2323";
const DEFAULT_NODE_COUNT: u16 = 4;
const DEFAULT_FAILED_LOGIN_THRESHOLD: i64 = 5;
const DEFAULT_FAILED_LOGIN_WINDOW_MINUTES: i64 = 10;
const DEFAULT_FAILED_LOGIN_LOCKOUT_MINUTES: i64 = 15;
const DEFAULT_NEW_USER_SECURITY_LEVEL: i32 = 10;
const DEFAULT_ARGON2_MEMORY_COST_KIB: u32 = 19_456;
const DEFAULT_ARGON2_ITERATIONS: u32 = 2;
const DEFAULT_ARGON2_PARALLELISM: u32 = 1;
const DEFAULT_AUDIT_RETENTION_DAYS: i64 = 365;
const DEFAULT_DATABASE_PATH: &str = "./data/oxidebbs.ddb";
const DEFAULT_ANSI_PATH: &str = "./assets/ansi";
const DEFAULT_SCREENS_PATH: &str = "./assets/screens";
const DEFAULT_DOORS_PATH: &str = "./doors";
const DEFAULT_RUNTIME_PATH: &str = "./runtime";
const DEFAULT_LOGS_PATH: &str = "./logs";
const DEFAULT_DOSEMU: &str = "dosemu";
const DEFAULT_ALLOWED_RUNNERS: &[&str] = &["dosemu", "dosemu2"];

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum DefaultAssetRoot {
    Ansi,
    Screens,
}

#[derive(Debug, Clone, Copy)]
struct DefaultAsset {
    root: DefaultAssetRoot,
    path: &'static str,
    bytes: &'static [u8],
}

const DEFAULT_ASSETS: &[DefaultAsset] = &[
    DefaultAsset {
        root: DefaultAssetRoot::Ansi,
        path: "logoff.ans",
        bytes: include_bytes!("../../../assets/ansi/logoff.ans"),
    },
    DefaultAsset {
        root: DefaultAssetRoot::Ansi,
        path: "logoff.asc",
        bytes: include_bytes!("../../../assets/ansi/logoff.asc"),
    },
    DefaultAsset {
        root: DefaultAssetRoot::Ansi,
        path: "welcome-preview.txt",
        bytes: include_bytes!("../../../assets/ansi/welcome-preview.txt"),
    },
    DefaultAsset {
        root: DefaultAssetRoot::Ansi,
        path: "welcome.ans",
        bytes: include_bytes!("../../../assets/ansi/welcome.ans"),
    },
    DefaultAsset {
        root: DefaultAssetRoot::Ansi,
        path: "welcome.asc",
        bytes: include_bytes!("../../../assets/ansi/welcome.asc"),
    },
    DefaultAsset {
        root: DefaultAssetRoot::Screens,
        path: "info/screen1.ans",
        bytes: include_bytes!("../../../assets/screens/info/screen1.ans"),
    },
    DefaultAsset {
        root: DefaultAssetRoot::Screens,
        path: "info/screen1.asc",
        bytes: include_bytes!("../../../assets/screens/info/screen1.asc"),
    },
    DefaultAsset {
        root: DefaultAssetRoot::Screens,
        path: "info/screen1.txt",
        bytes: include_bytes!("../../../assets/screens/info/screen1.txt"),
    },
    DefaultAsset {
        root: DefaultAssetRoot::Screens,
        path: "info/screen2.ans",
        bytes: include_bytes!("../../../assets/screens/info/screen2.ans"),
    },
    DefaultAsset {
        root: DefaultAssetRoot::Screens,
        path: "info/screen2.asc",
        bytes: include_bytes!("../../../assets/screens/info/screen2.asc"),
    },
    DefaultAsset {
        root: DefaultAssetRoot::Screens,
        path: "info/screen2.txt",
        bytes: include_bytes!("../../../assets/screens/info/screen2.txt"),
    },
    DefaultAsset {
        root: DefaultAssetRoot::Screens,
        path: "login/login-40.ans",
        bytes: include_bytes!("../../../assets/screens/login/login-40.ans"),
    },
    DefaultAsset {
        root: DefaultAssetRoot::Screens,
        path: "login/login.ans",
        bytes: include_bytes!("../../../assets/screens/login/login.ans"),
    },
    DefaultAsset {
        root: DefaultAssetRoot::Screens,
        path: "login/login.asc",
        bytes: include_bytes!("../../../assets/screens/login/login.asc"),
    },
    DefaultAsset {
        root: DefaultAssetRoot::Screens,
        path: "login/login.txt",
        bytes: include_bytes!("../../../assets/screens/login/login.txt"),
    },
    DefaultAsset {
        root: DefaultAssetRoot::Screens,
        path: "menus/main/main-40.ans",
        bytes: include_bytes!("../../../assets/screens/menus/main/main-40.ans"),
    },
    DefaultAsset {
        root: DefaultAssetRoot::Screens,
        path: "menus/main/main.ans",
        bytes: include_bytes!("../../../assets/screens/menus/main/main.ans"),
    },
    DefaultAsset {
        root: DefaultAssetRoot::Screens,
        path: "menus/main/main.asc",
        bytes: include_bytes!("../../../assets/screens/menus/main/main.asc"),
    },
    DefaultAsset {
        root: DefaultAssetRoot::Screens,
        path: "menus/main/main.txt",
        bytes: include_bytes!("../../../assets/screens/menus/main/main.txt"),
    },
];

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct DefaultAssetInstallSummary {
    pub installed: usize,
    pub skipped: usize,
}

#[derive(Debug, Clone)]
pub struct SetupAnswers {
    pub board_name: String,
    pub tagline: String,
    pub sysop_name: String,
    pub sysop_alias: String,
    pub sysop_password: String,
    pub timezone: String,
    pub telnet_bind: String,
    pub node_count: u16,
    pub database_path: PathBuf,
    pub include_example_door: bool,
    pub example_door_enabled: bool,
    pub include_sample_ansi: bool,
}

impl Default for SetupAnswers {
    fn default() -> Self {
        Self {
            board_name: DEFAULT_BOARD_NAME.to_string(),
            tagline: DEFAULT_TAGLINE.to_string(),
            sysop_name: DEFAULT_SYSOP_NAME.to_string(),
            sysop_alias: DEFAULT_SYSOP_ALIAS.to_string(),
            sysop_password: String::new(),
            timezone: DEFAULT_TIMEZONE.to_string(),
            telnet_bind: DEFAULT_TELNET_BIND.to_string(),
            node_count: DEFAULT_NODE_COUNT,
            database_path: PathBuf::from(DEFAULT_DATABASE_PATH),
            include_example_door: true,
            example_door_enabled: false,
            include_sample_ansi: true,
        }
    }
}

#[derive(Serialize)]
struct GeneratedConfig {
    board: GeneratedBoardConfig,
    telnet: GeneratedTelnetConfig,
    auth: GeneratedAuthConfig,
    audit: GeneratedAuditConfig,
    database: GeneratedDatabaseConfig,
    paths: GeneratedPathsConfig,
    nodes: GeneratedNodesConfig,
    terminal: GeneratedTerminalConfig,
    flow: GeneratedFlowConfig,
    screens: BTreeMap<String, GeneratedScreenConfig>,
    menus: BTreeMap<String, GeneratedMenuConfig>,
    doors: GeneratedDoorsConfig,
    ftn: GeneratedFtnConfig,
}

#[derive(Serialize)]
struct GeneratedBoardConfig {
    name: String,
    tagline: String,
    sysop_name: String,
    timezone: String,
}

#[derive(Serialize)]
struct GeneratedTelnetConfig {
    bind: String,
}

#[derive(Serialize)]
struct GeneratedAuthConfig {
    failed_login_threshold: i64,
    failed_login_window_minutes: i64,
    failed_login_lockout_minutes: i64,
    new_user_security_level: i32,
    argon2: GeneratedArgon2Config,
}

#[derive(Serialize)]
struct GeneratedArgon2Config {
    memory_cost_kib: u32,
    iterations: u32,
    parallelism: u32,
}

#[derive(Serialize)]
struct GeneratedAuditConfig {
    retention_days: i64,
}

#[derive(Serialize)]
struct GeneratedDatabaseConfig {
    path: String,
}

#[derive(Serialize)]
struct GeneratedPathsConfig {
    ansi: String,
    screens: String,
    doors: String,
    runtime: String,
    logs: String,
}

#[derive(Serialize)]
struct GeneratedNodesConfig {
    count: u16,
}

#[derive(Serialize)]
struct GeneratedTerminalConfig {
    default_encoding: String,
    clear_screen_on_connect: bool,
    welcome_screen: String,
    logoff_screen: String,
}

#[derive(Serialize)]
struct GeneratedFlowConfig {
    login_screen: String,
    login_menu: String,
    post_login_screens: Vec<String>,
    main_menu: String,
}

#[derive(Serialize)]
struct GeneratedScreenConfig {
    ansi: String,
    ansi_40: Option<String>,
    ascii: String,
    text: String,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pause: bool,
}

#[derive(Serialize)]
struct GeneratedMenuConfig {
    screen: String,
    prompt: String,
    items: Vec<GeneratedMenuItemConfig>,
}

#[derive(Serialize)]
struct GeneratedMenuItemConfig {
    key: String,
    label: String,
    action: String,
}

#[derive(Serialize)]
struct GeneratedDoorsConfig {
    enabled: bool,
    default_runner: String,
    allowed_runners: Vec<String>,
    definitions: Vec<GeneratedDoorDefinitionConfig>,
}

#[derive(Serialize)]
struct GeneratedDoorDefinitionConfig {
    key: String,
    name: String,
    runner: String,
    working_dir: String,
    command: String,
    drop_file: String,
    exclusive: bool,
    time_limit_minutes: u32,
    enabled: bool,
}

#[derive(Serialize)]
struct GeneratedFtnConfig {
    enabled: bool,
    reserved_network_name: String,
}

pub fn build_setup_toml(answers: &SetupAnswers) -> io::Result<String> {
    let mut screens = BTreeMap::new();
    screens.insert(
        "login".to_string(),
        GeneratedScreenConfig {
            ansi: "login/login.ans".to_string(),
            ansi_40: Some("login/login-40.ans".to_string()),
            ascii: "login/login.asc".to_string(),
            text: "login/login.txt".to_string(),
            pause: false,
        },
    );
    screens.insert(
        "screen1".to_string(),
        GeneratedScreenConfig {
            ansi: "info/screen1.ans".to_string(),
            ansi_40: None,
            ascii: "info/screen1.asc".to_string(),
            text: "info/screen1.txt".to_string(),
            pause: true,
        },
    );
    screens.insert(
        "screen2".to_string(),
        GeneratedScreenConfig {
            ansi: "info/screen2.ans".to_string(),
            ansi_40: None,
            ascii: "info/screen2.asc".to_string(),
            text: "info/screen2.txt".to_string(),
            pause: true,
        },
    );
    screens.insert(
        "main_menu".to_string(),
        GeneratedScreenConfig {
            ansi: "menus/main/main.ans".to_string(),
            ansi_40: Some("menus/main/main-40.ans".to_string()),
            ascii: "menus/main/main.asc".to_string(),
            text: "menus/main/main.txt".to_string(),
            pause: false,
        },
    );

    let mut menus = BTreeMap::new();
    menus.insert(
        "login".to_string(),
        GeneratedMenuConfig {
            screen: "login".to_string(),
            prompt: "Login? ".to_string(),
            items: vec![
                GeneratedMenuItemConfig {
                    key: "L".to_string(),
                    label: "Logon".to_string(),
                    action: "login".to_string(),
                },
                GeneratedMenuItemConfig {
                    key: "N".to_string(),
                    label: "New User".to_string(),
                    action: "new_user".to_string(),
                },
                GeneratedMenuItemConfig {
                    key: "G".to_string(),
                    label: "Goodbye".to_string(),
                    action: "logoff".to_string(),
                },
            ],
        },
    );
    menus.insert(
        "main".to_string(),
        GeneratedMenuConfig {
            screen: "main_menu".to_string(),
            prompt: "Command? ".to_string(),
            items: vec![
                GeneratedMenuItemConfig {
                    key: "D".to_string(),
                    label: "Doors".to_string(),
                    action: "doors".to_string(),
                },
                GeneratedMenuItemConfig {
                    key: "M".to_string(),
                    label: "Messages".to_string(),
                    action: "messages".to_string(),
                },
                GeneratedMenuItemConfig {
                    key: "N".to_string(),
                    label: "New User".to_string(),
                    action: "new_user".to_string(),
                },
                GeneratedMenuItemConfig {
                    key: "G".to_string(),
                    label: "Goodbye".to_string(),
                    action: "logoff".to_string(),
                },
            ],
        },
    );

    let doors = if answers.include_example_door {
        GeneratedDoorsConfig {
            enabled: true,
            default_runner: DEFAULT_DOSEMU.to_string(),
            allowed_runners: DEFAULT_ALLOWED_RUNNERS
                .iter()
                .map(std::string::ToString::to_string)
                .collect(),
            definitions: vec![GeneratedDoorDefinitionConfig {
                key: "oxide-check".to_string(),
                name: "Oxide Door Check".to_string(),
                runner: DEFAULT_DOSEMU.to_string(),
                working_dir: "./tools/doors/oxide-door-check/dist".to_string(),
                command: "OXIDECHK.EXE".to_string(),
                drop_file: "DORINFO1.DEF".to_string(),
                exclusive: false,
                time_limit_minutes: 5,
                enabled: answers.example_door_enabled,
            }],
        }
    } else {
        GeneratedDoorsConfig {
            enabled: false,
            default_runner: DEFAULT_DOSEMU.to_string(),
            allowed_runners: DEFAULT_ALLOWED_RUNNERS
                .iter()
                .map(std::string::ToString::to_string)
                .collect(),
            definitions: Vec::new(),
        }
    };

    let config = GeneratedConfig {
        board: GeneratedBoardConfig {
            name: answers.board_name.clone(),
            tagline: answers.tagline.clone(),
            sysop_name: answers.sysop_name.clone(),
            timezone: answers.timezone.clone(),
        },
        telnet: GeneratedTelnetConfig {
            bind: answers.telnet_bind.clone(),
        },
        auth: GeneratedAuthConfig {
            failed_login_threshold: DEFAULT_FAILED_LOGIN_THRESHOLD,
            failed_login_window_minutes: DEFAULT_FAILED_LOGIN_WINDOW_MINUTES,
            failed_login_lockout_minutes: DEFAULT_FAILED_LOGIN_LOCKOUT_MINUTES,
            new_user_security_level: DEFAULT_NEW_USER_SECURITY_LEVEL,
            argon2: GeneratedArgon2Config {
                memory_cost_kib: DEFAULT_ARGON2_MEMORY_COST_KIB,
                iterations: DEFAULT_ARGON2_ITERATIONS,
                parallelism: DEFAULT_ARGON2_PARALLELISM,
            },
        },
        audit: GeneratedAuditConfig {
            retention_days: DEFAULT_AUDIT_RETENTION_DAYS,
        },
        database: GeneratedDatabaseConfig {
            path: answers.database_path.to_string_lossy().into_owned(),
        },
        paths: GeneratedPathsConfig {
            ansi: DEFAULT_ANSI_PATH.to_string(),
            screens: DEFAULT_SCREENS_PATH.to_string(),
            doors: DEFAULT_DOORS_PATH.to_string(),
            runtime: DEFAULT_RUNTIME_PATH.to_string(),
            logs: DEFAULT_LOGS_PATH.to_string(),
        },
        nodes: GeneratedNodesConfig {
            count: answers.node_count,
        },
        terminal: GeneratedTerminalConfig {
            default_encoding: "cp437".to_string(),
            clear_screen_on_connect: true,
            welcome_screen: "welcome.ans".to_string(),
            logoff_screen: "logoff.ans".to_string(),
        },
        flow: GeneratedFlowConfig {
            login_screen: "login".to_string(),
            login_menu: "login".to_string(),
            post_login_screens: vec!["screen1".to_string(), "screen2".to_string()],
            main_menu: "main".to_string(),
        },
        screens,
        menus,
        doors,
        ftn: GeneratedFtnConfig {
            enabled: false,
            reserved_network_name: "OxideNet".to_string(),
        },
    };

    toml::to_string_pretty(&config)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

pub fn setup_required_directories(output_path: &Path, answers: &SetupAnswers) -> Vec<PathBuf> {
    let mut dirs = BTreeSet::new();
    if let Some(parent) = output_path.parent() {
        dirs.insert(parent.to_path_buf());
    }
    dirs.insert(PathBuf::from(DEFAULT_ANSI_PATH));
    dirs.insert(PathBuf::from(DEFAULT_SCREENS_PATH));
    dirs.insert(PathBuf::from(DEFAULT_DOORS_PATH));
    dirs.insert(PathBuf::from(DEFAULT_RUNTIME_PATH));
    dirs.insert(PathBuf::from(DEFAULT_LOGS_PATH));
    dirs.insert(
        answers
            .database_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf(),
    );
    dirs.into_iter().collect()
}

pub fn interactive_setup_answers() -> io::Result<SetupAnswers> {
    let stdin = io::stdin();
    let mut stdin = stdin.lock();
    let stdout = io::stdout();
    let mut stdout = stdout.lock();

    interactive_answers(&mut stdin, &mut stdout)
}

pub fn run_setup_with_answers(
    output_path: &Path,
    force: bool,
    answers: &SetupAnswers,
) -> io::Result<()> {
    let output = build_setup_toml(answers)?;
    if output_path.exists() && !force {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "output config already exists; use --force to overwrite",
        ));
    }

    for dir in setup_required_directories(output_path, answers) {
        std::fs::create_dir_all(dir)?;
    }

    if answers.include_sample_ansi {
        install_default_assets(
            Path::new(DEFAULT_ANSI_PATH),
            Path::new(DEFAULT_SCREENS_PATH),
        )?;
    }

    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    std::fs::write(output_path, output)?;
    Ok(())
}

pub fn install_default_assets(
    ansi_root: &Path,
    screens_root: &Path,
) -> io::Result<DefaultAssetInstallSummary> {
    let mut installed = 0;
    let mut skipped = 0;

    for asset in DEFAULT_ASSETS {
        let root = match asset.root {
            DefaultAssetRoot::Ansi => ansi_root,
            DefaultAssetRoot::Screens => screens_root,
        };
        let destination = root.join(asset.path);
        if destination.exists() {
            skipped += 1;
            continue;
        }
        if let Some(parent) = destination.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(destination, asset.bytes)?;
        installed += 1;
    }

    Ok(DefaultAssetInstallSummary { installed, skipped })
}

fn interactive_answers<R: BufRead, W: Write>(
    reader: &mut R,
    writer: &mut W,
) -> io::Result<SetupAnswers> {
    let board_name = prompt_line(reader, writer, "Board name", DEFAULT_BOARD_NAME)?;
    let tagline = prompt_line(reader, writer, "Board tagline", DEFAULT_TAGLINE)?;
    let sysop_name = prompt_line(reader, writer, "Sysop name", DEFAULT_SYSOP_NAME)?;
    let sysop_alias = prompt_line(reader, writer, "Sysop alias", DEFAULT_SYSOP_ALIAS)?;
    let sysop_password = loop {
        let password = prompt_line(reader, writer, "Sysop password", "")?;
        if !password.trim().is_empty() {
            break password;
        }
        writeln!(writer, "Sysop password is required")?;
    };
    let timezone = prompt_line(reader, writer, "Timezone", DEFAULT_TIMEZONE)?;
    let telnet_bind = prompt_line(reader, writer, "Telnet bind", DEFAULT_TELNET_BIND)?;
    let node_count = prompt_u16(reader, writer, "Node count", DEFAULT_NODE_COUNT)?;
    let database_path = PathBuf::from(prompt_line(
        reader,
        writer,
        "Database path",
        DEFAULT_DATABASE_PATH,
    )?);
    let include_example_door =
        prompt_yes_no(reader, writer, "Include example door definition", true)?;
    let include_sample_ansi = prompt_yes_no(reader, writer, "Create sample ANSI screens", true)?;

    Ok(SetupAnswers {
        board_name,
        tagline,
        sysop_name,
        sysop_alias,
        sysop_password,
        timezone,
        telnet_bind,
        node_count,
        database_path,
        include_example_door,
        example_door_enabled: false,
        include_sample_ansi,
    })
}

fn prompt_line<R: BufRead, W: Write>(
    reader: &mut R,
    writer: &mut W,
    prompt: &str,
    default: &str,
) -> io::Result<String> {
    write!(writer, "{prompt} [{default}]: ")?;
    writer.flush()?;
    let mut input = String::new();
    reader.read_line(&mut input)?;
    let trimmed = input.trim();
    if !trimmed.is_empty() {
        Ok(trimmed.to_string())
    } else {
        Ok(default.to_string())
    }
}

fn prompt_u16<R: BufRead, W: Write>(
    reader: &mut R,
    writer: &mut W,
    prompt: &str,
    default: u16,
) -> io::Result<u16> {
    loop {
        let line = prompt_line(reader, writer, prompt, &default.to_string())?;
        let parsed = line.parse::<u16>();
        match parsed {
            Ok(0) | Err(_) => {
                writeln!(writer, "Node count must be greater than 0")?;
            }
            Ok(value) => return Ok(value),
        }
    }
}

fn prompt_yes_no<R: BufRead, W: Write>(
    reader: &mut R,
    writer: &mut W,
    prompt: &str,
    default: bool,
) -> io::Result<bool> {
    let default_prompt = if default { "Y/n" } else { "y/N" };
    loop {
        write!(writer, "{prompt} [{default_prompt}]: ")?;
        writer.flush()?;
        let mut response = String::new();
        reader.read_line(&mut response)?;
        let response = if response.trim().is_empty() {
            if default { "y" } else { "n" }
        } else {
            response.trim()
        };
        let normalized = response.trim().to_ascii_lowercase();
        match normalized.as_str() {
            "y" | "yes" => return Ok(true),
            "n" | "no" => return Ok(false),
            _ => {
                writeln!(writer, "Please enter y, yes, n, or no")?;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{MenuItemConfig, OxideConfig};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_path(tag: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!(
            "oxidebbs-setup-{tag}-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system time should be valid")
                .as_nanos()
        ));
        path
    }

    #[test]
    fn generated_setup_config_with_defaults_is_valid() {
        let answers = SetupAnswers::default();
        let generated = build_setup_toml(&answers).expect("generate setup config");
        let parsed: OxideConfig = toml::from_str(&generated).expect("parse generated config");
        parsed.validate().expect("validate generated config");
        assert_eq!(parsed.board.name, "OxideBBS");
        assert_eq!(parsed.telnet.bind, "127.0.0.1:2323");
        assert_eq!(parsed.auth.failed_login_threshold, 5);
        assert_eq!(parsed.auth.failed_login_window_minutes, 10);
        assert_eq!(parsed.auth.failed_login_lockout_minutes, 15);
        assert_eq!(parsed.auth.new_user_security_level, 10);
        assert_eq!(parsed.auth.argon2.memory_cost_kib, 19_456);
        assert_eq!(parsed.auth.argon2.iterations, 2);
        assert_eq!(parsed.auth.argon2.parallelism, 1);
        assert_eq!(parsed.audit.retention_days, 365);
        assert!(parsed.doors.enabled);
        assert_eq!(parsed.doors.definitions.len(), 1);
        assert_eq!(parsed.doors.definitions[0].key, "oxide-check");
        assert_eq!(parsed.doors.definitions[0].command, "OXIDECHK.EXE");
        assert!(!parsed.doors.definitions[0].enabled);
        assert_eq!(
            parsed.doors.allowed_runners,
            vec!["dosemu".to_string(), "dosemu2".to_string()]
        );
    }

    #[test]
    fn generated_setup_config_without_example_door_has_empty_door_list() {
        let answers = SetupAnswers {
            include_example_door: false,
            ..SetupAnswers::default()
        };
        let generated = build_setup_toml(&answers).expect("generate setup config");
        let parsed: OxideConfig = toml::from_str(&generated).expect("parse generated config");
        parsed.validate().expect("validate generated config");
        assert!(!parsed.doors.enabled);
        assert!(parsed.doors.definitions.is_empty());
    }

    #[test]
    fn generated_setup_config_can_enable_example_door() {
        let answers = SetupAnswers {
            example_door_enabled: true,
            ..SetupAnswers::default()
        };
        let generated = build_setup_toml(&answers).expect("generate setup config");
        let parsed: OxideConfig = toml::from_str(&generated).expect("parse generated config");
        parsed.validate().expect("validate generated config");
        assert!(parsed.doors.enabled);
        assert_eq!(parsed.doors.definitions.len(), 1);
        assert!(parsed.doors.definitions[0].enabled);
    }

    #[test]
    fn generated_default_menu_assets_show_all_default_menu_keys() {
        let generated = build_setup_toml(&SetupAnswers::default()).expect("generate setup config");
        let parsed: OxideConfig = toml::from_str(&generated).expect("parse generated config");
        parsed.validate().expect("validate generated config");

        assert_default_menu_asset_contains_items(
            include_str!("../../../assets/screens/login/login.asc"),
            &parsed.menus["login"].items,
        );
        assert_default_menu_asset_contains_items(
            include_str!("../../../assets/screens/menus/main/main.asc"),
            &parsed.menus["main"].items,
        );
        assert_default_menu_asset_contains_items(
            include_str!("../../../assets/screens/menus/main/main.txt"),
            &parsed.menus["main"].items,
        );
    }

    fn assert_default_menu_asset_contains_items(asset: &str, items: &[MenuItemConfig]) {
        for item in items {
            assert!(
                asset.contains(&format!("[{}]", item.key)) || asset.contains(&item.key),
                "default menu asset should show key {}",
                item.key
            );
            assert!(
                asset.contains(&item.label),
                "default menu asset should show label {}",
                item.label
            );
        }
    }

    #[test]
    fn setup_required_directory_list_is_stable() {
        let answers = SetupAnswers {
            database_path: PathBuf::from("/tmp/oxidebbs/custom.ddb"),
            ..SetupAnswers::default()
        };
        let dirs = setup_required_directories(Path::new("config/oxidebbs.toml"), &answers);
        assert!(dirs.contains(&PathBuf::from("/tmp/oxidebbs")));
        assert!(dirs.contains(&PathBuf::from("./assets/ansi")));
    }

    #[test]
    fn default_asset_installer_writes_bundled_assets_without_overwriting_custom_files() {
        let base_dir = temp_path("default-assets");
        let ansi_root = base_dir.join("ansi");
        let screens_root = base_dir.join("screens");
        std::fs::create_dir_all(&ansi_root).expect("ansi root");
        std::fs::write(ansi_root.join("welcome.asc"), b"custom welcome").expect("custom welcome");

        let summary = install_default_assets(&ansi_root, &screens_root).expect("install assets");

        assert_eq!(summary.installed, DEFAULT_ASSETS.len() - 1);
        assert_eq!(summary.skipped, 1);
        assert_eq!(
            std::fs::read(ansi_root.join("welcome.asc")).expect("read custom"),
            b"custom welcome"
        );
        assert!(ansi_root.join("welcome.ans").is_file());
        assert!(ansi_root.join("logoff.asc").is_file());
        assert!(screens_root.join("login/login.ans").is_file());
        assert!(screens_root.join("menus/main/main.asc").is_file());

        let summary = install_default_assets(&ansi_root, &screens_root).expect("reinstall assets");
        assert_eq!(summary.installed, 0);
        assert_eq!(summary.skipped, DEFAULT_ASSETS.len());

        let _ = std::fs::remove_dir_all(base_dir);
    }
}
