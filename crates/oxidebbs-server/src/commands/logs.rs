use std::thread;
use std::time::Duration;

use clap::Subcommand;
use std::path::Path;

use crate::sysop_cli::{AppContext, CliResult};

#[derive(Subcommand)]
pub enum LogsCommand {
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

pub fn run_logs(command: LogsCommand, ctx: &AppContext) -> CliResult<()> {
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

fn log_files(logs_path: &Path) -> CliResult<Vec<std::path::PathBuf>> {
    if logs_path.is_file() {
        return Ok(vec![logs_path.to_path_buf()]);
    }
    if !logs_path.exists() {
        return Ok(Vec::new());
    }
    let mut files = Vec::new();
    for entry in std::fs::read_dir(logs_path)? {
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
        let content = std::fs::read_to_string(file)?;
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
