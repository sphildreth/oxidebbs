use std::collections::HashMap;

use clap::{Args, Subcommand};
use serde_json::Value as JsonValue;

use oxidebbs_db::{
    MessageAreaRecord, MessageRecord, list_messages, list_messages_in_area, move_message_to_area,
    update_message_area_enabled, update_message_area_levels, update_message_visibility,
};
use serde_json::json;

use crate::sysop_cli::{
    AppContext, CliError, CliResult, area_json, audit, emit_ok, generated_uuid, message_json,
    open_database, print_json, print_message, print_messages, require_message,
    require_message_area,
};

#[derive(Subcommand)]
pub enum MessagesCommand {
    Areas {
        #[command(subcommand)]
        command: MessageAreasCommand,
    },
    List {
        #[arg(long)]
        area: String,
    },
    Show {
        message_id: String,
    },
    Delete {
        message_id: String,
    },
    Move {
        message_id: String,
        #[arg(long = "to-area")]
        to_area: String,
    },
    Lock {
        message_id: String,
    },
    Unlock {
        message_id: String,
    },
    Search {
        query: String,
        #[arg(long)]
        area: Option<String>,
        #[arg(long)]
        user: Option<String>,
        #[arg(long)]
        network: Option<String>,
        #[arg(long, default_value_t = 50)]
        limit: usize,
    },
}

#[derive(Subcommand)]
pub enum MessageAreasCommand {
    List,
    Add(MessageAreaAddArgs),
    Show {
        key: String,
    },
    Enable {
        key: String,
    },
    Disable {
        key: String,
    },
    SetLevel {
        key: String,
        #[arg(long)]
        read: i64,
        #[arg(long)]
        post: i64,
    },
}

#[derive(Args, Debug, Clone)]
pub struct MessageAreaAddArgs {
    pub key: String,
    #[arg(long)]
    pub name: String,
    #[arg(long, default_value = "")]
    pub description: String,
    #[arg(long, default_value_t = 0)]
    pub read_level: i64,
    #[arg(long, default_value_t = 10)]
    pub post_level: i64,
}

pub fn run_messages(command: MessagesCommand, ctx: &AppContext) -> CliResult<()> {
    let db = open_database(&ctx.config)?;
    match command {
        MessagesCommand::Areas { command } => run_message_areas(command, ctx, &db),
        MessagesCommand::List { area } => {
            let area = require_message_area(&db, &area)?;
            let messages = list_messages_in_area(db.db(), &area.id)?;
            print_messages(&messages, ctx.json)
        }
        MessagesCommand::Show { message_id } => {
            let message = require_message(&db, &message_id)?;
            if ctx.json {
                print_json(&message_json(&message))?;
            } else {
                print_message(&message);
            }
            Ok(())
        }
        MessagesCommand::Delete { message_id } => {
            let message = require_message(&db, &message_id)?;
            update_message_visibility(db.db(), &message.id, "deleted")?;
            audit(
                &db,
                "message:delete",
                None,
                None,
                &format!("message {} visibility changed to deleted", message.id),
            )?;
            emit_ok(
                ctx.json,
                "message deleted",
                json!({"message_id": message.id}),
            )?;
            Ok(())
        }
        MessagesCommand::Move {
            message_id,
            to_area,
        } => {
            let message = require_message(&db, &message_id)?;
            let area = require_message_area(&db, &to_area)?;
            move_message_to_area(db.db(), &message.id, &area.id)?;
            audit(
                &db,
                "message:move",
                None,
                None,
                &format!(
                    "message {} moved from area {} to {} ({})",
                    message.id, message.area_id, area.key, area.id
                ),
            )?;
            emit_ok(
                ctx.json,
                "message moved",
                json!({"message_id": message.id, "area": area.key}),
            )?;
            Ok(())
        }
        MessagesCommand::Lock { message_id } => {
            let message = require_message(&db, &message_id)?;
            update_message_visibility(db.db(), &message.id, "hidden")?;
            audit(
                &db,
                "message:lock",
                None,
                None,
                &format!("message {} visibility changed to hidden", message.id),
            )?;
            emit_ok(
                ctx.json,
                "message locked",
                json!({"message_id": message.id}),
            )?;
            Ok(())
        }
        MessagesCommand::Unlock { message_id } => {
            let message = require_message(&db, &message_id)?;
            update_message_visibility(db.db(), &message.id, "normal")?;
            audit(
                &db,
                "message:unlock",
                None,
                None,
                &format!("message {} visibility changed to normal", message.id),
            )?;
            emit_ok(
                ctx.json,
                "message unlocked",
                json!({"message_id": message.id}),
            )?;
            Ok(())
        }
        MessagesCommand::Search {
            query,
            area,
            user,
            network,
            limit,
        } => {
            let needle = query.to_ascii_lowercase();
            let all_messages = list_messages(db.db())?;
            let all_areas = oxidebbs_db::list_message_areas(db.db())?;
            let areas_by_id: HashMap<_, _> = all_areas
                .iter()
                .map(|area| (area.id.as_str(), area))
                .collect();
            let area_record = area
                .as_ref()
                .map(|key| require_message_area(&db, key))
                .transpose()?;
            let user_record = user
                .as_ref()
                .map(|alias| {
                    oxidebbs_db::find_user_by_alias_ci(db.db(), alias)
                        .map_err(|e| CliError::Message(e.to_string()))
                        .and_then(|opt| {
                            opt.ok_or_else(|| {
                                CliError::Message(format!("user {alias:?} was not found"))
                            })
                        })
                })
                .transpose()?;

            let matches: Vec<_> = all_messages
                .into_iter()
                .filter(|message| {
                    message_matches_search(
                        message,
                        areas_by_id.get(message.area_id.as_str()).copied(),
                        &needle,
                    )
                })
                .filter(|message| {
                    if let Some(ref area) = area_record {
                        message.area_id == area.id
                    } else {
                        true
                    }
                })
                .filter(|message| {
                    if let Some(ref user) = user_record {
                        message.author_user_id == user.id
                    } else {
                        true
                    }
                })
                .filter(|message| {
                    if let Some(ref net_id) = network {
                        message_matches_network(
                            message,
                            areas_by_id.get(message.area_id.as_str()).copied(),
                            net_id,
                        )
                    } else {
                        true
                    }
                })
                .take(limit)
                .collect();
            print_messages(&matches, ctx.json)
        }
    }
}

fn run_message_areas(
    command: MessageAreasCommand,
    ctx: &AppContext,
    db: &oxidebbs_db::OxideDb,
) -> CliResult<()> {
    match command {
        MessageAreasCommand::List => {
            let areas = oxidebbs_db::list_message_areas(db.db())?;
            if ctx.json {
                print_json(&message_areas_json_payload(&areas))?;
            } else {
                for area in areas {
                    println!(
                        "{}\t{}\tread={}\tpost={}\tenabled={}",
                        area.key,
                        area.name,
                        area.read_security_level,
                        area.post_security_level,
                        area.enabled
                    );
                }
            }
        }
        MessageAreasCommand::Add(args) => {
            let area = MessageAreaRecord {
                id: generated_uuid(db)?,
                key: args.key,
                name: args.name,
                description: args.description,
                kind: "local".to_string(),
                network_id: None,
                read_security_level: args.read_level,
                post_security_level: args.post_level,
                moderated: false,
                enabled: true,
            };
            oxidebbs_db::insert_message_area(db.db(), &area)?;
            audit(
                db,
                "message-area:add",
                None,
                None,
                &format!("message area {} ({}) added", area.key, area.id),
            )?;
            emit_ok(ctx.json, "message area added", area_json(&area))?;
        }
        MessageAreasCommand::Show { key } => {
            let area = require_message_area(db, &key)?;
            if ctx.json {
                print_json(&area_json(&area))?;
            } else {
                println!("{} - {}", area.key, area.name);
                println!("id: {}", area.id);
                println!("description: {}", area.description);
                println!(
                    "levels: read={} post={}",
                    area.read_security_level, area.post_security_level
                );
                println!("enabled: {}", area.enabled);
            }
        }
        MessageAreasCommand::Enable { key } => {
            let area = require_message_area(db, &key)?;
            update_message_area_enabled(db.db(), &area.id, true)?;
            audit(
                db,
                "message-area:enable",
                None,
                None,
                &format!("message area {} ({}) enabled", area.key, area.id),
            )?;
            emit_ok(
                ctx.json,
                "message area enabled",
                json!({"area": area.key, "enabled": true}),
            )?;
        }
        MessageAreasCommand::Disable { key } => {
            let area = require_message_area(db, &key)?;
            update_message_area_enabled(db.db(), &area.id, false)?;
            audit(
                db,
                "message-area:disable",
                None,
                None,
                &format!("message area {} ({}) disabled", area.key, area.id),
            )?;
            emit_ok(
                ctx.json,
                "message area disabled",
                json!({"area": area.key, "enabled": false}),
            )?;
        }
        MessageAreasCommand::SetLevel { key, read, post } => {
            let area = require_message_area(db, &key)?;
            update_message_area_levels(db.db(), &area.id, read, post)?;
            audit(
                db,
                "message-area:set-level",
                None,
                None,
                &format!(
                    "message area {} ({}) levels changed from read={} post={} to read={} post={}",
                    area.key,
                    area.id,
                    area.read_security_level,
                    area.post_security_level,
                    read,
                    post
                ),
            )?;
            emit_ok(
                ctx.json,
                "message area levels updated",
                json!({"area": area.key, "read": read, "post": post}),
            )?;
        }
    }
    Ok(())
}

fn message_areas_json_payload(areas: &[MessageAreaRecord]) -> JsonValue {
    json!({
        "areas": areas.iter().map(area_json).collect::<Vec<_>>()
    })
}

fn message_matches_search(
    message: &MessageRecord,
    area: Option<&MessageAreaRecord>,
    needle: &str,
) -> bool {
    contains_search_text(&message.subject, needle)
        || contains_search_text(&message.body, needle)
        || contains_search_text(&message.author_display_name, needle)
        || message
            .author_network_address
            .as_deref()
            .is_some_and(|value| contains_search_text(value, needle))
        || message
            .network_message_id
            .as_deref()
            .is_some_and(|value| contains_search_text(value, needle))
        || area.is_some_and(|area| {
            contains_search_text(&area.key, needle)
                || area
                    .network_id
                    .as_deref()
                    .is_some_and(|value| contains_search_text(value, needle))
        })
}

fn message_matches_network(
    message: &MessageRecord,
    area: Option<&MessageAreaRecord>,
    network: &str,
) -> bool {
    equals_search_text(message.network_message_id.as_deref(), network)
        || equals_search_text(message.author_network_address.as_deref(), network)
        || area.is_some_and(|area| equals_search_text(area.network_id.as_deref(), network))
}

fn contains_search_text(value: &str, needle: &str) -> bool {
    value.to_ascii_lowercase().contains(needle)
}

fn equals_search_text(value: Option<&str>, expected: &str) -> bool {
    value.is_some_and(|value| value.eq_ignore_ascii_case(expected))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn search_area() -> MessageAreaRecord {
        MessageAreaRecord {
            id: "00000000-0000-4000-8000-000000000001".to_string(),
            key: "retro.echo".to_string(),
            name: "Retro Echo".to_string(),
            description: "Retro network messages".to_string(),
            kind: "echomail".to_string(),
            network_id: Some("fidonet".to_string()),
            read_security_level: 0,
            post_security_level: 10,
            moderated: false,
            enabled: true,
        }
    }

    fn search_message() -> MessageRecord {
        MessageRecord {
            id: "00000000-0000-4000-8000-000000000101".to_string(),
            area_id: "00000000-0000-4000-8000-000000000001".to_string(),
            author_user_id: "00000000-0000-4000-8000-000000000201".to_string(),
            author_kind: "network".to_string(),
            author_display_name: "Remote Sysop".to_string(),
            author_network_address: Some("1:105/42".to_string()),
            to_user_id: None,
            subject: "Packet status".to_string(),
            body: "Network body".to_string(),
            created_at: "2026-01-01T00:00:00.000000Z".to_string(),
            reply_to_id: None,
            network_message_id: Some("msgid-123".to_string()),
            visibility: "normal".to_string(),
        }
    }

    #[test]
    fn message_areas_list_json_shape_matches_contract() {
        let areas = vec![MessageAreaRecord {
            id: "00000000-0000-4000-8000-000000000001".to_string(),
            key: "general".to_string(),
            name: "General".to_string(),
            description: "General discussion".to_string(),
            kind: "local".to_string(),
            network_id: None,
            read_security_level: 0,
            post_security_level: 10,
            moderated: false,
            enabled: true,
        }];

        let payload = message_areas_json_payload(&areas);
        let areas = payload
            .as_object()
            .expect("payload object")
            .get("areas")
            .expect("areas key")
            .as_array()
            .expect("areas array");
        assert_eq!(areas.len(), 1);
        let area = areas[0].as_object().expect("single area");
        assert_eq!(area.get("key"), Some(&JsonValue::String("general".into())));
        assert_eq!(area.get("read_security_level"), Some(&JsonValue::from(0)));
        assert_eq!(area.get("enabled"), Some(&JsonValue::Bool(true)));
    }

    #[test]
    fn message_search_matches_area_key_and_network_metadata() {
        let area = search_area();
        let message = search_message();

        assert!(message_matches_search(&message, Some(&area), "retro"));
        assert!(message_matches_search(&message, Some(&area), "1:105/42"));
        assert!(message_matches_search(&message, Some(&area), "msgid-123"));
        assert!(message_matches_search(&message, Some(&area), "fidonet"));
        assert!(message_matches_network(&message, Some(&area), "fidonet"));
        assert!(message_matches_network(&message, Some(&area), "1:105/42"));
        assert!(message_matches_network(&message, Some(&area), "msgid-123"));
        assert!(!message_matches_search(&message, Some(&area), "missing"));
        assert!(!message_matches_network(&message, Some(&area), "othernet"));
    }
}
