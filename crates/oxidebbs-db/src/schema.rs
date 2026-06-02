use std::cmp::Ordering;

use decentdb::{Db, DbError, Value};

use super::SCHEMA_VERSION;
use super::migrations;

pub fn init_schema(db: &Db) -> decentdb::Result<()> {
    let version = existing_schema_version(db)?;

    match version {
        Some(version) => match version.cmp(&SCHEMA_VERSION) {
            Ordering::Less => {
                migrations::migrate_to_current(db)?;
                create_full_schema(db)
            }
            Ordering::Equal => Ok(()),
            Ordering::Greater => Err(DbError::sql(format!(
                "OxideBBS database schema version {version} is newer than supported version {SCHEMA_VERSION}"
            ))),
        },
        None => {
            if has_system_config_table(db)? {
                Err(DbError::sql(
                    "OxideBBS system_config table exists but schema_version marker is missing",
                ))
            } else if has_any_user_table(db)? {
                Err(DbError::sql(
                    "OxideBBS schema_version marker is missing; found existing database tables",
                ))
            } else {
                create_full_schema(db)
            }
        }
    }
}

fn has_any_user_table(db: &Db) -> decentdb::Result<bool> {
    Ok(!db.list_tables()?.is_empty())
}

fn has_system_config_table(db: &Db) -> decentdb::Result<bool> {
    Ok(db
        .list_tables()?
        .iter()
        .any(|table| table.name == "system_config"))
}

pub(crate) fn existing_schema_version(db: &Db) -> decentdb::Result<Option<i64>> {
    if !has_system_config_table(db)? {
        return Ok(None);
    }

    let result = db.execute_with_params(
        "SELECT value FROM system_config WHERE key = $1",
        &[Value::Text("schema_version".to_string())],
    )?;
    let Some(value) = result.rows().first().and_then(|row| row.values().first()) else {
        return Ok(None);
    };

    parse_schema_version(value).map(Some)
}

pub(crate) fn set_schema_version(db: &Db, version: i64) -> decentdb::Result<()> {
    db.execute_with_params(
        "INSERT INTO system_config (key, value)
         VALUES ($1, $2)
         ON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value, updated_at = CURRENT_TIMESTAMP",
        &[
            Value::Text("schema_version".to_string()),
            Value::Text(version.to_string()),
        ],
    )?;
    Ok(())
}

fn create_full_schema(db: &Db) -> decentdb::Result<()> {
    db.execute_batch(
        "CREATE TABLE IF NOT EXISTS system_config (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL,
            updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
        );

        CREATE TABLE IF NOT EXISTS users (
            id UUID PRIMARY KEY DEFAULT GEN_RANDOM_UUID(),
            alias TEXT NOT NULL UNIQUE CHECK (LENGTH(TRIM(alias)) > 0),
            alias_normalized TEXT NOT NULL UNIQUE CHECK (LENGTH(TRIM(alias_normalized)) > 0),
            real_name TEXT NOT NULL CHECK (LENGTH(TRIM(real_name)) > 0),
            email TEXT,
            password_hash TEXT NOT NULL,
            security_level INT NOT NULL DEFAULT 10 CHECK (security_level >= 0 AND security_level <= 255),
            is_sysop BOOL NOT NULL DEFAULT FALSE,
            created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
            last_login_at TIMESTAMPTZ,
            total_calls INT NOT NULL DEFAULT 0 CHECK (total_calls >= 0),
            time_bank_minutes INT NOT NULL DEFAULT 0 CHECK (time_bank_minutes >= 0),
            status TEXT NOT NULL DEFAULT 'active'
                CHECK (status = 'active' OR status = 'locked' OR status = 'disabled')
        );

        CREATE TABLE IF NOT EXISTS auth_attempts (
            scope TEXT NOT NULL CHECK (scope = 'ip' OR scope = 'alias'),
            scope_key TEXT NOT NULL CHECK (LENGTH(TRIM(scope_key)) > 0),
            failed_count INT NOT NULL DEFAULT 0 CHECK (failed_count >= 0),
            first_failed_at TIMESTAMPTZ,
            last_failed_at TIMESTAMPTZ,
            locked_until TIMESTAMPTZ,
            PRIMARY KEY (scope, scope_key)
        );

        CREATE TABLE IF NOT EXISTS audit_events (
            id UUID PRIMARY KEY DEFAULT GEN_RANDOM_UUID(),
            created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
            event_type TEXT NOT NULL CHECK (LENGTH(TRIM(event_type)) > 0),
            user_id UUID REFERENCES users(id) ON DELETE SET NULL,
            node_number INT CHECK (node_number IS NULL OR node_number > 0),
            details TEXT NOT NULL DEFAULT ''
        );

        CREATE TABLE IF NOT EXISTS message_areas (
            id UUID PRIMARY KEY DEFAULT GEN_RANDOM_UUID(),
            key TEXT NOT NULL UNIQUE CHECK (LENGTH(TRIM(key)) > 0),
            name TEXT NOT NULL CHECK (LENGTH(TRIM(name)) > 0),
            description TEXT NOT NULL DEFAULT '',
            kind TEXT NOT NULL DEFAULT 'local',
            network_id TEXT,
            read_security_level INT NOT NULL DEFAULT 0 CHECK (read_security_level >= 0 AND read_security_level <= 255),
            post_security_level INT NOT NULL DEFAULT 10 CHECK (post_security_level >= 0 AND post_security_level <= 255),
            moderated BOOL NOT NULL DEFAULT FALSE,
            enabled BOOL NOT NULL DEFAULT TRUE,
            CHECK (kind = 'local' OR kind = 'echomail' OR kind = 'netmail')
        );

        CREATE TABLE IF NOT EXISTS messages (
            id UUID PRIMARY KEY DEFAULT GEN_RANDOM_UUID(),
            area_id UUID NOT NULL REFERENCES message_areas(id) ON DELETE CASCADE,
            author_user_id UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
            to_user_id UUID REFERENCES users(id) ON DELETE SET NULL,
            subject TEXT NOT NULL CHECK (LENGTH(TRIM(subject)) > 0),
            body TEXT NOT NULL,
            created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
            reply_to_id UUID REFERENCES messages(id) ON DELETE SET NULL,
            network_message_id TEXT,
            visibility TEXT NOT NULL DEFAULT 'normal'
                CHECK (visibility = 'normal' OR visibility = 'deleted' OR visibility = 'hidden')
        );

        CREATE TABLE IF NOT EXISTS sessions (
            id UUID PRIMARY KEY DEFAULT GEN_RANDOM_UUID(),
            node_number INT NOT NULL CHECK (node_number > 0),
            user_id UUID REFERENCES users(id) ON DELETE SET NULL,
            transport TEXT NOT NULL CHECK (transport = 'telnet'),
            remote_address TEXT NOT NULL DEFAULT '',
            remote_ip IPADDR,
            remote_port INT CHECK (remote_port IS NULL OR (remote_port >= 0 AND remote_port <= 65535)),
            started_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
            ended_at TIMESTAMPTZ,
            disconnect_reason TEXT
        );

        CREATE TABLE IF NOT EXISTS doors (
            id UUID PRIMARY KEY DEFAULT GEN_RANDOM_UUID(),
            key TEXT NOT NULL UNIQUE CHECK (LENGTH(TRIM(key)) > 0),
            name TEXT NOT NULL CHECK (LENGTH(TRIM(name)) > 0),
            runner TEXT NOT NULL CHECK (LENGTH(TRIM(runner)) > 0),
            working_dir TEXT NOT NULL CHECK (LENGTH(TRIM(working_dir)) > 0),
            command TEXT NOT NULL CHECK (LENGTH(TRIM(command)) > 0),
            drop_file TEXT NOT NULL CHECK (LENGTH(TRIM(drop_file)) > 0),
            exclusive BOOL NOT NULL DEFAULT FALSE,
            time_limit_minutes INT NOT NULL DEFAULT 30 CHECK (time_limit_minutes > 0),
            enabled BOOL NOT NULL DEFAULT TRUE
        );

        CREATE TABLE IF NOT EXISTS door_runs (
            id UUID PRIMARY KEY DEFAULT GEN_RANDOM_UUID(),
            door_id UUID NOT NULL REFERENCES doors(id) ON DELETE RESTRICT,
            user_id UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
            node_number INT NOT NULL CHECK (node_number > 0),
            started_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
            ended_at TIMESTAMPTZ,
            exit_code INT,
            timed_out BOOL NOT NULL DEFAULT FALSE,
            disconnect_forced BOOL NOT NULL DEFAULT FALSE,
            bytes_in INT NOT NULL DEFAULT 0 CHECK (bytes_in >= 0),
            bytes_out INT NOT NULL DEFAULT 0 CHECK (bytes_out >= 0)
        );

        CREATE INDEX IF NOT EXISTS idx_audit_events_created_at ON audit_events (created_at);
        CREATE INDEX IF NOT EXISTS idx_audit_events_user_id ON audit_events (user_id);
        CREATE INDEX IF NOT EXISTS idx_messages_area_created_at ON messages (area_id, created_at);
        CREATE INDEX IF NOT EXISTS idx_messages_author_user_id ON messages (author_user_id);
        CREATE INDEX IF NOT EXISTS idx_messages_to_user_id ON messages (to_user_id);
        CREATE INDEX IF NOT EXISTS idx_sessions_user_id ON sessions (user_id);
        CREATE INDEX IF NOT EXISTS idx_sessions_started_at ON sessions (started_at);
        CREATE INDEX IF NOT EXISTS idx_door_runs_door_id ON door_runs (door_id);
        CREATE INDEX IF NOT EXISTS idx_door_runs_user_id ON door_runs (user_id);
        CREATE INDEX IF NOT EXISTS idx_door_runs_started_at ON door_runs (started_at);",
    )?;
    set_schema_version(db, SCHEMA_VERSION)?;
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

    parse_schema_version(value)
}

fn parse_schema_version(value: &Value) -> decentdb::Result<i64> {
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

    fn init_schema_2_probe_db(db: &Db) {
        db.execute_batch(
            "CREATE TABLE system_config (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL,
                updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
            );

            CREATE TABLE users (
                id UUID PRIMARY KEY DEFAULT GEN_RANDOM_UUID(),
                alias TEXT NOT NULL UNIQUE CHECK (LENGTH(TRIM(alias)) > 0),
                real_name TEXT NOT NULL CHECK (LENGTH(TRIM(real_name)) > 0),
                email TEXT,
                password_hash TEXT NOT NULL,
                security_level INT NOT NULL DEFAULT 10 CHECK (security_level >= 0 AND security_level <= 255),
                is_sysop BOOL NOT NULL DEFAULT FALSE,
                created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
                last_login_at TIMESTAMPTZ,
                total_calls INT NOT NULL DEFAULT 0 CHECK (total_calls >= 0),
                time_bank_minutes INT NOT NULL DEFAULT 0 CHECK (time_bank_minutes >= 0),
                status TEXT NOT NULL DEFAULT 'active'
                    CHECK (status = 'active' OR status = 'locked' OR status = 'disabled')
            );

            CREATE TABLE message_areas (
                id UUID PRIMARY KEY DEFAULT GEN_RANDOM_UUID(),
                key TEXT NOT NULL UNIQUE CHECK (LENGTH(TRIM(key)) > 0),
                name TEXT NOT NULL CHECK (LENGTH(TRIM(name)) > 0),
                description TEXT NOT NULL DEFAULT '',
                kind TEXT NOT NULL DEFAULT 'local',
                network_id TEXT,
                read_security_level INT NOT NULL DEFAULT 0 CHECK (read_security_level >= 0 AND read_security_level <= 255),
                post_security_level INT NOT NULL DEFAULT 10 CHECK (post_security_level >= 0 AND post_security_level <= 255),
                moderated BOOL NOT NULL DEFAULT FALSE,
                CHECK (kind = 'local' OR kind = 'echomail' OR kind = 'netmail')
            );

            CREATE TABLE messages (
                id UUID PRIMARY KEY DEFAULT GEN_RANDOM_UUID(),
                area_id UUID NOT NULL REFERENCES message_areas(id) ON DELETE CASCADE,
                author_user_id UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
                to_user_id UUID REFERENCES users(id) ON DELETE SET NULL,
                subject TEXT NOT NULL CHECK (LENGTH(TRIM(subject)) > 0),
                body TEXT NOT NULL,
                created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
                reply_to_id UUID REFERENCES messages(id) ON DELETE SET NULL,
                network_message_id TEXT,
                visibility TEXT NOT NULL DEFAULT 'normal'
                    CHECK (visibility = 'normal' OR visibility = 'deleted' OR visibility = 'hidden')
            );

            CREATE INDEX IF NOT EXISTS idx_messages_area_created_at ON messages (area_id, created_at);
            CREATE INDEX IF NOT EXISTS idx_messages_author_user_id ON messages (author_user_id);
            CREATE INDEX IF NOT EXISTS idx_messages_to_user_id ON messages (to_user_id);",
        )
        .expect("create schema-2 probe tables");

        db.execute_with_params(
            "INSERT INTO system_config (key, value) VALUES ($1, $2)",
            &[
                Value::Text("schema_version".to_string()),
                Value::Text("2".to_string()),
            ],
        )
        .expect("seed schema version 2");
    }

    #[test]
    fn schema_initializes_to_current_version() {
        let db = Db::open_or_create(":memory:", DbConfig::default()).expect("open DecentDB");
        init_schema(&db).expect("init schema");
        assert_eq!(schema_version(&db).expect("schema version"), SCHEMA_VERSION);

        let tables = [
            "system_config",
            "users",
            "auth_attempts",
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
        assert_eq!(
            schema_version(&db).expect("read schema version"),
            SCHEMA_VERSION
        );
    }

    #[test]
    fn schema_init_rejects_missing_schema_marker() {
        let db = Db::open_or_create(":memory:", DbConfig::default()).expect("open DecentDB");
        db.execute_batch(
            "CREATE TABLE system_config (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL,
                updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
            );",
        )
        .expect("create system_config");

        let err = init_schema(&db).expect_err("init should reject missing marker");
        assert!(err.to_string().contains("schema_version marker is missing"));
    }

    #[test]
    fn schema_init_rejects_tables_without_schema_marker() {
        let db = Db::open_or_create(":memory:", DbConfig::default()).expect("open DecentDB");
        db.execute_batch(
            "CREATE TABLE users (
                id UUID PRIMARY KEY DEFAULT GEN_RANDOM_UUID(),
                alias TEXT NOT NULL UNIQUE CHECK (LENGTH(TRIM(alias)) > 0),
                real_name TEXT NOT NULL CHECK (LENGTH(TRIM(real_name)) > 0),
                password_hash TEXT NOT NULL
            );",
        )
        .expect("create user table");

        let err = init_schema(&db).expect_err("init should reject unmarked existing tables");
        assert!(err.to_string().contains("found existing database tables"));
    }

    #[test]
    fn schema_init_rejects_newer_schema_marker() {
        let db = Db::open_or_create(":memory:", DbConfig::default()).expect("open DecentDB");
        db.execute_batch(
            "CREATE TABLE system_config (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL,
                updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
            );",
        )
        .expect("create system_config");
        db.execute_with_params(
            "INSERT INTO system_config (key, value) VALUES ($1, $2)",
            &[
                Value::Text("schema_version".to_string()),
                Value::Text("999".to_string()),
            ],
        )
        .expect("seed future schema marker");

        let err = init_schema(&db).expect_err("init should reject future marker");
        assert!(err.to_string().contains("newer than supported version"));
    }

    #[test]
    fn schema_init_migrates_schema_2_to_current() {
        let db = Db::open_or_create(":memory:", DbConfig::default()).expect("open DecentDB");

        init_schema_2_probe_db(&db);
        db.execute_with_params(
            "INSERT INTO message_areas (key, name, kind) VALUES ($1, $2, $3)",
            &[
                Value::Text("general".to_string()),
                Value::Text("General".to_string()),
                Value::Text("local".to_string()),
            ],
        )
        .expect("seed schema-2 area");

        init_schema(&db).expect("apply migrations");
        assert_eq!(schema_version(&db).expect("schema version"), SCHEMA_VERSION);

        let enabled = {
            let key = Value::Text("general".to_string());
            let result = db
                .execute_with_params("SELECT enabled FROM message_areas WHERE key = $1", &[key])
                .expect("enabled value");
            let row = result
                .rows()
                .first()
                .and_then(|row| row.values().first())
                .expect("enabled column");
            match row {
                Value::Bool(enabled) => *enabled,
                _ => false,
            }
        };
        assert!(enabled);
    }

    #[test]
    fn schema_uses_native_decentdb_types_and_foreign_keys() {
        let db = Db::open_or_create(":memory:", DbConfig::default()).expect("open DecentDB");
        init_schema(&db).expect("init schema");

        let columns = db
            .execute(
                "SELECT column_name, data_type
                 FROM information_schema.columns
                 WHERE table_name = 'sessions'
                 ORDER BY column_name",
            )
            .expect("columns");
        let pairs: Vec<(String, String)> = columns
            .rows()
            .iter()
            .map(|row| {
                let values = row.values();
                let name = match &values[0] {
                    Value::Text(value) => value.clone(),
                    other => panic!("unexpected column name value {other:?}"),
                };
                let data_type = match &values[1] {
                    Value::Text(value) => value.clone(),
                    other => panic!("unexpected data type value {other:?}"),
                };
                (name, data_type)
            })
            .collect();

        assert!(pairs.contains(&("id".to_string(), "UUID".to_string())));
        assert!(pairs.contains(&("remote_ip".to_string(), "IPADDR".to_string())));
        assert!(pairs.contains(&("started_at".to_string(), "TIMESTAMPTZ".to_string())));

        let foreign_keys = db
            .execute("SELECT * FROM pragma_foreign_key_list('messages')")
            .expect("foreign keys");
        assert!(
            foreign_keys.rows().len() >= 4,
            "messages should declare area/user/reply foreign keys"
        );
    }
}
