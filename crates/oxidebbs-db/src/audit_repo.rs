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
         VALUES ($1, $2, $3, $4, $5, $6)",
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
        "SELECT id, created_at, event_type, user_id, node_number, details
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
        "SELECT id, created_at, event_type, user_id, node_number, details
         FROM audit_events WHERE user_id = $1 ORDER BY created_at DESC LIMIT $2",
        &[Value::Text(user_id.to_string()), Value::Int64(limit)],
    )?;
    Ok(result.rows().iter().map(row_to_audit_event).collect())
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
    use decentdb::DbConfig;

    use std::sync::atomic::{AtomicU64, Ordering};

    static EVENT_SEQ: AtomicU64 = AtomicU64::new(0);

    fn test_db() -> Db {
        let db = Db::open_or_create(":memory:", DbConfig::default()).expect("open DecentDB");
        schema::init_schema(&db).expect("init schema");
        db
    }

    fn sample_event(event_type: &str, user_id: Option<&str>) -> AuditEventRecord {
        let seq = EVENT_SEQ.fetch_add(1, Ordering::Relaxed);
        let suffix = user_id.unwrap_or("none");
        AuditEventRecord {
            id: format!("evt-{event_type}-{suffix}-{seq}"),
            created_at: "2026-01-01T00:00:00Z".to_string(),
            event_type: event_type.to_string(),
            user_id: user_id.map(String::from),
            node_number: Some(1),
            details: format!("{event_type} happened"),
        }
    }

    #[test]
    fn insert_and_list_audit_events() {
        let db = test_db();
        insert_audit_event(&db, &sample_event("login_success", Some("uid-1"))).expect("insert");
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
        insert_audit_event(&db, &sample_event("login_success", Some("uid-1"))).expect("insert");
        insert_audit_event(&db, &sample_event("login_success", Some("uid-2"))).expect("insert");
        insert_audit_event(&db, &sample_event("login_failure", Some("uid-1"))).expect("insert");

        let events = list_audit_events_for_user(&db, "uid-1", 10).expect("list");
        assert_eq!(events.len(), 2);
        assert!(events.iter().all(|e| e.user_id.as_deref() == Some("uid-1")));
    }

    #[test]
    fn audit_events_respect_limit() {
        let db = test_db();
        for i in 0..5 {
            let event = AuditEventRecord {
                id: format!("evt-{i}"),
                created_at: format!("2026-01-0{i}T00:00:00Z"),
                event_type: "test".to_string(),
                user_id: None,
                node_number: None,
                details: "test".to_string(),
            };
            insert_audit_event(&db, &event).expect("insert");
        }

        let events = list_audit_events(&db, 3).expect("list");
        assert_eq!(events.len(), 3);
    }
}
