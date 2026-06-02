use decentdb::{Db, Value};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthAttemptRecord {
    pub scope: String,
    pub scope_key: String,
    pub failed_count: i64,
    pub first_failed_at: Option<String>,
    pub last_failed_at: Option<String>,
    pub locked_until: Option<String>,
}

pub fn find_auth_attempt(
    db: &Db,
    scope: &str,
    scope_key: &str,
) -> decentdb::Result<Option<AuthAttemptRecord>> {
    let result = db.execute_with_params(
        "SELECT scope, scope_key, failed_count, CAST(first_failed_at AS TEXT), CAST(last_failed_at AS TEXT), CAST(locked_until AS TEXT)
         FROM auth_attempts WHERE scope = $1 AND scope_key = $2",
        &[
            Value::Text(scope.to_string()),
            Value::Text(scope_key.to_string()),
        ],
    )?;
    Ok(result.rows().first().map(row_to_auth_attempt))
}

pub fn list_auth_attempts(db: &Db) -> decentdb::Result<Vec<AuthAttemptRecord>> {
    let result = db.execute(
        "SELECT scope, scope_key, failed_count, CAST(first_failed_at AS TEXT), CAST(last_failed_at AS TEXT), CAST(locked_until AS TEXT)
         FROM auth_attempts ORDER BY scope, scope_key",
    )?;
    Ok(result.rows().iter().map(row_to_auth_attempt).collect())
}

pub fn insert_auth_attempt(db: &Db, record: &AuthAttemptRecord) -> decentdb::Result<()> {
    db.execute_with_params(
        "INSERT INTO auth_attempts (scope, scope_key, failed_count, first_failed_at, last_failed_at, locked_until)
         VALUES ($1, $2, $3, $4, $5, $6)",
        &[
            Value::Text(record.scope.clone()),
            Value::Text(record.scope_key.clone()),
            Value::Int64(record.failed_count),
            record
                .first_failed_at
                .as_ref()
                .map(|value| Value::Text(value.clone()))
                .unwrap_or(Value::Null),
            record
                .last_failed_at
                .as_ref()
                .map(|value| Value::Text(value.clone()))
                .unwrap_or(Value::Null),
            record
                .locked_until
                .as_ref()
                .map(|value| Value::Text(value.clone()))
                .unwrap_or(Value::Null),
        ],
    )?;
    Ok(())
}

pub fn record_auth_failure(
    db: &Db,
    scope: &str,
    scope_key: &str,
    now: &str,
    window_minutes: i64,
    lockout_minutes: i64,
    threshold: i64,
) -> decentdb::Result<AuthAttemptRecord> {
    let normalized_key = scope_key.trim();
    let existing = find_auth_attempt(db, scope, normalized_key)?;
    let now_seconds = parse_timestamp_seconds(now);
    let within_window = existing
        .as_ref()
        .and_then(|record| record.first_failed_at.as_deref())
        .map(|first_failed_at| {
            timestamp_minutes_between(first_failed_at, now, now_seconds)
                .map(|minutes| minutes <= window_minutes)
                .unwrap_or(false)
        })
        .unwrap_or(false);
    let failed_count = if within_window {
        existing
            .as_ref()
            .map(|record| record.failed_count.saturating_add(1))
            .unwrap_or(1)
    } else {
        1
    };
    let first_failed_at = if within_window {
        existing
            .as_ref()
            .and_then(|record| record.first_failed_at.clone())
            .unwrap_or_else(|| now.to_string())
    } else {
        now.to_string()
    };
    let locked_until = if failed_count >= threshold {
        Some(add_minutes(now, lockout_minutes).unwrap_or_else(|| now.to_string()))
    } else {
        None
    };

    db.begin_transaction()?;
    let result = (|| {
        db.execute_with_params(
            "INSERT INTO auth_attempts (scope, scope_key, failed_count, first_failed_at, last_failed_at, locked_until)
             VALUES ($1, $2, $3, $4, $5, $6)
             ON CONFLICT (scope, scope_key) DO UPDATE SET
                failed_count = EXCLUDED.failed_count,
                first_failed_at = EXCLUDED.first_failed_at,
                last_failed_at = EXCLUDED.last_failed_at,
                locked_until = EXCLUDED.locked_until",
            &[
                Value::Text(scope.to_string()),
                Value::Text(normalized_key.to_string()),
                Value::Int64(failed_count),
                Value::Text(first_failed_at.clone()),
                Value::Text(now.to_string()),
                locked_until
                    .as_ref()
                    .map(|value| Value::Text(value.clone()))
                    .unwrap_or(Value::Null),
            ],
        )?;
        find_auth_attempt(db, scope, normalized_key)?.ok_or_else(|| {
            decentdb::DbError::sql(format!(
                "auth attempt row was not persisted for {scope}:{normalized_key}"
            ))
        })
    })();
    match result {
        Ok(record) => {
            db.commit_transaction()?;
            Ok(record)
        }
        Err(error) => {
            let _ = db.rollback_transaction();
            Err(error)
        }
    }
}

pub fn clear_auth_attempt(db: &Db, scope: &str, scope_key: &str) -> decentdb::Result<()> {
    db.execute_with_params(
        "DELETE FROM auth_attempts WHERE scope = $1 AND scope_key = $2",
        &[
            Value::Text(scope.to_string()),
            Value::Text(scope_key.trim().to_string()),
        ],
    )?;
    Ok(())
}

pub fn is_auth_scope_locked(
    db: &Db,
    scope: &str,
    scope_key: &str,
    now: &str,
) -> decentdb::Result<bool> {
    let Some(record) = find_auth_attempt(db, scope, scope_key.trim())? else {
        return Ok(false);
    };
    let Some(locked_until) = record.locked_until.as_deref() else {
        return Ok(false);
    };
    Ok(timestamp_is_after(locked_until, now))
}

fn row_to_auth_attempt(row: &decentdb::QueryRow) -> AuthAttemptRecord {
    let values = row.values();
    AuthAttemptRecord {
        scope: text_value(&values[0]),
        scope_key: text_value(&values[1]),
        failed_count: int_value(&values[2]),
        first_failed_at: opt_text_value(&values[3]),
        last_failed_at: opt_text_value(&values[4]),
        locked_until: opt_text_value(&values[5]),
    }
}

fn text_value(value: &Value) -> String {
    match value {
        Value::Text(text) => text.clone(),
        _ => String::new(),
    }
}

fn opt_text_value(value: &Value) -> Option<String> {
    match value {
        Value::Text(text) if !text.is_empty() => Some(text.clone()),
        _ => None,
    }
}

fn int_value(value: &Value) -> i64 {
    match value {
        Value::Int64(value) => *value,
        _ => 0,
    }
}

fn timestamp_minutes_between(start: &str, end: &str, parsed_end: Option<i64>) -> Option<i64> {
    let start = parse_timestamp_seconds(start)?;
    let end = parsed_end.or_else(|| parse_timestamp_seconds(end))?;
    Some(end.saturating_sub(start) / 60)
}

fn timestamp_is_after(left: &str, right: &str) -> bool {
    match (
        parse_timestamp_seconds(left),
        parse_timestamp_seconds(right),
    ) {
        (Some(left), Some(right)) => left > right,
        _ => left > right,
    }
}

fn add_minutes(timestamp: &str, minutes: i64) -> Option<String> {
    parse_timestamp_seconds(timestamp)
        .map(|seconds| format_timestamp_seconds(seconds.saturating_add(minutes.saturating_mul(60))))
}

fn parse_timestamp_seconds(raw: &str) -> Option<i64> {
    let raw = raw.trim();
    let (date, time) = raw.split_once('T').or_else(|| raw.split_once(' '))?;
    let mut date_parts = date.split('-');
    let year = date_parts.next()?.parse::<i64>().ok()?;
    let month = date_parts.next()?.parse::<u32>().ok()?;
    let day = date_parts.next()?.parse::<u32>().ok()?;
    if date_parts.next().is_some() {
        return None;
    }

    let time = time
        .strip_suffix('Z')
        .or_else(|| time.split_once('+').map(|(time, _)| time))
        .or_else(|| time.rsplit_once('-').map(|(time, _)| time))
        .unwrap_or(time);
    let time = time.split_once('.').map(|(time, _)| time).unwrap_or(time);
    let mut time_parts = time.split(':');
    let hour = time_parts.next()?.parse::<u32>().ok()?;
    let minute = time_parts.next()?.parse::<u32>().ok()?;
    let second = time_parts.next()?.parse::<u32>().ok()?;
    if time_parts.next().is_some()
        || !(1..=12).contains(&month)
        || day == 0
        || hour > 23
        || minute > 59
        || second > 60
    {
        return None;
    }

    let days = days_from_civil(year, month, day);
    Some(
        days.saturating_mul(86_400)
            .saturating_add(i64::from(hour).saturating_mul(3_600))
            .saturating_add(i64::from(minute).saturating_mul(60))
            .saturating_add(i64::from(second)),
    )
}

fn format_timestamp_seconds(seconds: i64) -> String {
    let days = seconds.div_euclid(86_400);
    let seconds_of_day = seconds.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    let hour = seconds_of_day / 3_600;
    let minute = (seconds_of_day % 3_600) / 60;
    let second = seconds_of_day % 60;
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}.000000Z")
}

fn days_from_civil(year: i64, month: u32, day: u32) -> i64 {
    let year = year - i64::from(month <= 2);
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let year_of_era = year - era * 400;
    let month = i64::from(month);
    let day = i64::from(day);
    let day_of_year = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let days = days + 719_468;
    let era = if days >= 0 { days } else { days - 146_096 } / 146_097;
    let day_of_era = days - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    let year = year + i64::from(month <= 2);
    (year, month as u32, day as u32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema;
    use decentdb::DbConfig;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn test_db() -> Db {
        let db = Db::open_or_create(":memory:", DbConfig::default()).expect("open DecentDB");
        schema::init_schema(&db).expect("init schema");
        db
    }

    fn temp_db_path(tag: &str) -> std::path::PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!(
            "oxidebbs-auth-{tag}-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system time should be valid")
                .as_nanos()
        ));
        path
    }

    #[test]
    fn five_bad_attempts_lock_alias_scope() {
        let db = test_db();
        for attempt in 1..=5 {
            let record = record_auth_failure(
                &db,
                "alias",
                "alice",
                "2026-06-02T09:00:00.000000Z",
                10,
                15,
                5,
            )
            .expect("record failure");
            assert_eq!(record.failed_count, attempt);
        }

        assert!(
            is_auth_scope_locked(&db, "alias", "alice", "2026-06-02T09:10:00.000000Z")
                .expect("locked")
        );
    }

    #[test]
    fn five_bad_attempts_lock_ip_scope() {
        let db = test_db();
        for _ in 0..5 {
            record_auth_failure(
                &db,
                "ip",
                "127.0.0.1",
                "2026-06-02T09:00:00.000000Z",
                10,
                15,
                5,
            )
            .expect("record failure");
        }

        assert!(
            is_auth_scope_locked(&db, "ip", "127.0.0.1", "2026-06-02T09:01:00.000000Z")
                .expect("locked")
        );
    }

    #[test]
    fn failure_window_resets_count() {
        let db = test_db();
        record_auth_failure(
            &db,
            "alias",
            "alice",
            "2026-06-02T09:00:00.000000Z",
            10,
            15,
            5,
        )
        .expect("first failure");
        let record = record_auth_failure(
            &db,
            "alias",
            "alice",
            "2026-06-02T09:11:00.000000Z",
            10,
            15,
            5,
        )
        .expect("second failure");

        assert_eq!(record.failed_count, 1);
        assert_eq!(
            record.first_failed_at.as_deref(),
            Some("2026-06-02T09:11:00.000000Z")
        );
    }

    #[test]
    fn clear_auth_attempt_removes_scope() {
        let db = test_db();
        record_auth_failure(
            &db,
            "alias",
            "alice",
            "2026-06-02T09:00:00.000000Z",
            10,
            15,
            5,
        )
        .expect("record failure");

        clear_auth_attempt(&db, "alias", "alice").expect("clear");

        assert!(
            find_auth_attempt(&db, "alias", "alice")
                .expect("find")
                .is_none()
        );
    }

    #[test]
    fn lockout_survives_reopening_database() {
        let path = temp_db_path("persistent-lockout");
        {
            let db =
                Db::open_or_create(&path, DbConfig::default()).expect("open persistent DecentDB");
            schema::init_schema(&db).expect("init schema");
            for _ in 0..5 {
                record_auth_failure(
                    &db,
                    "alias",
                    "alice",
                    "2026-06-02T09:00:00.000000Z",
                    10,
                    15,
                    5,
                )
                .expect("record failure");
            }
        }

        let db =
            Db::open_or_create(&path, DbConfig::default()).expect("reopen persistent DecentDB");
        schema::init_schema(&db).expect("init schema");
        assert!(
            is_auth_scope_locked(&db, "alias", "alice", "2026-06-02T09:05:00.000000Z")
                .expect("locked")
        );

        let _ = std::fs::remove_file(path);
    }
}
