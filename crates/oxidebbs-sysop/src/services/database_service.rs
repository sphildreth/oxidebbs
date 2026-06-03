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
        let result = db.execute("SELECT COUNT(*) FROM audit_events")?;
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
