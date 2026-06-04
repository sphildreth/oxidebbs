use clap::Subcommand;
use serde_json::json;

use crate::sysop_cli::{
    AppContext, CliResult, current_timestamp, open_database, print_audit_events, print_json,
    require_user,
};
use oxidebbs_db::AuditEventRecord;
use oxidebbs_db::list_audit_events;
use oxidebbs_db::{insert_audit_event, list_audit_events_for_user, purge_audit_events_older_than};

#[derive(Subcommand)]
pub enum AuditCommand {
    Recent {
        #[arg(short, long, default_value_t = 25)]
        limit: i64,
    },
    User {
        alias_or_id: String,
        #[arg(short, long, default_value_t = 25)]
        limit: i64,
    },
    Node {
        node_number: i64,
        #[arg(short, long, default_value_t = 25)]
        limit: i64,
    },
    Door {
        door_key: String,
        #[arg(short, long, default_value_t = 25)]
        limit: i64,
    },
    PurgeBefore {
        timestamp: String,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        json: bool,
    },
    PurgeRetention {
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        json: bool,
    },
}

pub fn run_audit(command: AuditCommand, ctx: &AppContext) -> CliResult<()> {
    let db = open_database(&ctx.config)?;
    match command {
        AuditCommand::Recent { limit } => {
            let events = list_audit_events(db.db(), limit)?;
            print_audit_events(&events, ctx.json)
        }
        AuditCommand::User { alias_or_id, limit } => {
            let user = require_user(&db, &alias_or_id)?;
            let events = list_audit_events_for_user(db.db(), &user.id, limit)?;
            print_audit_events(&events, ctx.json)
        }
        AuditCommand::Node { node_number, limit } => {
            let events: Vec<_> = list_audit_events(db.db(), limit)?
                .into_iter()
                .filter(|event| event.node_number == Some(node_number))
                .collect();
            print_audit_events(&events, ctx.json)
        }
        AuditCommand::Door { door_key, limit } => {
            let needle = door_key.to_ascii_lowercase();
            let events: Vec<_> = list_audit_events(db.db(), limit)?
                .into_iter()
                .filter(|event| event.details.to_ascii_lowercase().contains(&needle))
                .collect();
            print_audit_events(&events, ctx.json)
        }
        AuditCommand::PurgeBefore {
            timestamp,
            dry_run,
            json,
        } => {
            let json_output = ctx.json || json;
            if dry_run {
                let count = audit_event_count_before(db.db(), &timestamp)?;
                if json_output {
                    print_json(&json!({"deleted": count, "dry_run": true}))?;
                } else {
                    println!("[dry-run] would delete {count} audit events older than {timestamp}");
                }
            } else {
                let deleted = purge_audit_events_older_than(db.db(), &timestamp)
                    .map_err(|e| crate::sysop_cli::CliError::Message(e.to_string()))?;
                let event_id = crate::sysop_cli::generated_uuid(&db)?;
                let now = current_timestamp(&db)?;
                let audit_event = AuditEventRecord {
                    id: event_id,
                    created_at: now,
                    event_type: "audit:purge:before".to_string(),
                    user_id: None,
                    node_number: None,
                    details: format!("purged {deleted} events older than {timestamp}"),
                };
                insert_audit_event(db.db(), &audit_event)
                    .map_err(|e| crate::sysop_cli::CliError::Message(e.to_string()))?;
                if json_output {
                    print_json(&json!({"deleted": deleted, "dry_run": false}))?;
                } else {
                    println!("purged {deleted} audit events older than {timestamp}");
                }
            }
            Ok(())
        }
        AuditCommand::PurgeRetention { dry_run, json } => {
            let json_output = ctx.json || json;
            let retention_days = ctx.config.audit.retention_days;
            let now = current_timestamp(&db)?;
            let timestamp_calc = db.db().execute(&format!(
                "SELECT CAST(CAST(NOW() AS TIMESTAMPTZ) - INTERVAL '{retention_days} days' AS TEXT)"
            ))
            .map_err(|e| crate::sysop_cli::CliError::Message(e.to_string()))?;
            let cutoff_str = timestamp_calc
                .rows()
                .first()
                .and_then(|row| row.values().first())
                .and_then(|v| match v {
                    oxidebbs_db::Value::Text(s) => Some(s.clone()),
                    _ => None,
                })
                .unwrap_or(now);

            if dry_run {
                let count = audit_event_count_before(db.db(), &cutoff_str)?;
                if json_output {
                    print_json(&json!({"deleted": count, "dry_run": true}))?;
                } else {
                    println!(
                        "[dry-run] would delete {count} audit events older than {retention_days} days (before {cutoff_str})"
                    );
                }
            } else {
                let deleted = purge_audit_events_older_than(db.db(), &cutoff_str)
                    .map_err(|e| crate::sysop_cli::CliError::Message(e.to_string()))?;
                let event_id = crate::sysop_cli::generated_uuid(&db)?;
                let now2 = current_timestamp(&db)?;
                let audit_event = AuditEventRecord {
                    id: event_id,
                    created_at: now2,
                    event_type: "audit:purge:retention".to_string(),
                    user_id: None,
                    node_number: None,
                    details: format!(
                        "retention purge ({retention_days} days) deleted {deleted} events"
                    ),
                };
                insert_audit_event(db.db(), &audit_event)
                    .map_err(|e| crate::sysop_cli::CliError::Message(e.to_string()))?;
                if json_output {
                    print_json(&json!({"deleted": deleted, "dry_run": false}))?;
                } else {
                    println!("retention purge ({retention_days} days) deleted {deleted} events");
                }
            }
            Ok(())
        }
    }
}

fn audit_event_count_before(
    db: &oxidebbs_db::Db,
    cutoff: &str,
) -> Result<i64, crate::sysop_cli::CliError> {
    let result = db
        .execute_with_params(
            "SELECT COUNT(*) FROM audit_events WHERE created_at < CAST($1 AS TIMESTAMPTZ)",
            &[oxidebbs_db::Value::Text(cutoff.to_string())],
        )
        .map_err(|e| crate::sysop_cli::CliError::Message(e.to_string()))?;
    Ok(result
        .rows()
        .first()
        .and_then(|row| row.values().first())
        .and_then(|v| match v {
            oxidebbs_db::Value::Int64(n) => Some(*n),
            _ => None,
        })
        .unwrap_or(0))
}
