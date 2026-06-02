use decentdb::{Db, DbError};

use crate::SCHEMA_VERSION;
use crate::schema::{existing_schema_version, set_schema_version};

pub fn migrate_to_current(db: &Db) -> decentdb::Result<()> {
    let mut version = existing_schema_version(db)?
        .ok_or_else(|| DbError::sql("OxideBBS schema_version marker is missing"))?;
    if version > SCHEMA_VERSION {
        return Err(DbError::sql(format!(
            "OxideBBS database schema version {version} is newer than supported version {SCHEMA_VERSION}"
        )));
    }

    while version < SCHEMA_VERSION {
        match version {
            2 => migrate_2_to_3(db)?,
            3 => migrate_3_to_4(db)?,
            unknown => {
                return Err(DbError::sql(format!(
                    "Unsupported migration source schema version {unknown}; expected {SCHEMA_VERSION} or older known versions"
                )));
            }
        }
        version = existing_schema_version(db)?
            .ok_or_else(|| DbError::sql("OxideBBS schema_version marker is missing"))?;
    }

    Ok(())
}

fn migrate_2_to_3(db: &Db) -> decentdb::Result<()> {
    match existing_schema_version(db)? {
        Some(2) => {
            run_migration_transaction(db, || {
                rebuild_message_area_tables_for_v3(db)?;
                set_schema_version(db, 3)
            })?;
            Ok(())
        }
        Some(other) => Err(DbError::sql(format!(
            "Cannot apply migration 2 -> 3 from schema version {other}"
        ))),
        None => Err(DbError::sql(
            "Cannot apply migration 2 -> 3 because schema_version marker is missing",
        )),
    }
}

fn migrate_3_to_4(db: &Db) -> decentdb::Result<()> {
    match existing_schema_version(db)? {
        Some(3) => {
            run_migration_transaction(db, || {
                ensure_v3_tables_for_user_rebuild(db)?;
                rebuild_user_related_tables_for_v4(db)?;
                create_auth_attempts_table(db)?;
                set_schema_version(db, 4)
            })?;
            Ok(())
        }
        Some(other) => Err(DbError::sql(format!(
            "Cannot apply migration 3 -> 4 from schema version {other}"
        ))),
        None => Err(DbError::sql(
            "Cannot apply migration 3 -> 4 because schema_version marker is missing",
        )),
    }
}

fn ensure_v3_tables_for_user_rebuild(db: &Db) -> decentdb::Result<()> {
    db.execute_batch(
        "CREATE TABLE IF NOT EXISTS audit_events (
            id UUID PRIMARY KEY DEFAULT GEN_RANDOM_UUID(),
            created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
            event_type TEXT NOT NULL CHECK (LENGTH(TRIM(event_type)) > 0),
            user_id UUID REFERENCES users(id) ON DELETE SET NULL,
            node_number INT CHECK (node_number IS NULL OR node_number > 0),
            details TEXT NOT NULL DEFAULT ''
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
        CREATE INDEX IF NOT EXISTS idx_sessions_user_id ON sessions (user_id);
        CREATE INDEX IF NOT EXISTS idx_sessions_started_at ON sessions (started_at);
        CREATE INDEX IF NOT EXISTS idx_door_runs_door_id ON door_runs (door_id);
        CREATE INDEX IF NOT EXISTS idx_door_runs_user_id ON door_runs (user_id);
        CREATE INDEX IF NOT EXISTS idx_door_runs_started_at ON door_runs (started_at);",
    )?;
    Ok(())
}

fn rebuild_user_related_tables_for_v4(db: &Db) -> decentdb::Result<()> {
    db.execute_batch(
        "ALTER TABLE audit_events RENAME TO oxidebbs_schema3_audit_events;
        ALTER TABLE messages RENAME TO oxidebbs_schema3_messages;
        ALTER TABLE sessions RENAME TO oxidebbs_schema3_sessions;
        ALTER TABLE door_runs RENAME TO oxidebbs_schema3_door_runs;
        ALTER TABLE users RENAME TO oxidebbs_schema3_users;

        DROP INDEX IF EXISTS idx_audit_events_created_at;
        DROP INDEX IF EXISTS idx_audit_events_user_id;
        DROP INDEX IF EXISTS idx_messages_area_created_at;
        DROP INDEX IF EXISTS idx_messages_author_user_id;
        DROP INDEX IF EXISTS idx_messages_to_user_id;
        DROP INDEX IF EXISTS idx_sessions_user_id;
        DROP INDEX IF EXISTS idx_sessions_started_at;
        DROP INDEX IF EXISTS idx_door_runs_door_id;
        DROP INDEX IF EXISTS idx_door_runs_user_id;
        DROP INDEX IF EXISTS idx_door_runs_started_at;

        CREATE TABLE users_v4 (
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

        INSERT INTO users_v4 (
            id, alias, alias_normalized, real_name, email, password_hash,
            security_level, is_sysop, created_at, last_login_at, total_calls,
            time_bank_minutes, status
        )
        SELECT id, alias, LOWER(TRIM(alias)), real_name, email, password_hash,
               security_level, is_sysop, created_at, last_login_at, total_calls,
               time_bank_minutes, status
        FROM oxidebbs_schema3_users;

        ALTER TABLE users_v4 RENAME TO users;

        CREATE TABLE audit_events_v4 (
            id UUID PRIMARY KEY DEFAULT GEN_RANDOM_UUID(),
            created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
            event_type TEXT NOT NULL CHECK (LENGTH(TRIM(event_type)) > 0),
            user_id UUID REFERENCES users(id) ON DELETE SET NULL,
            node_number INT CHECK (node_number IS NULL OR node_number > 0),
            details TEXT NOT NULL DEFAULT ''
        );

        CREATE TABLE messages_v4 (
            id UUID PRIMARY KEY DEFAULT GEN_RANDOM_UUID(),
            area_id UUID NOT NULL REFERENCES message_areas(id) ON DELETE CASCADE,
            author_user_id UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
            to_user_id UUID REFERENCES users(id) ON DELETE SET NULL,
            subject TEXT NOT NULL CHECK (LENGTH(TRIM(subject)) > 0),
            body TEXT NOT NULL,
            created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
            reply_to_id UUID REFERENCES messages_v4(id) ON DELETE SET NULL,
            network_message_id TEXT,
            visibility TEXT NOT NULL DEFAULT 'normal'
                CHECK (visibility = 'normal' OR visibility = 'deleted' OR visibility = 'hidden')
        );

        CREATE TABLE sessions_v4 (
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

        CREATE TABLE door_runs_v4 (
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

        INSERT INTO audit_events_v4 (id, created_at, event_type, user_id, node_number, details)
        SELECT id, created_at, event_type, user_id, node_number, details
        FROM oxidebbs_schema3_audit_events;

        INSERT INTO messages_v4 (
            id, area_id, author_user_id, to_user_id, subject, body, created_at,
            reply_to_id, network_message_id, visibility
        )
        SELECT id, area_id, author_user_id, to_user_id, subject, body, created_at,
               NULL, network_message_id, visibility
        FROM oxidebbs_schema3_messages;

        UPDATE messages_v4
        SET reply_to_id = (
            SELECT oxidebbs_schema3_messages.reply_to_id
            FROM oxidebbs_schema3_messages
            WHERE oxidebbs_schema3_messages.id = messages_v4.id
        );

        INSERT INTO sessions_v4 (
            id, node_number, user_id, transport, remote_address, remote_ip,
            remote_port, started_at, ended_at, disconnect_reason
        )
        SELECT id, node_number, user_id, transport, remote_address, remote_ip,
               remote_port, started_at, ended_at, disconnect_reason
        FROM oxidebbs_schema3_sessions;

        INSERT INTO door_runs_v4 (
            id, door_id, user_id, node_number, started_at, ended_at, exit_code,
            timed_out, disconnect_forced, bytes_in, bytes_out
        )
        SELECT id, door_id, user_id, node_number, started_at, ended_at, exit_code,
               timed_out, disconnect_forced, bytes_in, bytes_out
        FROM oxidebbs_schema3_door_runs;

        ALTER TABLE audit_events_v4 RENAME TO audit_events;
        ALTER TABLE messages_v4 RENAME TO messages;
        ALTER TABLE sessions_v4 RENAME TO sessions;
        ALTER TABLE door_runs_v4 RENAME TO door_runs;

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
    Ok(())
}

fn create_auth_attempts_table(db: &Db) -> decentdb::Result<()> {
    db.execute_batch(
        "CREATE TABLE IF NOT EXISTS auth_attempts (
            scope TEXT NOT NULL CHECK (scope = 'ip' OR scope = 'alias'),
            scope_key TEXT NOT NULL CHECK (LENGTH(TRIM(scope_key)) > 0),
            failed_count INT NOT NULL DEFAULT 0 CHECK (failed_count >= 0),
            first_failed_at TIMESTAMPTZ,
            last_failed_at TIMESTAMPTZ,
            locked_until TIMESTAMPTZ,
            PRIMARY KEY (scope, scope_key)
        );",
    )?;
    Ok(())
}

fn rebuild_message_area_tables_for_v3(db: &Db) -> decentdb::Result<()> {
    // DecentDB 2.8.0 cannot drop the renamed schema-2 messages table because
    // its self-referential foreign key still points at itself, and unnamed
    // inline foreign keys cannot be removed with ALTER TABLE DROP CONSTRAINT.
    // Keep the old tables under explicit archive names outside runtime query
    // paths, then move freshly built v3 tables into the canonical names.
    db.execute_batch(
        "ALTER TABLE messages RENAME TO oxidebbs_schema2_messages;
        ALTER TABLE message_areas RENAME TO oxidebbs_schema2_message_areas;

        CREATE TABLE message_areas_v3 (
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

        CREATE TABLE messages_v3 (
            id UUID PRIMARY KEY DEFAULT GEN_RANDOM_UUID(),
            area_id UUID NOT NULL REFERENCES message_areas_v3(id) ON DELETE CASCADE,
            author_user_id UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
            to_user_id UUID REFERENCES users(id) ON DELETE SET NULL,
            subject TEXT NOT NULL CHECK (LENGTH(TRIM(subject)) > 0),
            body TEXT NOT NULL,
            created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
            reply_to_id UUID REFERENCES messages_v3(id) ON DELETE SET NULL,
            network_message_id TEXT,
            visibility TEXT NOT NULL DEFAULT 'normal'
                CHECK (visibility = 'normal' OR visibility = 'deleted' OR visibility = 'hidden')
        );

        INSERT INTO message_areas_v3 (
            id, key, name, description, kind, network_id,
            read_security_level, post_security_level, moderated, enabled
        )
        SELECT id, key, name, description, kind, network_id, read_security_level,
               post_security_level, moderated, TRUE
        FROM oxidebbs_schema2_message_areas;

        INSERT INTO messages_v3 (
            id, area_id, author_user_id, to_user_id, subject, body, created_at,
            reply_to_id, network_message_id, visibility
        )
        SELECT id, area_id, author_user_id, to_user_id, subject, body, created_at,
               NULL, network_message_id, visibility
        FROM oxidebbs_schema2_messages;

        UPDATE messages_v3
        SET reply_to_id = (
            SELECT oxidebbs_schema2_messages.reply_to_id
            FROM oxidebbs_schema2_messages
            WHERE oxidebbs_schema2_messages.id = messages_v3.id
        );

        DROP INDEX IF EXISTS idx_messages_area_created_at;
        DROP INDEX IF EXISTS idx_messages_author_user_id;
        DROP INDEX IF EXISTS idx_messages_to_user_id;

        ALTER TABLE message_areas_v3 RENAME TO message_areas;
        ALTER TABLE messages_v3 RENAME TO messages;

        CREATE INDEX IF NOT EXISTS idx_messages_area_created_at ON messages (area_id, created_at);
        CREATE INDEX IF NOT EXISTS idx_messages_author_user_id ON messages (author_user_id);
        CREATE INDEX IF NOT EXISTS idx_messages_to_user_id ON messages (to_user_id);",
    )?;
    Ok(())
}

fn run_migration_transaction(
    db: &Db,
    migration: impl FnOnce() -> decentdb::Result<()>,
) -> decentdb::Result<()> {
    db.begin_transaction()?;
    match migration() {
        Ok(()) => {
            db.commit_transaction()?;
            Ok(())
        }
        Err(error) => {
            let _ = db.rollback_transaction();
            Err(error)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema;
    use decentdb::{DbConfig, Value};

    const USER_1: &str = "00000000-0000-4000-8000-000000000011";
    const AREA_GENERAL: &str = "00000000-0000-4000-8000-000000000101";
    const MSG_1: &str = "00000000-0000-4000-8000-000000000201";
    const MSG_2: &str = "00000000-0000-4000-8000-000000000202";

    fn seed_schema_2_database(db: &Db) {
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
            CREATE INDEX IF NOT EXISTS idx_messages_to_user_id ON messages (to_user_id);

            INSERT INTO system_config (key, value) VALUES ('schema_version', '2');",
        )
        .expect("seed schema-2 database");

        db.execute_with_params(
            "INSERT INTO users (id, alias, real_name, password_hash, created_at)
             VALUES (UUID_PARSE($1), $2, $3, $4, $5)",
            &[
                Value::Text(USER_1.to_string()),
                Value::Text("alice".to_string()),
                Value::Text("Alice User".to_string()),
                Value::Text("hashed".to_string()),
                Value::Text("2026-01-01T00:00:00.000000Z".to_string()),
            ],
        )
        .expect("insert user");

        db.execute_with_params(
            "INSERT INTO message_areas (id, key, name, kind)
             VALUES (UUID_PARSE($1), $2, $3, $4)",
            &[
                Value::Text(AREA_GENERAL.to_string()),
                Value::Text("general".to_string()),
                Value::Text("General".to_string()),
                Value::Text("local".to_string()),
            ],
        )
        .expect("insert pre-migration message area");

        db.execute_with_params(
            "INSERT INTO messages (id, area_id, author_user_id, subject, body, created_at)
             VALUES (UUID_PARSE($1), UUID_PARSE($2), UUID_PARSE($3), $4, $5, $6)",
            &[
                Value::Text(MSG_1.to_string()),
                Value::Text(AREA_GENERAL.to_string()),
                Value::Text(USER_1.to_string()),
                Value::Text("First".to_string()),
                Value::Text("Body".to_string()),
                Value::Text("2026-01-01T00:00:00.000000Z".to_string()),
            ],
        )
        .expect("insert message");

        db.execute_with_params(
            "INSERT INTO messages (id, area_id, author_user_id, subject, body, created_at, reply_to_id)
             VALUES (UUID_PARSE($1), UUID_PARSE($2), UUID_PARSE($3), $4, $5, $6, UUID_PARSE($7))",
            &[
                Value::Text(MSG_2.to_string()),
                Value::Text(AREA_GENERAL.to_string()),
                Value::Text(USER_1.to_string()),
                Value::Text("Reply".to_string()),
                Value::Text("Body".to_string()),
                Value::Text("2026-01-01T00:01:00.000000Z".to_string()),
                Value::Text(MSG_1.to_string()),
            ],
        )
        .expect("insert reply");
    }

    #[test]
    fn migration_2_to_current_updates_schema_marker_and_enabled_column() {
        let db = Db::open_or_create(":memory:", DbConfig::default()).expect("open DecentDB");
        seed_schema_2_database(&db);

        migrate_to_current(&db).expect("apply migration");

        assert_eq!(
            schema::schema_version(&db).expect("schema version"),
            SCHEMA_VERSION
        );
        let enabled = {
            let result = db
                .execute("SELECT enabled FROM message_areas WHERE key = 'general'")
                .expect("query enabled");
            let row = result
                .rows()
                .first()
                .and_then(|row| row.values().first())
                .expect("enabled exists");
            match row {
                Value::Bool(enabled) => *enabled,
                _ => false,
            }
        };
        assert!(enabled);

        let reply_to = {
            let result = db
                .execute("SELECT UUID_TO_STRING(reply_to_id) FROM messages WHERE id = UUID_PARSE('00000000-0000-4000-8000-000000000202')")
                .expect("query reply");
            let row = result
                .rows()
                .first()
                .and_then(|row| row.values().first())
                .expect("reply_to exists");
            match row {
                Value::Text(value) => value.clone(),
                _ => String::new(),
            }
        };
        assert_eq!(reply_to, MSG_1);

        let alias_normalized = {
            let result = db
                .execute("SELECT alias_normalized FROM users WHERE alias = 'alice'")
                .expect("query normalized alias");
            result
                .rows()
                .first()
                .and_then(|row| row.values().first())
                .cloned()
        };
        assert_eq!(alias_normalized, Some(Value::Text("alice".to_string())));

        db.execute("SELECT * FROM auth_attempts LIMIT 0")
            .expect("auth attempts table exists");
    }

    #[test]
    fn migration_runner_rejects_future_versions() {
        let db = Db::open_or_create(":memory:", DbConfig::default()).expect("open DecentDB");
        db.execute_batch(
            "CREATE TABLE system_config (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL,
                updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
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
            INSERT INTO system_config (key, value) VALUES ('schema_version', '3');",
        )
        .expect("seed schema 3 database");
        db.execute_with_params(
            "UPDATE system_config SET value = '99' WHERE key = 'schema_version'",
            &[],
        )
        .expect("set future marker");

        let err = migrate_to_current(&db).expect_err("migration should reject future version");
        assert!(err.to_string().contains("newer than supported version"));
    }

    #[test]
    fn direct_alter_add_column_is_rejected_for_checked_table() {
        let db = Db::open_or_create(":memory:", DbConfig::default()).expect("open DecentDB");
        db.execute_batch(
            "CREATE TABLE message_areas (
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
            );",
        )
        .expect("create probe table");

        let error = db
            .execute_batch(
                "ALTER TABLE message_areas
             ADD COLUMN enabled BOOL NOT NULL DEFAULT TRUE;",
            )
            .expect_err("ALTER TABLE ADD COLUMN is rejected for checked tables");

        assert!(error.to_string().contains("CHECK constraints"));
    }

    #[test]
    fn table_rebuild_strategy_handles_checked_table_and_fk_data() {
        let db = Db::open_or_create(":memory:", DbConfig::default()).expect("open DecentDB");
        seed_schema_2_database(&db);

        rebuild_message_area_tables_for_v3(&db).expect("rebuild tables");

        let count = db
            .execute("SELECT COUNT(*) FROM messages")
            .expect("count messages")
            .rows()
            .first()
            .and_then(|row| row.values().first())
            .cloned();
        assert_eq!(count, Some(Value::Int64(2)));

        let archive_count = db
            .list_tables()
            .expect("list tables")
            .into_iter()
            .filter(|table| {
                table.name == "oxidebbs_schema2_message_areas"
                    || table.name == "oxidebbs_schema2_messages"
            })
            .count();
        assert_eq!(archive_count, 2);
    }
}
