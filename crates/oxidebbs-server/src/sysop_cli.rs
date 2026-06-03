use std::io::{self, Write};
use std::path::PathBuf;

use argon2::password_hash::{PasswordHash, PasswordHasher, SaltString};
use argon2::{Algorithm, Argon2, Params, PasswordVerifier as Argon2PasswordVerifier, Version};
use clap::{Parser, Subcommand};
use rand_core::OsRng;
use serde_json::{Value as JsonValue, json};
use thiserror::Error;

use oxidebbs_db::{
    AuditEventRecord, MessageAreaRecord, OxideDb, SessionRecord, UserRecord, Value,
    find_active_session_by_node, find_message_area_by_key, find_message_by_id,
    find_user_by_alias_ci, find_user_by_id, insert_audit_event,
};

use crate::config::{DoorDefConfig, OxideConfig, normalize_database_path};
use crate::serve;

use crate::commands::{
    AnsiCommand, AuditCommand, ConfigCommand, DbCommand, DoorsCommand, LogsCommand,
    MessagesCommand, NodesCommand, ServeArgs, SetupArgs, UsersCommand, run_ansi, run_audit,
    run_check, run_config, run_config_set, run_db, run_doors, run_logs, run_messages, run_nodes,
    run_serve, run_setup_command, run_status, run_sysop_preview, run_users,
};

pub(crate) type CliResult<T> = Result<T, CliError>;

#[derive(Debug, Error)]
pub(crate) enum CliError {
    #[error("{0}")]
    Message(String),

    #[error(transparent)]
    Io(#[from] io::Error),

    #[error(transparent)]
    Config(#[from] crate::config::ConfigError),

    #[error(transparent)]
    Database(#[from] oxidebbs_db::DbError),

    #[error(transparent)]
    Door(#[from] oxidebbs_door::DoorError),

    #[error(transparent)]
    Json(#[from] serde_json::Error),

    #[error(transparent)]
    TomlDe(#[from] toml::de::Error),

    #[error(transparent)]
    TomlSer(#[from] toml::ser::Error),

    #[error(transparent)]
    Serve(#[from] serve::ServeError),
}

#[derive(Parser)]
#[command(
    name = "oxidebbs",
    about = "OxideBBS - A modern BBS server implementation",
    version
)]
struct Cli {
    /// Path to the TOML configuration file
    #[arg(short, long, global = true)]
    config: Option<PathBuf>,

    /// Override the DecentDB data path from the config file
    #[arg(long, global = true)]
    data: Option<PathBuf>,

    /// Output machine-readable JSON where supported
    #[arg(long, global = true)]
    json: bool,

    /// Disable colored local terminal output
    #[arg(long, global = true)]
    no_color: bool,

    /// Increase local log verbosity
    #[arg(short, long, action = clap::ArgAction::Count, global = true)]
    verbose: u8,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
// Top-level order intentionally matches CLI contract and leaves the Clap help command at the bottom.
enum Command {
    /// Inspect ANSI/CP437 screen assets
    Ansi {
        #[command(subcommand)]
        command: AnsiCommand,
    },

    /// Read audit events
    Audit {
        #[command(subcommand)]
        command: AuditCommand,
    },

    /// Validate the configuration file and runtime paths
    Check,

    /// Inspect or edit configuration values
    Config {
        #[command(subcommand)]
        command: ConfigCommand,
    },

    /// Inspect and maintain DecentDB storage
    Db {
        #[command(subcommand)]
        command: DbCommand,
    },

    /// Inspect and test DOS door definitions
    Doors {
        #[command(subcommand)]
        command: DoorsCommand,
    },

    /// Read local log files
    Logs {
        #[command(subcommand)]
        command: LogsCommand,
    },

    /// Manage local message areas and messages
    Messages {
        #[command(subcommand)]
        command: MessagesCommand,
    },

    /// Inspect and control node/session state
    Nodes {
        #[command(subcommand)]
        command: NodesCommand,
    },

    /// Start the BBS server
    Serve(ServeArgs),

    /// Create a starter board installation
    Setup(SetupArgs),

    /// Show board status
    Status,

    /// Render a local sysop console preview
    Sysop,

    /// Manage users
    Users {
        #[command(subcommand)]
        command: UsersCommand,
    },
}

pub(crate) struct AppContext {
    pub(crate) config_path: PathBuf,
    pub(crate) config: OxideConfig,
    pub(crate) json: bool,
}

pub async fn run() -> CliResult<()> {
    let cli = Cli::parse();
    init_logging(cli.verbose);

    let config_path = cli.config.unwrap_or_else(default_config_path);
    let command = cli.command.unwrap_or(Command::Serve(ServeArgs::default()));

    match command {
        Command::Setup(args) => return run_setup_command(args, cli.data, cli.json),
        Command::Config {
            command: ConfigCommand::Set { key, value },
        } => return run_config_set(&config_path, &key, &value, cli.json),
        _ => {}
    }

    let mut config = OxideConfig::load(&config_path)?;
    if let Some(data_path) = cli.data {
        config.database.path = normalize_database_path(data_path);
    }

    let ctx = AppContext {
        config_path,
        config,
        json: cli.json,
    };

    match command {
        Command::Serve(args) => run_serve(args, &ctx).await,
        Command::Check => run_check(&ctx),
        Command::Status => run_status(&ctx),
        Command::Users { command } => run_users(command, &ctx),
        Command::Nodes { command } => run_nodes(command, &ctx),
        Command::Messages { command } => run_messages(command, &ctx),
        Command::Doors { command } => run_doors(command, &ctx),
        Command::Ansi { command } => run_ansi(command, &ctx),
        Command::Db { command } => run_db(command, &ctx),
        Command::Logs { command } => run_logs(command, &ctx),
        Command::Audit { command } => run_audit(command, &ctx),
        Command::Config { command } => run_config(command, &ctx),
        Command::Sysop => run_sysop_preview(&ctx),
        Command::Setup(_) => unreachable!("setup is handled before config load"),
    }
}

pub(crate) fn init_logging(verbose: u8) {
    let fallback = match verbose {
        0 => "info",
        1 => "debug",
        _ => "trace",
    };
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(fallback)),
        )
        .init();
}

pub(crate) fn default_config_path() -> PathBuf {
    let local_config = PathBuf::from("config/oxidebbs.toml");
    if local_config.exists() {
        return local_config;
    }

    PathBuf::from("config/oxidebbs.example.toml")
}

pub(crate) fn open_database(config: &OxideConfig) -> CliResult<OxideDb> {
    Ok(OxideDb::open_or_create(&config.database.path)?)
}

pub(crate) fn db_scalar_text(db: &OxideDb, sql: &str) -> CliResult<String> {
    let result = db.db().execute(sql)?;
    let value = result
        .rows()
        .first()
        .and_then(|row| row.values().first())
        .ok_or_else(|| CliError::Message(format!("query returned no scalar value: {sql}")))?;
    match value {
        Value::Text(value) => Ok(value.clone()),
        other => Err(CliError::Message(format!(
            "query returned non-text scalar for {sql}: {other:?}"
        ))),
    }
}

pub(crate) fn generated_uuid(db: &OxideDb) -> CliResult<String> {
    db_scalar_text(db, "SELECT UUID_TO_STRING(GEN_RANDOM_UUID())")
}

pub(crate) fn current_timestamp(db: &OxideDb) -> CliResult<String> {
    db_scalar_text(db, "SELECT CAST(NOW() AS TEXT)")
}

pub(crate) fn hash_password(password: &str) -> CliResult<String> {
    let salt = SaltString::generate(&mut OsRng);
    let password_hash = default_argon2()?
        .hash_password(password.as_bytes(), &salt)
        .map_err(|error| CliError::Message(format!("password hashing failed: {error}")))?;
    Ok(password_hash.to_string())
}

#[allow(dead_code)]
pub(crate) fn verify_password(password: &str, password_hash: &str) -> bool {
    let Ok(parsed_hash) = PasswordHash::new(password_hash) else {
        return false;
    };
    let Ok(argon2) = default_argon2() else {
        return false;
    };
    argon2
        .verify_password(password.as_bytes(), &parsed_hash)
        .is_ok()
}

fn default_argon2() -> CliResult<Argon2<'static>> {
    let params = Params::new(19_456, 2, 1, None)
        .map_err(|error| CliError::Message(format!("invalid Argon2 parameters: {error}")))?;
    Ok(Argon2::new(Algorithm::Argon2id, Version::V0x13, params))
}

pub(crate) fn audit(
    db: &OxideDb,
    event_type: &str,
    user_id: Option<&str>,
    node_number: Option<i64>,
    details: &str,
) -> CliResult<()> {
    insert_audit_event(
        db.db(),
        &AuditEventRecord {
            id: generated_uuid(db)?,
            created_at: current_timestamp(db)?,
            event_type: event_type.to_string(),
            user_id: user_id.map(ToOwned::to_owned),
            node_number,
            details: details.to_string(),
        },
    )?;
    Ok(())
}

pub(crate) fn require_user(db: &OxideDb, alias_or_id: &str) -> CliResult<UserRecord> {
    if let Some(user) = find_user_by_alias_ci(db.db(), alias_or_id)? {
        return Ok(user);
    }
    if looks_like_uuid(alias_or_id)
        && let Some(user) = find_user_by_id(db.db(), alias_or_id)?
    {
        return Ok(user);
    }
    Err(CliError::Message(format!(
        "user {alias_or_id:?} was not found"
    )))
}

pub(crate) fn looks_like_uuid(value: &str) -> bool {
    value.len() == 36
        && value
            .chars()
            .all(|character| character.is_ascii_hexdigit() || character == '-')
}

pub(crate) fn require_active_session(db: &OxideDb, node_number: i64) -> CliResult<SessionRecord> {
    find_active_session_by_node(db.db(), node_number)?.ok_or_else(|| {
        CliError::Message(format!(
            "node {node_number} does not have an active session"
        ))
    })
}

pub(crate) fn require_message_area(db: &OxideDb, key: &str) -> CliResult<MessageAreaRecord> {
    find_message_area_by_key(db.db(), key)?
        .ok_or_else(|| CliError::Message(format!("message area {key:?} was not found")))
}

pub(crate) fn require_message(
    db: &OxideDb,
    message_id: &str,
) -> CliResult<oxidebbs_db::MessageRecord> {
    find_message_by_id(db.db(), message_id)?
        .ok_or_else(|| CliError::Message(format!("message {message_id:?} was not found")))
}

pub(crate) fn require_config_door<'a>(
    config: &'a OxideConfig,
    key: &str,
) -> CliResult<&'a DoorDefConfig> {
    config
        .doors
        .definitions
        .iter()
        .find(|door| door.key.eq_ignore_ascii_case(key))
        .ok_or_else(|| CliError::Message(format!("door {key:?} was not found in config")))
}

pub(crate) fn user_json(user: &UserRecord) -> JsonValue {
    json!({
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

pub(crate) fn session_json(session: &SessionRecord) -> JsonValue {
    json!({
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

pub(crate) fn area_json(area: &MessageAreaRecord) -> JsonValue {
    json!({
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

pub(crate) fn message_json(message: &oxidebbs_db::MessageRecord) -> JsonValue {
    json!({
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

pub(crate) fn audit_json(event: &AuditEventRecord) -> JsonValue {
    json!({
        "id": event.id,
        "created_at": event.created_at,
        "event_type": event.event_type,
        "user_id": event.user_id,
        "node_number": event.node_number,
        "details": event.details
    })
}

pub(crate) fn print_json(value: &JsonValue) -> CliResult<()> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}

pub(crate) fn emit_ok(json_output: bool, message: &str, details: JsonValue) -> CliResult<()> {
    if json_output {
        print_json(&json!({"ok": true, "message": message, "details": details}))?;
    } else {
        println!("{message}");
    }
    Ok(())
}

pub(crate) fn print_user(user: &UserRecord) {
    println!("id: {}", user.id);
    println!("alias: {}", user.alias);
    println!("real name: {}", user.real_name);
    println!("email: {:?}", user.email);
    println!("security level: {}", user.security_level);
    println!("sysop: {}", user.is_sysop);
    println!("created: {}", user.created_at);
    println!("last login: {:?}", user.last_login_at);
    println!("total calls: {}", user.total_calls);
    println!("time bank minutes: {}", user.time_bank_minutes);
    println!("status: {}", user.status);
}

pub(crate) fn print_session(session: &SessionRecord) {
    println!("session: {}", session.id);
    println!("node: {}", session.node_number);
    println!("user: {:?}", session.user_id);
    println!("transport: {}", session.transport);
    println!("remote: {}", session.remote_address);
    println!("started: {}", session.started_at);
    println!("ended: {:?}", session.ended_at);
    println!("disconnect reason: {:?}", session.disconnect_reason);
}

pub(crate) fn print_sessions(sessions: &[SessionRecord], json_output: bool) -> CliResult<()> {
    if json_output {
        print_json(&JsonValue::Array(
            sessions.iter().map(session_json).collect(),
        ))?;
    } else {
        for session in sessions {
            println!(
                "{}\tnode={}\tuser={:?}\tstarted={}\tended={:?}",
                session.id,
                session.node_number,
                session.user_id,
                session.started_at,
                session.ended_at
            );
        }
    }
    Ok(())
}

pub(crate) fn print_messages(
    messages: &[oxidebbs_db::MessageRecord],
    json_output: bool,
) -> CliResult<()> {
    if json_output {
        print_json(&JsonValue::Array(
            messages.iter().map(message_json).collect(),
        ))?;
    } else {
        for message in messages {
            println!(
                "{}\t{}\t{}\t{}",
                message.id, message.created_at, message.visibility, message.subject
            );
        }
    }
    Ok(())
}

pub(crate) fn print_message(message: &oxidebbs_db::MessageRecord) {
    println!("id: {}", message.id);
    println!("area: {}", message.area_id);
    println!("author: {}", message.author_user_id);
    println!("to: {:?}", message.to_user_id);
    println!("created: {}", message.created_at);
    println!("visibility: {}", message.visibility);
    println!("subject: {}", message.subject);
    println!();
    println!("{}", message.body);
}

pub(crate) fn print_audit_events(events: &[AuditEventRecord], json_output: bool) -> CliResult<()> {
    if json_output {
        print_json(&JsonValue::Array(events.iter().map(audit_json).collect()))?;
    } else {
        for event in events {
            println!(
                "{}\t{}\tnode={:?}\tuser={:?}\t{}",
                event.created_at, event.event_type, event.node_number, event.user_id, event.details
            );
        }
    }
    Ok(())
}

pub(crate) fn prompt_line(prompt: &str, default: Option<&str>) -> CliResult<String> {
    match default {
        Some(default) => print!("{prompt} [{default}]: "),
        None => print!("{prompt}: "),
    }
    io::stdout().flush()?;
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let trimmed = input.trim();
    if trimmed.is_empty() {
        default
            .map(ToOwned::to_owned)
            .ok_or_else(|| CliError::Message(format!("{prompt} is required")))
    } else {
        Ok(trimmed.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn top_level_help_order_is_stable() {
        let mut names: Vec<_> = Cli::command()
            .get_subcommands()
            .map(|command| command.get_name().to_string())
            .collect();

        let has_help = names.iter().any(|name| name == "help");
        if has_help {
            assert_eq!(
                names.pop(),
                Some("help".to_string()),
                "help subcommand must be last"
            );
        }

        assert_eq!(
            names,
            vec![
                "ansi", "audit", "check", "config", "db", "doors", "logs", "messages", "nodes",
                "serve", "setup", "status", "sysop", "users",
            ]
        );
    }
}
