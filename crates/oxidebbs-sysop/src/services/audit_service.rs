use crate::SysopError;
use oxidebbs_db::{AuditEventRecord, Db, list_audit_events, list_audit_events_for_user};

pub struct AuditService;

impl AuditService {
    pub fn recent(db: &Db, limit: i64) -> Result<Vec<AuditEventRecord>, SysopError> {
        Ok(list_audit_events(db, limit)?)
    }

    pub fn for_user(
        db: &Db,
        user_id: &str,
        limit: i64,
    ) -> Result<Vec<AuditEventRecord>, SysopError> {
        Ok(list_audit_events_for_user(db, user_id, limit)?)
    }
}
