mod config;

use std::path::PathBuf;

use clap::{Parser, Subcommand};
use tracing::info;

use config::OxideConfig;

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
}

fn main() {
    let cli = Cli::parse();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

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

    match cli.command.unwrap_or(Command::Serve) {
        Command::Check => {
            println!("configuration OK: {}", cli.config.display());
            println!("  board:          {}", config.board.name);
            println!("  telnet bind:    {}", config.telnet.bind);
            println!("  database path:  {}", config.database.path.display());
            println!("  nodes:          {}", config.nodes.count);
            println!("  doors defined:  {}", config.doors.definitions.len());
        }
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
    }
}
