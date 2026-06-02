use clap::Args;
use serde_json::json;
use tracing::info;

use crate::commands::config::{CheckIssue, print_check_issues, validate_runtime};
use crate::serve;
use crate::sysop_cli::{AppContext, CliError, CliResult, print_json};

#[derive(Debug, Clone, Args, Default)]
pub struct ServeArgs {
    /// Override the telnet bind address
    #[arg(long)]
    pub bind: Option<String>,

    /// Validate startup prerequisites without listening for callers
    #[arg(long)]
    pub dry_run: bool,
}

pub async fn run_serve(args: ServeArgs, ctx: &AppContext) -> CliResult<()> {
    let mut config = ctx.config.clone();
    if let Some(bind) = args.bind {
        config.telnet.bind = bind;
    }

    if args.dry_run {
        let issues = validate_runtime(&config, &ctx.config_path);
        let errors = issues.iter().filter(|issue| issue.level == "error").count();
        if errors == 0 {
            serve::validate_startup_database(&config)?;
        }
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
    serve::run(&config, &ctx.config_path).await?;
    Ok(())
}
