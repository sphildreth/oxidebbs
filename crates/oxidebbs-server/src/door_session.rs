#![cfg_attr(
    not(unix),
    allow(dead_code, unreachable_code, unused_imports, unused_variables)
)]

use std::fs;
#[cfg(unix)]
use std::io;
#[cfg(unix)]
use std::os::unix::fs::MetadataExt;
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;
use std::path::PathBuf;
#[cfg(unix)]
use std::pin::Pin;
use std::process::Stdio;
#[cfg(unix)]
use std::task::{Context, Poll, ready};
use std::time::Duration;

#[cfg(unix)]
use nix::fcntl::{FcntlArg, OFlag, fcntl};
#[cfg(unix)]
use nix::sys::termios::{SetArg, cfmakeraw, tcgetattr, tcsetattr};
#[cfg(unix)]
use nix::unistd::geteuid;
use oxidebbs_core::door::DoorDefinition;
use oxidebbs_core::user::User;
use oxidebbs_db::{
    AuditEventRecord, DoorDefinitionRecord, DoorRunFinish, DoorRunRecord, OxideDb,
    find_door_by_key, finish_door_run, insert_audit_event, insert_door_definition, insert_door_run,
    list_door_definitions, update_door_definition,
};
#[cfg(test)]
use oxidebbs_door::DoorRunner;
use oxidebbs_door::{
    DoorCaller, DoorRunPlan, DoorRunRequest, cleanup_node_runtime_dir, node_runtime_dir,
    prepare_door_run, prepare_node_runtime_dir, runner_supports_dosemu2_cli,
};
use oxidebbs_telnet::Transport;
use oxidebbs_term::encode_cp437;
#[cfg(unix)]
use tokio::io::unix::AsyncFd;
#[cfg(unix)]
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadBuf};
#[cfg(unix)]
use tokio::process::{Child, Command};
#[cfg(unix)]
use tokio::time::{self, Instant as TokioInstant};
use tracing::{info, warn};

use crate::config::{DoorDefConfig, OxideConfig};
use crate::control::ServerRuntime;
use crate::serve::{ServeError, ServeResult};

#[cfg(unix)]
const DOOR_BRIDGE_POLL: Duration = Duration::from_millis(250);
#[cfg(unix)]
const DOOR_KILL_WAIT: Duration = Duration::from_secs(2);
#[cfg(unix)]
const DOOR_EXIT_WAIT: Duration = Duration::from_secs(5);
const DOSEMU2_CONFIG_FILE: &str = "OXDOSEMU2.CONF";
const DOSEMU2_COM1_PTY: &str = "OXCOM1.PTY";
const DOOR_LOG_DIR: &str = "doors";

pub(crate) struct DoorService<'a> {
    db: &'a OxideDb,
    config: &'a OxideConfig,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DoorExecutionSummary {
    pub door_name: String,
    pub run_id: Option<String>,
    pub exit_code: Option<i32>,
    pub timed_out: bool,
    pub disconnect_forced: bool,
    pub caller_disconnected: bool,
    pub disconnect_reason: Option<String>,
    pub early_exit_before_com1: bool,
    pub bytes_in: i64,
    pub bytes_out: i64,
    pub launch_error: Option<String>,
    pub stdout_log: Option<std::path::PathBuf>,
    pub stderr_log: Option<std::path::PathBuf>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(crate) struct DoorBridgeResult {
    pub exit_code: Option<i32>,
    pub timed_out: bool,
    pub disconnect_forced: bool,
    pub caller_disconnected: bool,
    pub disconnect_reason: Option<String>,
    pub early_exit_before_com1: bool,
    pub com1_connected: bool,
    pub bytes_in: i64,
    pub bytes_out: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum DoorSelection<'a> {
    Return,
    Door(&'a DoorDefinitionRecord),
    Invalid,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Dosemu2ComBridge {
    pty_path: std::path::PathBuf,
    config_path: std::path::PathBuf,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct DoorRunnerLogs {
    stdout: Option<std::path::PathBuf>,
    stderr: Option<std::path::PathBuf>,
    capture_errors: Vec<String>,
}

#[derive(Debug, Clone, Copy, Default)]
struct DoorFinishOptions<'a> {
    runtime: Option<&'a ServerRuntime>,
    note: Option<&'a str>,
}

struct DoorRuntimeDirectoryGuard {
    path: PathBuf,
    disarmed: bool,
}

impl DoorRuntimeDirectoryGuard {
    fn armed(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            disarmed: false,
        }
    }

    fn prepare(runtime_root: &Path, node_number: u16) -> ServeResult<Self> {
        let path = prepare_node_runtime_dir(runtime_root, node_number).map_err(door_error)?;
        Ok(Self::armed(path))
    }

    #[cfg(test)]
    fn disarm(&mut self) {
        self.disarmed = true;
    }
}

impl Drop for DoorRuntimeDirectoryGuard {
    fn drop(&mut self) {
        if self.disarmed {
            return;
        }
        if let Err(error) = cleanup_node_runtime_dir(&self.path) {
            warn!(
                path = %self.path.display(),
                "failed to clean door runtime directory: {error}"
            );
        }
    }
}

#[cfg(unix)]
struct AsyncPty {
    inner: AsyncFd<fs::File>,
}

#[cfg(unix)]
impl AsyncPty {
    fn new(file: fs::File) -> io::Result<Self> {
        Ok(Self {
            inner: AsyncFd::new(file)?,
        })
    }
}

#[cfg(unix)]
impl AsyncRead for AsyncPty {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        if buf.remaining() == 0 {
            return Poll::Ready(Ok(()));
        }

        loop {
            let mut guard = ready!(self.inner.poll_read_ready(cx))?;
            let read = guard.try_io(|inner| {
                nix::unistd::read(inner.get_ref(), buf.initialize_unfilled())
                    .map_err(errno_to_io_error)
            });
            match read {
                Ok(Ok(count)) => {
                    buf.advance(count);
                    return Poll::Ready(Ok(()));
                }
                Ok(Err(error)) => return Poll::Ready(Err(error)),
                Err(_would_block) => continue,
            }
        }
    }
}

#[cfg(unix)]
impl AsyncWrite for AsyncPty {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        loop {
            let mut guard = ready!(self.inner.poll_write_ready(cx))?;
            let write = guard.try_io(|inner| {
                nix::unistd::write(inner.get_ref(), buf).map_err(errno_to_io_error)
            });
            match write {
                Ok(result) => return Poll::Ready(result),
                Err(_would_block) => continue,
            }
        }
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }
}

impl<'a> DoorService<'a> {
    pub(crate) fn new(db: &'a OxideDb, config: &'a OxideConfig) -> Self {
        Self { db, config }
    }

    pub(crate) fn list_enabled_doors(&self) -> ServeResult<Vec<DoorDefinitionRecord>> {
        self.sync_configured_doors()?;
        if !self.config.doors.enabled {
            return Ok(Vec::new());
        }

        let mut doors = list_door_definitions(self.db.db())
            .map_err(ServeError::Database)?
            .into_iter()
            .filter(|door| door.enabled)
            .collect::<Vec<_>>();
        doors.sort_by(|left, right| left.key.cmp(&right.key));
        Ok(doors)
    }

    pub(crate) fn validate_door(
        &self,
        door: &DoorDefinitionRecord,
        _node_number: u16,
    ) -> Result<(), String> {
        if !self.config.doors.enabled {
            return Err("Doors are disabled for this board.".to_string());
        }
        if !door.enabled {
            return Err(format!("Door {} is disabled.", door.key));
        }
        if !(1..=240).contains(&door.time_limit_minutes) {
            return Err(format!(
                "Door {} time limit must be between 1 and 240 minutes.",
                door.key
            ));
        }
        #[cfg(not(unix))]
        {
            return Err(format!(
                "Door {} cannot be launched natively on this platform yet. Live DOS doors require Linux or a Docker deployment with DOSEMU2.",
                door.key
            ));
        }

        let doors_root = self.config.paths.doors.canonicalize().map_err(|error| {
            format!(
                "Door {} could not canonicalize doors path {}: {error}",
                door.key,
                self.config.paths.doors.display()
            )
        })?;
        let working_dir = Path::new(&door.working_dir)
            .canonicalize()
            .map_err(|error| {
                format!(
                    "Door {} working directory {} does not exist: {error}",
                    door.key, door.working_dir
                )
            })?;
        if !working_dir.starts_with(&doors_root) {
            return Err(format!(
                "Door {} working directory {} is outside paths.doors {}",
                door.key,
                working_dir.display(),
                self.config.paths.doors.display()
            ));
        }
        if !working_dir.is_dir() {
            return Err(format!(
                "Door {} working directory {} does not exist.",
                door.key,
                working_dir.display()
            ));
        }

        if !runner_supports_dosemu2_cli(&door.runner) {
            return Err(format!(
                "Door {} runner {:?} is not supported for live caller doors. The current bridge requires DOSEMU2; use runner = \"dosemu\".",
                door.key, door.runner
            ));
        }
        validate_door_runner(&door.runner, &self.config.doors.allowed_runners)?;
        if !command_exists(&door.runner) {
            return Err(format!(
                "Door {} runner {:?} was not found.",
                door.key, door.runner
            ));
        }

        if !supported_drop_file(&door.drop_file) {
            return Err(format!(
                "Door {} drop-file format {:?} is not supported.",
                door.key, door.drop_file
            ));
        }

        let command = first_command_token(&door.command)
            .ok_or_else(|| format!("Door {} command is empty.", door.key))?;
        if is_quoted_dos_command(command) {
            return Err(format!(
                "Door {} uses a quoted DOS command, which is not supported yet. Use DOS 8.3 paths instead.",
                door.key
            ));
        }
        let command_path =
            if command.contains(':') || command.contains('\\') || command.contains('/') {
                Path::new(command).to_path_buf()
            } else {
                working_dir.join(command)
            };
        if !command_path.is_file() {
            return Err(format!(
                "Door {} command {} was not found.",
                door.key, command
            ));
        }

        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn execute_with_runner<R: DoorRunner>(
        &self,
        runner: &R,
        user: &User,
        node_number: u16,
        door: &DoorDefinitionRecord,
        preserve_runtime: bool,
    ) -> ServeResult<DoorExecutionSummary> {
        self.validate_door(door, node_number)
            .map_err(ServeError::Runtime)?;
        let mut runtime_guard =
            DoorRuntimeDirectoryGuard::prepare(&self.config.paths.runtime, node_number)?;
        let request = self.build_request(user, node_number, door)?;
        let plan = prepare_door_run(&request).map_err(door_error)?;
        let run_id = generated_uuid(self.db)?;
        let run_id = self.insert_started_run(
            &run_id,
            door,
            user,
            node_number,
            None,
            format!(
                "door run {run_id} started for {}; mode=dry_run program={} args={:?} working_dir={} runtime_dir={} drop_file={}",
                door.key,
                plan.program,
                plan.args,
                plan.working_dir.display(),
                request.runtime_dir.display(),
                plan.drop_file_path.display()
            ),
        )?;
        let result = runner.run(&request).map_err(door_error)?;
        self.finish_run(
            &run_id,
            door,
            user,
            node_number,
            DoorBridgeResult {
                exit_code: result.exit_code,
                timed_out: result.timed_out,
                disconnect_forced: result.timed_out,
                ..DoorBridgeResult::default()
            },
            DoorFinishOptions::default(),
        )?;
        if preserve_runtime {
            runtime_guard.disarm();
        }

        Ok(DoorExecutionSummary {
            door_name: door.name.clone(),
            run_id: Some(run_id),
            exit_code: result.exit_code,
            timed_out: result.timed_out,
            disconnect_forced: result.timed_out,
            caller_disconnected: false,
            disconnect_reason: None,
            early_exit_before_com1: false,
            bytes_in: 0,
            bytes_out: 0,
            launch_error: None,
            stdout_log: None,
            stderr_log: None,
        })
    }

    #[cfg(unix)]
    pub(crate) async fn execute_interactive<T: Transport>(
        &self,
        transport: &mut T,
        runtime: &ServerRuntime,
        user: &User,
        node_number: u16,
        door: &DoorDefinitionRecord,
    ) -> ServeResult<DoorExecutionSummary> {
        self.validate_door(door, node_number)
            .map_err(ServeError::Runtime)?;
        let _runtime_guard =
            DoorRuntimeDirectoryGuard::prepare(&self.config.paths.runtime, node_number)?;
        let request = self.build_request(user, node_number, door)?;
        let mut plan = prepare_door_run(&request).map_err(door_error)?;
        let bridge_runtime_dir = plan.working_dir.clone();
        let com_bridge = prepare_dosemu2_com1_bridge(&mut plan, &bridge_runtime_dir)?;
        let run_id = generated_uuid(self.db)?;
        let (stdout, stderr, runner_logs) =
            door_runner_stdio(&self.config.paths.logs, &run_id, &door.key);
        self.insert_started_run(
            &run_id,
            door,
            user,
            node_number,
            Some(runtime),
            door_start_details(&run_id, &request, &plan, &com_bridge, &runner_logs),
        )?;

        let mut command = Command::new(&plan.program);
        command
            .args(&plan.args)
            .current_dir(&plan.working_dir)
            .stdin(Stdio::piped())
            .stdout(stdout)
            .stderr(stderr);

        info!(
            door = %door.key,
            run_id = %run_id,
            node = %node_number,
            program = %plan.program,
            args = ?plan.args,
            working_dir = %plan.working_dir.display(),
            runtime_dir = %request.runtime_dir.display(),
            pty = %com_bridge.pty_path.display(),
            stdout_log = ?runner_logs.stdout,
            stderr_log = ?runner_logs.stderr,
            "launching door runner"
        );

        if let Err(error) = validate_door_runner(&door.runner, &self.config.doors.allowed_runners) {
            let message = format!("door runner validation failed before launch: {error}");
            self.finish_run(
                &run_id,
                door,
                user,
                node_number,
                DoorBridgeResult::default(),
                DoorFinishOptions {
                    runtime: Some(runtime),
                    note: Some(&message),
                },
            )?;
            return Ok(DoorExecutionSummary {
                door_name: door.name.clone(),
                run_id: Some(run_id),
                exit_code: None,
                timed_out: false,
                disconnect_forced: false,
                caller_disconnected: false,
                disconnect_reason: None,
                early_exit_before_com1: false,
                bytes_in: 0,
                bytes_out: 0,
                launch_error: Some(message),
                stdout_log: runner_logs.stdout,
                stderr_log: runner_logs.stderr,
            });
        }

        let child = match command.spawn() {
            Ok(child) => child,
            Err(error) => {
                let message = format!("failed to launch door runner {:?}: {error}", plan.program);
                let bridge = DoorBridgeResult::default();
                self.finish_run(
                    &run_id,
                    door,
                    user,
                    node_number,
                    bridge,
                    DoorFinishOptions {
                        runtime: Some(runtime),
                        note: Some(&message),
                    },
                )?;
                return Ok(DoorExecutionSummary {
                    door_name: door.name.clone(),
                    run_id: Some(run_id),
                    exit_code: None,
                    timed_out: false,
                    disconnect_forced: false,
                    caller_disconnected: false,
                    disconnect_reason: None,
                    early_exit_before_com1: false,
                    bytes_in: 0,
                    bytes_out: 0,
                    launch_error: Some(message),
                    stdout_log: runner_logs.stdout,
                    stderr_log: runner_logs.stderr,
                });
            }
        };

        runtime.mark_node_in_door(node_number);
        let bridge = run_door_bridge(
            transport,
            runtime,
            node_number,
            child,
            com_bridge,
            plan.timeout,
        )
        .await;
        runtime.heartbeat_node(node_number);
        let bridge = match bridge {
            Ok(bridge) => bridge,
            Err(error) => {
                let forced = DoorBridgeResult {
                    disconnect_forced: true,
                    ..DoorBridgeResult::default()
                };
                if let Err(finish_error) = self.finish_run(
                    &run_id,
                    door,
                    user,
                    node_number,
                    forced,
                    DoorFinishOptions {
                        runtime: Some(runtime),
                        note: Some("bridge error"),
                    },
                ) {
                    warn!("failed to finish door run {run_id} after bridge error: {finish_error}");
                }
                return Err(error);
            }
        };

        if bridge.early_exit_before_com1 {
            warn!(
                door = %door.key,
                run_id = %run_id,
                node = %node_number,
                exit_code = ?bridge.exit_code,
                stdout_log = ?runner_logs.stdout,
                stderr_log = ?runner_logs.stderr,
                "door runner exited before DOSEMU2 COM1 bridge was available"
            );
        }

        let finish_note = door_finish_note(&bridge, &runner_logs);
        self.finish_run(
            &run_id,
            door,
            user,
            node_number,
            bridge.clone(),
            DoorFinishOptions {
                runtime: Some(runtime),
                note: Some(&finish_note),
            },
        )?;

        Ok(DoorExecutionSummary {
            door_name: door.name.clone(),
            run_id: Some(run_id),
            exit_code: bridge.exit_code,
            timed_out: bridge.timed_out,
            disconnect_forced: bridge.disconnect_forced,
            caller_disconnected: bridge.caller_disconnected,
            disconnect_reason: bridge.disconnect_reason,
            early_exit_before_com1: bridge.early_exit_before_com1,
            bytes_in: bridge.bytes_in,
            bytes_out: bridge.bytes_out,
            launch_error: None,
            stdout_log: runner_logs.stdout,
            stderr_log: runner_logs.stderr,
        })
    }

    #[cfg(not(unix))]
    pub(crate) async fn execute_interactive<T: Transport>(
        &self,
        _transport: &mut T,
        _runtime: &ServerRuntime,
        _user: &User,
        _node_number: u16,
        door: &DoorDefinitionRecord,
    ) -> ServeResult<DoorExecutionSummary> {
        Ok(DoorExecutionSummary {
            door_name: door.name.clone(),
            run_id: None,
            exit_code: None,
            timed_out: false,
            disconnect_forced: false,
            caller_disconnected: false,
            disconnect_reason: None,
            early_exit_before_com1: false,
            bytes_in: 0,
            bytes_out: 0,
            launch_error: Some(
                "live DOS door execution is not supported on this platform; use Linux or Docker with DOSEMU2".to_string(),
            ),
            stdout_log: None,
            stderr_log: None,
        })
    }

    fn sync_configured_doors(&self) -> ServeResult<()> {
        for door in &self.config.doors.definitions {
            match find_door_by_key(self.db.db(), &door.key).map_err(ServeError::Database)? {
                Some(existing) => {
                    let record = self.record_from_config_with_id(door, existing.id);
                    update_door_definition(self.db.db(), &record).map_err(ServeError::Database)?;
                }
                None => {
                    let record = self.record_from_config(door)?;
                    insert_door_definition(self.db.db(), &record).map_err(ServeError::Database)?;
                }
            }
        }
        Ok(())
    }

    fn build_request(
        &self,
        user: &User,
        node_number: u16,
        door: &DoorDefinitionRecord,
    ) -> ServeResult<DoorRunRequest> {
        Ok(DoorRunRequest {
            door: door_to_core(door)?,
            caller: door_caller(user),
            board_name: self.config.board.name.clone(),
            sysop_name: self.config.board.sysop_name.clone(),
            node_number,
            runtime_dir: node_runtime_dir(&self.config.paths.runtime, node_number),
        })
    }

    fn insert_started_run(
        &self,
        run_id: &str,
        door: &DoorDefinitionRecord,
        user: &User,
        node_number: u16,
        runtime: Option<&ServerRuntime>,
        details: String,
    ) -> ServeResult<String> {
        let started_at = current_timestamp(self.db)?;
        insert_door_run(
            self.db.db(),
            &DoorRunRecord {
                id: run_id.to_string(),
                door_id: door.id.clone(),
                user_id: user.id.clone(),
                node_number: i64::from(node_number),
                started_at: started_at.clone(),
                ended_at: None,
                exit_code: None,
                timed_out: false,
                disconnect_forced: false,
                bytes_in: 0,
                bytes_out: 0,
            },
        )
        .map_err(ServeError::Database)?;

        self.audit_door_event("door_started", door, user, node_number, runtime, details);

        Ok(run_id.to_string())
    }

    fn finish_run(
        &self,
        run_id: &str,
        door: &DoorDefinitionRecord,
        user: &User,
        node_number: u16,
        bridge: DoorBridgeResult,
        options: DoorFinishOptions<'_>,
    ) -> ServeResult<()> {
        let ended_at = current_timestamp(self.db)?;
        finish_door_run(
            self.db.db(),
            run_id,
            &DoorRunFinish {
                ended_at,
                exit_code: bridge.exit_code.map(i64::from),
                timed_out: bridge.timed_out,
                disconnect_forced: bridge.disconnect_forced,
                bytes_in: bridge.bytes_in,
                bytes_out: bridge.bytes_out,
            },
        )
        .map_err(ServeError::Database)?;

        let event_type = if bridge.timed_out {
            "door_timed_out"
        } else {
            "door_finished"
        };
        let mut details = format!(
            "door run {run_id} finished exit={:?} timed_out={} forced={} bytes_in={} bytes_out={}",
            bridge.exit_code,
            bridge.timed_out,
            bridge.disconnect_forced,
            bridge.bytes_in,
            bridge.bytes_out
        );
        if let Some(note) = options.note {
            details.push_str("; ");
            details.push_str(note);
        }
        self.audit_door_event(
            event_type,
            door,
            user,
            node_number,
            options.runtime,
            details,
        );
        Ok(())
    }

    fn audit_door_event(
        &self,
        event_type: &str,
        door: &DoorDefinitionRecord,
        user: &User,
        node_number: u16,
        runtime: Option<&ServerRuntime>,
        details: String,
    ) {
        if let Err(error) = insert_audit_event(
            self.db.db(),
            &AuditEventRecord {
                id: String::new(),
                created_at: String::new(),
                event_type: event_type.to_string(),
                user_id: Some(user.id.clone()),
                node_number: Some(i64::from(node_number)),
                details,
            },
        ) {
            warn!(
                "failed to insert {event_type} audit event for {}: {error}",
                door.key
            );
            if let Some(runtime) = runtime {
                runtime.record_audit_write_failure();
            }
        }
    }

    fn record_from_config(&self, door: &DoorDefConfig) -> ServeResult<DoorDefinitionRecord> {
        Ok(self.record_from_config_with_id(door, generated_uuid(self.db)?))
    }

    fn record_from_config_with_id(&self, door: &DoorDefConfig, id: String) -> DoorDefinitionRecord {
        DoorDefinitionRecord {
            id,
            key: door.key.clone(),
            name: door.name.clone(),
            runner: if door.runner.trim().is_empty() {
                self.config.doors.default_runner.clone()
            } else {
                door.runner.clone()
            },
            working_dir: door.working_dir.clone(),
            command: door.command.clone(),
            drop_file: door.drop_file.clone(),
            exclusive: door.exclusive,
            time_limit_minutes: i64::from(door.time_limit_minutes),
            enabled: self.config.doors.enabled && door.enabled,
            min_security_level: i64::from(door.min_security_level),
        }
    }
}

pub(crate) fn render_door_menu(doors: &[DoorDefinitionRecord]) -> String {
    let mut output = String::from("\r\nAvailable doors:\r\n");
    for (index, door) in doors.iter().enumerate() {
        output.push_str(&format!(
            "  {}) {} ({})\r\n",
            index + 1,
            door.name,
            door.key
        ));
    }
    output
}

pub(crate) fn select_door<'a>(
    doors: &'a [DoorDefinitionRecord],
    selected: &str,
) -> DoorSelection<'a> {
    let selected = selected.trim();
    if selected.is_empty() {
        return DoorSelection::Return;
    }

    if let Ok(index) = selected.parse::<usize>()
        && (1..=doors.len()).contains(&index)
    {
        return DoorSelection::Door(&doors[index - 1]);
    }

    doors
        .iter()
        .find(|door| door.key.eq_ignore_ascii_case(selected))
        .map(DoorSelection::Door)
        .unwrap_or(DoorSelection::Invalid)
}

#[cfg(unix)]
fn prepare_dosemu2_com1_bridge(
    plan: &mut DoorRunPlan,
    runtime_dir: &Path,
) -> ServeResult<Dosemu2ComBridge> {
    let pty_path = runtime_dir.join(DOSEMU2_COM1_PTY);
    if pty_path.exists() {
        fs::remove_file(&pty_path)?;
    }
    let config_path = runtime_dir.join(DOSEMU2_CONFIG_FILE);
    fs::write(&config_path, dosemu2_serial_config(&pty_path))?;
    add_dosemu2_config(plan, &config_path);
    Ok(Dosemu2ComBridge {
        pty_path,
        config_path,
    })
}

#[cfg(unix)]
fn dosemu2_serial_config(pty_path: &Path) -> String {
    format!(
        "$_cpu_vm = \"emulated\"\n$_cpu_vm_dpmi = \"emulated\"\n$_sound = (off)\n$_mouse_internal = (off)\n$_joy_device = \"\"\n$_pktdriver = (off)\n$_tcpdriver = (off)\n$_ttylocks = \"\"\n$_com1 = \"pts {}\"\n",
        escape_dosemu2_config_path(pty_path)
    )
}

#[cfg(unix)]
fn escape_dosemu2_config_path(path: &Path) -> String {
    path.display()
        .to_string()
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
}

#[cfg(unix)]
fn add_dosemu2_config(plan: &mut DoorRunPlan, config_path: &Path) {
    let mut args = Vec::with_capacity(plan.args.len() + 2);
    args.push("-f".to_string());
    args.push(config_path.display().to_string());
    args.append(&mut plan.args);
    plan.args = args;
}

fn door_runner_stdio(
    logs_dir: &Path,
    run_id: &str,
    door_key: &str,
) -> (Stdio, Stdio, DoorRunnerLogs) {
    let mut logs = DoorRunnerLogs::default();
    let door_logs_dir = logs_dir.join(DOOR_LOG_DIR);
    if let Err(error) = fs::create_dir_all(&door_logs_dir) {
        let message = format!(
            "failed to create door log directory {}: {error}",
            door_logs_dir.display()
        );
        warn!("{message}");
        logs.capture_errors.push(message);
        return (Stdio::null(), Stdio::null(), logs);
    }

    let prefix = format!("{}-{run_id}", sanitize_log_component(door_key));
    let stdout_path = door_logs_dir.join(format!("{prefix}.stdout.log"));
    let stderr_path = door_logs_dir.join(format!("{prefix}.stderr.log"));
    let stdout = capture_stdio_file(&stdout_path, "stdout", &mut logs);
    let stderr = capture_stdio_file(&stderr_path, "stderr", &mut logs);
    (stdout, stderr, logs)
}

fn capture_stdio_file(path: &Path, stream: &str, logs: &mut DoorRunnerLogs) -> Stdio {
    let mut options = fs::OpenOptions::new();
    options.create(true).truncate(true).write(true);
    #[cfg(unix)]
    options.mode(0o600);

    match options.open(path) {
        Ok(file) => {
            match stream {
                "stdout" => logs.stdout = Some(path.to_path_buf()),
                "stderr" => logs.stderr = Some(path.to_path_buf()),
                _ => {}
            }
            Stdio::from(file)
        }
        Err(error) => {
            let message = format!(
                "failed to create door {stream} log {}: {error}",
                path.display()
            );
            warn!("{message}");
            logs.capture_errors.push(message);
            Stdio::null()
        }
    }
}

fn sanitize_log_component(value: &str) -> String {
    let sanitized = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                character
            } else {
                '_'
            }
        })
        .collect::<String>();
    if sanitized.is_empty() {
        "door".to_string()
    } else {
        sanitized
    }
}

#[cfg(unix)]
fn door_start_details(
    run_id: &str,
    request: &DoorRunRequest,
    plan: &DoorRunPlan,
    com_bridge: &Dosemu2ComBridge,
    runner_logs: &DoorRunnerLogs,
) -> String {
    let mut details = format!(
        "door run {run_id} started for {}; program={} args={:?} working_dir={} runtime_dir={} drop_file={} dosemu_config={} com1_pty={}",
        request.door.key,
        plan.program,
        plan.args,
        plan.working_dir.display(),
        request.runtime_dir.display(),
        plan.drop_file_path.display(),
        com_bridge.config_path.display(),
        com_bridge.pty_path.display()
    );
    append_runner_log_details(&mut details, runner_logs);
    details
}

fn door_finish_note(bridge: &DoorBridgeResult, runner_logs: &DoorRunnerLogs) -> String {
    let mut details = format!(
        "com1_connected={} early_exit_before_com1={}",
        bridge.com1_connected, bridge.early_exit_before_com1
    );
    append_runner_log_details(&mut details, runner_logs);
    details
}

fn append_runner_log_details(details: &mut String, runner_logs: &DoorRunnerLogs) {
    if let Some(path) = runner_logs.stdout.as_ref() {
        details.push_str("; stdout_log=");
        details.push_str(&path.display().to_string());
    }
    if let Some(path) = runner_logs.stderr.as_ref() {
        details.push_str("; stderr_log=");
        details.push_str(&path.display().to_string());
    }
    if !runner_logs.capture_errors.is_empty() {
        details.push_str("; log_capture_errors=");
        details.push_str(&format!("{:?}", runner_logs.capture_errors));
    }
}

#[cfg(unix)]
pub(crate) async fn run_door_bridge<T: Transport>(
    transport: &mut T,
    runtime: &ServerRuntime,
    node_number: u16,
    mut child: Child,
    com_bridge: Dosemu2ComBridge,
    timeout: Duration,
) -> ServeResult<DoorBridgeResult> {
    let deadline = TokioInstant::now() + timeout;
    let mut result = DoorBridgeResult::default();
    let serial = match wait_for_com1_pty(
        transport,
        runtime,
        node_number,
        &mut child,
        &com_bridge,
        deadline,
        &mut result,
    )
    .await
    {
        Ok(serial) => serial,
        Err(error) => {
            if let Err(kill_error) = terminate_child(&mut child).await {
                warn!("failed to terminate door child after bridge startup error: {kill_error}");
            }
            return Err(error);
        }
    };
    let Some(serial) = serial else {
        return Ok(result);
    };

    if let Err(error) = bridge_connected_serial(
        transport,
        runtime,
        node_number,
        &mut child,
        serial,
        deadline,
        &mut result,
    )
    .await
    {
        if let Err(kill_error) = terminate_child(&mut child).await {
            warn!("failed to terminate door child after bridge I/O error: {kill_error}");
        }
        return Err(error);
    }

    Ok(result)
}

#[cfg(unix)]
async fn wait_for_com1_pty<T: Transport>(
    transport: &mut T,
    runtime: &ServerRuntime,
    node_number: u16,
    child: &mut Child,
    com_bridge: &Dosemu2ComBridge,
    deadline: TokioInstant,
    result: &mut DoorBridgeResult,
) -> ServeResult<Option<AsyncPty>> {
    loop {
        if let Some(status) = child.try_wait()? {
            result.exit_code = status.code();
            result.early_exit_before_com1 = true;
            return Ok(None);
        }
        if com_bridge.pty_path.exists() {
            runtime.heartbeat_node(node_number);
            result.com1_connected = true;
            return Ok(Some(open_com1_pty(&com_bridge.pty_path)?));
        }

        let timeout_sleep = time::sleep_until(deadline);
        tokio::pin!(timeout_sleep);
        let poll_sleep = time::sleep(DOOR_BRIDGE_POLL);
        tokio::pin!(poll_sleep);

        tokio::select! {
            _ = &mut timeout_sleep => {
                result.timed_out = true;
                result.disconnect_forced = true;
                result.exit_code = terminate_child(child).await?;
                return Ok(None);
            }
            _ = &mut poll_sleep => {
                runtime.heartbeat_node(node_number);
            }
            commands = runtime.wait_for_node_commands(node_number) => {
                for message in commands.messages {
                    write_bridge_message(transport, &message, result).await?;
                }
                if let Some(reason) = commands.disconnect_reason {
                    write_bridge_message(transport, "Disconnected by sysop.", result).await?;
                    result.disconnect_forced = true;
                    result.disconnect_reason = Some(reason);
                    result.exit_code = terminate_child(child).await?;
                    return Ok(None);
                }
            }
        }
    }
}

#[cfg(unix)]
fn open_com1_pty(path: &Path) -> ServeResult<AsyncPty> {
    let mut options = fs::OpenOptions::new();
    options.read(true).write(true);
    #[cfg(unix)]
    options.custom_flags(OFlag::O_NONBLOCK.bits());
    let file = options.open(path).map_err(|error| {
        ServeError::Runtime(format!(
            "failed to open DOSEMU2 COM1 PTY {}: {error}",
            path.display()
        ))
    })?;
    set_raw_mode(&file)?;
    set_nonblocking(&file)?;
    AsyncPty::new(file).map_err(ServeError::Network)
}

#[cfg(unix)]
fn set_raw_mode(file: &fs::File) -> ServeResult<()> {
    let mut termios = tcgetattr(file).map_err(|error| {
        ServeError::Runtime(format!("failed to read DOSEMU2 COM1 PTY mode: {error}"))
    })?;
    cfmakeraw(&mut termios);
    tcsetattr(file, SetArg::TCSANOW, &termios).map_err(|error| {
        ServeError::Runtime(format!("failed to set DOSEMU2 COM1 PTY raw mode: {error}"))
    })
}

#[cfg(unix)]
fn set_nonblocking(file: &fs::File) -> ServeResult<()> {
    let flags = fcntl(file, FcntlArg::F_GETFL)
        .map_err(|error| ServeError::Runtime(format!("failed to read COM1 PTY flags: {error}")))?;
    let flags = OFlag::from_bits_truncate(flags);
    fcntl(file, FcntlArg::F_SETFL(flags | OFlag::O_NONBLOCK)).map_err(|error| {
        ServeError::Runtime(format!("failed to set COM1 PTY nonblocking: {error}"))
    })?;
    Ok(())
}

#[cfg(unix)]
fn errno_to_io_error(error: nix::errno::Errno) -> io::Error {
    io::Error::from_raw_os_error(error as i32)
}

#[cfg(unix)]
async fn bridge_connected_serial<T, S>(
    transport: &mut T,
    runtime: &ServerRuntime,
    node_number: u16,
    child: &mut Child,
    serial: S,
    deadline: TokioInstant,
    result: &mut DoorBridgeResult,
) -> ServeResult<()>
where
    T: Transport,
    S: AsyncRead + AsyncWrite + Unpin,
{
    result.com1_connected = true;
    let (mut serial_reader, mut serial_writer) = tokio::io::split(serial);
    let mut serial_buf = [0_u8; 1024];

    loop {
        if let Some(status) = child.try_wait()? {
            result.exit_code = status.code();
            return Ok(());
        }

        let timeout_sleep = time::sleep_until(deadline);
        tokio::pin!(timeout_sleep);
        let poll_sleep = time::sleep(DOOR_BRIDGE_POLL);
        tokio::pin!(poll_sleep);

        tokio::select! {
            _ = &mut timeout_sleep => {
                result.timed_out = true;
                result.disconnect_forced = true;
                result.exit_code = terminate_child(child).await?;
                return Ok(());
            }
            _ = &mut poll_sleep => {
                runtime.heartbeat_node(node_number);
            }
            commands = runtime.wait_for_node_commands(node_number) => {
                for message in commands.messages {
                    write_bridge_message(transport, &message, result).await?;
                }
                if let Some(reason) = commands.disconnect_reason {
                    write_bridge_message(transport, "Disconnected by sysop.", result).await?;
                    result.disconnect_forced = true;
                    result.disconnect_reason = Some(reason);
                    result.exit_code = terminate_child(child).await?;
                    return Ok(());
                }
            }
            input = transport.read_byte() => {
                match input? {
                    Some(byte) => {
                        runtime.heartbeat_node(node_number);
                        serial_writer.write_all(&[byte]).await?;
                        result.bytes_in = result.bytes_in.saturating_add(1);
                    }
                    None => {
                        result.caller_disconnected = true;
                        result.disconnect_forced = true;
                        result.exit_code = terminate_child(child).await?;
                        return Ok(());
                    }
                }
            }
            read = serial_reader.read(&mut serial_buf) => {
                let count = read?;
                if count == 0 {
                    finish_child_after_serial_eof(child, result).await?;
                    return Ok(());
                } else {
                    runtime.heartbeat_node(node_number);
                    transport.write_all(&serial_buf[..count]).await?;
                    result.bytes_out = result.bytes_out.saturating_add(i64::try_from(count).unwrap_or(i64::MAX));
                }
            }
        }
    }
}

fn supported_drop_file(drop_file: &str) -> bool {
    matches!(
        drop_file.to_ascii_uppercase().as_str(),
        "DOOR.SYS" | "DORINFO1.DEF" | "CHAIN.TXT" | "DOORFILE.SR" | "PCBOARD.SYS" | "CALLINFO.BBS"
    )
}

fn command_exists(command: &str) -> bool {
    let path = Path::new(command);
    if path.components().count() > 1 {
        return path.is_file();
    }
    let Some(paths) = std::env::var_os("PATH") else {
        return false;
    };
    std::env::split_paths(&paths).any(|dir| dir.join(command).is_file())
}

fn validate_door_runner(runner: &str, allowed_runners: &[String]) -> Result<(), String> {
    if !allowed_runners.iter().any(|allowed| allowed == runner) {
        return Err(format!(
            "door runner {:?} is not allowed. expected one of {:?}",
            runner, allowed_runners
        ));
    }
    let runner_path = resolve_runner_path(runner)
        .ok_or_else(|| format!("door runner {:?} was not found on PATH", runner))?;
    let runner_path = runner_path
        .canonicalize()
        .map_err(|error| format!("door runner {:?} is not accessible: {error}", runner))?;
    if !runner_path.is_file() {
        return Err(format!(
            "door runner {:?} is not a regular file",
            runner_path.display()
        ));
    }
    validate_door_runner_security(&runner_path, runner)?;
    Ok(())
}

fn validate_door_runner_security(path: &Path, runner: &str) -> Result<(), String> {
    #[cfg(unix)]
    {
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
    }
    #[cfg(not(unix))]
    {
        let _ = runner;
    }
    Ok(())
}

fn resolve_runner_path(command: &str) -> Option<PathBuf> {
    let path = Path::new(command);
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

fn first_command_token(command: &str) -> Option<&str> {
    command.trim().split_ascii_whitespace().next()
}

fn is_quoted_dos_command(command: &str) -> bool {
    command.starts_with('"') || command.starts_with('\'')
}

fn door_to_core(door: &DoorDefinitionRecord) -> ServeResult<DoorDefinition> {
    Ok(DoorDefinition {
        id: door.id.clone(),
        key: door.key.clone(),
        name: door.name.clone(),
        runner: door.runner.clone(),
        working_dir: door.working_dir.clone(),
        command: door.command.clone(),
        drop_file: door.drop_file.clone(),
        exclusive: door.exclusive,
        time_limit_minutes: u32::try_from(door.time_limit_minutes).map_err(|error| {
            ServeError::Runtime(format!("door time limit is out of range: {error}"))
        })?,
        enabled: door.enabled,
        min_security_level: door.min_security_level as i32,
    })
}

fn door_caller(user: &User) -> DoorCaller {
    DoorCaller {
        alias: user.alias.clone(),
        real_name: user.real_name.clone(),
        location: "Local".to_string(),
        security_level: user.security_level,
        minutes_remaining: 30,
    }
}

#[cfg(unix)]
async fn write_bridge_message<T: Transport>(
    transport: &mut T,
    message: &str,
    result: &mut DoorBridgeResult,
) -> ServeResult<()> {
    let message = format!("\r\n{message}\r\n");
    let bytes = encode_text(&message);
    transport.write_all(&bytes).await?;
    result.bytes_out = result
        .bytes_out
        .saturating_add(i64::try_from(bytes.len()).unwrap_or(i64::MAX));
    Ok(())
}

#[cfg(unix)]
async fn finish_child_after_serial_eof(
    child: &mut Child,
    result: &mut DoorBridgeResult,
) -> ServeResult<()> {
    match child.try_wait()? {
        Some(status) => {
            result.exit_code = status.code();
            Ok(())
        }
        None => match time::timeout(DOOR_EXIT_WAIT, child.wait()).await {
            Ok(Ok(status)) => {
                result.exit_code = status.code();
                Ok(())
            }
            Ok(Err(error)) => Err(ServeError::Network(error)),
            Err(_) => {
                result.disconnect_forced = true;
                result.exit_code = terminate_child(child).await?;
                Ok(())
            }
        },
    }
}

#[cfg(unix)]
async fn terminate_child(child: &mut Child) -> ServeResult<Option<i32>> {
    match child.try_wait()? {
        Some(status) => Ok(status.code()),
        None => {
            let _ = child.start_kill();
            match time::timeout(DOOR_KILL_WAIT, child.wait()).await {
                Ok(Ok(status)) => Ok(status.code()),
                Ok(Err(error)) => Err(ServeError::Network(error)),
                Err(_) => {
                    child.start_kill()?;
                    let status = child.wait().await.map_err(ServeError::Network)?.code();
                    Ok(status)
                }
            }
        }
    }
}

fn door_error(error: oxidebbs_door::DoorError) -> ServeError {
    ServeError::Runtime(error.to_string())
}

fn generated_uuid(db: &OxideDb) -> ServeResult<String> {
    db_scalar_text(db, "SELECT UUID_TO_STRING(GEN_RANDOM_UUID())")
}

fn current_timestamp(db: &OxideDb) -> ServeResult<String> {
    db_scalar_text(db, "SELECT CAST(NOW() AS TEXT)")
}

fn db_scalar_text(db: &OxideDb, sql: &str) -> ServeResult<String> {
    let result = db.db().execute(sql).map_err(ServeError::Database)?;
    let value = result
        .rows()
        .first()
        .and_then(|row| row.values().first())
        .ok_or_else(|| ServeError::Runtime(format!("query returned no scalar value: {sql}")))?;

    match value {
        oxidebbs_db::Value::Text(value) => Ok(value.clone()),
        other => Err(ServeError::Runtime(format!(
            "query returned non-text scalar for {sql}: {other:?}"
        ))),
    }
}

fn encode_text(text: &str) -> Vec<u8> {
    match encode_cp437(text) {
        Ok(bytes) => bytes,
        Err(_) => encode_text_lossy(text),
    }
}

fn encode_text_lossy(text: &str) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(text.len());
    for character in text.chars() {
        let mut buffer = [0_u8; 4];
        let encoded = character.encode_utf8(&mut buffer);
        match encode_cp437(encoded) {
            Ok(encoded_bytes) => bytes.extend_from_slice(&encoded_bytes),
            Err(_) => bytes.push(b'?'),
        }
    }
    bytes
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::io::{Read, Write};

    use oxidebbs_core::user::UserStatus;
    use oxidebbs_db::{UserRecord, find_door_run_by_id, insert_user};
    use oxidebbs_door::DryRunDoorRunner;
    use oxidebbs_telnet::LoopbackTransport;

    use super::*;
    use crate::config::{
        AdminWebConfig, AuditConfig, AuthConfig, BoardConfig, DatabaseConfig, DoorDefConfig,
        DoorsConfig, FileTransfersConfig, FlowConfig, FtnConfig, LoggingConfig, MenuConfig,
        NetworkConfig, NodesConfig, PathsConfig, ScreenConfig, SerialConfig, SysopConfig,
        TelnetConfig, TerminalConfig,
    };

    const USER_ID: &str = "00000000-0000-4000-8000-000000000701";

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "oxidebbs-server-door-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("create temp dir");
        path
    }

    fn test_config(runtime: std::path::PathBuf, working_dir: std::path::PathBuf) -> OxideConfig {
        fs::write(working_dir.join("TEST.EXE"), b"").expect("create test command");
        fs::write(working_dir.join("dosemu"), b"").expect("create test runner");
        let runner = supported_runner_path(&working_dir);
        let doors_root = working_dir
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| working_dir.clone());
        OxideConfig {
            board: BoardConfig {
                name: "Test BBS".to_string(),
                tagline: "Testing".to_string(),
                sysop_name: "Sysop".to_string(),
                timezone: "UTC".to_string(),
            },
            telnet: TelnetConfig::default(),
            auth: AuthConfig::default(),
            audit: AuditConfig::default(),
            logging: LoggingConfig::default(),
            database: DatabaseConfig::default(),
            paths: PathsConfig {
                ansi: runtime.join("ansi"),
                screens: runtime.join("screens"),
                doors: doors_root,
                runtime: runtime.clone(),
                logs: working_dir.join("logs"),
            },
            nodes: NodesConfig::default(),
            sysop: SysopConfig::default(),
            terminal: TerminalConfig::default(),
            flow: FlowConfig::default(),
            screens: HashMap::<String, ScreenConfig>::new(),
            menus: HashMap::<String, MenuConfig>::new(),
            doors: DoorsConfig {
                enabled: true,
                default_runner: runner.clone(),
                allowed_runners: vec![runner.clone()],
                definitions: vec![DoorDefConfig {
                    key: "test".to_string(),
                    name: "Test Door".to_string(),
                    runner,
                    working_dir: working_dir.to_string_lossy().to_string(),
                    command: "TEST.EXE".to_string(),
                    drop_file: "DOOR.SYS".to_string(),
                    exclusive: false,
                    time_limit_minutes: 1,
                    enabled: true,
                    min_security_level: 0,
                }],
            },
            network: NetworkConfig::default(),
            ftn: FtnConfig::default(),
            serial: SerialConfig::default(),
            file_transfers: FileTransfersConfig::default(),
            admin_web: AdminWebConfig::default(),
        }
    }

    fn supported_runner_path(dir: &Path) -> String {
        dir.join("dosemu").to_string_lossy().to_string()
    }

    fn current_runner() -> String {
        std::env::current_exe()
            .expect("current exe")
            .to_string_lossy()
            .to_string()
    }

    fn test_user() -> User {
        User {
            id: USER_ID.to_string(),
            alias: "dooruser".to_string(),
            real_name: "Door User".to_string(),
            email: None,
            password_hash: "hash".to_string(),
            security_level: 10,
            is_sysop: false,
            created_at: "2026-01-01T00:00:00.000000Z".to_string(),
            last_login_at: None,
            total_calls: 0,
            time_bank_minutes: 0,
            status: UserStatus::Active,
        }
    }

    fn insert_test_user(db: &OxideDb) {
        let user = test_user();
        insert_user(
            db.db(),
            &UserRecord {
                id: user.id,
                alias: user.alias,
                real_name: user.real_name,
                email: user.email,
                password_hash: user.password_hash,
                security_level: i64::from(user.security_level),
                is_sysop: user.is_sysop,
                created_at: user.created_at,
                last_login_at: user.last_login_at,
                total_calls: user.total_calls,
                time_bank_minutes: user.time_bank_minutes,
                status: "active".to_string(),
            },
        )
        .expect("insert user");
    }

    #[test]
    fn render_and_select_door_menu_accepts_key_or_number() {
        let door = DoorDefinitionRecord {
            id: "00000000-0000-4000-8000-000000000801".to_string(),
            key: "test".to_string(),
            name: "Test Door".to_string(),
            runner: current_runner(),
            working_dir: ".".to_string(),
            command: "TEST.EXE".to_string(),
            drop_file: "DOOR.SYS".to_string(),
            exclusive: false,
            time_limit_minutes: 1,
            enabled: true,
            min_security_level: 0,
        };
        let doors = vec![door];
        let menu = render_door_menu(&doors);

        assert!(menu.contains("1) Test Door (test)"));
        assert!(matches!(select_door(&doors, "1"), DoorSelection::Door(_)));
        assert!(matches!(
            select_door(&doors, "TEST"),
            DoorSelection::Door(_)
        ));
        assert!(matches!(select_door(&doors, ""), DoorSelection::Return));
        assert_eq!(select_door(&doors, "bad"), DoorSelection::Invalid);
    }

    #[test]
    fn failed_door_audit_write_increments_runtime_counter() {
        let db = OxideDb::open_memory().expect("open db");
        let runtime_dir = temp_dir("audit-runtime");
        let working_dir = temp_dir("audit-working");
        let config = test_config(runtime_dir.clone(), working_dir.clone());
        let service = DoorService::new(&db, &config);
        let door = DoorDefinitionRecord {
            id: "00000000-0000-4000-8000-000000000802".to_string(),
            key: "test".to_string(),
            name: "Test Door".to_string(),
            runner: current_runner(),
            working_dir: working_dir.to_string_lossy().to_string(),
            command: "TEST.EXE".to_string(),
            drop_file: "DOOR.SYS".to_string(),
            exclusive: false,
            time_limit_minutes: 1,
            enabled: true,
            min_security_level: 0,
        };
        let mut user = test_user();
        user.id = "not-a-uuid".to_string();
        let runtime = ServerRuntime::new("test".to_string(), 1, 1, 60);

        service.audit_door_event(
            "door_started",
            &door,
            &user,
            1,
            Some(&runtime),
            "forced audit failure".to_string(),
        );

        assert_eq!(runtime.audit_write_failures(), 1);

        let _ = fs::remove_dir_all(runtime_dir);
        let _ = fs::remove_dir_all(working_dir);
    }

    #[test]
    fn dosemu2_serial_config_maps_com1_to_pty_bridge() {
        let config = dosemu2_serial_config(Path::new("/tmp/node-001/OXCOM1.PTY"));

        assert!(config.contains("$_cpu_vm = \"emulated\""));
        assert!(config.contains("$_cpu_vm_dpmi = \"emulated\""));
        assert!(config.contains("$_sound = (off)"));
        assert!(config.contains("$_mouse_internal = (off)"));
        assert!(config.contains("$_joy_device = \"\""));
        assert!(config.contains("$_pktdriver = (off)"));
        assert!(config.contains("$_tcpdriver = (off)"));
        assert!(config.contains("$_ttylocks = \"\""));
        assert!(config.contains("$_com1 = \"pts /tmp/node-001/OXCOM1.PTY\""));
    }

    #[test]
    fn add_dosemu2_config_prepends_runtime_conf() {
        let mut plan = DoorRunPlan {
            program: "dosemu".to_string(),
            args: vec!["-dumb".to_string(), "-quiet".to_string()],
            working_dir: Path::new("/tmp").to_path_buf(),
            drop_file_path: Path::new("/tmp/DORINFO1.DEF").to_path_buf(),
            timeout: Duration::from_secs(60),
        };

        add_dosemu2_config(&mut plan, Path::new("/tmp/OXDOSEMU2.CONF"));

        assert_eq!(plan.args[0], "-f");
        assert_eq!(plan.args[1], "/tmp/OXDOSEMU2.CONF");
        assert_eq!(plan.args[2], "-dumb");
        assert_eq!(plan.args[3], "-quiet");
    }

    #[test]
    fn prepare_dosemu2_com1_bridge_uses_absolute_runtime_paths() {
        let runtime_dir = temp_dir("absolute-bridge-runtime");
        let mut plan = DoorRunPlan {
            program: "dosemu".to_string(),
            args: vec!["-dumb".to_string(), "-quiet".to_string()],
            working_dir: runtime_dir.clone(),
            drop_file_path: runtime_dir.join("DOOR.SYS"),
            timeout: Duration::from_secs(60),
        };

        let bridge = prepare_dosemu2_com1_bridge(&mut plan, &runtime_dir).expect("prepare bridge");

        assert!(bridge.config_path.is_absolute());
        assert!(bridge.pty_path.is_absolute());
        assert_eq!(plan.args[0], "-f");
        assert_eq!(plan.args[1], bridge.config_path.display().to_string());
        let config = fs::read_to_string(&bridge.config_path).expect("read bridge config");
        assert!(config.contains(&bridge.pty_path.display().to_string()));

        let _ = fs::remove_dir_all(runtime_dir);
    }

    #[test]
    fn door_runner_stdio_creates_per_run_log_files() {
        let base = temp_dir("stdio-logs");
        let logs_dir = base.join("logs");

        let (_stdout, _stderr, logs) = door_runner_stdio(&logs_dir, "run-001", "oxide/check");

        assert!(logs.capture_errors.is_empty());
        let stdout = logs.stdout.expect("stdout path");
        let stderr = logs.stderr.expect("stderr path");
        assert_eq!(
            stdout.file_name().and_then(|name| name.to_str()),
            Some("oxide_check-run-001.stdout.log")
        );
        assert_eq!(
            stderr.file_name().and_then(|name| name.to_str()),
            Some("oxide_check-run-001.stderr.log")
        );
        assert!(stdout.is_file());
        assert!(stderr.is_file());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(&stdout)
                .expect("stdout metadata")
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(mode, 0o600);
            let mode = fs::metadata(&stderr)
                .expect("stderr metadata")
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(mode, 0o600);
        }

        let _ = fs::remove_dir_all(base);
    }

    #[tokio::test]
    async fn async_pty_reads_and_writes_without_file_seek() {
        let pty = nix::pty::openpty(None, None).expect("open pty");
        let mut master = fs::File::from(pty.master);
        let slave = fs::File::from(pty.slave);
        set_raw_mode(&slave).expect("raw mode");
        set_nonblocking(&slave).expect("nonblocking mode");
        let serial = AsyncPty::new(slave).expect("async pty");
        let (mut serial_reader, mut serial_writer) = tokio::io::split(serial);

        master.write_all(b"from-master").expect("write master");
        let mut inbound = [0_u8; 11];
        time::timeout(
            Duration::from_secs(1),
            serial_reader.read_exact(&mut inbound),
        )
        .await
        .expect("serial read timeout")
        .expect("serial read");
        assert_eq!(&inbound, b"from-master");

        serial_writer
            .write_all(b"to-master")
            .await
            .expect("write serial");
        let mut outbound = [0_u8; 9];
        master.read_exact(&mut outbound).expect("read master");
        assert_eq!(&outbound, b"to-master");
    }

    #[test]
    fn validate_door_rejects_disabled_missing_runner_and_bad_dropfile() {
        let runtime = temp_dir("validate-runtime");
        let working_dir = temp_dir("validate-working");
        fs::write(working_dir.join("TEST.EXE"), b"").expect("create test command");
        fs::write(working_dir.join("OXIDECHK.EXE"), b"").expect("create test command");
        fs::write(working_dir.join("LORD.EXE"), b"").expect("create test command");
        let config = test_config(runtime.clone(), working_dir);
        let db = OxideDb::open_memory().expect("open db");
        let service = DoorService::new(&db, &config);
        let mut door = service
            .list_enabled_doors()
            .expect("list doors")
            .pop()
            .expect("door");

        door.enabled = false;
        assert!(service.validate_door(&door, 1).is_err());

        door.enabled = true;
        door.runner = runtime.join("missing-runner").to_string_lossy().to_string();
        assert!(service.validate_door(&door, 1).is_err());

        door.runner = "dosbox".to_string();
        let unsupported_error = service.validate_door(&door, 1).unwrap_err();
        assert!(
            unsupported_error.contains("not supported")
                && unsupported_error.contains("DOSEMU2")
                && unsupported_error.contains("dosemu")
        );

        door.runner = supported_runner_path(Path::new(&door.working_dir));
        door.drop_file = "BAD.TXT".to_string();
        assert!(service.validate_door(&door, 1).is_err());

        door.command = "OXIDECHK.EXE".to_string();
        for drop_file in [
            "DOOR.SYS",
            "DORINFO1.DEF",
            "CHAIN.TXT",
            "DOORFILE.SR",
            "PCBOARD.SYS",
            "CALLINFO.BBS",
        ] {
            door.drop_file = drop_file.to_string();
            assert!(
                service.validate_door(&door, 1).is_ok(),
                "{drop_file} should validate"
            );
        }

        door.drop_file = "DOOR.SYS".to_string();
        door.command = "LORD.EXE /N1".to_string();
        assert!(service.validate_door(&door, 1).is_ok());

        door.command = "\"OXIDECHK.EXE\"".to_string();
        let quoted_error = service.validate_door(&door, 1).unwrap_err();
        assert!(
            quoted_error.contains("quoted DOS command") && quoted_error.contains("DOS 8.3 paths")
        );

        door.command = "   ".to_string();
        let empty_error = service.validate_door(&door, 1).unwrap_err();
        assert!(empty_error.contains("command is empty"));

        let _ = fs::remove_dir_all(runtime);
    }

    #[cfg(unix)]
    #[test]
    fn runner_revalidation_detects_permission_change_before_spawn() {
        let runtime = temp_dir("runner-recheck-runtime");
        let working_dir = temp_dir("runner-recheck-working");
        let config = test_config(runtime.clone(), working_dir.clone());
        let runner = supported_runner_path(&working_dir);

        validate_door_runner(&runner, &config.doors.allowed_runners).expect("safe runner");

        use std::os::unix::fs::PermissionsExt;
        let mode = fs::metadata(&runner)
            .expect("runner metadata")
            .permissions()
            .mode()
            | 0o002;
        fs::set_permissions(&runner, fs::Permissions::from_mode(mode))
            .expect("runner world-writable");

        let error = validate_door_runner(&runner, &config.doors.allowed_runners)
            .expect_err("unsafe runner should fail revalidation");
        assert!(error.contains("world-writable"));

        let _ = fs::remove_dir_all(runtime);
        let _ = fs::remove_dir_all(working_dir);
    }

    #[test]
    fn dry_run_service_preserves_runtime_when_requested() {
        let runtime = temp_dir("dry-runtime");
        let working_dir = temp_dir("dry-working");
        let config = test_config(runtime.clone(), working_dir);
        let db = OxideDb::open_memory().expect("open db");
        insert_test_user(&db);
        let service = DoorService::new(&db, &config);
        let door = service
            .list_enabled_doors()
            .expect("list doors")
            .pop()
            .expect("door");

        let summary = service
            .execute_with_runner(&DryRunDoorRunner, &test_user(), 1, &door, true)
            .expect("run dry");

        assert_eq!(summary.exit_code, Some(0));
        assert!(!summary.timed_out);
        assert!(runtime.join("node-001").join("DOOR.SYS").is_file());

        let run = find_door_run_by_id(db.db(), summary.run_id.as_deref().expect("run id"))
            .expect("find run")
            .expect("run exists");
        assert!(run.ended_at.is_some());
        assert_eq!(run.exit_code, Some(0));
        assert_eq!(run.bytes_in, 0);
        assert_eq!(run.bytes_out, 0);

        let _ = fs::remove_dir_all(runtime);
    }

    #[test]
    fn dry_run_service_cleans_runtime_directory_after_completion() {
        let runtime = temp_dir("dry-runtime-clean");
        let working_dir = temp_dir("dry-working-clean");
        let config = test_config(runtime.clone(), working_dir.clone());
        let db = OxideDb::open_memory().expect("open db");
        insert_test_user(&db);
        let service = DoorService::new(&db, &config);
        let door = service
            .list_enabled_doors()
            .expect("list doors")
            .pop()
            .expect("door");

        let summary = service
            .execute_with_runner(&DryRunDoorRunner, &test_user(), 1, &door, false)
            .expect("run dry");

        assert_eq!(summary.exit_code, Some(0));
        assert!(!runtime.join("node-001").exists());
        let run = find_door_run_by_id(db.db(), summary.run_id.as_deref().expect("run id"))
            .expect("find run")
            .expect("run exists");
        assert!(run.ended_at.is_some());

        let _ = fs::remove_dir_all(runtime);
        let _ = fs::remove_dir_all(working_dir);
    }

    #[test]
    fn dry_run_service_cleans_runtime_directory_after_prepare_door_run_failure() {
        let runtime = temp_dir("dry-runtime-prepare-fail");
        let working_dir = temp_dir("dry-working-prepare-fail");
        let config = test_config(runtime.clone(), working_dir.clone());
        let db = OxideDb::open_memory().expect("open db");
        insert_test_user(&db);
        let service = DoorService::new(&db, &config);
        let mut door = service
            .list_enabled_doors()
            .expect("list doors")
            .pop()
            .expect("door");

        let command_path = working_dir.join("COMMAND.EXE");
        fs::write(&command_path, b"fixture").expect("command fixture");
        let command_path = command_path.to_string_lossy().to_string();
        door.command = command_path;

        let result = service.execute_with_runner(&DryRunDoorRunner, &test_user(), 1, &door, false);
        assert!(result.is_err());
        assert!(!runtime.join("node-001").exists());

        let _ = fs::remove_dir_all(runtime);
        let _ = fs::remove_dir_all(working_dir);
    }

    #[test]
    fn runtime_guard_cleans_directory_after_command_staging_failure() {
        let runtime = temp_dir("stage-cleanup-runtime");
        let working_dir = temp_dir("stage-cleanup-working");
        let config = test_config(runtime.clone(), working_dir);
        let db = OxideDb::open_memory().expect("open db");
        let service = DoorService::new(&db, &config);
        let door = service
            .list_enabled_doors()
            .expect("list doors")
            .pop()
            .expect("door");

        let runtime_guard =
            DoorRuntimeDirectoryGuard::prepare(&config.paths.runtime, 1).expect("runtime guard");
        let request = service
            .build_request(&test_user(), 1, &door)
            .expect("build request");
        let plan = prepare_door_run(&request).expect("prepare door run");
        assert!(plan.working_dir.join("TEST.EXE").is_file());

        drop(runtime_guard);

        assert!(!runtime.join("node-001").exists());
        let _ = fs::remove_dir_all(runtime);
    }

    #[tokio::test]
    async fn bridge_forwards_caller_bytes_and_serial_output() {
        let (serial_bridge, mut serial_peer) = tokio::io::duplex(1024);
        let (mut transport, mut client) = LoopbackTransport::new();
        client.write_bytes(b"hello\n").expect("caller input");
        let runtime = ServerRuntime::new("test".to_string(), 1, 1, 30);
        runtime.mark_node_connected(
            1,
            "session".to_string(),
            "127.0.0.1:23".to_string(),
            "now".to_string(),
        );

        let serial_task = tokio::spawn(async move {
            let mut input = [0_u8; 6];
            serial_peer
                .read_exact(&mut input)
                .await
                .expect("read caller bytes");
            assert_eq!(&input, b"hello\n");
            serial_peer
                .write_all(b"serial-ready\r\n")
                .await
                .expect("write serial bytes");
        });

        let child = Command::new("sh")
            .arg("-c")
            .arg("sleep 5")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn sleep helper");

        let mut child = child;
        let mut result = DoorBridgeResult::default();
        bridge_connected_serial(
            &mut transport,
            &runtime,
            1,
            &mut child,
            serial_bridge,
            TokioInstant::now() + Duration::from_secs(2),
            &mut result,
        )
        .await
        .expect("bridge");

        serial_task.await.expect("serial task");

        let output = String::from_utf8_lossy(&client.read_output_bytes()).to_string();
        assert!(output.contains("serial-ready"));
        assert_eq!(result.bytes_in, 6);
        assert!(result.bytes_out >= 14);
        assert!(result.com1_connected);
    }

    #[tokio::test]
    async fn serial_eof_waits_for_child_exit_before_forcing_disconnect() {
        let (serial_bridge, serial_peer) = tokio::io::duplex(1024);
        drop(serial_peer);
        let (mut transport, _client) = LoopbackTransport::new();
        let runtime = ServerRuntime::new("test".to_string(), 1, 1, 30);
        runtime.mark_node_connected(
            1,
            "session".to_string(),
            "127.0.0.1:23".to_string(),
            "now".to_string(),
        );
        let mut child = Command::new("sh")
            .arg("-c")
            .arg("sleep 0.05; exit 7")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn exit helper");
        let mut result = DoorBridgeResult::default();

        bridge_connected_serial(
            &mut transport,
            &runtime,
            1,
            &mut child,
            serial_bridge,
            TokioInstant::now() + Duration::from_secs(2),
            &mut result,
        )
        .await
        .expect("bridge");

        assert_eq!(result.exit_code, Some(7));
        assert!(!result.disconnect_forced);
        assert!(result.com1_connected);
    }

    #[tokio::test]
    async fn bridge_marks_child_exit_before_com1_pty() {
        let runtime_dir = temp_dir("early-exit-runtime");
        let bridge = Dosemu2ComBridge {
            pty_path: runtime_dir.join(DOSEMU2_COM1_PTY),
            config_path: runtime_dir.join(DOSEMU2_CONFIG_FILE),
        };
        let (mut transport, _client) = LoopbackTransport::new();
        let runtime = ServerRuntime::new("test".to_string(), 1, 1, 30);
        runtime.mark_node_connected(
            1,
            "session".to_string(),
            "127.0.0.1:23".to_string(),
            "now".to_string(),
        );
        let child = Command::new("sh")
            .arg("-c")
            .arg("exit 7")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn exit helper");

        let result = run_door_bridge(
            &mut transport,
            &runtime,
            1,
            child,
            bridge,
            Duration::from_secs(2),
        )
        .await
        .expect("bridge");

        assert_eq!(result.exit_code, Some(7));
        assert!(result.early_exit_before_com1);
        assert!(!result.com1_connected);
        assert!(!result.timed_out);
        let _ = fs::remove_dir_all(runtime_dir);
    }

    #[tokio::test]
    async fn bridge_times_out_and_terminates_child() {
        let runtime_dir = temp_dir("missing-pty-runtime");
        let bridge = Dosemu2ComBridge {
            pty_path: runtime_dir.join(DOSEMU2_COM1_PTY),
            config_path: runtime_dir.join(DOSEMU2_CONFIG_FILE),
        };
        let (mut transport, _client) = LoopbackTransport::new();
        let runtime = ServerRuntime::new("test".to_string(), 1, 1, 30);
        runtime.mark_node_connected(
            1,
            "session".to_string(),
            "127.0.0.1:23".to_string(),
            "now".to_string(),
        );
        let child = Command::new("sh")
            .arg("-c")
            .arg("sleep 2")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn sleep helper");

        let result = run_door_bridge(
            &mut transport,
            &runtime,
            1,
            child,
            bridge,
            Duration::from_millis(50),
        )
        .await
        .expect("bridge");

        assert!(result.timed_out);
        assert!(result.disconnect_forced);
        let _ = fs::remove_dir_all(runtime_dir);
    }

    #[tokio::test]
    async fn runtime_guard_cleans_directory_after_bridge_timeout() {
        let runtime_root = temp_dir("bridge-timeout-cleanup");
        let bridge_runtime = runtime_root.join("node-001");
        let guard = DoorRuntimeDirectoryGuard::prepare(&runtime_root, 1).expect("runtime guard");
        let bridge = Dosemu2ComBridge {
            pty_path: bridge_runtime.join(DOSEMU2_COM1_PTY),
            config_path: bridge_runtime.join(DOSEMU2_CONFIG_FILE),
        };
        let runtime = ServerRuntime::new("test".to_string(), 1, 1, 30);
        runtime.mark_node_connected(
            1,
            "session".to_string(),
            "127.0.0.1:23".to_string(),
            "now".to_string(),
        );
        let (mut transport, _client) = LoopbackTransport::new();
        let child = Command::new("sh")
            .arg("-c")
            .arg("sleep 2")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn child");

        let result = run_door_bridge(
            &mut transport,
            &runtime,
            1,
            child,
            bridge,
            Duration::from_millis(50),
        )
        .await
        .expect("bridge");

        assert!(result.timed_out);
        drop(guard);
        assert!(!bridge_runtime.exists());

        let _ = fs::remove_dir_all(runtime_root);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn bridge_error_cleans_runtime_directory_via_guard() {
        let runtime = ServerRuntime::new("test".to_string(), 1, 1, 30);
        let runtime_root = temp_dir("bridge-error-cleanup");
        let bridge_runtime = runtime_root.join("node-001");
        let bridge_guard =
            DoorRuntimeDirectoryGuard::prepare(&runtime_root, 1).expect("runtime guard");
        let bridge = Dosemu2ComBridge {
            pty_path: bridge_runtime.join(DOSEMU2_COM1_PTY),
            config_path: bridge_runtime.join(DOSEMU2_CONFIG_FILE),
        };
        fs::create_dir_all(&bridge_runtime).expect("runtime dir");
        fs::write(&bridge.pty_path, b"").expect("create pty file");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&bridge.pty_path, fs::Permissions::from_mode(0o000))
                .expect("restrict pty");
        }
        let child = Command::new("sh")
            .arg("-c")
            .arg("sleep 2")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn child");
        let mut transport = LoopbackTransport::new().0;

        let result = run_door_bridge(
            &mut transport,
            &runtime,
            1,
            child,
            bridge,
            Duration::from_secs(2),
        )
        .await;

        assert!(result.is_err());
        drop(bridge_guard);
        assert!(!bridge_runtime.exists());
        let _ = fs::remove_dir_all(runtime_root);
    }
}
