use std::collections::HashSet;
use std::fs;
use std::path::Path;

use clap::Subcommand;
use serde::Deserialize;
use serde_json::Value as JsonValue;

use crate::{
    control::{ControlResponse, request_status},
    sysop_cli::{AppContext, CliError, CliResult, emit_ok, open_database, print_json},
};
use oxidebbs_db::{
    AuditEventRecord, AuthAttemptRecord, Db, DoorDefinitionRecord, DoorRunRecord,
    MessageAreaRecord, MessageRecord, SessionRecord, UserRecord, Value,
    insert_audit_event_preserving_record, insert_auth_attempt, insert_door_definition,
    insert_door_run, insert_message, insert_message_area, insert_session, insert_user,
    list_auth_attempts, list_door_definitions, list_message_areas, list_messages, list_users,
    read_schema_version,
};

#[derive(Debug, Clone, Deserialize)]
struct ImportSchema {
    schema_version: i64,
    users: Vec<ImportUserRecord>,
    #[serde(default)]
    auth_attempts: Vec<ImportAuthAttemptRecord>,
    message_areas: Vec<ImportMessageAreaRecord>,
    messages: Vec<ImportMessageRecord>,
    sessions: Vec<ImportSessionRecord>,
    doors: Vec<ImportDoorDefinitionRecord>,
    door_runs: Vec<ImportDoorRunRecord>,
    audit_events: Vec<ImportAuditEventRecord>,
}

#[derive(Debug, Clone, Deserialize)]
struct ImportAuthAttemptRecord {
    scope: String,
    scope_key: String,
    failed_count: i64,
    first_failed_at: Option<String>,
    last_failed_at: Option<String>,
    locked_until: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct ImportUserRecord {
    id: String,
    alias: String,
    real_name: String,
    email: Option<String>,
    password_hash: String,
    security_level: i64,
    is_sysop: bool,
    created_at: String,
    last_login_at: Option<String>,
    total_calls: i64,
    time_bank_minutes: i64,
    status: String,
}

#[derive(Debug, Clone, Deserialize)]
struct ImportMessageAreaRecord {
    id: String,
    key: String,
    name: String,
    description: String,
    kind: String,
    network_id: Option<String>,
    read_security_level: i64,
    post_security_level: i64,
    moderated: bool,
    enabled: bool,
}

#[derive(Debug, Clone, Deserialize)]
struct ImportMessageRecord {
    id: String,
    area_id: String,
    author_user_id: String,
    to_user_id: Option<String>,
    subject: String,
    body: String,
    created_at: String,
    reply_to_id: Option<String>,
    network_message_id: Option<String>,
    visibility: String,
}

#[derive(Debug, Clone, Deserialize)]
struct ImportSessionRecord {
    id: String,
    node_number: i64,
    user_id: Option<String>,
    transport: String,
    remote_address: String,
    remote_ip: Option<String>,
    remote_port: Option<i64>,
    started_at: String,
    ended_at: Option<String>,
    disconnect_reason: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct ImportDoorDefinitionRecord {
    id: String,
    key: String,
    name: String,
    runner: String,
    working_dir: String,
    command: String,
    drop_file: String,
    exclusive: bool,
    time_limit_minutes: i64,
    enabled: bool,
}

#[derive(Debug, Clone, Deserialize)]
struct ImportDoorRunRecord {
    id: String,
    door_id: String,
    user_id: String,
    node_number: i64,
    started_at: String,
    ended_at: Option<String>,
    exit_code: Option<i64>,
    timed_out: bool,
    disconnect_forced: bool,
    bytes_in: i64,
    bytes_out: i64,
}

#[derive(Debug, Clone, Deserialize)]
struct ImportAuditEventRecord {
    id: String,
    created_at: String,
    event_type: String,
    user_id: Option<String>,
    node_number: Option<i64>,
    details: String,
}

impl From<ImportUserRecord> for UserRecord {
    fn from(record: ImportUserRecord) -> Self {
        Self {
            id: record.id,
            alias: record.alias,
            real_name: record.real_name,
            email: record.email,
            password_hash: record.password_hash,
            security_level: record.security_level,
            is_sysop: record.is_sysop,
            created_at: record.created_at,
            last_login_at: record.last_login_at,
            total_calls: record.total_calls,
            time_bank_minutes: record.time_bank_minutes,
            status: record.status,
        }
    }
}

impl From<ImportAuthAttemptRecord> for AuthAttemptRecord {
    fn from(record: ImportAuthAttemptRecord) -> Self {
        Self {
            scope: record.scope,
            scope_key: record.scope_key,
            failed_count: record.failed_count,
            first_failed_at: record.first_failed_at,
            last_failed_at: record.last_failed_at,
            locked_until: record.locked_until,
        }
    }
}

impl From<ImportMessageAreaRecord> for MessageAreaRecord {
    fn from(record: ImportMessageAreaRecord) -> Self {
        Self {
            id: record.id,
            key: record.key,
            name: record.name,
            description: record.description,
            kind: record.kind,
            network_id: record.network_id,
            read_security_level: record.read_security_level,
            post_security_level: record.post_security_level,
            moderated: record.moderated,
            enabled: record.enabled,
        }
    }
}

impl From<ImportMessageRecord> for MessageRecord {
    fn from(record: ImportMessageRecord) -> Self {
        Self {
            id: record.id,
            area_id: record.area_id,
            author_user_id: record.author_user_id,
            to_user_id: record.to_user_id,
            subject: record.subject,
            body: record.body,
            created_at: record.created_at,
            reply_to_id: record.reply_to_id,
            network_message_id: record.network_message_id,
            visibility: record.visibility,
        }
    }
}

impl From<ImportSessionRecord> for SessionRecord {
    fn from(record: ImportSessionRecord) -> Self {
        Self {
            id: record.id,
            node_number: record.node_number,
            user_id: record.user_id,
            transport: record.transport,
            remote_address: record.remote_address,
            remote_ip: record.remote_ip,
            remote_port: record.remote_port,
            started_at: record.started_at,
            ended_at: record.ended_at,
            disconnect_reason: record.disconnect_reason,
        }
    }
}

impl From<ImportDoorDefinitionRecord> for DoorDefinitionRecord {
    fn from(record: ImportDoorDefinitionRecord) -> Self {
        Self {
            id: record.id,
            key: record.key,
            name: record.name,
            runner: record.runner,
            working_dir: record.working_dir,
            command: record.command,
            drop_file: record.drop_file,
            exclusive: record.exclusive,
            time_limit_minutes: record.time_limit_minutes,
            enabled: record.enabled,
        }
    }
}

impl From<ImportDoorRunRecord> for DoorRunRecord {
    fn from(record: ImportDoorRunRecord) -> Self {
        Self {
            id: record.id,
            door_id: record.door_id,
            user_id: record.user_id,
            node_number: record.node_number,
            started_at: record.started_at,
            ended_at: record.ended_at,
            exit_code: record.exit_code,
            timed_out: record.timed_out,
            disconnect_forced: record.disconnect_forced,
            bytes_in: record.bytes_in,
            bytes_out: record.bytes_out,
        }
    }
}

impl From<ImportAuditEventRecord> for AuditEventRecord {
    fn from(record: ImportAuditEventRecord) -> Self {
        Self {
            id: record.id,
            created_at: record.created_at,
            event_type: record.event_type,
            user_id: record.user_id,
            node_number: record.node_number,
            details: record.details,
        }
    }
}

fn db_scalar_i64(db: &Db, sql: &str) -> CliResult<i64> {
    let result = db.execute(sql)?;
    let value = result
        .rows()
        .first()
        .and_then(|row| row.values().first())
        .ok_or_else(|| CliError::Message(format!("query returned no scalar value: {sql}")))?;
    match value {
        oxidebbs_db::Value::Int64(value) => Ok(*value),
        other => Err(CliError::Message(format!(
            "query returned non-int scalar for {sql}: {other:?}"
        ))),
    }
}

fn text_value(value: &Value) -> String {
    match value {
        Value::Text(text) => text.clone(),
        _ => String::new(),
    }
}

fn opt_text_value(value: &Value) -> Option<String> {
    match value {
        Value::Text(text) => Some(text.clone()),
        _ => None,
    }
}

fn int_value(value: &Value) -> i64 {
    match value {
        Value::Int64(value) => *value,
        _ => 0,
    }
}

fn opt_int_value(value: &Value) -> Option<i64> {
    match value {
        Value::Int64(value) => Some(*value),
        _ => None,
    }
}

fn bool_value(value: &Value) -> bool {
    match value {
        Value::Bool(value) => *value,
        _ => false,
    }
}

fn list_all_sessions_for_export(db: &Db) -> CliResult<Vec<SessionRecord>> {
    let result = db.execute(
        "SELECT UUID_TO_STRING(id), node_number, UUID_TO_STRING(user_id), transport, remote_address, CAST(remote_ip AS TEXT), remote_port, CAST(started_at AS TEXT), CAST(ended_at AS TEXT), disconnect_reason
         FROM sessions ORDER BY started_at DESC",
    )?;
    Ok(result
        .rows()
        .iter()
        .map(|row| {
            let values = row.values();
            SessionRecord {
                id: text_value(&values[0]),
                node_number: int_value(&values[1]),
                user_id: opt_text_value(&values[2]),
                transport: text_value(&values[3]),
                remote_address: text_value(&values[4]),
                remote_ip: opt_text_value(&values[5]),
                remote_port: opt_int_value(&values[6]),
                started_at: text_value(&values[7]),
                ended_at: opt_text_value(&values[8]),
                disconnect_reason: opt_text_value(&values[9]),
            }
        })
        .collect())
}

fn list_all_door_runs_for_export(db: &Db) -> CliResult<Vec<DoorRunRecord>> {
    let result = db.execute(
        "SELECT UUID_TO_STRING(id), UUID_TO_STRING(door_id), UUID_TO_STRING(user_id), node_number, CAST(started_at AS TEXT), CAST(ended_at AS TEXT), exit_code, timed_out, disconnect_forced, bytes_in, bytes_out
         FROM door_runs ORDER BY started_at DESC",
    )?;
    Ok(result
        .rows()
        .iter()
        .map(|row| {
            let values = row.values();
            DoorRunRecord {
                id: text_value(&values[0]),
                door_id: text_value(&values[1]),
                user_id: text_value(&values[2]),
                node_number: int_value(&values[3]),
                started_at: text_value(&values[4]),
                ended_at: opt_text_value(&values[5]),
                exit_code: opt_int_value(&values[6]),
                timed_out: bool_value(&values[7]),
                disconnect_forced: bool_value(&values[8]),
                bytes_in: int_value(&values[9]),
                bytes_out: int_value(&values[10]),
            }
        })
        .collect())
}

fn list_all_audit_events_for_export(db: &Db) -> CliResult<Vec<AuditEventRecord>> {
    let result = db.execute(
        "SELECT UUID_TO_STRING(id), CAST(created_at AS TEXT), event_type, UUID_TO_STRING(user_id), node_number, details
         FROM audit_events ORDER BY created_at DESC",
    )?;
    Ok(result
        .rows()
        .iter()
        .map(|row| {
            let values = row.values();
            AuditEventRecord {
                id: text_value(&values[0]),
                created_at: text_value(&values[1]),
                event_type: text_value(&values[2]),
                user_id: opt_text_value(&values[3]),
                node_number: opt_int_value(&values[4]),
                details: text_value(&values[5]),
            }
        })
        .collect())
}

fn session_json(session: &oxidebbs_db::SessionRecord) -> JsonValue {
    serde_json::json!({
        "id": session.id,
        "node_number": session.node_number,
        "user_id": session.user_id,
        "transport": session.transport,
        "remote_address": session.remote_address,
        "remote_ip": session.remote_ip,
        "remote_port": session.remote_port,
        "started_at": session.started_at,
        "ended_at": session.ended_at,
        "disconnect_reason": session.disconnect_reason
    })
}

fn message_json(message: &oxidebbs_db::MessageRecord) -> JsonValue {
    serde_json::json!({
        "id": message.id,
        "area_id": message.area_id,
        "author_user_id": message.author_user_id,
        "to_user_id": message.to_user_id,
        "subject": message.subject,
        "body": message.body,
        "created_at": message.created_at,
        "reply_to_id": message.reply_to_id,
        "network_message_id": message.network_message_id,
        "visibility": message.visibility
    })
}

fn area_json(area: &oxidebbs_db::MessageAreaRecord) -> JsonValue {
    serde_json::json!({
        "id": area.id,
        "key": area.key,
        "name": area.name,
        "description": area.description,
        "kind": area.kind,
        "network_id": area.network_id,
        "read_security_level": area.read_security_level,
        "post_security_level": area.post_security_level,
        "moderated": area.moderated,
        "enabled": area.enabled
    })
}

fn user_json(user: &oxidebbs_db::UserRecord) -> JsonValue {
    serde_json::json!({
        "id": user.id,
        "alias": user.alias,
        "real_name": user.real_name,
        "email": user.email,
        "password_hash": user.password_hash,
        "security_level": user.security_level,
        "is_sysop": user.is_sysop,
        "created_at": user.created_at,
        "last_login_at": user.last_login_at,
        "total_calls": user.total_calls,
        "time_bank_minutes": user.time_bank_minutes,
        "status": user.status
    })
}

fn door_json(door: &oxidebbs_db::DoorDefinitionRecord) -> JsonValue {
    serde_json::json!({
        "id": door.id,
        "key": door.key,
        "name": door.name,
        "runner": door.runner,
        "working_dir": door.working_dir,
        "command": door.command,
        "drop_file": door.drop_file,
        "exclusive": door.exclusive,
        "time_limit_minutes": door.time_limit_minutes,
        "enabled": door.enabled
    })
}

fn door_run_json(run: &oxidebbs_db::DoorRunRecord) -> JsonValue {
    serde_json::json!({
        "id": run.id,
        "door_id": run.door_id,
        "user_id": run.user_id,
        "node_number": run.node_number,
        "started_at": run.started_at,
        "ended_at": run.ended_at,
        "exit_code": run.exit_code,
        "timed_out": run.timed_out,
        "disconnect_forced": run.disconnect_forced,
        "bytes_in": run.bytes_in,
        "bytes_out": run.bytes_out
    })
}

fn db_stats(db: &Db, active_sessions: i64) -> CliResult<JsonValue> {
    let open_sessions = db_scalar_i64(db, "SELECT COUNT(*) FROM sessions WHERE ended_at IS NULL")?;
    Ok(serde_json::json!({
        "schema_version": read_schema_version(db)?,
        "users": db_scalar_i64(db, "SELECT COUNT(*) FROM users")?,
        "message_areas": db_scalar_i64(db, "SELECT COUNT(*) FROM message_areas")?,
        "messages": db_scalar_i64(db, "SELECT COUNT(*) FROM messages")?,
        "sessions": db_scalar_i64(db, "SELECT COUNT(*) FROM sessions")?,
        "active_sessions": active_sessions,
        "open_sessions": open_sessions,
        "auth_attempts": db_scalar_i64(db, "SELECT COUNT(*) FROM auth_attempts")?,
        "doors": db_scalar_i64(db, "SELECT COUNT(*) FROM doors")?,
        "door_runs": db_scalar_i64(db, "SELECT COUNT(*) FROM door_runs")?,
        "audit_events": db_scalar_i64(db, "SELECT COUNT(*) FROM audit_events")?
    }))
}

fn live_active_session_count(ctx: &AppContext) -> CliResult<i64> {
    match request_status(&ctx.config.paths.runtime) {
        Ok(ControlResponse::Status { status, .. }) => Ok(i64::try_from(status.active_nodes)
            .map_err(|error| {
                CliError::Message(format!("active node count is out of range: {error}"))
            })?),
        Ok(ControlResponse::Error { error, .. }) => Err(CliError::Message(format!(
            "control socket reported error: {error}"
        ))),
        Ok(ControlResponse::Ok { .. }) => Err(CliError::Message(
            "control socket returned unexpected response to status request".to_string(),
        )),
        Ok(ControlResponse::Nodes { .. }) => Err(CliError::Message(
            "control socket returned unexpected response to status request".to_string(),
        )),
        Err(error) if error.is_unreachable() => Ok(0),
        Err(error) => Err(CliError::Message(format!("status request failed: {error}"))),
    }
}

fn print_stats(stats: &JsonValue) {
    if let Some(object) = stats.as_object() {
        for (key, value) in object {
            println!("{key}: {value}");
        }
    }
}

fn auth_attempt_json(attempt: &AuthAttemptRecord) -> JsonValue {
    serde_json::json!({
        "scope": attempt.scope,
        "scope_key": attempt.scope_key,
        "failed_count": attempt.failed_count,
        "first_failed_at": attempt.first_failed_at,
        "last_failed_at": attempt.last_failed_at,
        "locked_until": attempt.locked_until
    })
}

fn audit_json(event: &oxidebbs_db::AuditEventRecord) -> JsonValue {
    serde_json::json!({
        "id": event.id,
        "created_at": event.created_at,
        "event_type": event.event_type,
        "user_id": event.user_id,
        "node_number": event.node_number,
        "details": event.details
    })
}

fn db_export(db: &Db) -> CliResult<JsonValue> {
    Ok(serde_json::json!({
        "schema_version": read_schema_version(db)?,
        "users": list_users(db)?.iter().map(user_json).collect::<Vec<_>>(),
        "auth_attempts": list_auth_attempts(db)?.iter().map(auth_attempt_json).collect::<Vec<_>>(),
        "message_areas": list_message_areas(db)?.iter().map(area_json).collect::<Vec<_>>(),
        "messages": list_messages(db)?.iter().map(message_json).collect::<Vec<_>>(),
        "sessions": list_all_sessions_for_export(db)?.iter().map(session_json).collect::<Vec<_>>(),
        "doors": list_door_definitions(db)?.iter().map(door_json).collect::<Vec<_>>(),
        "door_runs": list_all_door_runs_for_export(db)?.iter().map(door_run_json).collect::<Vec<_>>(),
        "audit_events": list_all_audit_events_for_export(db)?.iter().map(audit_json).collect::<Vec<_>>()
    }))
}

fn parse_db_import(path: &Path, format: &str) -> CliResult<ImportSchema> {
    require_json_format(format)?;
    let body = fs::read_to_string(path)?;
    let payload = serde_json::from_str::<ImportSchema>(&body)?;
    Ok(payload)
}

fn ensure_import_target_is_schema_only(db: &Db) -> CliResult<()> {
    let system_rows = db_scalar_i64(
        db,
        "SELECT COUNT(*) FROM system_config WHERE key <> 'schema_version'",
    )?;
    if system_rows > 0 {
        return Err(CliError::Message(
            "import target contains unsupported system_config rows".to_string(),
        ));
    }

    for table in [
        "users",
        "auth_attempts",
        "message_areas",
        "messages",
        "sessions",
        "doors",
        "door_runs",
        "audit_events",
    ] {
        let count = db_scalar_i64(db, &format!("SELECT COUNT(*) FROM {table}"))?;
        if count > 0 {
            return Err(CliError::Message(format!(
                "import target must be schema-only; existing rows found in {table}"
            )));
        }
    }

    Ok(())
}

fn validate_import_payload(payload: &ImportSchema, current_schema_version: i64) -> CliResult<()> {
    if payload.schema_version != current_schema_version {
        return Err(CliError::Message(format!(
            "unsupported import schema version {}; expected {}",
            payload.schema_version, current_schema_version
        )));
    }

    let mut user_ids = HashSet::with_capacity(payload.users.len());
    for user in &payload.users {
        if !user_ids.insert(user.id.clone()) {
            return Err(CliError::Message(format!(
                "duplicate user id {} in import payload",
                user.id
            )));
        }
    }

    let mut area_ids = HashSet::with_capacity(payload.message_areas.len());
    for area in &payload.message_areas {
        if !area_ids.insert(area.id.clone()) {
            return Err(CliError::Message(format!(
                "duplicate message area id {} in import payload",
                area.id
            )));
        }
        if area.kind != "local" && area.kind != "echomail" && area.kind != "netmail" {
            return Err(CliError::Message(format!(
                "invalid message area kind {} for {}",
                area.kind, area.key
            )));
        }
    }

    let mut message_ids = HashSet::with_capacity(payload.messages.len());
    for message in &payload.messages {
        if !message_ids.insert(message.id.clone()) {
            return Err(CliError::Message(format!(
                "duplicate message id {} in import payload",
                message.id
            )));
        }
    }
    for message in &payload.messages {
        if !area_ids.contains(message.area_id.as_str()) {
            return Err(CliError::Message(format!(
                "message {} references missing area {}",
                message.id, message.area_id
            )));
        }
        if !user_ids.contains(message.author_user_id.as_str()) {
            return Err(CliError::Message(format!(
                "message {} references missing author {}",
                message.id, message.author_user_id
            )));
        }
        if let Some(to_user_id) = message.to_user_id.as_deref()
            && !user_ids.contains(to_user_id)
        {
            return Err(CliError::Message(format!(
                "message {} references missing recipient {}",
                message.id, to_user_id
            )));
        }
        if let Some(reply_to_id) = message.reply_to_id.as_deref()
            && !message_ids.contains(reply_to_id)
        {
            return Err(CliError::Message(format!(
                "message {} references missing parent message {}",
                message.id, reply_to_id
            )));
        }
    }

    for session in &payload.sessions {
        if let Some(user_id) = session.user_id.as_deref()
            && !user_ids.contains(user_id)
        {
            return Err(CliError::Message(format!(
                "session {} references missing user {}",
                session.id, user_id
            )));
        }
        if session.transport != "telnet" {
            return Err(CliError::Message(format!(
                "session {} has unsupported transport {}",
                session.id, session.transport
            )));
        }
    }

    let mut door_ids = HashSet::with_capacity(payload.doors.len());
    for door in &payload.doors {
        if !door_ids.insert(door.id.clone()) {
            return Err(CliError::Message(format!(
                "duplicate door id {} in import payload",
                door.id
            )));
        }
        if door.time_limit_minutes <= 0 {
            return Err(CliError::Message(format!(
                "door {} has invalid time limit {}",
                door.key, door.time_limit_minutes
            )));
        }
    }

    for run in &payload.door_runs {
        if !door_ids.contains(run.door_id.as_str()) {
            return Err(CliError::Message(format!(
                "door run {} references missing door {}",
                run.id, run.door_id
            )));
        }
        if !user_ids.contains(run.user_id.as_str()) {
            return Err(CliError::Message(format!(
                "door run {} references missing user {}",
                run.id, run.user_id
            )));
        }
        if run.node_number <= 0 {
            return Err(CliError::Message(format!(
                "door run {} has invalid node number {}",
                run.id, run.node_number
            )));
        }
        if run.bytes_in < 0 {
            return Err(CliError::Message(format!(
                "door run {} has invalid bytes_in {}",
                run.id, run.bytes_in
            )));
        }
        if run.bytes_out < 0 {
            return Err(CliError::Message(format!(
                "door run {} has invalid bytes_out {}",
                run.id, run.bytes_out
            )));
        }
    }

    for event in &payload.audit_events {
        if let Some(user_id) = event.user_id.as_deref()
            && !user_ids.contains(user_id)
        {
            return Err(CliError::Message(format!(
                "audit event {} references missing user {}",
                event.id, user_id
            )));
        }
    }

    Ok(())
}

fn insert_messages_with_replies(db: &Db, messages: &[MessageRecord]) -> CliResult<()> {
    for message in messages {
        let mut without_reply = message.clone();
        without_reply.reply_to_id = None;
        insert_message(db, &without_reply)?;
    }

    for message in messages {
        if let Some(reply_to_id) = &message.reply_to_id {
            db.execute_with_params(
                "UPDATE messages SET reply_to_id = UUID_PARSE($1) WHERE id = UUID_PARSE($2)",
                &[
                    Value::Text(reply_to_id.clone()),
                    Value::Text(message.id.clone()),
                ],
            )?;
        }
    }
    Ok(())
}

fn perform_db_import(db: &oxidebbs_db::OxideDb, payload: ImportSchema) -> CliResult<()> {
    let current_schema = read_schema_version(db.db())?;
    validate_import_payload(&payload, current_schema)?;
    ensure_import_target_is_schema_only(db.db())?;

    let users: Vec<UserRecord> = payload.users.into_iter().map(Into::into).collect();
    let auth_attempts: Vec<AuthAttemptRecord> =
        payload.auth_attempts.into_iter().map(Into::into).collect();
    let message_areas: Vec<MessageAreaRecord> =
        payload.message_areas.into_iter().map(Into::into).collect();
    let messages: Vec<MessageRecord> = payload.messages.into_iter().map(Into::into).collect();
    let sessions: Vec<SessionRecord> = payload.sessions.into_iter().map(Into::into).collect();
    let doors: Vec<DoorDefinitionRecord> = payload.doors.into_iter().map(Into::into).collect();
    let door_runs: Vec<DoorRunRecord> = payload.door_runs.into_iter().map(Into::into).collect();
    let audit_events: Vec<AuditEventRecord> =
        payload.audit_events.into_iter().map(Into::into).collect();

    let db = db.db();
    db.begin_transaction()?;
    let result = (|| -> CliResult<()> {
        for user in &users {
            insert_user(db, user)?;
        }
        for attempt in &auth_attempts {
            insert_auth_attempt(db, attempt)?;
        }
        for area in &message_areas {
            insert_message_area(db, area)?;
        }
        insert_messages_with_replies(db, &messages)?;
        for session in &sessions {
            insert_session(db, session)?;
        }
        for door in &doors {
            insert_door_definition(db, door)?;
        }
        for run in &door_runs {
            insert_door_run(db, run)?;
        }
        for event in &audit_events {
            insert_audit_event_preserving_record(db, event)?;
        }
        Ok(())
    })();

    match result {
        Ok(()) => {
            db.commit_transaction()?;
        }
        Err(error) => {
            let _ = db.rollback_transaction();
            return Err(error);
        }
    }

    Ok(())
}

fn db_compact() -> CliResult<()> {
    Err(CliError::Message(
        "db compact is unavailable: DecentDB does not expose a supported compaction API in this release".to_string(),
    ))
}

#[derive(Subcommand)]
// Lifecycle order keeps admin verbs grouped by maintenance workflow, with `verify` after restore primitives.
pub enum DbCommand {
    Init,
    Doctor,
    Stats,
    Backup {
        output_path: std::path::PathBuf,
    },
    Export {
        #[arg(long, default_value = "json")]
        format: String,
    },
    Import {
        #[arg(long, default_value = "json")]
        format: String,
        path: std::path::PathBuf,
    },
    Compact,
    Verify,
}

pub fn run_db(command: DbCommand, ctx: &AppContext) -> CliResult<()> {
    match command {
        DbCommand::Init => {
            let db = open_database(&ctx.config)?;
            emit_ok(
                ctx.json,
                "database initialized",
                JsonValue::Object(
                    serde_json::json!({
                        "path": ctx.config.database.path,
                        "schema_version": db.schema_version()?
                    })
                    .as_object()
                    .cloned()
                    .unwrap_or_default(),
                ),
            )
        }
        DbCommand::Doctor | DbCommand::Verify => {
            let db = open_database(&ctx.config)?;
            let version = db.schema_version()?;
            let stats = db_stats(db.db(), live_active_session_count(ctx)?)?;
            if ctx.json {
                print_json(
                    &serde_json::json!({"ok": true, "schema_version": version, "stats": stats}),
                )?;
            } else {
                println!("database OK: {}", ctx.config.database.path.display());
                println!("schema version: {version}");
                print_stats(&stats);
            }
            Ok(())
        }
        DbCommand::Stats => {
            let db = open_database(&ctx.config)?;
            let stats = db_stats(db.db(), live_active_session_count(ctx)?)?;
            if ctx.json {
                print_json(&stats)?;
            } else {
                print_stats(&stats);
            }
            Ok(())
        }
        DbCommand::Backup { output_path } => {
            let source = &ctx.config.database.path;
            if !source.exists() {
                return Err(CliError::Message(format!(
                    "database file {} does not exist",
                    source.display()
                )));
            }
            if let Some(parent) = output_path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(source, &output_path)?;
            emit_ok(
                ctx.json,
                "database backup complete",
                serde_json::json!({"output": output_path}),
            )
        }
        DbCommand::Export { format } => {
            require_json_format(&format)?;
            let db = open_database(&ctx.config)?;
            print_json(&db_export(db.db())?)?;
            Ok(())
        }
        DbCommand::Import { format, path } => {
            let payload = parse_db_import(&path, &format)?;
            let db = open_database(&ctx.config)?;
            let import_counts = (
                payload.users.len(),
                payload.auth_attempts.len(),
                payload.message_areas.len(),
                payload.messages.len(),
                payload.sessions.len(),
                payload.doors.len(),
                payload.door_runs.len(),
                payload.audit_events.len(),
            );
            perform_db_import(&db, payload)?;
            emit_ok(
                ctx.json,
                "database imported",
                serde_json::json!({
                    "schema_version": db.schema_version()?,
                    "users": import_counts.0,
                    "auth_attempts": import_counts.1,
                    "message_areas": import_counts.2,
                    "messages": import_counts.3,
                    "sessions": import_counts.4,
                    "doors": import_counts.5,
                    "door_runs": import_counts.6,
                    "audit_events": import_counts.7,
                }),
            )
        }
        DbCommand::Compact => db_compact(),
    }
}

fn require_json_format(format: &str) -> CliResult<()> {
    if format.eq_ignore_ascii_case("json") {
        Ok(())
    } else {
        Err(CliError::Message(format!(
            "unsupported format {format:?}; only json is supported"
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxidebbs_db::{
        SCHEMA_VERSION, find_message_by_id, insert_audit_event, insert_door_definition,
        insert_door_run, insert_message, insert_message_area, insert_session, insert_user,
        list_users,
    };
    use std::time::{SystemTime, UNIX_EPOCH};

    fn make_temp_path(tag: &str) -> std::path::PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!(
            "oxidebbs-phase5-{tag}-{}.json",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system time should be valid")
                .as_nanos()
        ));
        path
    }

    fn test_db() -> oxidebbs_db::OxideDb {
        let db = oxidebbs_db::OxideDb::open_memory().expect("open in-memory DB");
        assert_eq!(db.schema_version().expect("schema version"), SCHEMA_VERSION);
        db
    }

    fn seed_user(id: &str, alias: &str, is_sysop: bool) -> UserRecord {
        UserRecord {
            id: id.to_string(),
            alias: alias.to_string(),
            real_name: format!("{alias} User"),
            email: Some(format!("{alias}@example.com")),
            password_hash: "hash".to_string(),
            security_level: 10,
            is_sysop,
            created_at: "2026-01-01T00:00:00.000000Z".to_string(),
            last_login_at: None,
            total_calls: 0,
            time_bank_minutes: 0,
            status: "active".to_string(),
        }
    }

    fn seed_area(id: &str) -> MessageAreaRecord {
        MessageAreaRecord {
            id: id.to_string(),
            key: if id.ends_with("101") {
                "general".to_string()
            } else {
                "games".to_string()
            },
            name: "General".to_string(),
            description: "desc".to_string(),
            kind: "local".to_string(),
            network_id: None,
            read_security_level: 0,
            post_security_level: 10,
            moderated: false,
            enabled: true,
        }
    }

    fn seed_message(
        id: &str,
        area_id: &str,
        author_id: &str,
        reply_to_id: Option<&str>,
    ) -> MessageRecord {
        MessageRecord {
            id: id.to_string(),
            area_id: area_id.to_string(),
            author_user_id: author_id.to_string(),
            to_user_id: None,
            subject: "Hello".to_string(),
            body: "Body".to_string(),
            created_at: "2026-01-01T00:00:00.000000Z".to_string(),
            reply_to_id: reply_to_id.map(std::string::ToString::to_string),
            network_message_id: None,
            visibility: "normal".to_string(),
        }
    }

    fn table_counts(db: &oxidebbs_db::OxideDb) -> (i64, i64, i64, i64, i64, i64, i64, i64) {
        (
            db_scalar_i64(db.db(), "SELECT COUNT(*) FROM users").expect("count users"),
            db_scalar_i64(db.db(), "SELECT COUNT(*) FROM auth_attempts")
                .expect("count auth attempts"),
            db_scalar_i64(db.db(), "SELECT COUNT(*) FROM message_areas").expect("count areas"),
            db_scalar_i64(db.db(), "SELECT COUNT(*) FROM messages").expect("count messages"),
            db_scalar_i64(db.db(), "SELECT COUNT(*) FROM sessions").expect("count sessions"),
            db_scalar_i64(db.db(), "SELECT COUNT(*) FROM doors").expect("count doors"),
            db_scalar_i64(db.db(), "SELECT COUNT(*) FROM door_runs").expect("count runs"),
            db_scalar_i64(db.db(), "SELECT COUNT(*) FROM audit_events").expect("count events"),
        )
    }

    fn export_payload(db: &oxidebbs_db::OxideDb) -> ImportSchema {
        let value = db_export(db.db()).expect("export");
        serde_json::from_value(value).expect("parse export payload")
    }

    fn seeded_source_db() -> (oxidebbs_db::OxideDb, ImportSchema) {
        let source = test_db();
        let user = seed_user("00000000-0000-4000-8000-000000000001", "alice", true);
        let area = seed_area("00000000-0000-4000-8000-000000000101");
        let root_message = seed_message(
            "00000000-0000-4000-8000-000000000201",
            &area.id,
            &user.id,
            None,
        );
        let reply_message = seed_message(
            "00000000-0000-4000-8000-000000000202",
            &area.id,
            &user.id,
            Some(&root_message.id),
        );
        let session = SessionRecord {
            id: "00000000-0000-4000-8000-000000000301".to_string(),
            node_number: 1,
            user_id: Some(user.id.clone()),
            transport: "telnet".to_string(),
            remote_address: "127.0.0.1:2323".to_string(),
            remote_ip: Some("127.0.0.1".to_string()),
            remote_port: Some(2323),
            started_at: "2026-01-01T00:00:00.000000Z".to_string(),
            ended_at: None,
            disconnect_reason: None,
        };
        let door = DoorDefinitionRecord {
            id: "00000000-0000-4000-8000-000000000401".to_string(),
            key: "echo".to_string(),
            name: "Echo".to_string(),
            runner: "dosemu".to_string(),
            working_dir: "/tmp".to_string(),
            command: "ECHO.EXE".to_string(),
            drop_file: "DOOR.SYS".to_string(),
            exclusive: false,
            time_limit_minutes: 30,
            enabled: true,
        };
        let door_run = DoorRunRecord {
            id: "00000000-0000-4000-8000-000000000501".to_string(),
            door_id: door.id.clone(),
            user_id: user.id.clone(),
            node_number: 1,
            started_at: "2026-01-01T00:10:00.000000Z".to_string(),
            ended_at: Some("2026-01-01T00:20:00.000000Z".to_string()),
            exit_code: Some(0),
            timed_out: false,
            disconnect_forced: false,
            bytes_in: 1,
            bytes_out: 2,
        };
        let audit_event = ImportAuditEventRecord {
            id: "00000000-0000-4000-8000-000000000601".to_string(),
            created_at: "2026-01-01T00:00:00.000000Z".to_string(),
            event_type: "import_fixture".to_string(),
            user_id: Some(user.id.clone()),
            node_number: Some(1),
            details: "created fixture".to_string(),
        };

        insert_user(source.db(), &user).expect("seed user");
        insert_message_area(source.db(), &area).expect("seed area");
        insert_message(source.db(), &root_message).expect("seed root message");
        insert_message(source.db(), &reply_message).expect("seed reply");
        insert_session(source.db(), &session).expect("seed session");
        insert_door_definition(source.db(), &door).expect("seed door");
        insert_door_run(source.db(), &door_run).expect("seed run");
        insert_audit_event(source.db(), &audit_event.into()).expect("seed audit");

        let payload = export_payload(&source);
        (source, payload)
    }

    #[test]
    fn db_init_target_is_schema_only_for_import() {
        let db = test_db();
        assert_eq!(table_counts(&db), (0, 0, 0, 0, 0, 0, 0, 0));
        ensure_import_target_is_schema_only(db.db()).expect("schema-only target");
    }

    #[test]
    fn import_restores_schema_only_target() {
        let (_, payload) = seeded_source_db();
        let target = test_db();
        perform_db_import(&target, payload).expect("import");
        assert_eq!(table_counts(&target), (1, 0, 1, 2, 1, 1, 1, 1));
        let users = list_users(target.db()).expect("list users");
        assert_eq!(users[0].alias, "alice");
        assert_eq!(users[0].password_hash, "hash");
        let reply = find_message_by_id(target.db(), "00000000-0000-4000-8000-000000000202")
            .expect("find reply")
            .expect("reply exists");
        assert_eq!(
            reply.reply_to_id.as_deref(),
            Some("00000000-0000-4000-8000-000000000201")
        );
    }

    #[test]
    fn import_rejects_schema_version_mismatch() {
        let (_, mut payload) = seeded_source_db();
        payload.schema_version = 2;
        let target = test_db();
        let before = table_counts(&target);
        let err = perform_db_import(&target, payload).expect_err("expected schema mismatch");
        let after = table_counts(&target);
        assert!(
            err.to_string()
                .contains("unsupported import schema version")
        );
        assert_eq!(before, after);
    }

    #[test]
    fn import_rejects_populated_target() {
        let (_, payload) = seeded_source_db();
        let target = test_db();
        let seed = seed_user("00000000-0000-4000-8000-000000000777", "sysop", true);
        insert_user(target.db(), &seed).expect("seed target sysop");
        let event = AuditEventRecord {
            id: "00000000-0000-4000-8000-000000000778".to_string(),
            created_at: "2026-01-01T01:00:00.000000Z".to_string(),
            event_type: "seeded".to_string(),
            user_id: Some(seed.id),
            node_number: Some(1),
            details: "seeded".to_string(),
        };
        insert_audit_event(target.db(), &event).expect("seed target audit");
        let before = table_counts(&target);
        let err =
            perform_db_import(&target, payload).expect_err("expected populated target rejection");
        let after = table_counts(&target);
        assert!(
            err.to_string()
                .contains("import target must be schema-only")
        );
        assert_eq!(before, after);
    }

    #[test]
    fn import_rejects_malformed_json() {
        let path = make_temp_path("malformed");
        fs::write(&path, "{bad json").expect("write");
        let err = parse_db_import(&path, "json").expect_err("expected invalid json to fail");
        assert!(!err.to_string().is_empty());
        let _ = fs::remove_file(path);
    }

    #[test]
    fn compact_reports_explicit_unsupported_error() {
        let err = db_compact().expect_err("compact remains unsupported");
        assert!(err.to_string().contains("DecentDB does not expose"));
    }

    #[test]
    fn db_stats_json_shape_matches_contract() {
        let db = test_db();
        let stats = db_stats(db.db(), 0).expect("stats");
        let obj = stats.as_object().expect("stats object");
        assert!(obj.contains_key("schema_version"));
        assert!(obj.contains_key("users"));
        assert!(obj.contains_key("message_areas"));
        assert!(obj.contains_key("messages"));
        assert!(obj.contains_key("sessions"));
        assert!(obj.contains_key("active_sessions"));
        assert!(obj.contains_key("open_sessions"));
        assert!(obj.contains_key("auth_attempts"));
        assert!(obj.contains_key("doors"));
        assert!(obj.contains_key("door_runs"));
        assert!(obj.contains_key("audit_events"));
    }

    #[test]
    fn db_stats_distinguishes_live_active_from_open_session_rows() {
        let db = test_db();
        let user = seed_user("00000000-0000-4000-8000-000000000701", "sysop", true);
        insert_user(db.db(), &user).expect("seed user");
        insert_session(
            db.db(),
            &SessionRecord {
                id: "00000000-0000-4000-8000-000000000702".to_string(),
                node_number: 1,
                user_id: Some(user.id),
                transport: "telnet".to_string(),
                remote_address: "127.0.0.1:2323".to_string(),
                remote_ip: Some("127.0.0.1".to_string()),
                remote_port: Some(2323),
                started_at: "2026-01-01T00:00:00.000000Z".to_string(),
                ended_at: None,
                disconnect_reason: None,
            },
        )
        .expect("seed open session");

        let stats = db_stats(db.db(), 0).expect("stats");
        let obj = stats.as_object().expect("stats object");

        assert_eq!(obj.get("active_sessions"), Some(&JsonValue::from(0)));
        assert_eq!(obj.get("open_sessions"), Some(&JsonValue::from(1)));
    }

    #[test]
    fn import_rejects_unsupported_format() {
        let err = require_json_format("yaml").expect_err("unsupported import format");
        assert!(err.to_string().contains("unsupported format"));
    }
}
