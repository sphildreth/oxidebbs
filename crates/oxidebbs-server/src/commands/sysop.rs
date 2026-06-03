use crate::control::CONTROL_SOCKET_NAME;
use crate::sysop_cli::{AppContext, CliError, CliResult};
use oxidebbs_sysop::app::{AppConfig, run_tui};

pub async fn run_sysop_tui(ctx: &AppContext, readonly: bool) -> CliResult<()> {
    let config = AppConfig {
        config_path: ctx.config_path.clone(),
        readonly,
        tick_rate: std::time::Duration::from_millis(250),
        db_path: Some(ctx.config.database.path.clone()),
        logs_path: Some(ctx.config.paths.logs.clone()),
        screens_path: Some(ctx.config.paths.screens.clone()),
        control_socket_path: Some(ctx.config.paths.runtime.join(CONTROL_SOCKET_NAME)),
        node_count: ctx.config.nodes.count,
        board_name: ctx.config.board.name.clone(),
    };
    run_tui(config)
        .await
        .map_err(|e| CliError::Message(format!("TUI error: {e}")))?;
    Ok(())
}
