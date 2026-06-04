use clap::{Args, Subcommand};
use serde_json::{Value as JsonValue, json};

use crate::sysop_cli::{
    AppContext, CliError, CliResult, audit, current_timestamp, emit_ok, generated_uuid,
    hash_password, open_database, print_audit_events, print_json, print_sessions, print_user,
    prompt_line, require_user, user_json,
};
use oxidebbs_db::{
    UserRecord, insert_user, list_audit_events_for_user, list_recent_sessions, list_users,
    update_user_alias, update_user_is_sysop, update_user_password_hash, update_user_security_level,
    update_user_status,
};

#[derive(Subcommand)]
pub enum UsersCommand {
    List,
    Show {
        alias_or_id: String,
    },
    Add(UserAddArgs),
    ResetPassword(ResetPasswordArgs),
    SetLevel {
        alias_or_id: String,
        level: i64,
    },
    Enable {
        alias_or_id: String,
    },
    Disable {
        alias_or_id: String,
    },
    PromoteSysop {
        alias_or_id: String,
    },
    DemoteSysop {
        alias_or_id: String,
    },
    Rename {
        old_alias: String,
        new_alias: String,
    },
    Audit {
        alias_or_id: String,
        #[arg(short, long, default_value_t = 25)]
        limit: i64,
    },
    Sessions {
        alias_or_id: String,
        #[arg(short, long, default_value_t = 25)]
        limit: i64,
    },
    Delete {
        alias_or_id: String,
        #[arg(long)]
        reason: Option<String>,
    },
}

#[derive(Debug, Clone, Args)]
pub struct UserAddArgs {
    #[arg(long)]
    pub alias: Option<String>,
    #[arg(long)]
    pub real_name: Option<String>,
    #[arg(long)]
    pub email: Option<String>,
    #[arg(long)]
    pub password: Option<String>,
    #[arg(long, default_value_t = 10)]
    pub level: i64,
    #[arg(long)]
    pub sysop: bool,
}

#[derive(Debug, Clone, Args)]
pub struct ResetPasswordArgs {
    pub alias_or_id: String,
    /// New plaintext password to hash with Argon2id
    #[arg(long, conflicts_with = "password_hash")]
    pub password: Option<String>,
    /// Precomputed password hash for recovery/import workflows
    #[arg(long)]
    pub password_hash: Option<String>,
}

pub fn run_users(command: UsersCommand, ctx: &AppContext) -> CliResult<()> {
    let db = open_database(&ctx.config)?;
    match command {
        UsersCommand::List => {
            let users = list_users(db.db())?;
            if ctx.json {
                print_json(&users_json_payload(&users))?;
            } else {
                for user in users {
                    println!(
                        "{}\t{}\tlevel={}\tsysop={}\tstatus={}",
                        user.id, user.alias, user.security_level, user.is_sysop, user.status
                    );
                }
            }
        }
        UsersCommand::Show { alias_or_id } => {
            let user = require_user(&db, &alias_or_id)?;
            if ctx.json {
                print_json(&user_json(&user))?;
            } else {
                print_user(&user);
            }
        }
        UsersCommand::Add(args) => add_user(args, &db, ctx.json)?,
        UsersCommand::ResetPassword(args) => {
            let user = require_user(&db, &args.alias_or_id)?;
            let hash = match (args.password, args.password_hash) {
                (Some(password), None) => hash_password(&password)?,
                (None, Some(password_hash)) => password_hash,
                (None, None) => hash_password(&prompt_line("New password", None)?)?,
                (Some(_), Some(_)) => {
                    return Err(CliError::Message(
                        "--password and --password-hash are mutually exclusive".to_string(),
                    ));
                }
            };
            update_user_password_hash(db.db(), &user.id, &hash)?;
            audit(
                &db,
                "user:password-reset",
                Some(&user.id),
                None,
                &format!("password reset for user {} ({})", user.alias, user.id),
            )?;
            emit_ok(ctx.json, "password updated", json!({"user": user.alias}))?;
        }
        UsersCommand::SetLevel { alias_or_id, level } => {
            let user = require_user(&db, &alias_or_id)?;
            update_user_security_level(db.db(), &user.id, level)?;
            audit(
                &db,
                "user:security-level",
                Some(&user.id),
                None,
                &format!(
                    "user {} ({}) security level changed from {} to {}",
                    user.alias, user.id, user.security_level, level
                ),
            )?;
            emit_ok(
                ctx.json,
                "security level updated",
                json!({"user": user.alias, "level": level}),
            )?;
        }
        UsersCommand::Enable { alias_or_id } => {
            let user = require_user(&db, &alias_or_id)?;
            update_user_status(db.db(), &user.id, "active")?;
            audit(
                &db,
                "user:enable",
                Some(&user.id),
                None,
                &format!(
                    "user {} ({}) status changed from {} to active",
                    user.alias, user.id, user.status
                ),
            )?;
            emit_ok(ctx.json, "user enabled", json!({"user": user.alias}))?;
        }
        UsersCommand::Disable { alias_or_id } => {
            let user = require_user(&db, &alias_or_id)?;
            update_user_status(db.db(), &user.id, "disabled")?;
            audit(
                &db,
                "user:disable",
                Some(&user.id),
                None,
                &format!(
                    "user {} ({}) status changed from {} to disabled",
                    user.alias, user.id, user.status
                ),
            )?;
            emit_ok(
                ctx.json,
                "user disabled",
                json!({"user": user.alias, "status": "disabled"}),
            )?;
        }
        UsersCommand::Delete {
            alias_or_id,
            reason,
        } => {
            let reason_text = reason.ok_or_else(|| {
                CliError::Message("--reason is required for users delete".to_string())
            })?;
            let user = require_user(&db, &alias_or_id)?;
            update_user_status(db.db(), &user.id, "disabled")?;
            audit(
                &db,
                "user:delete",
                Some(&user.id),
                None,
                &format!(
                    "user {} ({}) disabled; reason: {}",
                    user.alias, user.id, reason_text
                ),
            )?;
            emit_ok(
                ctx.json,
                "user disabled; delete is implemented as a safe disable",
                json!({"user": user.alias, "status": "disabled", "reason": reason_text}),
            )?;
        }
        UsersCommand::PromoteSysop { alias_or_id } => {
            let user = require_user(&db, &alias_or_id)?;
            update_user_is_sysop(db.db(), &user.id, true)?;
            update_user_security_level(db.db(), &user.id, 255)?;
            audit(
                &db,
                "user:promote-sysop",
                Some(&user.id),
                None,
                &format!(
                    "user {} ({}) promoted to sysop; previous sysop={} level={}",
                    user.alias, user.id, user.is_sysop, user.security_level
                ),
            )?;
            emit_ok(
                ctx.json,
                "user promoted to sysop",
                json!({"user": user.alias}),
            )?;
        }
        UsersCommand::DemoteSysop { alias_or_id } => {
            let user = require_user(&db, &alias_or_id)?;
            update_user_is_sysop(db.db(), &user.id, false)?;
            audit(
                &db,
                "user:demote-sysop",
                Some(&user.id),
                None,
                &format!("user {} ({}) demoted from sysop", user.alias, user.id),
            )?;
            emit_ok(
                ctx.json,
                "user demoted from sysop",
                json!({"user": user.alias}),
            )?;
        }
        UsersCommand::Rename {
            old_alias,
            new_alias,
        } => {
            let user = require_user(&db, &old_alias)?;
            update_user_alias(db.db(), &user.id, &new_alias)?;
            audit(
                &db,
                "user:rename",
                Some(&user.id),
                None,
                &format!(
                    "user {} ({}) renamed from {} to {}",
                    user.alias, user.id, old_alias, new_alias
                ),
            )?;
            emit_ok(
                ctx.json,
                "user renamed",
                json!({"old_alias": old_alias, "new_alias": new_alias}),
            )?;
        }
        UsersCommand::Audit { alias_or_id, limit } => {
            let user = require_user(&db, &alias_or_id)?;
            let events = list_audit_events_for_user(db.db(), &user.id, limit)?;
            print_audit_events(&events, ctx.json)?;
        }
        UsersCommand::Sessions { alias_or_id, limit } => {
            let user = require_user(&db, &alias_or_id)?;
            let sessions: Vec<_> = list_recent_sessions(db.db(), limit)?
                .into_iter()
                .filter(|session| session.user_id.as_deref() == Some(user.id.as_str()))
                .collect();
            print_sessions(&sessions, ctx.json)?;
        }
    }
    Ok(())
}

fn users_json_payload(users: &[UserRecord]) -> JsonValue {
    json!({
        "users": users.iter().map(user_json).collect::<Vec<_>>()
    })
}

fn add_user(args: UserAddArgs, db: &oxidebbs_db::OxideDb, json_output: bool) -> CliResult<()> {
    let alias = match args.alias {
        Some(value) => value,
        None => prompt_line("Alias", None)?,
    };
    let real_name = match args.real_name {
        Some(value) => value,
        None => prompt_line("Real name", Some(&alias))?,
    };
    let password = match args.password {
        Some(value) => value,
        None => prompt_line("Password", None)?,
    };
    let now = current_timestamp(db)?;
    let user = UserRecord {
        id: generated_uuid(db)?,
        alias,
        real_name,
        email: args.email.filter(|value| !value.trim().is_empty()),
        password_hash: hash_password(&password)?,
        security_level: args.level,
        is_sysop: args.sysop,
        created_at: now,
        last_login_at: None,
        total_calls: 0,
        time_bank_minutes: 0,
        status: "active".to_string(),
    };
    insert_user(db.db(), &user)?;
    audit(
        db,
        "user:add",
        Some(&user.id),
        None,
        &format!(
            "user {} ({}) added with security level {} sysop={}",
            user.alias, user.id, user.security_level, user.is_sysop
        ),
    )?;
    if json_output {
        print_json(&user_json(&user))?;
    } else {
        println!("user added: {}", user.alias);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sysop_cli::current_timestamp;
    use oxidebbs_db::{OxideDb, SCHEMA_VERSION};

    #[test]
    fn users_list_json_shape_matches_contract() {
        let db = OxideDb::open_memory().expect("open in-memory db");
        assert_eq!(db.schema_version().expect("schema version"), SCHEMA_VERSION);
        let now = current_timestamp(&db).expect("timestamp");
        let users = vec![UserRecord {
            id: "00000000-0000-4000-8000-000000000001".to_string(),
            alias: "sysop".to_string(),
            real_name: "System Operator".to_string(),
            email: None,
            password_hash: "hash".to_string(),
            security_level: 100,
            is_sysop: true,
            created_at: now,
            last_login_at: None,
            total_calls: 0,
            time_bank_minutes: 0,
            status: "active".to_string(),
        }];

        let payload = users_json_payload(&users);
        let users = payload
            .as_object()
            .expect("users payload is object")
            .get("users")
            .expect("users key")
            .as_array()
            .expect("users as array");
        assert_eq!(users.len(), 1);
        let user = users[0].as_object().expect("user object");
        assert_eq!(user.get("alias"), Some(&JsonValue::String("sysop".into())));
        assert_eq!(user.get("security_level"), Some(&JsonValue::from(100)));
        assert_eq!(user.get("is_sysop"), Some(&JsonValue::Bool(true)));
        assert!(user.get("password_hash").is_none());
    }

    #[test]
    fn add_user_writes_audit_event_without_password_material() {
        let db = OxideDb::open_memory().expect("open in-memory db");
        let args = UserAddArgs {
            alias: Some("alice".to_string()),
            real_name: Some("Alice User".to_string()),
            email: None,
            password: Some("secret-password".to_string()),
            level: 42,
            sysop: false,
        };

        add_user(args, &db, false).expect("add user");
        let user = require_user(&db, "alice").expect("find user");
        let events = list_audit_events_for_user(db.db(), &user.id, 10).expect("audit events");

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type, "user:add");
        assert!(events[0].details.contains("alice"));
        assert!(!events[0].details.contains("secret-password"));
        assert!(!events[0].details.contains(&user.password_hash));
    }
}
