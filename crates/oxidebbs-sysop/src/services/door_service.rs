use crate::SysopError;
use crate::services::audit_service::AuditService;
use oxidebbs_db::{
    Db, DoorDefinitionRecord, DoorRunRecord, find_door_by_key, insert_door_definition,
    list_door_definitions, list_door_runs, update_door_definition, update_door_enabled,
};
use oxidebbs_door::parse_doors_toml;

pub struct DoorAdminService;

impl DoorAdminService {
    pub fn list(db: &Db) -> Result<Vec<DoorDefinitionRecord>, SysopError> {
        Ok(list_door_definitions(db)?)
    }

    pub fn find(db: &Db, key: &str) -> Result<Option<DoorDefinitionRecord>, SysopError> {
        Ok(find_door_by_key(db, key)?)
    }

    pub fn list_runs(db: &Db, limit: i64) -> Result<Vec<DoorRunRecord>, SysopError> {
        Ok(list_door_runs(db, limit)?)
    }

    pub fn set_enabled(
        db: &Db,
        door_id: &str,
        door_key: &str,
        enabled: bool,
    ) -> Result<(), SysopError> {
        update_door_enabled(db, door_id, enabled)?;
        AuditService::record(
            db,
            if enabled {
                "door_enabled"
            } else {
                "door_disabled"
            },
            None,
            None,
            &format!("door={door_key} enabled={enabled}"),
        )?;
        Ok(())
    }

    pub fn insert_door(db: &Db, door: &DoorDefinitionRecord) -> Result<(), SysopError> {
        insert_door_definition(db, door)?;
        AuditService::record(
            db,
            "door_inserted",
            None,
            None,
            &format!("door={} name={}", door.key, door.name),
        )?;
        Ok(())
    }

    pub fn update_door(db: &Db, door: &DoorDefinitionRecord) -> Result<(), SysopError> {
        update_door_definition(db, door)?;
        AuditService::record(
            db,
            "door_updated",
            None,
            None,
            &format!("door={} name={}", door.key, door.name),
        )?;
        Ok(())
    }

    pub fn check_config(contents: &str) -> Result<(usize, usize), SysopError> {
        let definitions = parse_doors_toml(contents)?;
        let enabled = definitions.iter().filter(|d| d.enabled).count();
        Ok((definitions.len(), enabled))
    }
}

#[cfg(test)]
mod tests {
    use super::DoorAdminService;
    use oxidebbs_db::{
        DoorDefinitionRecord, OxideDb, find_door_by_key, insert_door_definition, list_audit_events,
    };

    const DOOR_ID: &str = "00000000-0000-4000-8000-000000000911";

    fn sample_door() -> DoorDefinitionRecord {
        DoorDefinitionRecord {
            id: DOOR_ID.to_string(),
            key: "lord".to_string(),
            name: "Legend of the Red Dragon".to_string(),
            runner: "dosemu".to_string(),
            working_dir: "./doors/lord".to_string(),
            command: "LORD.EXE".to_string(),
            drop_file: "door.sys".to_string(),
            exclusive: false,
            time_limit_minutes: 30,
            enabled: true,
            min_security_level: 0,
        }
    }

    #[test]
    fn set_enabled_updates_door_and_audits_change() {
        let db = OxideDb::open_memory().expect("open db");
        insert_door_definition(db.db(), &sample_door()).expect("insert door");

        DoorAdminService::set_enabled(db.db(), DOOR_ID, "lord", false).expect("disable door");

        let door = find_door_by_key(db.db(), "lord")
            .expect("find door")
            .expect("door exists");
        assert!(!door.enabled);
        let events = list_audit_events(db.db(), 10).expect("list audit");
        assert_eq!(events[0].event_type, "door_disabled");
        assert!(events[0].details.contains("door=lord"));
    }
}
