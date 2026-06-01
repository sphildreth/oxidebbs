use std::collections::HashMap;
use std::thread;
use std::time::Duration;

use clap::Subcommand;
use serde_json::Value as JsonValue;
use serde_json::json;

use crate::{
    control::{
        ControlNodeStatus, ControlResponse, request_nodes, request_nodes_broadcast,
        request_nodes_disconnect, request_nodes_message, request_nodes_reset_stale,
    },
    sysop_cli::{
        AppContext, CliError, CliResult, audit, current_timestamp, emit_ok, open_database,
        print_json, print_session, require_active_session, session_json,
    },
};
use oxidebbs_db::{
    SessionRecord, end_session, find_active_session_by_node, find_user_by_id, list_active_sessions,
};

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
                nodes_json_from_db_session(db, node_number, session)
            })
            .collect::<Vec<_>>();
        print_json(&nodes_json_payload(nodes))?;
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
    nodes_json_payload(
        nodes
            .iter()
            .map(control_node_status_json)
            .collect::<Vec<_>>(),
    )
}

fn nodes_json_payload(nodes: Vec<JsonValue>) -> JsonValue {
    json!({ "nodes": nodes })
}

fn control_node_status_json(node: &ControlNodeStatus) -> JsonValue {
    json!({
        "node_number": node.node_number,
        "state": node.state,
        "user_alias": node.user_alias,
        "session": JsonValue::Null,
        "last_heartbeat_at": node.last_heartbeat_at,
        "heartbeat_age_seconds": node.heartbeat_age_seconds,
    })
}

fn nodes_json_from_db_session(
    db: &oxidebbs_db::OxideDb,
    node_number: i64,
    session: Option<&SessionRecord>,
) -> JsonValue {
    let user_alias = session.and_then(|session| {
        session.user_id.as_deref().and_then(|user_id| {
            find_user_by_id(db.db(), user_id)
                .ok()
                .flatten()
                .map(|user| user.alias)
        })
    });

    json!({
        "node_number": node_number,
        "state": if session.is_some() { "offline" } else { "available" },
        "user_alias": user_alias,
        "session": session.map(session_json),
        "last_heartbeat_at": JsonValue::Null,
        "heartbeat_age_seconds": JsonValue::Null,
    })
}

fn print_nodes_from_control(nodes: &[ControlNodeStatus], json_output: bool) -> CliResult<()> {
    if json_output {
        print_json(&control_nodes_to_json(nodes))?;
        return Ok(());
    }

    for node in nodes {
        if node.user_alias.is_some() || node.remote_address.is_some() {
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
            "node_number": node.node_number,
            "state": node.state,
            "user_alias": node.user_alias,
            "remote_address": node.remote_address,
            "connected_at": node.connected_at,
            "last_heartbeat_at": node.last_heartbeat_at,
            "heartbeat_age_seconds": node.heartbeat_age_seconds,
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
    if let Some(age) = node.heartbeat_age_seconds {
        println!("heartbeat_age_seconds: {age}");
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
                    "node_number": node_number,
                    "state": if session.is_some() { "offline" } else { "available" },
                    "user_alias": session
                        .as_ref()
                        .and_then(|session| {
                            session.user_id.as_deref().and_then(|user_id| {
                                find_user_by_id(db.db(), user_id)
                                    .ok()
                                    .flatten()
                                    .map(|user| user.alias)
                            })
                        }),
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
            match request_nodes_reset_stale(&ctx.config.paths.runtime) {
                Ok(ControlResponse::Ok { .. }) => {
                    emit_ok(
                        ctx.json,
                        "stale-node reset sent through live control socket",
                        json!({}),
                    )?;
                }
                Ok(ControlResponse::Error { error, .. }) => {
                    return Err(CliError::Message(format!(
                        "stale-node reset failed: {error}"
                    )));
                }
                Ok(ControlResponse::Status { .. }) | Ok(ControlResponse::Nodes { .. }) => {
                    return Err(CliError::Message(
                        "control socket returned unexpected response for stale-node reset"
                            .to_string(),
                    ));
                }
                Err(error) if error.is_unreachable() => {
                    audit(&db, "node_reset_stale_requested", None, None, "")?;
                    emit_ok(
                        ctx.json,
                        "stale-node reset recorded; live server not reachable",
                        json!({}),
                    )?;
                }
                Err(error) => {
                    return Err(CliError::Message(format!(
                        "stale-node reset failed: {error}"
                    )));
                }
            }
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxidebbs_db::{SCHEMA_VERSION, SessionRecord, insert_session};

    fn session_from_timestamp(id: &str, node_number: i64, started_at: &str) -> SessionRecord {
        SessionRecord {
            id: id.to_string(),
            node_number,
            user_id: Some("00000000-0000-4000-8000-000000000001".to_string()),
            transport: "telnet".to_string(),
            remote_address: "127.0.0.1:2323".to_string(),
            remote_ip: Some("127.0.0.1".to_string()),
            remote_port: Some(2323),
            started_at: started_at.to_string(),
            ended_at: None,
            disconnect_reason: None,
        }
    }

    #[test]
    fn nodes_list_json_shape_matches_contract() {
        let db = oxidebbs_db::OxideDb::open_memory().expect("open in-memory db");
        assert_eq!(db.schema_version().expect("schema version"), SCHEMA_VERSION);

        let user = oxidebbs_db::UserRecord {
            id: "00000000-0000-4000-8000-000000000001".to_string(),
            alias: "sysop".to_string(),
            real_name: "Sysop".to_string(),
            email: None,
            password_hash: "hash".to_string(),
            security_level: 100,
            is_sysop: true,
            created_at: "2026-01-01T00:00:00.000000Z".to_string(),
            last_login_at: None,
            total_calls: 0,
            time_bank_minutes: 0,
            status: "active".to_string(),
        };
        oxidebbs_db::insert_user(db.db(), &user).expect("seed user");

        let session = session_from_timestamp(
            "00000000-0000-4000-8000-000000000010",
            1,
            "2026-01-01T00:00:00.000000Z",
        );
        insert_session(db.db(), &session).expect("seed session");

        let payload = nodes_json_payload(vec![
            nodes_json_from_db_session(&db, 1, Some(&session)),
            nodes_json_from_db_session(&db, 2, None),
        ]);

        let nodes = payload
            .as_object()
            .expect("payload object")
            .get("nodes")
            .expect("nodes key")
            .as_array()
            .expect("nodes array");
        assert_eq!(nodes.len(), 2);
        assert_eq!(
            nodes[0].as_object().expect("first node").get("node_number"),
            Some(&JsonValue::from(1))
        );
        assert_eq!(
            nodes[0].as_object().expect("first node").get("state"),
            Some(&JsonValue::String("offline".into()))
        );
        assert_eq!(
            nodes[1].as_object().expect("second node").get("state"),
            Some(&JsonValue::String("available".into()))
        );
        let first = nodes[0].as_object().expect("first node");
        assert!(
            first
                .get("session")
                .and_then(JsonValue::as_object)
                .is_some()
        );
        assert_eq!(
            first.get("user_alias"),
            Some(&JsonValue::String("sysop".to_string()))
        );
    }

    #[test]
    fn control_nodes_json_shape_matches_contract() {
        let payload = control_nodes_to_json(&[ControlNodeStatus {
            node_number: 1,
            state: "main_menu".to_string(),
            user_alias: Some("sysop".to_string()),
            remote_address: Some("127.0.0.1:2323".to_string()),
            connected_at: Some("2026-01-01T00:00:00.000000Z".to_string()),
            last_heartbeat_at: Some("2026-01-01T00:00:05.000000Z".to_string()),
            heartbeat_age_seconds: Some(2),
        }]);

        let nodes = payload["nodes"].as_array().expect("nodes array");
        let node = nodes[0].as_object().expect("node object");
        assert_eq!(node.get("node_number"), Some(&JsonValue::from(1)));
        assert_eq!(
            node.get("state"),
            Some(&JsonValue::String("main_menu".to_string()))
        );
        assert_eq!(node.get("session"), Some(&JsonValue::Null));
        assert_eq!(node.get("heartbeat_age_seconds"), Some(&JsonValue::from(2)));
    }
}
