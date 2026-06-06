use std::path::Path;

use oxidebbs_db::{DbError, OxideDb, list_door_definitions, list_message_areas};
use serde::Serialize;
use serde_json::json;

use crate::config::OxideConfig;
use crate::control::ControlStatus;

pub struct AdminStatusSummary<'a> {
    pub board_name: &'a str,
    pub version: &'a str,
    pub database: &'a Path,
    pub telnet: &'a str,
    pub total_nodes: u64,
    pub active_nodes: u64,
    pub enabled_doors: usize,
    pub total_doors: usize,
    pub area_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AdminStatusPayload {
    pub board: String,
    pub version: String,
    pub nodes: AdminStatusNodes,
    pub doors: AdminStatusDoors,
    pub messages: AdminStatusMessages,
    pub runtime: AdminStatusRuntime,
    pub admin_web: AdminStatusAdminWeb,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AdminStatusNodes {
    pub total: u64,
    pub active: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AdminStatusDoors {
    pub enabled: usize,
    pub total: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AdminStatusMessages {
    pub areas: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AdminStatusRuntime {
    pub live: bool,
    pub uptime_seconds: Option<u64>,
    pub audit_write_failures: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AdminStatusAdminWeb {
    pub read_only: bool,
    pub public_status_enabled: bool,
}

pub fn build_admin_status_payload(
    config: &OxideConfig,
    db: &OxideDb,
    runtime: Option<ControlStatus>,
) -> Result<AdminStatusPayload, DbError> {
    let doors = list_door_definitions(db.db())?;
    let message_areas = list_message_areas(db.db())?;
    let enabled_doors = doors.iter().filter(|door| door.enabled).count();

    let nodes = AdminStatusNodes {
        total: runtime
            .as_ref()
            .map(|status| u64::from(status.node_count))
            .unwrap_or(u64::from(config.nodes.count)),
        active: runtime
            .as_ref()
            .map(|status| status.active_nodes as u64)
            .unwrap_or(0),
    };
    let runtime_status = AdminStatusRuntime {
        live: runtime.is_some(),
        uptime_seconds: runtime.as_ref().map(|status| status.uptime_seconds),
        audit_write_failures: runtime.as_ref().map(|status| status.audit_write_failures),
    };

    Ok(AdminStatusPayload {
        board: runtime
            .as_ref()
            .map(|status| status.board_name.clone())
            .unwrap_or_else(|| config.board.name.clone()),
        version: env!("CARGO_PKG_VERSION").to_string(),
        nodes,
        doors: AdminStatusDoors {
            enabled: enabled_doors,
            total: doors.len(),
        },
        messages: AdminStatusMessages {
            areas: message_areas.len(),
        },
        runtime: runtime_status,
        admin_web: AdminStatusAdminWeb {
            read_only: config.admin_web.read_only,
            public_status_enabled: config.admin_web.public_status_enabled,
        },
    })
}

pub fn admin_status_json_payload(summary: AdminStatusSummary<'_>) -> serde_json::Value {
    json!({
        "board": summary.board_name,
        "version": summary.version,
        "database": summary.database,
        "telnet": summary.telnet,
        "nodes": {
            "total": summary.total_nodes,
            "active": summary.active_nodes,
        },
        "doors": { "enabled": summary.enabled_doors, "total": summary.total_doors },
        "messages": { "areas": summary.area_count },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn admin_status_json_shape_matches_contract() {
        let payload = admin_status_json_payload(AdminStatusSummary {
            board_name: "Example BBS",
            version: "1.0.0",
            database: std::path::Path::new("./data/oxidebbs.ddb"),
            telnet: "127.0.0.1:2323",
            total_nodes: 4,
            active_nodes: 0,
            enabled_doors: 1,
            total_doors: 1,
            area_count: 1,
        });

        let payload = payload.as_object().expect("status payload object");
        assert_eq!(payload.get("board"), Some(&json!("Example BBS")));
        assert_eq!(payload.get("version"), Some(&json!("1.0.0")));
        assert_eq!(payload.get("telnet"), Some(&json!("127.0.0.1:2323")));
        let nodes = payload
            .get("nodes")
            .expect("nodes key")
            .as_object()
            .expect("nodes object");
        assert_eq!(nodes.get("total"), Some(&json!(4)));
        assert_eq!(nodes.get("active"), Some(&json!(0)));
    }

    #[test]
    fn public_admin_status_payload_omits_database_path() {
        let config: OxideConfig = toml::from_str(
            r#"
[board]
name = "Example BBS"
"#,
        )
        .expect("parse config");
        let db = OxideDb::open_memory().expect("open DB");

        let payload = build_admin_status_payload(&config, &db, None).expect("build status payload");
        let json = serde_json::to_value(payload).expect("serialize payload");

        assert!(json.get("database").is_none());
        assert!(json.get("telnet").is_none());
        assert_eq!(json.get("board"), Some(&json!("Example BBS")));
        assert_eq!(json["runtime"]["live"], json!(false));
        assert_eq!(json["admin_web"]["read_only"], json!(true));
    }
}
