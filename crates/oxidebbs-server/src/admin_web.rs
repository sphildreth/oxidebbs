use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use argon2::{Argon2, PasswordHash, PasswordVerifier};
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::routing::{get, post};
use axum::{Form, Json, Router};
use rand_core::OsRng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use time::OffsetDateTime;
use tokio::net::TcpListener;
use tokio::sync::RwLock;
use tracing::{info, warn};

use oxidebbs_db::{OxideDb, find_user_by_alias};

use crate::admin_status::{AdminStatusPayload, build_admin_status_payload};
use crate::config::OxideConfig;
use crate::control::ServerRuntime;
use crate::serve::{ServeError, ServeResult};

#[allow(dead_code)]
const SESSION_COOKIE_NAME: &str = "oxidebbs_session";
#[allow(dead_code)]
const CSRF_TOKEN_KEY: &str = "csrf_token";
#[allow(dead_code)]
const AUTHENTICATED_KEY: &str = "authenticated";
#[allow(dead_code)]
const USER_ID_KEY: &str = "user_id";

#[derive(Clone)]
struct AdminWebState {
    config: Arc<OxideConfig>,
    db: Arc<OxideDb>,
    runtime: Arc<ServerRuntime>,
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

        // Remove old attempts outside the window
        attempts.retain(|&attempt| now.duration_since(attempt) < window);

        // Check if we've exceeded the limit
        if attempts.len() >= max_attempts {
            return false;
        }

        // Record this attempt
        attempts.push(now);
        true
    }
}

#[derive(Default)]
struct SessionStore {
    sessions: HashMap<String, SessionData>,
}

#[derive(Clone)]
#[allow(dead_code)]
struct SessionData {
    authenticated: bool,
    user_id: Option<String>,
    csrf_token: Option<String>,
    created_at: Instant,
}

impl SessionStore {
    fn create_session(&mut self) -> String {
        let session_id = generate_session_id();
        self.sessions.insert(
            session_id.clone(),
            SessionData {
                authenticated: false,
                user_id: None,
                csrf_token: None,
                created_at: Instant::now(),
            },
        );
        session_id
    }

    #[allow(dead_code)]
    fn get_session(&self, session_id: &str) -> Option<&SessionData> {
        self.sessions.get(session_id)
    }

    fn get_session_mut(&mut self, session_id: &str) -> Option<&mut SessionData> {
        self.sessions.get_mut(session_id)
    }

    #[allow(dead_code)]
    fn delete_session(&mut self, session_id: &str) {
        self.sessions.remove(session_id);
    }
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct LoginRequest {
    username: String,
    password: String,
    csrf_token: String,
}

#[derive(Serialize)]
struct LoginResponse {
    success: bool,
    message: String,
}

#[derive(Serialize)]
struct CsrfTokenResponse {
    csrf_token: String,
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
) -> ServeResult<tokio::task::JoinHandle<ServeResult<()>>> {
    let bind: SocketAddr = config
        .admin_web
        .bind
        .parse()
        .map_err(|error| ServeError::Config(format!("invalid admin_web.bind: {error}")))?;

    let rate_limiter = Arc::new(RwLock::new(RateLimiter::default()));
    let sessions = Arc::new(RwLock::new(SessionStore::default()));

    let listener = TcpListener::bind(bind).await?;
    let app = admin_router(AdminWebState {
        config,
        db,
        runtime,
        rate_limiter,
        sessions,
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
        .route("/login", post(login_handler))
        .route("/logout", post(logout_handler))
        .route("/csrf-token", get(csrf_token_handler))
        .route("/api/nodes", get(api_nodes_handler))
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

async fn login_handler(
    State(state): State<AdminWebState>,
    Form(login): Form<LoginRequest>,
) -> Result<Json<LoginResponse>, StatusCode> {
    // Rate limiting
    let rate_limit_per_minute = state.config.admin_web.rate_limit_per_minute as usize;
    let window = Duration::from_secs(60);

    {
        let mut limiter = state.rate_limiter.write().await;
        if !limiter.check_rate_limit(&login.username, rate_limit_per_minute, window) {
            warn!("Rate limit exceeded for user: {}", login.username);
            return Ok(Json(LoginResponse {
                success: false,
                message: "Too many login attempts. Please try again later.".to_string(),
            }));
        }
    }

    // Verify credentials
    let user = find_user_by_alias(state.db.db(), &login.username)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let user = match user {
        Some(u) => u,
        None => {
            return Ok(Json(LoginResponse {
                success: false,
                message: "Invalid username or password".to_string(),
            }));
        }
    };

    // Verify password
    let password_hash =
        PasswordHash::new(&user.password_hash).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if Argon2::default()
        .verify_password(login.password.as_bytes(), &password_hash)
        .is_err()
    {
        return Ok(Json(LoginResponse {
            success: false,
            message: "Invalid username or password".to_string(),
        }));
    }

    // Create session
    let session_id = {
        let mut sessions = state.sessions.write().await;
        sessions.create_session()
    };

    // Update session data
    {
        let mut sessions = state.sessions.write().await;
        if let Some(session) = sessions.get_session_mut(&session_id) {
            session.authenticated = true;
            session.user_id = Some(user.id.clone());
            session.csrf_token = Some(generate_csrf_token());
        }
    }

    info!("User {} logged in successfully", login.username);

    Ok(Json(LoginResponse {
        success: true,
        message: "Login successful".to_string(),
    }))
}

async fn logout_handler(
    State(_state): State<AdminWebState>,
) -> Result<Json<LoginResponse>, StatusCode> {
    // In a real implementation, we'd get the session ID from a cookie
    // For now, just return success
    Ok(Json(LoginResponse {
        success: true,
        message: "Logout successful".to_string(),
    }))
}

async fn csrf_token_handler(
    State(_state): State<AdminWebState>,
) -> Result<Json<CsrfTokenResponse>, StatusCode> {
    let csrf_token = generate_csrf_token();

    // In a real implementation, we'd store this in the session
    // For now, just return the token

    Ok(Json(CsrfTokenResponse { csrf_token }))
}

async fn api_nodes_handler(
    State(state): State<AdminWebState>,
    _headers: HeaderMap,
) -> Result<Json<serde_json::Value>, StatusCode> {
    // In a real implementation, we'd check authentication from session cookie
    // For now, just return the nodes data

    // Get nodes data
    let nodes = state.runtime.nodes_snapshot();
    let nodes_json = serde_json::to_value(nodes).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(nodes_json))
}
