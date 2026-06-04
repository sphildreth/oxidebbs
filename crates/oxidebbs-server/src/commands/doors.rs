use std::collections::HashMap;
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::MetadataExt;
use std::path::Path;

use clap::{Args, Subcommand};
#[cfg(unix)]
use nix::unistd::geteuid;
use serde_json::Value as JsonValue;
use serde_json::json;

use oxidebbs_core::door::DoorDefinition;
use oxidebbs_db::{
    DoorDefinitionRecord, find_door_by_key, find_door_run_by_id, insert_door_definition,
    list_door_definitions, list_door_runs, update_door_definition, update_door_enabled,
};
use oxidebbs_door::{
    DoorCaller, DoorRunRequest, DoorRunner, DryRunDoorRunner, node_runtime_dir,
    render_callinfo_bbs, render_chain_txt, render_door_sys, render_doorfile_sr,
    render_dorinfo1_def, render_pcboard_sys, runner_supports_dosemu2_cli,
};

use crate::config::DoorDefConfig;
use crate::sysop_cli::{
    AppContext, CliError, CliResult, audit, emit_ok, generated_uuid, open_database, print_json,
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
    Add(DoorAddArgs),
    Edit(DoorEditArgs),
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

#[derive(Debug, Clone, Args)]
pub struct DoorAddArgs {
    pub key: String,
    pub name: String,
    #[arg(long, default_value = "dosemu")]
    pub runner: String,
    pub working_dir: String,
    pub command: String,
    #[arg(long, default_value = "door.sys")]
    pub drop_file: String,
    #[arg(long)]
    pub exclusive: bool,
    #[arg(long, default_value_t = 30)]
    pub time_limit_minutes: u32,
    #[arg(long, default_value_t = true)]
    pub enabled: bool,
    #[arg(long, default_value_t = 0)]
    pub min_security_level: i32,
}

#[derive(Debug, Clone, Args)]
pub struct DoorEditArgs {
    pub door_key: String,
    #[arg(long)]
    pub key: Option<String>,
    #[arg(long)]
    pub name: Option<String>,
    #[arg(long)]
    pub runner: Option<String>,
    #[arg(long)]
    pub working_dir: Option<String>,
    #[arg(long)]
    pub command: Option<String>,
    #[arg(long)]
    pub drop_file: Option<String>,
    #[arg(long)]
    pub exclusive: Option<bool>,
    #[arg(long)]
    pub time_limit_minutes: Option<u32>,
    #[arg(long)]
    pub enabled: Option<bool>,
    #[arg(long)]
    pub min_security_level: Option<i32>,
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

fn first_command_token(command: &str) -> Option<&str> {
    command.trim().split_ascii_whitespace().next()
}

fn is_quoted_dos_command(command: &str) -> bool {
    command.starts_with('"') || command.starts_with('\'')
}

pub fn run_doors(command: DoorsCommand, ctx: &AppContext) -> CliResult<()> {
    let db = open_database(&ctx.config)?;
    sync_configured_doors(&db, &ctx.config)?;
    match command {
        DoorsCommand::List => {
            let doors = effective_doors(&db, &ctx.config)?;
            if ctx.json {
                print_json(&doors_json_payload(&doors))?;
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
            let door = set_door_enabled(&db, &ctx.config, &door_key, enabled)?;
            audit(
                &db,
                "door:enable",
                None,
                None,
                &format!("door {} ({}) enabled", door.key, door.id),
            )?;
            emit_ok(
                ctx.json,
                "door enabled",
                json!({"door": door_key, "enabled": enabled}),
            )?;
        }
        DoorsCommand::Disable { door_key } => {
            let enabled = false;
            let door = set_door_enabled(&db, &ctx.config, &door_key, enabled)?;
            audit(
                &db,
                "door:disable",
                None,
                None,
                &format!("door {} ({}) disabled", door.key, door.id),
            )?;
            emit_ok(
                ctx.json,
                "door disabled",
                json!({"door": door_key, "enabled": enabled}),
            )?;
        }
        DoorsCommand::Test(args) => run_door_test(args, ctx, &db)?,
        DoorsCommand::Dropfile(args) => run_door_dropfile(args, ctx, &db)?,
        DoorsCommand::Add(args) => run_door_add(args, ctx, &db)?,
        DoorsCommand::Edit(args) => run_door_edit(args, ctx, &db)?,
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

fn doors_json_payload(doors: &[DoorDefinitionRecord]) -> JsonValue {
    json!({
        "doors": doors.iter().map(door_json).collect::<Vec<_>>()
    })
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
        board_name: ctx.config.board.name.clone(),
        sysop_name: ctx.config.board.sysop_name.clone(),
        node_number: args.node,
        runtime_dir,
    };

    require_cli_door_test_mode(args.dry_run)?;

    let result = DryRunDoorRunner.run(&request)?;

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

fn require_cli_door_test_mode(dry_run: bool) -> CliResult<()> {
    if dry_run {
        return Ok(());
    }

    Err(CliError::Message(
        "interactive DOS door tests require a caller session; use --dry-run for CLI validation and launch the door from the caller Doors menu for live COM1 serial testing".to_string(),
    ))
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
        "CHAIN.TXT" => render_chain_txt(&caller, args.node, 38_400),
        "DOORFILE.SR" => render_doorfile_sr(
            &caller,
            args.node,
            38_400,
            &ctx.config.board.name,
            &ctx.config.board.sysop_name,
        ),
        "PCBOARD.SYS" => render_pcboard_sys(&caller, args.node, 38_400, &ctx.config.board.name),
        "CALLINFO.BBS" => render_callinfo_bbs(&caller, args.node, 38_400),
        other => {
            return Err(CliError::Message(format!(
                "unsupported drop-file format {other:?}; supported formats are DOOR.SYS, DORINFO1.DEF, CHAIN.TXT, DOORFILE.SR, PCBOARD.SYS, and CALLINFO.BBS"
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

fn run_door_add(args: DoorAddArgs, ctx: &AppContext, db: &oxidebbs_db::OxideDb) -> CliResult<()> {
    validate_door_fields_before_write(
        &args.key,
        &args.command,
        args.time_limit_minutes,
        &args.drop_file,
    )?;
    if find_door_by_key(db.db(), &args.key)?.is_some() {
        return Err(CliError::Message(format!(
            "door {:?} already exists; use `doors edit` to update it",
            args.key
        )));
    }
    let record = DoorDefinitionRecord {
        id: generated_uuid(db)?,
        key: args.key.clone(),
        name: args.name.clone(),
        runner: args.runner.clone(),
        working_dir: args.working_dir.clone(),
        command: args.command.clone(),
        drop_file: args.drop_file.clone(),
        exclusive: args.exclusive,
        time_limit_minutes: i64::from(args.time_limit_minutes),
        enabled: args.enabled,
        min_security_level: i64::from(args.min_security_level),
    };
    insert_door_definition(db.db(), &record)?;
    audit(
        db,
        "door:add",
        None,
        None,
        &format!(
            "door {} ({}) added runner={} command={} drop_file={} enabled={} min_security_level={}",
            record.key,
            record.id,
            record.runner,
            record.command,
            record.drop_file,
            record.enabled,
            record.min_security_level
        ),
    )?;
    emit_ok(
        ctx.json,
        "door created",
        json!({"door": args.key, "id": record.id}),
    )?;
    Ok(())
}

fn run_door_edit(args: DoorEditArgs, ctx: &AppContext, db: &oxidebbs_db::OxideDb) -> CliResult<()> {
    let existing = require_effective_door(db, &ctx.config, &args.door_key)?;
    let key = args.key.unwrap_or(existing.key);
    let name = args.name.unwrap_or(existing.name);
    let runner = args.runner.unwrap_or(existing.runner);
    let working_dir = args.working_dir.unwrap_or(existing.working_dir);
    let command = args.command.unwrap_or(existing.command);
    let drop_file = args.drop_file.unwrap_or(existing.drop_file);
    let exclusive = args.exclusive.unwrap_or(existing.exclusive);
    let time_limit_minutes = args
        .time_limit_minutes
        .map(u32::try_from)
        .transpose()
        .map_err(|_| CliError::Message("time_limit_minutes must fit in u32".to_string()))?
        .unwrap_or(u32::try_from(existing.time_limit_minutes).unwrap_or(30));
    let enabled = args.enabled.unwrap_or(existing.enabled);
    let min_security_level = args
        .min_security_level
        .unwrap_or(existing.min_security_level as i32);

    validate_door_fields_before_write(&key, &command, time_limit_minutes, &drop_file)?;

    let record = DoorDefinitionRecord {
        id: existing.id,
        key,
        name,
        runner,
        working_dir,
        command,
        drop_file,
        exclusive,
        time_limit_minutes: i64::from(time_limit_minutes),
        enabled,
        min_security_level: i64::from(min_security_level),
    };
    update_door_definition(db.db(), &record)?;
    audit(
        db,
        "door:edit",
        None,
        None,
        &format!(
            "door {} ({}) updated runner={} command={} drop_file={} enabled={} min_security_level={}",
            record.key,
            record.id,
            record.runner,
            record.command,
            record.drop_file,
            record.enabled,
            record.min_security_level
        ),
    )?;
    emit_ok(
        ctx.json,
        "door updated",
        json!({"door": record.key, "id": record.id}),
    )?;
    Ok(())
}

fn validate_door_fields_before_write(
    key: &str,
    command: &str,
    time_limit_minutes: u32,
    drop_file: &str,
) -> CliResult<()> {
    if key.trim().is_empty() {
        return Err(CliError::Message("door key must not be blank".to_string()));
    }
    if command.trim().is_empty() {
        return Err(CliError::Message(
            "door command must not be blank".to_string(),
        ));
    }
    if !(1..=240).contains(&time_limit_minutes) {
        return Err(CliError::Message(
            "time limit minutes must be between 1 and 240".to_string(),
        ));
    }
    if !matches!(
        drop_file.to_ascii_uppercase().as_str(),
        "DOOR.SYS" | "DORINFO1.DEF" | "CHAIN.TXT" | "DOORFILE.SR" | "PCBOARD.SYS" | "CALLINFO.BBS"
    ) {
        return Err(CliError::Message(format!(
            "unsupported drop-file format {drop_file:?}"
        )));
    }
    Ok(())
}

fn set_door_enabled(
    db: &oxidebbs_db::OxideDb,
    config: &crate::config::OxideConfig,
    door_key: &str,
    enabled: bool,
) -> CliResult<DoorDefinitionRecord> {
    let door = require_config_door(config, door_key)?;
    match find_door_by_key(db.db(), door_key)? {
        Some(mut existing) => {
            update_door_enabled(db.db(), &existing.id, enabled)?;
            existing.enabled = enabled;
            Ok(existing)
        }
        None => {
            let mut record = door_record_from_config(db, door, true)?;
            record.enabled = enabled;
            insert_door_definition(db.db(), &record)?;
            Ok(record)
        }
    }
}

pub(crate) fn sync_configured_doors(
    db: &oxidebbs_db::OxideDb,
    config: &crate::config::OxideConfig,
) -> CliResult<()> {
    for door in &config.doors.definitions {
        match find_door_by_key(db.db(), &door.key)? {
            Some(existing) => {
                let record =
                    door_record_from_config_with_id(door, config.doors.enabled, existing.id);
                update_door_definition(db.db(), &record)?;
            }
            None => {
                insert_door_definition(
                    db.db(),
                    &door_record_from_config(db, door, config.doors.enabled)?,
                )?;
            }
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
        by_key.entry(key).or_insert(door_record_from_config(
            db,
            config_door,
            config.doors.enabled,
        )?);
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
    let doors_root = match std::path::Path::new(&config.paths.doors).canonicalize() {
        Ok(root) => root,
        Err(error) => {
            issues.push(CheckIssue::warning(format!(
                "doors path {} is not accessible: {error}",
                config.paths.doors.display()
            )));
            Path::new("/").to_path_buf()
        }
    };
    let working_dir = match std::path::Path::new(&door.working_dir).canonicalize() {
        Ok(path) => {
            if !path.starts_with(&doors_root) {
                issues.push(CheckIssue::error(format!(
                    "door working directory {} is outside doors path {}",
                    door.working_dir,
                    config.paths.doors.display()
                )));
            }
            path
        }
        Err(error) => {
            issues.push(CheckIssue::warning(format!(
                "door working directory {} does not exist: {error}",
                door.working_dir
            )));
            std::path::PathBuf::from(&door.working_dir)
        }
    };
    if !working_dir.is_dir() {
        issues.push(CheckIssue::warning(format!(
            "door working directory {} does not exist",
            working_dir.display()
        )));
    }
    match first_command_token(&door.command) {
        Some(command_name) if is_quoted_dos_command(command_name) => {
            issues.push(CheckIssue::error(
                "quoted DOS commands are not supported yet; use DOS 8.3 paths",
            ));
        }
        Some(command_name) => {
            let command_path = if command_name.contains(':')
                || command_name.contains('\\')
                || command_name.contains('/')
            {
                std::path::PathBuf::from(command_name)
            } else {
                working_dir.join(command_name)
            };
            if !command_path.exists() {
                issues.push(CheckIssue::warning(format!(
                    "door command {} was not found under {}",
                    command_name,
                    working_dir.display()
                )));
            }
        }
        None => issues.push(CheckIssue::error("door command is empty")),
    }
    if !command_exists(&door.runner) {
        issues.push(CheckIssue::warning(format!(
            "door runner {:?} was not found on PATH",
            door.runner
        )));
    }
    if !runner_supports_dosemu2_cli(&door.runner) {
        issues.push(CheckIssue::error(format!(
            "door runner {:?} is not supported for live caller doors; use DOSEMU2 runner \"dosemu\"",
            door.runner
        )));
    }
    if let Err(issue) = validate_door_runner(&door.runner, &config.doors.allowed_runners) {
        issues.push(issue);
    }
    if !matches!(
        door.drop_file.to_ascii_uppercase().as_str(),
        "DOOR.SYS" | "DORINFO1.DEF" | "CHAIN.TXT" | "DOORFILE.SR" | "PCBOARD.SYS" | "CALLINFO.BBS"
    ) {
        issues.push(CheckIssue::error(format!(
            "drop-file format {:?} is not supported",
            door.drop_file
        )));
    }
    if !(1..=240).contains(&door.time_limit_minutes) {
        issues.push(CheckIssue::error(
            "time limit must be in 1..=240 minutes".to_string(),
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

fn resolve_runner_path(command: &str) -> Option<std::path::PathBuf> {
    let path = std::path::Path::new(command);
    if path.components().count() > 1 {
        Some(path.to_path_buf())
    } else {
        let paths = std::env::var_os("PATH")?;
        std::env::split_paths(&paths).find_map(|dir| {
            let candidate = dir.join(command);
            if candidate.is_file() {
                Some(candidate)
            } else {
                None
            }
        })
    }
}

fn validate_door_runner(runner: &str, allowed_runners: &[String]) -> Result<(), CheckIssue> {
    if !allowed_runners.iter().any(|allowed| allowed == runner) {
        return Err(CheckIssue::error(format!(
            "door runner {:?} is not allowed. expected one of {:?}",
            runner, allowed_runners
        )));
    }
    let runner_path = resolve_runner_path(runner).ok_or_else(|| {
        CheckIssue::warning(format!("door runner {:?} was not found on PATH", runner))
    })?;
    let runner_path = runner_path.canonicalize().map_err(|error| {
        CheckIssue::warning(format!(
            "door runner {:?} is not accessible: {error}",
            runner
        ))
    })?;
    if !runner_path.is_file() {
        return Err(CheckIssue::error(format!(
            "door runner {:?} is not a regular file",
            runner_path.display()
        )));
    }
    validate_runner_file_permissions(&runner_path, runner)?;
    Ok(())
}

#[cfg(unix)]
fn validate_runner_file_permissions(
    path: &std::path::Path,
    runner: &str,
) -> Result<(), CheckIssue> {
    let metadata = path.metadata().map_err(|error| {
        CheckIssue::warning(format!("door runner {runner:?} metadata error: {error}"))
    })?;
    let mode = metadata.mode();
    if mode & 0o002 != 0 {
        return Err(CheckIssue::error(format!(
            "door runner {runner:?} is world-writable; refused for safety"
        )));
    }
    if mode & 0o020 != 0 {
        return Err(CheckIssue::error(format!(
            "door runner {runner:?} is group-writable; refused for safety"
        )));
    }
    let owner = metadata.uid();
    let server_uid = geteuid().as_raw();
    if owner != 0 && owner != server_uid {
        return Err(CheckIssue::error(format!(
            "door runner {runner:?} is owned by UID {owner}, not root or server UID {server_uid}"
        )));
    }
    Ok(())
}

#[cfg(not(unix))]
fn validate_runner_file_permissions(
    path: &std::path::Path,
    runner: &str,
) -> Result<(), CheckIssue> {
    if path.is_file() {
        Ok(())
    } else {
        Err(CheckIssue::error(format!(
            "door runner {runner:?} is not a regular file"
        )))
    }
}

fn door_record_from_config(
    db: &oxidebbs_db::OxideDb,
    door: &DoorDefConfig,
    board_doors_enabled: bool,
) -> CliResult<DoorDefinitionRecord> {
    Ok(door_record_from_config_with_id(
        door,
        board_doors_enabled,
        generated_uuid(db)?,
    ))
}

fn door_record_from_config_with_id(
    door: &DoorDefConfig,
    board_doors_enabled: bool,
    id: String,
) -> DoorDefinitionRecord {
    DoorDefinitionRecord {
        id,
        key: door.key.clone(),
        name: door.name.clone(),
        runner: door.runner.clone(),
        working_dir: door.working_dir.clone(),
        command: door.command.clone(),
        drop_file: door.drop_file.clone(),
        exclusive: door.exclusive,
        time_limit_minutes: i64::from(door.time_limit_minutes),
        enabled: board_doors_enabled && door.enabled,
        min_security_level: i64::from(door.min_security_level),
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
        min_security_level: door.min_security_level as i32,
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
        "enabled": door.enabled,
        "min_security_level": door.min_security_level
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn doors_list_json_shape_matches_contract() {
        let doors = vec![DoorDefinitionRecord {
            id: "00000000-0000-4000-8000-000000000001".to_string(),
            key: "lord".to_string(),
            name: "Legend of the Red Dragon".to_string(),
            runner: "dry-run".to_string(),
            working_dir: "doors/lord".to_string(),
            command: "lord.exe".to_string(),
            drop_file: "door.sys".to_string(),
            exclusive: false,
            time_limit_minutes: 30,
            enabled: true,
            min_security_level: 0,
        }];

        let payload = doors_json_payload(&doors);
        let doors = payload
            .as_object()
            .expect("payload object")
            .get("doors")
            .expect("doors key")
            .as_array()
            .expect("doors array");
        assert_eq!(doors.len(), 1);
        let door = doors[0].as_object().expect("single door");
        assert_eq!(
            door.get("runner"),
            Some(&JsonValue::String("dry-run".into()))
        );
        assert_eq!(door.get("enabled"), Some(&JsonValue::Bool(true)));
        assert_eq!(door.get("time_limit_minutes"), Some(&JsonValue::from(30)));
    }

    #[test]
    fn check_door_uses_first_command_token_and_rejects_quoted_commands() {
        let temp = std::env::temp_dir().join(format!(
            "oxidebbs-door-command-check-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&temp);
        fs::create_dir_all(&temp).expect("temp");
        fs::write(temp.join("LORD.EXE"), b"").expect("door exe");

        let mut door = DoorDefinitionRecord {
            id: "00000000-0000-4000-8000-000000000001".to_string(),
            key: "lord".to_string(),
            name: "Legend of the Red Dragon".to_string(),
            runner: std::env::current_exe()
                .expect("current exe")
                .to_string_lossy()
                .to_string(),
            working_dir: temp.to_string_lossy().to_string(),
            command: "LORD.EXE /N1".to_string(),
            drop_file: "door.sys".to_string(),
            exclusive: false,
            time_limit_minutes: 30,
            enabled: true,
            min_security_level: 0,
        };
        let mut config: crate::config::OxideConfig =
            toml::from_str("[board]\nname = \"Test\"\n").expect("config");
        config.paths.runtime = temp.join("runtime");
        config.doors.allowed_runners = vec![door.runner.clone()];
        let _ = fs::remove_dir_all(config.paths.runtime.clone());
        fs::create_dir_all(config.paths.runtime.clone()).expect("runtime dir");

        let check = check_door(&door, &config);
        assert!(
            !check
                .issues
                .iter()
                .any(|issue| issue.message.contains("door command"))
        );

        door.command = "\"LORD.EXE\"".to_string();
        let check = check_door(&door, &config);
        assert!(check.issues.iter().any(|issue| {
            issue.level == "error"
                && issue.message.contains("quoted DOS commands")
                && issue.message.contains("DOS 8.3 paths")
        }));

        door.command = "   ".to_string();
        let check = check_door(&door, &config);
        assert!(
            check
                .issues
                .iter()
                .any(|issue| issue.level == "error" && issue.message.contains("command is empty"))
        );

        door.command = "LORD.EXE".to_string();
        door.runner = "dosbox".to_string();
        let check = check_door(&door, &config);
        assert!(check.issues.iter().any(|issue| {
            issue.level == "error"
                && issue.message.contains("not supported")
                && issue.message.contains("DOSEMU2")
        }));

        let _ = fs::remove_dir_all(temp);
    }

    #[test]
    fn check_door_enforces_runner_allowlist_and_time_limit_cap() {
        let temp =
            std::env::temp_dir().join(format!("oxidebbs-door-allowlist-{}", std::process::id()));
        let working_dir = temp.join("working");
        let runtime = temp.join("runtime");
        let runner = temp.join("runners");
        let runner_path = runner.join("dosemu");
        let _ = fs::remove_dir_all(&temp);
        fs::create_dir_all(&working_dir).expect("working");
        fs::create_dir_all(&runner).expect("runners");
        fs::create_dir_all(&runtime).expect("runtime");
        fs::write(working_dir.join("LORD.EXE"), b"").expect("door exe");
        fs::write(&runner_path, b"#!/bin/sh\necho").expect("runner fixture");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&runner_path, fs::Permissions::from_mode(0o755))
                .expect("runner mode");
        }

        let door = DoorDefinitionRecord {
            id: "00000000-0000-4000-8000-000000000001".to_string(),
            key: "lord".to_string(),
            name: "Legend of the Red Dragon".to_string(),
            runner: runner_path.to_string_lossy().to_string(),
            working_dir: working_dir.to_string_lossy().to_string(),
            command: "LORD.EXE".to_string(),
            drop_file: "DOOR.SYS".to_string(),
            exclusive: false,
            time_limit_minutes: 241,
            enabled: true,
            min_security_level: 0,
        };
        let mut config: crate::config::OxideConfig =
            toml::from_str("[board]\nname = \"Test\"\n").expect("config");
        config.paths.runtime = runtime;
        config.doors.allowed_runners = vec![runner_path.to_string_lossy().to_string()];
        fs::create_dir_all(&working_dir).expect("working");

        let check = check_door(&door, &config);
        assert!(check.issues.iter().any(|issue| {
            issue.level == "error" && issue.message.contains("time limit must be in 1..=240")
        }));

        let mut good = door;
        good.time_limit_minutes = 240;
        let check = check_door(&good, &config);
        assert!(
            !check
                .issues
                .iter()
                .any(|issue| issue.level == "error"
                    && issue.message.contains("time limit must be in"))
        );
        let mut disallowed = good.clone();
        disallowed.runner = "dosbox".to_string();
        let check = check_door(&disallowed, &config);
        assert!(
            check
                .issues
                .iter()
                .any(|issue| issue.level == "error" && issue.message.contains("not allowed"))
        );

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(&runner_path)
                .expect("runner stat")
                .permissions()
                .mode();
            fs::set_permissions(&runner_path, fs::Permissions::from_mode(mode | 0o020))
                .expect("set group write");
            let check = check_door(&good, &config);
            assert!(check
                .issues
                .iter()
                .any(|issue| issue.level == "error" && issue.message.contains("group-writable")));
        }
        let _ = fs::remove_dir_all(temp);
    }

    #[test]
    fn cli_door_test_requires_dry_run_for_non_caller_context() {
        assert!(require_cli_door_test_mode(true).is_ok());

        let error = require_cli_door_test_mode(false).expect_err("expected error");

        assert!(error.to_string().contains("require a caller session"));
        assert!(error.to_string().contains("--dry-run"));
        assert!(error.to_string().contains("COM1 serial"));
    }

    #[test]
    fn sync_configured_doors_updates_existing_db_record_from_config() {
        let db = oxidebbs_db::OxideDb::open_memory().expect("open db");
        let door_id = "00000000-0000-4000-8000-000000000901".to_string();
        insert_door_definition(
            db.db(),
            &DoorDefinitionRecord {
                id: door_id.clone(),
                key: "oxide-check".to_string(),
                name: "Old Name".to_string(),
                runner: "dosbox".to_string(),
                working_dir: "./old".to_string(),
                command: "OLD.EXE".to_string(),
                drop_file: "DOOR.SYS".to_string(),
                exclusive: true,
                time_limit_minutes: 1,
                enabled: false,
                min_security_level: 0,
            },
        )
        .expect("insert stale door");
        let config: crate::config::OxideConfig = toml::from_str(
            r#"
[board]
name = "Test"

[doors]
enabled = true

[[doors.definitions]]
key = "oxide-check"
name = "Oxide Door Check"
runner = "dosemu2"
working_dir = "./tools/doors/oxide-door-check/dist"
command = "OXIDECHK.EXE"
drop_file = "DORINFO1.DEF"
exclusive = false
time_limit_minutes = 5
enabled = true
"#,
        )
        .expect("config");

        sync_configured_doors(&db, &config).expect("sync doors");

        let synced = find_door_by_key(db.db(), "oxide-check")
            .expect("find door")
            .expect("door exists");
        assert_eq!(synced.id, door_id);
        assert_eq!(synced.name, "Oxide Door Check");
        assert_eq!(synced.runner, "dosemu2");
        assert_eq!(synced.working_dir, "./tools/doors/oxide-door-check/dist");
        assert_eq!(synced.command, "OXIDECHK.EXE");
        assert_eq!(synced.drop_file, "DORINFO1.DEF");
        assert!(!synced.exclusive);
        assert_eq!(synced.time_limit_minutes, 5);
        assert!(synced.enabled);
    }
}
