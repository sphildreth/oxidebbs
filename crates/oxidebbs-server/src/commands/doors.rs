use std::collections::HashMap;
use std::fs;

use clap::{Args, Subcommand};
use serde_json::Value as JsonValue;
use serde_json::json;

use oxidebbs_core::door::DoorDefinition;
use oxidebbs_db::{
    DoorDefinitionRecord, find_door_by_key, find_door_run_by_id, insert_door_definition,
    list_door_definitions, list_door_runs, update_door_enabled,
};
use oxidebbs_door::{
    DoorCaller, DoorRunRequest, DoorRunner, DosBoxRunner, DryRunDoorRunner, node_runtime_dir,
    render_door_sys, render_dorinfo1_def,
};

use crate::config::DoorDefConfig;
use crate::sysop_cli::{
    AppContext, CliError, CliResult, emit_ok, generated_uuid, open_database, print_json,
    require_config_door, require_user,
};

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

#[derive(Subcommand)]
pub enum DoorsCommand {
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
pub struct DoorTestArgs {
    pub door_key: String,
    #[arg(long)]
    pub user: String,
    #[arg(long, default_value_t = 1)]
    pub node: u16,
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Debug, Clone, Args)]
pub struct DoorDropfileArgs {
    pub door_key: String,
    #[arg(long)]
    pub user: String,
    #[arg(long)]
    pub node: u16,
    #[arg(long, default_value = "door.sys")]
    pub format: String,
    #[arg(long)]
    pub output: Option<std::path::PathBuf>,
}

#[derive(Subcommand)]
pub enum DoorRunsCommand {
    List {
        #[arg(short, long, default_value_t = 25)]
        limit: i64,
    },
    Show {
        run_id: String,
    },
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

fn print_check_issues(issues: &[CheckIssue]) {
    for issue in issues {
        println!("{}: {}", issue.level, issue.message);
    }
}

fn command_exists(command: &str) -> bool {
    let path = std::path::Path::new(command);
    if path.components().count() > 1 {
        return path.exists();
    }
    let Some(paths) = std::env::var_os("PATH") else {
        return false;
    };
    std::env::split_paths(&paths).any(|dir| dir.join(command).is_file())
}

pub fn run_doors(command: DoorsCommand, ctx: &AppContext) -> CliResult<()> {
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
                        oxidebbs_door::cleanup_node_runtime_dir(&path)?;
                    }
                }
            }
            emit_ok(ctx.json, "door runtime directories cleaned", json!({}))?;
        }
    }
    Ok(())
}

pub fn run_door_test(
    args: DoorTestArgs,
    ctx: &AppContext,
    db: &oxidebbs_db::OxideDb,
) -> CliResult<()> {
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

pub fn run_door_dropfile(
    args: DoorDropfileArgs,
    ctx: &AppContext,
    db: &oxidebbs_db::OxideDb,
) -> CliResult<()> {
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
    db: &oxidebbs_db::OxideDb,
    config: &crate::config::OxideConfig,
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

pub(crate) fn sync_configured_doors(
    db: &oxidebbs_db::OxideDb,
    config: &crate::config::OxideConfig,
) -> CliResult<()> {
    for door in &config.doors.definitions {
        if find_door_by_key(db.db(), &door.key)?.is_none() {
            insert_door_definition(db.db(), &door_record_from_config(db, door)?)?;
        }
    }
    Ok(())
}

pub(crate) fn effective_doors(
    db: &oxidebbs_db::OxideDb,
    config: &crate::config::OxideConfig,
) -> CliResult<Vec<DoorDefinitionRecord>> {
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

fn require_effective_door(
    db: &oxidebbs_db::OxideDb,
    config: &crate::config::OxideConfig,
    key: &str,
) -> CliResult<DoorDefinitionRecord> {
    effective_doors(db, config)?
        .into_iter()
        .find(|door| door.key.eq_ignore_ascii_case(key))
        .ok_or_else(|| CliError::Message(format!("door {key:?} was not found")))
}

fn check_door(door: &DoorDefinitionRecord, config: &crate::config::OxideConfig) -> DoorCheck {
    let mut issues = Vec::new();
    let working_dir = std::path::PathBuf::from(&door.working_dir);
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
        issues.push(CheckIssue::error(
            "time limit must be greater than 0".to_string(),
        ));
    }
    if let Err(error) = std::fs::create_dir_all(&config.paths.runtime) {
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

fn door_record_from_config(
    db: &oxidebbs_db::OxideDb,
    door: &DoorDefConfig,
) -> CliResult<DoorDefinitionRecord> {
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

fn door_caller(user: &oxidebbs_db::UserRecord) -> DoorCaller {
    DoorCaller {
        alias: user.alias.clone(),
        real_name: user.real_name.clone(),
        location: "Local".to_string(),
        security_level: i32::try_from(user.security_level).unwrap_or(0),
        minutes_remaining: 30,
    }
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
