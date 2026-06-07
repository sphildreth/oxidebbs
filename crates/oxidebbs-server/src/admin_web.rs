use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant, UNIX_EPOCH};

use argon2::{Algorithm, Argon2, Params, PasswordHash, PasswordVerifier, Version};
use axum::extract::{ConnectInfo, Path as AxumPath, Request, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Form, Json, Router};
use rand_core::OsRng;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use time::OffsetDateTime;
use tokio::net::TcpListener;
use tokio::sync::RwLock;
use tracing::{info, warn};

use oxidebbs_db::{
    AuditEventRecord, OxideDb, UserRecord, find_user_by_alias_ci, insert_audit_event,
    list_audit_events, list_door_definitions, list_door_runs, list_message_areas,
    list_network_areas, list_network_links, list_network_messages, list_network_packets,
    list_network_poll_logs, list_network_profiles, list_oxidenet_applications, list_oxidenet_nodes,
    list_users, summarize_network_packets,
};
use oxidebbs_sysop::services::database_service::{DatabaseAdminService, DoctorStatus};

use crate::admin_status::{AdminStatusPayload, build_admin_status_payload};
use crate::config::{Argon2Config, OxideConfig};
use crate::control::ServerRuntime;
use crate::serve::CallerResources;
use crate::serve::{ServeError, ServeResult};

const SESSION_COOKIE_NAME: &str = "oxidebbs_session";
const CSRF_HEADER_NAME: &str = "x-csrf-token";
const REPLAY_NONCE_HEADER_NAME: &str = "x-oxidebbs-nonce";
const REPLAY_TIMESTAMP_HEADER_NAME: &str = "x-oxidebbs-timestamp";
const LOGIN_RATE_LIMIT_WINDOW: Duration = Duration::from_secs(60);
const MUTATION_RATE_LIMIT_WINDOW: Duration = Duration::from_secs(60);
const DUMMY_PASSWORD_HASH: &str = "$argon2id$v=19$m=19456,t=2,p=1$b3hpZGViYnMtZHVtbXktYXV0aC1zYWx0$CNvsc4yCQyC6gccREXpHZ6l9604svk9VP98AyAVSMtY";
const ADMIN_ROOT_HTML: &str = r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>OxideBBS Monitoring</title>
  <style>
    :root { color-scheme: light dark; font-family: system-ui, sans-serif; }
    body { margin: 2rem; max-width: 52rem; line-height: 1.5; }
    code { background: rgba(127, 127, 127, 0.16); padding: 0.1rem 0.25rem; }
  </style>
</head>
<body>
  <h1>OxideBBS Monitoring</h1>
  <p>The monitoring HTTP listener is running.</p>
  <p><a href="/health">Health check JSON</a> runs doctor checks and returns HTTP 200 when healthy.</p>
  <p><a href="/status">Public status JSON</a> is available when <code>public_status_enabled = true</code>.</p>
  <p>Authenticated read-only JSON endpoints start with <code>/api/</code> and use <code>/csrf-token</code> plus <code>POST /login</code>.</p>
  <p>This listener speaks HTTP directly. Use a local TLS reverse proxy for HTTPS.</p>
</body>
</html>
"#;

#[derive(Clone)]
struct AdminWebState {
    config: Arc<OxideConfig>,
    db: Arc<OxideDb>,
    runtime: Arc<ServerRuntime>,
    caller_resources: Option<CallerResources>,
    rate_limiter: Arc<RwLock<RateLimiter>>,
    sessions: Arc<RwLock<SessionStore>>,
}

#[derive(Default)]
struct RateLimiter {
    attempts: HashMap<String, Vec<Instant>>,
}

impl RateLimiter {
    fn check_rate_limit(&mut self, key: &str, max_attempts: usize, window: Duration) -> bool {
        let now = Instant::now();
        let attempts = self.attempts.entry(key.to_string()).or_default();
        attempts.retain(|attempt| {
            now.checked_duration_since(*attempt)
                .is_some_and(|age| age < window)
        });

        if attempts.len() >= max_attempts {
            return false;
        }

        attempts.push(now);
        true
    }
}

#[derive(Default)]
struct SessionStore {
    sessions: HashMap<String, SessionData>,
}

struct SessionData {
    authenticated: bool,
    user_id: Option<String>,
    csrf_token: String,
    csrf_issued_at: Instant,
    last_seen_at: Instant,
    used_nonces: HashMap<String, Instant>,
}

#[derive(Clone)]
struct AuthenticatedSession {
    session_id: String,
    user_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SessionError {
    Missing,
    Unauthenticated,
    InvalidCsrf,
    ReplayMissing,
    ReplayTimestamp,
    ReplayWindow,
    ReplayNonce,
}

impl SessionStore {
    fn create_or_refresh_csrf(
        &mut self,
        session_id: Option<&str>,
        timeout: Duration,
    ) -> (String, String) {
        let now = Instant::now();
        self.remove_expired(now, timeout);

        if let Some(session_id) = session_id
            && let Some(session) = self.sessions.get_mut(session_id)
        {
            session.last_seen_at = now;
            session.csrf_token = generate_csrf_token();
            session.csrf_issued_at = now;
            return (session_id.to_string(), session.csrf_token.clone());
        }

        self.create_session(now)
    }

    fn authenticate(
        &mut self,
        session_id: &str,
        user_id: &str,
        timeout: Duration,
    ) -> Result<String, SessionError> {
        let now = Instant::now();
        self.remove_expired(now, timeout);
        let Some(session) = self.sessions.get_mut(session_id) else {
            return Err(SessionError::Missing);
        };

        session.authenticated = true;
        session.user_id = Some(user_id.to_string());
        session.csrf_token = generate_csrf_token();
        session.csrf_issued_at = now;
        session.last_seen_at = now;
        session.used_nonces.clear();
        Ok(session.csrf_token.clone())
    }

    fn authenticated_session(
        &mut self,
        session_id: Option<&str>,
        timeout: Duration,
    ) -> Result<AuthenticatedSession, SessionError> {
        let session_id = session_id.ok_or(SessionError::Missing)?;
        let now = Instant::now();
        self.remove_expired(now, timeout);
        let Some(session) = self.sessions.get_mut(session_id) else {
            return Err(SessionError::Missing);
        };
        if !session.authenticated {
            return Err(SessionError::Unauthenticated);
        }
        let Some(user_id) = session.user_id.clone() else {
            return Err(SessionError::Unauthenticated);
        };

        session.last_seen_at = now;
        Ok(AuthenticatedSession {
            session_id: session_id.to_string(),
            user_id,
        })
    }

    fn validate_csrf(
        &mut self,
        session_id: &str,
        provided_token: Option<&str>,
        session_timeout: Duration,
        csrf_ttl: Duration,
    ) -> Result<(), SessionError> {
        let provided_token = provided_token.ok_or(SessionError::InvalidCsrf)?;
        let now = Instant::now();
        self.remove_expired(now, session_timeout);
        let Some(session) = self.sessions.get_mut(session_id) else {
            return Err(SessionError::Missing);
        };

        if session.csrf_token != provided_token {
            return Err(SessionError::InvalidCsrf);
        }
        if now
            .checked_duration_since(session.csrf_issued_at)
            .is_some_and(|age| age > csrf_ttl)
        {
            return Err(SessionError::InvalidCsrf);
        }

        session.last_seen_at = now;
        Ok(())
    }

    fn validate_replay(
        &mut self,
        session_id: &str,
        nonce: Option<&str>,
        timestamp: Option<&str>,
        replay_window: Duration,
    ) -> Result<(), SessionError> {
        let nonce = nonce.ok_or(SessionError::ReplayMissing)?;
        if nonce.trim().is_empty() || nonce.chars().any(char::is_whitespace) || nonce.len() > 128 {
            return Err(SessionError::ReplayNonce);
        }
        let timestamp = timestamp
            .and_then(|value| value.parse::<i64>().ok())
            .ok_or(SessionError::ReplayTimestamp)?;
        let now_timestamp = OffsetDateTime::now_utc().unix_timestamp();
        if now_timestamp.abs_diff(timestamp) > replay_window.as_secs() {
            return Err(SessionError::ReplayWindow);
        }

        let now = Instant::now();
        let Some(session) = self.sessions.get_mut(session_id) else {
            return Err(SessionError::Missing);
        };
        session.used_nonces.retain(|_, used_at| {
            now.checked_duration_since(*used_at)
                .is_some_and(|age| age <= replay_window)
        });
        if session.used_nonces.contains_key(nonce) {
            return Err(SessionError::ReplayNonce);
        }

        session.used_nonces.insert(nonce.to_string(), now);
        session.last_seen_at = now;
        Ok(())
    }

    fn delete_session(&mut self, session_id: &str) {
        self.sessions.remove(session_id);
    }

    fn create_session(&mut self, now: Instant) -> (String, String) {
        let session_id = generate_session_id();
        let csrf_token = generate_csrf_token();
        self.sessions.insert(
            session_id.clone(),
            SessionData {
                authenticated: false,
                user_id: None,
                csrf_token: csrf_token.clone(),
                csrf_issued_at: now,
                last_seen_at: now,
                used_nonces: HashMap::new(),
            },
        );
        (session_id, csrf_token)
    }

    fn remove_expired(&mut self, now: Instant, timeout: Duration) {
        self.sessions.retain(|_, session| {
            now.checked_duration_since(session.last_seen_at)
                .is_some_and(|age| age <= timeout)
        });
    }
}

#[derive(Deserialize)]
struct LoginRequest {
    username: String,
    password: String,
    csrf_token: String,
}

#[derive(Serialize)]
struct LoginResponse {
    success: bool,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    csrf_token: Option<String>,
}

#[derive(Serialize)]
struct CsrfTokenResponse {
    csrf_token: String,
}

#[derive(Serialize)]
struct HealthCheckFailure {
    name: String,
    detail: String,
    remediation: Option<String>,
}

#[derive(Serialize)]
struct MessageResponse {
    success: bool,
    message: String,
}

fn generate_session_id() -> String {
    let mut bytes = [0u8; 32];
    rand_core::RngCore::fill_bytes(&mut OsRng, &mut bytes);
    hex::encode(bytes)
}

fn generate_csrf_token() -> String {
    let mut hasher = Sha256::new();
    let mut random_bytes = [0u8; 32];
    rand_core::RngCore::fill_bytes(&mut OsRng, &mut random_bytes);
    hasher.update(random_bytes);
    hasher.update(OffsetDateTime::now_utc().unix_timestamp().to_be_bytes());
    hex::encode(hasher.finalize())
}

pub(crate) async fn start_admin_web(
    config: Arc<OxideConfig>,
    db: Arc<OxideDb>,
    runtime: Arc<ServerRuntime>,
    caller_resources: Option<CallerResources>,
) -> ServeResult<tokio::task::JoinHandle<ServeResult<()>>> {
    let bind: SocketAddr = config
        .admin_web
        .bind
        .parse()
        .map_err(|error| ServeError::Config(format!("invalid admin_web.bind: {error}")))?;

    let listener = TcpListener::bind(bind).await?;
    let app = admin_router(AdminWebState::new(config, db, runtime, caller_resources));

    info!(bind = %listener.local_addr()?, "monitoring web listener started");
    Ok(tokio::spawn(async move {
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await
        .map_err(ServeError::Network)
    }))
}

impl AdminWebState {
    pub(crate) fn new(
        config: Arc<OxideConfig>,
        db: Arc<OxideDb>,
        runtime: Arc<ServerRuntime>,
        caller_resources: Option<CallerResources>,
    ) -> Self {
        Self {
            config,
            db,
            runtime,
            caller_resources,
            rate_limiter: Arc::new(RwLock::new(RateLimiter::default())),
            sessions: Arc::new(RwLock::new(SessionStore::default())),
        }
    }
}

fn admin_router(state: AdminWebState) -> Router {
    let activity_log_state = state.clone();
    let terminal_state = state.caller_resources.clone();
    let terminal_config = state.config.clone();
    let terminal_db = state.db.clone();
    let terminal_runtime = state.runtime.clone();
    let mut router = Router::new()
        .route("/", get(root_handler))
        .route("/health", get(health_handler))
        .route("/healthz", get(health_handler))
        .route("/healtz", get(health_handler))
        .route("/status", get(status_handler))
        .route("/csrf-token", get(csrf_token_handler))
        .route("/login", post(login_handler))
        .route("/logout", post(logout_handler))
        .route("/api/status", get(api_status_handler))
        .route("/api/nodes", get(api_nodes_handler))
        .route("/api/users", get(api_users_handler))
        .route("/api/doors", get(api_doors_handler))
        .route("/api/messages", get(api_messages_handler))
        .route("/api/database", get(api_database_handler))
        .route("/api/logs", get(api_logs_handler))
        .route("/api/audit", get(api_audit_handler))
        .route("/api/network", get(api_network_handler))
        .route("/api/oxidenet", get(api_oxidenet_handler))
        .route(
            "/api/nodes/{node_number}/disconnect",
            post(api_node_disconnect_handler),
        )
        .with_state(state)
        .layer(middleware::from_fn_with_state(
            activity_log_state,
            log_admin_request,
        ));

    if let Some(caller_resources) = terminal_state {
        router = router.merge(crate::web_terminal::web_terminal_router(
            crate::web_terminal::WebTerminalState {
                config: terminal_config,
                _db: terminal_db,
                runtime: terminal_runtime,
                caller_resources,
            },
        ));
    }

    router.fallback(not_found_handler)
}

async fn root_handler() -> Html<&'static str> {
    Html(ADMIN_ROOT_HTML)
}

async fn not_found_handler() -> StatusCode {
    StatusCode::NOT_FOUND
}

async fn log_admin_request(
    State(state): State<AdminWebState>,
    request: Request,
    next: Next,
) -> Response {
    let started_at = Instant::now();
    let method = request.method().clone();
    let path = request.uri().path().to_string();
    let remote_addr = request
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|ConnectInfo(addr)| addr.to_string())
        .unwrap_or_else(|| "unknown".to_string());
    let user_id = activity_user_id_from_headers(&state, request.headers()).await;
    let response = next.run(request).await;
    let status = response.status();
    let elapsed_ms = u64::try_from(started_at.elapsed().as_millis()).unwrap_or(u64::MAX);
    let user_id = user_id.as_deref().unwrap_or("unauthenticated");

    if status.is_server_error() {
        warn!(
            method = %method,
            path = %path,
            status = status.as_u16(),
            elapsed_ms,
            remote_addr = %remote_addr,
            user_id = %user_id,
            "monitoring web request completed"
        );
    } else {
        info!(
            method = %method,
            path = %path,
            status = status.as_u16(),
            elapsed_ms,
            remote_addr = %remote_addr,
            user_id = %user_id,
            "monitoring web request completed"
        );
    }

    response
}

async fn health_handler(
    State(state): State<AdminWebState>,
    headers: HeaderMap,
) -> Result<Response, StatusCode> {
    ensure_origin_allowed(&state, &headers)?;

    let report = DatabaseAdminService::run_doctor(
        Some(&state.db),
        Some(&state.config.database.path),
        state.config.nodes.count,
    );
    let failed = report.failed_count();
    let healthy = failed == 0;
    let status = if healthy {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    let failed_checks = report
        .checks
        .iter()
        .filter(|check| check.status == DoctorStatus::Fail)
        .map(|check| HealthCheckFailure {
            name: check.name.clone(),
            detail: check.detail.clone(),
            remediation: check.remediation.clone(),
        })
        .collect::<Vec<_>>();

    Ok(json_response(
        status,
        &json!({
            "healthy": healthy,
            "doctor": {
                "checked_at": report.checked_at,
                "passed": report.passed_count(),
                "warnings": report.warning_count(),
                "failed": failed,
                "total": report.checks.len(),
                "failed_checks": failed_checks,
            }
        }),
    ))
}

async fn status_handler(
    State(state): State<AdminWebState>,
    headers: HeaderMap,
) -> Result<Response, StatusCode> {
    if !state.config.admin_web.public_status_enabled {
        return Err(StatusCode::NOT_FOUND);
    }
    ensure_origin_allowed(&state, &headers)?;

    let payload = admin_status_payload(&state)?;
    Ok(json_response(StatusCode::OK, &payload))
}

async fn csrf_token_handler(
    State(state): State<AdminWebState>,
    headers: HeaderMap,
) -> Result<Response, StatusCode> {
    ensure_origin_allowed(&state, &headers)?;

    let existing_session_id = session_cookie(&headers);
    let (session_id, csrf_token) = {
        let mut sessions = state.sessions.write().await;
        sessions.create_or_refresh_csrf(
            existing_session_id.as_deref(),
            session_timeout(&state.config),
        )
    };

    let mut response = json_response(StatusCode::OK, &CsrfTokenResponse { csrf_token });
    secure_session_cookie(
        &mut response,
        &session_id,
        state.config.admin_web.session_timeout_seconds,
    )?;
    no_store(&mut response);
    Ok(response)
}

async fn login_handler(
    State(state): State<AdminWebState>,
    headers: HeaderMap,
    Form(login): Form<LoginRequest>,
) -> Result<Response, StatusCode> {
    ensure_origin_allowed(&state, &headers)?;
    let Some(session_id) = session_cookie(&headers) else {
        return Err(StatusCode::FORBIDDEN);
    };

    {
        let mut sessions = state.sessions.write().await;
        if let Err(error) = sessions.validate_csrf(
            &session_id,
            Some(login.csrf_token.as_str()),
            session_timeout(&state.config),
            csrf_ttl(&state.config),
        ) {
            audit_admin_event(
                &state,
                "admin_web_csrf_rejected",
                None,
                None,
                "monitoring web login rejected by CSRF validation",
            );
            return Err(session_error_status(error));
        }
    }

    if !check_rate_limit(
        &state,
        &format!("login:{}", login.username.trim().to_ascii_lowercase()),
        LOGIN_RATE_LIMIT_WINDOW,
    )
    .await
    {
        audit_admin_event(
            &state,
            "admin_web_login_rate_limited",
            None,
            None,
            format!(
                "monitoring web login rate limited for alias {}",
                login.username
            ),
        );
        return Ok(json_response(
            StatusCode::TOO_MANY_REQUESTS,
            &LoginResponse {
                success: false,
                message: "Too many login attempts. Please try again later.".to_string(),
                csrf_token: None,
            },
        ));
    }

    let login_user = verify_admin_credentials(&state, &login.username, &login.password)?;
    let Some(user) = login_user else {
        audit_admin_event(
            &state,
            "admin_web_login_failure",
            None,
            None,
            format!("monitoring web login failed for alias {}", login.username),
        );
        return Ok(json_response(
            StatusCode::UNAUTHORIZED,
            &LoginResponse {
                success: false,
                message: "Invalid username or password".to_string(),
                csrf_token: None,
            },
        ));
    };

    let next_csrf_token = {
        let mut sessions = state.sessions.write().await;
        sessions
            .authenticate(&session_id, &user.id, session_timeout(&state.config))
            .map_err(|_| StatusCode::FORBIDDEN)?
    };

    audit_admin_event(
        &state,
        "admin_web_login_success",
        Some(user.id.as_str()),
        None,
        format!("monitoring web login successful for {}", user.alias),
    );
    info!(alias = %user.alias, "monitoring web login accepted");

    let mut response = json_response(
        StatusCode::OK,
        &LoginResponse {
            success: true,
            message: "Login successful".to_string(),
            csrf_token: Some(next_csrf_token),
        },
    );
    secure_session_cookie(
        &mut response,
        &session_id,
        state.config.admin_web.session_timeout_seconds,
    )?;
    no_store(&mut response);
    Ok(response)
}

async fn logout_handler(
    State(state): State<AdminWebState>,
    headers: HeaderMap,
) -> Result<Response, StatusCode> {
    ensure_origin_allowed(&state, &headers)?;
    let authenticated = require_authenticated_session(&state, &headers).await?;
    if let Err(status) = require_csrf_header(&state, &headers, &authenticated.session_id).await {
        audit_admin_event(
            &state,
            "admin_web_csrf_rejected",
            Some(authenticated.user_id.as_str()),
            None,
            "monitoring web logout rejected by CSRF validation",
        );
        return Err(status);
    }

    {
        let mut sessions = state.sessions.write().await;
        sessions.delete_session(&authenticated.session_id);
    }

    audit_admin_event(
        &state,
        "admin_web_logout",
        Some(authenticated.user_id.as_str()),
        None,
        "monitoring web logout",
    );

    let mut response = json_response(
        StatusCode::OK,
        &MessageResponse {
            success: true,
            message: "Logout successful".to_string(),
        },
    );
    clear_session_cookie(&mut response)?;
    no_store(&mut response);
    Ok(response)
}

async fn api_status_handler(
    State(state): State<AdminWebState>,
    headers: HeaderMap,
) -> Result<Response, StatusCode> {
    ensure_authenticated_read(&state, &headers).await?;
    let payload = admin_status_payload(&state)?;
    Ok(json_response(StatusCode::OK, &payload))
}

async fn api_nodes_handler(
    State(state): State<AdminWebState>,
    headers: HeaderMap,
) -> Result<Response, StatusCode> {
    ensure_authenticated_read(&state, &headers).await?;
    Ok(json_response(
        StatusCode::OK,
        &json!({ "nodes": state.runtime.nodes_snapshot() }),
    ))
}

async fn api_users_handler(
    State(state): State<AdminWebState>,
    headers: HeaderMap,
) -> Result<Response, StatusCode> {
    ensure_authenticated_read(&state, &headers).await?;
    let users = list_users(state.db.db()).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let users = users
        .iter()
        .map(|user| {
            json!({
                "id": user.id,
                "alias": user.alias,
                "security_level": user.security_level,
                "is_sysop": user.is_sysop,
                "status": user.status,
                "last_login_at": user.last_login_at,
                "total_calls": user.total_calls,
            })
        })
        .collect::<Vec<_>>();
    Ok(json_response(StatusCode::OK, &json!({ "users": users })))
}

async fn api_doors_handler(
    State(state): State<AdminWebState>,
    headers: HeaderMap,
) -> Result<Response, StatusCode> {
    ensure_authenticated_read(&state, &headers).await?;
    let doors =
        list_door_definitions(state.db.db()).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let runs = list_door_runs(state.db.db(), 25).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let doors = doors
        .iter()
        .map(|door| {
            json!({
                "id": door.id,
                "key": door.key,
                "name": door.name,
                "runner": door.runner,
                "drop_file": door.drop_file,
                "exclusive": door.exclusive,
                "time_limit_minutes": door.time_limit_minutes,
                "enabled": door.enabled,
                "min_security_level": door.min_security_level,
            })
        })
        .collect::<Vec<_>>();
    let runs = runs
        .iter()
        .map(|run| {
            json!({
                "id": run.id,
                "door_id": run.door_id,
                "node_number": run.node_number,
                "started_at": run.started_at,
                "ended_at": run.ended_at,
                "exit_code": run.exit_code,
                "timed_out": run.timed_out,
                "disconnect_forced": run.disconnect_forced,
                "bytes_in": run.bytes_in,
                "bytes_out": run.bytes_out,
            })
        })
        .collect::<Vec<_>>();
    Ok(json_response(
        StatusCode::OK,
        &json!({ "doors": doors, "recent_runs": runs }),
    ))
}

async fn api_messages_handler(
    State(state): State<AdminWebState>,
    headers: HeaderMap,
) -> Result<Response, StatusCode> {
    ensure_authenticated_read(&state, &headers).await?;
    let areas = list_message_areas(state.db.db()).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let areas = areas
        .iter()
        .map(|area| {
            json!({
                "id": area.id,
                "key": area.key,
                "name": area.name,
                "kind": area.kind,
                "network_id": area.network_id,
                "read_security_level": area.read_security_level,
                "post_security_level": area.post_security_level,
                "moderated": area.moderated,
                "enabled": area.enabled,
            })
        })
        .collect::<Vec<_>>();
    Ok(json_response(
        StatusCode::OK,
        &json!({ "message_areas": areas }),
    ))
}

async fn api_database_handler(
    State(state): State<AdminWebState>,
    headers: HeaderMap,
) -> Result<Response, StatusCode> {
    ensure_authenticated_read(&state, &headers).await?;
    let schema_version = state
        .db
        .schema_version()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(json_response(
        StatusCode::OK,
        &json!({
            "database": {
                "healthy": true,
                "schema_version": schema_version,
                "path_redacted": true,
            }
        }),
    ))
}

async fn api_logs_handler(
    State(state): State<AdminWebState>,
    headers: HeaderMap,
) -> Result<Response, StatusCode> {
    ensure_authenticated_read(&state, &headers).await?;
    Ok(json_response(
        StatusCode::OK,
        &json!({ "logs": log_file_summaries(&state.config.paths.logs) }),
    ))
}

async fn api_audit_handler(
    State(state): State<AdminWebState>,
    headers: HeaderMap,
) -> Result<Response, StatusCode> {
    ensure_authenticated_read(&state, &headers).await?;
    let events =
        list_audit_events(state.db.db(), 100).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let events = events
        .iter()
        .map(|event| {
            json!({
                "id": event.id,
                "created_at": event.created_at,
                "event_type": event.event_type,
                "user_id": event.user_id,
                "node_number": event.node_number,
                "details": event.details,
            })
        })
        .collect::<Vec<_>>();
    Ok(json_response(StatusCode::OK, &json!({ "audit": events })))
}

async fn api_network_handler(
    State(state): State<AdminWebState>,
    headers: HeaderMap,
) -> Result<Response, StatusCode> {
    ensure_authenticated_read(&state, &headers).await?;
    let profiles =
        list_network_profiles(state.db.db()).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let links = list_network_links(state.db.db()).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let areas = list_network_areas(state.db.db()).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let packets = summarize_network_packets(state.db.db(), None)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let recent_polls =
        list_network_poll_logs(state.db.db()).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let recent_messages =
        list_network_messages(state.db.db()).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let raw_packets =
        list_network_packets(state.db.db()).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let profiles = profiles
        .iter()
        .map(|profile| {
            json!({
                "id": profile.id,
                "key": profile.key,
                "name": profile.name,
                "adapter": profile.adapter,
                "local_address": {
                    "zone": profile.local_zone,
                    "net": profile.local_net,
                    "node": profile.local_node,
                    "point": profile.local_point,
                },
                "enabled": profile.enabled,
            })
        })
        .collect::<Vec<_>>();
    let links = links
        .iter()
        .map(|link| {
            json!({
                "id": link.id,
                "key": link.key,
                "network_id": link.network_id,
                "address": link.address,
                "host": link.host,
                "binkp_port": link.binkp_port,
                "password_redacted": true,
                "poll_schedule_minutes": link.poll_schedule_minutes,
                "compression": link.compression,
                "transport_security": link.transport_security,
                "enabled": link.enabled,
            })
        })
        .collect::<Vec<_>>();
    let areas = areas
        .iter()
        .map(|area| {
            json!({
                "id": area.id,
                "network_id": area.network_id,
                "area_tag": area.area_tag,
                "local_area_id": area.local_area_id,
                "description": area.description,
                "read_only": area.read_only,
                "subscribed": area.subscribed,
            })
        })
        .collect::<Vec<_>>();
    let packet_summary = packets
        .iter()
        .map(|packet| {
            json!({
                "direction": packet.direction,
                "status": packet.status,
                "count": packet.count,
                "total_size_bytes": packet.total_size_bytes,
            })
        })
        .collect::<Vec<_>>();
    let recent_polls = recent_polls
        .iter()
        .take(25)
        .map(|poll| {
            json!({
                "id": poll.id,
                "link_id": poll.link_id,
                "started_at": poll.started_at,
                "ended_at": poll.ended_at,
                "direction": poll.direction,
                "status": poll.status,
                "bytes_in": poll.bytes_in,
                "bytes_out": poll.bytes_out,
                "packets_in": poll.packets_in,
                "packets_out": poll.packets_out,
                "error_message": poll.error_message,
            })
        })
        .collect::<Vec<_>>();

    Ok(json_response(
        StatusCode::OK,
        &json!({
            "profiles": profiles,
            "links": links,
            "areas": areas,
            "packet_summary": packet_summary,
            "recent_polls": recent_polls,
            "message_count": recent_messages.len(),
            "packet_count": raw_packets.len(),
        }),
    ))
}

async fn api_oxidenet_handler(
    State(state): State<AdminWebState>,
    headers: HeaderMap,
) -> Result<Response, StatusCode> {
    ensure_authenticated_read(&state, &headers).await?;
    let applications = list_oxidenet_applications(state.db.db(), 100)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let nodes =
        list_oxidenet_nodes(state.db.db(), 100).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let applications = applications
        .iter()
        .map(|application| {
            json!({
                "id": application.id,
                "created_at": application.created_at,
                "updated_at": application.updated_at,
                "submitted_at": application.submitted_at,
                "reviewed_at": application.reviewed_at,
                "status": application.status,
                "board_name": application.board_name,
                "sysop_alias": application.sysop_alias,
                "contact_email": application.contact_email,
                "host": application.host,
                "binkp_port": application.binkp_port,
                "telnet_host": application.telnet_host,
                "telnet_port": application.telnet_port,
                "software": application.software,
                "software_version": application.software_version,
                "timezone": application.timezone,
                "region": application.region,
                "assigned_address": application.assigned_address,
                "policy_version": application.policy_version,
                "policy_accepted_at": application.policy_accepted_at,
                "reviewed_by_user_id": application.reviewed_by_user_id,
            })
        })
        .collect::<Vec<_>>();
    let nodes = nodes
        .iter()
        .map(|node| {
            json!({
                "id": node.id,
                "created_at": node.created_at,
                "updated_at": node.updated_at,
                "activated_at": node.activated_at,
                "suspended_at": node.suspended_at,
                "retired_at": node.retired_at,
                "network_key": node.network_key,
                "address": node.address,
                "zone": node.zone,
                "net": node.net,
                "node": node.node,
                "point": node.point,
                "hub_address": node.hub_address,
                "board_name": node.board_name,
                "sysop_alias": node.sysop_alias,
                "contact_email": node.contact_email,
                "host": node.host,
                "binkp_port": node.binkp_port,
                "telnet_host": node.telnet_host,
                "telnet_port": node.telnet_port,
                "software": node.software,
                "software_version": node.software_version,
                "status": node.status,
                "last_poll_at": node.last_poll_at,
                "last_successful_poll_at": node.last_successful_poll_at,
                "flags": node.flags,
            })
        })
        .collect::<Vec<_>>();
    Ok(json_response(
        StatusCode::OK,
        &json!({ "applications": applications, "nodes": nodes }),
    ))
}

async fn api_node_disconnect_handler(
    State(state): State<AdminWebState>,
    AxumPath(node_number): AxumPath<u16>,
    headers: HeaderMap,
) -> Result<Response, StatusCode> {
    ensure_origin_allowed(&state, &headers)?;
    let authenticated = require_authenticated_session(&state, &headers).await?;
    if let Err(status) = require_csrf_header(&state, &headers, &authenticated.session_id).await {
        audit_admin_event(
            &state,
            "admin_web_csrf_rejected",
            Some(authenticated.user_id.as_str()),
            Some(i64::from(node_number)),
            "monitoring web mutation rejected by CSRF validation",
        );
        return Err(status);
    }
    if let Err(status) = require_replay_headers(&state, &headers, &authenticated.session_id).await {
        audit_admin_event(
            &state,
            "admin_web_replay_rejected",
            Some(authenticated.user_id.as_str()),
            Some(i64::from(node_number)),
            "monitoring web mutation rejected by replay validation",
        );
        return Err(status);
    }

    if !check_rate_limit(
        &state,
        &format!("mutation:{}", authenticated.user_id),
        MUTATION_RATE_LIMIT_WINDOW,
    )
    .await
    {
        audit_admin_event(
            &state,
            "admin_web_mutation_rate_limited",
            Some(authenticated.user_id.as_str()),
            Some(i64::from(node_number)),
            "monitoring web mutation rate limited",
        );
        return Ok(json_response(
            StatusCode::TOO_MANY_REQUESTS,
            &MessageResponse {
                success: false,
                message: "Too many mutation attempts. Please try again later.".to_string(),
            },
        ));
    }

    audit_admin_event(
        &state,
        "admin_web_read_only_mutation_blocked",
        Some(authenticated.user_id.as_str()),
        Some(i64::from(node_number)),
        "monitoring web node disconnect blocked by read-only mode",
    );

    let status = if state.config.admin_web.read_only {
        StatusCode::FORBIDDEN
    } else {
        StatusCode::NOT_IMPLEMENTED
    };
    Ok(json_response(
        status,
        &MessageResponse {
            success: false,
            message: "Remote admin mutations are not enabled".to_string(),
        },
    ))
}

async fn ensure_authenticated_read(
    state: &AdminWebState,
    headers: &HeaderMap,
) -> Result<AuthenticatedSession, StatusCode> {
    ensure_origin_allowed(state, headers)?;
    require_authenticated_session(state, headers).await
}

async fn require_authenticated_session(
    state: &AdminWebState,
    headers: &HeaderMap,
) -> Result<AuthenticatedSession, StatusCode> {
    let session_id = session_cookie(headers);
    let mut sessions = state.sessions.write().await;
    sessions
        .authenticated_session(session_id.as_deref(), session_timeout(&state.config))
        .map_err(session_error_status)
}

async fn require_csrf_header(
    state: &AdminWebState,
    headers: &HeaderMap,
    session_id: &str,
) -> Result<(), StatusCode> {
    let csrf = header_str(headers, CSRF_HEADER_NAME);
    let mut sessions = state.sessions.write().await;
    sessions
        .validate_csrf(
            session_id,
            csrf,
            session_timeout(&state.config),
            csrf_ttl(&state.config),
        )
        .map_err(session_error_status)
}

async fn require_replay_headers(
    state: &AdminWebState,
    headers: &HeaderMap,
    session_id: &str,
) -> Result<(), StatusCode> {
    let nonce = header_str(headers, REPLAY_NONCE_HEADER_NAME);
    let timestamp = header_str(headers, REPLAY_TIMESTAMP_HEADER_NAME);
    let mut sessions = state.sessions.write().await;
    sessions
        .validate_replay(session_id, nonce, timestamp, replay_window(&state.config))
        .map_err(session_error_status)
}

async fn check_rate_limit(state: &AdminWebState, key: &str, window: Duration) -> bool {
    let max_attempts = state.config.admin_web.rate_limit_per_minute as usize;
    let mut limiter = state.rate_limiter.write().await;
    limiter.check_rate_limit(key, max_attempts, window)
}

fn verify_admin_credentials(
    state: &AdminWebState,
    username: &str,
    password: &str,
) -> Result<Option<UserRecord>, StatusCode> {
    let user = find_user_by_alias_ci(state.db.db(), username)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let admin_user = user.filter(|user| user.is_sysop && user.status == "active");
    let password_hash = admin_user
        .as_ref()
        .map(|user| user.password_hash.as_str())
        .unwrap_or(DUMMY_PASSWORD_HASH);

    if verify_stored_password(password, password_hash, &state.config.auth.argon2)? {
        Ok(admin_user)
    } else {
        Ok(None)
    }
}

fn verify_stored_password(
    password: &str,
    password_hash: &str,
    config: &Argon2Config,
) -> Result<bool, StatusCode> {
    let Ok(parsed_hash) = PasswordHash::new(password_hash) else {
        let parsed_dummy = PasswordHash::new(DUMMY_PASSWORD_HASH)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        let _ = configured_argon2(config)?.verify_password(password.as_bytes(), &parsed_dummy);
        return Ok(false);
    };
    Ok(configured_argon2(config)?
        .verify_password(password.as_bytes(), &parsed_hash)
        .is_ok())
}

fn configured_argon2(config: &Argon2Config) -> Result<Argon2<'static>, StatusCode> {
    let params = Params::new(
        config.memory_cost_kib,
        config.iterations,
        config.parallelism,
        None,
    )
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Argon2::new(Algorithm::Argon2id, Version::V0x13, params))
}

fn ensure_origin_allowed(state: &AdminWebState, headers: &HeaderMap) -> Result<(), StatusCode> {
    let Some(origin) = header_str(headers, "origin") else {
        return Ok(());
    };
    if state
        .config
        .admin_web
        .allowed_origins
        .iter()
        .any(|allowed| allowed == origin)
        || same_request_origin(state, headers, origin)
    {
        return Ok(());
    }

    Err(StatusCode::FORBIDDEN)
}

fn same_request_origin(state: &AdminWebState, headers: &HeaderMap, origin: &str) -> bool {
    let Some(host) = header_str(headers, "host") else {
        return false;
    };
    let scheme = if state.config.admin_web.behind_reverse_proxy {
        header_str(headers, "x-forwarded-proto").unwrap_or("https")
    } else {
        "http"
    };
    origin.eq_ignore_ascii_case(&format!("{scheme}://{host}"))
}

fn session_cookie(headers: &HeaderMap) -> Option<String> {
    let cookie = headers.get(header::COOKIE)?.to_str().ok()?;
    for part in cookie.split(';') {
        let (name, value) = part.trim().split_once('=')?;
        if name == SESSION_COOKIE_NAME && !value.trim().is_empty() {
            return Some(value.trim().to_string());
        }
    }
    None
}

fn header_str<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers.get(name)?.to_str().ok()
}

async fn activity_user_id_from_headers(
    state: &AdminWebState,
    headers: &HeaderMap,
) -> Option<String> {
    let session_id = session_cookie(headers)?;
    let now = Instant::now();
    let timeout = session_timeout(&state.config);
    let sessions = state.sessions.read().await;
    let session = sessions.sessions.get(&session_id)?;
    activity_user_id_from_session(session, now, timeout)
}

fn activity_user_id_from_session(
    session: &SessionData,
    now: Instant,
    timeout: Duration,
) -> Option<String> {
    if !session.authenticated {
        return None;
    }
    if now
        .checked_duration_since(session.last_seen_at)
        .is_none_or(|age| age > timeout)
    {
        return None;
    }
    session.user_id.clone()
}

fn session_timeout(config: &OxideConfig) -> Duration {
    Duration::from_secs(config.admin_web.session_timeout_seconds)
}

fn csrf_ttl(config: &OxideConfig) -> Duration {
    Duration::from_secs(config.admin_web.csrf_token_ttl_seconds)
}

fn replay_window(config: &OxideConfig) -> Duration {
    Duration::from_secs(config.admin_web.replay_window_seconds)
}

fn session_error_status(error: SessionError) -> StatusCode {
    match error {
        SessionError::Missing | SessionError::Unauthenticated => StatusCode::UNAUTHORIZED,
        SessionError::InvalidCsrf => StatusCode::FORBIDDEN,
        SessionError::ReplayMissing
        | SessionError::ReplayTimestamp
        | SessionError::ReplayWindow => StatusCode::BAD_REQUEST,
        SessionError::ReplayNonce => StatusCode::CONFLICT,
    }
}

fn admin_status_payload(state: &AdminWebState) -> Result<AdminStatusPayload, StatusCode> {
    let runtime_status = state.runtime.status();
    build_admin_status_payload(&state.config, &state.db, Some(runtime_status))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

fn json_response<T: Serialize>(status: StatusCode, payload: &T) -> Response {
    (status, Json(payload)).into_response()
}

fn secure_session_cookie(
    response: &mut Response,
    session_id: &str,
    max_age_seconds: u64,
) -> Result<(), StatusCode> {
    append_set_cookie(
        response,
        &format!(
            "{SESSION_COOKIE_NAME}={session_id}; Path=/; Max-Age={max_age_seconds}; HttpOnly; Secure; SameSite=Strict"
        ),
    )
}

fn clear_session_cookie(response: &mut Response) -> Result<(), StatusCode> {
    append_set_cookie(
        response,
        &format!("{SESSION_COOKIE_NAME}=; Path=/; Max-Age=0; HttpOnly; Secure; SameSite=Strict"),
    )
}

fn append_set_cookie(response: &mut Response, cookie: &str) -> Result<(), StatusCode> {
    let value = HeaderValue::from_str(cookie).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    response.headers_mut().append(header::SET_COOKIE, value);
    Ok(())
}

fn no_store(response: &mut Response) {
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
}

fn audit_admin_event(
    state: &AdminWebState,
    event_type: &str,
    user_id: Option<&str>,
    node_number: Option<i64>,
    details: impl Into<String>,
) {
    if let Err(error) = insert_audit_event(
        state.db.db(),
        &AuditEventRecord {
            id: String::new(),
            created_at: String::new(),
            event_type: event_type.to_string(),
            user_id: user_id.map(str::to_string),
            node_number,
            details: details.into(),
        },
    ) {
        warn!(%error, %event_type, "failed to insert monitoring web audit event");
        state.runtime.record_audit_write_failure();
    }
}

fn log_file_summaries(logs_path: &Path) -> Vec<serde_json::Value> {
    let Ok(entries) = std::fs::read_dir(logs_path) else {
        return Vec::new();
    };
    let mut summaries = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let metadata = entry.metadata().ok()?;
            if !metadata.is_file() {
                return None;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            let modified_unix = metadata
                .modified()
                .ok()
                .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
                .map(|duration| duration.as_secs());
            Some(json!({
                "name": name,
                "size_bytes": metadata.len(),
                "modified_unix": modified_unix,
            }))
        })
        .collect::<Vec<_>>();
    summaries.sort_by(|left, right| {
        right["modified_unix"]
            .as_u64()
            .cmp(&left["modified_unix"].as_u64())
            .then_with(|| left["name"].as_str().cmp(&right["name"].as_str()))
    });
    summaries
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;
    use axum::http::Request;
    use oxidebbs_db::OxideDb;
    use oxidebbs_db::{UserRecord, insert_user, list_audit_events};
    use serde_json::Value as JsonValue;
    use tower::util::ServiceExt;

    fn test_state() -> AdminWebState {
        test_state_with_rate_limit(30)
    }

    fn test_password() -> String {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("time")
            .as_nanos()
            .to_string()
    }

    fn mismatched_password(password: &str) -> String {
        let mut value = password.to_string();
        value.push('x');
        value
    }

    fn test_state_with_rate_limit(rate_limit_per_minute: u32) -> AdminWebState {
        let config: OxideConfig = toml::from_str(&format!(
            r#"
[board]
name = "Admin Test"

[database]
path = ":memory:"

[admin_web]
enabled = true
public_status_enabled = true
rate_limit_per_minute = {rate_limit_per_minute}
allowed_origins = ["https://admin.example.test"]
"#
        ))
        .expect("parse config");
        let db = Arc::new(OxideDb::open_memory().expect("open memory db"));
        let runtime = Arc::new(ServerRuntime::new(
            config.board.name.clone(),
            config.nodes.count,
            config.telnet.max_connections,
            config.telnet.idle_timeout_seconds,
        ));
        AdminWebState::new(Arc::new(config), db, runtime, None)
    }

    fn test_state_with_terminal(enabled: bool) -> AdminWebState {
        let mut config: OxideConfig =
            toml::from_str(include_str!("../../../config/oxidebbs.example.toml"))
                .expect("parse example config");
        config.admin_web.enabled = true;
        config.admin_web.public_status_enabled = true;
        config.web_terminal.enabled = enabled;
        config.database.path = ":memory:".into();
        let db = Arc::new(OxideDb::open_memory().expect("open memory db"));
        let runtime = Arc::new(ServerRuntime::new(
            config.board.name.clone(),
            config.nodes.count,
            config.telnet.max_connections,
            config.telnet.idle_timeout_seconds,
        ));
        let caller_resources = if enabled {
            Some(test_caller_resources(
                &config,
                Arc::clone(&db),
                Arc::clone(&runtime),
            ))
        } else {
            None
        };
        AdminWebState::new(Arc::new(config), db, runtime, caller_resources)
    }

    fn test_caller_resources(
        config: &OxideConfig,
        db: Arc<OxideDb>,
        runtime: Arc<crate::control::ServerRuntime>,
    ) -> CallerResources {
        let mut menus = HashMap::new();
        for menu_id in config.menus.keys() {
            menus.insert(
                menu_id.clone(),
                Arc::new(
                    config
                        .core_menu(menu_id)
                        .expect("configured default menu from example config"),
                ),
            );
        }
        let login_menu = menus
            .get(&config.flow.login_menu)
            .expect("login menu")
            .clone();
        let main_menu = menus
            .get(&config.flow.main_menu)
            .expect("main menu")
            .clone();
        crate::serve::caller_resources(
            db,
            Arc::new(config.clone()),
            login_menu,
            main_menu,
            Arc::new(menus),
            runtime,
        )
    }

    fn seed_user(state: &AdminWebState, alias: &str, password: &str, is_sysop: bool) -> String {
        let id = if is_sysop {
            "00000000-0000-4000-8000-000000000a11"
        } else {
            "00000000-0000-4000-8000-000000000b0b"
        };
        let user = UserRecord {
            id: id.to_string(),
            alias: alias.to_string(),
            real_name: format!("{alias} User"),
            email: Some(format!("{alias}@example.test")),
            password_hash: crate::sysop_cli::hash_password(password).expect("hash password"),
            security_level: 255,
            is_sysop,
            created_at: "2026-06-04T00:00:00.000000Z".to_string(),
            last_login_at: None,
            total_calls: 0,
            time_bank_minutes: 0,
            status: "active".to_string(),
        };
        insert_user(state.db.db(), &user).expect("insert user");
        id.to_string()
    }

    async fn json_body(response: Response) -> JsonValue {
        let bytes = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("read body");
        serde_json::from_slice(&bytes).expect("parse json")
    }

    fn set_cookie_pair(response: &Response) -> String {
        response
            .headers()
            .get(header::SET_COOKIE)
            .expect("set-cookie")
            .to_str()
            .expect("set-cookie string")
            .split(';')
            .next()
            .expect("cookie pair")
            .to_string()
    }

    fn csrf_headers(cookie_pair: &str, csrf_token: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::COOKIE,
            HeaderValue::from_str(cookie_pair).expect("cookie header"),
        );
        headers.insert(
            CSRF_HEADER_NAME,
            HeaderValue::from_str(csrf_token).expect("csrf header"),
        );
        headers
    }

    fn assert_handler_error(result: Result<Response, StatusCode>, expected: StatusCode) {
        match result {
            Err(actual) => assert_eq!(actual, expected),
            Ok(response) => panic!(
                "expected handler error {expected}, got response status {}",
                response.status()
            ),
        }
    }

    async fn csrf_session(state: &AdminWebState) -> (String, String) {
        let response = csrf_token_handler(State(state.clone()), HeaderMap::new())
            .await
            .expect("csrf response");
        let cookie_pair = set_cookie_pair(&response);
        let body = json_body(response).await;
        let csrf_token = body["csrf_token"].as_str().expect("csrf token").to_string();
        (cookie_pair, csrf_token)
    }

    async fn login_session(state: &AdminWebState, alias: &str, password: &str) -> (String, String) {
        let (cookie_pair, csrf_token) = csrf_session(state).await;
        let mut headers = HeaderMap::new();
        headers.insert(
            header::COOKIE,
            HeaderValue::from_str(&cookie_pair).expect("cookie header"),
        );
        let response = login_handler(
            State(state.clone()),
            headers,
            Form(LoginRequest {
                username: alias.to_string(),
                password: password.to_string(),
                csrf_token,
            }),
        )
        .await
        .expect("login response");
        assert_eq!(response.status(), StatusCode::OK);
        let cookie_pair = set_cookie_pair(&response);
        let body = json_body(response).await;
        let csrf_token = body["csrf_token"]
            .as_str()
            .expect("login csrf token")
            .to_string();
        (cookie_pair, csrf_token)
    }

    #[tokio::test]
    async fn csrf_token_mints_secure_session_cookie() {
        let state = test_state();
        let response = csrf_token_handler(State(state), HeaderMap::new())
            .await
            .expect("csrf response");
        let cookie = response
            .headers()
            .get(header::SET_COOKIE)
            .expect("cookie")
            .to_str()
            .expect("cookie value");

        assert!(cookie.starts_with("oxidebbs_session="));
        assert!(cookie.contains("HttpOnly"));
        assert!(cookie.contains("Secure"));
        assert!(cookie.contains("SameSite=Strict"));
        assert_eq!(response.status(), StatusCode::OK);
        let body = json_body(response).await;
        assert!(body["csrf_token"].as_str().is_some());
    }

    #[tokio::test]
    async fn root_page_identifies_admin_routes() {
        let response = root_handler().await.into_response();
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("read root body");
        let body = String::from_utf8(bytes.to_vec()).expect("root html");
        assert!(body.contains("OxideBBS Monitoring"));
        assert!(body.contains("/health"));
        assert!(body.contains("/status"));
        assert!(body.contains("/csrf-token"));
        assert!(body.contains("POST /login"));
        assert!(body.contains("HTTP directly"));
    }

    #[tokio::test]
    async fn health_passes_when_doctor_has_no_failures() {
        let state = test_state();
        let password = test_password();
        seed_user(&state, "Sysop", &password, true);

        let response = health_handler(State(state), HeaderMap::new())
            .await
            .expect("health response");
        assert_eq!(response.status(), StatusCode::OK);
        let body = json_body(response).await;
        assert_eq!(body["healthy"], true);
        assert_eq!(body["doctor"]["failed"], 0);
    }

    #[tokio::test]
    async fn health_fails_when_doctor_reports_failures() {
        let state = test_state();

        let response = health_handler(State(state), HeaderMap::new())
            .await
            .expect("health response");
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        let body = json_body(response).await;
        assert_eq!(body["healthy"], false);
        assert!(
            body["doctor"]["failed"]
                .as_u64()
                .is_some_and(|count| count > 0)
        );
        assert!(
            body["doctor"]["failed_checks"]
                .as_array()
                .is_some_and(|checks| {
                    checks
                        .iter()
                        .any(|check| check["name"].as_str() == Some("Sysop accounts"))
                })
        );
    }

    #[tokio::test]
    async fn activity_user_id_tracks_authenticated_unexpired_sessions_only() {
        let state = test_state();
        let password = test_password();
        let sysop_id = seed_user(&state, "Sysop", &password, true);

        let (pre_auth_cookie, _) = csrf_session(&state).await;
        let mut pre_auth_headers = HeaderMap::new();
        pre_auth_headers.insert(
            header::COOKIE,
            HeaderValue::from_str(&pre_auth_cookie).expect("pre-auth cookie header"),
        );
        assert_eq!(
            activity_user_id_from_headers(&state, &pre_auth_headers).await,
            None
        );

        let (cookie_pair, _) = login_session(&state, "Sysop", &password).await;
        let mut headers = HeaderMap::new();
        headers.insert(
            header::COOKIE,
            HeaderValue::from_str(&cookie_pair).expect("cookie header"),
        );
        assert_eq!(
            activity_user_id_from_headers(&state, &headers).await,
            Some(sysop_id)
        );

        let session_id = cookie_pair
            .split_once('=')
            .expect("session cookie pair")
            .1
            .to_string();
        {
            let mut sessions = state.sessions.write().await;
            let session = sessions
                .sessions
                .get_mut(&session_id)
                .expect("stored session");
            session.last_seen_at =
                Instant::now() - session_timeout(&state.config) - Duration::from_secs(1);
        }

        assert_eq!(activity_user_id_from_headers(&state, &headers).await, None);
    }

    #[tokio::test]
    async fn login_requires_session_cookie_and_csrf() {
        let state = test_state();
        let password = test_password();
        seed_user(&state, "Sysop", &password, true);

        let no_cookie = login_handler(
            State(state.clone()),
            HeaderMap::new(),
            Form(LoginRequest {
                username: "Sysop".to_string(),
                password: password.clone(),
                csrf_token: "missing".to_string(),
            }),
        )
        .await;
        assert_handler_error(no_cookie, StatusCode::FORBIDDEN);

        let (cookie_pair, _) = csrf_session(&state).await;
        let mut headers = HeaderMap::new();
        headers.insert(
            header::COOKIE,
            HeaderValue::from_str(&cookie_pair).expect("cookie header"),
        );
        let bad_csrf = login_handler(
            State(state),
            headers,
            Form(LoginRequest {
                username: "Sysop".to_string(),
                password,
                csrf_token: "bad".to_string(),
            }),
        )
        .await;
        assert_handler_error(bad_csrf, StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn login_accepts_only_active_sysop_users_and_protects_api_nodes() {
        let state = test_state();
        let password = test_password();
        let sysop_id = seed_user(&state, "Sysop", &password, true);
        seed_user(&state, "Caller", &password, false);

        let (cookie_pair, csrf_token) = login_session(&state, "Sysop", &password).await;
        let mut headers = HeaderMap::new();
        headers.insert(
            header::COOKIE,
            HeaderValue::from_str(&cookie_pair).expect("cookie header"),
        );
        let response = api_nodes_handler(State(state.clone()), headers)
            .await
            .expect("nodes response");
        assert_eq!(response.status(), StatusCode::OK);

        let no_auth = api_nodes_handler(State(state.clone()), HeaderMap::new()).await;
        assert_handler_error(no_auth, StatusCode::UNAUTHORIZED);

        let (caller_cookie, caller_csrf) = csrf_session(&state).await;
        let mut caller_headers = HeaderMap::new();
        caller_headers.insert(
            header::COOKIE,
            HeaderValue::from_str(&caller_cookie).expect("cookie header"),
        );
        let caller_response = login_handler(
            State(state.clone()),
            caller_headers,
            Form(LoginRequest {
                username: "Caller".to_string(),
                password,
                csrf_token: caller_csrf,
            }),
        )
        .await
        .expect("caller login response");
        assert_eq!(caller_response.status(), StatusCode::UNAUTHORIZED);

        let audits = list_audit_events(state.db.db(), 10).expect("audit events");
        assert!(
            audits
                .iter()
                .any(|event| event.event_type == "admin_web_login_success"
                    && event.user_id.as_deref() == Some(sysop_id.as_str()))
        );
        assert!(!csrf_token.is_empty());
    }

    #[tokio::test]
    async fn logout_requires_auth_and_csrf_then_deletes_cookie_session() {
        let state = test_state();
        let password = test_password();
        seed_user(&state, "Sysop", &password, true);
        let (cookie_pair, csrf_token) = login_session(&state, "Sysop", &password).await;

        let response = logout_handler(
            State(state.clone()),
            csrf_headers(&cookie_pair, &csrf_token),
        )
        .await
        .expect("logout response");
        assert_eq!(response.status(), StatusCode::OK);
        let cookie = response
            .headers()
            .get(header::SET_COOKIE)
            .expect("clear cookie")
            .to_str()
            .expect("cookie value");
        assert!(cookie.contains("Max-Age=0"));

        let mut headers = HeaderMap::new();
        headers.insert(
            header::COOKIE,
            HeaderValue::from_str(&cookie_pair).expect("cookie header"),
        );
        let after_logout = api_nodes_handler(State(state), headers).await;
        assert_handler_error(after_logout, StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn expired_session_cannot_access_authenticated_api() {
        let state = test_state();
        let password = test_password();
        seed_user(&state, "Sysop", &password, true);
        let (cookie_pair, _) = login_session(&state, "Sysop", &password).await;
        let session_id = cookie_pair
            .split_once('=')
            .expect("session cookie pair")
            .1
            .to_string();

        {
            let mut sessions = state.sessions.write().await;
            let session = sessions
                .sessions
                .get_mut(&session_id)
                .expect("stored session");
            session.last_seen_at =
                Instant::now() - session_timeout(&state.config) - Duration::from_secs(1);
        }

        let mut headers = HeaderMap::new();
        headers.insert(
            header::COOKIE,
            HeaderValue::from_str(&cookie_pair).expect("cookie header"),
        );
        let expired = api_nodes_handler(State(state), headers).await;
        assert_handler_error(expired, StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn mutation_stub_requires_csrf_replay_and_blocks_read_only() {
        let state = test_state();
        let password = test_password();
        seed_user(&state, "Sysop", &password, true);
        let (cookie_pair, csrf_token) = login_session(&state, "Sysop", &password).await;

        let missing_replay = api_node_disconnect_handler(
            State(state.clone()),
            AxumPath(1),
            csrf_headers(&cookie_pair, &csrf_token),
        )
        .await;
        assert_handler_error(missing_replay, StatusCode::BAD_REQUEST);

        let mut stale_timestamp_headers = csrf_headers(&cookie_pair, &csrf_token);
        stale_timestamp_headers.insert(REPLAY_NONCE_HEADER_NAME, HeaderValue::from_static("stale"));
        let stale_timestamp = OffsetDateTime::now_utc().unix_timestamp()
            - (state.config.admin_web.replay_window_seconds as i64)
            - 1;
        stale_timestamp_headers.insert(
            REPLAY_TIMESTAMP_HEADER_NAME,
            HeaderValue::from_str(&stale_timestamp.to_string()).expect("timestamp header"),
        );
        let stale_timestamp_result =
            api_node_disconnect_handler(State(state.clone()), AxumPath(1), stale_timestamp_headers)
                .await;
        assert_handler_error(stale_timestamp_result, StatusCode::BAD_REQUEST);

        let mut headers = csrf_headers(&cookie_pair, &csrf_token);
        headers.insert(
            REPLAY_NONCE_HEADER_NAME,
            HeaderValue::from_static("nonce-1"),
        );
        headers.insert(
            REPLAY_TIMESTAMP_HEADER_NAME,
            HeaderValue::from_str(&OffsetDateTime::now_utc().unix_timestamp().to_string())
                .expect("timestamp header"),
        );
        let blocked =
            api_node_disconnect_handler(State(state.clone()), AxumPath(1), headers.clone())
                .await
                .expect("read-only block response");
        assert_eq!(blocked.status(), StatusCode::FORBIDDEN);

        let replayed =
            api_node_disconnect_handler(State(state.clone()), AxumPath(1), headers).await;
        assert_handler_error(replayed, StatusCode::CONFLICT);

        let audits = list_audit_events(state.db.db(), 10).expect("audit events");
        assert!(
            audits
                .iter()
                .any(|event| event.event_type == "admin_web_replay_rejected")
        );
        assert!(
            audits
                .iter()
                .any(|event| event.event_type == "admin_web_read_only_mutation_blocked")
        );
    }

    #[tokio::test]
    async fn mutation_stub_is_rate_limited_after_valid_security_checks() {
        let state = test_state_with_rate_limit(2);
        let password = test_password();
        seed_user(&state, "Sysop", &password, true);
        let (cookie_pair, csrf_token) = login_session(&state, "Sysop", &password).await;

        for (index, expected) in [
            StatusCode::FORBIDDEN,
            StatusCode::FORBIDDEN,
            StatusCode::TOO_MANY_REQUESTS,
        ]
        .into_iter()
        .enumerate()
        {
            let mut headers = csrf_headers(&cookie_pair, &csrf_token);
            headers.insert(
                REPLAY_NONCE_HEADER_NAME,
                HeaderValue::from_str(&format!("mutation-rate-{index}")).expect("nonce header"),
            );
            headers.insert(
                REPLAY_TIMESTAMP_HEADER_NAME,
                HeaderValue::from_str(&OffsetDateTime::now_utc().unix_timestamp().to_string())
                    .expect("timestamp header"),
            );
            let response = api_node_disconnect_handler(State(state.clone()), AxumPath(1), headers)
                .await
                .expect("mutation response");
            assert_eq!(response.status(), expected);
        }

        let audits = list_audit_events(state.db.db(), 10).expect("audit events");
        assert!(
            audits
                .iter()
                .any(|event| event.event_type == "admin_web_mutation_rate_limited")
        );
    }

    #[tokio::test]
    async fn login_is_rate_limited() {
        let state = test_state_with_rate_limit(2);
        let password = test_password();
        let wrong_password = mismatched_password(&password);
        seed_user(&state, "Sysop", &password, true);
        let (cookie_pair, csrf_token) = csrf_session(&state).await;

        for expected in [
            StatusCode::UNAUTHORIZED,
            StatusCode::UNAUTHORIZED,
            StatusCode::TOO_MANY_REQUESTS,
        ] {
            let mut headers = HeaderMap::new();
            headers.insert(
                header::COOKIE,
                HeaderValue::from_str(&cookie_pair).expect("cookie header"),
            );
            let response = login_handler(
                State(state.clone()),
                headers,
                Form(LoginRequest {
                    username: "Sysop".to_string(),
                    password: wrong_password.clone(),
                    csrf_token: csrf_token.clone(),
                }),
            )
            .await
            .expect("login response");
            assert_eq!(response.status(), expected);
        }
    }

    #[tokio::test]
    async fn origin_header_must_match_allowlist_or_same_origin() {
        let state = test_state();
        let mut rejected = HeaderMap::new();
        rejected.insert("origin", HeaderValue::from_static("https://evil.example"));
        let result = csrf_token_handler(State(state.clone()), rejected).await;
        assert_handler_error(result, StatusCode::FORBIDDEN);

        let mut allowed = HeaderMap::new();
        allowed.insert(
            "origin",
            HeaderValue::from_static("https://admin.example.test"),
        );
        let result = csrf_token_handler(State(state.clone()), allowed)
            .await
            .expect("allowlisted origin");
        assert_eq!(result.status(), StatusCode::OK);

        let mut same_origin = HeaderMap::new();
        same_origin.insert("origin", HeaderValue::from_static("http://127.0.0.1:8080"));
        same_origin.insert("host", HeaderValue::from_static("127.0.0.1:8080"));
        let result = csrf_token_handler(State(state), same_origin)
            .await
            .expect("same origin");
        assert_eq!(result.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn terminal_routes_available_when_web_terminal_resources_are_present() {
        let state = test_state_with_terminal(true);
        let app = admin_router(state);

        let terminal_request = Request::builder()
            .uri("/terminal")
            .body(axum::body::Body::empty())
            .expect("terminal request");
        let terminal_response = app
            .clone()
            .oneshot(terminal_request)
            .await
            .expect("terminal response");
        assert_eq!(terminal_response.status(), StatusCode::OK);
        let terminal_body = to_bytes(terminal_response.into_body(), usize::MAX)
            .await
            .expect("terminal body");
        let terminal_body = String::from_utf8(terminal_body.to_vec()).expect("terminal html");
        assert!(terminal_body.contains("id=\"terminal\""));

        let ws_request = Request::builder()
            .uri("/terminal/ws")
            .body(axum::body::Body::empty())
            .expect("terminal ws request");
        let ws_response = app
            .clone()
            .oneshot(ws_request)
            .await
            .expect("terminal ws response");
        assert_ne!(ws_response.status(), StatusCode::NOT_FOUND);

        let zmodem_request = Request::builder()
            .uri("/terminal/zmodem.js")
            .body(axum::body::Body::empty())
            .expect("terminal zmodem request");
        let zmodem_response = app
            .oneshot(zmodem_request)
            .await
            .expect("terminal zmodem response");
        assert_ne!(zmodem_response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn terminal_routes_disabled_without_web_terminal_resources() {
        let state = test_state_with_terminal(false);
        let app = admin_router(state);

        let terminal_request = Request::builder()
            .uri("/terminal")
            .body(axum::body::Body::empty())
            .expect("terminal request");
        let terminal_response = app
            .clone()
            .oneshot(terminal_request)
            .await
            .expect("terminal response");
        assert_eq!(terminal_response.status(), StatusCode::NOT_FOUND);

        let ws_request = Request::builder()
            .uri("/terminal/ws")
            .body(axum::body::Body::empty())
            .expect("terminal ws request");
        let ws_response = app
            .clone()
            .oneshot(ws_request)
            .await
            .expect("terminal ws response");
        assert_eq!(ws_response.status(), StatusCode::NOT_FOUND);

        let zmodem_request = Request::builder()
            .uri("/terminal/zmodem.js")
            .body(axum::body::Body::empty())
            .expect("terminal zmodem request");
        let zmodem_response = app
            .oneshot(zmodem_request)
            .await
            .expect("terminal zmodem response");
        assert_eq!(zmodem_response.status(), StatusCode::NOT_FOUND);
    }
}
