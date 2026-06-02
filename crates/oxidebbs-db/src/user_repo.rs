use std::error::Error;
use std::fmt;

use decentdb::{Db, DbError, Value};

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

#[derive(Debug)]
pub enum UserInsertError {
    DuplicateAlias { alias: String },
    Db(DbError),
}

impl fmt::Display for UserInsertError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateAlias { alias } => {
                write!(formatter, "alias {alias:?} is already taken")
            }
            Self::Db(error) => write!(formatter, "{error}"),
        }
    }
}

impl Error for UserInsertError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::DuplicateAlias { .. } => None,
            Self::Db(error) => Some(error),
        }
    }
}

impl From<DbError> for UserInsertError {
    fn from(error: DbError) -> Self {
        Self::Db(error)
    }
}

pub fn insert_user(db: &Db, user: &UserRecord) -> decentdb::Result<()> {
    let params = user_insert_params(user);
    db.execute_with_params(
        "INSERT INTO users (id, alias, alias_normalized, real_name, email, password_hash, security_level, is_sysop, created_at, last_login_at, total_calls, time_bank_minutes, status)
         VALUES (UUID_PARSE($1), $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)",
        &params,
    )?;
    Ok(())
}

pub fn insert_user_if_alias_available(db: &Db, user: &UserRecord) -> Result<(), UserInsertError> {
    let params = user_insert_params(user);
    let result = db.execute_with_params(
        "INSERT INTO users (id, alias, alias_normalized, real_name, email, password_hash, security_level, is_sysop, created_at, last_login_at, total_calls, time_bank_minutes, status)
         VALUES (UUID_PARSE($1), $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
         ON CONFLICT (alias_normalized) DO NOTHING
         RETURNING UUID_TO_STRING(id)",
        &params,
    )?;

    if result.rows().is_empty() {
        Err(UserInsertError::DuplicateAlias {
            alias: user.alias.clone(),
        })
    } else {
        Ok(())
    }
}

fn user_insert_params(user: &UserRecord) -> [Value; 13] {
    [
        Value::Text(user.id.clone()),
        Value::Text(user.alias.clone()),
        Value::Text(normalize_alias(&user.alias)),
        Value::Text(user.real_name.clone()),
        user.email
            .as_ref()
            .map(|e| Value::Text(e.clone()))
            .unwrap_or(Value::Null),
        Value::Text(user.password_hash.clone()),
        Value::Int64(user.security_level),
        Value::Bool(user.is_sysop),
        Value::Text(user.created_at.clone()),
        user.last_login_at
            .as_ref()
            .map(|t| Value::Text(t.clone()))
            .unwrap_or(Value::Null),
        Value::Int64(user.total_calls),
        Value::Int64(user.time_bank_minutes),
        Value::Text(user.status.clone()),
    ]
}

pub fn find_user_by_alias(db: &Db, alias: &str) -> decentdb::Result<Option<UserRecord>> {
    let result = db.execute_with_params(
        "SELECT UUID_TO_STRING(id), alias, real_name, email, password_hash, security_level, is_sysop, CAST(created_at AS TEXT), CAST(last_login_at AS TEXT), total_calls, time_bank_minutes, status
         FROM users WHERE alias = $1",
        &[Value::Text(alias.to_string())],
    )?;
    Ok(result.rows().first().map(row_to_user))
}

pub fn find_user_by_alias_ci(db: &Db, alias: &str) -> decentdb::Result<Option<UserRecord>> {
    let result = db.execute_with_params(
        "SELECT UUID_TO_STRING(id), alias, real_name, email, password_hash, security_level, is_sysop, CAST(created_at AS TEXT), CAST(last_login_at AS TEXT), total_calls, time_bank_minutes, status
         FROM users WHERE alias_normalized = $1",
        &[Value::Text(normalize_alias(alias))],
    )?;
    Ok(result.rows().first().map(row_to_user))
}

pub fn find_user_by_id(db: &Db, id: &str) -> decentdb::Result<Option<UserRecord>> {
    let result = db.execute_with_params(
        "SELECT UUID_TO_STRING(id), alias, real_name, email, password_hash, security_level, is_sysop, CAST(created_at AS TEXT), CAST(last_login_at AS TEXT), total_calls, time_bank_minutes, status
         FROM users WHERE id = UUID_PARSE($1)",
        &[Value::Text(id.to_string())],
    )?;
    Ok(result.rows().first().map(row_to_user))
}

pub fn list_user_aliases_by_ids(
    db: &Db,
    user_ids: &[String],
) -> decentdb::Result<Vec<(String, String)>> {
    if user_ids.is_empty() {
        return Ok(Vec::new());
    }

    let placeholders = (1..=user_ids.len())
        .map(|index| format!("UUID_PARSE(${index})"))
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!("SELECT UUID_TO_STRING(id), alias FROM users WHERE id IN ({placeholders})");
    let params = user_ids
        .iter()
        .map(|user_id| Value::Text(user_id.clone()))
        .collect::<Vec<_>>();
    let result = db.execute_with_params(&sql, &params)?;

    Ok(result
        .rows()
        .iter()
        .map(|row| {
            let values = row.values();
            (text_value(&values[0]), text_value(&values[1]))
        })
        .collect())
}

pub fn list_users(db: &Db) -> decentdb::Result<Vec<UserRecord>> {
    let result = db.execute(
        "SELECT UUID_TO_STRING(id), alias, real_name, email, password_hash, security_level, is_sysop, CAST(created_at AS TEXT), CAST(last_login_at AS TEXT), total_calls, time_bank_minutes, status
         FROM users ORDER BY alias",
    )?;
    Ok(result.rows().iter().map(row_to_user).collect())
}

pub fn update_user_login(db: &Db, id: &str, login_at: &str) -> decentdb::Result<()> {
    db.execute_with_params(
        "UPDATE users SET last_login_at = $1, total_calls = total_calls + 1 WHERE id = UUID_PARSE($2)",
        &[
            Value::Text(login_at.to_string()),
            Value::Text(id.to_string()),
        ],
    )?;
    Ok(())
}

pub fn update_user_password_hash(db: &Db, id: &str, password_hash: &str) -> decentdb::Result<()> {
    db.execute_with_params(
        "UPDATE users SET password_hash = $1 WHERE id = UUID_PARSE($2)",
        &[
            Value::Text(password_hash.to_string()),
            Value::Text(id.to_string()),
        ],
    )?;
    Ok(())
}

pub fn update_user_security_level(db: &Db, id: &str, security_level: i64) -> decentdb::Result<()> {
    db.execute_with_params(
        "UPDATE users SET security_level = $1 WHERE id = UUID_PARSE($2)",
        &[Value::Int64(security_level), Value::Text(id.to_string())],
    )?;
    Ok(())
}

pub fn update_user_status(db: &Db, id: &str, status: &str) -> decentdb::Result<()> {
    db.execute_with_params(
        "UPDATE users SET status = $1 WHERE id = UUID_PARSE($2)",
        &[Value::Text(status.to_string()), Value::Text(id.to_string())],
    )?;
    Ok(())
}

pub fn update_user_is_sysop(db: &Db, id: &str, is_sysop: bool) -> decentdb::Result<()> {
    db.execute_with_params(
        "UPDATE users SET is_sysop = $1 WHERE id = UUID_PARSE($2)",
        &[Value::Bool(is_sysop), Value::Text(id.to_string())],
    )?;
    Ok(())
}

pub fn update_user_alias(db: &Db, id: &str, alias: &str) -> decentdb::Result<()> {
    db.execute_with_params(
        "UPDATE users SET alias = $1, alias_normalized = $2 WHERE id = UUID_PARSE($3)",
        &[
            Value::Text(alias.to_string()),
            Value::Text(normalize_alias(alias)),
            Value::Text(id.to_string()),
        ],
    )?;
    Ok(())
}

pub fn normalize_alias(alias: &str) -> String {
    alias.trim().to_ascii_lowercase()
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
    use std::sync::{Arc, Barrier};
    use std::thread;

    fn test_db() -> Db {
        let db = Db::open_or_create(":memory:", DbConfig::default()).expect("open DecentDB");
        schema::init_schema(&db).expect("init schema");
        db
    }

    fn test_uuid(seed: &str) -> String {
        let hash = seed.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
            hash.wrapping_mul(0x0000_0100_0000_01b3) ^ u64::from(byte)
        });
        format!(
            "00000000-0000-4000-8000-{:012x}",
            hash & 0x0000_ffff_ffff_ffff
        )
    }

    fn sample_user(alias: &str) -> UserRecord {
        UserRecord {
            id: test_uuid(alias),
            alias: alias.to_string(),
            real_name: format!("{alias} User"),
            email: Some(format!("{alias}@test.com")),
            password_hash: "hashed".to_string(),
            security_level: 10,
            is_sysop: false,
            created_at: "2026-01-01T00:00:00.000000Z".to_string(),
            last_login_at: None,
            total_calls: 0,
            time_bank_minutes: 0,
            status: "active".to_string(),
        }
    }

    fn sample_user_with_id(alias: &str, id_seed: &str) -> UserRecord {
        let mut user = sample_user(alias);
        user.id = test_uuid(id_seed);
        user
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
    fn find_by_alias_is_case_insensitive() {
        let db = test_db();
        let user = sample_user("Alice");
        insert_user(&db, &user).expect("insert");

        let found = find_user_by_alias_ci(&db, "alice").expect("find");
        assert_eq!(found, Some(user));
    }

    #[test]
    fn find_by_id() {
        let db = test_db();
        let user = sample_user("bob");
        insert_user(&db, &user).expect("insert");

        let found = find_user_by_id(&db, &user.id).expect("find");
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
    fn list_user_aliases_by_ids_returns_existing_aliases() {
        let db = test_db();
        let alice = sample_user("alice");
        let bob = sample_user("bob");
        insert_user(&db, &alice).expect("insert alice");
        insert_user(&db, &bob).expect("insert bob");

        let mut aliases = list_user_aliases_by_ids(
            &db,
            &[
                alice.id.clone(),
                "00000000-0000-4000-8000-ffffffffffff".to_string(),
                bob.id.clone(),
            ],
        )
        .expect("list aliases");
        aliases.sort_by(|left, right| left.1.cmp(&right.1));

        assert_eq!(
            aliases,
            vec![(alice.id, "alice".to_string()), (bob.id, "bob".to_string())]
        );
    }

    #[test]
    fn update_login_increments_calls() {
        let db = test_db();
        let mut user = sample_user("dave");
        user.total_calls = 5;
        insert_user(&db, &user).expect("insert");

        update_user_login(&db, &user.id, "2026-06-01T12:00:00.000000Z").expect("update");

        let found = find_user_by_id(&db, &user.id).expect("find").unwrap();
        assert_eq!(found.total_calls, 6);
        assert_eq!(
            found.last_login_at,
            Some("2026-06-01T12:00:00.000000Z".to_string())
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
    fn insert_user_if_alias_available_returns_typed_duplicate_alias() {
        let db = test_db();
        insert_user_if_alias_available(&db, &sample_user("eve")).expect("insert");

        let result = insert_user_if_alias_available(&db, &sample_user("eve"));

        assert!(matches!(
            result,
            Err(UserInsertError::DuplicateAlias { alias }) if alias == "eve"
        ));
    }

    #[test]
    fn duplicate_alias_differing_only_by_case_is_rejected() {
        let db = test_db();
        insert_user(&db, &sample_user("Alice")).expect("insert");
        let result = insert_user(&db, &sample_user("alice"));
        assert!(result.is_err());
    }

    #[test]
    fn insert_user_if_alias_available_returns_typed_case_duplicate_alias() {
        let db = test_db();
        insert_user_if_alias_available(&db, &sample_user("Alice")).expect("insert");

        let result = insert_user_if_alias_available(&db, &sample_user("alice"));

        assert!(matches!(
            result,
            Err(UserInsertError::DuplicateAlias { alias }) if alias == "alice"
        ));
    }

    #[derive(Debug, PartialEq, Eq)]
    enum ConcurrentInsertOutcome {
        Inserted,
        Duplicate,
        Db,
    }

    #[test]
    fn insert_user_if_alias_available_allows_one_concurrent_duplicate_alias() {
        let db = test_db();
        let barrier = Arc::new(Barrier::new(2));

        let outcomes = thread::scope(|scope| {
            let handles = (0..2)
                .map(|index| {
                    let barrier = Arc::clone(&barrier);
                    let db = &db;
                    scope.spawn(move || {
                        let user = sample_user_with_id("RaceAlias", &format!("race-alias-{index}"));
                        barrier.wait();
                        match insert_user_if_alias_available(db, &user) {
                            Ok(()) => ConcurrentInsertOutcome::Inserted,
                            Err(UserInsertError::DuplicateAlias { .. }) => {
                                ConcurrentInsertOutcome::Duplicate
                            }
                            Err(UserInsertError::Db(_)) => ConcurrentInsertOutcome::Db,
                        }
                    })
                })
                .collect::<Vec<_>>();

            handles
                .into_iter()
                .map(|handle| handle.join().expect("join insert thread"))
                .collect::<Vec<_>>()
        });

        assert_eq!(
            outcomes
                .iter()
                .filter(|outcome| **outcome == ConcurrentInsertOutcome::Inserted)
                .count(),
            1
        );
        assert_eq!(
            outcomes
                .iter()
                .filter(|outcome| **outcome == ConcurrentInsertOutcome::Duplicate)
                .count(),
            1
        );
        assert!(!outcomes.contains(&ConcurrentInsertOutcome::Db));

        let matching_users = list_users(&db)
            .expect("list users")
            .into_iter()
            .filter(|user| user.alias.eq_ignore_ascii_case("RaceAlias"))
            .count();
        assert_eq!(matching_users, 1);
    }

    #[test]
    fn update_password_hash_replaces_hash() {
        let db = test_db();
        insert_user(&db, &sample_user("frank")).expect("insert");
        let user = sample_user("frank");

        update_user_password_hash(&db, &user.id, "$argon2id$new").expect("update hash");

        let found = find_user_by_id(&db, &user.id).expect("find").unwrap();
        assert_eq!(found.password_hash, "$argon2id$new");
    }

    #[test]
    fn update_security_level_changes_value() {
        let db = test_db();
        let user = sample_user("security");
        insert_user(&db, &user).expect("insert");

        update_user_security_level(&db, &user.id, 255).expect("update");

        let found = find_user_by_id(&db, &user.id).expect("find").unwrap();
        assert_eq!(found.security_level, 255);
    }

    #[test]
    fn update_status_changes_value() {
        let db = test_db();
        let user = sample_user("status");
        insert_user(&db, &user).expect("insert");

        update_user_status(&db, &user.id, "disabled").expect("update");

        let found = find_user_by_id(&db, &user.id).expect("find").unwrap();
        assert_eq!(found.status, "disabled");
    }

    #[test]
    fn update_is_sysop_changes_value() {
        let db = test_db();
        let mut user = sample_user("sysop");
        user.is_sysop = false;
        insert_user(&db, &user).expect("insert");

        update_user_is_sysop(&db, &user.id, true).expect("update");

        let found = find_user_by_id(&db, &user.id).expect("find").unwrap();
        assert!(found.is_sysop);
    }

    #[test]
    fn update_alias_changes_lookup_key() {
        let db = test_db();
        let user = sample_user("oldalias");
        insert_user(&db, &user).expect("insert");

        update_user_alias(&db, &user.id, "newalias").expect("rename");

        assert!(
            find_user_by_alias_ci(&db, "oldalias")
                .expect("find old")
                .is_none()
        );
        let found = find_user_by_alias_ci(&db, "newalias")
            .expect("find new")
            .expect("user exists");
        assert_eq!(found.id, user.id);
    }

    #[test]
    fn update_alias_rejects_case_insensitive_collision() {
        let db = test_db();
        let user = sample_user("first");
        let other = sample_user("other");
        insert_user(&db, &user).expect("insert first");
        insert_user(&db, &other).expect("insert other");

        let result = update_user_alias(&db, &other.id, "FIRST");

        assert!(result.is_err());
    }

    #[test]
    fn invalid_user_uuid_is_rejected() {
        let db = test_db();
        let mut user = sample_user("invalid");
        user.id = "uid-invalid".to_string();

        let result = insert_user(&db, &user);

        assert!(result.is_err());
    }

    #[test]
    fn sequential_login_updates_increment_calls_with_single_atomic_statement() {
        let db = test_db();
        let user = sample_user("counter");
        insert_user(&db, &user).expect("insert");

        // DecentDB executes the single UPDATE statement atomically; keeping
        // last_login_at and total_calls in one statement avoids split writes.
        update_user_login(&db, &user.id, "2026-06-01T12:00:00.000000Z").expect("first");
        update_user_login(&db, &user.id, "2026-06-01T12:30:00.000000Z").expect("second");

        let found = find_user_by_id(&db, &user.id).expect("find").unwrap();
        assert_eq!(found.total_calls, 2);
        assert_eq!(
            found.last_login_at,
            Some("2026-06-01T12:30:00.000000Z".to_string())
        );
    }
}
