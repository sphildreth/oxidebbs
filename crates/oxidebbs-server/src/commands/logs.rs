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
    collect_log_files(logs_path, &mut files)?;
    files.sort();
    Ok(files)
}

fn collect_log_files(path: &Path, files: &mut Vec<std::path::PathBuf>) -> CliResult<()> {
    for entry in std::fs::read_dir(path)? {
        let path = entry?.path();
        if path.is_dir() {
            collect_log_files(&path, files)?;
        } else if path.is_file() {
            files.push(path);
        }
    }
    Ok(())
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
            "no log lines found under {}",
            ctx.config.paths.logs.display()
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let mut dir = std::env::temp_dir();
        dir.push(format!(
            "oxidebbs-logs-{name}-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system time should be valid")
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    #[test]
    fn all_log_lines_reads_nested_door_logs() {
        let dir = temp_dir("nested");
        std::fs::write(dir.join("oxidebbs-server.log"), "server-start\n")
            .expect("write server log");
        let doors = dir.join("doors");
        std::fs::create_dir_all(&doors).expect("create door logs dir");
        std::fs::write(doors.join("door.stdout.log"), "door-output\n").expect("write door log");

        let lines = all_log_lines(&dir).expect("read logs");
        assert!(lines.iter().any(|line| line == "server-start"));
        assert!(lines.iter().any(|line| line == "door-output"));
    }
}
