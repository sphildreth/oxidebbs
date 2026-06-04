use crate::SysopError;
use crate::services::audit_service::AuditService;
use argon2::password_hash::{PasswordHasher, SaltString};
use argon2::{Algorithm, Argon2, Params, Version};
use oxidebbs_db::{
    AuditEventRecord, Db, UserRecord, find_user_by_alias_ci, find_user_by_id, list_users,
    update_user_is_sysop, update_user_password_hash, update_user_security_level,
    update_user_status,
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
        AuditService::record(
            db,
            "user_password_reset",
            Some(user_id),
            None,
            "password reset from sysop TUI",
        )?;
        Ok(())
    }

    pub fn set_security_level(db: &Db, user_id: &str, level: i64) -> Result<(), SysopError> {
        if !(0..=255).contains(&level) {
            return Err(SysopError::Message(
                "security level must be between 0 and 255".to_string(),
            ));
        }
        update_user_security_level(db, user_id, level)?;
        AuditService::record(
            db,
            "user_security_level_changed",
            Some(user_id),
            None,
            &format!("security_level={level}"),
        )?;
        Ok(())
    }

    pub fn set_status(db: &Db, user_id: &str, status: &str) -> Result<(), SysopError> {
        update_user_status(db, user_id, status)?;
        AuditService::record(
            db,
            if status == "disabled" {
                "user_disabled"
            } else {
                "user_enabled"
            },
            Some(user_id),
            None,
            &format!("status={status}"),
        )?;
        Ok(())
    }

    pub fn set_sysop(db: &Db, user_id: &str, is_sysop: bool) -> Result<(), SysopError> {
        update_user_is_sysop(db, user_id, is_sysop)?;
        if is_sysop {
            update_user_security_level(db, user_id, 255)?;
        }
        AuditService::record(
            db,
            if is_sysop {
                "user_promoted_sysop"
            } else {
                "user_demoted_sysop"
            },
            Some(user_id),
            None,
            &format!("is_sysop={is_sysop}"),
        )?;
        Ok(())
    }

    pub fn view_user_audit_history(
        db: &Db,
        user_id: &str,
        limit: i64,
    ) -> Result<Vec<AuditEventRecord>, SysopError> {
        Ok(oxidebbs_db::list_audit_events_for_user(db, user_id, limit)?)
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

#[cfg(test)]
mod tests {
    use super::UserAdminService;
    use oxidebbs_db::{
        OxideDb, UserRecord, find_user_by_id, insert_user, list_audit_events_for_user,
    };

    const USER_ID: &str = "00000000-0000-4000-8000-000000000901";

    fn sample_user() -> UserRecord {
        UserRecord {
            id: USER_ID.to_string(),
            alias: "alice".to_string(),
            real_name: "Alice User".to_string(),
            email: None,
            password_hash: "hash".to_string(),
            security_level: 10,
            is_sysop: false,
            created_at: "2026-01-01T00:00:00.000000Z".to_string(),
            last_login_at: None,
            total_calls: 0,
            time_bank_minutes: 0,
            status: "active".to_string(),
        }
    }

    #[test]
    fn set_security_level_validates_range() {
        let db = OxideDb::open_memory().expect("open db");
        insert_user(db.db(), &sample_user()).expect("insert user");

        let error = UserAdminService::set_security_level(db.db(), USER_ID, 300)
            .expect_err("security level should fail");

        assert!(
            error
                .to_string()
                .contains("security level must be between 0 and 255")
        );
    }

    #[test]
    fn set_security_level_audits_change() {
        let db = OxideDb::open_memory().expect("open db");
        insert_user(db.db(), &sample_user()).expect("insert user");

        UserAdminService::set_security_level(db.db(), USER_ID, 42).expect("set level");

        let events = list_audit_events_for_user(db.db(), USER_ID, 10).expect("list audit");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type, "user_security_level_changed");
        assert!(events[0].details.contains("security_level=42"));
    }

    #[test]
    fn set_sysop_promotes_security_level_and_audits() {
        let db = OxideDb::open_memory().expect("open db");
        insert_user(db.db(), &sample_user()).expect("insert user");

        UserAdminService::set_sysop(db.db(), USER_ID, true).expect("promote");

        let user = find_user_by_id(db.db(), USER_ID)
            .expect("find user")
            .unwrap();
        assert!(user.is_sysop);
        assert_eq!(user.security_level, 255);
        let events = list_audit_events_for_user(db.db(), USER_ID, 10).expect("list audit");
        assert_eq!(events[0].event_type, "user_promoted_sysop");
    }
}
