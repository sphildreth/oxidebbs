use std::collections::HashMap;
use std::thread;
use std::time::Duration;

use clap::Subcommand;
use serde_json::Value as JsonValue;
use serde_json::json;

use crate::sysop_cli::{
    AppContext, CliResult, audit, current_timestamp, emit_ok, open_database, print_json,
    print_session, require_active_session, session_json,
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

pub fn run_nodes(command: NodesCommand, ctx: &AppContext) -> CliResult<()> {
    let db = open_database(&ctx.config)?;
    match command {
        NodesCommand::List => print_nodes(&db, ctx),
        NodesCommand::Watch { interval } => loop {
            print_nodes(&db, ctx)?;
            thread::sleep(Duration::from_secs(interval));
        },
        NodesCommand::Show { node_number } => {
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
                "sysop marked active session disconnected; live transport control requires a future control socket",
            )?;
            emit_ok(
                ctx.json,
                "node session marked disconnected",
                json!({"node": node_number}),
            )?;
            Ok(())
        }
        NodesCommand::Message { node_number, text } => {
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
                "node message recorded for delivery by a future live control channel",
                json!({"node": node_number, "text": text}),
            )?;
            Ok(())
        }
        NodesCommand::Broadcast { text } => {
            audit(&db, "node_broadcast_requested", None, None, &text)?;
            emit_ok(
                ctx.json,
                "broadcast recorded for delivery by a future live control channel",
                json!({"text": text}),
            )?;
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
            )?;
            Ok(())
        }
    }
}

fn print_nodes(db: &oxidebbs_db::OxideDb, ctx: &AppContext) -> CliResult<()> {
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
    } else {
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
    }
    Ok(())
}
