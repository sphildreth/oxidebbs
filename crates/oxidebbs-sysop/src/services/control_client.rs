use std::io::{BufRead, Read, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::SysopError;

pub const CONTROL_SOCKET_NAME: &str = "oxidebbs-control.sock";
const CONTROL_CONNECT_TIMEOUT: Duration = Duration::from_millis(250);
const SOCKET_READ_TIMEOUT: Duration = Duration::from_secs(2);
const SOCKET_WRITE_TIMEOUT: Duration = Duration::from_secs(2);
const MAX_REQUEST_BYTES: usize = 64 * 1024;

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

pub fn control_socket_path(runtime_dir: &Path) -> PathBuf {
    runtime_dir.join(CONTROL_SOCKET_NAME)
}

pub fn send_control_request(
    socket_path: &Path,
    request: &ControlRequest,
) -> Result<ControlResponse, SysopError> {
    let mut stream = UnixStream::connect(socket_path)
        .map_err(|e| SysopError::Control(format!("connect failed: {e}")))?;
    stream
        .set_read_timeout(Some(SOCKET_READ_TIMEOUT))
        .map_err(|e| SysopError::Control(format!("set timeout: {e}")))?;
    stream
        .set_write_timeout(Some(SOCKET_WRITE_TIMEOUT))
        .map_err(|e| SysopError::Control(format!("set timeout: {e}")))?;

    let request_json =
        serde_json::to_vec(request).map_err(|e| SysopError::Control(format!("serialize: {e}")))?;
    if request_json.len() > MAX_REQUEST_BYTES {
        return Err(SysopError::Control("request too large".into()));
    }

    let header = format!("{}\n", request_json.len());
    stream
        .write_all(header.as_bytes())
        .map_err(|e| SysopError::Control(format!("write header: {e}")))?;
    stream
        .write_all(&request_json)
        .map_err(|e| SysopError::Control(format!("write body: {e}")))?;

    let mut reader = std::io::BufReader::new(&stream);
    let mut header_line = String::new();
    let bytes_read = reader
        .read_line(&mut header_line)
        .map_err(|e| SysopError::Control(format!("read header: {e}")))?;
    if bytes_read == 0 {
        return Err(SysopError::Control("empty response".into()));
    }
    let response_len: usize = header_line
        .trim()
        .parse()
        .map_err(|e| SysopError::Control(format!("invalid response length: {e}")))?;
    if response_len > MAX_REQUEST_BYTES {
        return Err(SysopError::Control("response too large".into()));
    }

    let mut response_json = vec![0u8; response_len];
    reader
        .read_exact(&mut response_json)
        .map_err(|e| SysopError::Control(format!("read body: {e}")))?;

    let response: ControlResponse = serde_json::from_slice(&response_json)
        .map_err(|e| SysopError::Control(format!("deserialize: {e}")))?;

    Ok(response)
}

pub fn is_socket_available(socket_path: &Path) -> bool {
    if !socket_path.exists() {
        return false;
    }
    match UnixStream::connect(socket_path) {
        Ok(mut stream) => {
            let _ = stream.set_write_timeout(Some(CONTROL_CONNECT_TIMEOUT));
            let ping = serde_json::to_vec(&ControlRequest::Status).unwrap_or_default();
            let header = format!("{}\n", ping.len());
            stream.write_all(header.as_bytes()).is_ok() && stream.write_all(&ping).is_ok()
        }
        Err(_) => false,
    }
}
