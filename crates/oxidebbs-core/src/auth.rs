use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::user::{User, UserStatus};

const MIN_ALIAS_LEN: usize = 2;
const MAX_ALIAS_LEN: usize = 32;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewUserInput {
    pub id: String,
    pub alias: String,
    pub real_name: String,
    pub email: Option<String>,
    pub password_hash: String,
    pub security_level: i32,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginAttempt {
    pub alias: String,
    pub password: String,
    pub login_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginSuccess {
    pub user: User,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AuthFlowError {
    #[error("alias must be between {min} and {max} ASCII characters")]
    InvalidAlias { min: usize, max: usize },

    #[error("real name is required")]
    MissingRealName,

    #[error("password hash is required")]
    MissingPasswordHash,

    #[error("user account is not active")]
    UserNotActive,

    #[error("password verification failed")]
    PasswordRejected,

    #[error("security level must be between 0 and 255")]
    InvalidSecurityLevel,
}

pub trait PasswordVerifier {
    fn verify(&self, password: &str, password_hash: &str) -> bool;
}

impl<F> PasswordVerifier for F
where
    F: for<'a, 'b> Fn(&'a str, &'b str) -> bool,
{
    fn verify(&self, password: &str, password_hash: &str) -> bool {
        self(password, password_hash)
    }
}

pub fn create_new_user(input: NewUserInput) -> Result<User, AuthFlowError> {
    validate_alias(&input.alias)?;
    if input.real_name.trim().is_empty() {
        return Err(AuthFlowError::MissingRealName);
    }
    if input.password_hash.trim().is_empty() {
        return Err(AuthFlowError::MissingPasswordHash);
    }
    if !(0..=255).contains(&input.security_level) {
        return Err(AuthFlowError::InvalidSecurityLevel);
    }

    Ok(User {
        id: input.id,
        alias: input.alias.trim().to_string(),
        real_name: input.real_name.trim().to_string(),
        email: input.email.and_then(normalize_optional_email),
        password_hash: input.password_hash,
        security_level: input.security_level,
        is_sysop: false,
        created_at: input.created_at,
        last_login_at: None,
        total_calls: 0,
        time_bank_minutes: 0,
        status: UserStatus::Active,
    })
}

pub fn login_user(
    user: &User,
    attempt: &LoginAttempt,
    verifier: &impl PasswordVerifier,
) -> Result<LoginSuccess, AuthFlowError> {
    if user.status != UserStatus::Active {
        return Err(AuthFlowError::UserNotActive);
    }
    if !user.alias.eq_ignore_ascii_case(attempt.alias.trim()) {
        return Err(AuthFlowError::PasswordRejected);
    }
    if !verifier.verify(&attempt.password, &user.password_hash) {
        return Err(AuthFlowError::PasswordRejected);
    }

    let mut logged_in = user.clone();
    logged_in.last_login_at = Some(attempt.login_at.clone());
    logged_in.total_calls += 1;
    Ok(LoginSuccess { user: logged_in })
}

fn validate_alias(alias: &str) -> Result<(), AuthFlowError> {
    let alias = alias.trim();
    let len = alias.chars().count();
    let valid = (MIN_ALIAS_LEN..=MAX_ALIAS_LEN).contains(&len)
        && alias
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '_' | '-'));
    if valid {
        Ok(())
    } else {
        Err(AuthFlowError::InvalidAlias {
            min: MIN_ALIAS_LEN,
            max: MAX_ALIAS_LEN,
        })
    }
}

fn normalize_optional_email(email: String) -> Option<String> {
    let email = email.trim();
    if email.is_empty() {
        None
    } else {
        Some(email.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Argon2TestVerifier;

    impl PasswordVerifier for Argon2TestVerifier {
        fn verify(&self, password: &str, password_hash: &str) -> bool {
            password == "secret" && password_hash.starts_with("$argon2id$")
        }
    }

    struct AlwaysVerifier;

    impl PasswordVerifier for AlwaysVerifier {
        fn verify(&self, _password: &str, _password_hash: &str) -> bool {
            true
        }
    }

    fn new_user_input(alias: &str) -> NewUserInput {
        NewUserInput {
            id: format!("uid-{alias}"),
            alias: alias.to_string(),
            real_name: "Test User".to_string(),
            email: Some(" test@example.com ".to_string()),
            password_hash: "$argon2id$v=19$hash".to_string(),
            security_level: 10,
            created_at: "2026-01-01T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn new_user_flow_normalizes_profile_defaults() {
        let user = create_new_user(new_user_input("sysop_1")).expect("create user");

        assert_eq!(user.alias, "sysop_1");
        assert_eq!(user.email.as_deref(), Some("test@example.com"));
        assert_eq!(user.security_level, 10);
        assert_eq!(user.status, UserStatus::Active);
        assert_eq!(user.total_calls, 0);
    }

    #[test]
    fn new_user_flow_uses_configured_security_level() {
        let mut input = new_user_input("starter");
        input.security_level = 20;

        let user = create_new_user(input).expect("create user");

        assert_eq!(user.security_level, 20);
    }

    #[test]
    fn new_user_flow_rejects_out_of_range_security_level() {
        let mut input = new_user_input("starter");
        input.security_level = 256;

        let error = create_new_user(input).expect_err("invalid level");

        assert_eq!(error, AuthFlowError::InvalidSecurityLevel);
    }

    #[test]
    fn new_user_flow_rejects_invalid_alias() {
        let error = create_new_user(new_user_input("bad alias")).expect_err("invalid alias");

        assert_eq!(
            error,
            AuthFlowError::InvalidAlias {
                min: MIN_ALIAS_LEN,
                max: MAX_ALIAS_LEN
            }
        );
    }

    #[test]
    fn login_flow_accepts_verified_password_and_updates_call_state() {
        let user = create_new_user(new_user_input("Alice")).expect("create user");
        let attempt = LoginAttempt {
            alias: "alice".to_string(),
            password: "secret".to_string(),
            login_at: "2026-02-01T00:00:00Z".to_string(),
        };

        let success = login_user(&user, &attempt, &Argon2TestVerifier).expect("login");

        assert_eq!(success.user.total_calls, 1);
        assert_eq!(
            success.user.last_login_at.as_deref(),
            Some("2026-02-01T00:00:00Z")
        );
    }

    #[test]
    fn login_flow_rejects_inactive_user() {
        let mut user = create_new_user(new_user_input("Alice")).expect("create user");
        user.status = UserStatus::Locked;
        let attempt = LoginAttempt {
            alias: "Alice".to_string(),
            password: "secret".to_string(),
            login_at: "2026-02-01T00:00:00Z".to_string(),
        };

        let error = login_user(&user, &attempt, &AlwaysVerifier).expect_err("locked user");

        assert_eq!(error, AuthFlowError::UserNotActive);
    }
}
