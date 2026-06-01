use decentdb::{Db, Value};

#[derive(Debug, Clone, PartialEq)]
pub struct DoorDefinitionRecord {
    pub id: String,
    pub key: String,
    pub name: String,
    pub runner: String,
    pub working_dir: String,
    pub command: String,
    pub drop_file: String,
    pub exclusive: bool,
    pub time_limit_minutes: i64,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DoorRunRecord {
    pub id: String,
    pub door_id: String,
    pub user_id: String,
    pub node_number: i64,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub exit_code: Option<i64>,
    pub timed_out: bool,
    pub disconnect_forced: bool,
    pub bytes_in: i64,
    pub bytes_out: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DoorRunFinish {
    pub ended_at: String,
    pub exit_code: Option<i64>,
    pub timed_out: bool,
    pub disconnect_forced: bool,
    pub bytes_in: i64,
    pub bytes_out: i64,
}

pub fn insert_door_definition(db: &Db, door: &DoorDefinitionRecord) -> decentdb::Result<()> {
    db.execute_with_params(
        "INSERT INTO doors (id, key, name, runner, working_dir, command, drop_file, exclusive, time_limit_minutes, enabled)
         VALUES (UUID_PARSE($1), $2, $3, $4, $5, $6, $7, $8, $9, $10)",
        &[
            Value::Text(door.id.clone()),
            Value::Text(door.key.clone()),
            Value::Text(door.name.clone()),
            Value::Text(door.runner.clone()),
            Value::Text(door.working_dir.clone()),
            Value::Text(door.command.clone()),
            Value::Text(door.drop_file.clone()),
            Value::Bool(door.exclusive),
            Value::Int64(door.time_limit_minutes),
            Value::Bool(door.enabled),
        ],
    )?;
    Ok(())
}

pub fn list_door_definitions(db: &Db) -> decentdb::Result<Vec<DoorDefinitionRecord>> {
    let result = db.execute(
        "SELECT UUID_TO_STRING(id), key, name, runner, working_dir, command, drop_file, exclusive, time_limit_minutes, enabled
         FROM doors ORDER BY key",
    )?;
    Ok(result.rows().iter().map(row_to_door).collect())
}

pub fn find_door_by_key(db: &Db, key: &str) -> decentdb::Result<Option<DoorDefinitionRecord>> {
    let result = db.execute_with_params(
        "SELECT UUID_TO_STRING(id), key, name, runner, working_dir, command, drop_file, exclusive, time_limit_minutes, enabled
         FROM doors WHERE key = $1",
        &[Value::Text(key.to_string())],
    )?;
    Ok(result.rows().first().map(row_to_door))
}

pub fn update_door_enabled(db: &Db, id: &str, enabled: bool) -> decentdb::Result<()> {
    db.execute_with_params(
        "UPDATE doors SET enabled = $1 WHERE id = UUID_PARSE($2)",
        &[Value::Bool(enabled), Value::Text(id.to_string())],
    )?;
    Ok(())
}

pub fn insert_door_run(db: &Db, run: &DoorRunRecord) -> decentdb::Result<()> {
    db.execute_with_params(
        "INSERT INTO door_runs (id, door_id, user_id, node_number, started_at, ended_at, exit_code, timed_out, disconnect_forced, bytes_in, bytes_out)
         VALUES (UUID_PARSE($1), UUID_PARSE($2), UUID_PARSE($3), $4, $5, $6, $7, $8, $9, $10, $11)",
        &[
            Value::Text(run.id.clone()),
            Value::Text(run.door_id.clone()),
            Value::Text(run.user_id.clone()),
            Value::Int64(run.node_number),
            Value::Text(run.started_at.clone()),
            run.ended_at.as_ref().map(|value| Value::Text(value.clone())).unwrap_or(Value::Null),
            run.exit_code.map(Value::Int64).unwrap_or(Value::Null),
            Value::Bool(run.timed_out),
            Value::Bool(run.disconnect_forced),
            Value::Int64(run.bytes_in),
            Value::Int64(run.bytes_out),
        ],
    )?;
    Ok(())
}

pub fn finish_door_run(db: &Db, id: &str, finish: &DoorRunFinish) -> decentdb::Result<()> {
    db.execute_with_params(
        "UPDATE door_runs SET ended_at = $1, exit_code = $2, timed_out = $3, disconnect_forced = $4, bytes_in = $5, bytes_out = $6 WHERE id = UUID_PARSE($7)",
        &[
            Value::Text(finish.ended_at.clone()),
            finish.exit_code.map(Value::Int64).unwrap_or(Value::Null),
            Value::Bool(finish.timed_out),
            Value::Bool(finish.disconnect_forced),
            Value::Int64(finish.bytes_in),
            Value::Int64(finish.bytes_out),
            Value::Text(id.to_string()),
        ],
    )?;
    Ok(())
}

pub fn list_door_runs(db: &Db, limit: i64) -> decentdb::Result<Vec<DoorRunRecord>> {
    let result = db.execute_with_params(
        "SELECT UUID_TO_STRING(id), UUID_TO_STRING(door_id), UUID_TO_STRING(user_id), node_number, CAST(started_at AS TEXT), CAST(ended_at AS TEXT), exit_code, timed_out, disconnect_forced, bytes_in, bytes_out
         FROM door_runs ORDER BY started_at DESC LIMIT $1",
        &[Value::Int64(limit)],
    )?;
    Ok(result.rows().iter().map(row_to_run).collect())
}

pub fn find_door_run_by_id(db: &Db, id: &str) -> decentdb::Result<Option<DoorRunRecord>> {
    let result = db.execute_with_params(
        "SELECT UUID_TO_STRING(id), UUID_TO_STRING(door_id), UUID_TO_STRING(user_id), node_number, CAST(started_at AS TEXT), CAST(ended_at AS TEXT), exit_code, timed_out, disconnect_forced, bytes_in, bytes_out
         FROM door_runs WHERE id = UUID_PARSE($1)",
        &[Value::Text(id.to_string())],
    )?;
    Ok(result.rows().first().map(row_to_run))
}

fn row_to_door(row: &decentdb::QueryRow) -> DoorDefinitionRecord {
    let values = row.values();
    DoorDefinitionRecord {
        id: text_value(&values[0]),
        key: text_value(&values[1]),
        name: text_value(&values[2]),
        runner: text_value(&values[3]),
        working_dir: text_value(&values[4]),
        command: text_value(&values[5]),
        drop_file: text_value(&values[6]),
        exclusive: bool_value(&values[7]),
        time_limit_minutes: int_value(&values[8]),
        enabled: bool_value(&values[9]),
    }
}

fn row_to_run(row: &decentdb::QueryRow) -> DoorRunRecord {
    let values = row.values();
    DoorRunRecord {
        id: text_value(&values[0]),
        door_id: text_value(&values[1]),
        user_id: text_value(&values[2]),
        node_number: int_value(&values[3]),
        started_at: text_value(&values[4]),
        ended_at: opt_text_value(&values[5]),
        exit_code: opt_int_value(&values[6]),
        timed_out: bool_value(&values[7]),
        disconnect_forced: bool_value(&values[8]),
        bytes_in: int_value(&values[9]),
        bytes_out: int_value(&values[10]),
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
        Value::Text(text) => Some(text.clone()),
        _ => None,
    }
}

fn int_value(value: &Value) -> i64 {
    match value {
        Value::Int64(value) => *value,
        _ => 0,
    }
}

fn opt_int_value(value: &Value) -> Option<i64> {
    match value {
        Value::Int64(value) => Some(*value),
        _ => None,
    }
}

fn bool_value(value: &Value) -> bool {
    match value {
        Value::Bool(value) => *value,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema;
    use crate::user_repo::{UserRecord, insert_user};
    use decentdb::DbConfig;

    const USER_1: &str = "00000000-0000-4000-8000-000000000031";
    const DOOR_LORD: &str = "00000000-0000-4000-8000-000000000401";
    const RUN_1: &str = "00000000-0000-4000-8000-000000000501";

    fn test_db() -> Db {
        let db = Db::open_or_create(":memory:", DbConfig::default()).expect("open DecentDB");
        schema::init_schema(&db).expect("init schema");
        db
    }

    fn insert_test_user(db: &Db) {
        insert_user(
            db,
            &UserRecord {
                id: USER_1.to_string(),
                alias: "dooruser".to_string(),
                real_name: "Door User".to_string(),
                email: None,
                password_hash: "hashed".to_string(),
                security_level: 10,
                is_sysop: false,
                created_at: "2026-01-01T00:00:00.000000Z".to_string(),
                last_login_at: None,
                total_calls: 0,
                time_bank_minutes: 0,
                status: "active".to_string(),
            },
        )
        .expect("insert test user");
    }

    fn sample_door() -> DoorDefinitionRecord {
        DoorDefinitionRecord {
            id: DOOR_LORD.to_string(),
            key: "lord".to_string(),
            name: "Legend of the Red Dragon".to_string(),
            runner: "dosbox".to_string(),
            working_dir: "./doors/lord".to_string(),
            command: "LORD.EXE".to_string(),
            drop_file: "DOOR.SYS".to_string(),
            exclusive: false,
            time_limit_minutes: 30,
            enabled: true,
        }
    }

    fn sample_run() -> DoorRunRecord {
        DoorRunRecord {
            id: RUN_1.to_string(),
            door_id: DOOR_LORD.to_string(),
            user_id: USER_1.to_string(),
            node_number: 1,
            started_at: "2026-01-01T00:00:00.000000Z".to_string(),
            ended_at: None,
            exit_code: None,
            timed_out: false,
            disconnect_forced: false,
            bytes_in: 0,
            bytes_out: 0,
        }
    }

    #[test]
    fn inserts_and_lists_doors() {
        let db = test_db();
        insert_door_definition(&db, &sample_door()).expect("insert door");

        let doors = list_door_definitions(&db).expect("list");

        assert_eq!(doors.len(), 1);
        assert_eq!(doors[0].key, "lord");
    }

    #[test]
    fn records_and_finishes_door_run() {
        let db = test_db();
        insert_test_user(&db);
        insert_door_definition(&db, &sample_door()).expect("insert door");
        insert_door_run(&db, &sample_run()).expect("insert run");

        finish_door_run(
            &db,
            RUN_1,
            &DoorRunFinish {
                ended_at: "2026-01-01T00:05:00.000000Z".to_string(),
                exit_code: Some(0),
                timed_out: false,
                disconnect_forced: false,
                bytes_in: 12,
                bytes_out: 34,
            },
        )
        .expect("finish run");

        let runs = list_door_runs(&db, 10).expect("list runs");
        assert_eq!(
            runs[0].ended_at.as_deref(),
            Some("2026-01-01T00:05:00.000000Z")
        );
        assert_eq!(runs[0].exit_code, Some(0));
        assert_eq!(runs[0].bytes_in, 12);
        assert_eq!(runs[0].bytes_out, 34);
    }

    #[test]
    fn finds_door_run_by_id() {
        let db = test_db();
        insert_test_user(&db);
        insert_door_definition(&db, &sample_door()).expect("insert door");
        insert_door_run(&db, &sample_run()).expect("insert run");

        let found = find_door_run_by_id(&db, RUN_1).expect("find");

        assert_eq!(found, Some(sample_run()));
    }

    #[test]
    fn finds_door_by_key() {
        let db = test_db();
        insert_door_definition(&db, &sample_door()).expect("insert door");

        let found = find_door_by_key(&db, "lord").expect("find");

        assert_eq!(found, Some(sample_door()));
    }

    #[test]
    fn updates_door_enabled() {
        let db = test_db();
        insert_door_definition(&db, &sample_door()).expect("insert door");
        update_door_enabled(&db, DOOR_LORD, false).expect("update");

        let found = find_door_by_key(&db, "lord").expect("find").unwrap();
        assert!(!found.enabled);
    }

    #[test]
    fn door_run_foreign_keys_reject_missing_door_or_user() {
        let db = test_db();
        let result = insert_door_run(&db, &sample_run());

        assert!(result.is_err());
    }
}
