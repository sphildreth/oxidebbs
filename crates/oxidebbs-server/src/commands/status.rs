use serde_json::json;

use crate::{
    commands::doors,
    control::{ControlResponse, request_status},
    sysop_cli::{AppContext, CliError, CliResult, open_database, print_json},
};
use oxidebbs_db::list_message_areas;

fn request_live_status(
    config: &crate::config::OxideConfig,
) -> CliResult<Option<serde_json::Value>> {
    match request_status(&config.paths.runtime) {
        Ok(ControlResponse::Status { status, .. }) => Ok(Some(json!({
            "board_name": status.board_name,
            "uptime_seconds": status.uptime_seconds,
            "node_count": status.node_count,
            "active_nodes": status.active_nodes
        }))),
        Ok(ControlResponse::Error { error, .. }) => Err(CliError::Message(format!(
            "control socket reported error: {error}"
        ))),
        Ok(ControlResponse::Ok { .. }) => Err(CliError::Message(
            "control socket returned unexpected response to status request".to_string(),
        )),
        Ok(ControlResponse::Nodes { .. }) => Err(CliError::Message(
            "control socket returned unexpected response to status request".to_string(),
        )),
        Err(error) if error.is_unreachable() => Ok(None),
        Err(error) => Err(CliError::Message(format!("status request failed: {error}"))),
    }
}

pub fn run_status(ctx: &AppContext) -> CliResult<()> {
    let db = open_database(&ctx.config)?;
    doors::sync_configured_doors(&db, &ctx.config)?;
    let doors = doors::effective_doors(&db, &ctx.config)?;
    let enabled_doors = doors.iter().filter(|door| door.enabled).count();
    let message_areas = list_message_areas(db.db())?;
    let version = env!("CARGO_PKG_VERSION");
    let live_status = request_live_status(&ctx.config)?;

    let total_nodes = live_status
        .as_ref()
        .and_then(|status| status.get("node_count").and_then(|count| count.as_u64()))
        .unwrap_or(ctx.config.nodes.count as u64);
    let active_nodes = live_status
        .as_ref()
        .and_then(|status| status.get("active_nodes").and_then(|count| count.as_u64()))
        .unwrap_or(0);
    let uptime_seconds = live_status.as_ref().and_then(|status| {
        status
            .get("uptime_seconds")
            .and_then(|value| value.as_u64())
    });
    let board_name = live_status
        .as_ref()
        .and_then(|status| status.get("board_name").and_then(|name| name.as_str()))
        .unwrap_or(&ctx.config.board.name);

    if ctx.json {
        print_json(&status_json_payload(StatusJsonSummary {
            board_name,
            version,
            database: &ctx.config.database.path,
            telnet: &ctx.config.telnet.bind,
            total_nodes,
            active_nodes,
            enabled_doors,
            total_doors: doors.len(),
            area_count: message_areas.len(),
        }))?;
    } else {
        println!("OxideBBS Status");
        println!("Board:        {board_name}");
        println!("Version:      {version}");
        println!("Database:     {}", ctx.config.database.path.display());
        println!("Telnet:       {}", ctx.config.telnet.bind);
        println!(
            "Nodes:        {} total, {} active",
            total_nodes, active_nodes
        );
        println!("Doors:        {enabled_doors} enabled");
        println!("Messages:     {} areas", message_areas.len());
        match uptime_seconds {
            Some(seconds) => println!("Uptime:      {} seconds (live runtime)", seconds),
            None => println!("Uptime:      unavailable while control socket is unreachable"),
        }
    }
    Ok(())
}

struct StatusJsonSummary<'a> {
    board_name: &'a str,
    version: &'a str,
    database: &'a std::path::Path,
    telnet: &'a str,
    total_nodes: u64,
    active_nodes: u64,
    enabled_doors: usize,
    total_doors: usize,
    area_count: usize,
}

fn status_json_payload(summary: StatusJsonSummary<'_>) -> serde_json::Value {
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
    fn status_json_shape_matches_contract() {
        let payload = status_json_payload(StatusJsonSummary {
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
}
