//! Door definitions, drop files, and runners.

use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, ExitStatus};
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
pub struct DosBoxRunner;

impl DoorRunner for DosBoxRunner {
    fn run(&self, request: &DoorRunRequest) -> Result<DoorRunResult, DoorError> {
        let plan = prepare_door_run(request)?;
        let mut command = Command::new(&plan.program);
        command.args(&plan.args).current_dir(&plan.working_dir);
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

pub fn prepare_node_runtime_dir(
    base: impl AsRef<Path>,
    node_number: u16,
) -> Result<PathBuf, DoorError> {
    let dir = node_runtime_dir(base, node_number);
    fs::create_dir_all(&dir)?;
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
        "DORINFO1.DEF" => render_dorinfo1_def("OxideBBS", "Sysop", &request.caller),
        _ => render_door_sys(&request.caller, request.node_number, 38_400),
    };
    fs::write(&drop_file_path, drop_contents)?;
    fs::write(
        request.runtime_dir.join("OXNODE.TXT"),
        format!("node={}\r\n", request.node_number),
    )?;

    dosbox_plan(request, drop_file_path)
}

pub fn resolve_dosbox_command(command: &str) -> Result<String, DoorError> {
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

    let mut split = trimmed.splitn(2, |c: char| c.is_ascii_whitespace());
    let command_token = split.next().unwrap_or_default();
    let args = split.next().unwrap_or_default();

    if command_token.contains(':') || command_token.contains('\\') || command_token.contains('/') {
        if args.is_empty() {
            return Ok(command_token.to_string());
        }
        return Ok(format!("{command_token} {args}"));
    }

    if args.is_empty() {
        Ok(command_token.to_string())
    } else {
        Ok(format!("{command_token} {args}"))
    }
}

pub fn dosbox_plan(
    request: &DoorRunRequest,
    drop_file_path: PathBuf,
) -> Result<DoorRunPlan, DoorError> {
    let command = resolve_dosbox_command(&request.door.command)?;
    let working_dir = absolute_host_path(&request.runtime_dir)?;
    let door_working_dir = absolute_host_path(Path::new(&request.door.working_dir))?;
    let runtime_dir = absolute_host_path(&request.runtime_dir)?;
    Ok(DoorRunPlan {
        program: request.door.runner.clone(),
        args: vec![
            "-c".to_string(),
            mount_command("c", &door_working_dir),
            "-c".to_string(),
            mount_command("d", &runtime_dir),
            "-c".to_string(),
            "path C:\\".to_string(),
            "-c".to_string(),
            "d:".to_string(),
            "-c".to_string(),
            command,
            "-c".to_string(),
            "exit".to_string(),
        ],
        working_dir,
        drop_file_path,
        timeout: Duration::from_secs(u64::from(request.door.time_limit_minutes) * 60),
    })
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

fn mount_command(drive: &str, host_path: &Path) -> String {
    format!("mount {drive} \"{}\"", host_path.display())
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
    "dosbox".to_string()
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
            runner: "dosbox".to_string(),
            working_dir: "./doors/lord".to_string(),
            command: "LORD.EXE".to_string(),
            drop_file: drop_file.to_string(),
            exclusive: false,
            time_limit_minutes: 1,
            enabled: true,
        }
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

        cleanup_node_runtime_dir(&base).expect("cleanup");
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
        let request = DoorRunRequest {
            door: door("DOOR.SYS"),
            caller: caller(),
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
        let request = DoorRunRequest {
            door: door("DOOR.SYS"),
            caller: caller(),
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
    fn dosbox_plan_mounts_door_working_directory_as_c_and_runtime_as_d() {
        let cwd = std::env::current_dir().expect("current dir");
        let expected_door_dir = cwd.join("doors/lord");
        let expected_runtime_dir = cwd.join("runtime/node-001");
        let request = DoorRunRequest {
            door: door("DORINFO1.DEF"),
            caller: caller(),
            node_number: 1,
            runtime_dir: PathBuf::from("./runtime/node-001"),
        };
        let plan =
            dosbox_plan(&request, PathBuf::from("./runtime/node-001/DORINFO1.DEF")).expect("plan");

        let command_windows = plan.args.windows(2).enumerate().collect::<Vec<_>>();
        let mount_c = command_windows
            .iter()
            .find(|(_, window)| window[0] == "-c" && window[1].starts_with("mount c "))
            .map(|(idx, _window)| *idx);
        let mount_d = command_windows
            .iter()
            .find(|(_, window)| window[0] == "-c" && window[1].starts_with("mount d "))
            .map(|(idx, _window)| *idx);
        let switch_d = command_windows
            .iter()
            .find(|(_, window)| window[0] == "-c" && window[1] == "d:")
            .map(|(idx, _)| *idx);
        let path_c = command_windows
            .iter()
            .find(|(_, window)| window[0] == "-c" && window[1] == "path C:\\")
            .map(|(idx, _)| *idx);
        let command_arg = command_windows
            .iter()
            .find(|(_, window)| window[0] == "-c" && window[1] == "LORD.EXE")
            .map(|(_, window)| window[1].as_str());

        assert_eq!(plan.program, "dosbox");
        let mount_c_idx = mount_c.expect("expected mount c command pair");
        let mount_d_idx = mount_d.expect("expected mount d command pair");
        let path_c_idx = path_c.expect("expected path c command pair");
        let switch_d_idx = switch_d.expect("expected d: switch command pair");
        let command_arg = command_arg.expect("expected resolved DOS command");
        assert_eq!(
            plan.args[mount_c_idx + 1],
            format!("mount c \"{}\"", expected_door_dir.display())
        );
        assert_eq!(
            plan.args[mount_d_idx + 1],
            format!("mount d \"{}\"", expected_runtime_dir.display())
        );
        assert_eq!(plan.args[path_c_idx + 1], "path C:\\");
        assert_eq!(plan.args[switch_d_idx + 1], "d:");
        assert!(mount_c_idx < mount_d_idx);
        assert!(mount_d_idx < path_c_idx);
        assert!(path_c_idx < switch_d_idx);
        assert_eq!(command_arg, "LORD.EXE");
        assert_eq!(plan.working_dir, expected_runtime_dir);
        assert_eq!(plan.timeout, Duration::from_secs(60));
    }

    #[test]
    fn dosbox_plan_uses_configured_runner_program() {
        let mut door = door("DOOR.SYS");
        door.runner = "/opt/dosbox-staging/dosbox".to_string();
        let request = DoorRunRequest {
            door,
            caller: caller(),
            node_number: 1,
            runtime_dir: PathBuf::from("./runtime/node-001"),
        };

        let plan =
            dosbox_plan(&request, PathBuf::from("./runtime/node-001/DOOR.SYS")).expect("plan");

        assert_eq!(plan.program, "/opt/dosbox-staging/dosbox");
    }

    #[test]
    fn dosbox_plan_resolves_bare_command_through_path() {
        let request = DoorRunRequest {
            door: {
                let mut command_door = door("DORINFO1.DEF");
                command_door.command = "OXIDECHK.EXE".to_string();
                command_door
            },
            caller: caller(),
            node_number: 1,
            runtime_dir: PathBuf::from("./runtime/node-001"),
        };
        let plan =
            dosbox_plan(&request, PathBuf::from("./runtime/node-001/DORINFO1.DEF")).expect("plan");

        let command = plan
            .args
            .iter()
            .find(|arg| arg.contains("OXIDECHK.EXE"))
            .expect("missing command arg");
        assert_eq!(command, "OXIDECHK.EXE");
    }

    #[test]
    fn resolve_dosbox_command_preserves_bare_token() {
        assert_eq!(
            resolve_dosbox_command("LORD.EXE /N1").expect("resolved"),
            "LORD.EXE /N1"
        );
    }

    #[test]
    fn resolve_dosbox_command_preserves_path_like_token() {
        assert_eq!(
            resolve_dosbox_command("C:\\LORD\\START.BAT").expect("resolved"),
            "C:\\LORD\\START.BAT"
        );
        assert_eq!(
            resolve_dosbox_command("UTILS\\DOOR.EXE").expect("resolved"),
            "UTILS\\DOOR.EXE"
        );
    }

    #[test]
    fn resolve_dosbox_command_rejects_quoted_command() {
        let error = resolve_dosbox_command("\"C:\\LORD\\START.BAT\"").expect_err("expected error");
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
        let request = DoorRunRequest {
            door: door("DORINFO1.DEF"),
            caller: caller(),
            node_number: 1,
            runtime_dir: runtime_dir.clone(),
        };

        DryRunDoorRunner.run(&request).expect("dry run");

        assert!(runtime_dir.join("DORINFO1.DEF").is_file());

        cleanup_node_runtime_dir(&base).expect("cleanup");
    }

    #[test]
    fn prepare_door_run_writes_door_sys_into_runtime_dir() {
        let base = temp_path("runtime-doorsys");
        let runtime_dir = prepare_node_runtime_dir(&base, 1).expect("runtime");
        let request = DoorRunRequest {
            door: door("DOOR.SYS"),
            caller: caller(),
            node_number: 1,
            runtime_dir: runtime_dir.clone(),
        };

        DryRunDoorRunner.run(&request).expect("dry run");

        assert!(runtime_dir.join("DOOR.SYS").is_file());

        cleanup_node_runtime_dir(&base).expect("cleanup");
    }
}
