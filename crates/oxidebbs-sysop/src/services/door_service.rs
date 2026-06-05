use crate::SysopError;
use crate::services::audit_service::AuditService;
use oxidebbs_db::{
    Db, DoorDefinitionRecord, DoorProviderCredentialRecord, DoorRunRecord, Value,
    delete_door_provider_credential, find_door_by_key, find_door_provider_credential,
    insert_door_definition, insert_door_provider_credential, list_door_definitions,
    list_door_provider_credentials, list_door_runs, update_door_definition, update_door_enabled,
    update_door_provider_credential,
};
use oxidebbs_door::REDACTED_PROVIDER_SECRET;
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

    pub fn list_provider_credentials(
        db: &Db,
        door_id: &str,
    ) -> Result<Vec<DoorProviderCredentialRecord>, SysopError> {
        Ok(list_door_provider_credentials(db, door_id)?)
    }

    pub fn upsert_provider_credential(
        db: &Db,
        door_id: &str,
        provider_name: &str,
        credential_ref: &str,
    ) -> Result<(), SysopError> {
        let now = db_scalar_text(db, "SELECT CAST(NOW() AS TEXT)")?;
        match find_door_provider_credential(db, door_id, provider_name)? {
            Some(existing) => update_door_provider_credential(
                db,
                &DoorProviderCredentialRecord {
                    id: existing.id,
                    door_id: existing.door_id,
                    provider_name: existing.provider_name,
                    credential_ref: credential_ref.to_string(),
                    created_at: existing.created_at,
                    updated_at: now,
                },
            )?,
            None => insert_door_provider_credential(
                db,
                &DoorProviderCredentialRecord {
                    id: db_scalar_text(db, "SELECT UUID_TO_STRING(GEN_RANDOM_UUID())")?,
                    door_id: door_id.to_string(),
                    provider_name: provider_name.to_string(),
                    credential_ref: credential_ref.to_string(),
                    created_at: now.clone(),
                    updated_at: now,
                },
            )?,
        }
        AuditService::record(
            db,
            "door_provider_credential_updated",
            None,
            None,
            &format!(
                "door_id={door_id} provider={provider_name} credential_ref={REDACTED_PROVIDER_SECRET}"
            ),
        )?;
        Ok(())
    }

    pub fn delete_provider_credential(
        db: &Db,
        door_id: &str,
        provider_name: &str,
    ) -> Result<(), SysopError> {
        if let Some(existing) = find_door_provider_credential(db, door_id, provider_name)? {
            delete_door_provider_credential(db, &existing.id)?;
            AuditService::record(
                db,
                "door_provider_credential_deleted",
                None,
                None,
                &format!(
                    "door_id={door_id} provider={provider_name} credential_ref={REDACTED_PROVIDER_SECRET}"
                ),
            )?;
        }
        Ok(())
    }

    pub fn check_config(contents: &str) -> Result<(usize, usize), SysopError> {
        let definitions = parse_doors_toml(contents)?;
        let enabled = definitions.iter().filter(|d| d.enabled).count();
        Ok((definitions.len(), enabled))
    }
}

fn db_scalar_text(db: &Db, sql: &str) -> Result<String, SysopError> {
    let result = db.execute(sql)?;
    let value = result
        .rows()
        .first()
        .and_then(|row| row.values().first())
        .ok_or_else(|| SysopError::Message(format!("query returned no scalar value: {sql}")))?;
    match value {
        Value::Text(value) => Ok(value.clone()),
        other => Err(SysopError::Message(format!(
            "query returned non-text scalar for {sql}: {other:?}"
        ))),
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

    #[test]
    fn provider_credentials_are_stored_but_redacted_in_audit_details() {
        let db = OxideDb::open_memory().expect("open db");
        insert_door_definition(db.db(), &sample_door()).expect("insert door");

        DoorAdminService::upsert_provider_credential(
            db.db(),
            DOOR_ID,
            "bbslink",
            "vault://doors/lord/bbslink",
        )
        .expect("upsert credential");

        let credentials =
            DoorAdminService::list_provider_credentials(db.db(), DOOR_ID).expect("credentials");
        assert_eq!(credentials.len(), 1);
        assert_eq!(credentials[0].credential_ref, "vault://doors/lord/bbslink");

        let events = list_audit_events(db.db(), 10).expect("list audit");
        assert_eq!(events[0].event_type, "door_provider_credential_updated");
        assert!(events[0].details.contains("credential_ref=[redacted]"));
        assert!(!events[0].details.contains("vault://doors/lord/bbslink"));
    }
}
