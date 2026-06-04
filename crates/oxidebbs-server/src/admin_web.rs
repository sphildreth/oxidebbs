use std::net::SocketAddr;
use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use tokio::net::TcpListener;
use tracing::info;

use oxidebbs_db::OxideDb;

use crate::admin_status::{AdminStatusPayload, build_admin_status_payload};
use crate::config::OxideConfig;
use crate::control::ServerRuntime;
use crate::serve::{ServeError, ServeResult};

#[derive(Clone)]
struct AdminWebState {
    config: Arc<OxideConfig>,
    db: Arc<OxideDb>,
    runtime: Arc<ServerRuntime>,
}

pub(crate) async fn start_admin_web(
    config: Arc<OxideConfig>,
    db: Arc<OxideDb>,
    runtime: Arc<ServerRuntime>,
) -> ServeResult<tokio::task::JoinHandle<ServeResult<()>>> {
    let bind: SocketAddr = config
        .admin_web
        .bind
        .parse()
        .map_err(|error| ServeError::Config(format!("invalid admin_web.bind: {error}")))?;
    let listener = TcpListener::bind(bind).await?;
    let app = admin_router(AdminWebState {
        config,
        db,
        runtime,
    });

    info!(bind = %listener.local_addr()?, "admin web status listener started");
    Ok(tokio::spawn(async move {
        axum::serve(listener, app)
            .await
            .map_err(ServeError::Network)
    }))
}

fn admin_router(state: AdminWebState) -> Router {
    Router::new()
        .route("/status", get(status_handler))
        .with_state(state)
}

async fn status_handler(
    State(state): State<AdminWebState>,
) -> Result<Json<AdminStatusPayload>, StatusCode> {
    if !state.config.admin_web.public_status_enabled {
        return Err(StatusCode::NOT_FOUND);
    }

    let runtime_status = state.runtime.status();
    let payload = build_admin_status_payload(&state.config, &state.db, Some(runtime_status))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(payload))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config(public_status_enabled: bool) -> Arc<OxideConfig> {
        let toml = format!(
            r#"
[board]
name = "Example BBS"

[admin_web]
enabled = true
public_status_enabled = {public_status_enabled}
"#
        );
        Arc::new(toml::from_str(&toml).expect("parse config"))
    }

    fn test_state(public_status_enabled: bool) -> AdminWebState {
        AdminWebState {
            config: test_config(public_status_enabled),
            db: Arc::new(OxideDb::open_memory().expect("open DB")),
            runtime: Arc::new(ServerRuntime::new("Example BBS".to_string(), 4, 4, 60)),
        }
    }

    #[tokio::test]
    async fn status_handler_returns_public_status_payload() {
        let response = status_handler(State(test_state(true)))
            .await
            .expect("status response");

        assert_eq!(response.0.board, "Example BBS");
        assert_eq!(response.0.nodes.total, 4);
        assert!(response.0.runtime.live);
    }

    #[tokio::test]
    async fn status_handler_is_hidden_when_public_status_is_disabled() {
        let error = status_handler(State(test_state(false)))
            .await
            .expect_err("status is hidden");

        assert_eq!(error, StatusCode::NOT_FOUND);
    }
}
