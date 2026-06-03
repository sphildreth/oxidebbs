use std::collections::{HashMap, HashSet};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::SysopError;
use oxidebbs_db::{
    Db, OxideDb, SCHEMA_VERSION, Value, list_active_sessions, list_audit_events,
    list_auth_attempts, list_door_definitions, list_door_runs, list_message_areas, list_messages,
    list_recent_sessions, list_users, read_schema_version,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DoctorStatus {
    Pass,
    Warn,
    Fail,
}

impl DoctorStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::Pass => "PASS",
            Self::Warn => "WARN",
            Self::Fail => "FAIL",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DoctorCheck {
    pub name: String,
    pub status: DoctorStatus,
    pub detail: String,
    pub remediation: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DoctorReport {
    pub checked_at: String,
    pub database_path: Option<String>,
    pub checks: Vec<DoctorCheck>,
}

impl DoctorReport {
    fn new(database_path: Option<String>) -> Self {
        Self {
            checked_at: checked_at_string(),
            database_path,
            checks: Vec::new(),
        }
    }

    pub fn passed_count(&self) -> usize {
        self.count_status(DoctorStatus::Pass)
    }

    pub fn warning_count(&self) -> usize {
        self.count_status(DoctorStatus::Warn)
    }

    pub fn failed_count(&self) -> usize {
        self.count_status(DoctorStatus::Fail)
    }

    fn count_status(&self, status: DoctorStatus) -> usize {
        self.checks
            .iter()
            .filter(|check| check.status == status)
            .count()
    }

    fn push(
        &mut self,
        status: DoctorStatus,
        name: impl Into<String>,
        detail: impl Into<String>,
        remediation: Option<String>,
    ) {
        self.checks.push(DoctorCheck {
            name: name.into(),
            status,
            detail: detail.into(),
            remediation,
        });
    }

    fn pass(&mut self, name: impl Into<String>, detail: impl Into<String>) {
        self.push(DoctorStatus::Pass, name, detail, None);
    }

    fn warn(
        &mut self,
        name: impl Into<String>,
        detail: impl Into<String>,
        remediation: impl Into<String>,
    ) {
        self.push(DoctorStatus::Warn, name, detail, Some(remediation.into()));
    }

    fn fail(
        &mut self,
        name: impl Into<String>,
        detail: impl Into<String>,
        remediation: impl Into<String>,
    ) {
        self.push(DoctorStatus::Fail, name, detail, Some(remediation.into()));
    }
}

pub struct DatabaseAdminService;

impl DatabaseAdminService {
    pub fn schema_version(db: &Db) -> Result<i64, SysopError> {
        Ok(read_schema_version(db)?)
    }

    pub fn count_users(db: &Db) -> Result<i64, SysopError> {
        let result = db.execute("SELECT COUNT(*) FROM users")?;
        Ok(result
            .rows()
            .first()
            .and_then(|row| row.values().first())
            .and_then(|v| match v {
                Value::Int64(n) => Some(*n),
                _ => None,
            })
            .unwrap_or(0))
    }

    pub fn count_messages(db: &Db) -> Result<i64, SysopError> {
        let result = db.execute("SELECT COUNT(*) FROM messages")?;
        Ok(result
            .rows()
            .first()
            .and_then(|row| row.values().first())
            .and_then(|v| match v {
                Value::Int64(n) => Some(*n),
                _ => None,
            })
            .unwrap_or(0))
    }

    pub fn count_audit_events(db: &Db) -> Result<i64, SysopError> {
        Self::count_table(db, "audit_events")
    }

    pub fn count_sessions(db: &Db) -> Result<i64, SysopError> {
        Self::count_table(db, "sessions")
    }

    pub fn count_doors(db: &Db) -> Result<i64, SysopError> {
        Self::count_table(db, "doors")
    }

    pub fn count_door_runs(db: &Db) -> Result<i64, SysopError> {
        Self::count_table(db, "door_runs")
    }

    pub fn run_doctor(
        db: Option<&OxideDb>,
        db_path: Option<&Path>,
        configured_node_count: u16,
    ) -> DoctorReport {
        let mut report = DoctorReport::new(db_path.map(|path| path.display().to_string()));
        Self::check_database_path(&mut report, db_path);

        let Some(db) = db else {
            report.fail(
                "Database connection",
                "No DecentDB handle is open in the sysop TUI.",
                "Verify the configured database path and start the sysop TUI from a board directory with readable DecentDB storage.",
            );
            return report;
        };

        report.pass("Database connection", "DecentDB handle is open.");
        let inner = db.db();

        Self::check_schema_version(&mut report, inner);
        Self::check_required_tables(&mut report, inner);

        let users = match list_users(inner) {
            Ok(users) => {
                let active = users.iter().filter(|user| user.status == "active").count();
                let locked_or_disabled = users.len().saturating_sub(active);
                report.pass(
                    "User repository",
                    format!(
                        "{} user(s) readable; {active} active, {locked_or_disabled} locked/disabled.",
                        users.len()
                    ),
                );

                let sysops = users.iter().filter(|user| user.is_sysop).count();
                if sysops == 0 {
                    report.fail(
                        "Sysop accounts",
                        "No account has is_sysop=true.",
                        "Run setup or promote a trusted account with users promote-sysop before operating the board.",
                    );
                } else {
                    report.pass(
                        "Sysop accounts",
                        format!("{sysops} sysop account(s) found."),
                    );
                }

                let invalid_aliases = users
                    .iter()
                    .filter(|user| user.alias.trim().is_empty())
                    .count();
                if invalid_aliases == 0 {
                    report.pass("User aliases", "All loaded user aliases are non-empty.");
                } else {
                    report.fail(
                        "User aliases",
                        format!("{invalid_aliases} user record(s) have empty aliases."),
                        "Repair or restore the affected user rows from a known-good backup.",
                    );
                }

                let invalid_levels = users
                    .iter()
                    .filter(|user| !(0..=255).contains(&user.security_level))
                    .count();
                if invalid_levels == 0 {
                    report.pass(
                        "User security levels",
                        "All loaded user security levels are within 0..=255.",
                    );
                } else {
                    report.fail(
                        "User security levels",
                        format!(
                            "{invalid_levels} user record(s) have out-of-range security levels."
                        ),
                        "Update affected users to a level between 0 and 255.",
                    );
                }

                Some(users)
            }
            Err(error) => {
                report.fail(
                    "User repository",
                    format!("Unable to list users: {error}"),
                    "Check the users table and schema version, then restore or migrate the database if needed.",
                );
                None
            }
        };

        let message_areas = match list_message_areas(inner) {
            Ok(areas) => {
                let enabled = areas.iter().filter(|area| area.enabled).count();
                if areas.is_empty() {
                    report.warn(
                        "Message areas",
                        "No message areas are configured.",
                        "Run setup or add at least one enabled local message area before callers post messages.",
                    );
                } else {
                    report.pass(
                        "Message areas",
                        format!("{} area(s) readable; {enabled} enabled.", areas.len()),
                    );
                    if enabled == 0 {
                        report.warn(
                            "Enabled message areas",
                            "All message areas are disabled.",
                            "Enable at least one message area if callers should read or post messages.",
                        );
                    } else {
                        report.pass(
                            "Enabled message areas",
                            format!("{enabled} enabled message area(s) are available."),
                        );
                    }
                }

                let invalid_levels = areas
                    .iter()
                    .filter(|area| {
                        !(0..=255).contains(&area.read_security_level)
                            || !(0..=255).contains(&area.post_security_level)
                    })
                    .count();
                if invalid_levels == 0 {
                    report.pass(
                        "Message area security levels",
                        "All loaded message area read/post levels are within 0..=255.",
                    );
                } else {
                    report.fail(
                        "Message area security levels",
                        format!(
                            "{invalid_levels} message area(s) have out-of-range read/post levels."
                        ),
                        "Update message area read/post security levels to values between 0 and 255.",
                    );
                }

                Some(areas)
            }
            Err(error) => {
                report.fail(
                    "Message areas",
                    format!("Unable to list message areas: {error}"),
                    "Check the message_areas table and schema version.",
                );
                None
            }
        };

        let messages = match list_messages(inner) {
            Ok(messages) => {
                let mut visibility_counts = HashMap::<&str, usize>::new();
                for message in &messages {
                    *visibility_counts
                        .entry(message.visibility.as_str())
                        .or_default() += 1;
                }
                let normal = visibility_counts.get("normal").copied().unwrap_or(0);
                let hidden = visibility_counts.get("hidden").copied().unwrap_or(0);
                let deleted = visibility_counts.get("deleted").copied().unwrap_or(0);
                report.pass(
                    "Message repository",
                    format!(
                        "{} message(s) readable; normal={normal}, hidden={hidden}, deleted={deleted}.",
                        messages.len()
                    ),
                );

                let invalid_visibility = messages
                    .iter()
                    .filter(|message| {
                        !matches!(message.visibility.as_str(), "normal" | "hidden" | "deleted")
                    })
                    .count();
                if invalid_visibility == 0 {
                    report.pass(
                        "Message visibility values",
                        "All loaded messages use a supported visibility value.",
                    );
                } else {
                    report.fail(
                        "Message visibility values",
                        format!(
                            "{invalid_visibility} message(s) have unsupported visibility values."
                        ),
                        "Repair affected message rows to normal, hidden, or deleted.",
                    );
                }

                Some(messages)
            }
            Err(error) => {
                report.fail(
                    "Message repository",
                    format!("Unable to list messages: {error}"),
                    "Check the messages table and message-area/user foreign keys.",
                );
                None
            }
        };

        if let (Some(users), Some(areas), Some(messages)) =
            (users.as_ref(), message_areas.as_ref(), messages.as_ref())
        {
            let user_ids: HashSet<&str> = users.iter().map(|user| user.id.as_str()).collect();
            let area_ids: HashSet<&str> = areas.iter().map(|area| area.id.as_str()).collect();
            let message_ids: HashSet<&str> =
                messages.iter().map(|message| message.id.as_str()).collect();
            let missing_area_refs = messages
                .iter()
                .filter(|message| !area_ids.contains(message.area_id.as_str()))
                .count();
            let missing_author_refs = messages
                .iter()
                .filter(|message| !user_ids.contains(message.author_user_id.as_str()))
                .count();
            let missing_to_refs = messages
                .iter()
                .filter(|message| {
                    message
                        .to_user_id
                        .as_deref()
                        .map(|id| !user_ids.contains(id))
                        .unwrap_or(false)
                })
                .count();
            let missing_reply_refs = messages
                .iter()
                .filter(|message| {
                    message
                        .reply_to_id
                        .as_deref()
                        .map(|id| !message_ids.contains(id))
                        .unwrap_or(false)
                })
                .count();
            let missing_refs =
                missing_area_refs + missing_author_refs + missing_to_refs + missing_reply_refs;
            if missing_refs == 0 {
                report.pass(
                    "Message references",
                    "Loaded message area, author, recipient, and reply references resolve.",
                );
            } else {
                report.fail(
                    "Message references",
                    format!(
                        "Broken refs: areas={missing_area_refs}, authors={missing_author_refs}, recipients={missing_to_refs}, replies={missing_reply_refs}."
                    ),
                    "Run a verified restore or repair affected message rows before exposing message boards.",
                );
            }
        }

        Self::check_sessions(&mut report, inner, configured_node_count);

        let doors = match list_door_definitions(inner) {
            Ok(doors) => {
                let enabled = doors.iter().filter(|door| door.enabled).count();
                if doors.is_empty() {
                    report.warn(
                        "Door definitions",
                        "No door definitions are configured.",
                        "Configure doors if the caller Doors menu should launch external programs.",
                    );
                } else {
                    report.pass(
                        "Door definitions",
                        format!(
                            "{} door definition(s) readable; {enabled} enabled.",
                            doors.len()
                        ),
                    );
                    if enabled == 0 {
                        report.warn(
                            "Enabled doors",
                            "All door definitions are disabled.",
                            "Enable selected doors when callers should be allowed to launch them.",
                        );
                    } else {
                        report.pass("Enabled doors", format!("{enabled} enabled door(s) found."));
                    }
                }

                let invalid_doors = doors
                    .iter()
                    .filter(|door| {
                        door.key.trim().is_empty()
                            || door.name.trim().is_empty()
                            || door.runner.trim().is_empty()
                            || door.working_dir.trim().is_empty()
                            || door.command.trim().is_empty()
                            || door.drop_file.trim().is_empty()
                            || door.time_limit_minutes <= 0
                    })
                    .count();
                if invalid_doors == 0 {
                    report.pass(
                        "Door definition fields",
                        "All loaded door definitions have required fields and positive time limits.",
                    );
                } else {
                    report.fail(
                        "Door definition fields",
                        format!("{invalid_doors} door definition(s) have missing required fields or invalid time limits."),
                        "Fix the affected door definitions before enabling caller door access.",
                    );
                }

                let long_time_limits = doors
                    .iter()
                    .filter(|door| door.time_limit_minutes > 240)
                    .count();
                if long_time_limits == 0 {
                    report.pass(
                        "Door time limits",
                        "All loaded door time limits are within the 1..=240 minute policy window.",
                    );
                } else {
                    report.warn(
                        "Door time limits",
                        format!("{long_time_limits} door definition(s) exceed 240 minutes."),
                        "Reduce unusually long door time limits to stay inside the documented runtime policy.",
                    );
                }

                Some(doors)
            }
            Err(error) => {
                report.fail(
                    "Door definitions",
                    format!("Unable to list doors: {error}"),
                    "Check the doors table and door schema migration state.",
                );
                None
            }
        };

        let door_runs = match list_door_runs(inner, 100) {
            Ok(runs) => {
                let unfinished = runs.iter().filter(|run| run.ended_at.is_none()).count();
                report.pass(
                    "Door run repository",
                    format!(
                        "{} recent door run(s) readable; {unfinished} unfinished in the sample.",
                        runs.len()
                    ),
                );
                if unfinished > 0 {
                    report.warn(
                        "Unfinished door runs",
                        format!("{unfinished} sampled door run(s) have no ended_at value."),
                        "If no caller is currently in a door, inspect stale door_runs and node state.",
                    );
                }
                Some(runs)
            }
            Err(error) => {
                report.fail(
                    "Door run repository",
                    format!("Unable to list recent door runs: {error}"),
                    "Check the door_runs table and door/user references.",
                );
                None
            }
        };

        if let (Some(users), Some(doors), Some(runs)) =
            (users.as_ref(), doors.as_ref(), door_runs.as_ref())
        {
            let user_ids: HashSet<&str> = users.iter().map(|user| user.id.as_str()).collect();
            let door_ids: HashSet<&str> = doors.iter().map(|door| door.id.as_str()).collect();
            let missing_door_refs = runs
                .iter()
                .filter(|run| !door_ids.contains(run.door_id.as_str()))
                .count();
            let missing_user_refs = runs
                .iter()
                .filter(|run| !user_ids.contains(run.user_id.as_str()))
                .count();
            if missing_door_refs == 0 && missing_user_refs == 0 {
                report.pass(
                    "Door run references",
                    "Sampled door run door/user references resolve.",
                );
            } else {
                report.fail(
                    "Door run references",
                    format!("Broken refs: doors={missing_door_refs}, users={missing_user_refs}."),
                    "Restore or repair affected door run rows before relying on door history.",
                );
            }
        }

        match list_audit_events(inner, 25) {
            Ok(events) => {
                report.pass(
                    "Audit repository",
                    format!("{} recent audit event(s) readable.", events.len()),
                );
            }
            Err(error) => report.fail(
                "Audit repository",
                format!("Unable to list recent audit events: {error}"),
                "Check audit_events storage before performing sysop actions that rely on audit trails.",
            ),
        }

        match list_auth_attempts(inner) {
            Ok(records) => {
                let locked = records
                    .iter()
                    .filter(|record| record.locked_until.is_some())
                    .count();
                report.pass(
                    "Auth attempt repository",
                    format!(
                        "{} auth attempt scope(s) readable; {locked} currently carry a lockout timestamp.",
                        records.len()
                    ),
                );
            }
            Err(error) => report.fail(
                "Auth attempt repository",
                format!("Unable to list auth attempts: {error}"),
                "Check auth_attempts storage; caller login lockout checks depend on it.",
            ),
        }

        report
    }

    fn count_table(db: &Db, table: &str) -> Result<i64, SysopError> {
        let result = db.execute(&format!("SELECT COUNT(*) FROM {table}"))?;
        Ok(result
            .rows()
            .first()
            .and_then(|row| row.values().first())
            .and_then(|v| match v {
                Value::Int64(n) => Some(*n),
                _ => None,
            })
            .unwrap_or(0))
    }

    fn check_database_path(report: &mut DoctorReport, db_path: Option<&Path>) {
        let Some(path) = db_path else {
            report.warn(
                "Database path",
                "No explicit database path was provided to the doctor.",
                "Start the sysop TUI with --config or configure a persistent database path.",
            );
            return;
        };

        report.pass(
            "Database path",
            format!("Using DecentDB path {}.", path.display()),
        );

        if path == Path::new(":memory:") {
            report.warn(
                "Database file",
                "The current database is in-memory; filesystem checks were skipped.",
                "Use a persistent database path for production boards.",
            );
            return;
        }

        if path.exists() {
            if path.is_file() {
                report.pass(
                    "Database file",
                    format!("Database file exists at {}.", path.display()),
                );
            } else {
                report.fail(
                    "Database file",
                    format!("{} exists but is not a regular file.", path.display()),
                    "Set the database path to a DecentDB file, not a directory or special file.",
                );
            }
        } else {
            report.fail(
                "Database file",
                format!("{} does not exist.", path.display()),
                "Run setup or start the server once with a writable database directory.",
            );
        }

        let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        else {
            report.warn(
                "Database directory",
                "The database path has no parent directory component.",
                "Use a path inside a dedicated data directory for production boards.",
            );
            return;
        };

        if !parent.exists() {
            report.fail(
                "Database directory",
                format!("Parent directory {} does not exist.", parent.display()),
                "Create the data directory or update the configured database path.",
            );
            return;
        }

        if !parent.is_dir() {
            report.fail(
                "Database directory",
                format!("Parent path {} is not a directory.", parent.display()),
                "Move the database into a normal writable data directory.",
            );
            return;
        }

        report.pass(
            "Database directory",
            format!("Parent directory {} exists.", parent.display()),
        );

        match write_probe(parent) {
            Ok(()) => report.pass(
                "Database directory write probe",
                format!(
                    "Created and removed a temporary probe file in {}.",
                    parent.display()
                ),
            ),
            Err(error) => report.fail(
                "Database directory write probe",
                format!(
                    "Unable to create/remove a probe file in {}: {error}",
                    parent.display()
                ),
                "Fix directory ownership or permissions for the OS user running OxideBBS.",
            ),
        }
    }

    fn check_schema_version(report: &mut DoctorReport, db: &Db) {
        match read_schema_version(db) {
            Ok(version) if version == SCHEMA_VERSION => report.pass(
                "Schema version",
                format!("Schema version {version} matches expected {SCHEMA_VERSION}."),
            ),
            Ok(version) => report.fail(
                "Schema version",
                format!("Schema version {version} does not match expected {SCHEMA_VERSION}."),
                "Run the supported migration path or restore a database matching this binary.",
            ),
            Err(error) => report.fail(
                "Schema version",
                format!("Unable to read schema version: {error}"),
                "Check system_config and DecentDB schema initialization.",
            ),
        }
    }

    fn check_required_tables(report: &mut DoctorReport, db: &Db) {
        for table in [
            "system_config",
            "users",
            "auth_attempts",
            "audit_events",
            "message_areas",
            "messages",
            "sessions",
            "doors",
            "door_runs",
        ] {
            match Self::count_table(db, table) {
                Ok(count) => report.pass(
                    format!("{table} table"),
                    format!("Table is readable; {count} row(s) found."),
                ),
                Err(error) => report.fail(
                    format!("{table} table"),
                    format!("Unable to count rows: {error}"),
                    "Verify the schema, migrations, and DecentDB file integrity.",
                ),
            }
        }
    }

    fn check_sessions(report: &mut DoctorReport, db: &Db, configured_node_count: u16) {
        match list_active_sessions(db) {
            Ok(active_sessions) => {
                report.pass(
                    "Active sessions",
                    format!("{} active session(s) readable.", active_sessions.len()),
                );

                let invalid_nodes = active_sessions
                    .iter()
                    .filter(|session| session.node_number <= 0)
                    .count();
                if invalid_nodes == 0 {
                    report.pass(
                        "Active session node numbers",
                        "All active sessions use positive node numbers.",
                    );
                } else {
                    report.fail(
                        "Active session node numbers",
                        format!(
                            "{invalid_nodes} active session(s) have non-positive node numbers."
                        ),
                        "Repair or close invalid session rows.",
                    );
                }

                let out_of_range = active_sessions
                    .iter()
                    .filter(|session| {
                        configured_node_count > 0
                            && session.node_number > i64::from(configured_node_count)
                    })
                    .count();
                if out_of_range == 0 {
                    report.pass(
                        "Configured node range",
                        format!(
                            "Active sessions fit within the configured {configured_node_count} node(s)."
                        ),
                    );
                } else {
                    report.warn(
                        "Configured node range",
                        format!(
                            "{out_of_range} active session(s) exceed the configured {configured_node_count} node(s)."
                        ),
                        "Check node count configuration and close stale session rows if the server is not using those nodes.",
                    );
                }

                let mut seen = HashSet::new();
                let duplicates = active_sessions
                    .iter()
                    .filter(|session| !seen.insert(session.node_number))
                    .count();
                if duplicates == 0 {
                    report.pass(
                        "Active session uniqueness",
                        "No duplicate active node assignments were found.",
                    );
                } else {
                    report.fail(
                        "Active session uniqueness",
                        format!("{duplicates} duplicate active node assignment(s) were found."),
                        "Use node reset/cleanup tooling to close stale duplicate active sessions.",
                    );
                }
            }
            Err(error) => report.fail(
                "Active sessions",
                format!("Unable to list active sessions: {error}"),
                "Check the sessions table before relying on live node state.",
            ),
        }

        match list_recent_sessions(db, 50) {
            Ok(recent_sessions) => report.pass(
                "Recent session history",
                format!("{} recent session(s) readable.", recent_sessions.len()),
            ),
            Err(error) => report.fail(
                "Recent session history",
                format!("Unable to list recent sessions: {error}"),
                "Check session history storage and schema migrations.",
            ),
        }
    }
}

fn checked_at_string() -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    format!("unix:{seconds}")
}

fn write_probe(parent: &Path) -> Result<(), String> {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let probe_path = parent.join(format!(
        ".oxidebbs-doctor-write-probe-{}-{nanos}",
        std::process::id()
    ));

    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&probe_path)
        .map_err(|error| error.to_string())?;
    file.write_all(b"oxidebbs doctor write probe")
        .map_err(|error| error.to_string())?;
    drop(file);
    fs::remove_file(&probe_path).map_err(|error| error.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{DatabaseAdminService, DoctorStatus};
    use oxidebbs_db::{OxideDb, UserRecord, insert_user};

    #[test]
    fn doctor_reports_schema_tables_and_missing_starter_data() {
        let db = OxideDb::open_memory().expect("open memory database");

        let report = DatabaseAdminService::run_doctor(Some(&db), Some(Path::new(":memory:")), 8);

        assert!(
            report
                .checks
                .iter()
                .any(|check| check.name == "Schema version" && check.status == DoctorStatus::Pass)
        );
        assert!(
            report
                .checks
                .iter()
                .any(|check| check.name == "users table" && check.status == DoctorStatus::Pass)
        );
        assert!(
            report
                .checks
                .iter()
                .any(|check| check.name == "Sysop accounts" && check.status == DoctorStatus::Fail)
        );
        assert!(
            report
                .checks
                .iter()
                .any(|check| check.name == "Message areas" && check.status == DoctorStatus::Warn)
        );
    }

    #[test]
    fn doctor_passes_sysop_account_check_when_sysop_exists() {
        let db = OxideDb::open_memory().expect("open memory database");
        insert_user(
            db.db(),
            &UserRecord {
                id: "00000000-0000-4000-8000-000000000001".to_string(),
                alias: "sysop".to_string(),
                real_name: "Sysop".to_string(),
                email: None,
                password_hash: "hash".to_string(),
                security_level: 255,
                is_sysop: true,
                created_at: "2026-01-01T00:00:00.000000Z".to_string(),
                last_login_at: None,
                total_calls: 0,
                time_bank_minutes: 0,
                status: "active".to_string(),
            },
        )
        .expect("insert sysop");

        let report = DatabaseAdminService::run_doctor(Some(&db), Some(Path::new(":memory:")), 8);

        assert!(
            report
                .checks
                .iter()
                .any(|check| check.name == "Sysop accounts" && check.status == DoctorStatus::Pass)
        );
    }
}
