use std::collections::BTreeMap;
#[cfg(unix)]
use std::io::{BufRead, BufReader, Write as IoWrite};
use std::io::{Error as IoError, ErrorKind};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
#[cfg(unix)]
use std::time::Duration;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::Notify;

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
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ControlNodeStatus {
    pub node_number: u16,
    pub state: String,
    pub user_alias: Option<String>,
    pub remote_address: Option<String>,
    pub connected_at: Option<String>,
    pub last_heartbeat_at: Option<String>,
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

#[derive(Debug, Clone)]
struct RuntimeNode {
    session_id: String,
    user_id: Option<String>,
    user_alias: Option<String>,
    remote_address: String,
    connected_at: String,
    last_heartbeat_at: String,
}

#[derive(Debug)]
pub struct ServerRuntime {
    board_name: String,
    node_count: u16,
    started_at: SystemTime,
    nodes: Mutex<BTreeMap<u16, RuntimeNode>>,
    disconnect_requests: Mutex<BTreeMap<u16, String>>,
    node_messages: Mutex<BTreeMap<u16, Vec<String>>>,
    command_notify: Notify,
}

impl ServerRuntime {
    pub fn new(board_name: String, node_count: u16) -> Self {
        Self {
            board_name,
            node_count,
            started_at: SystemTime::now(),
            nodes: Mutex::new(BTreeMap::new()),
            disconnect_requests: Mutex::new(BTreeMap::new()),
            node_messages: Mutex::new(BTreeMap::new()),
            command_notify: Notify::new(),
        }
    }

    pub fn mark_node_connected(
        &self,
        node_number: u16,
        session_id: String,
        remote_address: String,
        connected_at: String,
    ) {
        let now = timestamp_string();
        if let Ok(mut nodes) = self.nodes.lock() {
            nodes.insert(
                node_number,
                RuntimeNode {
                    session_id,
                    user_id: None,
                    user_alias: None,
                    remote_address,
                    connected_at,
                    last_heartbeat_at: now,
                },
            );
        }
        if let Ok(mut disconnects) = self.disconnect_requests.lock() {
            disconnects.remove(&node_number);
        }
        if let Ok(mut messages) = self.node_messages.lock() {
            messages.remove(&node_number);
        }
    }

    pub fn mark_node_disconnected(&self, node_number: u16) -> Option<String> {
        let session_id = self
            .nodes
            .lock()
            .ok()
            .and_then(|mut nodes| nodes.remove(&node_number))
            .map(|node| node.session_id);
        if let Ok(mut disconnects) = self.disconnect_requests.lock() {
            disconnects.remove(&node_number);
        }
        if let Ok(mut messages) = self.node_messages.lock() {
            messages.remove(&node_number);
        }
        session_id
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
            node.last_heartbeat_at = timestamp_string();
        }
    }

    pub fn heartbeat_node(&self, node_number: u16) {
        if let Ok(mut nodes) = self.nodes.lock()
            && let Some(node) = nodes.get_mut(&node_number)
        {
            node.last_heartbeat_at = timestamp_string();
        }
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
        }
    }

    pub fn nodes_snapshot(&self) -> Vec<ControlNodeStatus> {
        let nodes = self
            .nodes
            .lock()
            .map(|nodes| nodes.clone())
            .unwrap_or_default();
        (1..=self.node_count)
            .map(|node_number| {
                if let Some(node) = nodes.get(&node_number) {
                    node_status(node_number, Some(node))
                } else {
                    ControlNodeStatus {
                        node_number,
                        state: "available".to_string(),
                        user_alias: None,
                        remote_address: None,
                        connected_at: None,
                        last_heartbeat_at: None,
                    }
                }
            })
            .collect()
    }

    pub fn node_status(&self, node_number: u16) -> Option<ControlNodeStatus> {
        self.nodes.lock().ok().and_then(|nodes| {
            nodes
                .get(&node_number)
                .map(|node| node_status(node_number, Some(node)))
        })
    }

    pub fn request_node_disconnect(&self, node_number: u16, reason: String) -> bool {
        if self.node_status(node_number).is_none() {
            return false;
        }
        if let Ok(mut disconnects) = self.disconnect_requests.lock() {
            disconnects.insert(node_number, reason);
            self.command_notify.notify_waiters();
            return true;
        }
        false
    }

    pub fn queue_node_message(&self, node_number: u16, text: String) -> bool {
        if self.node_status(node_number).is_none() {
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
}

fn node_status(node_number: u16, node: Option<&RuntimeNode>) -> ControlNodeStatus {
    if let Some(node) = node {
        ControlNodeStatus {
            node_number,
            state: "active".to_string(),
            user_alias: node.user_alias.clone(),
            remote_address: Some(node.remote_address.clone()),
            connected_at: Some(node.connected_at.clone()),
            last_heartbeat_at: Some(node.last_heartbeat_at.clone()),
        }
    } else {
        ControlNodeStatus {
            node_number,
            state: "available".to_string(),
            user_alias: None,
            remote_address: None,
            connected_at: None,
            last_heartbeat_at: None,
        }
    }
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

#[cfg(unix)]
pub async fn start_control_listener(
    runtime_dir: &Path,
    runtime: Arc<ServerRuntime>,
) -> Result<tokio::task::JoinHandle<()>, ControlError> {
    let listener = bind_control_listener(runtime_dir).await?;
    Ok(tokio::spawn(async move {
        if let Err(error) = run_control_accept_loop(listener, runtime).await {
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
async fn bind_control_listener(
    runtime_dir: &Path,
) -> Result<tokio::net::UnixListener, ControlError> {
    let socket_path = control_socket_path(runtime_dir);
    tokio::fs::create_dir_all(runtime_dir)
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
    tokio::net::UnixListener::bind(&socket_path).map_err(ControlError::Io)
}

#[cfg(unix)]
async fn run_control_accept_loop(
    listener: tokio::net::UnixListener,
    runtime: Arc<ServerRuntime>,
) -> Result<(), ControlError> {
    loop {
        let (stream, _) = listener.accept().await.map_err(ControlError::Io)?;
        let runtime = Arc::clone(&runtime);
        tokio::spawn(async move {
            if let Err(error) = handle_control_connection(stream, runtime).await {
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
    runtime: Arc<ServerRuntime>,
) -> Result<(), ControlError> {
    use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};

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
    fn control_unknown_request_rejected() {
        let bad = r#"{"type":"does.not.exist"}"#;
        assert!(serde_json::from_str::<ControlRequest>(bad).is_err());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn control_socket_round_trip() {
        let runtime_dir = temporary_runtime_path();
        fs::create_dir_all(&runtime_dir).expect("runtime dir");
        let runtime = Arc::new(ServerRuntime::new("test".to_string(), TEST_NODE_COUNT));

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

        listen_task.abort();
        let _ = fs::remove_dir_all(&runtime_dir);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn control_socket_clients_do_not_block_each_other() {
        let runtime_dir = temporary_runtime_path();
        fs::create_dir_all(&runtime_dir).expect("runtime dir");
        let runtime = Arc::new(ServerRuntime::new("test".to_string(), TEST_NODE_COUNT));

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
