//! Door definitions, drop files, and runners.

use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use oxidebbs_core::door::DoorDefinition;
use serde::Deserialize;
use thiserror::Error;

pub const CRATE_NAME: &str = "oxidebbs-door";

#[derive(Debug, Error)]
pub enum DoorError {
    #[error("failed to read door config {path}: {source}")]
    ReadConfig {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("failed to parse door config {path}: {source}")]
    ParseConfig {
        path: PathBuf,
        source: toml::de::Error,
    },

    #[error("door config is invalid: {0}")]
    InvalidConfig(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("door process timed out after {timeout:?}")]
    Timeout { timeout: Duration },
}

#[derive(Debug, Clone, Deserialize)]
pub struct DoorConfigFile {
    #[serde(default, alias = "doors")]
    pub definitions: Vec<DoorConfigDefinition>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DoorConfigDefinition {
    pub key: String,
    pub name: String,
    #[serde(default = "default_runner")]
    pub runner: String,
    pub working_dir: String,
    pub command: String,
    #[serde(default = "default_drop_file")]
    pub drop_file: String,
    #[serde(default)]
    pub exclusive: bool,
    #[serde(default = "default_time_limit")]
    pub time_limit_minutes: u32,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DoorCaller {
    pub alias: String,
    pub real_name: String,
    pub location: String,
    pub security_level: i32,
    pub minutes_remaining: u32,
}

#[derive(Debug, Clone)]
pub struct DoorRunRequest {
    pub door: DoorDefinition,
    pub caller: DoorCaller,
    pub board_name: String,
    pub sysop_name: String,
    pub node_number: u16,
    pub runtime_dir: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DoorRunPlan {
    pub program: String,
    pub args: Vec<String>,
    pub working_dir: PathBuf,
    pub drop_file_path: PathBuf,
    pub timeout: Duration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DoorRunResult {
    pub exit_code: Option<i32>,
    pub timed_out: bool,
}

pub trait DoorRunner {
    fn run(&self, request: &DoorRunRequest) -> Result<DoorRunResult, DoorError>;
}

pub struct DryRunDoorRunner;

impl DoorRunner for DryRunDoorRunner {
    fn run(&self, request: &DoorRunRequest) -> Result<DoorRunResult, DoorError> {
        prepare_door_run(request)?;
        Ok(DoorRunResult {
            exit_code: Some(0),
            timed_out: false,
        })
    }
}

#[derive(Debug, Default, Clone)]
pub struct Dosemu2Runner;

impl DoorRunner for Dosemu2Runner {
    fn run(&self, request: &DoorRunRequest) -> Result<DoorRunResult, DoorError> {
        let plan = prepare_door_run(request)?;
        let mut command = Command::new(&plan.program);
        command
            .args(&plan.args)
            .current_dir(&plan.working_dir)
            .stdin(Stdio::piped());
        run_with_timeout(command, plan.timeout)
    }
}

pub fn parse_doors_toml(contents: &str) -> Result<Vec<DoorDefinition>, DoorError> {
    let config: DoorConfigFile =
        toml::from_str(contents).map_err(|source| DoorError::ParseConfig {
            path: PathBuf::from("<inline>"),
            source,
        })?;
    config.into_definitions()
}

pub fn load_doors_toml(path: impl AsRef<Path>) -> Result<Vec<DoorDefinition>, DoorError> {
    let path = path.as_ref();
    let contents = fs::read_to_string(path).map_err(|source| DoorError::ReadConfig {
        path: path.to_path_buf(),
        source,
    })?;
    let config: DoorConfigFile =
        toml::from_str(&contents).map_err(|source| DoorError::ParseConfig {
            path: path.to_path_buf(),
            source,
        })?;
    config.into_definitions()
}

pub fn node_runtime_dir(base: impl AsRef<Path>, node_number: u16) -> PathBuf {
    base.as_ref().join(format!("node-{node_number:03}"))
}

pub fn runner_supports_dosemu2_cli(runner: &str) -> bool {
    let runner_name = Path::new(runner)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(runner)
        .to_ascii_lowercase();
    matches!(runner_name.as_str(), "dosemu" | "dosemu.bin" | "dosemu2")
}

pub fn prepare_node_runtime_dir(
    base: impl AsRef<Path>,
    node_number: u16,
) -> Result<PathBuf, DoorError> {
    let dir = node_runtime_dir(base, node_number);
    fs::create_dir_all(&dir)?;
    #[cfg(unix)]
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o700))?;
    Ok(dir)
}

pub fn cleanup_node_runtime_dir(path: impl AsRef<Path>) -> Result<(), DoorError> {
    let path = path.as_ref();
    if path.exists() {
        fs::remove_dir_all(path)?;
    }
    Ok(())
}

pub fn render_door_sys(caller: &DoorCaller, node_number: u16, baud_rate: u32) -> String {
    [
        "COM1:".to_string(),
        baud_rate.to_string(),
        "8".to_string(),
        node_number.to_string(),
        caller.minutes_remaining.to_string(),
        caller.alias.clone(),
        caller.real_name.clone(),
        caller.location.clone(),
        caller.security_level.to_string(),
    ]
    .join("\r\n")
        + "\r\n"
}

pub fn render_dorinfo1_def(board_name: &str, sysop_name: &str, caller: &DoorCaller) -> String {
    let (first_name, last_name) = split_name(&caller.real_name);
    [
        board_name.to_string(),
        sysop_name.to_string(),
        "COM1".to_string(),
        "38400 BAUD,N,8,1".to_string(),
        "0".to_string(),
        first_name,
        last_name,
        caller.location.clone(),
        caller.security_level.to_string(),
        caller.minutes_remaining.to_string(),
    ]
    .join("\r\n")
        + "\r\n"
}

pub fn prepare_door_run(request: &DoorRunRequest) -> Result<DoorRunPlan, DoorError> {
    fs::create_dir_all(&request.runtime_dir)?;
    let drop_file_path = request.runtime_dir.join(&request.door.drop_file);
    let drop_contents = match request.door.drop_file.to_ascii_uppercase().as_str() {
        "DORINFO1.DEF" => {
            render_dorinfo1_def(&request.board_name, &request.sysop_name, &request.caller)
        }
        _ => render_door_sys(&request.caller, request.node_number, 38_400),
    };
    fs::write(&drop_file_path, drop_contents)?;
    fs::write(
        request.runtime_dir.join("OXNODE.TXT"),
        format!("node={}\r\n", request.node_number),
    )?;

    dosemu2_plan(request, drop_file_path)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dosemu2Command {
    pub executable: PathBuf,
    pub args: Vec<String>,
}

pub fn resolve_dosemu2_command(
    working_dir: &Path,
    command: &str,
) -> Result<Dosemu2Command, DoorError> {
    let trimmed = command.trim();
    if trimmed.is_empty() {
        return Err(DoorError::InvalidConfig(
            "door command is required".to_string(),
        ));
    }
    if trimmed.starts_with('"') {
        return Err(DoorError::InvalidConfig(
            "quoted DOS commands are not supported yet; use DOS 8.3 paths".to_string(),
        ));
    }

    let mut split = trimmed.split_ascii_whitespace();
    let command_token = split.next().unwrap_or_default();
    if command_token.contains(':') || command_token.contains('\\') || command_token.contains('/') {
        return Err(DoorError::InvalidConfig(
            "path-like DOS commands are not supported for DOSEMU2 runs yet; set working_dir and use a DOS 8.3 filename".to_string(),
        ));
    }

    Ok(Dosemu2Command {
        executable: normalize_path(working_dir.join(command_token)),
        args: split.map(ToString::to_string).collect(),
    })
}

pub fn dosemu2_plan(
    request: &DoorRunRequest,
    drop_file_path: PathBuf,
) -> Result<DoorRunPlan, DoorError> {
    let working_dir = absolute_host_path(&request.runtime_dir)?;
    let door_working_dir = absolute_host_path(Path::new(&request.door.working_dir))?;
    let runtime_dir = absolute_host_path(&request.runtime_dir)?;
    let command = resolve_dosemu2_command(&door_working_dir, &request.door.command)?;
    let runtime_command = stage_dosemu2_command(&runtime_dir, &command)?;
    let mut dos_command = runtime_command;
    if !command.args.is_empty() {
        dos_command.push(' ');
        dos_command.push_str(&command.args.join(" "));
    }
    let args = vec![
        "-dumb".to_string(),
        "-quiet".to_string(),
        "-K".to_string(),
        runtime_dir.display().to_string(),
        "-E".to_string(),
        dos_command,
    ];
    Ok(DoorRunPlan {
        program: request.door.runner.clone(),
        args,
        working_dir,
        drop_file_path,
        timeout: Duration::from_secs(u64::from(request.door.time_limit_minutes) * 60),
    })
}

fn stage_dosemu2_command(
    runtime_dir: &Path,
    command: &Dosemu2Command,
) -> Result<String, DoorError> {
    fs::create_dir_all(runtime_dir)?;
    let file_name = command.executable.file_name().ok_or_else(|| {
        DoorError::InvalidConfig("door command executable must include a filename".to_string())
    })?;
    let runtime_executable = runtime_dir.join(file_name);
    if normalize_path(command.executable.clone()) != normalize_path(runtime_executable.clone()) {
        if runtime_executable.exists() {
            fs::remove_file(&runtime_executable)?;
        }
        if let Err(link_error) = fs::hard_link(&command.executable, &runtime_executable) {
            fs::copy(&command.executable, &runtime_executable).map_err(|copy_error| {
                DoorError::Io(std::io::Error::new(
                    copy_error.kind(),
                    format!(
                        "failed to stage door command {} into runtime {}: hard link failed: {link_error}; copy failed: {copy_error}",
                        command.executable.display(),
                        runtime_executable.display()
                    ),
                ))
            })?;
        }
    }

    Ok(file_name.to_string_lossy().to_string())
}

fn absolute_host_path(path: &Path) -> Result<PathBuf, DoorError> {
    let path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };
    Ok(normalize_path(path))
}

fn normalize_path(path: PathBuf) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(component.as_os_str()),
            Component::Normal(part) => normalized.push(part),
        }
    }
    normalized
}

fn run_with_timeout(mut command: Command, timeout: Duration) -> Result<DoorRunResult, DoorError> {
    let mut child = command.spawn()?;
    let started = Instant::now();
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(DoorRunResult {
                exit_code: status_code(status),
                timed_out: false,
            });
        }
        if started.elapsed() >= timeout {
            let _ = child.kill();
            let _ = child.wait();
            return Ok(DoorRunResult {
                exit_code: None,
                timed_out: true,
            });
        }
        thread::sleep(Duration::from_millis(50));
    }
}

fn status_code(status: ExitStatus) -> Option<i32> {
    status.code()
}

fn split_name(name: &str) -> (String, String) {
    let mut parts = name.split_whitespace();
    let first = parts.next().unwrap_or(name).to_string();
    let last = parts.collect::<Vec<_>>().join(" ");
    (first, last)
}

fn default_runner() -> String {
    "dosemu".to_string()
}

fn default_drop_file() -> String {
    "DOOR.SYS".to_string()
}

fn default_time_limit() -> u32 {
    30
}

fn default_enabled() -> bool {
    true
}

impl DoorConfigFile {
    fn into_definitions(self) -> Result<Vec<DoorDefinition>, DoorError> {
        self.definitions
            .into_iter()
            .map(DoorConfigDefinition::into_definition)
            .collect()
    }
}

impl DoorConfigDefinition {
    fn into_definition(self) -> Result<DoorDefinition, DoorError> {
        if self.key.trim().is_empty() {
            return Err(DoorError::InvalidConfig("door key is required".to_string()));
        }
        if self.command.trim().is_empty() {
            return Err(DoorError::InvalidConfig(format!(
                "door {:?} command is required",
                self.key
            )));
        }
        Ok(DoorDefinition {
            id: format!("door-{}", self.key.trim()),
            key: self.key.trim().to_string(),
            name: self.name.trim().to_string(),
            runner: self.runner,
            working_dir: self.working_dir,
            command: self.command,
            drop_file: self.drop_file,
            exclusive: self.exclusive,
            time_limit_minutes: self.time_limit_minutes,
            enabled: self.enabled,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("oxidebbs-door-{name}-{}", std::process::id()))
    }

    fn caller() -> DoorCaller {
        DoorCaller {
            alias: "Alice".to_string(),
            real_name: "Alice Sysop".to_string(),
            location: "Localhost".to_string(),
            security_level: 50,
            minutes_remaining: 30,
        }
    }

    fn door(drop_file: &str) -> DoorDefinition {
        DoorDefinition {
            id: "door-lord".to_string(),
            key: "lord".to_string(),
            name: "Legend of the Red Dragon".to_string(),
            runner: "dosemu".to_string(),
            working_dir: "./doors/lord".to_string(),
            command: "LORD.EXE".to_string(),
            drop_file: drop_file.to_string(),
            exclusive: false,
            time_limit_minutes: 1,
            enabled: true,
        }
    }

    fn door_with_working_dir(drop_file: &str, working_dir: &Path) -> DoorDefinition {
        let mut door = door(drop_file);
        door.working_dir = working_dir.display().to_string();
        door
    }

    fn write_command_fixture(working_dir: &Path, command_name: &str) {
        fs::create_dir_all(working_dir).expect("create working dir");
        fs::write(working_dir.join(command_name), b"fixture command").expect("write command");
    }

    #[test]
    fn parses_doors_toml_definitions() {
        let doors = parse_doors_toml(
            r#"
[[definitions]]
key = "lord"
name = "Legend of the Red Dragon"
working_dir = "./doors/lord"
command = "LORD.EXE"
"#,
        )
        .expect("parse");

        assert_eq!(doors.len(), 1);
        assert_eq!(doors[0].key, "lord");
        assert_eq!(doors[0].drop_file, "DOOR.SYS");
    }

    #[test]
    fn creates_node_runtime_directory() {
        let base = temp_path("runtime");
        let path = prepare_node_runtime_dir(&base, 2).expect("prepare");

        assert_eq!(path, base.join("node-002"));
        assert!(path.is_dir());
        #[cfg(unix)]
        {
            let mode = fs::metadata(&path)
                .expect("dir metadata")
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(mode, 0o700);
        }

        cleanup_node_runtime_dir(&base).expect("cleanup");
    }

    #[test]
    fn identifies_dosemu2_cli_compatible_runner_names() {
        assert!(runner_supports_dosemu2_cli("dosemu"));
        assert!(runner_supports_dosemu2_cli("/usr/bin/dosemu.bin"));
        assert!(runner_supports_dosemu2_cli("dosemu2"));
        assert!(!runner_supports_dosemu2_cli("dosbox"));
        assert!(!runner_supports_dosemu2_cli("/usr/bin/dosbox-staging"));
    }

    #[test]
    fn renders_door_sys_with_caller_context() {
        let contents = render_door_sys(&caller(), 1, 38_400);

        assert!(contents.contains("Alice\r\n"));
        assert!(contents.contains("38400\r\n"));
    }

    #[test]
    fn renders_dorinfo1_def_with_split_name() {
        let contents = render_dorinfo1_def("Oxide", "Sysop", &caller());

        assert!(contents.contains("Alice\r\nSysop\r\n"));
    }

    #[test]
    fn dry_run_writes_drop_file_and_finishes_successfully() {
        let base = temp_path("dry-run");
        let runtime_dir = prepare_node_runtime_dir(&base, 1).expect("runtime");
        let working_dir = base.join("door-files");
        write_command_fixture(&working_dir, "LORD.EXE");
        let request = DoorRunRequest {
            door: door_with_working_dir("DOOR.SYS", &working_dir),
            caller: caller(),
            board_name: "Test Board".to_string(),
            sysop_name: "Test Sysop".to_string(),
            node_number: 1,
            runtime_dir: runtime_dir.clone(),
        };

        let result = DryRunDoorRunner.run(&request).expect("dry run");

        assert_eq!(result.exit_code, Some(0));
        assert!(runtime_dir.join("DOOR.SYS").is_file());

        cleanup_node_runtime_dir(&base).expect("cleanup");
    }

    #[test]
    fn dry_run_writes_oxnode_txt() {
        let base = temp_path("dry-run-node");
        let runtime_dir = prepare_node_runtime_dir(&base, 1).expect("runtime");
        let working_dir = base.join("door-files");
        write_command_fixture(&working_dir, "LORD.EXE");
        let request = DoorRunRequest {
            door: door_with_working_dir("DOOR.SYS", &working_dir),
            caller: caller(),
            board_name: "Test Board".to_string(),
            sysop_name: "Test Sysop".to_string(),
            node_number: 1,
            runtime_dir: runtime_dir.clone(),
        };

        let result = DryRunDoorRunner.run(&request).expect("dry run");
        let oxnode = fs::read_to_string(runtime_dir.join("OXNODE.TXT")).expect("oxnode");

        assert_eq!(result.exit_code, Some(0));
        assert!(runtime_dir.join("DOOR.SYS").is_file());
        assert_eq!(oxnode, "node=1\r\n");

        cleanup_node_runtime_dir(&base).expect("cleanup");
    }

    #[test]
    fn dosemu2_plan_stages_command_into_runtime() {
        let base = temp_path("dosemu2-plan");
        let door_dir = base.join("doors/lord");
        let runtime_dir = base.join("runtime/node-001");
        write_command_fixture(&door_dir, "LORD.EXE");
        let request = DoorRunRequest {
            door: door_with_working_dir("DORINFO1.DEF", &door_dir),
            caller: caller(),
            board_name: "Test Board".to_string(),
            sysop_name: "Test Sysop".to_string(),
            node_number: 1,
            runtime_dir: runtime_dir.clone(),
        };
        let plan = dosemu2_plan(&request, runtime_dir.join("DORINFO1.DEF")).expect("plan");
        let expected_runtime_dir = absolute_host_path(&runtime_dir).expect("runtime abs");

        assert_eq!(plan.program, "dosemu");
        assert_eq!(plan.args[0], "-dumb");
        assert_eq!(plan.args[1], "-quiet");
        assert_eq!(plan.args[2], "-K");
        assert_eq!(plan.args[3], expected_runtime_dir.display().to_string());
        assert_eq!(plan.args[4], "-E");
        assert_eq!(plan.args[5], "LORD.EXE");
        assert_eq!(plan.working_dir, expected_runtime_dir);
        assert_eq!(plan.timeout, Duration::from_secs(60));
        assert!(runtime_dir.join("LORD.EXE").is_file());

        cleanup_node_runtime_dir(&base).expect("cleanup");
    }

    #[test]
    fn dosemu2_plan_uses_configured_runner_program() {
        let base = temp_path("dosemu2-plan-runner");
        let door_dir = base.join("doors/lord");
        let runtime_dir = base.join("runtime/node-001");
        write_command_fixture(&door_dir, "LORD.EXE");
        let mut door = door("DOOR.SYS");
        door.runner = "/opt/dosemu2/bin/dosemu".to_string();
        door.working_dir = door_dir.display().to_string();
        let request = DoorRunRequest {
            door,
            caller: caller(),
            board_name: "Test Board".to_string(),
            sysop_name: "Test Sysop".to_string(),
            node_number: 1,
            runtime_dir: runtime_dir.clone(),
        };

        let plan = dosemu2_plan(&request, runtime_dir.join("DOOR.SYS")).expect("plan");

        assert_eq!(plan.program, "/opt/dosemu2/bin/dosemu");

        cleanup_node_runtime_dir(&base).expect("cleanup");
    }

    #[test]
    fn dosemu2_plan_resolves_bare_command_through_working_dir() {
        let base = temp_path("dosemu2-plan-command");
        let door_dir = base.join("doors/lord");
        let runtime_dir = base.join("runtime/node-001");
        write_command_fixture(&door_dir, "OXIDECHK.EXE");
        let request = DoorRunRequest {
            door: {
                let mut command_door = door("DORINFO1.DEF");
                command_door.working_dir = door_dir.display().to_string();
                command_door.command = "OXIDECHK.EXE".to_string();
                command_door
            },
            caller: caller(),
            board_name: "Test Board".to_string(),
            sysop_name: "Test Sysop".to_string(),
            node_number: 1,
            runtime_dir: runtime_dir.clone(),
        };
        let plan = dosemu2_plan(&request, runtime_dir.join("DORINFO1.DEF")).expect("plan");

        assert_eq!(plan.args[5], "OXIDECHK.EXE");
        assert!(runtime_dir.join("OXIDECHK.EXE").is_file());

        cleanup_node_runtime_dir(&base).expect("cleanup");
    }

    #[test]
    fn resolve_dosemu2_command_preserves_bare_token_and_args() {
        let working_dir = Path::new("/doors/lord");
        let resolved = resolve_dosemu2_command(working_dir, "LORD.EXE /N1").expect("resolved");
        assert_eq!(
            resolved.executable,
            Path::new("/doors/lord").join("LORD.EXE")
        );
        assert_eq!(resolved.args, vec!["/N1"]);
    }

    #[test]
    fn resolve_dosemu2_command_rejects_path_like_token() {
        let error = resolve_dosemu2_command(Path::new("/doors/lord"), "C:\\LORD\\START.BAT")
            .expect_err("expected error");
        match error {
            DoorError::InvalidConfig(message) => {
                assert!(message.contains("path-like DOS commands are not supported"));
            }
            _ => panic!("unexpected error variant"),
        }
    }

    #[test]
    fn resolve_dosemu2_command_rejects_quoted_command() {
        let error = resolve_dosemu2_command(Path::new("/doors/lord"), "\"START.BAT\"")
            .expect_err("expected error");
        match error {
            DoorError::InvalidConfig(message) => {
                assert!(message.contains("quoted DOS commands are not supported"));
            }
            _ => panic!("unexpected error variant"),
        }
    }

    #[test]
    fn prepare_door_run_rejects_empty_command() {
        let base = temp_path("dry-run-empty");
        let runtime_dir = prepare_node_runtime_dir(&base, 1).expect("runtime");
        let request = DoorRunRequest {
            door: {
                let mut empty_command = door("DOOR.SYS");
                empty_command.command = "   ".to_string();
                empty_command
            },
            caller: caller(),
            board_name: "Test Board".to_string(),
            sysop_name: "Test Sysop".to_string(),
            node_number: 1,
            runtime_dir: runtime_dir.clone(),
        };

        let error = prepare_door_run(&request).expect_err("missing command");
        match error {
            DoorError::InvalidConfig(message) => {
                assert!(message.contains("door command is required"));
            }
            _ => panic!("unexpected error variant"),
        }

        cleanup_node_runtime_dir(&base).expect("cleanup");
    }

    #[test]
    fn prepare_door_run_writes_dorinfo1_def_into_runtime_dir() {
        let base = temp_path("runtime-dorinfo");
        let runtime_dir = prepare_node_runtime_dir(&base, 1).expect("runtime");
        let working_dir = base.join("door-files");
        write_command_fixture(&working_dir, "LORD.EXE");
        let request = DoorRunRequest {
            door: door_with_working_dir("DORINFO1.DEF", &working_dir),
            caller: caller(),
            board_name: "Test Board".to_string(),
            sysop_name: "Test Sysop".to_string(),
            node_number: 1,
            runtime_dir: runtime_dir.clone(),
        };

        DryRunDoorRunner.run(&request).expect("dry run");

        let drop_file = fs::read_to_string(runtime_dir.join("DORINFO1.DEF")).expect("drop file");
        assert!(drop_file.starts_with("Test Board\r\nTest Sysop\r\n"));
        assert!(drop_file.contains("Alice\r\nSysop\r\n"));

        cleanup_node_runtime_dir(&base).expect("cleanup");
    }

    #[test]
    fn prepare_door_run_writes_door_sys_into_runtime_dir() {
        let base = temp_path("runtime-doorsys");
        let runtime_dir = prepare_node_runtime_dir(&base, 1).expect("runtime");
        let working_dir = base.join("door-files");
        write_command_fixture(&working_dir, "LORD.EXE");
        let request = DoorRunRequest {
            door: door_with_working_dir("DOOR.SYS", &working_dir),
            caller: caller(),
            board_name: "Test Board".to_string(),
            sysop_name: "Test Sysop".to_string(),
            node_number: 1,
            runtime_dir: runtime_dir.clone(),
        };

        DryRunDoorRunner.run(&request).expect("dry run");

        assert!(runtime_dir.join("DOOR.SYS").is_file());

        cleanup_node_runtime_dir(&base).expect("cleanup");
    }
}
