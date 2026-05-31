use decentdb::{Db, Value};

use super::SCHEMA_VERSION;

pub fn init_schema(db: &Db) -> decentdb::Result<()> {
    db.execute_batch(
        "CREATE TABLE IF NOT EXISTS system_config (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS users (
            id TEXT PRIMARY KEY,
            alias TEXT NOT NULL UNIQUE,
            real_name TEXT NOT NULL,
            email TEXT,
            password_hash TEXT NOT NULL,
            security_level INTEGER NOT NULL DEFAULT 10,
            is_sysop BOOLEAN NOT NULL DEFAULT FALSE,
            created_at TEXT NOT NULL,
            last_login_at TEXT,
            total_calls INTEGER NOT NULL DEFAULT 0,
            time_bank_minutes INTEGER NOT NULL DEFAULT 0,
            status TEXT NOT NULL DEFAULT 'active'
        );

        CREATE TABLE IF NOT EXISTS audit_events (
            id TEXT PRIMARY KEY,
            created_at TEXT NOT NULL,
            event_type TEXT NOT NULL,
            user_id TEXT,
            node_number INTEGER,
            details TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS message_areas (
            id TEXT PRIMARY KEY,
            key TEXT NOT NULL UNIQUE,
            name TEXT NOT NULL,
            description TEXT NOT NULL DEFAULT '',
            kind TEXT NOT NULL DEFAULT 'local',
            network_id TEXT,
            read_security_level INTEGER NOT NULL DEFAULT 0,
            post_security_level INTEGER NOT NULL DEFAULT 10,
            moderated BOOLEAN NOT NULL DEFAULT FALSE
        );

        CREATE TABLE IF NOT EXISTS messages (
            id TEXT PRIMARY KEY,
            area_id TEXT NOT NULL,
            author_user_id TEXT NOT NULL,
            to_user_id TEXT,
            subject TEXT NOT NULL,
            body TEXT NOT NULL,
            created_at TEXT NOT NULL,
            reply_to_id TEXT,
            network_message_id TEXT,
            visibility TEXT NOT NULL DEFAULT 'normal'
        );

        CREATE TABLE IF NOT EXISTS sessions (
            id TEXT PRIMARY KEY,
            node_number INTEGER NOT NULL,
            user_id TEXT,
            transport TEXT NOT NULL,
            remote_address TEXT NOT NULL DEFAULT '',
            started_at TEXT NOT NULL,
            ended_at TEXT,
            disconnect_reason TEXT
        );

        CREATE TABLE IF NOT EXISTS doors (
            id TEXT PRIMARY KEY,
            key TEXT NOT NULL UNIQUE,
            name TEXT NOT NULL,
            runner TEXT NOT NULL,
            working_dir TEXT NOT NULL,
            command TEXT NOT NULL,
            drop_file TEXT NOT NULL,
            exclusive BOOLEAN NOT NULL DEFAULT FALSE,
            time_limit_minutes INTEGER NOT NULL DEFAULT 30,
            enabled BOOLEAN NOT NULL DEFAULT TRUE
        );

        CREATE TABLE IF NOT EXISTS door_runs (
            id TEXT PRIMARY KEY,
            door_id TEXT NOT NULL,
            user_id TEXT NOT NULL,
            node_number INTEGER NOT NULL,
            started_at TEXT NOT NULL,
            ended_at TEXT,
            exit_code INTEGER,
            timed_out BOOLEAN NOT NULL DEFAULT FALSE,
            disconnect_forced BOOLEAN NOT NULL DEFAULT FALSE,
            bytes_in INTEGER NOT NULL DEFAULT 0,
            bytes_out INTEGER NOT NULL DEFAULT 0
        );",
    )?;

    db.execute_with_params(
        "INSERT INTO system_config (key, value)
         VALUES ($1, $2)
         ON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value",
        &[
            Value::Text("schema_version".to_string()),
            Value::Text(SCHEMA_VERSION.to_string()),
        ],
    )?;
    Ok(())
}

pub fn schema_version(db: &Db) -> decentdb::Result<i64> {
    let result = db.execute_with_params(
        "SELECT value FROM system_config WHERE key = $1",
        &[Value::Text("schema_version".to_string())],
    )?;
    let value = result
        .rows()
        .first()
        .and_then(|row| row.values().first())
        .ok_or_else(|| decentdb::DbError::sql("OxideBBS schema_version is missing"))?;

    match value {
        Value::Text(raw) => raw.parse::<i64>().map_err(|error| {
            decentdb::DbError::sql(format!("invalid OxideBBS schema version {raw:?}: {error}"))
        }),
        other => Err(decentdb::DbError::sql(format!(
            "invalid OxideBBS schema_version value: {other:?}"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use decentdb::DbConfig;

    #[test]
    fn schema_creates_all_tables() {
        let db = Db::open_or_create(":memory:", DbConfig::default()).expect("open DecentDB");
        init_schema(&db).expect("init schema");

        let tables = [
            "system_config",
            "users",
            "audit_events",
            "message_areas",
            "messages",
            "sessions",
            "doors",
            "door_runs",
        ];
        for table in &tables {
            let result = db
                .execute(&format!("SELECT * FROM {table} LIMIT 0"))
                .unwrap_or_else(|_| panic!("query {table}"));
            assert!(!result.columns().is_empty(), "{table} should exist");
        }
    }

    #[test]
    fn schema_init_is_idempotent() {
        let db = Db::open_or_create(":memory:", DbConfig::default()).expect("open DecentDB");
        init_schema(&db).expect("first init");
        init_schema(&db).expect("second init");
        assert_eq!(schema_version(&db).expect("read schema version"), 1);
    }
}
