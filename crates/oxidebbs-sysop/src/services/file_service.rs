use crate::SysopError;
use crate::services::audit_service::AuditService;
use oxidebbs_db::{
    Db, FileAreaRecord, FileEntryRecord, FileTransferRecord, list_file_areas, list_file_entries,
    list_file_transfers, update_file_area, update_file_entry_approved,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileDashboard {
    pub areas: Vec<FileAreaRecord>,
    pub entries: Vec<FileEntryRecord>,
    pub transfers: Vec<FileTransferRecord>,
    pub enabled_areas: usize,
    pub pending_entries: usize,
    pub failed_transfers: usize,
}

pub struct FileAdminService;

impl FileAdminService {
    pub fn load(db: &Db) -> Result<FileDashboard, SysopError> {
        let areas = list_file_areas(db)?;
        let entries = list_file_entries(db)?;
        let transfers = list_file_transfers(db)?;

        Ok(FileDashboard {
            enabled_areas: areas.iter().filter(|area| area.enabled).count(),
            pending_entries: entries.iter().filter(|entry| !entry.approved).count(),
            failed_transfers: transfers
                .iter()
                .filter(|transfer| transfer.outcome != "success")
                .count(),
            areas,
            entries,
            transfers,
        })
    }

    pub fn set_area_enabled(db: &Db, area_id: &str, enabled: bool) -> Result<(), SysopError> {
        let mut area = list_file_areas(db)?
            .into_iter()
            .find(|area| area.id == area_id)
            .ok_or_else(|| SysopError::Message(format!("file area {area_id} was not found")))?;
        area.enabled = enabled;
        update_file_area(db, &area)?;
        AuditService::record(
            db,
            if enabled {
                "file_area_enabled"
            } else {
                "file_area_disabled"
            },
            None,
            None,
            &format!("area_id={} key={}", area.id, area.key),
        )?;
        Ok(())
    }

    pub fn set_entry_approved(db: &Db, entry_id: &str, approved: bool) -> Result<(), SysopError> {
        update_file_entry_approved(db, entry_id, approved)?;
        AuditService::record(
            db,
            if approved {
                "file_entry_approved"
            } else {
                "file_entry_unapproved"
            },
            None,
            None,
            &format!("entry_id={entry_id}"),
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::FileAdminService;
    use oxidebbs_db::{
        FileAreaRecord, FileEntryRecord, OxideDb, find_file_entry_by_id, insert_file_area,
        insert_file_entry, list_audit_events, list_file_areas,
    };

    fn area() -> FileAreaRecord {
        FileAreaRecord {
            id: String::new(),
            key: "main".to_string(),
            name: "Main Files".to_string(),
            description: "Main file area".to_string(),
            root_path: "/tmp/oxidebbs-main-files".to_string(),
            read_security_level: 0,
            download_security_level: 10,
            upload_security_level: 20,
            max_upload_bytes: Some(1_048_576),
            enabled: true,
            created_at: String::new(),
            updated_at: String::new(),
        }
    }

    fn entry(area_id: String) -> FileEntryRecord {
        FileEntryRecord {
            id: String::new(),
            area_id,
            storage_name: "demo.zip".to_string(),
            display_name: "Demo ZIP".to_string(),
            original_name: Some("demo.zip".to_string()),
            size_bytes: 1024,
            content_crc32: Some("1234ABCD".to_string()),
            description: "Demo archive".to_string(),
            uploader_user_id: None,
            download_count: 0,
            approved: false,
            created_at: String::new(),
            updated_at: String::new(),
        }
    }

    #[test]
    fn load_counts_pending_entries_and_mutations_audit() {
        let db = OxideDb::open_memory().expect("open db");
        insert_file_area(db.db(), &area()).expect("insert area");
        let area_id = list_file_areas(db.db()).expect("areas")[0].id.clone();
        insert_file_entry(db.db(), &entry(area_id.clone())).expect("insert entry");
        let dashboard = FileAdminService::load(db.db()).expect("load files dashboard");
        assert_eq!(dashboard.areas.len(), 1);
        assert_eq!(dashboard.enabled_areas, 1);
        assert_eq!(dashboard.pending_entries, 1);

        FileAdminService::set_area_enabled(db.db(), &area_id, false).expect("disable area");
        let updated_area = list_file_areas(db.db()).expect("areas")[0].clone();
        assert!(!updated_area.enabled);

        let entry_id = dashboard.entries[0].id.clone();
        FileAdminService::set_entry_approved(db.db(), &entry_id, true).expect("approve entry");
        let updated_entry = find_file_entry_by_id(db.db(), &entry_id)
            .expect("find entry")
            .expect("entry exists");
        assert!(updated_entry.approved);

        let events = list_audit_events(db.db(), 10).expect("audit");
        assert!(
            events
                .iter()
                .any(|event| event.event_type == "file_area_disabled")
        );
        assert!(
            events
                .iter()
                .any(|event| event.event_type == "file_entry_approved")
        );
    }
}
