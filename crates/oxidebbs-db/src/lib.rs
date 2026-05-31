//! DecentDB repository layer.

use std::path::Path;

pub use decentdb::{Db, DbConfig, DbError, QueryResult, QueryRow, Value};

pub const SCHEMA_VERSION: i64 = 1;

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
        init_schema(&db)?;
        Ok(Self { db })
    }

    pub fn schema_version(&self) -> decentdb::Result<i64> {
        schema_version(&self.db)
    }

    pub fn db(&self) -> &Db {
        &self.db
    }

    pub fn into_inner(self) -> Db {
        self.db
    }
}

pub fn init_schema(db: &Db) -> decentdb::Result<()> {
    db.execute(
        "CREATE TABLE IF NOT EXISTS system_config (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        )",
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
        .ok_or_else(|| DbError::sql("OxideBBS schema_version is missing"))?;

    match value {
        Value::Text(raw) => raw.parse::<i64>().map_err(|error| {
            DbError::sql(format!("invalid OxideBBS schema version {raw:?}: {error}"))
        }),
        other => Err(DbError::sql(format!(
            "invalid OxideBBS schema_version value: {other:?}"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opens_memory_database_and_initializes_schema_marker() {
        let db = OxideDb::open_memory().expect("open in-memory DecentDB");

        assert_eq!(db.schema_version().expect("read schema version"), 1);
    }

    #[test]
    fn init_schema_is_idempotent() {
        let db = Db::open_or_create(":memory:", DbConfig::default()).expect("open DecentDB");

        init_schema(&db).expect("first schema init");
        init_schema(&db).expect("second schema init");

        assert_eq!(schema_version(&db).expect("read schema version"), 1);
    }
}
