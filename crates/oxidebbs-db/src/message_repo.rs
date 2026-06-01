use decentdb::{Db, Value};

#[derive(Debug, Clone, PartialEq)]
pub struct MessageAreaRecord {
    pub id: String,
    pub key: String,
    pub name: String,
    pub description: String,
    pub kind: String,
    pub network_id: Option<String>,
    pub read_security_level: i64,
    pub post_security_level: i64,
    pub moderated: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MessageRecord {
    pub id: String,
    pub area_id: String,
    pub author_user_id: String,
    pub to_user_id: Option<String>,
    pub subject: String,
    pub body: String,
    pub created_at: String,
    pub reply_to_id: Option<String>,
    pub network_message_id: Option<String>,
    pub visibility: String,
}

pub fn insert_message_area(db: &Db, area: &MessageAreaRecord) -> decentdb::Result<()> {
    db.execute_with_params(
        "INSERT INTO message_areas (id, key, name, description, kind, network_id, read_security_level, post_security_level, moderated)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
        &[
            Value::Text(area.id.clone()),
            Value::Text(area.key.clone()),
            Value::Text(area.name.clone()),
            Value::Text(area.description.clone()),
            Value::Text(area.kind.clone()),
            area.network_id.as_ref().map(|id| Value::Text(id.clone())).unwrap_or(Value::Null),
            Value::Int64(area.read_security_level),
            Value::Int64(area.post_security_level),
            Value::Bool(area.moderated),
        ],
    )?;
    Ok(())
}

pub fn list_message_areas(db: &Db) -> decentdb::Result<Vec<MessageAreaRecord>> {
    let result = db.execute(
        "SELECT id, key, name, description, kind, network_id, read_security_level, post_security_level, moderated
         FROM message_areas ORDER BY key",
    )?;
    Ok(result.rows().iter().map(row_to_area).collect())
}

pub fn find_message_area_by_key(db: &Db, key: &str) -> decentdb::Result<Option<MessageAreaRecord>> {
    let result = db.execute_with_params(
        "SELECT id, key, name, description, kind, network_id, read_security_level, post_security_level, moderated
         FROM message_areas WHERE key = $1",
        &[Value::Text(key.to_string())],
    )?;
    Ok(result.rows().first().map(row_to_area))
}

pub fn insert_message(db: &Db, message: &MessageRecord) -> decentdb::Result<()> {
    db.execute_with_params(
        "INSERT INTO messages (id, area_id, author_user_id, to_user_id, subject, body, created_at, reply_to_id, network_message_id, visibility)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
        &[
            Value::Text(message.id.clone()),
            Value::Text(message.area_id.clone()),
            Value::Text(message.author_user_id.clone()),
            message.to_user_id.as_ref().map(|id| Value::Text(id.clone())).unwrap_or(Value::Null),
            Value::Text(message.subject.clone()),
            Value::Text(message.body.clone()),
            Value::Text(message.created_at.clone()),
            message.reply_to_id.as_ref().map(|id| Value::Text(id.clone())).unwrap_or(Value::Null),
            message
                .network_message_id
                .as_ref()
                .map(|id| Value::Text(id.clone()))
                .unwrap_or(Value::Null),
            Value::Text(message.visibility.clone()),
        ],
    )?;
    Ok(())
}

pub fn list_messages_in_area(db: &Db, area_id: &str) -> decentdb::Result<Vec<MessageRecord>> {
    let result = db.execute_with_params(
        "SELECT id, area_id, author_user_id, to_user_id, subject, body, created_at, reply_to_id, network_message_id, visibility
         FROM messages WHERE area_id = $1 ORDER BY created_at",
        &[Value::Text(area_id.to_string())],
    )?;
    Ok(result.rows().iter().map(row_to_message).collect())
}

pub fn update_message_visibility(
    db: &Db,
    message_id: &str,
    visibility: &str,
) -> decentdb::Result<()> {
    db.execute_with_params(
        "UPDATE messages SET visibility = $1 WHERE id = $2",
        &[
            Value::Text(visibility.to_string()),
            Value::Text(message_id.to_string()),
        ],
    )?;
    Ok(())
}

fn row_to_area(row: &decentdb::QueryRow) -> MessageAreaRecord {
    let values = row.values();
    MessageAreaRecord {
        id: text_value(&values[0]),
        key: text_value(&values[1]),
        name: text_value(&values[2]),
        description: text_value(&values[3]),
        kind: text_value(&values[4]),
        network_id: opt_text_value(&values[5]),
        read_security_level: int_value(&values[6]),
        post_security_level: int_value(&values[7]),
        moderated: bool_value(&values[8]),
    }
}

fn row_to_message(row: &decentdb::QueryRow) -> MessageRecord {
    let values = row.values();
    MessageRecord {
        id: text_value(&values[0]),
        area_id: text_value(&values[1]),
        author_user_id: text_value(&values[2]),
        to_user_id: opt_text_value(&values[3]),
        subject: text_value(&values[4]),
        body: text_value(&values[5]),
        created_at: text_value(&values[6]),
        reply_to_id: opt_text_value(&values[7]),
        network_message_id: opt_text_value(&values[8]),
        visibility: text_value(&values[9]),
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

fn bool_value(value: &Value) -> bool {
    match value {
        Value::Bool(value) => *value,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema;
    use decentdb::DbConfig;

    fn test_db() -> Db {
        let db = Db::open_or_create(":memory:", DbConfig::default()).expect("open DecentDB");
        schema::init_schema(&db).expect("init schema");
        db
    }

    fn sample_area(key: &str) -> MessageAreaRecord {
        MessageAreaRecord {
            id: format!("area-{key}"),
            key: key.to_string(),
            name: format!("{key} area"),
            description: "discussion".to_string(),
            kind: "local".to_string(),
            network_id: None,
            read_security_level: 0,
            post_security_level: 10,
            moderated: false,
        }
    }

    fn sample_message(id: &str, area_id: &str) -> MessageRecord {
        MessageRecord {
            id: id.to_string(),
            area_id: area_id.to_string(),
            author_user_id: "uid-1".to_string(),
            to_user_id: None,
            subject: "Subject".to_string(),
            body: "Body".to_string(),
            created_at: "2026-01-01T00:00:00Z".to_string(),
            reply_to_id: None,
            network_message_id: None,
            visibility: "normal".to_string(),
        }
    }

    #[test]
    fn inserts_and_finds_message_area() {
        let db = test_db();
        let area = sample_area("general");
        insert_message_area(&db, &area).expect("insert");

        let found = find_message_area_by_key(&db, "general").expect("find");

        assert_eq!(found, Some(area));
    }

    #[test]
    fn lists_messages_in_area() {
        let db = test_db();
        insert_message_area(&db, &sample_area("general")).expect("insert area");
        insert_message(&db, &sample_message("msg-1", "area-general")).expect("insert message");

        let messages = list_messages_in_area(&db, "area-general").expect("list");

        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].id, "msg-1");
    }

    #[test]
    fn updates_message_visibility() {
        let db = test_db();
        insert_message_area(&db, &sample_area("general")).expect("insert area");
        insert_message(&db, &sample_message("msg-1", "area-general")).expect("insert message");

        update_message_visibility(&db, "msg-1", "deleted").expect("update");

        let messages = list_messages_in_area(&db, "area-general").expect("list");
        assert_eq!(messages[0].visibility, "deleted");
    }
}
