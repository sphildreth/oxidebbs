use crate::SysopError;
use oxidebbs_db::{Db, read_schema_version};

pub struct DatabaseAdminService;

impl DatabaseAdminService {
    pub fn schema_version(db: &Db) -> Result<i64, SysopError> {
        Ok(read_schema_version(db)?)
    }

    pub fn count_users(db: &Db) -> Result<i64, SysopError> {
        let result = db.execute("SELECT COUNT(*) FROM users")?;
        Ok(result
            .rows()
            .first()
            .and_then(|row| row.values().first())
            .and_then(|v| match v {
                oxidebbs_db::Value::Int64(n) => Some(*n),
                _ => None,
            })
            .unwrap_or(0))
    }

    pub fn count_messages(db: &Db) -> Result<i64, SysopError> {
        let result = db.execute("SELECT COUNT(*) FROM messages")?;
        Ok(result
            .rows()
            .first()
            .and_then(|row| row.values().first())
            .and_then(|v| match v {
                oxidebbs_db::Value::Int64(n) => Some(*n),
                _ => None,
            })
            .unwrap_or(0))
    }

    pub fn count_audit_events(db: &Db) -> Result<i64, SysopError> {
        Self::count_table(db, "audit_events")
    }

    pub fn count_sessions(db: &Db) -> Result<i64, SysopError> {
        Self::count_table(db, "sessions")
    }

    pub fn count_doors(db: &Db) -> Result<i64, SysopError> {
        Self::count_table(db, "doors")
    }

    pub fn count_door_runs(db: &Db) -> Result<i64, SysopError> {
        Self::count_table(db, "door_runs")
    }

    fn count_table(db: &Db, table: &str) -> Result<i64, SysopError> {
        let result = db.execute(&format!("SELECT COUNT(*) FROM {table}"))?;
        Ok(result
            .rows()
            .first()
            .and_then(|row| row.values().first())
            .and_then(|v| match v {
                oxidebbs_db::Value::Int64(n) => Some(*n),
                _ => None,
            })
            .unwrap_or(0))
    }
}
