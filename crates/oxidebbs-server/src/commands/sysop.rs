use crate::sysop_cli::{AppContext, CliError, CliResult};

pub async fn run_sysop_tui(ctx: &AppContext) -> CliResult<()> {
    let config = oxidebbs_sysop::AppConfig {
        config_path: ctx.config_path.clone(),
        readonly: false,
        tick_rate: std::time::Duration::from_millis(250),
        db_path: Some(ctx.config.database.path.clone()),
        control_socket_path: Some(ctx.config.paths.runtime.join("control.sock")),
        node_count: ctx.config.nodes.count,
        board_name: ctx.config.board.name.clone(),
    };
    oxidebbs_sysop::run_tui(config)
        .await
        .map_err(|e| CliError::Message(format!("sysop tui failed: {e}")))?;
    Ok(())
}

pub fn run_sysop_preview(ctx: &AppContext) -> CliResult<()> {
    use oxidebbs_db::{list_active_sessions, list_audit_events};
    use oxidebbs_sysop::{SysopConsoleSnapshot, render_sysop_console_text};
    let db = crate::sysop_cli::open_database(&ctx.config)?;
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
