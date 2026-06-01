use serde_json::json;

use crate::{
    commands::doors,
    control::{ControlResponse, request_status},
    sysop_cli::{AppContext, CliError, CliResult, open_database, print_json},
};
use oxidebbs_db::list_active_sessions;
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
    let active = list_active_sessions(db.db())?;
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
        .unwrap_or(active.len() as u64);
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
        print_json(&json!({
            "board": board_name,
            "version": version,
            "database": ctx.config.database.path,
            "telnet": ctx.config.telnet.bind,
            "nodes": {
                "total": total_nodes,
                "active": active_nodes,
                "live_control": live_status.is_some(),
            },
            "doors": { "enabled": enabled_doors, "total": doors.len() },
            "messages": { "areas": message_areas.len() },
            "uptime_seconds": uptime_seconds,
            "control": {
                "reachable": live_status.is_some(),
                "socket": ctx.config.paths.runtime.join("oxidebbs-control.sock")
            }
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
