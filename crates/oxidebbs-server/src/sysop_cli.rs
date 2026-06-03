use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use argon2::password_hash::{PasswordHash, PasswordHasher, SaltString};
use argon2::{Algorithm, Argon2, Params, PasswordVerifier as Argon2PasswordVerifier, Version};
use clap::{Parser, Subcommand, ValueEnum};
use rand_core::OsRng;
use serde_json::{Value as JsonValue, json};
use thiserror::Error;

use oxidebbs_db::{
    AuditEventRecord, MessageAreaRecord, OxideDb, SessionRecord, UserRecord, Value,
    find_active_session_by_node, find_message_area_by_key, find_message_by_id,
    find_user_by_alias_ci, find_user_by_id, insert_audit_event,
};

use crate::config::{
    DoorDefConfig, LoggingRotationConfig, OxideConfig, normalize_database_path,
    validate_logging_format, validate_logging_level,
};
use crate::serve;

use crate::commands::{
    AnsiCommand, AuditCommand, ConfigCommand, DbCommand, DoorsCommand, LogsCommand,
    MessagesCommand, NodesCommand, ServeArgs, SetupArgs, UsersCommand, run_ansi, run_audit,
    run_check, run_config, run_config_set, run_db, run_doors, run_logs, run_messages, run_nodes,
    run_serve, run_setup_command, run_status, run_sysop_tui, run_users,
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

    /// Launch the interactive sysop TUI
    Sysop {
        #[arg(long)]
        tui: bool,
        /// Run the sysop TUI in read-only mode
        #[arg(long)]
        readonly: bool,
        /// Do not start an embedded server if no live control socket is reachable
        #[arg(long)]
        connect_only: bool,
        /// Theme name to apply to the TUI (default: oxide-classic)
        #[arg(long)]
        theme: Option<SysopTheme>,
    },

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

    let config_path = cli.config.unwrap_or_else(default_config_path);
    let command = cli.command.unwrap_or(Command::Serve(ServeArgs::default()));

    match command {
        Command::Setup(args) => {
            init_console_logging(cli.verbose)?;
            return run_setup_command(args, cli.data, cli.json);
        }
        Command::Config {
            command: ConfigCommand::Set { key, value },
        } => {
            init_console_logging(cli.verbose)?;
            return run_config_set(&config_path, &key, &value, cli.json);
        }
        _ => {}
    }

    let mut config = OxideConfig::load(&config_path)?;
    if let Some(data_path) = cli.data {
        config.database.path = normalize_database_path(data_path);
    }
    let log_level = effective_log_level(cli.verbose, &command, &config)?;
    init_logging(&config, &log_level, command_uses_console_logging(&command))?;

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
        Command::Sysop {
            readonly,
            connect_only,
            theme,
            ..
        } => {
            run_sysop_tui(
                &ctx,
                readonly,
                connect_only,
                theme
                    .as_ref()
                    .map(SysopTheme::as_ref)
                    .unwrap_or_else(|| SysopTheme::OxideClassic.as_ref()),
            )
            .await
        }
        Command::Setup(_) => unreachable!("setup is handled before config load"),
    }
}

#[derive(Clone, Debug, PartialEq, Eq, ValueEnum)]
enum SysopTheme {
    #[value(name = "oxide-classic")]
    OxideClassic,
    #[value(name = "wildcat")]
    Wildcat,
    #[value(name = "telegard")]
    Telegard,
    #[value(name = "vbbs")]
    Vbbs,
    #[value(name = "mystic")]
    Mystic,
    #[value(name = "midnight")]
    Midnight,
    #[value(name = "high-contrast")]
    HighContrast,
}

impl SysopTheme {
    fn as_ref(&self) -> &'static str {
        match self {
            Self::OxideClassic => "oxide-classic",
            Self::Wildcat => "wildcat",
            Self::Telegard => "telegard",
            Self::Vbbs => "vbbs",
            Self::Mystic => "mystic",
            Self::Midnight => "midnight",
            Self::HighContrast => "high-contrast",
        }
    }
}

fn verbose_log_level(verbose: u8) -> &'static str {
    match verbose {
        0 => "info",
        1 => "debug",
        _ => "trace",
    }
}

fn effective_log_level(verbose: u8, command: &Command, config: &OxideConfig) -> CliResult<String> {
    let level = if let Command::Serve(args) = command
        && let Some(log_level) = args.log_level.as_deref()
    {
        log_level
    } else if verbose > 0 {
        verbose_log_level(verbose)
    } else {
        &config.logging.level
    };
    validate_logging_level(level).map_err(CliError::Message)?;
    Ok(level.trim().to_ascii_lowercase())
}

fn command_uses_console_logging(command: &Command) -> bool {
    !matches!(command, Command::Sysop { .. })
}

fn init_console_logging(verbose: u8) -> CliResult<()> {
    init_logging_with_file(verbose_log_level(verbose), None, "text", true)
}

pub(crate) fn init_logging(
    config: &OxideConfig,
    level: &str,
    console_enabled: bool,
) -> CliResult<()> {
    validate_logging_format(&config.logging.format).map_err(CliError::Message)?;
    let file = if config.logging.file_enabled {
        std::fs::create_dir_all(&config.paths.logs)?;
        let path = config.paths.logs.join(config.logging.file_name.trim());
        Some(RotatingLogFile::open(
            path,
            LogRotationPolicy::from_config(&config.logging.rotation)?,
        )?)
    } else {
        None
    };
    init_logging_with_file(level, file, &config.logging.format, console_enabled)
}

fn init_logging_with_file(
    level: &str,
    file: Option<RotatingLogFile>,
    file_format: &str,
    console_enabled: bool,
) -> CliResult<()> {
    use tracing_subscriber::prelude::*;

    let env_filter = tracing_subscriber::EnvFilter::new(level);
    let console_layer =
        console_enabled.then(|| tracing_subscriber::fmt::layer().with_writer(io::stderr));
    let file_format = file_format.trim().to_ascii_lowercase();

    match file {
        Some(file) if file_format == "json" => tracing_subscriber::registry()
            .with(env_filter)
            .with(console_layer)
            .with(
                tracing_subscriber::fmt::layer()
                    .json()
                    .flatten_event(true)
                    .with_ansi(false)
                    .with_writer(file),
            )
            .try_init()
            .map_err(|error| CliError::Message(format!("failed to initialize logging: {error}"))),
        Some(file) => tracing_subscriber::registry()
            .with(env_filter)
            .with(console_layer)
            .with(
                tracing_subscriber::fmt::layer()
                    .with_ansi(false)
                    .with_writer(file),
            )
            .try_init()
            .map_err(|error| CliError::Message(format!("failed to initialize logging: {error}"))),
        None => tracing_subscriber::registry()
            .with(env_filter)
            .with(console_layer)
            .try_init()
            .map_err(|error| CliError::Message(format!("failed to initialize logging: {error}"))),
    }
}

#[derive(Clone)]
struct RotatingLogFile {
    state: Arc<Mutex<RotatingLogState>>,
}

impl RotatingLogFile {
    fn open(path: PathBuf, policy: LogRotationPolicy) -> io::Result<Self> {
        let file = open_log_file(&path)?;
        let current_size = file.metadata()?.len();
        Ok(Self {
            state: Arc::new(Mutex::new(RotatingLogState {
                path,
                policy,
                file,
                current_size,
                active_date: current_utc_date_string(),
            })),
        })
    }
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for RotatingLogFile {
    type Writer = RotatingLogFileWriter;

    fn make_writer(&'a self) -> Self::Writer {
        RotatingLogFileWriter {
            state: Arc::clone(&self.state),
        }
    }
}

struct RotatingLogFileWriter {
    state: Arc<Mutex<RotatingLogState>>,
}

impl Write for RotatingLogFileWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| io::Error::other("log file lock poisoned"))?;
        state.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| io::Error::other("log file lock poisoned"))?;
        state.file.flush()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LogRotationStrategy {
    Never,
    Daily,
    Size,
}

#[derive(Debug, Clone, Copy)]
struct LogRotationPolicy {
    strategy: LogRotationStrategy,
    max_size_bytes: u64,
    max_files: usize,
}

impl LogRotationPolicy {
    fn from_config(config: &LoggingRotationConfig) -> CliResult<Self> {
        let strategy = match config.strategy.trim().to_ascii_lowercase().as_str() {
            "never" => LogRotationStrategy::Never,
            "daily" => LogRotationStrategy::Daily,
            "size" => LogRotationStrategy::Size,
            other => {
                return Err(CliError::Message(format!(
                    "logging.rotation.strategy must be one of never, daily, or size, got {other:?}"
                )));
            }
        };
        if config.max_size_mb == 0 {
            return Err(CliError::Message(
                "logging.rotation.max_size_mb must be greater than 0".to_string(),
            ));
        }
        if config.max_files == 0 {
            return Err(CliError::Message(
                "logging.rotation.max_files must be greater than 0".to_string(),
            ));
        }

        Ok(Self {
            strategy,
            max_size_bytes: config.max_size_mb.saturating_mul(1024 * 1024),
            max_files: config.max_files,
        })
    }
}

struct RotatingLogState {
    path: PathBuf,
    policy: LogRotationPolicy,
    file: File,
    current_size: u64,
    active_date: String,
}

impl RotatingLogState {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.rotate_if_needed(buf.len() as u64)?;
        self.file.write_all(buf)?;
        self.current_size = self.current_size.saturating_add(buf.len() as u64);
        Ok(buf.len())
    }

    fn rotate_if_needed(&mut self, incoming_bytes: u64) -> io::Result<()> {
        match self.policy.strategy {
            LogRotationStrategy::Never => Ok(()),
            LogRotationStrategy::Daily => {
                let today = current_utc_date_string();
                if today != self.active_date {
                    let previous_date = std::mem::replace(&mut self.active_date, today);
                    self.rotate_daily(&previous_date)?;
                }
                Ok(())
            }
            LogRotationStrategy::Size => {
                if self.current_size > 0
                    && self.current_size.saturating_add(incoming_bytes) > self.policy.max_size_bytes
                {
                    self.rotate_size()?;
                }
                Ok(())
            }
        }
    }

    fn rotate_daily(&mut self, date: &str) -> io::Result<()> {
        self.file.flush()?;
        let archive_path = unique_daily_archive_path(&self.path, date);
        if self.path.exists() && self.current_size > 0 {
            std::fs::rename(&self.path, archive_path)?;
        }
        prune_daily_archives(&self.path, self.policy.max_files)?;
        self.file = open_log_file(&self.path)?;
        self.current_size = self.file.metadata()?.len();
        Ok(())
    }

    fn rotate_size(&mut self) -> io::Result<()> {
        self.file.flush()?;
        let oldest = numbered_archive_path(&self.path, self.policy.max_files);
        if oldest.exists() {
            std::fs::remove_file(oldest)?;
        }
        for index in (1..self.policy.max_files).rev() {
            let source = numbered_archive_path(&self.path, index);
            if source.exists() {
                std::fs::rename(source, numbered_archive_path(&self.path, index + 1))?;
            }
        }
        if self.path.exists() && self.current_size > 0 {
            std::fs::rename(&self.path, numbered_archive_path(&self.path, 1))?;
        }
        self.file = open_log_file(&self.path)?;
        self.current_size = self.file.metadata()?.len();
        Ok(())
    }
}

fn open_log_file(path: &Path) -> io::Result<File> {
    OpenOptions::new().create(true).append(true).open(path)
}

fn unique_daily_archive_path(path: &Path, date: &str) -> PathBuf {
    let mut attempt = 0usize;
    loop {
        let candidate = daily_archive_path(path, date, attempt);
        if !candidate.exists() {
            return candidate;
        }
        attempt += 1;
    }
}

fn daily_archive_path(path: &Path, date: &str, attempt: usize) -> PathBuf {
    let parent = path.parent().unwrap_or_else(|| Path::new(""));
    let stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("oxidebbs-server");
    let suffix = if attempt == 0 {
        date.to_string()
    } else {
        format!("{date}.{attempt}")
    };
    let file_name = match path.extension().and_then(|value| value.to_str()) {
        Some(extension) => format!("{stem}.{suffix}.{extension}"),
        None => format!("{stem}.{suffix}"),
    };
    parent.join(file_name)
}

fn numbered_archive_path(path: &Path, index: usize) -> PathBuf {
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("oxidebbs-server.log");
    path.with_file_name(format!("{file_name}.{index}"))
}

fn prune_daily_archives(path: &Path, max_files: usize) -> io::Result<()> {
    let Some(parent) = path.parent() else {
        return Ok(());
    };
    let stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("oxidebbs-server");
    let extension = path.extension().and_then(|value| value.to_str());
    let mut archives = std::fs::read_dir(parent)?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|entry| is_daily_archive(entry, stem, extension))
        .collect::<Vec<_>>();
    archives.sort();
    let remove_count = archives.len().saturating_sub(max_files);
    for archive in archives.into_iter().take(remove_count) {
        std::fs::remove_file(archive)?;
    }
    Ok(())
}

fn is_daily_archive(path: &Path, stem: &str, extension: Option<&str>) -> bool {
    let Some(file_name) = path.file_name().and_then(|value| value.to_str()) else {
        return false;
    };
    let expected_prefix = format!("{stem}.");
    if !file_name.starts_with(&expected_prefix) {
        return false;
    }
    match extension {
        Some(extension) => file_name.ends_with(&format!(".{extension}")),
        None => true,
    }
}

fn current_utc_date_string() -> String {
    let days_since_epoch = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() / 86_400)
        .unwrap_or(0) as i64;
    let (year, month, day) = civil_from_days(days_since_epoch);
    format!("{year:04}-{month:02}-{day:02}")
}

fn civil_from_days(days_since_epoch: i64) -> (i32, u32, u32) {
    let z = days_since_epoch + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let day_of_era = z - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_parameter = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_parameter + 2) / 5 + 1;
    let month = month_parameter + if month_parameter < 10 { 3 } else { -9 };
    let year = year + if month <= 2 { 1 } else { 0 };
    (year as i32, month as u32, day as u32)
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

    fn minimal_config() -> OxideConfig {
        toml::from_str(
            r#"
[board]
name = "Test BBS"
"#,
        )
        .expect("parse minimal config")
    }

    #[test]
    fn serve_log_level_overrides_global_verbose_and_config() {
        let mut config = minimal_config();
        config.logging.level = "info".to_string();
        let command = Command::Serve(ServeArgs {
            bind: None,
            dry_run: false,
            log_level: Some("warn".to_string()),
        });

        let level = effective_log_level(2, &command, &config).expect("effective level");
        assert_eq!(level, "warn");
    }

    #[test]
    fn global_verbose_overrides_config_when_serve_level_is_absent() {
        let mut config = minimal_config();
        config.logging.level = "info".to_string();
        let command = Command::Serve(ServeArgs {
            bind: None,
            dry_run: false,
            log_level: None,
        });

        let level = effective_log_level(1, &command, &config).expect("effective level");
        assert_eq!(level, "debug");
    }

    #[test]
    fn sysop_command_disables_console_logging_for_tui() {
        let sysop = Command::Sysop {
            tui: false,
            readonly: false,
            connect_only: false,
            theme: None,
        };
        let serve = Command::Serve(ServeArgs {
            bind: None,
            dry_run: false,
            log_level: None,
        });

        assert!(!command_uses_console_logging(&sysop));
        assert!(command_uses_console_logging(&serve));
    }

    #[test]
    fn size_rotating_log_file_moves_archives() {
        use tracing_subscriber::fmt::MakeWriter as _;

        let mut dir = std::env::temp_dir();
        dir.push(format!(
            "oxidebbs-log-rotate-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system time should be valid")
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).expect("create temp log dir");
        let path = dir.join("oxidebbs-server.log");
        let log_file = RotatingLogFile::open(
            path.clone(),
            LogRotationPolicy {
                strategy: LogRotationStrategy::Size,
                max_size_bytes: 12,
                max_files: 2,
            },
        )
        .expect("open rotating log");

        {
            let mut writer = log_file.make_writer();
            writer.write_all(b"first-line\n").expect("write first log");
            writer
                .write_all(b"second-line\n")
                .expect("write second log");
            writer.write_all(b"third-line\n").expect("write third log");
            writer.flush().expect("flush log");
        }

        assert!(path.exists());
        assert!(numbered_archive_path(&path, 1).exists());
        assert!(numbered_archive_path(&path, 2).exists());

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn utc_day_conversion_matches_unix_epoch() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(1), (1970, 1, 2));
    }

    #[test]
    fn sysop_command_respects_readonly_flag_and_custom_config() {
        let config_path = PathBuf::from("config/custom.toml");
        let cli = Cli::parse_from([
            "oxidebbs",
            "--config",
            config_path.to_str().expect("config path not utf-8"),
            "sysop",
            "--readonly",
        ]);

        assert_eq!(cli.config, Some(config_path));

        match cli.command {
            Some(Command::Sysop {
                tui,
                readonly,
                connect_only,
                theme,
                ..
            }) => {
                assert!(readonly);
                assert!(!tui);
                assert!(!connect_only);
                assert_eq!(theme, None);
            }
            _ => panic!("expected sysop command"),
        }
    }

    #[test]
    fn sysop_command_accepts_connect_only_mode() {
        let cli = Cli::parse_from(["oxidebbs", "sysop", "--connect-only"]);

        match cli.command {
            Some(Command::Sysop {
                connect_only,
                readonly,
                theme,
                ..
            }) => {
                assert!(connect_only);
                assert!(!readonly);
                assert_eq!(theme, None);
            }
            _ => panic!("expected sysop command"),
        }
    }

    #[test]
    fn sysop_command_accepts_theme_flag() {
        let cli = Cli::parse_from(["oxidebbs", "sysop", "--theme", "midnight"]);

        match cli.command {
            Some(Command::Sysop {
                theme: Some(theme), ..
            }) => {
                assert_eq!(theme, SysopTheme::Midnight);
            }
            _ => panic!("expected sysop command"),
        }
    }

    #[test]
    fn sysop_command_rejects_invalid_theme() {
        let error = match Cli::try_parse_from(["oxidebbs", "sysop", "--theme", "no-such-theme"]) {
            Ok(_) => panic!("expected parse to fail"),
            Err(error) => error,
        };
        let message = error.to_string();
        assert!(message.contains("high-contrast"));
        assert!(message.contains("wildcat"));
        assert!(message.contains("telegard"));
    }

    #[test]
    fn sysop_theme_values_match_sysop_theme_registry() {
        let cli_values = SysopTheme::value_variants()
            .iter()
            .map(SysopTheme::as_ref)
            .collect::<Vec<_>>();

        assert_eq!(cli_values, oxidebbs_sysop::theme::Theme::available_names());
    }
}
