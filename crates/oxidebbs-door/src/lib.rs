//! Door definitions, drop files, and runners.

use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
#[cfg(unix)]
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Component, Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant};

mod oxdoor_package;
use oxidebbs_core::door::DoorDefinition;
use serde::Deserialize;
use thiserror::Error;

pub const CRATE_NAME: &str = "oxidebbs-door";

pub use crate::oxdoor_package::{
    inspect_oxide_door_package, OxDoorPackageInspection, OxDoorPackageSummary,
};

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

    #[error("failed to read door package {path}: {source}")]
    ReadDoorPackage {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("failed to parse door package {path}: {source}")]
    ParseDoorPackage {
        path: PathBuf,
        source: toml::de::Error,
    },

    #[error("invalid door package {path}: {message}")]
    InvalidDoorPackage {
        path: PathBuf,
        message: String,
    },
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

pub fn sync_door_runtime_outputs(
    runtime_dir: impl AsRef<Path>,
    working_dir: impl AsRef<Path>,
    drop_file: &str,
) -> Result<(), DoorError> {
    let runtime_dir = absolute_host_path(runtime_dir.as_ref())?;
    let working_dir = absolute_host_path(working_dir.as_ref())?;
    if runtime_dir == working_dir || !runtime_dir.exists() {
        return Ok(());
    }

    fs::create_dir_all(&working_dir)?;
    sync_runtime_output_dir(&runtime_dir, &runtime_dir, &working_dir, drop_file)
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
    let (sysop_first_name, sysop_last_name) = split_name(sysop_name);
    let (first_name, last_name) = split_name(&caller.real_name);
    [
        board_name.to_string(),
        sysop_first_name,
        sysop_last_name,
        "COM1".to_string(),
        "38400 BAUD,N,8,1".to_string(),
        "0".to_string(),
        first_name,
        last_name,
        caller.location.clone(),
        "1".to_string(),
        caller.security_level.to_string(),
        caller.minutes_remaining.to_string(),
    ]
    .join("\r\n")
        + "\r\n"
}

pub fn render_chain_txt(_caller: &DoorCaller, node_number: u16, baud_rate: u32) -> String {
    format!("{} {} COM1\r\n", node_number, baud_rate)
}

pub fn render_doorfile_sr(
    caller: &DoorCaller,
    node_number: u16,
    baud_rate: u32,
    board_name: &str,
    sysop_name: &str,
) -> String {
    [
        board_name.to_string(),
        sysop_name.to_string(),
        "COM1".to_string(),
        baud_rate.to_string(),
        "8N1".to_string(),
        node_number.to_string(),
        caller.alias.clone(),
        caller.real_name.clone(),
        caller.location.clone(),
        caller.security_level.to_string(),
        caller.minutes_remaining.to_string(),
        "N".to_string(),
        "0".to_string(),
    ]
    .join("\r\n")
        + "\r\n"
}

pub fn render_pcboard_sys(
    caller: &DoorCaller,
    node_number: u16,
    baud_rate: u32,
    board_name: &str,
) -> String {
    [
        board_name.to_string(),
        caller.alias.clone(),
        caller.real_name.clone(),
        "COM1".to_string(),
        baud_rate.to_string(),
        "8N1".to_string(),
        node_number.to_string(),
        caller.security_level.to_string(),
        caller.minutes_remaining.to_string(),
    ]
    .join("\r\n")
        + "\r\n"
}

pub fn render_callinfo_bbs(caller: &DoorCaller, node_number: u16, baud_rate: u32) -> String {
    [
        caller.alias.clone(),
        node_number.to_string(),
        baud_rate.to_string(),
        "COM1".to_string(),
        "Y".to_string(),
        caller.location.clone(),
    ]
    .join("\r\n")
        + "\r\n"
}

pub fn prepare_door_run(request: &DoorRunRequest) -> Result<DoorRunPlan, DoorError> {
    fs::create_dir_all(&request.runtime_dir)?;
    let drop_file_path = request.runtime_dir.join(&request.door.drop_file);
    let plan = dosemu2_plan(request, drop_file_path.clone())?;
    let drop_contents = match request.door.drop_file.to_ascii_uppercase().as_str() {
        "DORINFO1.DEF" => {
            render_dorinfo1_def(&request.board_name, &request.sysop_name, &request.caller)
        }
        "CHAIN.TXT" => render_chain_txt(&request.caller, request.node_number, 38_400),
        "DOORFILE.SR" => render_doorfile_sr(
            &request.caller,
            request.node_number,
            38_400,
            &request.board_name,
            &request.sysop_name,
        ),
        "PCBOARD.SYS" => render_pcboard_sys(
            &request.caller,
            request.node_number,
            38_400,
            &request.board_name,
        ),
        "CALLINFO.BBS" => render_callinfo_bbs(&request.caller, request.node_number, 38_400),
        _ => render_door_sys(&request.caller, request.node_number, 38_400),
    };
    fs::write(&drop_file_path, drop_contents)?;
    fs::write(
        request.runtime_dir.join("OXNODE.TXT"),
        format!("node={}\r\n", request.node_number),
    )?;

    Ok(plan)
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
    stage_dosemu2_working_dir(&door_working_dir, &runtime_dir)?;
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

fn stage_dosemu2_working_dir(source_dir: &Path, runtime_dir: &Path) -> Result<(), DoorError> {
    fs::create_dir_all(runtime_dir)?;
    if normalize_path(source_dir.to_path_buf()) == normalize_path(runtime_dir.to_path_buf()) {
        return Ok(());
    }

    for entry in fs::read_dir(source_dir)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        if file_type.is_symlink() {
            continue;
        }
        stage_dosemu2_support_entry(
            &entry.path(),
            &runtime_dir.join(entry.file_name()),
            &file_type,
        )?;
    }

    Ok(())
}

fn stage_dosemu2_support_entry(
    source_path: &Path,
    target_path: &Path,
    file_type: &fs::FileType,
) -> Result<(), DoorError> {
    if file_type.is_dir() {
        fs::create_dir_all(target_path)?;
        for entry in fs::read_dir(source_path)? {
            let entry = entry?;
            let child_file_type = entry.file_type()?;
            if child_file_type.is_symlink() {
                continue;
            }
            stage_dosemu2_support_entry(
                &entry.path(),
                &target_path.join(entry.file_name()),
                &child_file_type,
            )?;
        }
        return Ok(());
    }

    if file_type.is_file() {
        stage_dosemu2_file(source_path, target_path)?;
    }

    Ok(())
}

fn stage_dosemu2_file(source_path: &Path, target_path: &Path) -> Result<(), DoorError> {
    if let Some(parent) = target_path.parent() {
        fs::create_dir_all(parent)?;
    }
    if target_path.exists() {
        if fs::symlink_metadata(target_path)?.is_dir() {
            fs::remove_dir_all(target_path)?;
        } else {
            fs::remove_file(target_path)?;
        }
    }
    if let Err(link_error) = fs::hard_link(source_path, target_path) {
        fs::copy(source_path, target_path).map_err(|copy_error| {
            DoorError::Io(std::io::Error::new(
                copy_error.kind(),
                format!(
                    "failed to stage door support file {} into runtime {}: hard link failed: {link_error}; copy failed: {copy_error}",
                    source_path.display(),
                    target_path.display()
                ),
            ))
        })?;
    }
    Ok(())
}

const GENERATED_RUNTIME_FILES: &[&str] = &[
    "CALLINFO.BBS",
    "CHAIN.TXT",
    "DOOR.SYS",
    "DOORFILE.SR",
    "DORINFO1.DEF",
    "OXCOM1.PTY",
    "OXDOSEMU2.CONF",
    "OXNODE.TXT",
    "PCBOARD.SYS",
];

fn sync_runtime_output_dir(
    runtime_root: &Path,
    source_dir: &Path,
    working_root: &Path,
    drop_file: &str,
) -> Result<(), DoorError> {
    for entry in fs::read_dir(source_dir)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        if file_type.is_symlink() {
            continue;
        }

        let source_path = entry.path();
        let relative_path = source_path.strip_prefix(runtime_root).map_err(|error| {
            DoorError::Io(std::io::Error::other(format!(
                "failed to compute runtime output path for {} relative to {}: {error}",
                source_path.display(),
                runtime_root.display()
            )))
        })?;
        if is_generated_runtime_output(relative_path, drop_file) {
            continue;
        }

        sync_runtime_output_entry(
            runtime_root,
            &source_path,
            &working_root.join(entry.file_name()),
            &file_type,
            drop_file,
        )?;
    }
    Ok(())
}

fn sync_runtime_output_entry(
    runtime_root: &Path,
    source_path: &Path,
    target_path: &Path,
    file_type: &fs::FileType,
    drop_file: &str,
) -> Result<(), DoorError> {
    if file_type.is_dir() {
        if target_path.exists() && !target_path.is_dir() {
            fs::remove_file(target_path)?;
        }
        fs::create_dir_all(target_path)?;
        return sync_runtime_output_dir(runtime_root, source_path, target_path, drop_file);
    }

    if !file_type.is_file() {
        return Ok(());
    }

    if paths_refer_to_same_file(source_path, target_path)? {
        return Ok(());
    }
    if target_path.is_file() && files_have_same_contents(source_path, target_path)? {
        return Ok(());
    }

    if let Some(parent) = target_path.parent() {
        fs::create_dir_all(parent)?;
    }
    if target_path.exists() {
        if fs::symlink_metadata(target_path)?.is_dir() {
            fs::remove_dir_all(target_path)?;
        } else {
            fs::remove_file(target_path)?;
        }
    }
    fs::copy(source_path, target_path).map_err(|error| {
        DoorError::Io(std::io::Error::new(
            error.kind(),
            format!(
                "failed to sync door runtime output {} back to {}: {error}",
                source_path.display(),
                target_path.display()
            ),
        ))
    })?;
    Ok(())
}

fn is_generated_runtime_output(relative_path: &Path, drop_file: &str) -> bool {
    if relative_path.components().count() != 1 {
        return false;
    }
    let Some(name) = relative_path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    is_generated_runtime_file_name(name, drop_file)
}

fn is_generated_runtime_file_name(name: &str, drop_file: &str) -> bool {
    let name = name.to_ascii_uppercase();
    if GENERATED_RUNTIME_FILES.contains(&name.as_str()) {
        return true;
    }
    let drop_file = Path::new(drop_file)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(drop_file)
        .to_ascii_uppercase();
    name == drop_file
}

fn paths_refer_to_same_file(left: &Path, right: &Path) -> Result<bool, DoorError> {
    if !left.exists() || !right.exists() {
        return Ok(false);
    }
    #[cfg(unix)]
    {
        let left = fs::metadata(left)?;
        let right = fs::metadata(right)?;
        Ok(left.dev() == right.dev() && left.ino() == right.ino())
    }
    #[cfg(not(unix))]
    {
        Ok(false)
    }
}

fn files_have_same_contents(left: &Path, right: &Path) -> Result<bool, DoorError> {
    let left_metadata = fs::metadata(left)?;
    let right_metadata = fs::metadata(right)?;
    if left_metadata.len() != right_metadata.len() {
        return Ok(false);
    }

    let mut left_file = fs::File::open(left)?;
    let mut right_file = fs::File::open(right)?;
    let mut left_buffer = [0_u8; 8192];
    let mut right_buffer = [0_u8; 8192];
    loop {
        let left_count = left_file.read(&mut left_buffer)?;
        let right_count = right_file.read(&mut right_buffer)?;
        if left_count != right_count {
            return Ok(false);
        }
        if left_count == 0 {
            return Ok(true);
        }
        if left_buffer[..left_count] != right_buffer[..right_count] {
            return Ok(false);
        }
    }
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

pub const BBSLINK_PROVIDER_KEY: &str = "bbslink";
pub const DOORPARTY_PROVIDER_KEY: &str = "doorparty";
pub const REDACTED_PROVIDER_SECRET: &str = "[redacted]";

#[derive(Clone, PartialEq, Eq)]
pub struct RemoteDoorSecret {
    value: String,
}

impl RemoteDoorSecret {
    pub fn new(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
        }
    }

    #[must_use]
    pub fn expose_secret(&self) -> &str {
        &self.value
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.value.trim().is_empty()
    }

    #[must_use]
    pub fn redacted(&self) -> &'static str {
        if self.value.is_empty() {
            ""
        } else {
            REDACTED_PROVIDER_SECRET
        }
    }
}

impl fmt::Debug for RemoteDoorSecret {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("RemoteDoorSecret")
            .field(&self.redacted())
            .finish()
    }
}

impl fmt::Display for RemoteDoorSecret {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.redacted())
    }
}

impl From<String> for RemoteDoorSecret {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl From<&str> for RemoteDoorSecret {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RedactedRemoteDoorProviderConfig {
    pub provider_key: String,
    pub endpoint: String,
    pub account: String,
    pub secret: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BbsLinkConfig {
    pub system_id: String,
    pub auth_code: RemoteDoorSecret,
    pub endpoint: String,
}

impl BbsLinkConfig {
    pub fn new(
        system_id: impl Into<String>,
        auth_code: impl Into<RemoteDoorSecret>,
        endpoint: impl Into<String>,
    ) -> Self {
        Self {
            system_id: system_id.into(),
            auth_code: auth_code.into(),
            endpoint: endpoint.into(),
        }
    }

    #[must_use]
    pub fn redacted(&self) -> RedactedRemoteDoorProviderConfig {
        RedactedRemoteDoorProviderConfig {
            provider_key: BBSLINK_PROVIDER_KEY.to_string(),
            endpoint: self.endpoint.clone(),
            account: self.system_id.clone(),
            secret: self.auth_code.redacted().to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct BbsLinkProvider {
    config: BbsLinkConfig,
}

impl BbsLinkProvider {
    pub fn new(config: BbsLinkConfig) -> Self {
        Self { config }
    }

    #[must_use]
    pub fn config(&self) -> &BbsLinkConfig {
        &self.config
    }

    #[must_use]
    pub fn redacted_config(&self) -> RedactedRemoteDoorProviderConfig {
        self.config.redacted()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DoorPartyConfig {
    pub account: String,
    pub password: RemoteDoorSecret,
    pub endpoint: String,
}

impl DoorPartyConfig {
    pub fn new(
        account: impl Into<String>,
        password: impl Into<RemoteDoorSecret>,
        endpoint: impl Into<String>,
    ) -> Self {
        Self {
            account: account.into(),
            password: password.into(),
            endpoint: endpoint.into(),
        }
    }

    #[must_use]
    pub fn redacted(&self) -> RedactedRemoteDoorProviderConfig {
        RedactedRemoteDoorProviderConfig {
            provider_key: DOORPARTY_PROVIDER_KEY.to_string(),
            endpoint: self.endpoint.clone(),
            account: self.account.clone(),
            secret: self.password.redacted().to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct DoorPartyProvider {
    config: DoorPartyConfig,
}

impl DoorPartyProvider {
    pub fn new(config: DoorPartyConfig) -> Self {
        Self { config }
    }

    #[must_use]
    pub fn config(&self) -> &DoorPartyConfig {
        &self.config
    }

    #[must_use]
    pub fn redacted_config(&self) -> RedactedRemoteDoorProviderConfig {
        self.config.redacted()
    }
}

pub trait RemoteDoorProvider: Send + Sync {
    fn validate_config(&self) -> Result<(), DoorError>;

    fn dry_run_session(&self, caller: &DoorCaller) -> Result<DoorRunResult, DoorError>;

    fn launch_session(
        &self,
        caller: &DoorCaller,
        io: &mut dyn RemoteSessionIo,
    ) -> Result<DoorRunResult, DoorError>;
}

pub trait RemoteSessionIo {
    fn read_byte(&mut self) -> std::io::Result<Option<u8>>;
    fn write_all(&mut self, bytes: &[u8]) -> std::io::Result<()>;
    fn flush(&mut self) -> std::io::Result<()>;
    fn has_remote_data(&self) -> bool;
    fn has_caller_data(&self) -> bool;
    fn is_remote_closed(&self) -> bool;
    fn read_caller_byte(&mut self) -> std::io::Result<Option<u8>>;
    fn write_remote(&mut self, bytes: &[u8]) -> std::io::Result<()>;
    fn read_remote_byte(&mut self) -> std::io::Result<Option<u8>>;
    fn write_caller(&mut self, bytes: &[u8]) -> std::io::Result<()>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TcpRemoteSessionResult {
    pub run_result: DoorRunResult,
    pub caller_output: Vec<u8>,
}

pub struct TcpRemoteSessionIo {
    stream: TcpStream,
    caller_input: Vec<u8>,
    caller_input_pos: usize,
    caller_output: Vec<u8>,
    remote_closed: bool,
}

impl TcpRemoteSessionIo {
    pub fn connect(endpoint: &str, caller_input: &[u8]) -> Result<Self, DoorError> {
        let target = remote_endpoint_socket_target(endpoint)?;
        let mut addrs = target.to_socket_addrs()?;
        let addr = addrs.next().ok_or_else(|| {
            DoorError::InvalidConfig(format!(
                "remote endpoint {endpoint:?} resolved no addresses"
            ))
        })?;
        let stream = TcpStream::connect_timeout(&addr, Duration::from_secs(5))?;
        stream.set_nodelay(true)?;
        stream.set_nonblocking(true)?;
        Ok(Self {
            stream,
            caller_input: caller_input.to_vec(),
            caller_input_pos: 0,
            caller_output: Vec::new(),
            remote_closed: false,
        })
    }

    #[must_use]
    pub fn caller_output(&self) -> &[u8] {
        &self.caller_output
    }

    #[must_use]
    pub fn into_caller_output(self) -> Vec<u8> {
        self.caller_output
    }

    fn remote_data_available(&self) -> std::io::Result<bool> {
        let mut byte = [0_u8; 1];
        match self.stream.peek(&mut byte) {
            Ok(count) => Ok(count > 0),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => Ok(false),
            Err(error) => Err(error),
        }
    }

    fn remote_is_closed(&self) -> std::io::Result<bool> {
        let mut byte = [0_u8; 1];
        match self.stream.peek(&mut byte) {
            Ok(0) => Ok(true),
            Ok(_) => Ok(false),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => Ok(false),
            Err(error) => Err(error),
        }
    }
}

impl RemoteSessionIo for TcpRemoteSessionIo {
    fn read_byte(&mut self) -> std::io::Result<Option<u8>> {
        self.read_remote_byte()
    }

    fn write_all(&mut self, bytes: &[u8]) -> std::io::Result<()> {
        write_nonblocking_all(&mut self.stream, bytes)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.stream.flush()
    }

    fn has_remote_data(&self) -> bool {
        !self.remote_closed && self.remote_data_available().unwrap_or(false)
    }

    fn has_caller_data(&self) -> bool {
        self.caller_input_pos < self.caller_input.len()
    }

    fn is_remote_closed(&self) -> bool {
        self.remote_closed || self.remote_is_closed().unwrap_or(false)
    }

    fn read_caller_byte(&mut self) -> std::io::Result<Option<u8>> {
        if self.caller_input_pos < self.caller_input.len() {
            let byte = self.caller_input[self.caller_input_pos];
            self.caller_input_pos += 1;
            Ok(Some(byte))
        } else {
            Ok(None)
        }
    }

    fn write_remote(&mut self, bytes: &[u8]) -> std::io::Result<()> {
        write_nonblocking_all(&mut self.stream, bytes)
    }

    fn read_remote_byte(&mut self) -> std::io::Result<Option<u8>> {
        let mut byte = [0_u8; 1];
        match self.stream.read(&mut byte) {
            Ok(0) => {
                self.remote_closed = true;
                Ok(None)
            }
            Ok(_) => Ok(Some(byte[0])),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => Ok(None),
            Err(error) => Err(error),
        }
    }

    fn write_caller(&mut self, bytes: &[u8]) -> std::io::Result<()> {
        self.caller_output.extend_from_slice(bytes);
        Ok(())
    }
}

impl RemoteDoorProvider for BbsLinkProvider {
    fn validate_config(&self) -> Result<(), DoorError> {
        validate_required_field("BBSLink system_id", &self.config.system_id)?;
        validate_required_secret("BBSLink", "auth_code", &self.config.auth_code)?;
        validate_remote_endpoint("BBSLink", &self.config.endpoint)
    }

    fn dry_run_session(&self, caller: &DoorCaller) -> Result<DoorRunResult, DoorError> {
        self.validate_config()?;
        validate_required_field("caller alias", &caller.alias)?;
        Ok(DoorRunResult {
            exit_code: Some(0),
            timed_out: false,
        })
    }

    fn launch_session(
        &self,
        caller: &DoorCaller,
        io: &mut dyn RemoteSessionIo,
    ) -> Result<DoorRunResult, DoorError> {
        self.validate_config()?;
        validate_required_field("caller alias", &caller.alias)?;

        let auth_line = format!(
            "SYS {} AUTH {}\r\n",
            self.config.system_id,
            self.config.auth_code.expose_secret()
        );
        io.write_all(auth_line.as_bytes()).map_err(DoorError::Io)?;
        io.flush().map_err(DoorError::Io)?;

        let caller_line = format!("USER {} SEC {}\r\n", caller.alias, caller.security_level);
        io.write_all(caller_line.as_bytes())
            .map_err(DoorError::Io)?;
        io.flush().map_err(DoorError::Io)?;

        bridge_remote_session(io)
    }
}

impl BbsLinkProvider {
    pub fn launch_tcp_session(
        &self,
        caller: &DoorCaller,
        caller_input: &[u8],
    ) -> Result<TcpRemoteSessionResult, DoorError> {
        let mut io = TcpRemoteSessionIo::connect(&self.config.endpoint, caller_input)?;
        let run_result = self.launch_session(caller, &mut io)?;
        Ok(TcpRemoteSessionResult {
            run_result,
            caller_output: io.into_caller_output(),
        })
    }
}

impl RemoteDoorProvider for DoorPartyProvider {
    fn validate_config(&self) -> Result<(), DoorError> {
        validate_required_field("DoorParty account", &self.config.account)?;
        validate_required_secret("DoorParty", "password", &self.config.password)?;
        validate_remote_endpoint("DoorParty", &self.config.endpoint)
    }

    fn dry_run_session(&self, caller: &DoorCaller) -> Result<DoorRunResult, DoorError> {
        self.validate_config()?;
        validate_required_field("caller alias", &caller.alias)?;
        Ok(DoorRunResult {
            exit_code: Some(0),
            timed_out: false,
        })
    }

    fn launch_session(
        &self,
        caller: &DoorCaller,
        io: &mut dyn RemoteSessionIo,
    ) -> Result<DoorRunResult, DoorError> {
        self.validate_config()?;
        validate_required_field("caller alias", &caller.alias)?;

        let auth_line = format!(
            "ACCT {} PASS {}\r\n",
            self.config.account,
            self.config.password.expose_secret()
        );
        io.write_all(auth_line.as_bytes()).map_err(DoorError::Io)?;
        io.flush().map_err(DoorError::Io)?;

        let caller_line = format!("USER {} SEC {}\r\n", caller.alias, caller.security_level);
        io.write_all(caller_line.as_bytes())
            .map_err(DoorError::Io)?;
        io.flush().map_err(DoorError::Io)?;

        bridge_remote_session(io)
    }
}

impl DoorPartyProvider {
    pub fn launch_tcp_session(
        &self,
        caller: &DoorCaller,
        caller_input: &[u8],
    ) -> Result<TcpRemoteSessionResult, DoorError> {
        let mut io = TcpRemoteSessionIo::connect(&self.config.endpoint, caller_input)?;
        let run_result = self.launch_session(caller, &mut io)?;
        Ok(TcpRemoteSessionResult {
            run_result,
            caller_output: io.into_caller_output(),
        })
    }
}

fn bridge_remote_session(io: &mut dyn RemoteSessionIo) -> Result<DoorRunResult, DoorError> {
    let mut remote_closed = false;
    let mut caller_closed = false;
    loop {
        if !remote_closed && io.has_remote_data() {
            match io.read_remote_byte() {
                Ok(Some(byte)) => {
                    io.write_caller(&[byte]).map_err(DoorError::Io)?;
                }
                Ok(None) => {
                    remote_closed = true;
                }
                Err(e) => {
                    return Err(DoorError::Io(e));
                }
            }
        } else if !remote_closed && io.is_remote_closed() {
            remote_closed = true;
        }

        if !caller_closed && io.has_caller_data() {
            match io.read_caller_byte() {
                Ok(Some(byte)) => {
                    io.write_remote(&[byte]).map_err(DoorError::Io)?;
                }
                Ok(None) => {
                    caller_closed = true;
                }
                Err(e) => {
                    return Err(DoorError::Io(e));
                }
            }
        }

        if remote_closed || caller_closed {
            return Ok(DoorRunResult {
                exit_code: Some(0),
                timed_out: false,
            });
        }

        if !io.has_remote_data() && !io.has_caller_data() {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }
}

fn write_nonblocking_all(stream: &mut TcpStream, mut bytes: &[u8]) -> std::io::Result<()> {
    while !bytes.is_empty() {
        match stream.write(bytes) {
            Ok(0) => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::WriteZero,
                    "remote provider socket accepted zero bytes",
                ));
            }
            Ok(count) => bytes = &bytes[count..],
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(5));
            }
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

fn validate_required_field(label: &str, value: &str) -> Result<(), DoorError> {
    if value.trim().is_empty() {
        return Err(DoorError::InvalidConfig(format!("{label} is required")));
    }
    Ok(())
}

fn validate_required_secret(
    provider_name: &str,
    field_name: &str,
    value: &RemoteDoorSecret,
) -> Result<(), DoorError> {
    if value.is_empty() {
        return Err(DoorError::InvalidConfig(format!(
            "{provider_name} {field_name} is required"
        )));
    }
    Ok(())
}

fn validate_remote_endpoint(provider_name: &str, endpoint: &str) -> Result<(), DoorError> {
    let trimmed = endpoint.trim();
    if trimmed.is_empty() {
        return Err(DoorError::InvalidConfig(format!(
            "{provider_name} endpoint is required"
        )));
    }
    if trimmed.chars().any(char::is_control) || trimmed.split_whitespace().count() > 1 {
        return Err(DoorError::InvalidConfig(format!(
            "{provider_name} endpoint must be a single host, host:port, or URI value"
        )));
    }
    Ok(())
}

fn remote_endpoint_socket_target(endpoint: &str) -> Result<String, DoorError> {
    let trimmed = endpoint.trim();
    if trimmed.is_empty() {
        return Err(DoorError::InvalidConfig(
            "remote provider endpoint is required".to_string(),
        ));
    }
    let authority = match trimmed.split_once("://") {
        Some(("telnet" | "tcp", rest)) => rest,
        Some((scheme, _)) => {
            return Err(DoorError::InvalidConfig(format!(
                "remote provider endpoint scheme {scheme:?} is not supported; use host:port, telnet://host:port, or tcp://host:port"
            )));
        }
        None => trimmed,
    };
    let authority = authority.trim_end_matches('/');
    if authority.is_empty() || authority.contains('/') {
        return Err(DoorError::InvalidConfig(
            "remote provider endpoint must not include a path".to_string(),
        ));
    }
    if endpoint_authority_has_port(authority) {
        Ok(authority.to_string())
    } else {
        Ok(format!("{authority}:23"))
    }
}

fn endpoint_authority_has_port(authority: &str) -> bool {
    if let Some(rest) = authority.strip_prefix('[') {
        return rest
            .split_once(']')
            .and_then(|(_, after_bracket)| after_bracket.strip_prefix(':'))
            .is_some_and(|port| port.parse::<u16>().is_ok());
    }
    authority
        .rsplit_once(':')
        .is_some_and(|(_, port)| port.parse::<u16>().is_ok())
}

#[derive(Default)]
pub struct ProviderRegistry {
    providers: HashMap<String, Box<dyn RemoteDoorProvider>>,
}

impl ProviderRegistry {
    pub fn new() -> Self {
        Self {
            providers: HashMap::new(),
        }
    }

    pub fn register(&mut self, name: impl Into<String>, provider: Box<dyn RemoteDoorProvider>) {
        self.providers.insert(name.into(), provider);
    }

    pub fn get(&self, name: &str) -> Option<&dyn RemoteDoorProvider> {
        self.providers.get(name).map(Box::as_ref)
    }

    pub fn names(&self) -> Vec<&str> {
        self.providers.keys().map(String::as_str).collect()
    }
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
            min_security_level: 0,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::{Shutdown, TcpListener};
    use std::thread::JoinHandle;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("oxidebbs-door-{name}-{}", std::process::id()))
    }

    fn remote_provider_secret() -> String {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos()
            .to_string()
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
            min_security_level: 0,
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
    fn syncs_runtime_outputs_back_without_generated_drop_files() {
        let base = temp_path("sync-outputs");
        let _ = fs::remove_dir_all(&base);
        let runtime = base.join("runtime");
        let working = base.join("working");
        fs::create_dir_all(runtime.join("data")).expect("create runtime");
        fs::create_dir_all(&working).expect("create working");
        fs::write(working.join("KEEP.DAT"), b"old score").expect("write old data");
        fs::write(runtime.join("KEEP.DAT"), b"new score").expect("write changed data");
        fs::write(runtime.join("SCORE.DAT"), b"scoreboard").expect("write new data");
        fs::write(runtime.join("DORINFO1.DEF"), b"drop").expect("write drop file");
        fs::write(runtime.join("OXNODE.TXT"), b"node").expect("write runtime file");
        fs::write(runtime.join("data").join("HOF.DAT"), b"hall").expect("write nested data");

        sync_door_runtime_outputs(&runtime, &working, "DORINFO1.DEF").expect("sync outputs");

        assert_eq!(
            fs::read(working.join("KEEP.DAT")).expect("read changed data"),
            b"new score".to_vec()
        );
        assert_eq!(
            fs::read(working.join("SCORE.DAT")).expect("read new data"),
            b"scoreboard".to_vec()
        );
        assert_eq!(
            fs::read(working.join("data").join("HOF.DAT")).expect("read nested data"),
            b"hall".to_vec()
        );
        assert!(!working.join("DORINFO1.DEF").exists());
        assert!(!working.join("OXNODE.TXT").exists());
        let _ = fs::remove_dir_all(&base);
    }

    fn spawn_fake_provider_server(response: &'static [u8]) -> (String, JoinHandle<Vec<u8>>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind fake provider");
        let addr = listener.local_addr().expect("fake provider addr");
        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept provider client");
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .expect("set read timeout");
            let mut received = Vec::new();
            let mut buffer = [0_u8; 512];
            loop {
                match stream.read(&mut buffer) {
                    Ok(0) => break,
                    Ok(count) => {
                        received.extend_from_slice(&buffer[..count]);
                        if String::from_utf8_lossy(&received).contains("USER Alice SEC 50\r\n") {
                            break;
                        }
                    }
                    Err(error)
                        if error.kind() == std::io::ErrorKind::WouldBlock
                            || error.kind() == std::io::ErrorKind::TimedOut =>
                    {
                        break;
                    }
                    Err(error) => panic!("fake provider read failed: {error}"),
                }
            }
            stream.write_all(response).expect("write fake response");
            stream.flush().expect("flush fake response");
            let _ = stream.shutdown(Shutdown::Write);
            received
        });
        (format!("tcp://{addr}"), handle)
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
    fn renders_door_sys_with_exact_bytes() {
        let contents = render_door_sys(&caller(), 1, 38_400);

        assert_eq!(
            contents,
            concat!(
                "COM1:\r\n",
                "38400\r\n",
                "8\r\n",
                "1\r\n",
                "30\r\n",
                "Alice\r\n",
                "Alice Sysop\r\n",
                "Localhost\r\n",
                "50\r\n"
            )
        );
    }

    #[test]
    fn renders_dorinfo1_def_with_exact_bytes() {
        let contents = render_dorinfo1_def("Oxide", "Sysop", &caller());

        assert_eq!(
            contents,
            concat!(
                "Oxide\r\n",
                "Sysop\r\n",
                "\r\n",
                "COM1\r\n",
                "38400 BAUD,N,8,1\r\n",
                "0\r\n",
                "Alice\r\n",
                "Sysop\r\n",
                "Localhost\r\n",
                "1\r\n",
                "50\r\n",
                "30\r\n"
            )
        );
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
        assert!(drop_file.starts_with("Test Board\r\nTest\r\nSysop\r\n"));
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

    #[test]
    fn prepare_door_run_writes_each_supported_drop_file_with_exact_bytes() {
        let base = temp_path("runtime-supported-drop-files");
        let _ = cleanup_node_runtime_dir(&base);
        let runtime_dir = prepare_node_runtime_dir(&base, 3).expect("runtime");
        let working_dir = base.join("door-files");
        write_command_fixture(&working_dir, "LORD.EXE");
        let expected_drop_files = [
            (
                "DOOR.SYS",
                concat!(
                    "COM1:\r\n",
                    "38400\r\n",
                    "8\r\n",
                    "3\r\n",
                    "30\r\n",
                    "Alice\r\n",
                    "Alice Sysop\r\n",
                    "Localhost\r\n",
                    "50\r\n"
                ),
            ),
            (
                "DORINFO1.DEF",
                concat!(
                    "Test Board\r\n",
                    "Test\r\n",
                    "Sysop\r\n",
                    "COM1\r\n",
                    "38400 BAUD,N,8,1\r\n",
                    "0\r\n",
                    "Alice\r\n",
                    "Sysop\r\n",
                    "Localhost\r\n",
                    "1\r\n",
                    "50\r\n",
                    "30\r\n"
                ),
            ),
            ("CHAIN.TXT", "3 38400 COM1\r\n"),
            (
                "DOORFILE.SR",
                concat!(
                    "Test Board\r\n",
                    "Test Sysop\r\n",
                    "COM1\r\n",
                    "38400\r\n",
                    "8N1\r\n",
                    "3\r\n",
                    "Alice\r\n",
                    "Alice Sysop\r\n",
                    "Localhost\r\n",
                    "50\r\n",
                    "30\r\n",
                    "N\r\n",
                    "0\r\n"
                ),
            ),
            (
                "PCBOARD.SYS",
                concat!(
                    "Test Board\r\n",
                    "Alice\r\n",
                    "Alice Sysop\r\n",
                    "COM1\r\n",
                    "38400\r\n",
                    "8N1\r\n",
                    "3\r\n",
                    "50\r\n",
                    "30\r\n"
                ),
            ),
            (
                "CALLINFO.BBS",
                concat!(
                    "Alice\r\n",
                    "3\r\n",
                    "38400\r\n",
                    "COM1\r\n",
                    "Y\r\n",
                    "Localhost\r\n"
                ),
            ),
        ];

        for (drop_file, expected_contents) in expected_drop_files {
            let request = DoorRunRequest {
                door: door_with_working_dir(drop_file, &working_dir),
                caller: caller(),
                board_name: "Test Board".to_string(),
                sysop_name: "Test Sysop".to_string(),
                node_number: 3,
                runtime_dir: runtime_dir.clone(),
            };

            prepare_door_run(&request).expect("prepare door run");
            let actual_contents = fs::read(runtime_dir.join(drop_file)).expect("drop file");

            assert_eq!(
                actual_contents.as_slice(),
                expected_contents.as_bytes(),
                "drop file {drop_file}"
            );
        }

        cleanup_node_runtime_dir(&base).expect("cleanup");
    }

    #[test]
    fn renders_chain_txt_with_exact_bytes() {
        let contents = render_chain_txt(&caller(), 3, 38_400);
        assert_eq!(contents, "3 38400 COM1\r\n");
    }

    #[test]
    fn renders_doorfile_sr_with_exact_bytes() {
        let contents = render_doorfile_sr(&caller(), 1, 38_400, "Oxide", "Sysop");
        assert_eq!(
            contents,
            concat!(
                "Oxide\r\n",
                "Sysop\r\n",
                "COM1\r\n",
                "38400\r\n",
                "8N1\r\n",
                "1\r\n",
                "Alice\r\n",
                "Alice Sysop\r\n",
                "Localhost\r\n",
                "50\r\n",
                "30\r\n",
                "N\r\n",
                "0\r\n"
            )
        );
    }

    #[test]
    fn renders_pcboard_sys_with_exact_bytes() {
        let contents = render_pcboard_sys(&caller(), 2, 57_600, "Oxide");
        assert_eq!(
            contents,
            concat!(
                "Oxide\r\n",
                "Alice\r\n",
                "Alice Sysop\r\n",
                "COM1\r\n",
                "57600\r\n",
                "8N1\r\n",
                "2\r\n",
                "50\r\n",
                "30\r\n"
            )
        );
    }

    #[test]
    fn renders_callinfo_bbs_with_exact_bytes() {
        let contents = render_callinfo_bbs(&caller(), 1, 38_400);
        assert_eq!(
            contents,
            concat!(
                "Alice\r\n",
                "1\r\n",
                "38400\r\n",
                "COM1\r\n",
                "Y\r\n",
                "Localhost\r\n"
            )
        );
    }

    #[test]
    fn bbslink_provider_dry_run_validates_config_without_network() {
        let provider = BbsLinkProvider::new(BbsLinkConfig::new(
            "oxide-system",
            "bbslink-auth-code",
            "bbslink.example:23",
        ));

        let result = provider.dry_run_session(&caller()).expect("dry run");

        assert_eq!(
            result,
            DoorRunResult {
                exit_code: Some(0),
                timed_out: false,
            }
        );
    }

    #[test]
    fn doorparty_provider_dry_run_validates_config_without_network() {
        let provider_password = remote_provider_secret();
        let provider = DoorPartyProvider::new(DoorPartyConfig::new(
            "oxide-account",
            provider_password,
            "telnet://doorparty.example:23",
        ));

        let result = provider.dry_run_session(&caller()).expect("dry run");

        assert_eq!(
            result,
            DoorRunResult {
                exit_code: Some(0),
                timed_out: false,
            }
        );
    }

    #[test]
    fn remote_provider_validation_rejects_missing_required_fields() {
        let missing_bbslink_auth =
            BbsLinkProvider::new(BbsLinkConfig::new("oxide-system", "", "bbslink.example:23"));
        let missing_doorparty_endpoint = DoorPartyProvider::new(DoorPartyConfig::new(
            "oxide-account",
            remote_provider_secret(),
            " ",
        ));

        let bbslink_error = missing_bbslink_auth
            .validate_config()
            .expect_err("missing auth should fail");
        let doorparty_error = missing_doorparty_endpoint
            .validate_config()
            .expect_err("missing endpoint should fail");

        match bbslink_error {
            DoorError::InvalidConfig(message) => assert!(message.contains("auth_code")),
            _ => panic!("unexpected error variant"),
        }
        match doorparty_error {
            DoorError::InvalidConfig(message) => assert!(message.contains("endpoint")),
            _ => panic!("unexpected error variant"),
        }
    }

    #[test]
    fn remote_provider_secrets_are_redacted_by_default() {
        let provider_password = remote_provider_secret();
        let provider = DoorPartyProvider::new(DoorPartyConfig::new(
            "oxide-account",
            provider_password.clone(),
            "doorparty.example:23",
        ));

        let config_debug = format!("{:?}", provider.config());
        let redacted = provider.redacted_config();

        assert_eq!(
            provider.config().password.expose_secret(),
            provider_password.as_str()
        );
        assert_eq!(
            provider.config().password.to_string(),
            REDACTED_PROVIDER_SECRET
        );
        assert!(!config_debug.contains(&provider_password));
        assert!(config_debug.contains(REDACTED_PROVIDER_SECRET));
        assert_eq!(redacted.provider_key, DOORPARTY_PROVIDER_KEY);
        assert_eq!(redacted.secret, REDACTED_PROVIDER_SECRET);
    }

    #[test]
    fn provider_registry_registers_and_retrieves_providers() {
        struct StubProvider;

        impl RemoteDoorProvider for StubProvider {
            fn validate_config(&self) -> Result<(), DoorError> {
                Ok(())
            }

            fn dry_run_session(&self, _caller: &DoorCaller) -> Result<DoorRunResult, DoorError> {
                Ok(DoorRunResult {
                    exit_code: Some(0),
                    timed_out: false,
                })
            }

            fn launch_session(
                &self,
                _caller: &DoorCaller,
                _io: &mut dyn RemoteSessionIo,
            ) -> Result<DoorRunResult, DoorError> {
                Ok(DoorRunResult {
                    exit_code: Some(0),
                    timed_out: false,
                })
            }
        }

        let mut registry = ProviderRegistry::new();
        registry.register("stub", Box::new(StubProvider));

        assert!(registry.get("stub").is_some());
        assert!(registry.get("missing").is_none());
        assert_eq!(registry.names(), vec!["stub"]);
    }

    #[test]
    fn prepare_door_run_writes_chain_txt() {
        let base = temp_path("runtime-chain");
        let runtime_dir = prepare_node_runtime_dir(&base, 1).expect("runtime");
        let working_dir = base.join("door-files");
        write_command_fixture(&working_dir, "LORD.EXE");
        let request = DoorRunRequest {
            door: door_with_working_dir("CHAIN.TXT", &working_dir),
            caller: caller(),
            board_name: "Test Board".to_string(),
            sysop_name: "Test Sysop".to_string(),
            node_number: 3,
            runtime_dir: runtime_dir.clone(),
        };

        DryRunDoorRunner.run(&request).expect("dry run");

        let drop_file = fs::read_to_string(runtime_dir.join("CHAIN.TXT")).expect("drop file");
        assert!(drop_file.starts_with("3 "));
        assert!(drop_file.contains("COM1\r\n"));

        cleanup_node_runtime_dir(&base).expect("cleanup");
    }

    struct FakeRemoteSessionIo {
        remote_input: Vec<u8>,
        remote_output: Vec<u8>,
        caller_input: Vec<u8>,
        caller_output: Vec<u8>,
        remote_pos: usize,
        caller_pos: usize,
    }

    impl FakeRemoteSessionIo {
        fn new(remote_responses: &[u8], caller_inputs: &[u8]) -> Self {
            Self {
                remote_input: remote_responses.to_vec(),
                remote_output: Vec::new(),
                caller_input: caller_inputs.to_vec(),
                caller_output: Vec::new(),
                remote_pos: 0,
                caller_pos: 0,
            }
        }

        fn remote_received(&self) -> &[u8] {
            &self.remote_output
        }

        fn caller_received(&self) -> &[u8] {
            &self.caller_output
        }
    }

    impl RemoteSessionIo for FakeRemoteSessionIo {
        fn read_byte(&mut self) -> std::io::Result<Option<u8>> {
            self.read_remote_byte()
        }

        fn write_all(&mut self, bytes: &[u8]) -> std::io::Result<()> {
            self.write_remote(bytes)
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }

        fn has_remote_data(&self) -> bool {
            self.remote_pos < self.remote_input.len()
        }

        fn has_caller_data(&self) -> bool {
            self.caller_pos < self.caller_input.len()
        }

        fn is_remote_closed(&self) -> bool {
            self.remote_pos >= self.remote_input.len()
        }

        fn read_caller_byte(&mut self) -> std::io::Result<Option<u8>> {
            if self.caller_pos < self.caller_input.len() {
                let byte = self.caller_input[self.caller_pos];
                self.caller_pos += 1;
                Ok(Some(byte))
            } else {
                Ok(None)
            }
        }

        fn write_remote(&mut self, bytes: &[u8]) -> std::io::Result<()> {
            self.remote_output.extend_from_slice(bytes);
            Ok(())
        }

        fn read_remote_byte(&mut self) -> std::io::Result<Option<u8>> {
            if self.remote_pos < self.remote_input.len() {
                let byte = self.remote_input[self.remote_pos];
                self.remote_pos += 1;
                Ok(Some(byte))
            } else {
                Ok(None)
            }
        }

        fn write_caller(&mut self, bytes: &[u8]) -> std::io::Result<()> {
            self.caller_output.extend_from_slice(bytes);
            Ok(())
        }
    }

    #[test]
    fn bbslink_provider_launch_session_sends_auth_and_user() {
        let provider = BbsLinkProvider::new(BbsLinkConfig::new(
            "oxide-system",
            "bbslink-auth-code",
            "bbslink.example:23",
        ));

        let remote_responses = b"OK\r\nWelcome to Door\r\n";
        let caller_inputs = b"";
        let mut io = FakeRemoteSessionIo::new(remote_responses, caller_inputs);

        let result = provider.launch_session(&caller(), &mut io).expect("launch");

        assert_eq!(result.exit_code, Some(0));
        assert!(!result.timed_out);

        let sent = String::from_utf8_lossy(io.remote_received());
        assert!(sent.contains("SYS oxide-system AUTH bbslink-auth-code"));
        assert!(sent.contains("USER Alice SEC 50"));
    }

    #[test]
    fn doorparty_provider_launch_session_sends_auth_and_user() {
        let provider_password = remote_provider_secret();
        let provider = DoorPartyProvider::new(DoorPartyConfig::new(
            "oxide-account",
            provider_password.clone(),
            "doorparty.example:23",
        ));

        let remote_responses = b"OK\r\nWelcome to Door\r\n";
        let caller_inputs = b"";
        let mut io = FakeRemoteSessionIo::new(remote_responses, caller_inputs);

        let result = provider.launch_session(&caller(), &mut io).expect("launch");

        assert_eq!(result.exit_code, Some(0));
        assert!(!result.timed_out);

        let sent = String::from_utf8_lossy(io.remote_received());
        assert!(sent.contains(&format!("ACCT oxide-account PASS {provider_password}")));
        assert!(sent.contains("USER Alice SEC 50"));
    }

    #[test]
    fn bbslink_provider_launch_session_bridges_remote_to_caller() {
        let provider = BbsLinkProvider::new(BbsLinkConfig::new(
            "oxide-system",
            "bbslink-auth-code",
            "bbslink.example:23",
        ));

        let remote_responses = b"OK\r\nHello from door\r\n";
        let caller_inputs = b"";
        let mut io = FakeRemoteSessionIo::new(remote_responses, caller_inputs);

        let result = provider.launch_session(&caller(), &mut io).expect("launch");

        assert_eq!(result.exit_code, Some(0));
        let caller_received = String::from_utf8_lossy(io.caller_received());
        assert!(caller_received.contains("OK"));
        assert!(caller_received.contains("Hello from door"));
    }

    #[test]
    fn doorparty_provider_launch_session_rejects_invalid_config() {
        let provider = DoorPartyProvider::new(DoorPartyConfig::new(
            "",
            remote_provider_secret(),
            "doorparty.example:23",
        ));

        let mut io = FakeRemoteSessionIo::new(b"", b"");
        let result = provider.launch_session(&caller(), &mut io);

        assert!(result.is_err());
    }

    #[test]
    fn bbslink_provider_live_connector_uses_local_tcp_server() {
        let (endpoint, server) = spawn_fake_provider_server(b"OK\r\nWelcome BBSLink\r\n");
        let provider = BbsLinkProvider::new(BbsLinkConfig::new(
            "oxide-system",
            "bbslink-auth-code",
            endpoint,
        ));

        let result = provider
            .launch_tcp_session(&caller(), b"")
            .expect("launch tcp session");
        let received = server.join().expect("server finished");
        let received = String::from_utf8_lossy(&received);

        assert_eq!(result.run_result.exit_code, Some(0));
        assert!(!result.run_result.timed_out);
        assert!(received.contains("SYS oxide-system AUTH bbslink-auth-code"));
        assert!(received.contains("USER Alice SEC 50"));
        assert_eq!(result.caller_output, b"OK\r\nWelcome BBSLink\r\n");
    }

    #[test]
    fn doorparty_provider_live_connector_uses_local_tcp_server() {
        let (endpoint, server) = spawn_fake_provider_server(b"OK\r\nWelcome DoorParty\r\n");
        let provider_password = remote_provider_secret();
        let provider = DoorPartyProvider::new(DoorPartyConfig::new(
            "oxide-account",
            provider_password.clone(),
            endpoint,
        ));

        let result = provider
            .launch_tcp_session(&caller(), b"")
            .expect("launch tcp session");
        let received = server.join().expect("server finished");
        let received = String::from_utf8_lossy(&received);

        assert_eq!(result.run_result.exit_code, Some(0));
        assert!(!result.run_result.timed_out);
        assert!(received.contains(&format!("ACCT oxide-account PASS {provider_password}")));
        assert!(received.contains("USER Alice SEC 50"));
        assert_eq!(result.caller_output, b"OK\r\nWelcome DoorParty\r\n");
    }

    #[test]
    fn remote_endpoint_socket_target_accepts_supported_endpoint_forms() {
        assert_eq!(
            remote_endpoint_socket_target("door.example").expect("host"),
            "door.example:23"
        );
        assert_eq!(
            remote_endpoint_socket_target("door.example:2323").expect("host port"),
            "door.example:2323"
        );
        assert_eq!(
            remote_endpoint_socket_target("telnet://door.example:2323").expect("telnet"),
            "door.example:2323"
        );

        let error =
            remote_endpoint_socket_target("https://door.example/login").expect_err("bad scheme");
        match error {
            DoorError::InvalidConfig(message) => {
                assert!(message.contains("scheme"));
            }
            _ => panic!("unexpected error variant"),
        }
    }
}
