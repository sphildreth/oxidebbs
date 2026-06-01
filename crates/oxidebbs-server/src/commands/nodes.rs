use std::collections::HashMap;
use std::thread;
use std::time::Duration;

use clap::Subcommand;
use serde_json::Value as JsonValue;
use serde_json::json;

use crate::{
    control::{
        ControlNodeStatus, ControlResponse, request_nodes, request_nodes_broadcast,
        request_nodes_disconnect, request_nodes_message,
    },
    sysop_cli::{
        AppContext, CliError, CliResult, audit, current_timestamp, emit_ok, open_database,
        print_json, print_session, require_active_session, session_json,
    },
};
use oxidebbs_db::{end_session, find_active_session_by_node, list_active_sessions};

#[derive(Subcommand)]
pub enum NodesCommand {
    List,
    Watch {
        #[arg(short, long, default_value_t = 5)]
        interval: u64,
    },
    Show {
        node_number: i64,
    },
    Disconnect {
        node_number: i64,
    },
    Message {
        node_number: i64,
        text: String,
    },
    Broadcast {
        text: String,
    },
    Disable {
        node_number: i64,
    },
    Enable {
        node_number: i64,
    },
    ResetStale,
}

fn request_live_nodes(ctx: &AppContext) -> CliResult<Option<Vec<ControlNodeStatus>>> {
    match request_nodes(&ctx.config.paths.runtime) {
        Ok(ControlResponse::Nodes { nodes, .. }) => Ok(Some(nodes)),
        Ok(ControlResponse::Error { error, .. }) => Err(CliError::Message(format!(
            "control socket reported error: {error}"
        ))),
        Ok(ControlResponse::Ok { .. }) => Err(CliError::Message(
            "control socket returned unexpected nodes response".to_string(),
        )),
        Ok(ControlResponse::Status { .. }) => Err(CliError::Message(
            "control socket returned unexpected nodes response".to_string(),
        )),
        Err(error) if error.is_unreachable() => Ok(None),
        Err(error) => Err(CliError::Message(format!("nodes request failed: {error}"))),
    }
}

fn print_nodes_fallback(db: &oxidebbs_db::OxideDb, ctx: &AppContext) -> CliResult<()> {
    let active = list_active_sessions(db.db())?;
    let mut by_node = HashMap::new();
    for session in active {
        by_node.insert(session.node_number, session);
    }

    if ctx.json {
        let nodes = (1..=ctx.config.nodes.count)
            .map(|number| {
                let node_number = i64::from(number);
                let session = by_node.get(&node_number);
                json!({
                    "node": node_number,
                    "state": if session.is_some() { "active" } else { "available" },
                    "session": session.map(session_json)
                })
            })
            .collect::<Vec<_>>();
        print_json(&JsonValue::Array(nodes))?;
        return Ok(());
    }

    for number in 1..=ctx.config.nodes.count {
        let node_number = i64::from(number);
        if let Some(session) = by_node.get(&node_number) {
            println!(
                "node {}\tactive\t{}\t{}",
                node_number, session.transport, session.remote_address
            );
        } else {
            println!("node {node_number}\tavailable");
        }
    }

    Ok(())
}

fn control_nodes_to_json(nodes: &[ControlNodeStatus]) -> JsonValue {
    JsonValue::Array(
        nodes
            .iter()
            .map(|node| {
                json!({
                    "node": node.node_number,
                    "state": node.state,
                    "user_alias": node.user_alias,
                    "remote_address": node.remote_address,
                    "connected_at": node.connected_at,
                    "last_heartbeat_at": node.last_heartbeat_at,
                })
            })
            .collect(),
    )
}

fn print_nodes_from_control(nodes: &[ControlNodeStatus], json_output: bool) -> CliResult<()> {
    if json_output {
        print_json(&control_nodes_to_json(nodes))?;
        return Ok(());
    }

    for node in nodes {
        if node.state == "active" {
            println!(
                "node {}\t{}\t{}\t{}",
                node.node_number,
                node.state,
                node.user_alias.as_deref().unwrap_or("-"),
                node.remote_address.as_deref().unwrap_or("-")
            );
        } else {
            println!("node {}\t{}", node.node_number, node.state);
        }
    }
    Ok(())
}

fn print_nodes(db: &oxidebbs_db::OxideDb, ctx: &AppContext) -> CliResult<()> {
    let live_nodes = request_live_nodes(ctx)?;

    if let Some(nodes) = live_nodes {
        print_nodes_from_control(&nodes, ctx.json)
    } else {
        print_nodes_fallback(db, ctx)
    }
}

fn show_node_live(
    nodes: &[ControlNodeStatus],
    node_number: i64,
    json_output: bool,
) -> CliResult<()> {
    let Some(node) = nodes
        .iter()
        .find(|node| node.node_number as i64 == node_number)
    else {
        return Err(CliError::Message(format!(
            "node {node_number} was not found"
        )));
    };

    if json_output {
        print_json(&json!({
            "node": node.node_number,
            "state": node.state,
            "user_alias": node.user_alias,
            "remote_address": node.remote_address,
            "connected_at": node.connected_at,
            "last_heartbeat_at": node.last_heartbeat_at,
        }))?;
        return Ok(());
    }

    println!("node {}: {}", node.node_number, node.state);
    if let Some(user_alias) = node.user_alias.as_deref() {
        println!("user: {user_alias}");
    }
    if let Some(remote) = node.remote_address.as_deref() {
        println!("remote: {remote}");
    }
    if let Some(connected_at) = node.connected_at.as_deref() {
        println!("connected_at: {connected_at}");
    }
    if let Some(last_heartbeat) = node.last_heartbeat_at.as_deref() {
        println!("last_heartbeat_at: {last_heartbeat}");
    }
    Ok(())
}

fn request_u16_node(node_number: i64) -> CliResult<u16> {
    u16::try_from(node_number)
        .map_err(|_| CliError::Message(format!("node number {node_number} is out of range")))
}

pub fn run_nodes(command: NodesCommand, ctx: &AppContext) -> CliResult<()> {
    let db = open_database(&ctx.config)?;
    match command {
        NodesCommand::List => print_nodes(&db, ctx),
        NodesCommand::Watch { interval } => loop {
            print_nodes(&db, ctx)?;
            thread::sleep(Duration::from_secs(interval));
        },
        NodesCommand::Show { node_number } => {
            if let Some(nodes) = request_live_nodes(ctx)? {
                show_node_live(&nodes, node_number, ctx.json)?;
                return Ok(());
            }

            let session = find_active_session_by_node(db.db(), node_number)?;
            if ctx.json {
                print_json(&json!({
                    "node": node_number,
                    "state": if session.is_some() { "active" } else { "available" },
                    "session": session.as_ref().map(session_json)
                }))?;
            } else if let Some(session) = session {
                println!("node {node_number}: active");
                print_session(&session);
            } else {
                println!("node {node_number}: available");
            }
            Ok(())
        }
        NodesCommand::Disconnect { node_number } => {
            let node = request_u16_node(node_number)?;
            match request_nodes_disconnect(
                &ctx.config.paths.runtime,
                node,
                "sysop_disconnect".to_string(),
            ) {
                Ok(ControlResponse::Ok { .. }) => {
                    emit_ok(
                        ctx.json,
                        "node disconnect sent to running server",
                        json!({ "node": node_number }),
                    )?;
                }
                Ok(ControlResponse::Error { error, .. }) => {
                    return Err(CliError::Message(format!(
                        "node disconnect failed: {error}"
                    )));
                }
                Ok(ControlResponse::Status { .. }) | Ok(ControlResponse::Nodes { .. }) => {
                    return Err(CliError::Message(
                        "control socket returned unexpected response for node disconnect"
                            .to_string(),
                    ));
                }
                Err(error) if error.is_unreachable() => {
                    let session = require_active_session(&db, node_number)?;
                    end_session(
                        db.db(),
                        &session.id,
                        &current_timestamp(&db)?,
                        "sysop_disconnect",
                    )?;
                    audit(
                        &db,
                        "node_disconnect_requested",
                        session.user_id.as_deref(),
                        Some(node_number),
                        "sysop marked active session disconnected; live control socket unreachable",
                    )?;
                    emit_ok(
                        ctx.json,
                        "node session marked disconnected (live server not reachable)",
                        json!({"node": node_number}),
                    )?;
                }
                Err(error) => {
                    return Err(CliError::Message(format!(
                        "node disconnect failed: {error}"
                    )));
                }
            }
            Ok(())
        }
        NodesCommand::Message { node_number, text } => {
            let node = request_u16_node(node_number)?;
            match request_nodes_message(&ctx.config.paths.runtime, node, text.clone()) {
                Ok(ControlResponse::Ok { .. }) => {
                    emit_ok(
                        ctx.json,
                        "node message sent through live control socket",
                        json!({"node": node_number}),
                    )?;
                }
                Ok(ControlResponse::Error { error, .. }) => {
                    return Err(CliError::Message(format!("node message failed: {error}")));
                }
                Ok(ControlResponse::Status { .. }) | Ok(ControlResponse::Nodes { .. }) => {
                    return Err(CliError::Message(
                        "control socket returned unexpected response for node message".to_string(),
                    ));
                }
                Err(error) if error.is_unreachable() => {
                    let session = require_active_session(&db, node_number)?;
                    audit(
                        &db,
                        "node_message_requested",
                        session.user_id.as_deref(),
                        Some(node_number),
                        &text,
                    )?;
                    emit_ok(
                        ctx.json,
                        "node message recorded for delivery; live server not reachable",
                        json!({"node": node_number, "text": text}),
                    )?;
                }
                Err(error) => {
                    return Err(CliError::Message(format!("node message failed: {error}")));
                }
            }
            Ok(())
        }
        NodesCommand::Broadcast { text } => {
            match request_nodes_broadcast(&ctx.config.paths.runtime, text.clone()) {
                Ok(ControlResponse::Ok { .. }) => {
                    emit_ok(
                        ctx.json,
                        "broadcast sent through live control socket",
                        json!({"text": text}),
                    )?;
                }
                Ok(ControlResponse::Error { error, .. }) => {
                    return Err(CliError::Message(format!("broadcast failed: {error}")));
                }
                Ok(ControlResponse::Status { .. }) | Ok(ControlResponse::Nodes { .. }) => {
                    return Err(CliError::Message(
                        "control socket returned unexpected response for broadcast".to_string(),
                    ));
                }
                Err(error) if error.is_unreachable() => {
                    audit(&db, "node_broadcast_requested", None, None, &text)?;
                    emit_ok(
                        ctx.json,
                        "broadcast recorded; live server not reachable",
                        json!({"text": text}),
                    )?;
                }
                Err(error) => {
                    return Err(CliError::Message(format!("broadcast failed: {error}")));
                }
            }
            Ok(())
        }
        NodesCommand::Disable { node_number } => {
            audit(&db, "node_disable_requested", None, Some(node_number), "")?;
            emit_ok(
                ctx.json,
                "node disable recorded; persistent node state is not yet modeled",
                json!({"node": node_number}),
            )?;
            Ok(())
        }
        NodesCommand::Enable { node_number } => {
            audit(&db, "node_enable_requested", None, Some(node_number), "")?;
            emit_ok(
                ctx.json,
                "node enable recorded; persistent node state is not yet modeled",
                json!({"node": node_number}),
            )?;
            Ok(())
        }
        NodesCommand::ResetStale => {
            audit(&db, "node_reset_stale_requested", None, None, "")?;
            emit_ok(
                ctx.json,
                "stale-node reset recorded; stale detection requires runtime heartbeats",
                json!({}),
            )
        }
    }
}
