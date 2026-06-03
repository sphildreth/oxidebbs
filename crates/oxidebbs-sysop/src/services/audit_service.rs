use crate::SysopError;
use oxidebbs_db::{
    AuditEventRecord, Db, insert_audit_event, list_audit_events, list_audit_events_for_user,
};

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

    pub fn record(
        db: &Db,
        event_type: &str,
        user_id: Option<&str>,
        node_number: Option<i64>,
        details: &str,
    ) -> Result<(), SysopError> {
        insert_audit_event(
            db,
            &AuditEventRecord {
                id: String::new(),
                created_at: String::new(),
                event_type: event_type.to_string(),
                user_id: user_id.map(ToOwned::to_owned),
                node_number,
                details: details.to_string(),
            },
        )?;
        Ok(())
    }
}
