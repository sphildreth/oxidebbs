use std::fs;
use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use oxidebbs_core::door::DoorDefinition;
use oxidebbs_core::user::User;
use oxidebbs_db::{
    AuditEventRecord, DoorDefinitionRecord, DoorRunFinish, DoorRunRecord, OxideDb,
    find_door_by_key, finish_door_run, insert_audit_event, insert_door_definition, insert_door_run,
    list_door_definitions,
};
#[cfg(test)]
use oxidebbs_door::DoorRunner;
use oxidebbs_door::{
    DoorCaller, DoorRunRequest, cleanup_node_runtime_dir, node_runtime_dir, prepare_door_run,
    prepare_node_runtime_dir,
};
use oxidebbs_telnet::Transport;
use oxidebbs_term::encode_cp437;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};
use tokio::process::{Child, Command};
use tokio::time::{self, Instant as TokioInstant};
use tracing::warn;

use crate::config::{DoorDefConfig, OxideConfig};
use crate::control::ServerRuntime;
use crate::serve::{ServeError, ServeResult};

const DOOR_BRIDGE_POLL: Duration = Duration::from_millis(250);
const DOOR_KILL_WAIT: Duration = Duration::from_secs(2);

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
    pub bytes_in: i64,
    pub bytes_out: i64,
    pub launch_error: Option<String>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(crate) struct DoorBridgeResult {
    pub exit_code: Option<i32>,
    pub timed_out: bool,
    pub disconnect_forced: bool,
    pub caller_disconnected: bool,
    pub disconnect_reason: Option<String>,
    pub bytes_in: i64,
    pub bytes_out: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum DoorSelection<'a> {
    Return,
    Door(&'a DoorDefinitionRecord),
    Invalid,
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
        node_number: u16,
    ) -> Result<(), String> {
        if !self.config.doors.enabled {
            return Err("Doors are disabled for this board.".to_string());
        }
        if !door.enabled {
            return Err(format!("Door {} is disabled.", door.key));
        }
        if door.time_limit_minutes <= 0 {
            return Err(format!(
                "Door {} must have a positive time limit.",
                door.key
            ));
        }
        if door.time_limit_minutes > i64::from(u32::MAX) {
            return Err(format!("Door {} time limit is too large.", door.key));
        }

        let working_dir = Path::new(&door.working_dir);
        if !working_dir.is_dir() {
            return Err(format!(
                "Door {} working directory {} does not exist.",
                door.key,
                working_dir.display()
            ));
        }

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

        let runtime_dir = prepare_node_runtime_dir(&self.config.paths.runtime, node_number)
            .map_err(|error| {
                format!(
                    "Door runtime directory {} is not writable: {error}.",
                    self.config.paths.runtime.display()
                )
            })?;
        let probe = runtime_dir.join(".oxidebbs-write-test");
        fs::write(&probe, b"ok").map_err(|error| {
            format!(
                "Door runtime directory {} is not writable: {error}.",
                runtime_dir.display()
            )
        })?;
        let _ = fs::remove_file(probe);

        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn execute_with_runner<R: DoorRunner>(
        &self,
        runner: &R,
        user: &User,
        node_number: u16,
        door: &DoorDefinitionRecord,
        cleanup_runtime: bool,
    ) -> ServeResult<DoorExecutionSummary> {
        self.validate_door(door, node_number)
            .map_err(ServeError::Runtime)?;
        let request = self.build_request(user, node_number, door)?;
        let _plan = prepare_door_run(&request).map_err(door_error)?;
        let run_id = self.insert_started_run(door, user, node_number)?;
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
            None,
        )?;

        if cleanup_runtime && let Err(error) = cleanup_node_runtime_dir(&request.runtime_dir) {
            warn!(
                path = %request.runtime_dir.display(),
                "failed to clean door runtime directory: {error}"
            );
        }

        Ok(DoorExecutionSummary {
            door_name: door.name.clone(),
            run_id: Some(run_id),
            exit_code: result.exit_code,
            timed_out: result.timed_out,
            disconnect_forced: result.timed_out,
            caller_disconnected: false,
            disconnect_reason: None,
            bytes_in: 0,
            bytes_out: 0,
            launch_error: None,
        })
    }

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
        let request = self.build_request(user, node_number, door)?;
        let plan = prepare_door_run(&request).map_err(door_error)?;
        let run_id = self.insert_started_run(door, user, node_number)?;

        let mut command = Command::new(&plan.program);
        command
            .args(&plan.args)
            .current_dir(&plan.working_dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let child = match command.spawn() {
            Ok(child) => child,
            Err(error) => {
                let message = format!("failed to launch door runner {:?}: {error}", plan.program);
                let bridge = DoorBridgeResult::default();
                self.finish_run(&run_id, door, user, node_number, bridge, Some(&message))?;
                cleanup_runtime_dir(&request);
                return Ok(DoorExecutionSummary {
                    door_name: door.name.clone(),
                    run_id: Some(run_id),
                    exit_code: None,
                    timed_out: false,
                    disconnect_forced: false,
                    caller_disconnected: false,
                    disconnect_reason: None,
                    bytes_in: 0,
                    bytes_out: 0,
                    launch_error: Some(message),
                });
            }
        };

        runtime.mark_node_in_door(node_number);
        let bridge = run_door_bridge(transport, runtime, node_number, child, plan.timeout).await;
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
                    Some("bridge error"),
                ) {
                    warn!("failed to finish door run {run_id} after bridge error: {finish_error}");
                }
                cleanup_runtime_dir(&request);
                return Err(error);
            }
        };

        self.finish_run(&run_id, door, user, node_number, bridge.clone(), None)?;
        cleanup_runtime_dir(&request);

        Ok(DoorExecutionSummary {
            door_name: door.name.clone(),
            run_id: Some(run_id),
            exit_code: bridge.exit_code,
            timed_out: bridge.timed_out,
            disconnect_forced: bridge.disconnect_forced,
            caller_disconnected: bridge.caller_disconnected,
            disconnect_reason: bridge.disconnect_reason,
            bytes_in: bridge.bytes_in,
            bytes_out: bridge.bytes_out,
            launch_error: None,
        })
    }

    fn sync_configured_doors(&self) -> ServeResult<()> {
        for door in &self.config.doors.definitions {
            if find_door_by_key(self.db.db(), &door.key)
                .map_err(ServeError::Database)?
                .is_none()
            {
                let record = self.record_from_config(door)?;
                insert_door_definition(self.db.db(), &record).map_err(ServeError::Database)?;
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
            node_number,
            runtime_dir: node_runtime_dir(&self.config.paths.runtime, node_number),
        })
    }

    fn insert_started_run(
        &self,
        door: &DoorDefinitionRecord,
        user: &User,
        node_number: u16,
    ) -> ServeResult<String> {
        let run_id = generated_uuid(self.db)?;
        let started_at = current_timestamp(self.db)?;
        insert_door_run(
            self.db.db(),
            &DoorRunRecord {
                id: run_id.clone(),
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

        self.audit_door_event(
            "door_started",
            door,
            user,
            node_number,
            format!("door run {run_id} started for {}", door.key),
        );

        Ok(run_id)
    }

    fn finish_run(
        &self,
        run_id: &str,
        door: &DoorDefinitionRecord,
        user: &User,
        node_number: u16,
        bridge: DoorBridgeResult,
        note: Option<&str>,
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
        if let Some(note) = note {
            details.push_str("; ");
            details.push_str(note);
        }
        self.audit_door_event(event_type, door, user, node_number, details);
        Ok(())
    }

    fn audit_door_event(
        &self,
        event_type: &str,
        door: &DoorDefinitionRecord,
        user: &User,
        node_number: u16,
        details: String,
    ) {
        let event_id = match generated_uuid(self.db) {
            Ok(id) => id,
            Err(error) => {
                warn!(
                    "failed to generate {event_type} audit id for {}: {error}",
                    door.key
                );
                return;
            }
        };
        let created_at = match current_timestamp(self.db) {
            Ok(value) => value,
            Err(error) => {
                warn!(
                    "failed to generate {event_type} audit timestamp for {}: {error}",
                    door.key
                );
                return;
            }
        };
        if let Err(error) = insert_audit_event(
            self.db.db(),
            &AuditEventRecord {
                id: event_id,
                created_at,
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
        }
    }

    fn record_from_config(&self, door: &DoorDefConfig) -> ServeResult<DoorDefinitionRecord> {
        Ok(DoorDefinitionRecord {
            id: generated_uuid(self.db)?,
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
            enabled: door.enabled,
        })
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

pub(crate) async fn run_door_bridge<T: Transport>(
    transport: &mut T,
    runtime: &ServerRuntime,
    node_number: u16,
    mut child: Child,
    timeout: Duration,
) -> ServeResult<DoorBridgeResult> {
    let mut child_stdin = child
        .stdin
        .take()
        .ok_or_else(|| ServeError::Runtime("door process missing stdin stream".to_string()))?;
    let mut child_stdout = child
        .stdout
        .take()
        .ok_or_else(|| ServeError::Runtime("door process missing stdout stream".to_string()))?;
    let mut child_stderr = child
        .stderr
        .take()
        .ok_or_else(|| ServeError::Runtime("door process missing stderr stream".to_string()))?;

    let deadline = TokioInstant::now() + timeout;
    let mut result = DoorBridgeResult::default();
    let mut stdout_open = true;
    let mut stderr_open = true;
    let mut stdout_buf = [0_u8; 1024];
    let mut stderr_buf = [0_u8; 1024];

    loop {
        if let Some(status) = child.try_wait()? {
            result.exit_code = status.code();
            if stdout_open {
                drain_remaining_output(&mut child_stdout, transport, &mut result).await?;
            }
            if stderr_open {
                drain_remaining_output(&mut child_stderr, transport, &mut result).await?;
            }
            break;
        }

        let timeout_sleep = time::sleep_until(deadline);
        tokio::pin!(timeout_sleep);
        let poll_sleep = time::sleep(DOOR_BRIDGE_POLL);
        tokio::pin!(poll_sleep);

        tokio::select! {
            _ = &mut timeout_sleep => {
                result.timed_out = true;
                result.disconnect_forced = true;
                result.exit_code = terminate_child(&mut child).await?;
                if stdout_open {
                    drain_remaining_output(&mut child_stdout, transport, &mut result).await?;
                }
                if stderr_open {
                    drain_remaining_output(&mut child_stderr, transport, &mut result).await?;
                }
                break;
            }
            _ = &mut poll_sleep => {
                runtime.heartbeat_node(node_number);
            }
            commands = runtime.wait_for_node_commands(node_number) => {
                for message in commands.messages {
                    write_bridge_message(transport, &message, &mut result).await?;
                }
                if let Some(reason) = commands.disconnect_reason {
                    write_bridge_message(transport, "Disconnected by sysop.", &mut result).await?;
                    result.disconnect_forced = true;
                    result.disconnect_reason = Some(reason);
                    result.exit_code = terminate_child(&mut child).await?;
                    if stdout_open {
                        drain_remaining_output(&mut child_stdout, transport, &mut result).await?;
                    }
                    if stderr_open {
                        drain_remaining_output(&mut child_stderr, transport, &mut result).await?;
                    }
                    break;
                }
            }
            input = transport.read_byte() => {
                match input? {
                    Some(byte) => {
                        runtime.heartbeat_node(node_number);
                        child_stdin.write_all(&[byte]).await?;
                        result.bytes_in = result.bytes_in.saturating_add(1);
                    }
                    None => {
                        result.caller_disconnected = true;
                        result.disconnect_forced = true;
                        result.exit_code = terminate_child(&mut child).await?;
                        if stdout_open {
                            drain_remaining_output(&mut child_stdout, transport, &mut result).await?;
                        }
                        if stderr_open {
                            drain_remaining_output(&mut child_stderr, transport, &mut result).await?;
                        }
                        break;
                    }
                }
            }
            read = child_stdout.read(&mut stdout_buf), if stdout_open => {
                let count = read?;
                if count == 0 {
                    stdout_open = false;
                } else {
                    runtime.heartbeat_node(node_number);
                    transport.write_all(&stdout_buf[..count]).await?;
                    result.bytes_out = result.bytes_out.saturating_add(i64::try_from(count).unwrap_or(i64::MAX));
                }
            }
            read = child_stderr.read(&mut stderr_buf), if stderr_open => {
                let count = read?;
                if count == 0 {
                    stderr_open = false;
                } else {
                    runtime.heartbeat_node(node_number);
                    transport.write_all(&stderr_buf[..count]).await?;
                    result.bytes_out = result.bytes_out.saturating_add(i64::try_from(count).unwrap_or(i64::MAX));
                }
            }
        }
    }

    Ok(result)
}

fn supported_drop_file(drop_file: &str) -> bool {
    matches!(
        drop_file.to_ascii_uppercase().as_str(),
        "DOOR.SYS" | "DORINFO1.DEF"
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

fn cleanup_runtime_dir(request: &DoorRunRequest) {
    if let Err(error) = cleanup_node_runtime_dir(&request.runtime_dir) {
        warn!(
            path = %request.runtime_dir.display(),
            "failed to clean door runtime directory: {error}"
        );
    }
}

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

async fn drain_remaining_output<R: AsyncRead + Unpin, T: Transport>(
    reader: &mut R,
    transport: &mut T,
    result: &mut DoorBridgeResult,
) -> ServeResult<()> {
    let mut bytes = Vec::new();
    reader.read_to_end(&mut bytes).await?;
    if !bytes.is_empty() {
        transport.write_all(&bytes).await?;
        result.bytes_out = result
            .bytes_out
            .saturating_add(i64::try_from(bytes.len()).unwrap_or(i64::MAX));
    }
    Ok(())
}

async fn terminate_child(child: &mut Child) -> ServeResult<Option<i32>> {
    match child.try_wait()? {
        Some(status) => Ok(status.code()),
        None => {
            let _ = child.start_kill();
            match time::timeout(DOOR_KILL_WAIT, child.wait()).await {
                Ok(Ok(status)) => Ok(status.code()),
                Ok(Err(error)) => Err(ServeError::Network(error)),
                Err(_) => Ok(None),
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
        Err(_) => text.as_bytes().to_vec(),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use oxidebbs_core::user::UserStatus;
    use oxidebbs_db::{UserRecord, find_door_run_by_id, insert_user};
    use oxidebbs_door::DryRunDoorRunner;
    use oxidebbs_telnet::LoopbackTransport;

    use super::*;
    use crate::config::{
        BoardConfig, DatabaseConfig, DoorDefConfig, DoorsConfig, FlowConfig, FtnConfig, MenuConfig,
        NodesConfig, PathsConfig, ScreenConfig, TelnetConfig, TerminalConfig,
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
        OxideConfig {
            board: BoardConfig {
                name: "Test BBS".to_string(),
                tagline: "Testing".to_string(),
                sysop_name: "Sysop".to_string(),
                timezone: "UTC".to_string(),
            },
            telnet: TelnetConfig::default(),
            database: DatabaseConfig::default(),
            paths: PathsConfig {
                ansi: runtime.join("ansi"),
                screens: runtime.join("screens"),
                doors: runtime.join("doors"),
                runtime: runtime.clone(),
                logs: working_dir.join("logs"),
            },
            nodes: NodesConfig::default(),
            terminal: TerminalConfig::default(),
            flow: FlowConfig::default(),
            screens: HashMap::<String, ScreenConfig>::new(),
            menus: HashMap::<String, MenuConfig>::new(),
            doors: DoorsConfig {
                enabled: true,
                default_runner: current_runner(),
                definitions: vec![DoorDefConfig {
                    key: "test".to_string(),
                    name: "Test Door".to_string(),
                    runner: current_runner(),
                    working_dir: working_dir.to_string_lossy().to_string(),
                    command: "TEST.EXE".to_string(),
                    drop_file: "DOOR.SYS".to_string(),
                    exclusive: false,
                    time_limit_minutes: 1,
                    enabled: true,
                }],
            },
            ftn: FtnConfig::default(),
        }
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
    fn validate_door_rejects_disabled_missing_runner_and_bad_dropfile() {
        let runtime = temp_dir("validate-runtime");
        let working_dir = temp_dir("validate-working");
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

        door.runner = current_runner();
        door.drop_file = "BAD.TXT".to_string();
        assert!(service.validate_door(&door, 1).is_err());

        let _ = fs::remove_dir_all(runtime);
    }

    #[test]
    fn dry_run_service_inserts_and_finishes_door_run() {
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
            .execute_with_runner(&DryRunDoorRunner, &test_user(), 1, &door, false)
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

    #[tokio::test]
    async fn bridge_forwards_caller_bytes_and_child_output() {
        let (mut transport, mut client) = LoopbackTransport::new();
        client.write_bytes(b"hello\n").expect("caller input");
        let runtime = ServerRuntime::new("test".to_string(), 1, 1, 30);
        runtime.mark_node_connected(
            1,
            "session".to_string(),
            "127.0.0.1:23".to_string(),
            "now".to_string(),
        );

        let child = Command::new("sh")
            .arg("-c")
            .arg("printf ready; IFS= read -r line; printf 'echo:%s\\n' \"$line\"")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn echo helper");

        let result = run_door_bridge(&mut transport, &runtime, 1, child, Duration::from_secs(2))
            .await
            .expect("bridge");

        let output = String::from_utf8_lossy(&client.read_output_bytes()).to_string();
        assert!(output.contains("ready"));
        assert!(output.contains("echo:hello"));
        assert!(result.bytes_in >= 6);
        assert!(result.bytes_out >= 15);
        assert_eq!(result.exit_code, Some(0));
    }

    #[tokio::test]
    async fn bridge_times_out_and_terminates_child() {
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
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn sleep helper");

        let result = run_door_bridge(
            &mut transport,
            &runtime,
            1,
            child,
            Duration::from_millis(50),
        )
        .await
        .expect("bridge");

        assert!(result.timed_out);
        assert!(result.disconnect_forced);
    }
}
