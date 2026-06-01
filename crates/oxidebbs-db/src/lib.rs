//! DecentDB repository layer.

use std::path::Path;

pub use decentdb::{Db, DbConfig, DbError, QueryResult, QueryRow, Value};

mod audit_repo;
mod door_repo;
mod message_repo;
mod migrations;
mod schema;
mod session_repo;
mod user_repo;

pub const SCHEMA_VERSION: i64 = 3;

pub use audit_repo::{
    AuditEventRecord, insert_audit_event, list_audit_events, list_audit_events_for_user,
};
pub use door_repo::{
    DoorDefinitionRecord, DoorRunFinish, DoorRunRecord, find_door_by_key, find_door_run_by_id,
    finish_door_run, insert_door_definition, insert_door_run, list_door_definitions,
    list_door_runs, update_door_enabled,
};
pub use message_repo::{
    MessageAreaRecord, MessageRecord, find_message_area_by_key, find_message_by_id, insert_message,
    insert_message_area, list_message_areas, list_messages, list_messages_in_area,
    move_message_to_area, update_message_area_enabled, update_message_area_levels,
    update_message_visibility,
};
pub use migrations::migrate_to_current;
pub use schema::schema_version as read_schema_version;
pub use session_repo::{
    SessionRecord, end_session, find_active_session_by_node, insert_session, list_active_sessions,
    list_recent_sessions, update_session_user,
};
pub use user_repo::{
    UserRecord, find_user_by_alias, find_user_by_alias_ci, find_user_by_id, insert_user,
    list_users, update_user_alias, update_user_is_sysop, update_user_login,
    update_user_password_hash, update_user_security_level, update_user_status,
};

pub struct OxideDb {
    db: Db,
}

impl OxideDb {
    pub fn open_or_create(path: impl AsRef<Path>) -> decentdb::Result<Self> {
        Self::open_or_create_with_config(path, DbConfig::default())
    }

    pub fn open_memory() -> decentdb::Result<Self> {
        Self::open_or_create(":memory:")
    }

    pub fn open_or_create_with_config(
        path: impl AsRef<Path>,
        config: DbConfig,
    ) -> decentdb::Result<Self> {
        let db = Db::open_or_create(path, config)?;
        schema::init_schema(&db)?;
        Ok(Self { db })
    }

    pub fn schema_version(&self) -> decentdb::Result<i64> {
        schema::schema_version(&self.db)
    }

    pub fn db(&self) -> &Db {
        &self.db
    }

    pub fn into_inner(self) -> Db {
        self.db
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opens_memory_database_and_initializes_schema_marker() {
        let db = OxideDb::open_memory().expect("open in-memory DecentDB");

        assert_eq!(db.schema_version().expect("read schema version"), 3);
    }

    #[test]
    fn init_schema_is_idempotent() {
        let db = Db::open_or_create(":memory:", DbConfig::default()).expect("open DecentDB");

        schema::init_schema(&db).expect("first schema init");
        schema::init_schema(&db).expect("second schema init");

        assert_eq!(schema::schema_version(&db).expect("read schema version"), 3);
    }
}
