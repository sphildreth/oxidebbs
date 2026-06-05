use crate::SysopError;
use oxidebbs_db::{
    AuditEventRecord, Db, insert_audit_event, list_audit_events, list_audit_events_for_user,
};
use std::fs;
use std::path::Path;

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

    pub fn export(events: &[AuditEventRecord], output: &Path) -> Result<(), SysopError> {
        if let Some(parent) = output.parent()
            && !parent.as_os_str().is_empty()
        {
            fs::create_dir_all(parent)?;
        }
        let mut text = String::from("created_at\tevent_type\tuser_id\tnode_number\tdetails\n");
        for event in events {
            text.push_str(&format!(
                "{}\t{}\t{}\t{}\t{}\n",
                event.created_at,
                event.event_type,
                event.user_id.as_deref().unwrap_or(""),
                event
                    .node_number
                    .map(|node| node.to_string())
                    .unwrap_or_default(),
                event.details.replace(['\t', '\n', '\r'], " ")
            ));
        }
        fs::write(output, text)?;
        Ok(())
    }
}
