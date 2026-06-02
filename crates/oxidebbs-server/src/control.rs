use std::collections::BTreeMap;
#[cfg(unix)]
use std::io::{BufRead, BufReader, Write as IoWrite};
use std::io::{Error as IoError, ErrorKind};
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
#[cfg(any(unix, test))]
use std::time::Duration;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use oxidebbs_core::node::NodeStatus;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::{Notify, OwnedSemaphorePermit, Semaphore};

pub const CONTROL_SOCKET_NAME: &str = "oxidebbs-control.sock";
pub const MAX_CONTROL_REQUEST_BYTES: usize = 64 * 1024;
#[cfg(unix)]
const CONTROL_CONNECT_TIMEOUT: Duration = Duration::from_millis(250);
#[cfg(unix)]
const SOCKET_READ_TIMEOUT: Duration = Duration::from_secs(2);
#[cfg(unix)]
const SOCKET_WRITE_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Debug, Error)]
pub enum ControlError {
    #[error("control socket is unavailable: {0}")]
    Unavailable(String),

    #[error("control socket is not supported on this platform yet: {0}")]
    #[cfg_attr(unix, allow(dead_code))]
    Unsupported(String),

    #[error("protocol error: {0}")]
    Protocol(String),

    #[error("I/O error: {0}")]
    Io(#[from] IoError),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

impl ControlError {
    pub fn is_unreachable(&self) -> bool {
        match self {
            Self::Unavailable(_) | Self::Unsupported(_) => true,
            Self::Io(error) => matches!(
                error.kind(),
                ErrorKind::NotFound
                    | ErrorKind::ConnectionRefused
                    | ErrorKind::ConnectionAborted
                    | ErrorKind::ConnectionReset
                    | ErrorKind::TimedOut
            ),
            Self::Protocol(_) | Self::Json(_) => false,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type")]
pub enum ControlRequest {
    #[serde(rename = "status")]
    Status,

    #[serde(rename = "nodes.list")]
    NodesList,

    #[serde(rename = "nodes.disconnect")]
    NodeDisconnect { node_number: u16, reason: String },

    #[serde(rename = "nodes.message")]
    NodeMessage { node_number: u16, text: String },

    #[serde(rename = "nodes.broadcast")]
    NodeBroadcast { text: String },

    #[serde(rename = "nodes.reset_stale")]
    NodesResetStale,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type")]
pub enum ControlResponse {
    #[serde(rename = "ok")]
    Ok { ok: bool },

    #[serde(rename = "status")]
    Status { ok: bool, status: ControlStatus },

    #[serde(rename = "nodes")]
    Nodes {
        ok: bool,
        nodes: Vec<ControlNodeStatus>,
    },

    #[serde(rename = "error")]
    Error { ok: bool, error: String },
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ControlStatus {
    pub board_name: String,
    pub uptime_seconds: u64,
    pub node_count: u16,
    pub active_nodes: usize,
    pub audit_write_failures: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ControlNodeStatus {
    pub node_number: u16,
    pub state: String,
    pub user_alias: Option<String>,
    pub remote_address: Option<String>,
    pub connected_at: Option<String>,
    pub last_heartbeat_at: Option<String>,
    pub heartbeat_age_seconds: Option<u64>,
}

#[derive(Debug, Default)]
pub struct RuntimeNodeCommands {
    pub disconnect_reason: Option<String>,
    pub messages: Vec<String>,
}

impl RuntimeNodeCommands {
    pub fn is_empty(&self) -> bool {
        self.disconnect_reason.is_none() && self.messages.is_empty()
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeNodeState {
    Available,
    Connecting,
    Login,
    MainMenu,
    ReadingMessages,
    PostingMessage,
    InDoor,
    Disconnecting,
    Offline,
    Stale,
}

impl RuntimeNodeState {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Available => "available",
            Self::Connecting => "connecting",
            Self::Login => "login",
            Self::MainMenu => "main_menu",
            Self::ReadingMessages => "reading_messages",
            Self::PostingMessage => "posting_message",
            Self::InDoor => "in_door",
            Self::Disconnecting => "disconnecting",
            Self::Offline => "offline",
            Self::Stale => "stale",
        }
    }

    #[allow(dead_code)]
    pub const fn to_core_status(self) -> NodeStatus {
        match self {
            Self::Available => NodeStatus::Idle,
            Self::Connecting => NodeStatus::Connected,
            Self::Login => NodeStatus::LoggingIn,
            Self::MainMenu | Self::ReadingMessages | Self::PostingMessage => NodeStatus::InMenu,
            Self::InDoor => NodeStatus::InDoor,
            Self::Disconnecting | Self::Offline | Self::Stale => NodeStatus::Disconnected,
        }
    }
}

#[derive(Debug, Clone)]
struct RuntimeNode {
    session_id: Option<String>,
    user_id: Option<String>,
    user_alias: Option<String>,
    state: RuntimeNodeState,
    remote_address: Option<String>,
    connected_at: Option<String>,
    last_heartbeat_at: Instant,
    last_heartbeat_text: String,
}

#[derive(Debug)]
pub struct ServerRuntime {
    board_name: String,
    node_count: u16,
    stale_after_seconds: u64,
    started_at: SystemTime,
    nodes: Mutex<BTreeMap<u16, RuntimeNode>>,
    allocation: Arc<Semaphore>,
    disconnect_requests: Mutex<BTreeMap<u16, String>>,
    node_messages: Mutex<BTreeMap<u16, Vec<String>>>,
    audit_write_failures: AtomicU64,
    command_notify: Notify,
}

impl ServerRuntime {
    pub fn new(
        board_name: String,
        node_count: u16,
        max_connections: u32,
        stale_after_seconds: u64,
    ) -> Self {
        let max_connections = usize::try_from(max_connections).unwrap_or(usize::MAX);
        let max_slots = usize::from(node_count).min(max_connections);
        Self {
            board_name,
            node_count,
            stale_after_seconds,
            started_at: SystemTime::now(),
            nodes: Mutex::new(BTreeMap::new()),
            allocation: Arc::new(Semaphore::new(max_slots)),
            disconnect_requests: Mutex::new(BTreeMap::new()),
            node_messages: Mutex::new(BTreeMap::new()),
            audit_write_failures: AtomicU64::new(0),
            command_notify: Notify::new(),
        }
    }

    pub fn try_allocate_node(self: &Arc<Self>) -> Option<NodeAllocation> {
        let permit = self.allocation.clone().try_acquire_owned().ok()?;
        let now = timestamp_string();
        let now_instant = Instant::now();
        let node_number = {
            let mut nodes = self.nodes.lock().ok()?;
            let node_number =
                (1..=self.node_count).find(|node_number| !nodes.contains_key(node_number));
            if let Some(node_number) = node_number {
                nodes.insert(
                    node_number,
                    RuntimeNode {
                        session_id: None,
                        user_id: None,
                        user_alias: None,
                        state: RuntimeNodeState::Connecting,
                        remote_address: None,
                        connected_at: Some(now.clone()),
                        last_heartbeat_at: now_instant,
                        last_heartbeat_text: now.clone(),
                    },
                );
            }
            node_number
        };
        if let Some(node_number) = node_number {
            Some(NodeAllocation {
                runtime: Arc::clone(self),
                node_number,
                _permit: permit,
            })
        } else {
            drop(permit);
            None
        }
    }

    pub fn mark_node_connected(
        &self,
        node_number: u16,
        session_id: String,
        remote_address: String,
        connected_at: String,
    ) {
        if let Ok(mut nodes) = self.nodes.lock() {
            match nodes.get_mut(&node_number) {
                Some(node) => {
                    node.session_id = Some(session_id);
                    node.user_id = None;
                    node.user_alias = None;
                    node.state = RuntimeNodeState::Connecting;
                    node.remote_address = Some(remote_address);
                    node.connected_at = Some(connected_at);
                    node.last_heartbeat_at = Instant::now();
                    node.last_heartbeat_text = timestamp_string();
                }
                None => {
                    nodes.insert(
                        node_number,
                        RuntimeNode {
                            session_id: Some(session_id),
                            user_id: None,
                            user_alias: None,
                            state: RuntimeNodeState::Connecting,
                            remote_address: Some(remote_address),
                            connected_at: Some(connected_at),
                            last_heartbeat_at: Instant::now(),
                            last_heartbeat_text: timestamp_string(),
                        },
                    );
                }
            }
        }
        if let Ok(mut disconnects) = self.disconnect_requests.lock() {
            disconnects.remove(&node_number);
        }
        if let Ok(mut messages) = self.node_messages.lock() {
            messages.remove(&node_number);
        }
    }

    pub fn mark_node_disconnected(&self, node_number: u16) -> Option<String> {
        self.nodes.lock().ok().and_then(|mut nodes| {
            let session_id = nodes.remove(&node_number).and_then(|node| node.session_id);
            if session_id.is_some() {
                if let Ok(mut disconnects) = self.disconnect_requests.lock() {
                    disconnects.remove(&node_number);
                }
                if let Ok(mut messages) = self.node_messages.lock() {
                    messages.remove(&node_number);
                }
            }
            session_id
        })
    }

    pub fn mark_node_login(&self, node_number: u16) {
        self.set_node_state(node_number, RuntimeNodeState::Login);
    }

    pub fn mark_node_main_menu(&self, node_number: u16) {
        self.set_node_state(node_number, RuntimeNodeState::MainMenu);
    }

    pub fn mark_node_reading_messages(&self, node_number: u16) {
        self.set_node_state(node_number, RuntimeNodeState::ReadingMessages);
    }

    pub fn mark_node_posting_message(&self, node_number: u16) {
        self.set_node_state(node_number, RuntimeNodeState::PostingMessage);
    }

    pub fn mark_node_in_door(&self, node_number: u16) {
        self.set_node_state(node_number, RuntimeNodeState::InDoor);
    }

    pub fn mark_node_disconnecting(&self, node_number: u16) {
        self.set_node_state(node_number, RuntimeNodeState::Disconnecting);
    }

    pub fn status(&self) -> ControlStatus {
        let active_nodes = self
            .nodes
            .lock()
            .map(|nodes| nodes.len())
            .unwrap_or_default();
        let uptime_seconds = self
            .started_at
            .elapsed()
            .map(|duration| duration.as_secs())
            .unwrap_or(0);

        ControlStatus {
            board_name: self.board_name.clone(),
            uptime_seconds,
            node_count: self.node_count,
            active_nodes,
            audit_write_failures: self.audit_write_failures(),
        }
    }

    pub fn record_audit_write_failure(&self) {
        self.audit_write_failures.fetch_add(1, Ordering::Relaxed);
    }

    pub fn audit_write_failures(&self) -> u64 {
        self.audit_write_failures.load(Ordering::Relaxed)
    }

    pub fn nodes_snapshot(&self) -> Vec<ControlNodeStatus> {
        let now = Instant::now();
        let stale_after_seconds = self.stale_after_seconds;
        let nodes = self
            .nodes
            .lock()
            .map(|nodes| nodes.clone())
            .unwrap_or_default();

        (1..=self.node_count)
            .map(|node_number| {
                if let Some(node) = nodes.get(&node_number) {
                    node_status(node_number, node, now, stale_after_seconds)
                } else {
                    available_node_status(node_number)
                }
            })
            .collect()
    }

    #[cfg(test)]
    pub fn node_status(&self, node_number: u16) -> Option<ControlNodeStatus> {
        if node_number == 0 || node_number > self.node_count {
            return None;
        }

        let now = Instant::now();
        let stale_after_seconds = self.stale_after_seconds;
        match self.nodes.lock() {
            Ok(nodes) => Some(match nodes.get(&node_number) {
                Some(node) => node_status(node_number, node, now, stale_after_seconds),
                None => available_node_status(node_number),
            }),
            Err(_) => None,
        }
    }

    pub fn request_node_disconnect(&self, node_number: u16, reason: String) -> bool {
        if !self.has_runtime_node(node_number) {
            return false;
        }
        self.mark_node_disconnecting(node_number);
        if let Ok(mut disconnects) = self.disconnect_requests.lock() {
            disconnects.insert(node_number, reason);
            self.command_notify.notify_waiters();
            return true;
        }
        false
    }

    pub fn queue_node_message(&self, node_number: u16, text: String) -> bool {
        if !self.has_runtime_node(node_number) {
            return false;
        }
        if let Ok(mut messages) = self.node_messages.lock() {
            messages.entry(node_number).or_default().push(text);
            self.command_notify.notify_waiters();
            return true;
        }
        false
    }

    pub fn queue_broadcast(&self, text: String) -> usize {
        let active_nodes = self
            .nodes
            .lock()
            .map(|nodes| nodes.keys().copied().collect::<Vec<_>>())
            .unwrap_or_default();
        if active_nodes.is_empty() {
            return 0;
        }

        if let Ok(mut messages) = self.node_messages.lock() {
            for node_number in &active_nodes {
                messages.entry(*node_number).or_default().push(text.clone());
            }
            self.command_notify.notify_waiters();
            return active_nodes.len();
        }
        0
    }

    pub fn request_stale_disconnects(&self, reason: &str) -> Vec<u16> {
        let now = Instant::now();
        let stale_after_seconds = self.stale_after_seconds;
        let stale_nodes = self
            .nodes
            .lock()
            .map(|mut nodes| {
                nodes
                    .iter_mut()
                    .filter_map(|(node_number, node)| {
                        if node_state_is_stale(node, now, stale_after_seconds) {
                            node.state = RuntimeNodeState::Disconnecting;
                            Some(*node_number)
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        if !stale_nodes.is_empty() {
            if let Ok(mut disconnects) = self.disconnect_requests.lock() {
                for node_number in &stale_nodes {
                    disconnects.insert(*node_number, reason.to_string());
                }
            }
            self.command_notify.notify_waiters();
        }

        stale_nodes
    }

    pub fn take_node_commands(&self, node_number: u16) -> RuntimeNodeCommands {
        let disconnect_reason = self
            .disconnect_requests
            .lock()
            .ok()
            .and_then(|mut disconnects| disconnects.remove(&node_number));
        let messages = self
            .node_messages
            .lock()
            .ok()
            .and_then(|mut messages| messages.remove(&node_number))
            .unwrap_or_default();

        RuntimeNodeCommands {
            disconnect_reason,
            messages,
        }
    }

    pub async fn wait_for_node_commands(&self, node_number: u16) -> RuntimeNodeCommands {
        loop {
            let notified = self.command_notify.notified();
            let commands = self.take_node_commands(node_number);
            if !commands.is_empty() {
                return commands;
            }
            notified.await;
        }
    }

    pub fn set_node_user(
        &self,
        node_number: u16,
        user_id: Option<String>,
        user_alias: Option<String>,
    ) {
        if let Ok(mut nodes) = self.nodes.lock()
            && let Some(node) = nodes.get_mut(&node_number)
        {
            node.user_id = user_id;
            node.user_alias = user_alias;
            node.last_heartbeat_at = Instant::now();
            node.last_heartbeat_text = timestamp_string();
        }
    }

    pub fn heartbeat_node(&self, node_number: u16) {
        if let Ok(mut nodes) = self.nodes.lock()
            && let Some(node) = nodes.get_mut(&node_number)
        {
            node.last_heartbeat_at = Instant::now();
            node.last_heartbeat_text = timestamp_string();
        }
    }

    fn set_node_state(&self, node_number: u16, state: RuntimeNodeState) {
        if let Ok(mut nodes) = self.nodes.lock()
            && let Some(node) = nodes.get_mut(&node_number)
        {
            node.state = state;
            node.last_heartbeat_at = Instant::now();
            node.last_heartbeat_text = timestamp_string();
        }
    }

    fn has_runtime_node(&self, node_number: u16) -> bool {
        self.nodes
            .lock()
            .map(|nodes| nodes.contains_key(&node_number))
            .unwrap_or(false)
    }

    #[cfg(test)]
    pub(crate) fn force_node_heartbeat_age(&self, node_number: u16, age: Duration) {
        if let Ok(mut nodes) = self.nodes.lock()
            && let Some(node) = nodes.get_mut(&node_number)
        {
            node.last_heartbeat_at = Instant::now() - age;
            node.last_heartbeat_text = format!("forced-{}s-ago", age.as_secs());
        }
    }

    fn release_node(&self, node_number: u16) {
        if let Ok(mut nodes) = self.nodes.lock() {
            nodes.remove(&node_number);
        }
        if let Ok(mut disconnects) = self.disconnect_requests.lock() {
            disconnects.remove(&node_number);
        }
        if let Ok(mut messages) = self.node_messages.lock() {
            messages.remove(&node_number);
        }
    }
}

pub struct NodeAllocation {
    pub node_number: u16,
    runtime: Arc<ServerRuntime>,
    _permit: OwnedSemaphorePermit,
}

impl Drop for NodeAllocation {
    fn drop(&mut self) {
        self.runtime.release_node(self.node_number);
    }
}

fn node_status(
    node_number: u16,
    node: &RuntimeNode,
    now: Instant,
    stale_after_seconds: u64,
) -> ControlNodeStatus {
    let heartbeat_age_seconds = now.duration_since(node.last_heartbeat_at).as_secs();
    let state = if node_state_is_stale(node, now, stale_after_seconds) {
        RuntimeNodeState::Stale
    } else {
        node.state
    };

    ControlNodeStatus {
        node_number,
        state: state.as_str().to_string(),
        user_alias: node.user_alias.clone(),
        remote_address: node.remote_address.clone(),
        connected_at: node.connected_at.clone(),
        last_heartbeat_at: Some(node.last_heartbeat_text.clone()),
        heartbeat_age_seconds: Some(heartbeat_age_seconds),
    }
}

fn available_node_status(node_number: u16) -> ControlNodeStatus {
    ControlNodeStatus {
        node_number,
        state: RuntimeNodeState::Available.as_str().to_string(),
        user_alias: None,
        remote_address: None,
        connected_at: None,
        last_heartbeat_at: None,
        heartbeat_age_seconds: None,
    }
}

fn node_state_is_stale(node: &RuntimeNode, now: Instant, stale_after_seconds: u64) -> bool {
    if matches!(
        node.state,
        RuntimeNodeState::Disconnecting | RuntimeNodeState::Offline
    ) {
        return false;
    }
    now.duration_since(node.last_heartbeat_at).as_secs() > stale_after_seconds
}

pub fn control_socket_path(runtime_dir: &Path) -> PathBuf {
    runtime_dir.join(CONTROL_SOCKET_NAME)
}

pub fn normalize_control_text(value: &str) -> String {
    let mut normalized = String::with_capacity(value.len());
    let mut in_line_break = false;

    for ch in value.chars() {
        if ch == '\r' || ch == '\n' {
            if !in_line_break {
                normalized.push(' ');
                in_line_break = true;
            }
        } else {
            normalized.push(ch);
            in_line_break = false;
        }
    }

    normalized
}

pub fn request_status(runtime_dir: &Path) -> Result<ControlResponse, ControlError> {
    send_control_request(runtime_dir, &ControlRequest::Status)
}

pub fn request_nodes(runtime_dir: &Path) -> Result<ControlResponse, ControlError> {
    send_control_request(runtime_dir, &ControlRequest::NodesList)
}

pub fn request_nodes_message(
    runtime_dir: &Path,
    node_number: u16,
    text: String,
) -> Result<ControlResponse, ControlError> {
    send_control_request(
        runtime_dir,
        &ControlRequest::NodeMessage {
            node_number,
            text: normalize_control_text(&text),
        },
    )
}

pub fn request_nodes_broadcast(
    runtime_dir: &Path,
    text: String,
) -> Result<ControlResponse, ControlError> {
    send_control_request(
        runtime_dir,
        &ControlRequest::NodeBroadcast {
            text: normalize_control_text(&text),
        },
    )
}

pub fn request_nodes_disconnect(
    runtime_dir: &Path,
    node_number: u16,
    reason: String,
) -> Result<ControlResponse, ControlError> {
    send_control_request(
        runtime_dir,
        &ControlRequest::NodeDisconnect {
            node_number,
            reason: normalize_control_text(&reason),
        },
    )
}

pub fn request_nodes_reset_stale(runtime_dir: &Path) -> Result<ControlResponse, ControlError> {
    send_control_request(runtime_dir, &ControlRequest::NodesResetStale)
}

#[cfg(unix)]
pub async fn start_control_listener(
    runtime_dir: &Path,
    runtime: Arc<ServerRuntime>,
) -> Result<tokio::task::JoinHandle<()>, ControlError> {
    let listener = bind_control_listener(runtime_dir).await?;
    Ok(tokio::spawn(async move {
        if let Err(error) =
            run_control_accept_loop(listener.socket, listener.server_uid, runtime).await
        {
            tracing::warn!(%error, "control listener stopped");
        }
    }))
}

#[cfg(not(unix))]
pub async fn start_control_listener(
    _runtime_dir: &Path,
    _runtime: Arc<ServerRuntime>,
) -> Result<tokio::task::JoinHandle<()>, ControlError> {
    Err(ControlError::Unsupported(
        "control socket is not supported on this platform yet".to_string(),
    ))
}

#[cfg(unix)]
struct BoundControlListener {
    socket: tokio::net::UnixListener,
    server_uid: u32,
}

#[cfg(unix)]
async fn bind_control_listener(runtime_dir: &Path) -> Result<BoundControlListener, ControlError> {
    let socket_path = control_socket_path(runtime_dir);
    tokio::fs::create_dir_all(runtime_dir)
        .await
        .map_err(IoError::other)?;
    tokio::fs::set_permissions(runtime_dir, std::fs::Permissions::from_mode(0o700))
        .await
        .map_err(IoError::other)?;

    if socket_path.exists() && is_socket_in_use(&socket_path).await {
        return Err(ControlError::Unavailable(format!(
            "control socket {} already active",
            socket_path.display()
        )));
    }

    if socket_path.exists() {
        tokio::fs::remove_file(&socket_path)
            .await
            .map_err(IoError::other)?;
    }

    tracing::info!(path = %socket_path.display(), "starting control listener");
    let listener = tokio::net::UnixListener::bind(&socket_path).map_err(ControlError::Io)?;
    tokio::fs::set_permissions(&socket_path, std::fs::Permissions::from_mode(0o600))
        .await
        .map_err(IoError::other)?;
    Ok(BoundControlListener {
        socket: listener,
        server_uid: control_process_uid(),
    })
}

#[cfg(unix)]
async fn run_control_accept_loop(
    listener: tokio::net::UnixListener,
    server_uid: u32,
    runtime: Arc<ServerRuntime>,
) -> Result<(), ControlError> {
    loop {
        let (stream, _) = listener.accept().await.map_err(ControlError::Io)?;
        let runtime = Arc::clone(&runtime);
        tokio::spawn(async move {
            if let Err(error) = handle_control_connection(stream, server_uid, runtime).await {
                tracing::warn!(%error, "control connection failed");
            }
        });
    }
}

#[cfg(unix)]
fn send_control_request(
    runtime_dir: &Path,
    request: &ControlRequest,
) -> Result<ControlResponse, ControlError> {
    use std::net::Shutdown;
    use std::os::unix::net::UnixStream;

    let socket_path = control_socket_path(runtime_dir);
    let mut socket = UnixStream::connect(&socket_path).map_err(map_io_unavailable)?;
    socket
        .set_read_timeout(Some(SOCKET_READ_TIMEOUT))
        .map_err(ControlError::Io)?;
    socket
        .set_write_timeout(Some(SOCKET_WRITE_TIMEOUT))
        .map_err(ControlError::Io)?;

    let request_line = serde_json::to_string(request).map_err(ControlError::Json)? + "\n";
    socket
        .write_all(request_line.as_bytes())
        .map_err(ControlError::Io)?;
    socket.shutdown(Shutdown::Write).map_err(ControlError::Io)?;

    let mut response_line = String::new();
    let mut reader = BufReader::new(&mut socket);
    reader
        .read_line(&mut response_line)
        .map_err(ControlError::Io)?;
    let response_line = response_line.trim_end();
    if response_line.is_empty() {
        return Err(ControlError::Protocol("empty control response".to_string()));
    }

    serde_json::from_str::<ControlResponse>(response_line).map_err(ControlError::Json)
}

#[cfg(not(unix))]
fn send_control_request(
    _runtime_dir: &Path,
    _request: &ControlRequest,
) -> Result<ControlResponse, ControlError> {
    Err(ControlError::Unsupported(
        "control socket is not supported on this platform yet".to_string(),
    ))
}

#[cfg(unix)]
fn map_io_unavailable(error: IoError) -> ControlError {
    match error.kind() {
        ErrorKind::NotFound
        | ErrorKind::ConnectionRefused
        | ErrorKind::ConnectionAborted
        | ErrorKind::ConnectionReset
        | ErrorKind::TimedOut => ControlError::Unavailable(error.to_string()),
        _ => ControlError::Io(error),
    }
}

#[cfg(unix)]
async fn is_socket_in_use(socket_path: &Path) -> bool {
    matches!(
        tokio::time::timeout(
            CONTROL_CONNECT_TIMEOUT,
            tokio::net::UnixStream::connect(socket_path),
        )
        .await,
        Ok(Ok(_))
    )
}

#[cfg(unix)]
async fn handle_control_connection(
    mut stream: tokio::net::UnixStream,
    owner_uid: u32,
    runtime: Arc<ServerRuntime>,
) -> Result<(), ControlError> {
    use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};

    if let Err(error) = authorize_control_peer_uid(&stream, owner_uid) {
        write_control_response(
            &mut stream,
            ControlResponse::Error {
                ok: false,
                error: error.to_string(),
            },
        )
        .await?;
        return Ok(());
    }

    let mut request_line = Vec::new();
    let bytes_read = {
        let reader = BufReader::new(&mut stream);
        let mut limited = reader.take((MAX_CONTROL_REQUEST_BYTES + 1) as u64);
        limited
            .read_until(b'\n', &mut request_line)
            .await
            .map_err(ControlError::Io)?
    };
    if bytes_read == 0 {
        return Ok(());
    }
    if bytes_read > MAX_CONTROL_REQUEST_BYTES {
        write_control_response(
            &mut stream,
            ControlResponse::Error {
                ok: false,
                error: format!("request exceeded {MAX_CONTROL_REQUEST_BYTES} bytes"),
            },
        )
        .await?;
        return Ok(());
    }

    let request_text = match std::str::from_utf8(&request_line) {
        Ok(request_text) => request_text.trim_end_matches(&['\r', '\n'][..]),
        Err(error) => {
            write_control_response(
                &mut stream,
                ControlResponse::Error {
                    ok: false,
                    error: error.to_string(),
                },
            )
            .await?;
            return Ok(());
        }
    };
    let request = match serde_json::from_str::<ControlRequest>(request_text) {
        Ok(request) => request,
        Err(error) => {
            write_control_response(
                &mut stream,
                ControlResponse::Error {
                    ok: false,
                    error: error.to_string(),
                },
            )
            .await?;
            return Ok(());
        }
    };

    let response = handle_control_request(request, runtime);
    write_control_response(&mut stream, response).await?;
    Ok(())
}

#[cfg(unix)]
fn handle_control_request(request: ControlRequest, runtime: Arc<ServerRuntime>) -> ControlResponse {
    match request {
        ControlRequest::Status => ControlResponse::Status {
            ok: true,
            status: runtime.status(),
        },
        ControlRequest::NodesList => ControlResponse::Nodes {
            ok: true,
            nodes: runtime.nodes_snapshot(),
        },
        ControlRequest::NodeDisconnect {
            node_number,
            reason,
        } => {
            if runtime.request_node_disconnect(node_number, reason) {
                tracing::info!(
                    node = node_number,
                    "disconnect requested through control socket"
                );
                ControlResponse::Ok { ok: true }
            } else {
                ControlResponse::Error {
                    ok: false,
                    error: format!("node {node_number} is not active"),
                }
            }
        }
        ControlRequest::NodeMessage { node_number, text } => {
            if runtime.queue_node_message(node_number, format!("[SYSOP] {text}")) {
                tracing::info!(
                    node = node_number,
                    text_len = text.len(),
                    "message requested through control socket"
                );
                ControlResponse::Ok { ok: true }
            } else {
                ControlResponse::Error {
                    ok: false,
                    error: format!("node {node_number} is not active"),
                }
            }
        }
        ControlRequest::NodeBroadcast { text } => {
            let active_nodes = runtime.queue_broadcast(format!("[SYSOP BROADCAST] {text}"));
            tracing::info!(
                broadcast_bytes = text.len(),
                active_nodes,
                "broadcast requested through control socket"
            );
            ControlResponse::Ok { ok: true }
        }
        ControlRequest::NodesResetStale => {
            let nodes = runtime.request_stale_disconnects("stale_node_reset");
            tracing::info!(
                stale_nodes = nodes.len(),
                "stale node reset requested through control socket"
            );
            ControlResponse::Ok { ok: true }
        }
    }
}

#[cfg(unix)]
async fn write_control_response(
    stream: &mut tokio::net::UnixStream,
    response: ControlResponse,
) -> Result<(), ControlError> {
    use tokio::io::AsyncWriteExt;

    let response_line = serde_json::to_string(&response).map_err(ControlError::Json)?;
    stream
        .write_all(response_line.as_bytes())
        .await
        .map_err(ControlError::Io)?;
    stream.write_all(b"\n").await.map_err(ControlError::Io)?;
    stream.shutdown().await.map_err(ControlError::Io)?;
    Ok(())
}

#[cfg(unix)]
fn authorize_control_peer_uid(
    stream: &tokio::net::UnixStream,
    expected_uid: u32,
) -> Result<(), ControlError> {
    let peer_uid = control_peer_uid(stream)?;
    if is_authorized_control_uid(peer_uid, expected_uid) {
        Ok(())
    } else {
        Err(ControlError::Protocol(format!(
            "control socket access denied for peer uid {peer_uid} (expected {expected_uid})",
        )))
    }
}

#[cfg(unix)]
fn control_peer_uid(stream: &tokio::net::UnixStream) -> Result<u32, ControlError> {
    let peer_cred = stream.peer_cred().map_err(|error| {
        ControlError::Protocol(format!(
            "unable to obtain peer uid from control socket: {error}"
        ))
    })?;
    Ok(peer_cred.uid())
}

#[cfg(unix)]
fn control_process_uid() -> u32 {
    nix::unistd::Uid::effective().as_raw()
}

#[cfg(unix)]
fn is_authorized_control_uid(peer_uid: u32, expected_uid: u32) -> bool {
    peer_uid == expected_uid
}

fn timestamp_string() -> String {
    let since_epoch = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    format!("{}.{}", since_epoch.as_secs(), since_epoch.subsec_millis())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::SystemTime;

    const TEST_NODE_COUNT: u16 = 4;

    fn temporary_runtime_path() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        let mut path = std::env::temp_dir();
        path.push(format!("oxidebbs-control-test-{nanos}"));
        let _ = fs::remove_dir_all(&path);
        path
    }

    #[test]
    fn control_request_json_round_trip() {
        let request = ControlRequest::NodeMessage {
            node_number: 1,
            text: "hello".to_string(),
        };
        let request_json = serde_json::to_string(&request).expect("serialize");
        let parsed = serde_json::from_str::<ControlRequest>(&request_json).expect("parse");
        match parsed {
            ControlRequest::NodeMessage { node_number, text } => {
                assert_eq!(node_number, 1);
                assert_eq!(text, "hello");
            }
            _ => panic!("unexpected variant"),
        }
    }

    #[test]
    fn control_response_json_round_trip() {
        let response = ControlResponse::Nodes {
            ok: true,
            nodes: vec![ControlNodeStatus {
                node_number: 1,
                state: "active".to_string(),
                user_alias: Some("sysop".to_string()),
                remote_address: Some("127.0.0.1:10".to_string()),
                connected_at: Some("now".to_string()),
                last_heartbeat_at: Some("now".to_string()),
                heartbeat_age_seconds: Some(0),
            }],
        };
        let response_json = serde_json::to_string(&response).expect("serialize");
        let parsed = serde_json::from_str::<ControlResponse>(&response_json).expect("parse");
        match parsed {
            ControlResponse::Nodes { nodes, .. } => {
                assert_eq!(nodes.len(), 1);
                assert_eq!(nodes[0].node_number, 1);
            }
            _ => panic!("unexpected variant"),
        }
    }

    #[test]
    fn control_status_json_includes_audit_write_failures() {
        let runtime = ServerRuntime::new("test".to_string(), TEST_NODE_COUNT, 4, 60);
        runtime.record_audit_write_failure();

        let response = ControlResponse::Status {
            ok: true,
            status: runtime.status(),
        };
        let response_json = serde_json::to_value(&response).expect("serialize");

        assert_eq!(
            response_json["status"]["audit_write_failures"],
            serde_json::json!(1)
        );
    }

    #[test]
    fn control_unknown_request_rejected() {
        let bad = r#"{"type":"does.not.exist"}"#;
        assert!(serde_json::from_str::<ControlRequest>(bad).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn control_peer_uid_rejection_checks_exact_uid_match() {
        let uid = 1000u32;
        assert!(is_authorized_control_uid(uid, uid));
        assert!(!is_authorized_control_uid(uid, uid.saturating_add(1)));
    }

    #[test]
    fn runtime_states_map_to_core_node_status() {
        assert_eq!(
            RuntimeNodeState::Available.to_core_status(),
            NodeStatus::Idle
        );
        assert_eq!(
            RuntimeNodeState::Connecting.to_core_status(),
            NodeStatus::Connected
        );
        assert_eq!(
            RuntimeNodeState::Login.to_core_status(),
            NodeStatus::LoggingIn
        );
        assert_eq!(
            RuntimeNodeState::MainMenu.to_core_status(),
            NodeStatus::InMenu
        );
        assert_eq!(
            RuntimeNodeState::ReadingMessages.to_core_status(),
            NodeStatus::InMenu
        );
        assert_eq!(
            RuntimeNodeState::PostingMessage.to_core_status(),
            NodeStatus::InMenu
        );
        assert_eq!(
            RuntimeNodeState::InDoor.to_core_status(),
            NodeStatus::InDoor
        );
        assert_eq!(
            RuntimeNodeState::Disconnecting.to_core_status(),
            NodeStatus::Disconnected
        );
        assert_eq!(
            RuntimeNodeState::Stale.to_core_status(),
            NodeStatus::Disconnected
        );
    }

    #[test]
    fn runtime_state_transitions_and_heartbeat_updates() {
        let runtime = Arc::new(ServerRuntime::new(
            "test".to_string(),
            TEST_NODE_COUNT,
            4,
            60,
        ));
        runtime.mark_node_connected(
            1,
            "session-1".to_string(),
            "127.0.0.1:5000".to_string(),
            "connected".to_string(),
        );
        runtime.mark_node_login(1);
        assert_eq!(runtime.node_status(1).expect("node").state, "login");

        runtime.force_node_heartbeat_age(1, Duration::from_secs(10));
        assert!(
            runtime
                .node_status(1)
                .expect("aged node")
                .heartbeat_age_seconds
                .expect("heartbeat age")
                >= 10
        );

        runtime.heartbeat_node(1);
        assert!(
            runtime
                .node_status(1)
                .expect("fresh node")
                .heartbeat_age_seconds
                .expect("heartbeat age")
                <= 1
        );
    }

    #[test]
    fn stale_nodes_are_detected_and_reset_to_disconnect() {
        let runtime = Arc::new(ServerRuntime::new(
            "test".to_string(),
            TEST_NODE_COUNT,
            4,
            1,
        ));
        runtime.mark_node_connected(
            2,
            "session-2".to_string(),
            "127.0.0.1:5001".to_string(),
            "connected".to_string(),
        );
        runtime.force_node_heartbeat_age(2, Duration::from_secs(5));

        assert_eq!(runtime.node_status(2).expect("node").state, "stale");
        assert_eq!(
            runtime.request_stale_disconnects("stale_node_reset"),
            vec![2]
        );
        assert_eq!(runtime.node_status(2).expect("node").state, "disconnecting");
        assert_eq!(
            runtime.take_node_commands(2).disconnect_reason.as_deref(),
            Some("stale_node_reset")
        );
    }

    #[test]
    fn runtime_nodes_snapshot_is_ordered() {
        let runtime = Arc::new(ServerRuntime::new(
            "test".to_string(),
            TEST_NODE_COUNT,
            4,
            60,
        ));
        runtime.mark_node_connected(
            3,
            "session-3".to_string(),
            "127.0.0.1:5002".to_string(),
            "connected".to_string(),
        );

        let nodes = runtime.nodes_snapshot();
        assert_eq!(
            nodes
                .iter()
                .map(|node| node.node_number)
                .collect::<Vec<_>>(),
            vec![1, 2, 3, 4]
        );
        assert_eq!(nodes[0].state, "available");
        assert_eq!(nodes[2].state, "connecting");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn control_socket_round_trip() {
        let runtime_dir = temporary_runtime_path();
        fs::create_dir_all(&runtime_dir).expect("runtime dir");
        let runtime = Arc::new(ServerRuntime::new(
            "test".to_string(),
            TEST_NODE_COUNT,
            4,
            60,
        ));

        let listen_task = start_control_listener(&runtime_dir, Arc::clone(&runtime))
            .await
            .expect("start listener");

        let socket_path = control_socket_path(&runtime_dir);
        for _ in 0..100 {
            if socket_path.exists() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert!(socket_path.exists());
        #[cfg(unix)]
        {
            let socket_mode = fs::metadata(&socket_path)
                .expect("socket metadata")
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(socket_mode, 0o600);

            let runtime_mode = fs::metadata(&runtime_dir)
                .expect("runtime metadata")
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(runtime_mode, 0o700);
        }

        runtime.mark_node_connected(
            1,
            "session-1".to_string(),
            "127.0.0.1:5000".to_string(),
            "connected".to_string(),
        );
        runtime.heartbeat_node(1);

        let response = tokio::task::spawn_blocking({
            let runtime_dir = runtime_dir.clone();
            move || request_status(&runtime_dir).expect("request status")
        })
        .await
        .expect("join");

        match response {
            ControlResponse::Status { status, .. } => {
                assert_eq!(status.node_count, TEST_NODE_COUNT);
                assert_eq!(status.active_nodes, 1);
            }
            other => panic!("unexpected response: {other:?}"),
        }

        let response = tokio::task::spawn_blocking({
            let runtime_dir = runtime_dir.clone();
            move || {
                request_nodes_message(&runtime_dir, 1, "hi\nthere".to_string())
                    .expect("request message")
            }
        })
        .await
        .expect("join");
        assert!(matches!(response, ControlResponse::Ok { ok: true }));
        let commands = runtime.take_node_commands(1);
        assert_eq!(commands.messages, vec!["[SYSOP] hi there"]);

        let response = tokio::task::spawn_blocking({
            let runtime_dir = runtime_dir.clone();
            move || {
                request_nodes_disconnect(&runtime_dir, 1, "sysop_disconnect".to_string())
                    .expect("request disconnect")
            }
        })
        .await
        .expect("join");
        assert!(matches!(response, ControlResponse::Ok { ok: true }));
        let commands = runtime.take_node_commands(1);
        assert_eq!(
            commands.disconnect_reason.as_deref(),
            Some("sysop_disconnect")
        );

        runtime.mark_node_connected(
            2,
            "session-2".to_string(),
            "127.0.0.1:5001".to_string(),
            "connected".to_string(),
        );
        runtime.force_node_heartbeat_age(2, Duration::from_secs(120));
        let response = tokio::task::spawn_blocking({
            let runtime_dir = runtime_dir.clone();
            move || request_nodes_reset_stale(&runtime_dir).expect("request reset stale")
        })
        .await
        .expect("join");
        assert!(matches!(response, ControlResponse::Ok { ok: true }));
        assert_eq!(
            runtime.take_node_commands(2).disconnect_reason.as_deref(),
            Some("stale_node_reset")
        );

        listen_task.abort();
        let _ = fs::remove_dir_all(&runtime_dir);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn control_socket_clients_do_not_block_each_other() {
        let runtime_dir = temporary_runtime_path();
        fs::create_dir_all(&runtime_dir).expect("runtime dir");
        let runtime = Arc::new(ServerRuntime::new(
            "test".to_string(),
            TEST_NODE_COUNT,
            4,
            60,
        ));

        let listen_task = start_control_listener(&runtime_dir, Arc::clone(&runtime))
            .await
            .expect("start listener");

        let socket_path = control_socket_path(&runtime_dir);
        for _ in 0..100 {
            if socket_path.exists() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert!(socket_path.exists());

        let handles = (0..2).map(|_| {
            let runtime_dir = runtime_dir.clone();
            tokio::task::spawn_blocking(move || {
                let response = request_nodes(&runtime_dir).expect("request nodes");
                matches!(response, ControlResponse::Nodes { ok: true, .. })
            })
        });

        for handle in handles {
            assert!(handle.await.expect("join"));
        }

        listen_task.abort();
        let _ = fs::remove_dir_all(&runtime_dir);
    }
}
