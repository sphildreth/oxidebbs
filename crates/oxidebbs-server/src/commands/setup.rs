use std::path::PathBuf;

use clap::Args;
use serde_json::json;

use oxidebbs_db::{
    MessageAreaRecord, OxideDb, UserRecord, find_message_area_by_key, find_user_by_alias_ci,
    insert_message_area, insert_user,
};

use crate::setup;
use crate::sysop_cli::{
    CliError, CliResult, current_timestamp, generated_uuid, hash_password, print_json,
};

#[derive(Debug, Clone, Args)]
pub struct SetupArgs {
    /// Output configuration file path
    #[arg(short, long, default_value = "config/oxidebbs.toml")]
    pub output: PathBuf,

    /// Overwrite an existing output file
    #[arg(long)]
    pub force: bool,

    /// Board name for non-interactive setup
    #[arg(long)]
    pub board_name: Option<String>,

    /// Initial sysop alias for non-interactive setup
    #[arg(long)]
    pub sysop_alias: Option<String>,

    /// Initial sysop password for non-interactive setup
    #[arg(long)]
    pub sysop_password: Option<String>,

    /// Telnet port for non-interactive setup
    #[arg(long)]
    pub telnet_port: Option<u16>,

    /// Node count for non-interactive setup
    #[arg(long)]
    pub nodes: Option<u16>,

    /// Skip bundled sample ANSI screen directories
    #[arg(long)]
    pub no_sample_ansi: bool,
}

pub fn run_setup_command(
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
