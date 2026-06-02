use std::fs;
use std::io::{self, Write};

use clap::{Subcommand, ValueEnum};
use serde_json::Value as JsonValue;
use serde_json::json;

use crate::config::TerminalCapabilities;
use crate::setup::install_default_assets;
use crate::sysop_cli::{AppContext, CliError, CliResult, emit_ok, print_json};
use oxidebbs_term::{decode_cp437, encode_cp437, render_plain_text};

#[derive(Debug)]
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

    fn to_json(&self) -> JsonValue {
        json!({"level": self.level, "message": self.message})
    }
}

#[derive(Subcommand)]
pub enum AnsiCommand {
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
        input: std::path::PathBuf,
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
pub enum Encoding {
    Utf8,
    Cp437,
}

pub fn run_ansi(command: AnsiCommand, ctx: &AppContext) -> CliResult<()> {
    match command {
        AnsiCommand::List => {
            let screens = ctx
                .config
                .screens
                .iter()
                .map(|(name, screen)| {
                    json!({"name": name, "asset": screen.asset_for(TerminalCapabilities::ansi_80())})
                })
                .collect::<Vec<_>>();
            if ctx.json {
                print_json(&JsonValue::Array(screens))?;
            } else {
                for (name, screen) in &ctx.config.screens {
                    let asset = screen
                        .asset_for(TerminalCapabilities::ansi_80())
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
            let summary =
                install_default_assets(&ctx.config.paths.ansi, &ctx.config.paths.screens)?;
            emit_ok(
                ctx.json,
                "default ANSI/screen assets are present",
                json!({
                    "ansi": ctx.config.paths.ansi,
                    "screens": ctx.config.paths.screens,
                    "installed": summary.installed,
                    "skipped": summary.skipped
                }),
            )?;
        }
        AnsiCommand::Preview { screen_name } => {
            let asset_path = load_screen_asset(ctx, &screen_name, true)?;
            let bytes = fs::read(&asset_path)?;
            if ctx.json {
                print_json(&json!({
                    "screen": screen_name,
                    "preview": render_plain_text(&bytes)
                }))?;
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

fn load_screen_asset(
    ctx: &AppContext,
    screen_name: &str,
    ansi: bool,
) -> CliResult<std::path::PathBuf> {
    let screen = ctx
        .config
        .screens
        .get(screen_name)
        .ok_or_else(|| CliError::Message(format!("screen {screen_name:?} was not found")))?;
    let capabilities = if ansi {
        TerminalCapabilities::ansi_80()
    } else {
        TerminalCapabilities::plain_text()
    };
    let asset = screen
        .asset_for(capabilities)
        .ok_or_else(|| CliError::Message(format!("screen {screen_name:?} has no usable asset")))?;
    Ok(ctx.config.paths.screens.join(asset))
}

fn validate_screen(ctx: &AppContext, screen_name: &str) -> Vec<CheckIssue> {
    let mut issues = Vec::new();
    let Some(screen) = ctx.config.screens.get(screen_name) else {
        issues.push(CheckIssue::error(format!(
            "screen {screen_name:?} is not configured"
        )));
        return issues;
    };

    for asset in screen_assets(screen) {
        let path = ctx.config.paths.screens.join(asset);
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

fn screen_assets(screen: &crate::config::ScreenConfig) -> Vec<&str> {
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
