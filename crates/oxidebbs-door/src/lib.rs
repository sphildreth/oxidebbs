//! Door definitions, drop files, and runners.

use std::fs;
use std::path::{Path, PathBuf};
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

    Ok(dosbox_plan(request, drop_file_path))
}

pub fn dosbox_plan(request: &DoorRunRequest, drop_file_path: PathBuf) -> DoorRunPlan {
    let working_dir = PathBuf::from(&request.door.working_dir);
    DoorRunPlan {
        program: "dosbox".to_string(),
        args: vec![
            "-c".to_string(),
            format!("mount c {}", working_dir.display()),
            "-c".to_string(),
            "c:".to_string(),
            "-c".to_string(),
            request.door.command.clone(),
            "-c".to_string(),
            "exit".to_string(),
        ],
        working_dir,
        drop_file_path,
        timeout: Duration::from_secs(u64::from(request.door.time_limit_minutes) * 60),
    }
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
    fn dosbox_plan_mounts_door_working_directory() {
        let request = DoorRunRequest {
            door: door("DORINFO1.DEF"),
            caller: caller(),
            node_number: 1,
            runtime_dir: PathBuf::from("./runtime/node-001"),
        };
        let plan = dosbox_plan(&request, PathBuf::from("./runtime/node-001/DORINFO1.DEF"));

        assert_eq!(plan.program, "dosbox");
        assert!(plan.args.iter().any(|arg| arg.contains("mount c")));
        assert_eq!(plan.timeout, Duration::from_secs(60));
    }
}
