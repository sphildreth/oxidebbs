use std::path::Path;

use serde_json::Value as JsonValue;

use clap::Subcommand;

use crate::config::DoorDefConfig;
use crate::sysop_cli::{AppContext, CliError, CliResult, emit_ok, print_json};

#[derive(Subcommand)]
pub enum ConfigCommand {
    Show,
    Check,
    Paths,
    Get { key: String },
    Set { key: String, value: String },
}

pub fn run_config(command: ConfigCommand, ctx: &AppContext) -> CliResult<()> {
    match command {
        ConfigCommand::Show => {
            let raw = std::fs::read_to_string(&ctx.config_path)?;
            if ctx.json {
                let parsed: toml::Value = toml::from_str(&raw)?;
                print_json(&serde_json::to_value(parsed)?)?;
            } else {
                print!("{raw}");
            }
            Ok(())
        }
        ConfigCommand::Check => run_check(ctx),
        ConfigCommand::Paths => {
            let paths = serde_json::json!({
                "config": ctx.config_path,
                "database": ctx.config.database.path,
                "ansi": ctx.config.paths.ansi,
                "screens": ctx.config.paths.screens,
                "doors": ctx.config.paths.doors,
                "runtime": ctx.config.paths.runtime,
                "logs": ctx.config.paths.logs
            });
            if ctx.json {
                print_json(&paths)?;
            } else {
                println!("config: {}", ctx.config_path.display());
                println!("database: {}", ctx.config.database.path.display());
                println!("ansi: {}", ctx.config.paths.ansi.display());
                println!("screens: {}", ctx.config.paths.screens.display());
                println!("doors: {}", ctx.config.paths.doors.display());
                println!("runtime: {}", ctx.config.paths.runtime.display());
                println!("logs: {}", ctx.config.paths.logs.display());
            }
            Ok(())
        }
        ConfigCommand::Get { key } => {
            let raw = std::fs::read_to_string(&ctx.config_path)?;
            let parsed: toml::Value = toml::from_str(&raw)?;
            let value = get_toml_path(&parsed, &key)
                .ok_or_else(|| CliError::Message(format!("config key {key:?} not found")))?;
            if ctx.json {
                print_json(&serde_json::to_value(value)?)?;
            } else {
                println!("{value}");
            }
            Ok(())
        }
        ConfigCommand::Set { .. } => unreachable!("config set is handled before config load"),
    }
}

#[derive(Debug, Clone)]
pub(crate) struct CheckIssue {
    pub(crate) level: &'static str,
    message: String,
}

impl CheckIssue {
    fn error(message: impl Into<String>) -> Self {
        Self {
            level: "error",
            message: message.into(),
        }
    }

    fn warning(message: impl Into<String>) -> Self {
        Self {
            level: "warning",
            message: message.into(),
        }
    }

    pub(crate) fn to_json(&self) -> JsonValue {
        serde_json::json!({"level": self.level, "message": self.message})
    }
}

pub(crate) fn run_check(ctx: &AppContext) -> CliResult<()> {
    let issues = validate_runtime(&ctx.config, &ctx.config_path);
    let errors = issues.iter().filter(|issue| issue.level == "error").count();
    if ctx.json {
        print_json(&serde_json::json!({
            "ok": errors == 0,
            "issues": issues.iter().map(CheckIssue::to_json).collect::<Vec<_>>()
        }))?;
    } else {
        if errors == 0 {
            println!("configuration OK: {}", ctx.config_path.display());
            println!("  board:          {}", ctx.config.board.name);
            println!("  telnet bind:    {}", ctx.config.telnet.bind);
            println!("  database path:  {}", ctx.config.database.path.display());
            println!("  nodes:          {}", ctx.config.nodes.count);
            println!("  doors defined:  {}", ctx.config.doors.definitions.len());
            print_check_issues(&issues);
        } else {
            print_check_issues(&issues);
        }
    }

    if errors > 0 {
        return Err(CliError::Message("configuration check failed".to_string()));
    }
    Ok(())
}

pub(crate) fn validate_runtime(
    config: &crate::config::OxideConfig,
    config_path: &std::path::Path,
) -> Vec<CheckIssue> {
    let mut issues = Vec::new();
    use std::net::SocketAddr;
    if !config_path.is_file() {
        issues.push(CheckIssue::error(format!(
            "config file {} does not exist",
            config_path.display()
        )));
    }
    if config.telnet.bind.parse::<SocketAddr>().is_err() {
        issues.push(CheckIssue::error(format!(
            "telnet.bind {:?} is not a valid socket address",
            config.telnet.bind
        )));
    }
    if config.nodes.count == 0 {
        issues.push(CheckIssue::error("nodes.count must be greater than 0"));
    }
    if let Some(parent) = config.database.path.parent()
        && !parent.exists()
    {
        issues.push(CheckIssue::warning(format!(
            "database parent directory {} does not exist yet",
            parent.display()
        )));
    }
    for (label, path) in [
        ("ansi", &config.paths.ansi),
        ("screens", &config.paths.screens),
        ("doors", &config.paths.doors),
        ("runtime", &config.paths.runtime),
        ("logs", &config.paths.logs),
    ] {
        if !path.exists() {
            issues.push(CheckIssue::warning(format!(
                "{label} path {} does not exist yet",
                path.display()
            )));
        }
    }
    for screen_name in config.screens.keys() {
        issues.extend(validate_screen_assets(config, screen_name));
    }
    for door in config.doors.definitions.iter().filter(|door| door.enabled) {
        issues.extend(check_configured_door(door, config));
    }
    issues
}

fn validate_screen_assets(
    config: &crate::config::OxideConfig,
    screen_name: &str,
) -> Vec<CheckIssue> {
    let mut issues = Vec::new();
    let Some(screen) = config.screens.get(screen_name) else {
        issues.push(CheckIssue::error(format!(
            "screen {screen_name:?} is not configured"
        )));
        return issues;
    };

    for asset in screen_assets(screen) {
        let path = config.paths.screens.join(asset);
        if !path.is_file() {
            issues.push(CheckIssue::error(format!(
                "screen {screen_name:?} asset {} is missing",
                path.display()
            )));
        }
    }
    issues
}

pub(crate) fn print_check_issues(issues: &[CheckIssue]) {
    for issue in issues {
        println!("{}: {}", issue.level, issue.message);
    }
}

fn check_configured_door(
    door: &DoorDefConfig,
    config: &crate::config::OxideConfig,
) -> Vec<CheckIssue> {
    let mut issues = Vec::new();
    let working_dir = std::path::PathBuf::from(&door.working_dir);
    if !working_dir.is_dir() {
        issues.push(CheckIssue::warning(format!(
            "door working directory {} does not exist",
            working_dir.display()
        )));
    }
    match first_command_token(&door.command) {
        Some(command_name) if is_quoted_dos_command(command_name) => {
            issues.push(CheckIssue::error(
                "quoted DOS commands are not supported yet; use DOS 8.3 paths",
            ));
        }
        Some(command_name) => {
            let command_path = if command_name.contains(':')
                || command_name.contains('\\')
                || command_name.contains('/')
            {
                std::path::PathBuf::from(command_name)
            } else {
                working_dir.join(command_name)
            };
            if !command_path.exists() {
                issues.push(CheckIssue::warning(format!(
                    "door command {} was not found under {}",
                    command_name,
                    working_dir.display()
                )));
            }
        }
        None => issues.push(CheckIssue::error("door command is empty")),
    }
    if !command_exists(&door.runner) {
        issues.push(CheckIssue::warning(format!(
            "door runner {:?} was not found on PATH",
            door.runner
        )));
    }
    if !matches!(
        door.drop_file.to_ascii_uppercase().as_str(),
        "DOOR.SYS" | "DORINFO1.DEF"
    ) {
        issues.push(CheckIssue::error(format!(
            "drop-file format {:?} is not supported",
            door.drop_file
        )));
    }
    if door.time_limit_minutes == 0 {
        issues.push(CheckIssue::error("time limit must be greater than 0"));
    }
    if let Err(error) = std::fs::create_dir_all(&config.paths.runtime) {
        issues.push(CheckIssue::error(format!(
            "runtime directory {} is not writable: {error}",
            config.paths.runtime.display()
        )));
    }
    issues
}

fn command_exists(command: &str) -> bool {
    let path = std::path::Path::new(command);
    if path.components().count() > 1 {
        return path.exists();
    }
    let Some(paths) = std::env::var_os("PATH") else {
        return false;
    };
    std::env::split_paths(&paths).any(|dir| dir.join(command).is_file())
}

fn first_command_token(command: &str) -> Option<&str> {
    command.trim().split_ascii_whitespace().next()
}

fn is_quoted_dos_command(command: &str) -> bool {
    command.starts_with('"') || command.starts_with('\'')
}

fn screen_assets(screen: &crate::config::ScreenConfig) -> Vec<&str> {
    let mut assets = Vec::new();
    if let Some(asset) = screen.ansi.as_deref() {
        assets.push(asset);
    }
    if let Some(asset) = screen.ansi_40.as_deref() {
        assets.push(asset);
    }
    if let Some(asset) = screen.ascii.as_deref() {
        assets.push(asset);
    }
    if let Some(asset) = screen.text.as_deref() {
        assets.push(asset);
    }
    assets
}

pub fn run_config_set(
    config_path: &Path,
    key: &str,
    raw_value: &str,
    json_output: bool,
) -> CliResult<()> {
    let raw = std::fs::read_to_string(config_path)?;
    let mut parsed: toml::Value = toml::from_str(&raw)?;
    set_toml_path(&mut parsed, key, infer_toml_value(raw_value))?;
    let updated = toml::to_string_pretty(&parsed)?;
    std::fs::write(config_path, updated)?;
    emit_ok(
        json_output,
        "configuration updated",
        serde_json::json!({"key": key}),
    )
}

fn get_toml_path<'a>(value: &'a toml::Value, key: &str) -> Option<&'a toml::Value> {
    let mut current = value;
    for segment in key.split('.') {
        current = current.get(segment)?;
    }
    Some(current)
}

fn set_toml_path(value: &mut toml::Value, key: &str, new_value: toml::Value) -> CliResult<()> {
    let mut segments = key.split('.').peekable();
    let mut current = value;
    while let Some(segment) = segments.next() {
        if segments.peek().is_none() {
            let table = current
                .as_table_mut()
                .ok_or_else(|| CliError::Message(format!("config path {key:?} is not a table")))?;
            table.insert(segment.to_string(), new_value);
            return Ok(());
        }
        current = current.get_mut(segment).ok_or_else(|| {
            CliError::Message(format!("config path segment {segment:?} not found"))
        })?;
    }
    Err(CliError::Message("config key cannot be empty".to_string()))
}

fn infer_toml_value(raw: &str) -> toml::Value {
    if raw.eq_ignore_ascii_case("true") {
        return toml::Value::Boolean(true);
    }
    if raw.eq_ignore_ascii_case("false") {
        return toml::Value::Boolean(false);
    }
    if let Ok(value) = raw.parse::<i64>() {
        return toml::Value::Integer(value);
    }
    toml::Value::String(raw.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{config::OxideConfig, sysop_cli::AppContext};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_path(tag: &str) -> std::path::PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!(
            "oxidebbs-phase6-{tag}-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system time should be valid")
                .as_nanos()
        ));
        path
    }

    fn load_example_config_for_repo() -> (OxideConfig, std::path::PathBuf) {
        let mut config_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        config_path.push("../../config/oxidebbs.example.toml");
        let workspace_root = config_path
            .parent()
            .and_then(|path| path.parent())
            .expect("workspace root");
        let mut config = OxideConfig::load(&config_path).expect("load example config");

        config.paths.ansi = workspace_root.join(config.paths.ansi);
        config.paths.screens = workspace_root.join(config.paths.screens);
        config.paths.doors = workspace_root.join(config.paths.doors);
        config.paths.runtime = workspace_root.join(config.paths.runtime);
        config.paths.logs = workspace_root.join(config.paths.logs);
        config.database.path = workspace_root.join(&config.database.path);

        for door in &mut config.doors.definitions {
            door.working_dir = workspace_root
                .join(&door.working_dir)
                .to_string_lossy()
                .to_string();
        }

        (config, config_path)
    }

    #[test]
    fn run_check_on_example_config_has_no_errors() {
        let (mut config, config_path) = load_example_config_for_repo();
        let runtime = temp_path("runtime");
        config.paths.runtime = runtime.clone();
        let ctx = AppContext {
            config_path,
            config,
            json: false,
        };
        run_check(&ctx).expect("example config check");
        let _ = std::fs::remove_dir_all(runtime);
    }

    #[test]
    fn check_issues_require_valid_telnet_bind() {
        let (mut config, config_path) = load_example_config_for_repo();
        let runtime = temp_path("runtime");
        config.paths.runtime = runtime.clone();
        config.telnet.bind = "bad bind".to_string();
        let issues = validate_runtime(&config, &config_path);
        assert_eq!(
            issues.iter().filter(|issue| issue.level == "error").count(),
            1
        );
        let first = issues.first().expect("issue exists");
        assert!(first.message.contains("telnet.bind"));
        let _ = std::fs::remove_dir_all(runtime);
    }

    #[test]
    fn door_check_uses_first_command_token_and_rejects_quoted_commands() {
        let (mut config, _config_path) = load_example_config_for_repo();
        let runtime = temp_path("door-command-runtime");
        let working_dir = temp_path("door-command-working");
        std::fs::create_dir_all(&working_dir).expect("working dir");
        std::fs::write(working_dir.join("LORD.EXE"), b"").expect("door exe");
        config.paths.runtime = runtime.clone();
        config.doors.definitions[0].working_dir = working_dir.to_string_lossy().to_string();
        config.doors.definitions[0].runner = std::env::current_exe()
            .expect("current exe")
            .to_string_lossy()
            .to_string();
        config.doors.definitions[0].command = "LORD.EXE /N1".to_string();

        let door = config.doors.definitions[0].clone();
        let issues = check_configured_door(&door, &config);
        assert!(
            !issues
                .iter()
                .any(|issue| issue.message.contains("door command"))
        );

        let mut door = config.doors.definitions[0].clone();
        door.command = "\"LORD.EXE\"".to_string();
        let issues = check_configured_door(&door, &config);
        assert!(issues.iter().any(|issue| {
            issue.level == "error"
                && issue.message.contains("quoted DOS commands")
                && issue.message.contains("DOS 8.3 paths")
        }));

        let mut door = config.doors.definitions[0].clone();
        door.command = "   ".to_string();
        let issues = check_configured_door(&door, &config);
        assert!(
            issues
                .iter()
                .any(|issue| issue.level == "error" && issue.message.contains("command is empty"))
        );

        let _ = std::fs::remove_dir_all(runtime);
        let _ = std::fs::remove_dir_all(working_dir);
    }
}
