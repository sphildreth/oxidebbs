use crate::SysopError;
use oxidebbs_db::{
    Db, DoorDefinitionRecord, DoorRunRecord, find_door_by_key, list_door_definitions,
    list_door_runs,
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

    pub fn check_config(contents: &str) -> Result<(usize, usize), SysopError> {
        let definitions = parse_doors_toml(contents)?;
        let enabled = definitions.iter().filter(|d| d.enabled).count();
        Ok((definitions.len(), enabled))
    }
}
