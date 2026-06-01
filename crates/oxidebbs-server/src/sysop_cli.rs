use std::collections::HashMap;
use std::fs;
use std::io::{self, Write};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

use argon2::password_hash::{PasswordHash, PasswordHasher, SaltString};
use argon2::{Argon2, PasswordVerifier as Argon2PasswordVerifier};
use clap::{Args, Parser, Subcommand, ValueEnum};
use rand_core::OsRng;
use serde_json::{Value as JsonValue, json};
use thiserror::Error;
use tracing::info;

use oxidebbs_core::door::DoorDefinition;
use oxidebbs_db::{
    AuditEventRecord, Db, DoorDefinitionRecord, MessageAreaRecord, OxideDb, SessionRecord,
    UserRecord, Value, end_session, find_active_session_by_node, find_door_by_key,
    find_door_run_by_id, find_message_area_by_key, find_message_by_id, find_user_by_alias_ci,
    find_user_by_id, insert_audit_event, insert_door_definition, insert_message_area, insert_user,
    list_active_sessions, list_audit_events, list_audit_events_for_user, list_door_definitions,
    list_door_runs, list_message_areas, list_messages, list_messages_in_area, list_recent_sessions,
    list_users, move_message_to_area, read_schema_version, update_door_enabled,
    update_message_area_enabled, update_message_area_levels, update_message_visibility,
    update_user_alias, update_user_is_sysop, update_user_password_hash, update_user_security_level,
    update_user_status,
};
use oxidebbs_door::{
    DoorCaller, DoorRunRequest, DoorRunner, DosBoxRunner, DryRunDoorRunner,
    cleanup_node_runtime_dir, node_runtime_dir, render_door_sys, render_dorinfo1_def,
};
use oxidebbs_sysop::{SysopConsoleSnapshot, render_sysop_console_text};
use oxidebbs_term::{decode_cp437, encode_cp437, render_plain_text};

use crate::config::{DoorDefConfig, OxideConfig, ScreenConfig};
use crate::{serve, setup};

type CliResult<T> = Result<T, CliError>;

#[derive(Debug, Error)]
pub enum CliError {
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
    about = "OxideBBS - Rust-native BBS engine for telnet callers",
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
enum Command {
    /// Backwards-compatible aliases for the original admin command group
    Admin {
        #[command(subcommand)]
        command: AdminCommand,
    },

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

#[derive(Debug, Clone, Args, Default)]
struct ServeArgs {
    /// Override the telnet bind address
    #[arg(long)]
    bind: Option<String>,

    /// Validate startup prerequisites without listening for callers
    #[arg(long)]
    dry_run: bool,
}

#[derive(Debug, Clone, Args)]
struct SetupArgs {
    /// Output configuration file path
    #[arg(short, long, default_value = "config/oxidebbs.toml")]
    output: PathBuf,

    /// Overwrite an existing output file
    #[arg(long)]
    force: bool,

    /// Board name for non-interactive setup
    #[arg(long)]
    board_name: Option<String>,

    /// Initial sysop alias for non-interactive setup
    #[arg(long)]
    sysop_alias: Option<String>,

    /// Initial sysop password for non-interactive setup
    #[arg(long)]
    sysop_password: Option<String>,

    /// Telnet port for non-interactive setup
    #[arg(long)]
    telnet_port: Option<u16>,

    /// Node count for non-interactive setup
    #[arg(long)]
    nodes: Option<u16>,

    /// Skip bundled sample ANSI screen directories
    #[arg(long)]
    no_sample_ansi: bool,
}

#[derive(Subcommand)]
enum UsersCommand {
    List,
    Show {
        alias_or_id: String,
    },
    Add(UserAddArgs),
    ResetPassword(ResetPasswordArgs),
    SetLevel {
        alias_or_id: String,
        level: i64,
    },
    Enable {
        alias_or_id: String,
    },
    Disable {
        alias_or_id: String,
    },
    PromoteSysop {
        alias_or_id: String,
    },
    DemoteSysop {
        alias_or_id: String,
    },
    Rename {
        old_alias: String,
        new_alias: String,
    },
    Audit {
        alias_or_id: String,
        #[arg(short, long, default_value_t = 25)]
        limit: i64,
    },
    Sessions {
        alias_or_id: String,
        #[arg(short, long, default_value_t = 25)]
        limit: i64,
    },
    Delete {
        alias_or_id: String,
    },
}

#[derive(Debug, Clone, Args)]
struct UserAddArgs {
    #[arg(long)]
    alias: Option<String>,
    #[arg(long)]
    real_name: Option<String>,
    #[arg(long)]
    email: Option<String>,
    #[arg(long)]
    password: Option<String>,
    #[arg(long, default_value_t = 10)]
    level: i64,
    #[arg(long)]
    sysop: bool,
}

#[derive(Debug, Clone, Args)]
struct ResetPasswordArgs {
    alias_or_id: String,
    /// New plaintext password to hash with Argon2id
    #[arg(long, conflicts_with = "password_hash")]
    password: Option<String>,
    /// Precomputed password hash for recovery/import workflows
    #[arg(long)]
    password_hash: Option<String>,
}

#[derive(Subcommand)]
enum NodesCommand {
    List,
    Watch {
        #[arg(short, long, default_value_t = 5)]
        interval: u64,
    },
    Show {
        node_number: i64,
    },
    Disconnect {
        node_number: i64,
    },
    Message {
        node_number: i64,
        text: String,
    },
    Broadcast {
        text: String,
    },
    Disable {
        node_number: i64,
    },
    Enable {
        node_number: i64,
    },
    ResetStale,
}

#[derive(Subcommand)]
enum MessagesCommand {
    Areas {
        #[command(subcommand)]
        command: MessageAreasCommand,
    },
    List {
        #[arg(long)]
        area: String,
    },
    Show {
        message_id: String,
    },
    Delete {
        message_id: String,
    },
    Move {
        message_id: String,
        #[arg(long = "to-area")]
        to_area: String,
    },
    Lock {
        message_id: String,
    },
    Unlock {
        message_id: String,
    },
    Search {
        query: String,
    },
}

#[derive(Subcommand)]
enum MessageAreasCommand {
    List,
    Add(MessageAreaAddArgs),
    Show {
        key: String,
    },
    Enable {
        key: String,
    },
    Disable {
        key: String,
    },
    SetLevel {
        key: String,
        #[arg(long)]
        read: i64,
        #[arg(long)]
        post: i64,
    },
}

#[derive(Debug, Clone, Args)]
struct MessageAreaAddArgs {
    key: String,
    #[arg(long)]
    name: String,
    #[arg(long, default_value = "")]
    description: String,
    #[arg(long, default_value_t = 0)]
    read_level: i64,
    #[arg(long, default_value_t = 10)]
    post_level: i64,
}

#[derive(Subcommand)]
enum DoorsCommand {
    List,
    Show {
        door_key: String,
    },
    Check {
        door_key: Option<String>,
    },
    Enable {
        door_key: String,
    },
    Disable {
        door_key: String,
    },
    Test(DoorTestArgs),
    Dropfile(DoorDropfileArgs),
    Add,
    Edit {
        door_key: String,
    },
    Runs {
        #[command(subcommand)]
        command: Option<DoorRunsCommand>,
    },
    Cleanup,
}

#[derive(Debug, Clone, Args)]
struct DoorTestArgs {
    door_key: String,
    #[arg(long)]
    user: String,
    #[arg(long, default_value_t = 1)]
    node: u16,
    #[arg(long)]
    dry_run: bool,
}

#[derive(Debug, Clone, Args)]
struct DoorDropfileArgs {
    door_key: String,
    #[arg(long)]
    user: String,
    #[arg(long)]
    node: u16,
    #[arg(long, default_value = "door.sys")]
    format: String,
    #[arg(long)]
    output: Option<PathBuf>,
}

#[derive(Subcommand)]
enum DoorRunsCommand {
    List {
        #[arg(short, long, default_value_t = 25)]
        limit: i64,
    },
    Show {
        run_id: String,
    },
}

#[derive(Subcommand)]
enum AnsiCommand {
    List,
    Show {
        screen_name: String,
        #[arg(long)]
        raw: bool,
    },
    Validate {
        screen_name: String,
    },
    InstallDefaults,
    Preview {
        screen_name: String,
    },
    Convert {
        input: PathBuf,
        #[arg(long)]
        from: Encoding,
        #[arg(long)]
        to: Encoding,
    },
    Inspect {
        screen_name: String,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum Encoding {
    Utf8,
    Cp437,
}

#[derive(Subcommand)]
enum DbCommand {
    Init,
    Doctor,
    Stats,
    Backup {
        output_path: PathBuf,
    },
    Export {
        #[arg(long, default_value = "json")]
        format: String,
    },
    Import {
        #[arg(long, default_value = "json")]
        format: String,
        path: PathBuf,
    },
    Compact,
    Verify,
}

#[derive(Subcommand)]
enum LogsCommand {
    Tail {
        #[arg(short, long, default_value_t = 100)]
        lines: usize,
        #[arg(short, long)]
        follow: bool,
    },
    Recent {
        #[arg(short, long, default_value_t = 100)]
        lines: usize,
    },
    Search {
        query: String,
    },
}

#[derive(Subcommand)]
enum AuditCommand {
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
}

#[derive(Subcommand)]
enum ConfigCommand {
    Show,
    Check,
    Paths,
    Get { key: String },
    Set { key: String, value: String },
}

#[derive(Subcommand)]
enum AdminCommand {
    Users,
    ResetPassword {
        user_id: String,
        password_hash: String,
    },
    Nodes,
    RecentCalls {
        #[arg(short, long, default_value_t = 10)]
        limit: i64,
    },
    TestDoorConfig {
        path: PathBuf,
    },
    ConsolePreview,
}

struct AppContext {
    config_path: PathBuf,
    config: OxideConfig,
    json: bool,
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
        config.database.path = data_path;
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
        Command::Admin { command } => run_admin_alias(command, &ctx),
        Command::Setup(_) => unreachable!("setup is handled before config load"),
    }
}

fn init_logging(verbose: u8) {
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

fn default_config_path() -> PathBuf {
    let local_config = PathBuf::from("config/oxidebbs.toml");
    if local_config.exists() {
        return local_config;
    }

    PathBuf::from("config/oxidebbs.example.toml")
}

async fn run_serve(args: ServeArgs, ctx: &AppContext) -> CliResult<()> {
    let mut config = ctx.config.clone();
    if let Some(bind) = args.bind {
        config.telnet.bind = bind;
    }

    if args.dry_run {
        let issues = validate_runtime(&config, &ctx.config_path);
        let errors = issues.iter().filter(|issue| issue.level == "error").count();
        if ctx.json {
            print_json(&json!({
                "command": "serve --dry-run",
                "ok": errors == 0,
                "issues": issues.iter().map(CheckIssue::to_json).collect::<Vec<_>>()
            }))?;
        } else if errors == 0 {
            println!("serve dry-run OK");
        } else {
            print_check_issues(&issues);
        }
        if errors > 0 {
            return Err(CliError::Message(
                "serve dry-run found blocking issues".to_string(),
            ));
        }
        return Ok(());
    }

    info!(board = %config.board.name, "starting OxideBBS");
    println!(
        "OxideBBS \"{}\" - telnet {} with {} node(s)",
        config.board.name, config.telnet.bind, config.nodes.count
    );
    serve::run(&config).await?;
    Ok(())
}

fn run_setup_command(
    args: SetupArgs,
    data_override: Option<PathBuf>,
    json_output: bool,
) -> CliResult<()> {
    let mut answers = setup_answers(args.clone())?;
    if let Some(data_path) = data_override {
        answers.database_path = data_path;
    }
    setup::run_setup_with_answers(&args.output, args.force, &answers)?;

    let db = OxideDb::open_or_create(&answers.database_path)?;
    seed_initial_sysop(&db, &answers)?;
    seed_default_message_area(&db)?;

    if json_output {
        print_json(&json!({
            "ok": true,
            "config": args.output,
            "database": answers.database_path,
            "sysop_alias": answers.sysop_alias,
            "nodes": answers.node_count
        }))?;
    } else {
        println!(
            "setup complete: wrote configuration to {}",
            args.output.display()
        );
        println!("database initialized: {}", answers.database_path.display());
        println!("initial sysop account: {}", answers.sysop_alias);
    }
    Ok(())
}

fn setup_answers(args: SetupArgs) -> CliResult<setup::SetupAnswers> {
    let has_noninteractive = args.board_name.is_some()
        || args.sysop_alias.is_some()
        || args.sysop_password.is_some()
        || args.telnet_port.is_some()
        || args.nodes.is_some()
        || args.no_sample_ansi;

    if !has_noninteractive {
        return setup::interactive_setup_answers().map_err(CliError::Io);
    }

    let mut answers = setup::SetupAnswers::default();
    if let Some(board_name) = args.board_name {
        answers.board_name = board_name;
    }
    if let Some(sysop_alias) = args.sysop_alias {
        answers.sysop_alias = sysop_alias;
    }
    if let Some(sysop_password) = args.sysop_password {
        answers.sysop_password = sysop_password;
    } else {
        return Err(CliError::Message(
            "non-interactive setup requires --sysop-password".to_string(),
        ));
    }
    if let Some(port) = args.telnet_port {
        answers.telnet_bind = format!("0.0.0.0:{port}");
    }
    if let Some(nodes) = args.nodes {
        if nodes == 0 {
            return Err(CliError::Message(
                "--nodes must be greater than 0".to_string(),
            ));
        }
        answers.node_count = nodes;
    }
    if args.no_sample_ansi {
        answers.include_sample_ansi = false;
    }
    Ok(answers)
}

fn seed_initial_sysop(db: &OxideDb, answers: &setup::SetupAnswers) -> CliResult<()> {
    if find_user_by_alias_ci(db.db(), &answers.sysop_alias)?.is_some() {
        return Ok(());
    }

    let now = current_timestamp(db)?;
    let user = UserRecord {
        id: generated_uuid(db)?,
        alias: answers.sysop_alias.clone(),
        real_name: answers.sysop_name.clone(),
        email: None,
        password_hash: hash_password(&answers.sysop_password)?,
        security_level: 255,
        is_sysop: true,
        created_at: now,
        last_login_at: None,
        total_calls: 0,
        time_bank_minutes: 0,
        status: "active".to_string(),
    };
    insert_user(db.db(), &user)?;
    Ok(())
}

fn seed_default_message_area(db: &OxideDb) -> CliResult<()> {
    if find_message_area_by_key(db.db(), "general")?.is_some() {
        return Ok(());
    }
    insert_message_area(
        db.db(),
        &MessageAreaRecord {
            id: generated_uuid(db)?,
            key: "general".to_string(),
            name: "General".to_string(),
            description: "Default local message area".to_string(),
            kind: "local".to_string(),
            network_id: None,
            read_security_level: 0,
            post_security_level: 10,
            moderated: false,
            enabled: true,
        },
    )?;
    Ok(())
}

fn run_check(ctx: &AppContext) -> CliResult<()> {
    let issues = validate_runtime(&ctx.config, &ctx.config_path);
    let errors = issues.iter().filter(|issue| issue.level == "error").count();
    if ctx.json {
        print_json(&json!({
            "ok": errors == 0,
            "issues": issues.iter().map(CheckIssue::to_json).collect::<Vec<_>>()
        }))?;
    } else if errors == 0 {
        println!("configuration OK: {}", ctx.config_path.display());
        println!("  board:          {}", ctx.config.board.name);
        println!("  telnet bind:    {}", ctx.config.telnet.bind);
        println!("  database path:  {}", ctx.config.database.path.display());
        println!("  nodes:          {}", ctx.config.nodes.count);
        println!("  doors defined:  {}", ctx.config.doors.definitions.len());
        print_check_issues(&issues);
    } else {
        print_check_issues(&issues);
    }

    if errors > 0 {
        return Err(CliError::Message("configuration check failed".to_string()));
    }
    Ok(())
}

fn run_status(ctx: &AppContext) -> CliResult<()> {
    let db = open_database(&ctx.config)?;
    sync_configured_doors(&db, &ctx.config)?;
    let active = list_active_sessions(db.db())?;
    let doors = effective_doors(&db, &ctx.config)?;
    let enabled_doors = doors.iter().filter(|door| door.enabled).count();
    let message_areas = list_message_areas(db.db())?;
    let version = env!("CARGO_PKG_VERSION");

    if ctx.json {
        print_json(&json!({
            "board": ctx.config.board.name,
            "version": version,
            "database": ctx.config.database.path,
            "telnet": ctx.config.telnet.bind,
            "nodes": { "total": ctx.config.nodes.count, "active": active.len() },
            "doors": { "enabled": enabled_doors, "total": doors.len() },
            "messages": { "areas": message_areas.len() }
        }))?;
    } else {
        println!("OxideBBS Status");
        println!("Board:        {}", ctx.config.board.name);
        println!("Version:      {version}");
        println!("Database:     {}", ctx.config.database.path.display());
        println!("Telnet:       {}", ctx.config.telnet.bind);
        println!(
            "Nodes:        {} total, {} active",
            ctx.config.nodes.count,
            active.len()
        );
        println!("Doors:        {enabled_doors} enabled");
        println!("Messages:     {} areas", message_areas.len());
        println!("Uptime:       not available without a live control socket");
    }
    Ok(())
}

fn run_users(command: UsersCommand, ctx: &AppContext) -> CliResult<()> {
    let db = open_database(&ctx.config)?;
    match command {
        UsersCommand::List => {
            let users = list_users(db.db())?;
            if ctx.json {
                print_json(&JsonValue::Array(users.iter().map(user_json).collect()))?;
            } else {
                for user in users {
                    println!(
                        "{}\t{}\tlevel={}\tsysop={}\tstatus={}",
                        user.id, user.alias, user.security_level, user.is_sysop, user.status
                    );
                }
            }
        }
        UsersCommand::Show { alias_or_id } => {
            let user = require_user(&db, &alias_or_id)?;
            if ctx.json {
                print_json(&user_json(&user))?;
            } else {
                print_user(&user);
            }
        }
        UsersCommand::Add(args) => add_user(args, ctx, &db)?,
        UsersCommand::ResetPassword(args) => {
            let user = require_user(&db, &args.alias_or_id)?;
            let hash = match (args.password, args.password_hash) {
                (Some(password), None) => hash_password(&password)?,
                (None, Some(password_hash)) => password_hash,
                (None, None) => hash_password(&prompt_line("New password", None)?)?,
                (Some(_), Some(_)) => {
                    return Err(CliError::Message(
                        "--password and --password-hash are mutually exclusive".to_string(),
                    ));
                }
            };
            update_user_password_hash(db.db(), &user.id, &hash)?;
            emit_ok(ctx.json, "password updated", json!({"user": user.alias}))?;
        }
        UsersCommand::SetLevel { alias_or_id, level } => {
            let user = require_user(&db, &alias_or_id)?;
            update_user_security_level(db.db(), &user.id, level)?;
            emit_ok(
                ctx.json,
                "security level updated",
                json!({"user": user.alias, "level": level}),
            )?;
        }
        UsersCommand::Enable { alias_or_id } => {
            let user = require_user(&db, &alias_or_id)?;
            update_user_status(db.db(), &user.id, "active")?;
            emit_ok(ctx.json, "user enabled", json!({"user": user.alias}))?;
        }
        UsersCommand::Disable { alias_or_id } | UsersCommand::Delete { alias_or_id } => {
            let user = require_user(&db, &alias_or_id)?;
            update_user_status(db.db(), &user.id, "disabled")?;
            emit_ok(
                ctx.json,
                "user disabled; delete is implemented as a safe disable",
                json!({"user": user.alias, "status": "disabled"}),
            )?;
        }
        UsersCommand::PromoteSysop { alias_or_id } => {
            let user = require_user(&db, &alias_or_id)?;
            update_user_is_sysop(db.db(), &user.id, true)?;
            update_user_security_level(db.db(), &user.id, 255)?;
            emit_ok(
                ctx.json,
                "user promoted to sysop",
                json!({"user": user.alias}),
            )?;
        }
        UsersCommand::DemoteSysop { alias_or_id } => {
            let user = require_user(&db, &alias_or_id)?;
            update_user_is_sysop(db.db(), &user.id, false)?;
            emit_ok(
                ctx.json,
                "user demoted from sysop",
                json!({"user": user.alias}),
            )?;
        }
        UsersCommand::Rename {
            old_alias,
            new_alias,
        } => {
            let user = require_user(&db, &old_alias)?;
            update_user_alias(db.db(), &user.id, &new_alias)?;
            emit_ok(
                ctx.json,
                "user renamed",
                json!({"old_alias": old_alias, "new_alias": new_alias}),
            )?;
        }
        UsersCommand::Audit { alias_or_id, limit } => {
            let user = require_user(&db, &alias_or_id)?;
            let events = list_audit_events_for_user(db.db(), &user.id, limit)?;
            print_audit_events(&events, ctx.json)?;
        }
        UsersCommand::Sessions { alias_or_id, limit } => {
            let user = require_user(&db, &alias_or_id)?;
            let sessions: Vec<_> = list_recent_sessions(db.db(), limit)?
                .into_iter()
                .filter(|session| session.user_id.as_deref() == Some(user.id.as_str()))
                .collect();
            print_sessions(&sessions, ctx.json)?;
        }
    }
    Ok(())
}

fn add_user(args: UserAddArgs, ctx: &AppContext, db: &OxideDb) -> CliResult<()> {
    let alias = match args.alias {
        Some(value) => value,
        None => prompt_line("Alias", None)?,
    };
    let real_name = match args.real_name {
        Some(value) => value,
        None => prompt_line("Real name", Some(&alias))?,
    };
    let password = match args.password {
        Some(value) => value,
        None => prompt_line("Password", None)?,
    };
    let now = current_timestamp(db)?;
    let user = UserRecord {
        id: generated_uuid(db)?,
        alias,
        real_name,
        email: args.email.filter(|value| !value.trim().is_empty()),
        password_hash: hash_password(&password)?,
        security_level: args.level,
        is_sysop: args.sysop,
        created_at: now,
        last_login_at: None,
        total_calls: 0,
        time_bank_minutes: 0,
        status: "active".to_string(),
    };
    insert_user(db.db(), &user)?;
    if ctx.json {
        print_json(&user_json(&user))?;
    } else {
        println!("user added: {}", user.alias);
    }
    Ok(())
}

fn run_nodes(command: NodesCommand, ctx: &AppContext) -> CliResult<()> {
    let db = open_database(&ctx.config)?;
    match command {
        NodesCommand::List => print_nodes(&db, ctx),
        NodesCommand::Watch { interval } => loop {
            print_nodes(&db, ctx)?;
            thread::sleep(Duration::from_secs(interval));
        },
        NodesCommand::Show { node_number } => {
            let session = find_active_session_by_node(db.db(), node_number)?;
            if ctx.json {
                print_json(&json!({
                    "node": node_number,
                    "state": if session.is_some() { "active" } else { "available" },
                    "session": session.as_ref().map(session_json)
                }))?;
            } else if let Some(session) = session {
                println!("node {node_number}: active");
                print_session(&session);
            } else {
                println!("node {node_number}: available");
            }
            Ok(())
        }
        NodesCommand::Disconnect { node_number } => {
            let session = require_active_session(&db, node_number)?;
            end_session(
                db.db(),
                &session.id,
                &current_timestamp(&db)?,
                "sysop_disconnect",
            )?;
            audit(
                &db,
                "node_disconnect_requested",
                session.user_id.as_deref(),
                Some(node_number),
                "sysop marked active session disconnected; live transport control requires a future control socket",
            )?;
            emit_ok(
                ctx.json,
                "node session marked disconnected",
                json!({"node": node_number}),
            )?;
            Ok(())
        }
        NodesCommand::Message { node_number, text } => {
            let session = require_active_session(&db, node_number)?;
            audit(
                &db,
                "node_message_requested",
                session.user_id.as_deref(),
                Some(node_number),
                &text,
            )?;
            emit_ok(
                ctx.json,
                "node message recorded for delivery by a future live control channel",
                json!({"node": node_number, "text": text}),
            )?;
            Ok(())
        }
        NodesCommand::Broadcast { text } => {
            audit(&db, "node_broadcast_requested", None, None, &text)?;
            emit_ok(
                ctx.json,
                "broadcast recorded for delivery by a future live control channel",
                json!({"text": text}),
            )?;
            Ok(())
        }
        NodesCommand::Disable { node_number } => {
            audit(&db, "node_disable_requested", None, Some(node_number), "")?;
            emit_ok(
                ctx.json,
                "node disable recorded; persistent node state is not yet modeled",
                json!({"node": node_number}),
            )?;
            Ok(())
        }
        NodesCommand::Enable { node_number } => {
            audit(&db, "node_enable_requested", None, Some(node_number), "")?;
            emit_ok(
                ctx.json,
                "node enable recorded; persistent node state is not yet modeled",
                json!({"node": node_number}),
            )?;
            Ok(())
        }
        NodesCommand::ResetStale => {
            audit(&db, "node_reset_stale_requested", None, None, "")?;
            emit_ok(
                ctx.json,
                "stale-node reset recorded; stale detection requires runtime heartbeats",
                json!({}),
            )?;
            Ok(())
        }
    }
}

fn print_nodes(db: &OxideDb, ctx: &AppContext) -> CliResult<()> {
    let active = list_active_sessions(db.db())?;
    let mut by_node = HashMap::new();
    for session in active {
        by_node.insert(session.node_number, session);
    }

    if ctx.json {
        let nodes = (1..=ctx.config.nodes.count)
            .map(|number| {
                let node_number = i64::from(number);
                let session = by_node.get(&node_number);
                json!({
                    "node": node_number,
                    "state": if session.is_some() { "active" } else { "available" },
                    "session": session.map(session_json)
                })
            })
            .collect::<Vec<_>>();
        print_json(&JsonValue::Array(nodes))?;
    } else {
        for number in 1..=ctx.config.nodes.count {
            let node_number = i64::from(number);
            if let Some(session) = by_node.get(&node_number) {
                println!(
                    "node {}\tactive\t{}\t{}",
                    node_number, session.transport, session.remote_address
                );
            } else {
                println!("node {node_number}\tavailable");
            }
        }
    }
    Ok(())
}

fn run_messages(command: MessagesCommand, ctx: &AppContext) -> CliResult<()> {
    let db = open_database(&ctx.config)?;
    match command {
        MessagesCommand::Areas { command } => run_message_areas(command, ctx, &db),
        MessagesCommand::List { area } => {
            let area = require_message_area(&db, &area)?;
            let messages = list_messages_in_area(db.db(), &area.id)?;
            print_messages(&messages, ctx.json)
        }
        MessagesCommand::Show { message_id } => {
            let message = require_message(&db, &message_id)?;
            if ctx.json {
                print_json(&message_json(&message))?;
            } else {
                print_message(&message);
            }
            Ok(())
        }
        MessagesCommand::Delete { message_id } => {
            let message = require_message(&db, &message_id)?;
            update_message_visibility(db.db(), &message.id, "deleted")?;
            emit_ok(
                ctx.json,
                "message deleted",
                json!({"message_id": message.id}),
            )?;
            Ok(())
        }
        MessagesCommand::Move {
            message_id,
            to_area,
        } => {
            let message = require_message(&db, &message_id)?;
            let area = require_message_area(&db, &to_area)?;
            move_message_to_area(db.db(), &message.id, &area.id)?;
            emit_ok(
                ctx.json,
                "message moved",
                json!({"message_id": message.id, "area": area.key}),
            )?;
            Ok(())
        }
        MessagesCommand::Lock { message_id } => {
            let message = require_message(&db, &message_id)?;
            update_message_visibility(db.db(), &message.id, "hidden")?;
            emit_ok(
                ctx.json,
                "message locked",
                json!({"message_id": message.id}),
            )?;
            Ok(())
        }
        MessagesCommand::Unlock { message_id } => {
            let message = require_message(&db, &message_id)?;
            update_message_visibility(db.db(), &message.id, "normal")?;
            emit_ok(
                ctx.json,
                "message unlocked",
                json!({"message_id": message.id}),
            )?;
            Ok(())
        }
        MessagesCommand::Search { query } => {
            let needle = query.to_ascii_lowercase();
            let matches: Vec<_> = list_messages(db.db())?
                .into_iter()
                .filter(|message| {
                    message.subject.to_ascii_lowercase().contains(&needle)
                        || message.body.to_ascii_lowercase().contains(&needle)
                })
                .collect();
            print_messages(&matches, ctx.json)
        }
    }
}

fn run_message_areas(
    command: MessageAreasCommand,
    ctx: &AppContext,
    db: &OxideDb,
) -> CliResult<()> {
    match command {
        MessageAreasCommand::List => {
            let areas = list_message_areas(db.db())?;
            if ctx.json {
                print_json(&JsonValue::Array(areas.iter().map(area_json).collect()))?;
            } else {
                for area in areas {
                    println!(
                        "{}\t{}\tread={}\tpost={}\tenabled={}",
                        area.key,
                        area.name,
                        area.read_security_level,
                        area.post_security_level,
                        area.enabled
                    );
                }
            }
        }
        MessageAreasCommand::Add(args) => {
            let area = MessageAreaRecord {
                id: generated_uuid(db)?,
                key: args.key,
                name: args.name,
                description: args.description,
                kind: "local".to_string(),
                network_id: None,
                read_security_level: args.read_level,
                post_security_level: args.post_level,
                moderated: false,
                enabled: true,
            };
            insert_message_area(db.db(), &area)?;
            emit_ok(ctx.json, "message area added", area_json(&area))?;
        }
        MessageAreasCommand::Show { key } => {
            let area = require_message_area(db, &key)?;
            if ctx.json {
                print_json(&area_json(&area))?;
            } else {
                println!("{} - {}", area.key, area.name);
                println!("id: {}", area.id);
                println!("description: {}", area.description);
                println!(
                    "levels: read={} post={}",
                    area.read_security_level, area.post_security_level
                );
                println!("enabled: {}", area.enabled);
            }
        }
        MessageAreasCommand::Enable { key } => {
            let area = require_message_area(db, &key)?;
            update_message_area_enabled(db.db(), &area.id, true)?;
            emit_ok(
                ctx.json,
                "message area enabled",
                json!({"area": area.key, "enabled": true}),
            )?;
        }
        MessageAreasCommand::Disable { key } => {
            let area = require_message_area(db, &key)?;
            update_message_area_enabled(db.db(), &area.id, false)?;
            emit_ok(
                ctx.json,
                "message area disabled",
                json!({"area": area.key, "enabled": false}),
            )?;
        }
        MessageAreasCommand::SetLevel { key, read, post } => {
            let area = require_message_area(db, &key)?;
            update_message_area_levels(db.db(), &area.id, read, post)?;
            emit_ok(
                ctx.json,
                "message area levels updated",
                json!({"area": area.key, "read": read, "post": post}),
            )?;
        }
    }
    Ok(())
}

fn run_doors(command: DoorsCommand, ctx: &AppContext) -> CliResult<()> {
    let db = open_database(&ctx.config)?;
    sync_configured_doors(&db, &ctx.config)?;
    match command {
        DoorsCommand::List => {
            let doors = effective_doors(&db, &ctx.config)?;
            if ctx.json {
                print_json(&JsonValue::Array(doors.iter().map(door_json).collect()))?;
            } else {
                for door in doors {
                    println!(
                        "{}\t{}\trunner={}\tenabled={}",
                        door.key, door.name, door.runner, door.enabled
                    );
                }
            }
        }
        DoorsCommand::Show { door_key } => {
            let door = require_effective_door(&db, &ctx.config, &door_key)?;
            if ctx.json {
                print_json(&door_json(&door))?;
            } else {
                println!("{} - {}", door.key, door.name);
                println!("runner: {}", door.runner);
                println!("working dir: {}", door.working_dir);
                println!("command: {}", door.command);
                println!("drop file: {}", door.drop_file);
                println!("exclusive: {}", door.exclusive);
                println!("time limit: {} minutes", door.time_limit_minutes);
                println!("enabled: {}", door.enabled);
            }
        }
        DoorsCommand::Check { door_key } => {
            let doors = if let Some(key) = door_key {
                vec![require_effective_door(&db, &ctx.config, &key)?]
            } else {
                effective_doors(&db, &ctx.config)?
            };
            let checks = doors
                .iter()
                .map(|door| check_door(door, &ctx.config))
                .collect::<Vec<_>>();
            let errors = checks
                .iter()
                .flat_map(|check| check.issues.iter())
                .filter(|issue| issue.level == "error")
                .count();
            if ctx.json {
                print_json(&JsonValue::Array(
                    checks.iter().map(DoorCheck::to_json).collect(),
                ))?;
            } else {
                for check in &checks {
                    println!("door {}:", check.key);
                    print_check_issues(&check.issues);
                    if check.issues.is_empty() {
                        println!("  OK");
                    }
                }
            }
            if errors > 0 {
                return Err(CliError::Message("door check failed".to_string()));
            }
        }
        DoorsCommand::Enable { door_key } => {
            let enabled = true;
            set_door_enabled(&db, &ctx.config, &door_key, enabled)?;
            emit_ok(
                ctx.json,
                "door enabled",
                json!({"door": door_key, "enabled": enabled}),
            )?;
        }
        DoorsCommand::Disable { door_key } => {
            let enabled = false;
            set_door_enabled(&db, &ctx.config, &door_key, enabled)?;
            emit_ok(
                ctx.json,
                "door disabled",
                json!({"door": door_key, "enabled": enabled}),
            )?;
        }
        DoorsCommand::Test(args) => run_door_test(args, ctx, &db)?,
        DoorsCommand::Dropfile(args) => run_door_dropfile(args, ctx, &db)?,
        DoorsCommand::Add => {
            return Err(CliError::Message(
                "door add is intentionally deferred to config-file editing for v1; use [[doors.definitions]] in the board config".to_string(),
            ));
        }
        DoorsCommand::Edit { door_key } => {
            return Err(CliError::Message(format!(
                "door edit for {door_key:?} is intentionally deferred to config-file editing for v1"
            )));
        }
        DoorsCommand::Runs { command } => match command
            .unwrap_or(DoorRunsCommand::List { limit: 25 })
        {
            DoorRunsCommand::List { limit } => {
                let runs = list_door_runs(db.db(), limit)?;
                if ctx.json {
                    print_json(&JsonValue::Array(runs.iter().map(door_run_json).collect()))?;
                } else {
                    for run in runs {
                        println!(
                            "{}\tdoor={}\tuser={}\tnode={}\tstarted={}\tended={:?}\texit={:?}",
                            run.id,
                            run.door_id,
                            run.user_id,
                            run.node_number,
                            run.started_at,
                            run.ended_at,
                            run.exit_code
                        );
                    }
                }
            }
            DoorRunsCommand::Show { run_id } => {
                let run = find_door_run_by_id(db.db(), &run_id)?
                    .ok_or_else(|| CliError::Message(format!("door run {run_id:?} not found")))?;
                if ctx.json {
                    print_json(&door_run_json(&run))?;
                } else {
                    println!("run: {}", run.id);
                    println!("door: {}", run.door_id);
                    println!("user: {}", run.user_id);
                    println!("node: {}", run.node_number);
                    println!("started: {}", run.started_at);
                    println!("ended: {:?}", run.ended_at);
                    println!("exit code: {:?}", run.exit_code);
                    println!("timed out: {}", run.timed_out);
                }
            }
        },
        DoorsCommand::Cleanup => {
            if ctx.config.paths.runtime.exists() {
                for entry in fs::read_dir(&ctx.config.paths.runtime)? {
                    let entry = entry?;
                    let path = entry.path();
                    if path.is_dir()
                        && path
                            .file_name()
                            .and_then(|name| name.to_str())
                            .is_some_and(|name| name.starts_with("node-"))
                    {
                        cleanup_node_runtime_dir(&path)?;
                    }
                }
            }
            emit_ok(ctx.json, "door runtime directories cleaned", json!({}))?;
        }
    }
    Ok(())
}

fn run_door_test(args: DoorTestArgs, ctx: &AppContext, db: &OxideDb) -> CliResult<()> {
    let door = require_effective_door(db, &ctx.config, &args.door_key)?;
    let user = require_user(db, &args.user)?;
    let runtime_dir = node_runtime_dir(&ctx.config.paths.runtime, args.node);
    let request = DoorRunRequest {
        door: door_to_core(&door),
        caller: door_caller(&user),
        node_number: args.node,
        runtime_dir,
    };

    let result = if args.dry_run {
        DryRunDoorRunner.run(&request)?
    } else {
        DosBoxRunner.run(&request)?
    };

    if ctx.json {
        print_json(&json!({
            "door": door.key,
            "user": user.alias,
            "node": args.node,
            "dry_run": args.dry_run,
            "exit_code": result.exit_code,
            "timed_out": result.timed_out
        }))?;
    } else {
        println!(
            "door test complete: {} exit={:?} timed_out={}",
            door.key, result.exit_code, result.timed_out
        );
    }
    Ok(())
}

fn run_door_dropfile(args: DoorDropfileArgs, ctx: &AppContext, db: &OxideDb) -> CliResult<()> {
    let _door = require_effective_door(db, &ctx.config, &args.door_key)?;
    let user = require_user(db, &args.user)?;
    let caller = door_caller(&user);
    let format = args.format.to_ascii_uppercase();
    let contents = match format.as_str() {
        "DORINFO1.DEF" => render_dorinfo1_def(
            &ctx.config.board.name,
            &ctx.config.board.sysop_name,
            &caller,
        ),
        "DOOR.SYS" => render_door_sys(&caller, args.node, 38_400),
        other => {
            return Err(CliError::Message(format!(
                "unsupported drop-file format {other:?}; supported formats are DOOR.SYS and DORINFO1.DEF"
            )));
        }
    };

    if let Some(output) = args.output {
        fs::write(&output, contents)?;
        emit_ok(ctx.json, "drop file written", json!({"path": output}))?;
    } else if ctx.json {
        print_json(&json!({"format": format, "contents": contents}))?;
    } else {
        print!("{contents}");
    }
    Ok(())
}

fn set_door_enabled(
    db: &OxideDb,
    config: &OxideConfig,
    door_key: &str,
    enabled: bool,
) -> CliResult<()> {
    let door = require_config_door(config, door_key)?;
    match find_door_by_key(db.db(), door_key)? {
        Some(existing) => update_door_enabled(db.db(), &existing.id, enabled)?,
        None => {
            let mut record = door_record_from_config(db, door)?;
            record.enabled = enabled;
            insert_door_definition(db.db(), &record)?;
        }
    }
    Ok(())
}

fn run_ansi(command: AnsiCommand, ctx: &AppContext) -> CliResult<()> {
    match command {
        AnsiCommand::List => {
            let screens = ctx
                .config
                .screens
                .iter()
                .map(|(name, screen)| json!({"name": name, "asset": screen.asset_for(crate::config::TerminalCapabilities::ansi_80())}))
                .collect::<Vec<_>>();
            if ctx.json {
                print_json(&JsonValue::Array(screens))?;
            } else {
                for (name, screen) in &ctx.config.screens {
                    let asset = screen
                        .asset_for(crate::config::TerminalCapabilities::ansi_80())
                        .unwrap_or("<missing>");
                    println!("{name}\t{asset}");
                }
            }
        }
        AnsiCommand::Show { screen_name, raw } => {
            let asset_path = load_screen_asset(ctx, &screen_name, true)?;
            let bytes = fs::read(&asset_path)?;
            if raw {
                io::stdout().write_all(&bytes)?;
            } else if ctx.json {
                print_json(&json!({
                    "screen": screen_name,
                    "asset": asset_path,
                    "preview": render_plain_text(&bytes)
                }))?;
            } else {
                println!("{}", render_plain_text(&bytes));
            }
        }
        AnsiCommand::Validate { screen_name } => {
            let issues = validate_screen(ctx, &screen_name);
            let errors = issues.iter().filter(|issue| issue.level == "error").count();
            if ctx.json {
                print_json(&json!({
                    "screen": screen_name,
                    "ok": errors == 0,
                    "issues": issues.iter().map(CheckIssue::to_json).collect::<Vec<_>>()
                }))?;
            } else if issues.is_empty() {
                println!("screen OK: {screen_name}");
            } else {
                print_check_issues(&issues);
            }
            if errors > 0 {
                return Err(CliError::Message(format!(
                    "screen {screen_name:?} is invalid"
                )));
            }
        }
        AnsiCommand::InstallDefaults => {
            fs::create_dir_all(&ctx.config.paths.ansi)?;
            fs::create_dir_all(&ctx.config.paths.screens)?;
            emit_ok(
                ctx.json,
                "default ANSI/screen directories are present",
                json!({"ansi": ctx.config.paths.ansi, "screens": ctx.config.paths.screens}),
            )?;
        }
        AnsiCommand::Preview { screen_name } => {
            let asset_path = load_screen_asset(ctx, &screen_name, true)?;
            let bytes = fs::read(&asset_path)?;
            if ctx.json {
                print_json(&json!({"screen": screen_name, "preview": render_plain_text(&bytes)}))?;
            } else {
                println!("{}", render_plain_text(&bytes));
            }
        }
        AnsiCommand::Convert { input, from, to } => {
            let bytes = fs::read(&input)?;
            match (from, to) {
                (Encoding::Utf8, Encoding::Cp437) => {
                    let text = String::from_utf8(bytes)
                        .map_err(|error| CliError::Message(error.to_string()))?;
                    let encoded = encode_cp437(&text)
                        .map_err(|error| CliError::Message(error.to_string()))?;
                    io::stdout().write_all(&encoded)?;
                }
                (Encoding::Cp437, Encoding::Utf8) => {
                    println!("{}", decode_cp437(&bytes));
                }
                (Encoding::Utf8, Encoding::Utf8) | (Encoding::Cp437, Encoding::Cp437) => {
                    io::stdout().write_all(&bytes)?;
                }
            }
        }
        AnsiCommand::Inspect { screen_name } => {
            let asset_path = load_screen_asset(ctx, &screen_name, true)?;
            let bytes = fs::read(&asset_path)?;
            let ansi_sequences = bytes.iter().filter(|byte| **byte == 0x1b).count();
            if ctx.json {
                print_json(&json!({
                    "screen": screen_name,
                    "asset": asset_path,
                    "bytes": bytes.len(),
                    "ansi_escape_count": ansi_sequences
                }))?;
            } else {
                println!("screen: {screen_name}");
                println!("asset: {}", asset_path.display());
                println!("bytes: {}", bytes.len());
                println!("ANSI ESC bytes: {ansi_sequences}");
            }
        }
    }
    Ok(())
}

fn run_db(command: DbCommand, ctx: &AppContext) -> CliResult<()> {
    match command {
        DbCommand::Init => {
            let db = open_database(&ctx.config)?;
            emit_ok(
                ctx.json,
                "database initialized",
                json!({"path": ctx.config.database.path, "schema_version": db.schema_version()?}),
            )
        }
        DbCommand::Doctor | DbCommand::Verify => {
            let db = open_database(&ctx.config)?;
            let version = db.schema_version()?;
            let stats = db_stats(db.db())?;
            if ctx.json {
                print_json(&json!({"ok": true, "schema_version": version, "stats": stats}))?;
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
                json!({"output": output_path}),
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

fn run_logs(command: LogsCommand, ctx: &AppContext) -> CliResult<()> {
    match command {
        LogsCommand::Tail { lines, follow } => {
            loop {
                print_recent_log_lines(ctx, lines)?;
                if !follow {
                    break;
                }
                thread::sleep(Duration::from_secs(2));
            }
            Ok(())
        }
        LogsCommand::Recent { lines } => print_recent_log_lines(ctx, lines),
        LogsCommand::Search { query } => {
            for line in all_log_lines(&ctx.config.paths.logs)? {
                if line.contains(&query) {
                    println!("{line}");
                }
            }
            Ok(())
        }
    }
}

fn run_audit(command: AuditCommand, ctx: &AppContext) -> CliResult<()> {
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
    }
}

fn run_config(command: ConfigCommand, ctx: &AppContext) -> CliResult<()> {
    match command {
        ConfigCommand::Show => {
            let raw = fs::read_to_string(&ctx.config_path)?;
            if ctx.json {
                let parsed: toml::Value = toml::from_str(&raw)?;
                print_json(&serde_json::to_value(parsed)?)?;
            } else {
                print!("{raw}");
            }
            Ok(())
        }
        ConfigCommand::Check => run_check(ctx),
        ConfigCommand::Paths => {
            let paths = json!({
                "config": ctx.config_path,
                "database": ctx.config.database.path,
                "ansi": ctx.config.paths.ansi,
                "screens": ctx.config.paths.screens,
                "doors": ctx.config.paths.doors,
                "runtime": ctx.config.paths.runtime,
                "logs": ctx.config.paths.logs
            });
            if ctx.json {
                print_json(&paths)?;
            } else {
                println!("config: {}", ctx.config_path.display());
                println!("database: {}", ctx.config.database.path.display());
                println!("ansi: {}", ctx.config.paths.ansi.display());
                println!("screens: {}", ctx.config.paths.screens.display());
                println!("doors: {}", ctx.config.paths.doors.display());
                println!("runtime: {}", ctx.config.paths.runtime.display());
                println!("logs: {}", ctx.config.paths.logs.display());
            }
            Ok(())
        }
        ConfigCommand::Get { key } => {
            let raw = fs::read_to_string(&ctx.config_path)?;
            let parsed: toml::Value = toml::from_str(&raw)?;
            let value = get_toml_path(&parsed, &key)
                .ok_or_else(|| CliError::Message(format!("config key {key:?} not found")))?;
            if ctx.json {
                print_json(&serde_json::to_value(value)?)?;
            } else {
                println!("{value}");
            }
            Ok(())
        }
        ConfigCommand::Set { .. } => unreachable!("config set is handled before config load"),
    }
}

fn run_config_set(
    config_path: &Path,
    key: &str,
    raw_value: &str,
    json_output: bool,
) -> CliResult<()> {
    let raw = fs::read_to_string(config_path)?;
    let mut parsed: toml::Value = toml::from_str(&raw)?;
    set_toml_path(&mut parsed, key, infer_toml_value(raw_value))?;
    let updated = toml::to_string_pretty(&parsed)?;
    fs::write(config_path, updated)?;
    emit_ok(json_output, "configuration updated", json!({"key": key}))
}

fn run_sysop_preview(ctx: &AppContext) -> CliResult<()> {
    let db = open_database(&ctx.config)?;
    let active_nodes = list_active_sessions(db.db())?.len();
    let recent_calls = list_audit_events(db.db(), 5)?
        .into_iter()
        .map(|event| format!("{} {}", event.created_at, event.event_type))
        .collect();
    let snapshot = SysopConsoleSnapshot {
        board_name: ctx.config.board.name.clone(),
        active_nodes,
        recent_calls,
    };
    println!("{}", render_sysop_console_text(&snapshot, 60, 10));
    Ok(())
}

fn run_admin_alias(command: AdminCommand, ctx: &AppContext) -> CliResult<()> {
    match command {
        AdminCommand::Users => run_users(UsersCommand::List, ctx),
        AdminCommand::ResetPassword {
            user_id,
            password_hash,
        } => run_users(
            UsersCommand::ResetPassword(ResetPasswordArgs {
                alias_or_id: user_id,
                password: None,
                password_hash: Some(password_hash),
            }),
            ctx,
        ),
        AdminCommand::Nodes => run_nodes(NodesCommand::List, ctx),
        AdminCommand::RecentCalls { limit } => run_audit(AuditCommand::Recent { limit }, ctx),
        AdminCommand::TestDoorConfig { path } => {
            let contents = fs::read_to_string(&path)?;
            let check = oxidebbs_sysop::test_door_config(&contents)
                .map_err(|error| CliError::Message(error.to_string()))?;
            if ctx.json {
                print_json(&json!({"definitions": check.definitions, "enabled": check.enabled}))?;
            } else {
                println!(
                    "door config OK: {} definition(s), {} enabled",
                    check.definitions, check.enabled
                );
            }
            Ok(())
        }
        AdminCommand::ConsolePreview => run_sysop_preview(ctx),
    }
}

#[derive(Debug, Clone)]
struct CheckIssue {
    level: &'static str,
    message: String,
}

impl CheckIssue {
    fn error(message: impl Into<String>) -> Self {
        Self {
            level: "error",
            message: message.into(),
        }
    }

    fn warning(message: impl Into<String>) -> Self {
        Self {
            level: "warning",
            message: message.into(),
        }
    }

    fn to_json(&self) -> JsonValue {
        json!({"level": self.level, "message": self.message})
    }
}

#[derive(Debug, Clone)]
struct DoorCheck {
    key: String,
    issues: Vec<CheckIssue>,
}

impl DoorCheck {
    fn to_json(&self) -> JsonValue {
        json!({
            "key": self.key,
            "ok": !self.issues.iter().any(|issue| issue.level == "error"),
            "issues": self.issues.iter().map(CheckIssue::to_json).collect::<Vec<_>>()
        })
    }
}

fn validate_runtime(config: &OxideConfig, config_path: &Path) -> Vec<CheckIssue> {
    let mut issues = Vec::new();
    if !config_path.is_file() {
        issues.push(CheckIssue::error(format!(
            "config file {} does not exist",
            config_path.display()
        )));
    }
    if config.telnet.bind.parse::<SocketAddr>().is_err() {
        issues.push(CheckIssue::error(format!(
            "telnet.bind {:?} is not a valid socket address",
            config.telnet.bind
        )));
    }
    if config.nodes.count == 0 {
        issues.push(CheckIssue::error("nodes.count must be greater than 0"));
    }
    if let Some(parent) = config.database.path.parent()
        && !parent.exists()
    {
        issues.push(CheckIssue::warning(format!(
            "database parent directory {} does not exist yet",
            parent.display()
        )));
    }
    for (label, path) in [
        ("ansi", &config.paths.ansi),
        ("screens", &config.paths.screens),
        ("doors", &config.paths.doors),
        ("runtime", &config.paths.runtime),
        ("logs", &config.paths.logs),
    ] {
        if !path.exists() {
            issues.push(CheckIssue::warning(format!(
                "{label} path {} does not exist yet",
                path.display()
            )));
        }
    }
    for screen_name in config.screens.keys() {
        issues.extend(validate_screen_assets(config, screen_name));
    }
    for door in &config.doors.definitions {
        issues.extend(
            check_door(
                &door_record_from_config_only(door, config.doors.enabled),
                config,
            )
            .issues,
        );
    }
    issues
}

fn validate_screen(ctx: &AppContext, screen_name: &str) -> Vec<CheckIssue> {
    validate_screen_assets(&ctx.config, screen_name)
}

fn validate_screen_assets(config: &OxideConfig, screen_name: &str) -> Vec<CheckIssue> {
    let mut issues = Vec::new();
    let Some(screen) = config.screens.get(screen_name) else {
        issues.push(CheckIssue::error(format!(
            "screen {screen_name:?} is not configured"
        )));
        return issues;
    };

    for asset in screen_assets(screen) {
        let path = config.paths.screens.join(asset);
        if !path.is_file() {
            issues.push(CheckIssue::error(format!(
                "screen {screen_name:?} asset {} is missing",
                path.display()
            )));
        }
    }
    issues
}

fn print_check_issues(issues: &[CheckIssue]) {
    for issue in issues {
        println!("{}: {}", issue.level, issue.message);
    }
}

fn check_door(door: &DoorDefinitionRecord, config: &OxideConfig) -> DoorCheck {
    let mut issues = Vec::new();
    let working_dir = PathBuf::from(&door.working_dir);
    if !working_dir.is_dir() {
        issues.push(CheckIssue::warning(format!(
            "door working directory {} does not exist",
            working_dir.display()
        )));
    }
    let command_name = door
        .command
        .split_whitespace()
        .next()
        .unwrap_or(door.command.as_str());
    if !working_dir.join(command_name).exists() {
        issues.push(CheckIssue::warning(format!(
            "door command {} was not found under {}",
            command_name,
            working_dir.display()
        )));
    }
    if !command_exists(&door.runner) {
        issues.push(CheckIssue::warning(format!(
            "door runner {:?} was not found on PATH",
            door.runner
        )));
    }
    if !matches!(
        door.drop_file.to_ascii_uppercase().as_str(),
        "DOOR.SYS" | "DORINFO1.DEF"
    ) {
        issues.push(CheckIssue::error(format!(
            "drop-file format {:?} is not supported",
            door.drop_file
        )));
    }
    if door.time_limit_minutes <= 0 {
        issues.push(CheckIssue::error("time limit must be greater than 0"));
    }
    if let Err(error) = fs::create_dir_all(&config.paths.runtime) {
        issues.push(CheckIssue::error(format!(
            "runtime directory {} is not writable: {error}",
            config.paths.runtime.display()
        )));
    }
    DoorCheck {
        key: door.key.clone(),
        issues,
    }
}

fn command_exists(command: &str) -> bool {
    let path = Path::new(command);
    if path.components().count() > 1 {
        return path.exists();
    }
    let Some(paths) = std::env::var_os("PATH") else {
        return false;
    };
    std::env::split_paths(&paths).any(|dir| dir.join(command).is_file())
}

fn open_database(config: &OxideConfig) -> CliResult<OxideDb> {
    Ok(OxideDb::open_or_create(&config.database.path)?)
}

fn db_scalar_text(db: &OxideDb, sql: &str) -> CliResult<String> {
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

fn db_scalar_i64(db: &Db, sql: &str) -> CliResult<i64> {
    let result = db.execute(sql)?;
    let value = result
        .rows()
        .first()
        .and_then(|row| row.values().first())
        .ok_or_else(|| CliError::Message(format!("query returned no scalar value: {sql}")))?;
    match value {
        Value::Int64(value) => Ok(*value),
        other => Err(CliError::Message(format!(
            "query returned non-int scalar for {sql}: {other:?}"
        ))),
    }
}

fn generated_uuid(db: &OxideDb) -> CliResult<String> {
    db_scalar_text(db, "SELECT UUID_TO_STRING(GEN_RANDOM_UUID())")
}

fn current_timestamp(db: &OxideDb) -> CliResult<String> {
    db_scalar_text(db, "SELECT CAST(NOW() AS TEXT)")
}

fn hash_password(password: &str) -> CliResult<String> {
    let salt = SaltString::generate(&mut OsRng);
    let password_hash = Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map_err(|error| CliError::Message(format!("password hashing failed: {error}")))?;
    Ok(password_hash.to_string())
}

#[allow(dead_code)]
fn verify_password(password: &str, password_hash: &str) -> bool {
    let Ok(parsed_hash) = PasswordHash::new(password_hash) else {
        return false;
    };
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed_hash)
        .is_ok()
}

fn audit(
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

fn require_user(db: &OxideDb, alias_or_id: &str) -> CliResult<UserRecord> {
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

fn looks_like_uuid(value: &str) -> bool {
    value.len() == 36
        && value
            .chars()
            .all(|character| character.is_ascii_hexdigit() || character == '-')
}

fn require_active_session(db: &OxideDb, node_number: i64) -> CliResult<SessionRecord> {
    find_active_session_by_node(db.db(), node_number)?.ok_or_else(|| {
        CliError::Message(format!(
            "node {node_number} does not have an active session"
        ))
    })
}

fn require_message_area(db: &OxideDb, key: &str) -> CliResult<MessageAreaRecord> {
    find_message_area_by_key(db.db(), key)?
        .ok_or_else(|| CliError::Message(format!("message area {key:?} was not found")))
}

fn require_message(db: &OxideDb, message_id: &str) -> CliResult<oxidebbs_db::MessageRecord> {
    find_message_by_id(db.db(), message_id)?
        .ok_or_else(|| CliError::Message(format!("message {message_id:?} was not found")))
}

fn require_config_door<'a>(config: &'a OxideConfig, key: &str) -> CliResult<&'a DoorDefConfig> {
    config
        .doors
        .definitions
        .iter()
        .find(|door| door.key.eq_ignore_ascii_case(key))
        .ok_or_else(|| CliError::Message(format!("door {key:?} was not found in config")))
}

fn require_effective_door(
    db: &OxideDb,
    config: &OxideConfig,
    key: &str,
) -> CliResult<DoorDefinitionRecord> {
    effective_doors(db, config)?
        .into_iter()
        .find(|door| door.key.eq_ignore_ascii_case(key))
        .ok_or_else(|| CliError::Message(format!("door {key:?} was not found")))
}

fn sync_configured_doors(db: &OxideDb, config: &OxideConfig) -> CliResult<()> {
    for door in &config.doors.definitions {
        if find_door_by_key(db.db(), &door.key)?.is_none() {
            insert_door_definition(db.db(), &door_record_from_config(db, door)?)?;
        }
    }
    Ok(())
}

fn effective_doors(db: &OxideDb, config: &OxideConfig) -> CliResult<Vec<DoorDefinitionRecord>> {
    let db_doors = list_door_definitions(db.db())?;
    let mut by_key: HashMap<String, DoorDefinitionRecord> = db_doors
        .into_iter()
        .map(|door| (door.key.to_ascii_lowercase(), door))
        .collect();
    for config_door in &config.doors.definitions {
        let key = config_door.key.to_ascii_lowercase();
        by_key
            .entry(key)
            .or_insert(door_record_from_config(db, config_door)?);
    }
    let mut doors = by_key.into_values().collect::<Vec<_>>();
    doors.sort_by(|left, right| left.key.cmp(&right.key));
    Ok(doors)
}

fn door_record_from_config(db: &OxideDb, door: &DoorDefConfig) -> CliResult<DoorDefinitionRecord> {
    let mut record = door_record_from_config_only(door, true);
    record.id = generated_uuid(db)?;
    Ok(record)
}

fn door_record_from_config_only(
    door: &DoorDefConfig,
    board_doors_enabled: bool,
) -> DoorDefinitionRecord {
    DoorDefinitionRecord {
        id: format!("door-{}", door.key),
        key: door.key.clone(),
        name: door.name.clone(),
        runner: door.runner.clone(),
        working_dir: door.working_dir.clone(),
        command: door.command.clone(),
        drop_file: door.drop_file.clone(),
        exclusive: door.exclusive,
        time_limit_minutes: i64::from(door.time_limit_minutes),
        enabled: board_doors_enabled && door.enabled,
    }
}

fn door_to_core(door: &DoorDefinitionRecord) -> DoorDefinition {
    DoorDefinition {
        id: door.id.clone(),
        key: door.key.clone(),
        name: door.name.clone(),
        runner: door.runner.clone(),
        working_dir: door.working_dir.clone(),
        command: door.command.clone(),
        drop_file: door.drop_file.clone(),
        exclusive: door.exclusive,
        time_limit_minutes: u32::try_from(door.time_limit_minutes).unwrap_or(30),
        enabled: door.enabled,
    }
}

fn door_caller(user: &UserRecord) -> DoorCaller {
    DoorCaller {
        alias: user.alias.clone(),
        real_name: user.real_name.clone(),
        location: "Local".to_string(),
        security_level: i32::try_from(user.security_level).unwrap_or(0),
        minutes_remaining: 30,
    }
}

fn load_screen_asset(ctx: &AppContext, screen_name: &str, ansi: bool) -> CliResult<PathBuf> {
    let screen = ctx
        .config
        .screens
        .get(screen_name)
        .ok_or_else(|| CliError::Message(format!("screen {screen_name:?} was not found")))?;
    let capabilities = if ansi {
        crate::config::TerminalCapabilities::ansi_80()
    } else {
        crate::config::TerminalCapabilities::plain_text()
    };
    let asset = screen
        .asset_for(capabilities)
        .ok_or_else(|| CliError::Message(format!("screen {screen_name:?} has no usable asset")))?;
    Ok(ctx.config.paths.screens.join(asset))
}

fn screen_assets(screen: &ScreenConfig) -> Vec<&str> {
    let mut assets = Vec::new();
    if let Some(asset) = screen.ansi.as_deref() {
        assets.push(asset);
    }
    if let Some(asset) = screen.ansi_40.as_deref() {
        assets.push(asset);
    }
    if let Some(asset) = screen.ascii.as_deref() {
        assets.push(asset);
    }
    if let Some(asset) = screen.text.as_deref() {
        assets.push(asset);
    }
    assets
}

fn user_json(user: &UserRecord) -> JsonValue {
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

fn session_json(session: &SessionRecord) -> JsonValue {
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

fn area_json(area: &MessageAreaRecord) -> JsonValue {
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

fn message_json(message: &oxidebbs_db::MessageRecord) -> JsonValue {
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

fn door_json(door: &DoorDefinitionRecord) -> JsonValue {
    json!({
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
    json!({
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

fn audit_json(event: &AuditEventRecord) -> JsonValue {
    json!({
        "id": event.id,
        "created_at": event.created_at,
        "event_type": event.event_type,
        "user_id": event.user_id,
        "node_number": event.node_number,
        "details": event.details
    })
}

fn print_json(value: &JsonValue) -> CliResult<()> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}

fn emit_ok(json_output: bool, message: &str, details: JsonValue) -> CliResult<()> {
    if json_output {
        print_json(&json!({"ok": true, "message": message, "details": details}))?;
    } else {
        println!("{message}");
    }
    Ok(())
}

fn print_user(user: &UserRecord) {
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

fn print_session(session: &SessionRecord) {
    println!("session: {}", session.id);
    println!("node: {}", session.node_number);
    println!("user: {:?}", session.user_id);
    println!("transport: {}", session.transport);
    println!("remote: {}", session.remote_address);
    println!("started: {}", session.started_at);
    println!("ended: {:?}", session.ended_at);
    println!("disconnect reason: {:?}", session.disconnect_reason);
}

fn print_sessions(sessions: &[SessionRecord], json_output: bool) -> CliResult<()> {
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

fn print_messages(messages: &[oxidebbs_db::MessageRecord], json_output: bool) -> CliResult<()> {
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

fn print_message(message: &oxidebbs_db::MessageRecord) {
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

fn print_audit_events(events: &[AuditEventRecord], json_output: bool) -> CliResult<()> {
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

fn prompt_line(prompt: &str, default: Option<&str>) -> CliResult<String> {
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

fn db_stats(db: &Db) -> CliResult<JsonValue> {
    Ok(json!({
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

fn db_export(db: &Db) -> CliResult<JsonValue> {
    Ok(json!({
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

fn require_json_format(format: &str) -> CliResult<()> {
    if format.eq_ignore_ascii_case("json") {
        Ok(())
    } else {
        Err(CliError::Message(format!(
            "unsupported format {format:?}; only json is supported"
        )))
    }
}

fn log_files(logs_path: &Path) -> CliResult<Vec<PathBuf>> {
    if logs_path.is_file() {
        return Ok(vec![logs_path.to_path_buf()]);
    }
    if !logs_path.exists() {
        return Ok(Vec::new());
    }
    let mut files = Vec::new();
    for entry in fs::read_dir(logs_path)? {
        let path = entry?.path();
        if path.is_file() {
            files.push(path);
        }
    }
    files.sort();
    Ok(files)
}

fn all_log_lines(logs_path: &Path) -> CliResult<Vec<String>> {
    let mut lines = Vec::new();
    for file in log_files(logs_path)? {
        let content = fs::read_to_string(file)?;
        lines.extend(content.lines().map(ToOwned::to_owned));
    }
    Ok(lines)
}

fn print_recent_log_lines(ctx: &AppContext, line_count: usize) -> CliResult<()> {
    let lines = all_log_lines(&ctx.config.paths.logs)?;
    let start = lines.len().saturating_sub(line_count);
    for line in &lines[start..] {
        println!("{line}");
    }
    if lines.is_empty() {
        println!(
            "no log files found under {}",
            ctx.config.paths.logs.display()
        );
    }
    Ok(())
}

fn get_toml_path<'a>(value: &'a toml::Value, key: &str) -> Option<&'a toml::Value> {
    let mut current = value;
    for segment in key.split('.') {
        current = current.get(segment)?;
    }
    Some(current)
}

fn set_toml_path(value: &mut toml::Value, key: &str, new_value: toml::Value) -> CliResult<()> {
    let mut segments = key.split('.').peekable();
    let mut current = value;
    while let Some(segment) = segments.next() {
        if segments.peek().is_none() {
            let table = current
                .as_table_mut()
                .ok_or_else(|| CliError::Message(format!("config path {key:?} is not a table")))?;
            table.insert(segment.to_string(), new_value);
            return Ok(());
        }
        current = current.get_mut(segment).ok_or_else(|| {
            CliError::Message(format!("config path segment {segment:?} not found"))
        })?;
    }
    Err(CliError::Message("config key cannot be empty".to_string()))
}

fn infer_toml_value(raw: &str) -> toml::Value {
    if raw.eq_ignore_ascii_case("true") {
        return toml::Value::Boolean(true);
    }
    if raw.eq_ignore_ascii_case("false") {
        return toml::Value::Boolean(false);
    }
    if let Ok(value) = raw.parse::<i64>() {
        return toml::Value::Integer(value);
    }
    toml::Value::String(raw.to_string())
}
