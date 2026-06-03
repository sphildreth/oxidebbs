use oxidebbs_db::{Db, SessionRecord, find_active_session_by_node, list_active_sessions};
use std::path::PathBuf;

use crate::SysopError;
use crate::events::NodeStatusSnapshot;
use crate::services::control_client::{
    ControlNodeStatus, ControlRequest, ControlResponse, is_socket_available, send_control_request,
};

pub struct NodeAdminService {
    control_socket_path: Option<PathBuf>,
}

impl NodeAdminService {
    pub fn new(control_socket_path: Option<PathBuf>) -> Self {
        Self {
            control_socket_path,
        }
    }

    pub fn list_active(db: &Db) -> Result<Vec<SessionRecord>, SysopError> {
        Ok(list_active_sessions(db)?)
    }

    pub fn find_session(db: &Db, node_number: i64) -> Result<Option<SessionRecord>, SysopError> {
        Ok(find_active_session_by_node(db, node_number)?)
    }

    pub fn list_nodes(
        &self,
        db: &Db,
        total_configured: u16,
    ) -> Result<Vec<NodeStatusSnapshot>, SysopError> {
        if let Some(ref path) = self.control_socket_path
            && is_socket_available(path)
        {
            match send_control_request(path, &ControlRequest::NodesList) {
                Ok(ControlResponse::Nodes { nodes, .. }) => {
                    return Ok(nodes.into_iter().map(control_node_to_snapshot).collect());
                }
                Ok(_) => {}
                Err(_) => {}
            }
        }

        let sessions = list_active_sessions(db)?;
        let mut snapshots: Vec<NodeStatusSnapshot> = sessions
            .into_iter()
            .map(|s| NodeStatusSnapshot {
                node_number: s.node_number as u16,
                state: "connected".to_string(),
                user_alias: None,
                remote_address: Some(s.remote_address),
                connected_at: Some(s.started_at),
                last_heartbeat_at: None,
                heartbeat_age_seconds: None,
            })
            .collect();

        for node_num in 1..=total_configured {
            if !snapshots.iter().any(|n| n.node_number == node_num) {
                snapshots.push(NodeStatusSnapshot {
                    node_number: node_num,
                    state: "available".to_string(),
                    user_alias: None,
                    remote_address: None,
                    connected_at: None,
                    last_heartbeat_at: None,
                    heartbeat_age_seconds: None,
                });
            }
        }
        snapshots.sort_by_key(|n| n.node_number);
        Ok(snapshots)
    }

    pub fn disconnect_node(&self, node_number: u16, reason: &str) -> Result<(), SysopError> {
        if let Some(ref path) = self.control_socket_path
            && is_socket_available(path)
        {
            match send_control_request(
                path,
                &ControlRequest::NodeDisconnect {
                    node_number,
                    reason: reason.to_string(),
                },
            ) {
                Ok(ControlResponse::Ok { ok: true, .. }) => return Ok(()),
                Ok(ControlResponse::Error { error, .. }) => {
                    return Err(SysopError::Control(format!("disconnect failed: {error}")));
                }
                Ok(_) => {}
                Err(e) => return Err(e),
            }
        }
        Err(SysopError::Control(
            "control socket unavailable for disconnect".into(),
        ))
    }

    pub fn send_message(&self, node_number: u16, text: &str) -> Result<(), SysopError> {
        if let Some(ref path) = self.control_socket_path
            && is_socket_available(path)
        {
            match send_control_request(
                path,
                &ControlRequest::NodeMessage {
                    node_number,
                    text: text.to_string(),
                },
            ) {
                Ok(ControlResponse::Ok { ok: true, .. }) => return Ok(()),
                Ok(ControlResponse::Error { error, .. }) => {
                    return Err(SysopError::Control(format!("message failed: {error}")));
                }
                Ok(_) => {}
                Err(e) => return Err(e),
            }
        }
        Err(SysopError::Control(
            "control socket unavailable for message".into(),
        ))
    }

    pub fn broadcast(&self, text: &str) -> Result<(), SysopError> {
        if let Some(ref path) = self.control_socket_path
            && is_socket_available(path)
        {
            match send_control_request(
                path,
                &ControlRequest::NodeBroadcast {
                    text: text.to_string(),
                },
            ) {
                Ok(ControlResponse::Ok { ok: true, .. }) => return Ok(()),
                Ok(ControlResponse::Error { error, .. }) => {
                    return Err(SysopError::Control(format!("broadcast failed: {error}")));
                }
                Ok(_) => {}
                Err(e) => return Err(e),
            }
        }
        Err(SysopError::Control(
            "control socket unavailable for broadcast".into(),
        ))
    }

    pub fn reset_stale(&self) -> Result<(), SysopError> {
        if let Some(ref path) = self.control_socket_path
            && is_socket_available(path)
        {
            match send_control_request(path, &ControlRequest::NodesResetStale) {
                Ok(ControlResponse::Ok { ok: true, .. }) => return Ok(()),
                Ok(ControlResponse::Error { error, .. }) => {
                    return Err(SysopError::Control(format!("reset stale failed: {error}")));
                }
                Ok(_) => {}
                Err(e) => return Err(e),
            }
        }
        Err(SysopError::Control(
            "control socket unavailable for reset".into(),
        ))
    }
}

fn control_node_to_snapshot(node: ControlNodeStatus) -> NodeStatusSnapshot {
    NodeStatusSnapshot {
        node_number: node.node_number,
        state: node.state,
        user_alias: node.user_alias,
        remote_address: node.remote_address,
        connected_at: node.connected_at,
        last_heartbeat_at: node.last_heartbeat_at,
        heartbeat_age_seconds: node.heartbeat_age_seconds,
    }
}
