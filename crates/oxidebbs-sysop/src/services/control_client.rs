use std::io::{BufRead, Write};
use std::net::Shutdown;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::SysopError;

pub const CONTROL_SOCKET_NAME: &str = "oxidebbs-control.sock";
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

    let request_line = serde_json::to_string(request)
        .map_err(|e| SysopError::Control(format!("serialize: {e}")))?
        + "\n";
    if request_line.len() > MAX_REQUEST_BYTES {
        return Err(SysopError::Control("request too large".into()));
    }
    stream
        .write_all(request_line.as_bytes())
        .map_err(|e| SysopError::Control(format!("write request: {e}")))?;
    stream
        .shutdown(Shutdown::Write)
        .map_err(|e| SysopError::Control(format!("finish request: {e}")))?;

    let mut reader = std::io::BufReader::new(&stream);
    let mut response_line = String::new();
    let bytes_read = reader
        .read_line(&mut response_line)
        .map_err(|e| SysopError::Control(format!("read response: {e}")))?;
    if bytes_read == 0 {
        return Err(SysopError::Control("empty response".into()));
    }
    if bytes_read > MAX_REQUEST_BYTES {
        return Err(SysopError::Control("response too large".into()));
    }
    let response: ControlResponse = serde_json::from_str(response_line.trim_end())
        .map_err(|e| SysopError::Control(format!("deserialize: {e}")))?;

    Ok(response)
}

pub fn is_socket_available(socket_path: &Path) -> bool {
    if !socket_path.exists() {
        return false;
    }
    matches!(
        send_control_request(socket_path, &ControlRequest::Status),
        Ok(ControlResponse::Status { ok: true, .. })
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::{BufRead, Write};
    use std::os::unix::net::UnixListener;
    use std::thread;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn send_control_request_uses_json_line_protocol() {
        let dir = std::env::temp_dir().join(format!(
            "oxidebbs-control-client-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system time should be valid")
                .as_nanos()
        ));
        fs::create_dir_all(&dir).expect("create temp dir");
        let socket_path = dir.join(CONTROL_SOCKET_NAME);
        let listener = UnixListener::bind(&socket_path).expect("bind test socket");

        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept control client");
            let mut request_line = String::new();
            {
                let mut reader = std::io::BufReader::new(&stream);
                reader
                    .read_line(&mut request_line)
                    .expect("read request line");
            }
            assert_eq!(request_line, "{\"type\":\"status\"}\n");
            stream
                .write_all(
                    br#"{"type":"status","ok":true,"status":{"board_name":"Test BBS","uptime_seconds":3,"node_count":2,"active_nodes":1,"audit_write_failures":0}}"#,
                )
                .expect("write response");
            stream.write_all(b"\n").expect("write response newline");
        });

        let response = send_control_request(&socket_path, &ControlRequest::Status)
            .expect("control request should succeed");
        match response {
            ControlResponse::Status { ok: true, status } => {
                assert_eq!(status.board_name, "Test BBS");
                assert_eq!(status.node_count, 2);
                assert_eq!(status.active_nodes, 1);
            }
            other => panic!("unexpected response: {other:?}"),
        }

        server.join().expect("server thread should finish");
        let _ = fs::remove_dir_all(dir);
    }
}
