use decentdb::{Db, Value};

#[derive(Debug, Clone, PartialEq)]
pub struct AuditEventRecord {
    pub id: String,
    pub created_at: String,
    pub event_type: String,
    pub user_id: Option<String>,
    pub node_number: Option<i64>,
    pub details: String,
}

pub fn insert_audit_event(db: &Db, event: &AuditEventRecord) -> decentdb::Result<()> {
    db.execute_with_params(
        "INSERT INTO audit_events (id, created_at, event_type, user_id, node_number, details)
         VALUES (GEN_RANDOM_UUID(), CURRENT_TIMESTAMP, $1, UUID_PARSE($2), $3, $4)",
        &[
            Value::Text(event.event_type.clone()),
            event
                .user_id
                .as_ref()
                .map(|u| Value::Text(u.clone()))
                .unwrap_or(Value::Null),
            event.node_number.map(Value::Int64).unwrap_or(Value::Null),
            Value::Text(event.details.clone()),
        ],
    )?;
    Ok(())
}

pub fn insert_audit_event_preserving_record(
    db: &Db,
    event: &AuditEventRecord,
) -> decentdb::Result<()> {
    db.execute_with_params(
        "INSERT INTO audit_events (id, created_at, event_type, user_id, node_number, details)
         VALUES (UUID_PARSE($1), $2, $3, UUID_PARSE($4), $5, $6)",
        &[
            Value::Text(event.id.clone()),
            Value::Text(event.created_at.clone()),
            Value::Text(event.event_type.clone()),
            event
                .user_id
                .as_ref()
                .map(|u| Value::Text(u.clone()))
                .unwrap_or(Value::Null),
            event.node_number.map(Value::Int64).unwrap_or(Value::Null),
            Value::Text(event.details.clone()),
        ],
    )?;
    Ok(())
}

pub fn list_audit_events(db: &Db, limit: i64) -> decentdb::Result<Vec<AuditEventRecord>> {
    let result = db.execute_with_params(
        "SELECT UUID_TO_STRING(id), CAST(created_at AS TEXT), event_type, UUID_TO_STRING(user_id), node_number, details
         FROM audit_events ORDER BY created_at DESC LIMIT $1",
        &[Value::Int64(limit)],
    )?;
    Ok(result.rows().iter().map(row_to_audit_event).collect())
}

pub fn list_audit_events_for_user(
    db: &Db,
    user_id: &str,
    limit: i64,
) -> decentdb::Result<Vec<AuditEventRecord>> {
    let result = db.execute_with_params(
        "SELECT UUID_TO_STRING(id), CAST(created_at AS TEXT), event_type, UUID_TO_STRING(user_id), node_number, details
         FROM audit_events WHERE user_id = UUID_PARSE($1) ORDER BY created_at DESC LIMIT $2",
        &[Value::Text(user_id.to_string()), Value::Int64(limit)],
    )?;
    Ok(result.rows().iter().map(row_to_audit_event).collect())
}

pub fn purge_audit_events_older_than(db: &Db, cutoff_timestamp: &str) -> decentdb::Result<i64> {
    let before = audit_event_count(db)?;
    db.execute_with_params(
        "DELETE FROM audit_events WHERE created_at < CAST($1 AS TIMESTAMPTZ)",
        &[Value::Text(cutoff_timestamp.to_string())],
    )?;
    let after = audit_event_count(db)?;
    Ok(before.saturating_sub(after))
}

fn audit_event_count(db: &Db) -> decentdb::Result<i64> {
    let result = db.execute("SELECT COUNT(*) FROM audit_events")?;
    Ok(result
        .rows()
        .first()
        .and_then(|row| row.values().first())
        .and_then(|value| match value {
            Value::Int64(count) => Some(*count),
            _ => None,
        })
        .unwrap_or(0))
}

fn row_to_audit_event(row: &decentdb::QueryRow) -> AuditEventRecord {
    let values = row.values();
    AuditEventRecord {
        id: match &values[0] {
            Value::Text(s) => s.clone(),
            _ => String::new(),
        },
        created_at: match &values[1] {
            Value::Text(s) => s.clone(),
            _ => String::new(),
        },
        event_type: match &values[2] {
            Value::Text(s) => s.clone(),
            _ => String::new(),
        },
        user_id: match &values[3] {
            Value::Text(s) => Some(s.clone()),
            _ => None,
        },
        node_number: match &values[4] {
            Value::Int64(n) => Some(*n),
            _ => None,
        },
        details: match &values[5] {
            Value::Text(s) => s.clone(),
            _ => String::new(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema;
    use crate::user_repo::{UserRecord, insert_user};
    use decentdb::DbConfig;

    use std::sync::atomic::{AtomicU64, Ordering};

    static EVENT_SEQ: AtomicU64 = AtomicU64::new(0);
    const USER_1: &str = "00000000-0000-4000-8000-000000000001";
    const USER_2: &str = "00000000-0000-4000-8000-000000000002";

    fn test_db() -> Db {
        let db = Db::open_or_create(":memory:", DbConfig::default()).expect("open DecentDB");
        schema::init_schema(&db).expect("init schema");
        db
    }

    fn insert_test_user(db: &Db, id: &str, alias: &str) {
        insert_user(
            db,
            &UserRecord {
                id: id.to_string(),
                alias: alias.to_string(),
                real_name: format!("{alias} User"),
                email: None,
                password_hash: "hashed".to_string(),
                security_level: 10,
                is_sysop: false,
                created_at: "2026-01-01T00:00:00.000000Z".to_string(),
                last_login_at: None,
                total_calls: 0,
                time_bank_minutes: 0,
                status: "active".to_string(),
            },
        )
        .expect("insert test user");
    }

    fn sample_event(event_type: &str, user_id: Option<&str>) -> AuditEventRecord {
        let seq = EVENT_SEQ.fetch_add(1, Ordering::Relaxed);
        AuditEventRecord {
            id: format!("00000000-0000-4000-9000-{:012x}", seq + 1),
            created_at: "2026-01-01T00:00:00.000000Z".to_string(),
            event_type: event_type.to_string(),
            user_id: user_id.map(String::from),
            node_number: Some(1),
            details: format!("{event_type} happened"),
        }
    }

    #[test]
    fn insert_and_list_audit_events() {
        let db = test_db();
        insert_test_user(&db, USER_1, "alice");
        insert_audit_event(&db, &sample_event("login_success", Some(USER_1))).expect("insert");
        insert_audit_event(&db, &sample_event("caller_connected", None)).expect("insert");

        let events = list_audit_events(&db, 10).expect("list");
        assert_eq!(events.len(), 2);
        let types: Vec<&str> = events.iter().map(|e| e.event_type.as_str()).collect();
        assert!(types.contains(&"login_success"));
        assert!(types.contains(&"caller_connected"));
    }

    #[test]
    fn filters_audit_events_by_user() {
        let db = test_db();
        insert_test_user(&db, USER_1, "alice");
        insert_test_user(&db, USER_2, "bob");
        insert_audit_event(&db, &sample_event("login_success", Some(USER_1))).expect("insert");
        insert_audit_event(&db, &sample_event("login_success", Some(USER_2))).expect("insert");
        insert_audit_event(&db, &sample_event("login_failure", Some(USER_1))).expect("insert");

        let events = list_audit_events_for_user(&db, USER_1, 10).expect("list");
        assert_eq!(events.len(), 2);
        assert!(events.iter().all(|e| e.user_id.as_deref() == Some(USER_1)));
    }

    #[test]
    fn audit_events_respect_limit() {
        let db = test_db();
        for day in 1..=5 {
            let event = AuditEventRecord {
                id: format!("00000000-0000-4000-9000-{:012x}", day + 100),
                created_at: format!("2026-01-0{day}T00:00:00.000000Z"),
                event_type: "test".to_string(),
                user_id: None,
                node_number: None,
                details: "test".to_string(),
            };
            insert_audit_event_preserving_record(&db, &event).expect("insert");
        }

        let events = list_audit_events(&db, 3).expect("list");
        assert_eq!(events.len(), 3);
    }

    #[test]
    fn purge_audit_events_older_than_deletes_only_old_rows() {
        let db = test_db();
        let old = AuditEventRecord {
            id: "00000000-0000-4000-9000-000000000201".to_string(),
            created_at: "2025-01-01T00:00:00.000000Z".to_string(),
            event_type: "old".to_string(),
            user_id: None,
            node_number: None,
            details: "old event".to_string(),
        };
        let new = AuditEventRecord {
            id: "00000000-0000-4000-9000-000000000202".to_string(),
            created_at: "2026-01-01T00:00:00.000000Z".to_string(),
            event_type: "new".to_string(),
            user_id: None,
            node_number: None,
            details: "new event".to_string(),
        };
        insert_audit_event_preserving_record(&db, &old).expect("insert old");
        insert_audit_event_preserving_record(&db, &new).expect("insert new");

        let deleted =
            purge_audit_events_older_than(&db, "2025-06-01T00:00:00.000000Z").expect("purge");

        assert_eq!(deleted, 1);
        let events = list_audit_events(&db, 10).expect("list");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type, "new");
    }
}
