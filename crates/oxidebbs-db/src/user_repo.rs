use decentdb::{Db, Value};

#[derive(Debug, Clone, PartialEq)]
pub struct UserRecord {
    pub id: String,
    pub alias: String,
    pub real_name: String,
    pub email: Option<String>,
    pub password_hash: String,
    pub security_level: i64,
    pub is_sysop: bool,
    pub created_at: String,
    pub last_login_at: Option<String>,
    pub total_calls: i64,
    pub time_bank_minutes: i64,
    pub status: String,
}

pub fn insert_user(db: &Db, user: &UserRecord) -> decentdb::Result<()> {
    db.execute_with_params(
        "INSERT INTO users (id, alias, real_name, email, password_hash, security_level, is_sysop, created_at, last_login_at, total_calls, time_bank_minutes, status)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)",
        &[
            Value::Text(user.id.clone()),
            Value::Text(user.alias.clone()),
            Value::Text(user.real_name.clone()),
            user.email.as_ref().map(|e| Value::Text(e.clone())).unwrap_or(Value::Null),
            Value::Text(user.password_hash.clone()),
            Value::Int64(user.security_level),
            Value::Bool(user.is_sysop),
            Value::Text(user.created_at.clone()),
            user.last_login_at.as_ref().map(|t| Value::Text(t.clone())).unwrap_or(Value::Null),
            Value::Int64(user.total_calls),
            Value::Int64(user.time_bank_minutes),
            Value::Text(user.status.clone()),
        ],
    )?;
    Ok(())
}

pub fn find_user_by_alias(db: &Db, alias: &str) -> decentdb::Result<Option<UserRecord>> {
    let result = db.execute_with_params(
        "SELECT id, alias, real_name, email, password_hash, security_level, is_sysop, created_at, last_login_at, total_calls, time_bank_minutes, status
         FROM users WHERE alias = $1",
        &[Value::Text(alias.to_string())],
    )?;
    Ok(result.rows().first().map(row_to_user))
}

pub fn find_user_by_id(db: &Db, id: &str) -> decentdb::Result<Option<UserRecord>> {
    let result = db.execute_with_params(
        "SELECT id, alias, real_name, email, password_hash, security_level, is_sysop, created_at, last_login_at, total_calls, time_bank_minutes, status
         FROM users WHERE id = $1",
        &[Value::Text(id.to_string())],
    )?;
    Ok(result.rows().first().map(row_to_user))
}

pub fn list_users(db: &Db) -> decentdb::Result<Vec<UserRecord>> {
    let result = db.execute(
        "SELECT id, alias, real_name, email, password_hash, security_level, is_sysop, created_at, last_login_at, total_calls, time_bank_minutes, status
         FROM users ORDER BY alias",
    )?;
    Ok(result.rows().iter().map(row_to_user).collect())
}

pub fn update_user_login(db: &Db, id: &str, login_at: &str) -> decentdb::Result<()> {
    db.execute_with_params(
        "UPDATE users SET last_login_at = $1, total_calls = total_calls + 1 WHERE id = $2",
        &[
            Value::Text(login_at.to_string()),
            Value::Text(id.to_string()),
        ],
    )?;
    Ok(())
}

pub fn update_user_password_hash(db: &Db, id: &str, password_hash: &str) -> decentdb::Result<()> {
    db.execute_with_params(
        "UPDATE users SET password_hash = $1 WHERE id = $2",
        &[
            Value::Text(password_hash.to_string()),
            Value::Text(id.to_string()),
        ],
    )?;
    Ok(())
}

fn row_to_user(row: &decentdb::QueryRow) -> UserRecord {
    let values = row.values();
    UserRecord {
        id: text_value(&values[0]),
        alias: text_value(&values[1]),
        real_name: text_value(&values[2]),
        email: opt_text_value(&values[3]),
        password_hash: text_value(&values[4]),
        security_level: int_value(&values[5]),
        is_sysop: bool_value(&values[6]),
        created_at: text_value(&values[7]),
        last_login_at: opt_text_value(&values[8]),
        total_calls: int_value(&values[9]),
        time_bank_minutes: int_value(&values[10]),
        status: text_value(&values[11]),
    }
}

fn text_value(v: &Value) -> String {
    match v {
        Value::Text(s) => s.clone(),
        _ => String::new(),
    }
}

fn opt_text_value(v: &Value) -> Option<String> {
    match v {
        Value::Text(s) => Some(s.clone()),
        _ => None,
    }
}

fn int_value(v: &Value) -> i64 {
    match v {
        Value::Int64(n) => *n,
        _ => 0,
    }
}

fn bool_value(v: &Value) -> bool {
    match v {
        Value::Bool(b) => *b,
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

    fn sample_user(alias: &str) -> UserRecord {
        UserRecord {
            id: format!("uid-{alias}"),
            alias: alias.to_string(),
            real_name: format!("{alias} User"),
            email: Some(format!("{alias}@test.com")),
            password_hash: "hashed".to_string(),
            security_level: 10,
            is_sysop: false,
            created_at: "2026-01-01T00:00:00Z".to_string(),
            last_login_at: None,
            total_calls: 0,
            time_bank_minutes: 0,
            status: "active".to_string(),
        }
    }

    #[test]
    fn insert_and_find_by_alias() {
        let db = test_db();
        let user = sample_user("alice");
        insert_user(&db, &user).expect("insert");

        let found = find_user_by_alias(&db, "alice").expect("find");
        assert_eq!(found, Some(user));
    }

    #[test]
    fn find_nonexistent_user_returns_none() {
        let db = test_db();
        let found = find_user_by_alias(&db, "nobody").expect("find");
        assert_eq!(found, None);
    }

    #[test]
    fn find_by_id() {
        let db = test_db();
        let user = sample_user("bob");
        insert_user(&db, &user).expect("insert");

        let found = find_user_by_id(&db, "uid-bob").expect("find");
        assert_eq!(found, Some(user));
    }

    #[test]
    fn list_users_returns_sorted() {
        let db = test_db();
        insert_user(&db, &sample_user("charlie")).expect("insert");
        insert_user(&db, &sample_user("alice")).expect("insert");
        insert_user(&db, &sample_user("bob")).expect("insert");

        let users = list_users(&db).expect("list");
        let aliases: Vec<&str> = users.iter().map(|u| u.alias.as_str()).collect();
        assert_eq!(aliases, vec!["alice", "bob", "charlie"]);
    }

    #[test]
    fn update_login_increments_calls() {
        let db = test_db();
        let mut user = sample_user("dave");
        user.total_calls = 5;
        insert_user(&db, &user).expect("insert");

        update_user_login(&db, "uid-dave", "2026-06-01T12:00:00Z").expect("update");

        let found = find_user_by_id(&db, "uid-dave").expect("find").unwrap();
        assert_eq!(found.total_calls, 6);
        assert_eq!(
            found.last_login_at,
            Some("2026-06-01T12:00:00Z".to_string())
        );
    }

    #[test]
    fn duplicate_alias_is_rejected() {
        let db = test_db();
        insert_user(&db, &sample_user("eve")).expect("insert");
        let result = insert_user(&db, &sample_user("eve"));
        assert!(result.is_err());
    }

    #[test]
    fn update_password_hash_replaces_hash() {
        let db = test_db();
        insert_user(&db, &sample_user("frank")).expect("insert");

        update_user_password_hash(&db, "uid-frank", "$argon2id$new").expect("update hash");

        let found = find_user_by_id(&db, "uid-frank").expect("find").unwrap();
        assert_eq!(found.password_hash, "$argon2id$new");
    }
}
