use std::path::Path;

use serde_json::Value as JsonValue;

use clap::Subcommand;
#[cfg(unix)]
use nix::unistd::geteuid;
use oxidebbs_door::runner_supports_dosemu2_cli;
#[cfg(unix)]
use std::os::unix::fs::MetadataExt;

use crate::config::DoorDefConfig;
use crate::sysop_cli::{AppContext, CliError, CliResult, emit_ok, print_json};

pub(crate) const TELNET_PLAINTEXT_EXPOSURE_WARNING: &str = "telnet bind address is reachable beyond loopback; telnet is plaintext and sends credentials and caller traffic without encryption";

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
    match config.telnet.bind.parse::<SocketAddr>() {
        Ok(bind) => {
            if telnet_bind_exposes_plaintext(bind) {
                issues.push(CheckIssue::warning(TELNET_PLAINTEXT_EXPOSURE_WARNING));
            }
        }
        Err(_) => {
            issues.push(CheckIssue::error(format!(
                "telnet.bind {:?} is not a valid socket address",
                config.telnet.bind
            )));
        }
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
        ("logs", &config.paths.logs),
    ] {
        if !path.exists() {
            issues.push(CheckIssue::warning(format!(
                "{label} path {} does not exist yet",
                path.display()
            )));
        }
    }
    validate_runtime_directory(&config.paths.runtime, &mut issues);
    for screen_name in config.screens.keys() {
        issues.extend(validate_screen_assets(config, screen_name));
    }
    for door in config.doors.definitions.iter().filter(|door| door.enabled) {
        issues.extend(check_configured_door(door, config));
    }
    issues
}

fn telnet_bind_exposes_plaintext(bind: std::net::SocketAddr) -> bool {
    let ip = bind.ip();
    ip.is_unspecified() || !ip.is_loopback()
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
    let doors_root = match std::path::Path::new(&config.paths.doors).canonicalize() {
        Ok(root) => root,
        Err(error) => {
            issues.push(CheckIssue::warning(format!(
                "doors path {} is not accessible: {error}",
                config.paths.doors.display()
            )));
            Path::new("/").to_path_buf()
        }
    };
    let working_dir = Path::new(&door.working_dir).canonicalize();
    let working_dir = match working_dir {
        Ok(path) => {
            if !path.starts_with(&doors_root) {
                issues.push(CheckIssue::error(format!(
                    "door working directory {} escapes doors path {}",
                    door.working_dir,
                    config.paths.doors.display()
                )));
            }
            path
        }
        Err(error) => {
            issues.push(CheckIssue::warning(format!(
                "door working directory {} does not exist: {error}",
                door.working_dir
            )));
            Path::new(&door.working_dir).to_path_buf()
        }
    };
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
    if !runner_supports_dosemu2_cli(&door.runner) {
        issues.push(CheckIssue::error(format!(
            "door runner {:?} is not supported for live caller doors; use DOSEMU2 runner \"dosemu\"",
            door.runner
        )));
    }
    if let Err(error) = validate_door_runner(&door.runner, &config.doors.allowed_runners) {
        issues.push(error);
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
    if !(1..=240).contains(&door.time_limit_minutes) {
        issues.push(CheckIssue::error("time limit must be in 1..=240 minutes"));
    }
    issues
}

fn validate_runtime_directory(path: &Path, issues: &mut Vec<CheckIssue>) {
    if let Err(error) = std::fs::create_dir_all(path) {
        issues.push(CheckIssue::error(format!(
            "runtime directory {} is not writable: {error}",
            path.display()
        )));
        return;
    }

    let metadata = match path.metadata() {
        Ok(metadata) => metadata,
        Err(error) => {
            issues.push(CheckIssue::error(format!(
                "runtime directory {} metadata is not readable: {error}",
                path.display()
            )));
            return;
        }
    };
    if !metadata.is_dir() {
        issues.push(CheckIssue::error(format!(
            "runtime path {} is not a directory",
            path.display()
        )));
        return;
    }

    #[cfg(unix)]
    {
        let mode = metadata.mode() & 0o777;
        if mode != 0o700 {
            issues.push(CheckIssue::warning(format!(
                "runtime directory {} has mode {mode:o}; expected 700 for local control socket isolation",
                path.display()
            )));
        }
    }
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

fn resolve_runner_path(command: &str) -> Option<std::path::PathBuf> {
    let path = std::path::Path::new(command);
    if path.components().count() > 1 {
        Some(path.to_path_buf())
    } else {
        let paths = std::env::var_os("PATH")?;
        std::env::split_paths(&paths).find_map(|dir| {
            let candidate = dir.join(command);
            if candidate.is_file() {
                Some(candidate)
            } else {
                None
            }
        })
    }
}

fn validate_door_runner(runner: &str, allowed_runners: &[String]) -> Result<(), CheckIssue> {
    if !allowed_runners.iter().any(|allowed| allowed == runner) {
        return Err(CheckIssue::error(format!(
            "door runner {:?} is not allowed. expected one of {:?}",
            runner, allowed_runners
        )));
    }
    let runner_path = resolve_runner_path(runner).ok_or_else(|| {
        CheckIssue::warning(format!("door runner {:?} was not found on PATH", runner))
    })?;
    let runner_path = runner_path.canonicalize().map_err(|error| {
        CheckIssue::warning(format!(
            "door runner {:?} is not accessible: {error}",
            runner
        ))
    })?;
    if !runner_path.is_file() {
        return Err(CheckIssue::error(format!(
            "door runner {:?} is not a regular file",
            runner_path.display()
        )));
    }
    validate_runner_file_permissions(&runner_path, runner).map_err(CheckIssue::error)?;
    Ok(())
}

#[cfg(unix)]
fn validate_runner_file_permissions(path: &Path, runner: &str) -> Result<(), String> {
    let metadata = path
        .metadata()
        .map_err(|error| format!("door runner {runner:?} metadata error: {error}"))?;
    let mode = metadata.mode();
    if mode & 0o002 != 0 {
        return Err(format!(
            "door runner {runner:?} is world-writable; refused for safety"
        ));
    }
    if mode & 0o020 != 0 {
        return Err(format!(
            "door runner {runner:?} is group-writable; refused for safety"
        ));
    }
    let owner = metadata.uid();
    let server_uid = geteuid().as_raw();
    if owner != 0 && owner != server_uid {
        return Err(format!(
            "door runner {runner:?} is owned by UID {owner}, not root or server UID {server_uid}"
        ));
    }
    Ok(())
}

#[cfg(not(unix))]
fn validate_runner_file_permissions(path: &Path, runner: &str) -> Result<(), String> {
    if path.is_file() {
        Ok(())
    } else {
        Err(format!("door runner {runner:?} is not a regular file"))
    }
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
    use std::fs;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;
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
    fn check_warns_for_public_telnet_bind() {
        let (mut config, config_path) = load_example_config_for_repo();
        let runtime = temp_path("public-bind-runtime");
        config.paths.runtime = runtime.clone();
        config.telnet.bind = "0.0.0.0:2323".to_string();

        let issues = validate_runtime(&config, &config_path);

        assert!(issues.iter().any(|issue| {
            issue.level == "warning" && issue.message == TELNET_PLAINTEXT_EXPOSURE_WARNING
        }));
        let _ = std::fs::remove_dir_all(runtime);
    }

    #[test]
    fn check_does_not_warn_for_example_loopback_bind() {
        let (mut config, config_path) = load_example_config_for_repo();
        let runtime = temp_path("loopback-bind-runtime");
        config.paths.runtime = runtime.clone();

        let issues = validate_runtime(&config, &config_path);

        assert!(
            !issues
                .iter()
                .any(|issue| issue.message == TELNET_PLAINTEXT_EXPOSURE_WARNING)
        );
        let _ = std::fs::remove_dir_all(runtime);
    }

    #[cfg(unix)]
    #[test]
    fn check_warns_for_runtime_directory_mode_that_weakens_control_socket_isolation() {
        let (mut config, config_path) = load_example_config_for_repo();
        let runtime = temp_path("runtime-permissions");
        fs::create_dir_all(&runtime).expect("runtime dir");
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&runtime, fs::Permissions::from_mode(0o755))
            .expect("runtime permissions");
        config.paths.runtime = runtime.clone();

        let issues = validate_runtime(&config, &config_path);

        assert!(issues.iter().any(|issue| {
            issue.level == "warning"
                && issue.message.contains("runtime directory")
                && issue.message.contains("expected 700")
        }));
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

        let mut door = config.doors.definitions[0].clone();
        door.runner = "dosbox".to_string();
        let issues = check_configured_door(&door, &config);
        assert!(issues.iter().any(|issue| {
            issue.level == "error"
                && issue.message.contains("not supported")
                && issue.message.contains("DOSEMU2")
        }));

        let _ = std::fs::remove_dir_all(runtime);
        let _ = std::fs::remove_dir_all(working_dir);
    }

    #[cfg(unix)]
    #[test]
    fn check_configured_door_validates_contained_working_dir_and_rejects_symlink_escape() {
        let (mut config, config_path) = load_example_config_for_repo();
        let _ = std::fs::remove_dir_all(config_path.join("runtime-check"));
        let runtime = temp_path("runtime");
        let doors_root = temp_path("door-config-root");
        let outside_root = temp_path("door-config-outside");
        let outside_target = outside_root.join("outside-target");
        let symlink_target = doors_root.join("outside-link");

        fs::create_dir_all(&doors_root).expect("doors root");
        fs::create_dir_all(&outside_target).expect("outside target");
        std::os::unix::fs::symlink(&outside_target, &symlink_target).expect("symlink escape");

        config.paths.runtime = runtime.clone();
        config.paths.doors = doors_root.clone();

        let mut door = config.doors.definitions[0].clone();
        door.working_dir = outside_target.to_string_lossy().to_string();
        door.runner = doors_root.join("dosemu").to_string_lossy().to_string();
        config.doors.allowed_runners = vec![door.runner.clone()];

        let runner_contents = "echo";
        std::fs::write(&door.runner, runner_contents).expect("runner fixture");
        let issues = check_configured_door(&door, &config);
        assert!(issues.iter().any(|issue| {
            issue.level == "error" && issue.message.contains("escapes doors path")
        }));

        let mut door = config.doors.definitions[0].clone();
        door.working_dir = symlink_target.to_string_lossy().to_string();
        let issues = check_configured_door(&door, &config);
        assert!(issues.iter().any(|issue| {
            issue.level == "error" && issue.message.contains("escapes doors path")
        }));

        let _ = std::fs::remove_dir_all(runtime);
        let _ = std::fs::remove_dir_all(doors_root);
        let _ = std::fs::remove_dir_all(outside_root);
    }

    #[test]
    fn check_configured_door_enforces_runner_allowlist_and_time_limit_cap() {
        let (mut config, _config_path) = load_example_config_for_repo();
        let runtime = temp_path("runner-allowlist-runtime");
        config.paths.runtime = runtime.clone();
        let runner = temp_path("runner");
        let runner_path = runner.join("dosemu");
        fs::create_dir_all(&runner).expect("runner dir");
        std::fs::write(&runner_path, b"echo").expect("runner");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&runner_path, fs::Permissions::from_mode(0o755))
                .expect("runner perms");
        }
        config.doors.allowed_runners = vec![runner_path.to_string_lossy().to_string()];
        config.doors.definitions[0].runner = "dosbox".to_string();
        let mut door = config.doors.definitions[0].clone();
        door.time_limit_minutes = 241;
        let issues = check_configured_door(&door, &config);
        assert!(
            issues
                .iter()
                .any(|issue| { issue.level == "error" && issue.message.contains("not allowed") })
        );
        assert!(issues.iter().any(|issue| {
            issue.level == "error" && issue.message.contains("time limit must be in 1..=240")
        }));
        let mut door = config.doors.definitions[0].clone();
        door.runner = runner_path.to_string_lossy().to_string();
        door.time_limit_minutes = 240;
        let issues = check_configured_door(&door, &config);
        assert!(!issues.iter().any(|issue| {
            issue.level == "error" && issue.message.contains("time limit must be in")
        }));
        door.time_limit_minutes = 0;
        let issues = check_configured_door(&door, &config);
        assert!(issues.iter().any(|issue| {
            issue.level == "error" && issue.message.contains("time limit must be in")
        }));

        #[cfg(unix)]
        {
            let mode = fs::metadata(&runner_path)
                .expect("runner stat")
                .permissions()
                .mode();
            let mut mode = mode;
            mode |= 0o020;
            fs::set_permissions(&runner_path, fs::Permissions::from_mode(mode))
                .expect("runner group writable");
            let door = door.clone();
            let issues = check_configured_door(&door, &config);
            assert!(issues.iter().any(|issue| {
                issue.level == "error" && issue.message.contains("group-writable")
            }));
            let mode = fs::metadata(&runner_path)
                .expect("runner stat")
                .permissions()
                .mode()
                & !0o020;
            fs::set_permissions(&runner_path, fs::Permissions::from_mode(mode))
                .expect("runner no group write");
            let mode = mode | 0o002;
            fs::set_permissions(&runner_path, fs::Permissions::from_mode(mode))
                .expect("runner world writable");
            let issues = check_configured_door(&door, &config);
            assert!(issues.iter().any(|issue| {
                issue.level == "error" && issue.message.contains("world-writable")
            }));
        }

        let _ = fs::remove_dir_all(runner);
        let _ = fs::remove_dir_all(runtime);
    }
}
