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

pub fn insert_door_definition(db: &Db, door: &DoorDefinitionRecord) -> decentdb::Result<()> {
    db.execute_with_params(
        "INSERT INTO doors (id, key, name, runner, working_dir, command, drop_file, exclusive, time_limit_minutes, enabled)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
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
        "SELECT id, key, name, runner, working_dir, command, drop_file, exclusive, time_limit_minutes, enabled
         FROM doors ORDER BY key",
    )?;
    Ok(result.rows().iter().map(row_to_door).collect())
}

pub fn insert_door_run(db: &Db, run: &DoorRunRecord) -> decentdb::Result<()> {
    db.execute_with_params(
        "INSERT INTO door_runs (id, door_id, user_id, node_number, started_at, ended_at, exit_code, timed_out, disconnect_forced, bytes_in, bytes_out)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)",
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

pub fn finish_door_run(
    db: &Db,
    id: &str,
    ended_at: &str,
    exit_code: Option<i64>,
    timed_out: bool,
    disconnect_forced: bool,
) -> decentdb::Result<()> {
    db.execute_with_params(
        "UPDATE door_runs SET ended_at = $1, exit_code = $2, timed_out = $3, disconnect_forced = $4 WHERE id = $5",
        &[
            Value::Text(ended_at.to_string()),
            exit_code.map(Value::Int64).unwrap_or(Value::Null),
            Value::Bool(timed_out),
            Value::Bool(disconnect_forced),
            Value::Text(id.to_string()),
        ],
    )?;
    Ok(())
}

pub fn list_door_runs(db: &Db, limit: i64) -> decentdb::Result<Vec<DoorRunRecord>> {
    let result = db.execute_with_params(
        "SELECT id, door_id, user_id, node_number, started_at, ended_at, exit_code, timed_out, disconnect_forced, bytes_in, bytes_out
         FROM door_runs ORDER BY started_at DESC LIMIT $1",
        &[Value::Int64(limit)],
    )?;
    Ok(result.rows().iter().map(row_to_run).collect())
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
    use decentdb::DbConfig;

    fn test_db() -> Db {
        let db = Db::open_or_create(":memory:", DbConfig::default()).expect("open DecentDB");
        schema::init_schema(&db).expect("init schema");
        db
    }

    fn sample_door() -> DoorDefinitionRecord {
        DoorDefinitionRecord {
            id: "door-lord".to_string(),
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
            id: "run-1".to_string(),
            door_id: "door-lord".to_string(),
            user_id: "uid-1".to_string(),
            node_number: 1,
            started_at: "2026-01-01T00:00:00Z".to_string(),
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
        insert_door_run(&db, &sample_run()).expect("insert run");

        finish_door_run(&db, "run-1", "2026-01-01T00:05:00Z", Some(0), false, false)
            .expect("finish run");

        let runs = list_door_runs(&db, 10).expect("list runs");
        assert_eq!(runs[0].ended_at.as_deref(), Some("2026-01-01T00:05:00Z"));
        assert_eq!(runs[0].exit_code, Some(0));
    }
}
