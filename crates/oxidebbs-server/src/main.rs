mod config;
mod setup;

use std::path::PathBuf;

use clap::{Parser, Subcommand};
use tracing::info;

use config::OxideConfig;
use oxidebbs_db::OxideDb;
use oxidebbs_sysop::{SysopConsoleSnapshot, render_sysop_console_text};

#[derive(Parser)]
#[command(
    name = "oxidebbs",
    about = "OxideBBS — Rust-native BBS engine for telnet callers",
    version
)]
struct Cli {
    /// Path to the TOML configuration file
    #[arg(
        short,
        long,
        default_value = "config/oxidebbs.example.toml",
        global = true
    )]
    config: PathBuf,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Start the BBS server
    Serve,

    /// Validate the configuration file and exit
    Check,

    /// Create a starter configuration file interactively
    Setup {
        /// Output configuration file path
        #[arg(short, long, default_value = "config/oxidebbs.toml")]
        output: PathBuf,

        /// Overwrite an existing output file
        #[arg(long)]
        force: bool,
    },

    /// Run local sysop/admin commands
    Admin {
        #[command(subcommand)]
        command: AdminCommand,
    },
}

#[derive(Subcommand)]
enum AdminCommand {
    /// List users
    Users,

    /// Reset a user's password hash
    ResetPassword {
        user_id: String,
        password_hash: String,
    },

    /// List active node sessions
    Nodes,

    /// Show recent audit events
    RecentCalls {
        #[arg(short, long, default_value_t = 10)]
        limit: i64,
    },

    /// Parse and validate a doors.toml file
    TestDoorConfig { path: PathBuf },

    /// Render a text preview of the local Ratatui sysop console
    ConsolePreview,
}

fn main() {
    let cli = Cli::parse();
    let command = cli.command.unwrap_or(Command::Serve);

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    match command {
        Command::Setup { output, force } => {
            if let Err(error) = setup::run_setup(&output, force) {
                eprintln!("error: {error}");
                std::process::exit(1);
            }
            println!(
                "setup complete: wrote configuration to {}",
                output.display()
            );
            println!("directories are prepared for a starter OxideBBS installation");
        }
        Command::Check | Command::Admin { .. } | Command::Serve => {
            let config = match OxideConfig::load(&cli.config) {
                Ok(config) => {
                    info!(path = %cli.config.display(), "configuration loaded");
                    config
                }
                Err(error) => {
                    eprintln!("error: {error}");
                    std::process::exit(1);
                }
            };

            match command {
                Command::Check => {
                    println!("configuration OK: {}", cli.config.display());
                    println!("  board:          {}", config.board.name);
                    println!("  telnet bind:    {}", config.telnet.bind);
                    println!("  database path:  {}", config.database.path.display());
                    println!("  nodes:          {}", config.nodes.count);
                    println!("  doors defined:  {}", config.doors.definitions.len());
                }
                Command::Admin { command } => run_admin(command, &config),
                Command::Serve => {
                    info!(board = %config.board.name, "starting OxideBBS");
                    info!(bind = %config.telnet.bind, "telnet listener");
                    info!(nodes = config.nodes.count, "node slots");
                    println!(
                        "OxideBBS \"{}\" — telnet {} with {} node(s)",
                        config.board.name, config.telnet.bind, config.nodes.count
                    );
                    println!("Server startup is not yet implemented. Config loading works.");
                }
                Command::Setup { .. } => unreachable!("setup handled above"),
            }
        }
    }
}

fn run_admin(command: AdminCommand, config: &OxideConfig) {
    match command {
        AdminCommand::Users => {
            let db = open_database(config);
            for user in oxidebbs_sysop::list_users(db.db()).unwrap_or_else(exit_with_error) {
                println!(
                    "{}\t{}\tlevel={}\tstatus={}",
                    user.id, user.alias, user.security_level, user.status
                );
            }
        }
        AdminCommand::ResetPassword {
            user_id,
            password_hash,
        } => {
            let db = open_database(config);
            oxidebbs_sysop::reset_password(db.db(), &user_id, &password_hash)
                .unwrap_or_else(exit_with_error);
            println!("password hash updated for {user_id}");
        }
        AdminCommand::Nodes => {
            let db = open_database(config);
            for session in oxidebbs_sysop::list_nodes(db.db()).unwrap_or_else(exit_with_error) {
                println!(
                    "node {}\t{}\t{}",
                    session.node_number, session.transport, session.remote_address
                );
            }
        }
        AdminCommand::RecentCalls { limit } => {
            let db = open_database(config);
            for event in
                oxidebbs_sysop::show_recent_calls(db.db(), limit).unwrap_or_else(exit_with_error)
            {
                println!(
                    "{}\t{}\tnode={:?}\tuser={:?}\t{}",
                    event.created_at,
                    event.event_type,
                    event.node_number,
                    event.user_id,
                    event.details
                );
            }
        }
        AdminCommand::TestDoorConfig { path } => {
            let contents = std::fs::read_to_string(&path).unwrap_or_else(|error| {
                eprintln!("error: failed to read {}: {error}", path.display());
                std::process::exit(1);
            });
            let check = oxidebbs_sysop::test_door_config(&contents).unwrap_or_else(exit_with_error);
            println!(
                "door config OK: {} definition(s), {} enabled",
                check.definitions, check.enabled
            );
        }
        AdminCommand::ConsolePreview => {
            let db = open_database(config);
            let active_nodes = oxidebbs_sysop::list_nodes(db.db())
                .map(|nodes| nodes.len())
                .unwrap_or_else(exit_with_error);
            let recent_calls = oxidebbs_sysop::show_recent_calls(db.db(), 5)
                .unwrap_or_else(exit_with_error)
                .into_iter()
                .map(|event| format!("{} {}", event.created_at, event.event_type))
                .collect();
            let snapshot = SysopConsoleSnapshot {
                board_name: config.board.name.clone(),
                active_nodes,
                recent_calls,
            };
            println!("{}", render_sysop_console_text(&snapshot, 60, 10));
        }
    }
}

fn open_database(config: &OxideConfig) -> OxideDb {
    OxideDb::open_or_create(&config.database.path).unwrap_or_else(|error| {
        eprintln!(
            "error: failed to open database {}: {error}",
            config.database.path.display()
        );
        std::process::exit(1);
    })
}

fn exit_with_error<T, E: std::fmt::Display>(error: E) -> T {
    eprintln!("error: {error}");
    std::process::exit(1);
}
