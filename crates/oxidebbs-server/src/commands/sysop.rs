use std::path::Path;
use std::time::{Duration, Instant};

use crate::control::CONTROL_SOCKET_NAME;
use crate::sysop_cli::{AppContext, CliError, CliResult};
use oxidebbs_sysop::app::{AppConfig, run_tui};
use oxidebbs_sysop::services::control_client::is_socket_available;
use tokio::sync::oneshot;
use tokio::time::sleep;

const EMBEDDED_SERVE_STARTUP_TIMEOUT: Duration = Duration::from_secs(5);
const CONTROL_SOCKET_POLL_INTERVAL: Duration = Duration::from_millis(50);

pub async fn run_sysop_tui(
    ctx: &AppContext,
    readonly: bool,
    confirm_quit: bool,
    connect_only: bool,
    theme_name: &str,
) -> CliResult<()> {
    let socket_path = ctx.config.paths.runtime.join(CONTROL_SOCKET_NAME);
    let config = AppConfig {
        config_path: ctx.config_path.clone(),
        readonly,
        confirm_quit,
        tick_rate: std::time::Duration::from_millis(250),
        db_path: Some(ctx.config.database.path.clone()),
        logs_path: Some(ctx.config.paths.logs.clone()),
        screens_path: Some(ctx.config.paths.screens.clone()),
        control_socket_path: Some(socket_path.clone()),
        node_count: ctx.config.nodes.count,
        theme_name: theme_name.to_string(),
        board_name: ctx.config.board.name.clone(),
    };

    if connect_only || is_socket_available(&socket_path) {
        return run_tui_as_cli(config).await;
    }

    run_tui_with_embedded_serve(ctx, config, &socket_path).await
}

async fn run_tui_as_cli(config: AppConfig) -> CliResult<()> {
    run_tui(config)
        .await
        .map_err(|e| CliError::Message(format!("TUI error: {e}")))?;
    Ok(())
}

async fn run_tui_with_embedded_serve(
    ctx: &AppContext,
    config: AppConfig,
    socket_path: &Path,
) -> CliResult<()> {
    let server_config = ctx.config.clone();
    let config_path = ctx.config_path.clone();
    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
    let server_task = tokio::spawn(async move {
        crate::serve::run_until_shutdown(&server_config, &config_path, async move {
            let _ = shutdown_rx.await;
            Ok(())
        })
        .await
    });

    if let Err(startup_error) = wait_for_control_socket(socket_path, &server_task).await {
        let _ = shutdown_tx.send(());
        return match server_task.await {
            Ok(Ok(())) => Err(startup_error),
            Ok(Err(error)) => Err(error.into()),
            Err(error) => Err(CliError::Message(format!(
                "embedded serve task failed: {error}"
            ))),
        };
    }

    let tui_result = run_tui_as_cli(config).await;
    let _ = shutdown_tx.send(());
    let serve_result = server_task.await;

    tui_result?;
    match serve_result {
        Ok(Ok(())) => Ok(()),
        Ok(Err(error)) => Err(error.into()),
        Err(error) => Err(CliError::Message(format!(
            "embedded serve task failed: {error}"
        ))),
    }
}

async fn wait_for_control_socket(
    socket_path: &Path,
    server_task: &tokio::task::JoinHandle<crate::serve::ServeResult<()>>,
) -> CliResult<()> {
    let deadline = Instant::now() + EMBEDDED_SERVE_STARTUP_TIMEOUT;
    loop {
        if is_socket_available(socket_path) {
            return Ok(());
        }
        if server_task.is_finished() {
            return Err(CliError::Message(
                "embedded serve exited before opening the control socket".to_string(),
            ));
        }
        if Instant::now() >= deadline {
            return Err(CliError::Message(format!(
                "embedded serve did not open {} within {} seconds",
                socket_path.display(),
                EMBEDDED_SERVE_STARTUP_TIMEOUT.as_secs()
            )));
        }
        sleep(CONTROL_SOCKET_POLL_INTERVAL).await;
    }
}
