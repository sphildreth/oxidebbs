use crate::SysopError;
use argon2::password_hash::{PasswordHasher, SaltString};
use argon2::{Algorithm, Argon2, Params, Version};
use oxidebbs_db::{
    Db, UserRecord, find_user_by_alias_ci, find_user_by_id, list_users, update_user_is_sysop,
    update_user_password_hash, update_user_security_level, update_user_status,
};
use rand_core::OsRng;

pub struct UserAdminService;

impl UserAdminService {
    pub fn list(db: &Db) -> Result<Vec<UserRecord>, SysopError> {
        Ok(list_users(db)?)
    }

    pub fn find_by_id(db: &Db, id: &str) -> Result<Option<UserRecord>, SysopError> {
        Ok(find_user_by_id(db, id)?)
    }

    pub fn find_by_alias(db: &Db, alias: &str) -> Result<Option<UserRecord>, SysopError> {
        Ok(find_user_by_alias_ci(db, alias)?)
    }

    pub fn reset_password(db: &Db, user_id: &str, password: &str) -> Result<(), SysopError> {
        let hash = hash_password(password)?;
        update_user_password_hash(db, user_id, &hash)?;
        Ok(())
    }

    pub fn set_security_level(db: &Db, user_id: &str, level: i64) -> Result<(), SysopError> {
        update_user_security_level(db, user_id, level)?;
        Ok(())
    }

    pub fn set_status(db: &Db, user_id: &str, status: &str) -> Result<(), SysopError> {
        update_user_status(db, user_id, status)?;
        Ok(())
    }

    pub fn set_sysop(db: &Db, user_id: &str, is_sysop: bool) -> Result<(), SysopError> {
        update_user_is_sysop(db, user_id, is_sysop)?;
        Ok(())
    }
}

fn hash_password(password: &str) -> Result<String, SysopError> {
    let salt = SaltString::generate(&mut OsRng);
    let params = Params::new(19_456, 2, 1, None)
        .map_err(|e| SysopError::Message(format!("invalid Argon2 parameters: {e}")))?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let password_hash = argon2
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| SysopError::Message(format!("password hashing failed: {e}")))?;
    Ok(password_hash.to_string())
}
