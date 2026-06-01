use oxidebbs_db::{list_active_sessions, list_audit_events};
use oxidebbs_sysop::{SysopConsoleSnapshot, render_sysop_console_text};

use crate::sysop_cli::{AppContext, CliResult, open_database};

pub fn run_sysop_preview(ctx: &AppContext) -> CliResult<()> {
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
