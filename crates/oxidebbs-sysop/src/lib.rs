//! Local sysop/admin tooling.

use oxidebbs_db::{
    AuditEventRecord, Db, SessionRecord, UserRecord, list_active_sessions, list_audit_events,
    list_users as db_list_users, update_user_password_hash,
};
use oxidebbs_door::{DoorError, parse_doors_toml};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::{Line, Text};
use ratatui::widgets::{Block, Borders, Paragraph, Widget};
use thiserror::Error;

pub mod app;
pub mod command_palette;
pub mod events;
pub mod input;
pub mod screens;
pub mod services;
pub mod theme;
pub mod widgets;

// Re-export the main entry point
pub use app::{AppConfig, run_tui};

pub const CRATE_NAME: &str = "oxidebbs-sysop";

#[derive(Debug, Error)]
pub enum SysopError {
    #[error("database error: {0}")]
    Database(#[from] oxidebbs_db::DbError),

    #[error("door config error: {0}")]
    DoorConfig(#[source] Box<DoorError>),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("control socket error: {0}")]
    Control(String),

    #[error("{0}")]
    Message(String),
}

impl From<DoorError> for SysopError {
    fn from(error: DoorError) -> Self {
        Self::DoorConfig(Box::new(error))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdminCommand {
    ListUsers,
    ResetPassword { user_id: String },
    ListNodes,
    ShowRecentCalls { limit: i64 },
    TestDoorConfig,
    PrototypeConsole,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DoorConfigCheck {
    pub definitions: usize,
    pub enabled: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SysopConsoleSnapshot {
    pub board_name: String,
    pub active_nodes: usize,
    pub recent_calls: Vec<String>,
}

pub fn list_users(db: &Db) -> Result<Vec<UserRecord>, SysopError> {
    Ok(db_list_users(db)?)
}

pub fn reset_password(db: &Db, user_id: &str, password_hash: &str) -> Result<(), SysopError> {
    update_user_password_hash(db, user_id, password_hash)?;
    Ok(())
}

pub fn list_nodes(db: &Db) -> Result<Vec<SessionRecord>, SysopError> {
    Ok(list_active_sessions(db)?)
}

pub fn show_recent_calls(db: &Db, limit: i64) -> Result<Vec<AuditEventRecord>, SysopError> {
    Ok(list_audit_events(db, limit)?)
}

pub fn test_door_config(contents: &str) -> Result<DoorConfigCheck, SysopError> {
    let definitions = parse_doors_toml(contents)?;
    Ok(DoorConfigCheck {
        enabled: definitions.iter().filter(|door| door.enabled).count(),
        definitions: definitions.len(),
    })
}

pub fn render_sysop_console(area: Rect, buffer: &mut Buffer, snapshot: &SysopConsoleSnapshot) {
    let mut lines = vec![
        Line::from(format!("Board: {}", snapshot.board_name)),
        Line::from(format!("Active nodes: {}", snapshot.active_nodes)),
        Line::from("Recent calls:"),
    ];
    lines.extend(
        snapshot
            .recent_calls
            .iter()
            .map(|call| Line::from(call.as_str())),
    );

    Paragraph::new(Text::from(lines))
        .block(Block::new().title("OxideBBS Sysop").borders(Borders::ALL))
        .render(area, buffer);
}

pub fn render_sysop_console_text(
    snapshot: &SysopConsoleSnapshot,
    width: u16,
    height: u16,
) -> String {
    let area = Rect::new(0, 0, width, height);
    let mut buffer = Buffer::empty(area);
    render_sysop_console(area, &mut buffer, snapshot);
    buffer_to_string(&buffer, width, height)
}

fn buffer_to_string(buffer: &Buffer, width: u16, height: u16) -> String {
    let mut output = String::new();
    for row in 0..height {
        for column in 0..width {
            output.push_str(buffer[(column, row)].symbol());
        }
        if row + 1 < height {
            output.push('\n');
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxidebbs_db::{
        AuditEventRecord, OxideDb, SessionRecord, UserRecord, find_user_by_id, insert_audit_event,
        insert_session, insert_user,
    };

    const USER_ALICE: &str = "00000000-0000-4000-8000-000000000701";
    const SESSION_1: &str = "00000000-0000-4000-8000-000000000702";
    const EVENT_1: &str = "00000000-0000-4000-8000-000000000703";

    fn sample_user(alias: &str) -> UserRecord {
        UserRecord {
            id: USER_ALICE.to_string(),
            alias: alias.to_string(),
            real_name: format!("{alias} User"),
            email: None,
            password_hash: "old".to_string(),
            security_level: 10,
            is_sysop: false,
            created_at: "2026-01-01T00:00:00.000000Z".to_string(),
            last_login_at: None,
            total_calls: 0,
            time_bank_minutes: 0,
            status: "active".to_string(),
        }
    }

    #[test]
    fn admin_list_users_returns_repository_users() {
        let db = OxideDb::open_memory().expect("open db");
        insert_user(db.db(), &sample_user("alice")).expect("insert user");

        let users = list_users(db.db()).expect("list users");

        assert_eq!(users.len(), 1);
        assert_eq!(users[0].alias, "alice");
    }

    #[test]
    fn reset_password_replaces_hash() {
        let db = OxideDb::open_memory().expect("open db");
        insert_user(db.db(), &sample_user("alice")).expect("insert user");

        reset_password(db.db(), USER_ALICE, "new-hash").expect("reset");

        let user = find_user_by_id(db.db(), USER_ALICE).expect("find").unwrap();
        assert_eq!(user.password_hash, "new-hash");
    }

    #[test]
    fn list_nodes_returns_active_sessions() {
        let db = OxideDb::open_memory().expect("open db");
        insert_user(db.db(), &sample_user("alice")).expect("insert user");
        insert_session(
            db.db(),
            &SessionRecord {
                id: SESSION_1.to_string(),
                node_number: 1,
                user_id: Some(USER_ALICE.to_string()),
                transport: "telnet".to_string(),
                remote_address: "127.0.0.1:2323".to_string(),
                remote_ip: Some("127.0.0.1".to_string()),
                remote_port: Some(2323),
                started_at: "2026-01-01T00:00:00.000000Z".to_string(),
                ended_at: None,
                disconnect_reason: None,
            },
        )
        .expect("insert session");

        let nodes = list_nodes(db.db()).expect("list nodes");

        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].node_number, 1);
    }

    #[test]
    fn recent_calls_reads_audit_events() {
        let db = OxideDb::open_memory().expect("open db");
        insert_user(db.db(), &sample_user("alice")).expect("insert user");
        insert_audit_event(
            db.db(),
            &AuditEventRecord {
                id: EVENT_1.to_string(),
                created_at: "2026-01-01T00:00:00.000000Z".to_string(),
                event_type: "login_success".to_string(),
                user_id: Some(USER_ALICE.to_string()),
                node_number: Some(1),
                details: "alice logged in".to_string(),
            },
        )
        .expect("insert event");

        let calls = show_recent_calls(db.db(), 5).expect("recent calls");

        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].event_type, "login_success");
    }

    #[test]
    fn test_door_config_counts_enabled_doors() {
        let check = test_door_config(
            r#"
[[definitions]]
key = "lord"
name = "Legend of the Red Dragon"
working_dir = "./doors/lord"
command = "LORD.EXE"

[[definitions]]
key = "disabled"
name = "Disabled"
working_dir = "./doors/disabled"
command = "DISABLED.EXE"
enabled = false
"#,
        )
        .expect("check doors");

        assert_eq!(
            check,
            DoorConfigCheck {
                definitions: 2,
                enabled: 1
            }
        );
    }

    #[test]
    fn ratatui_console_renders_status_text() {
        let area = Rect::new(0, 0, 40, 8);
        let mut buffer = Buffer::empty(area);
        let snapshot = SysopConsoleSnapshot {
            board_name: "Oxide".to_string(),
            active_nodes: 2,
            recent_calls: vec!["alice login_success".to_string()],
        };

        render_sysop_console(area, &mut buffer, &snapshot);

        let rendered = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        assert!(rendered.contains("OxideBBS Sysop"));
        assert!(rendered.contains("Active nodes: 2"));
    }

    #[test]
    fn ratatui_console_can_render_to_text_preview() {
        let snapshot = SysopConsoleSnapshot {
            board_name: "Oxide".to_string(),
            active_nodes: 1,
            recent_calls: vec!["node 1 alice".to_string()],
        };

        let rendered = render_sysop_console_text(&snapshot, 40, 8);

        assert!(rendered.contains("OxideBBS Sysop"));
        assert!(rendered.contains("node 1 alice"));
    }
}
