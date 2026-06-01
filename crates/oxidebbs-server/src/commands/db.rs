use std::fs;

use clap::Subcommand;
use serde_json::Value as JsonValue;

use crate::sysop_cli::{AppContext, CliError, CliResult, emit_ok, open_database, print_json};
use oxidebbs_db::{Db, list_messages, list_recent_sessions, list_users, read_schema_version};
use oxidebbs_db::{list_audit_events, list_door_definitions, list_door_runs, list_message_areas};

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

fn db_stats(db: &Db) -> CliResult<JsonValue> {
    Ok(serde_json::json!({
        "schema_version": read_schema_version(db)?,
        "users": db_scalar_i64(db, "SELECT COUNT(*) FROM users")?,
        "message_areas": db_scalar_i64(db, "SELECT COUNT(*) FROM message_areas")?,
        "messages": db_scalar_i64(db, "SELECT COUNT(*) FROM messages")?,
        "sessions": db_scalar_i64(db, "SELECT COUNT(*) FROM sessions")?,
        "active_sessions": db_scalar_i64(db, "SELECT COUNT(*) FROM sessions WHERE ended_at IS NULL")?,
        "doors": db_scalar_i64(db, "SELECT COUNT(*) FROM doors")?,
        "door_runs": db_scalar_i64(db, "SELECT COUNT(*) FROM door_runs")?,
        "audit_events": db_scalar_i64(db, "SELECT COUNT(*) FROM audit_events")?
    }))
}

fn print_stats(stats: &JsonValue) {
    if let Some(object) = stats.as_object() {
        for (key, value) in object {
            println!("{key}: {value}");
        }
    }
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
        "message_areas": list_message_areas(db)?.iter().map(area_json).collect::<Vec<_>>(),
        "messages": list_messages(db)?.iter().map(message_json).collect::<Vec<_>>(),
        "sessions": list_recent_sessions(db, 10_000)?.iter().map(session_json).collect::<Vec<_>>(),
        "doors": list_door_definitions(db)?.iter().map(door_json).collect::<Vec<_>>(),
        "door_runs": list_door_runs(db, 10_000)?.iter().map(door_run_json).collect::<Vec<_>>(),
        "audit_events": list_audit_events(db, 10_000)?.iter().map(audit_json).collect::<Vec<_>>()
    }))
}

#[derive(Subcommand)]
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
            let stats = db_stats(db.db())?;
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
            let stats = db_stats(db.db())?;
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
            require_json_format(&format)?;
            let _parsed: JsonValue = serde_json::from_str(&fs::read_to_string(&path)?)?;
            Err(CliError::Message(
                "db import is intentionally read-only in v1 until restore semantics are specified; JSON parsed successfully".to_string(),
            ))
        }
        DbCommand::Compact => Err(CliError::Message(
            "db compact is deferred until DecentDB exposes a supported compaction API".to_string(),
        )),
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
