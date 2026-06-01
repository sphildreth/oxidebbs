use clap::Subcommand;

use crate::sysop_cli::{AppContext, CliResult, open_database, print_audit_events, require_user};
use oxidebbs_db::{list_audit_events, list_audit_events_for_user};

#[derive(Subcommand)]
pub enum AuditCommand {
    Recent {
        #[arg(short, long, default_value_t = 25)]
        limit: i64,
    },
    User {
        alias_or_id: String,
        #[arg(short, long, default_value_t = 25)]
        limit: i64,
    },
    Node {
        node_number: i64,
        #[arg(short, long, default_value_t = 25)]
        limit: i64,
    },
    Door {
        door_key: String,
        #[arg(short, long, default_value_t = 25)]
        limit: i64,
    },
}

pub fn run_audit(command: AuditCommand, ctx: &AppContext) -> CliResult<()> {
    let db = open_database(&ctx.config)?;
    match command {
        AuditCommand::Recent { limit } => {
            let events = list_audit_events(db.db(), limit)?;
            print_audit_events(&events, ctx.json)
        }
        AuditCommand::User { alias_or_id, limit } => {
            let user = require_user(&db, &alias_or_id)?;
            let events = list_audit_events_for_user(db.db(), &user.id, limit)?;
            print_audit_events(&events, ctx.json)
        }
        AuditCommand::Node { node_number, limit } => {
            let events: Vec<_> = list_audit_events(db.db(), limit)?
                .into_iter()
                .filter(|event| event.node_number == Some(node_number))
                .collect();
            print_audit_events(&events, ctx.json)
        }
        AuditCommand::Door { door_key, limit } => {
            let needle = door_key.to_ascii_lowercase();
            let events: Vec<_> = list_audit_events(db.db(), limit)?
                .into_iter()
                .filter(|event| event.details.to_ascii_lowercase().contains(&needle))
                .collect();
            print_audit_events(&events, ctx.json)
        }
    }
}
