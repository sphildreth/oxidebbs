use decentdb::{Db, Value};

#[derive(Debug, Clone, PartialEq)]
pub struct SessionRecord {
    pub id: String,
    pub node_number: i64,
    pub user_id: Option<String>,
    pub transport: String,
    pub remote_address: String,
    pub remote_ip: Option<String>,
    pub remote_port: Option<i64>,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub disconnect_reason: Option<String>,
}

pub fn insert_session(db: &Db, session: &SessionRecord) -> decentdb::Result<()> {
    db.execute_with_params(
        "INSERT INTO sessions (id, node_number, user_id, transport, remote_address, remote_ip, remote_port, started_at, ended_at, disconnect_reason)
         VALUES (UUID_PARSE($1), $2, UUID_PARSE($3), $4, $5, $6, $7, $8, $9, $10)",
        &[
            Value::Text(session.id.clone()),
            Value::Int64(session.node_number),
            session
                .user_id
                .as_ref()
                .map(|value| Value::Text(value.clone()))
                .unwrap_or(Value::Null),
            Value::Text(session.transport.clone()),
            Value::Text(session.remote_address.clone()),
            session
                .remote_ip
                .as_ref()
                .map(|value| Value::Text(value.clone()))
                .unwrap_or(Value::Null),
            session.remote_port.map(Value::Int64).unwrap_or(Value::Null),
            Value::Text(session.started_at.clone()),
            session
                .ended_at
                .as_ref()
                .map(|value| Value::Text(value.clone()))
                .unwrap_or(Value::Null),
            session
                .disconnect_reason
                .as_ref()
                .map(|value| Value::Text(value.clone()))
                .unwrap_or(Value::Null),
        ],
    )?;
    Ok(())
}

pub fn end_session(
    db: &Db,
    session_id: &str,
    ended_at: &str,
    disconnect_reason: &str,
) -> decentdb::Result<()> {
    db.execute_with_params(
        "UPDATE sessions SET ended_at = $1, disconnect_reason = $2 WHERE id = UUID_PARSE($3)",
        &[
            Value::Text(ended_at.to_string()),
            Value::Text(disconnect_reason.to_string()),
            Value::Text(session_id.to_string()),
        ],
    )?;
    Ok(())
}

pub fn list_active_sessions(db: &Db) -> decentdb::Result<Vec<SessionRecord>> {
    let result = db.execute(
        "SELECT UUID_TO_STRING(id), node_number, UUID_TO_STRING(user_id), transport, remote_address, CAST(remote_ip AS TEXT), remote_port, CAST(started_at AS TEXT), CAST(ended_at AS TEXT), disconnect_reason
         FROM sessions WHERE ended_at IS NULL ORDER BY node_number",
    )?;
    Ok(result.rows().iter().map(row_to_session).collect())
}

pub fn list_recent_sessions(db: &Db, limit: i64) -> decentdb::Result<Vec<SessionRecord>> {
    let result = db.execute_with_params(
        "SELECT UUID_TO_STRING(id), node_number, UUID_TO_STRING(user_id), transport, remote_address, CAST(remote_ip AS TEXT), remote_port, CAST(started_at AS TEXT), CAST(ended_at AS TEXT), disconnect_reason
         FROM sessions ORDER BY started_at DESC LIMIT $1",
        &[Value::Int64(limit)],
    )?;
    Ok(result.rows().iter().map(row_to_session).collect())
}

fn row_to_session(row: &decentdb::QueryRow) -> SessionRecord {
    let values = row.values();
    SessionRecord {
        id: text_value(&values[0]),
        node_number: int_value(&values[1]),
        user_id: opt_text_value(&values[2]),
        transport: text_value(&values[3]),
        remote_address: text_value(&values[4]),
        remote_ip: opt_text_value(&values[5]),
        remote_port: opt_int_value(&values[6]),
        started_at: text_value(&values[7]),
        ended_at: opt_text_value(&values[8]),
        disconnect_reason: opt_text_value(&values[9]),
    }
}

fn text_value(value: &Value) -> String {
    match value {
        Value::Text(text) => text.clone(),
        _ => String::new(),
    }
}

fn opt_text_value(value: &Value) -> Option<String> {
    match value {
        Value::Text(text) => Some(text.clone()),
        _ => None,
    }
}

fn int_value(value: &Value) -> i64 {
    match value {
        Value::Int64(value) => *value,
        _ => 0,
    }
}

fn opt_int_value(value: &Value) -> Option<i64> {
    match value {
        Value::Int64(value) => Some(*value),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema;
    use decentdb::DbConfig;

    const SESSION_1: &str = "00000000-0000-4000-8000-000000000601";
    const SESSION_2: &str = "00000000-0000-4000-8000-000000000602";

    fn test_db() -> Db {
        let db = Db::open_or_create(":memory:", DbConfig::default()).expect("open DecentDB");
        schema::init_schema(&db).expect("init schema");
        db
    }

    fn sample_session(id: &str, node_number: i64) -> SessionRecord {
        SessionRecord {
            id: id.to_string(),
            node_number,
            user_id: None,
            transport: "telnet".to_string(),
            remote_address: "127.0.0.1:2323".to_string(),
            remote_ip: Some("127.0.0.1".to_string()),
            remote_port: Some(2323),
            started_at: "2026-01-01T00:00:00.000000Z".to_string(),
            ended_at: None,
            disconnect_reason: None,
        }
    }

    #[test]
    fn active_sessions_exclude_ended_sessions() {
        let db = test_db();
        insert_session(&db, &sample_session(SESSION_1, 1)).expect("insert");
        insert_session(&db, &sample_session(SESSION_2, 2)).expect("insert");
        end_session(&db, SESSION_2, "2026-01-01T01:00:00.000000Z", "user_logoff").expect("end");

        let active = list_active_sessions(&db).expect("list active");

        assert_eq!(active.len(), 1);
        assert_eq!(active[0].id, SESSION_1);
        assert_eq!(active[0].remote_ip.as_deref(), Some("127.0.0.1"));
        assert_eq!(active[0].remote_port, Some(2323));
    }

    #[test]
    fn recent_sessions_respect_limit() {
        let db = test_db();
        insert_session(&db, &sample_session(SESSION_1, 1)).expect("insert");
        insert_session(&db, &sample_session(SESSION_2, 2)).expect("insert");

        let recent = list_recent_sessions(&db, 1).expect("list recent");

        assert_eq!(recent.len(), 1);
    }

    #[test]
    fn invalid_remote_ip_is_rejected() {
        let db = test_db();
        let mut session = sample_session(SESSION_1, 1);
        session.remote_ip = Some("not-an-ip".to_string());

        let result = insert_session(&db, &session);

        assert!(result.is_err());
    }
}
