use clap::{Args, Subcommand};
use serde_json::Value as JsonValue;

use oxidebbs_db::{
    MessageAreaRecord, list_messages, list_messages_in_area, move_message_to_area,
    update_message_area_enabled, update_message_area_levels, update_message_visibility,
};
use serde_json::json;

use crate::sysop_cli::{
    AppContext, CliResult, area_json, emit_ok, generated_uuid, message_json, open_database,
    print_json, print_message, print_messages, require_message, require_message_area,
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
            emit_ok(
                ctx.json,
                "message unlocked",
                json!({"message_id": message.id}),
            )?;
            Ok(())
        }
        MessagesCommand::Search { query } => {
            let needle = query.to_ascii_lowercase();
            let matches: Vec<_> = list_messages(db.db())?
                .into_iter()
                .filter(|message| {
                    message.subject.to_ascii_lowercase().contains(&needle)
                        || message.body.to_ascii_lowercase().contains(&needle)
                })
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
                print_json(&JsonValue::Array(areas.iter().map(area_json).collect()))?;
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
            emit_ok(
                ctx.json,
                "message area enabled",
                json!({"area": area.key, "enabled": true}),
            )?;
        }
        MessageAreasCommand::Disable { key } => {
            let area = require_message_area(db, &key)?;
            update_message_area_enabled(db.db(), &area.id, false)?;
            emit_ok(
                ctx.json,
                "message area disabled",
                json!({"area": area.key, "enabled": false}),
            )?;
        }
        MessageAreasCommand::SetLevel { key, read, post } => {
            let area = require_message_area(db, &key)?;
            update_message_area_levels(db.db(), &area.id, read, post)?;
            emit_ok(
                ctx.json,
                "message area levels updated",
                json!({"area": area.key, "read": read, "post": post}),
            )?;
        }
    }
    Ok(())
}
