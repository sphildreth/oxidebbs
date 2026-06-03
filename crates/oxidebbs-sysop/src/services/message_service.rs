use crate::SysopError;
use oxidebbs_db::{
    Db, MessageAreaRecord, MessageRecord, find_message_area_by_key, find_message_by_id,
    list_message_areas, list_messages_in_area, update_message_visibility,
};

pub struct MessageAdminService;

impl MessageAdminService {
    pub fn list_areas(db: &Db) -> Result<Vec<MessageAreaRecord>, SysopError> {
        Ok(list_message_areas(db)?)
    }

    pub fn find_area(db: &Db, key: &str) -> Result<Option<MessageAreaRecord>, SysopError> {
        Ok(find_message_area_by_key(db, key)?)
    }

    pub fn list_messages(db: &Db, area_id: &str) -> Result<Vec<MessageRecord>, SysopError> {
        Ok(list_messages_in_area(db, area_id)?)
    }

    pub fn find_message(db: &Db, id: &str) -> Result<Option<MessageRecord>, SysopError> {
        Ok(find_message_by_id(db, id)?)
    }

    pub fn delete_message(db: &Db, message_id: &str) -> Result<(), SysopError> {
        update_message_visibility(db, message_id, "deleted")?;
        Ok(())
    }
}
