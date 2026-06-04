use decentdb::{Db, Value};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileAreaRecord {
    pub id: String,
    pub key: String,
    pub name: String,
    pub description: String,
    pub root_path: String,
    pub read_security_level: i64,
    pub download_security_level: i64,
    pub upload_security_level: i64,
    pub max_upload_bytes: Option<i64>,
    pub enabled: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileEntryRecord {
    pub id: String,
    pub area_id: String,
    pub storage_name: String,
    pub display_name: String,
    pub original_name: Option<String>,
    pub size_bytes: i64,
    pub content_crc32: Option<String>,
    pub description: String,
    pub uploader_user_id: Option<String>,
    pub download_count: i64,
    pub approved: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileTransferRecord {
    pub id: String,
    pub node_number: i64,
    pub user_id: String,
    pub area_id: Option<String>,
    pub file_entry_id: Option<String>,
    pub direction: String,
    pub protocol: String,
    pub requested_name: Option<String>,
    pub storage_name: Option<String>,
    pub declared_size_bytes: Option<i64>,
    pub transferred_payload_bytes: i64,
    pub committed_size_bytes: Option<i64>,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub duration_ms: Option<i64>,
    pub outcome: String,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub retry_count: i64,
}

pub fn insert_file_area(db: &Db, record: &FileAreaRecord) -> decentdb::Result<()> {
    db.execute_with_params(
        "INSERT INTO file_areas (key, name, description, root_path, read_security_level, download_security_level, upload_security_level, max_upload_bytes, enabled)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
        &[
            Value::Text(record.key.clone()),
            Value::Text(record.name.clone()),
            Value::Text(record.description.clone()),
            Value::Text(record.root_path.clone()),
            Value::Int64(record.read_security_level),
            Value::Int64(record.download_security_level),
            Value::Int64(record.upload_security_level),
            record
                .max_upload_bytes
                .map(Value::Int64)
                .unwrap_or(Value::Null),
            Value::Bool(record.enabled),
        ],
    )?;
    Ok(())
}

pub fn list_file_areas(db: &Db) -> decentdb::Result<Vec<FileAreaRecord>> {
    let result = db.execute(
        "SELECT UUID_TO_STRING(id), key, name, description, root_path, read_security_level, download_security_level, upload_security_level, max_upload_bytes, enabled, CAST(created_at AS TEXT), CAST(updated_at AS TEXT)
         FROM file_areas ORDER BY key",
    )?;
    Ok(result.rows().iter().map(row_to_file_area).collect())
}

pub fn find_file_area_by_key(db: &Db, key: &str) -> decentdb::Result<Option<FileAreaRecord>> {
    let result = db.execute_with_params(
        "SELECT UUID_TO_STRING(id), key, name, description, root_path, read_security_level, download_security_level, upload_security_level, max_upload_bytes, enabled, CAST(created_at AS TEXT), CAST(updated_at AS TEXT)
         FROM file_areas WHERE key = $1",
        &[Value::Text(key.to_string())],
    )?;
    Ok(result.rows().first().map(row_to_file_area))
}

pub fn update_file_area(db: &Db, record: &FileAreaRecord) -> decentdb::Result<()> {
    db.execute_with_params(
        "UPDATE file_areas
         SET key = $2,
             name = $3,
             description = $4,
             root_path = $5,
             read_security_level = $6,
             download_security_level = $7,
             upload_security_level = $8,
             max_upload_bytes = $9,
             enabled = $10,
             updated_at = CURRENT_TIMESTAMP
         WHERE id = UUID_PARSE($1)",
        &[
            Value::Text(record.id.clone()),
            Value::Text(record.key.clone()),
            Value::Text(record.name.clone()),
            Value::Text(record.description.clone()),
            Value::Text(record.root_path.clone()),
            Value::Int64(record.read_security_level),
            Value::Int64(record.download_security_level),
            Value::Int64(record.upload_security_level),
            record
                .max_upload_bytes
                .map(Value::Int64)
                .unwrap_or(Value::Null),
            Value::Bool(record.enabled),
        ],
    )?;
    Ok(())
}

pub fn insert_file_entry(db: &Db, record: &FileEntryRecord) -> decentdb::Result<()> {
    db.execute_with_params(
        "INSERT INTO file_entries (area_id, storage_name, display_name, original_name, size_bytes, content_crc32, description, uploader_user_id, approved)
         VALUES (UUID_PARSE($1), $2, $3, $4, $5, $6, $7, UUID_PARSE($8), $9)",
        &[
            Value::Text(record.area_id.clone()),
            Value::Text(record.storage_name.clone()),
            Value::Text(record.display_name.clone()),
            record
                .original_name
                .as_ref()
                .map(|v| Value::Text(v.clone()))
                .unwrap_or(Value::Null),
            Value::Int64(record.size_bytes),
            record
                .content_crc32
                .as_ref()
                .map(|v| Value::Text(v.clone()))
                .unwrap_or(Value::Null),
            Value::Text(record.description.clone()),
            record
                .uploader_user_id
                .as_ref()
                .map(|v| Value::Text(v.clone()))
                .unwrap_or(Value::Null),
            Value::Bool(record.approved),
        ],
    )?;
    Ok(())
}

pub fn list_file_entries(db: &Db) -> decentdb::Result<Vec<FileEntryRecord>> {
    let result = db.execute(
        "SELECT UUID_TO_STRING(id), UUID_TO_STRING(area_id), storage_name, display_name, original_name, size_bytes, content_crc32, description, UUID_TO_STRING(uploader_user_id), download_count, approved, CAST(created_at AS TEXT), CAST(updated_at AS TEXT)
         FROM file_entries ORDER BY created_at",
    )?;
    Ok(result.rows().iter().map(row_to_file_entry).collect())
}

pub fn find_file_entry_by_id(db: &Db, id: &str) -> decentdb::Result<Option<FileEntryRecord>> {
    let result = db.execute_with_params(
        "SELECT UUID_TO_STRING(id), UUID_TO_STRING(area_id), storage_name, display_name, original_name, size_bytes, content_crc32, description, UUID_TO_STRING(uploader_user_id), download_count, approved, CAST(created_at AS TEXT), CAST(updated_at AS TEXT)
         FROM file_entries WHERE id = UUID_PARSE($1)",
        &[Value::Text(id.to_string())],
    )?;
    Ok(result.rows().first().map(row_to_file_entry))
}

pub fn update_file_entry_approved(db: &Db, id: &str, approved: bool) -> decentdb::Result<()> {
    db.execute_with_params(
        "UPDATE file_entries SET approved = $2, updated_at = CURRENT_TIMESTAMP WHERE id = UUID_PARSE($1)",
        &[Value::Text(id.to_string()), Value::Bool(approved)],
    )?;
    Ok(())
}

pub fn insert_file_transfer(db: &Db, record: &FileTransferRecord) -> decentdb::Result<()> {
    db.execute_with_params(
        "INSERT INTO file_transfers (node_number, user_id, area_id, file_entry_id, direction, protocol, requested_name, storage_name, declared_size_bytes, transferred_payload_bytes, committed_size_bytes, started_at, ended_at, duration_ms, outcome, error_code, error_message, retry_count)
         VALUES ($1, UUID_PARSE($2), UUID_PARSE($3), UUID_PARSE($4), $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18)",
        &[
            Value::Int64(record.node_number),
            Value::Text(record.user_id.clone()),
            record
                .area_id
                .as_ref()
                .map(|v| Value::Text(v.clone()))
                .unwrap_or(Value::Null),
            record
                .file_entry_id
                .as_ref()
                .map(|v| Value::Text(v.clone()))
                .unwrap_or(Value::Null),
            Value::Text(record.direction.clone()),
            Value::Text(record.protocol.clone()),
            record
                .requested_name
                .as_ref()
                .map(|v| Value::Text(v.clone()))
                .unwrap_or(Value::Null),
            record
                .storage_name
                .as_ref()
                .map(|v| Value::Text(v.clone()))
                .unwrap_or(Value::Null),
            record
                .declared_size_bytes
                .map(Value::Int64)
                .unwrap_or(Value::Null),
            Value::Int64(record.transferred_payload_bytes),
            record
                .committed_size_bytes
                .map(Value::Int64)
                .unwrap_or(Value::Null),
            Value::Text(record.started_at.clone()),
            record
                .ended_at
                .as_ref()
                .map(|v| Value::Text(v.clone()))
                .unwrap_or(Value::Null),
            record.duration_ms.map(Value::Int64).unwrap_or(Value::Null),
            Value::Text(record.outcome.clone()),
            record
                .error_code
                .as_ref()
                .map(|v| Value::Text(v.clone()))
                .unwrap_or(Value::Null),
            record
                .error_message
                .as_ref()
                .map(|v| Value::Text(v.clone()))
                .unwrap_or(Value::Null),
            Value::Int64(record.retry_count),
        ],
    )?;
    Ok(())
}

pub fn list_file_transfers(db: &Db) -> decentdb::Result<Vec<FileTransferRecord>> {
    let result = db.execute(
        "SELECT UUID_TO_STRING(id), node_number, UUID_TO_STRING(user_id), UUID_TO_STRING(area_id), UUID_TO_STRING(file_entry_id), direction, protocol, requested_name, storage_name, declared_size_bytes, transferred_payload_bytes, committed_size_bytes, CAST(started_at AS TEXT), CAST(ended_at AS TEXT), duration_ms, outcome, error_code, error_message, retry_count
         FROM file_transfers ORDER BY started_at DESC",
    )?;
    Ok(result.rows().iter().map(row_to_file_transfer).collect())
}

pub fn find_file_transfer_by_id(db: &Db, id: &str) -> decentdb::Result<Option<FileTransferRecord>> {
    let result = db.execute_with_params(
        "SELECT UUID_TO_STRING(id), node_number, UUID_TO_STRING(user_id), UUID_TO_STRING(area_id), UUID_TO_STRING(file_entry_id), direction, protocol, requested_name, storage_name, declared_size_bytes, transferred_payload_bytes, committed_size_bytes, CAST(started_at AS TEXT), CAST(ended_at AS TEXT), duration_ms, outcome, error_code, error_message, retry_count
         FROM file_transfers WHERE id = UUID_PARSE($1)",
        &[Value::Text(id.to_string())],
    )?;
    Ok(result.rows().first().map(row_to_file_transfer))
}

fn row_to_file_area(row: &decentdb::QueryRow) -> FileAreaRecord {
    let values = row.values();
    FileAreaRecord {
        id: text_value(&values[0]),
        key: text_value(&values[1]),
        name: text_value(&values[2]),
        description: text_value(&values[3]),
        root_path: text_value(&values[4]),
        read_security_level: int_value(&values[5]),
        download_security_level: int_value(&values[6]),
        upload_security_level: int_value(&values[7]),
        max_upload_bytes: opt_int_value(&values[8]),
        enabled: bool_value(&values[9]),
        created_at: text_value(&values[10]),
        updated_at: text_value(&values[11]),
    }
}

fn row_to_file_entry(row: &decentdb::QueryRow) -> FileEntryRecord {
    let values = row.values();
    FileEntryRecord {
        id: text_value(&values[0]),
        area_id: text_value(&values[1]),
        storage_name: text_value(&values[2]),
        display_name: text_value(&values[3]),
        original_name: opt_text_value(&values[4]),
        size_bytes: int_value(&values[5]),
        content_crc32: opt_text_value(&values[6]),
        description: text_value(&values[7]),
        uploader_user_id: opt_text_value(&values[8]),
        download_count: int_value(&values[9]),
        approved: bool_value(&values[10]),
        created_at: text_value(&values[11]),
        updated_at: text_value(&values[12]),
    }
}

fn row_to_file_transfer(row: &decentdb::QueryRow) -> FileTransferRecord {
    let values = row.values();
    FileTransferRecord {
        id: text_value(&values[0]),
        node_number: int_value(&values[1]),
        user_id: text_value(&values[2]),
        area_id: opt_text_value(&values[3]),
        file_entry_id: opt_text_value(&values[4]),
        direction: text_value(&values[5]),
        protocol: text_value(&values[6]),
        requested_name: opt_text_value(&values[7]),
        storage_name: opt_text_value(&values[8]),
        declared_size_bytes: opt_int_value(&values[9]),
        transferred_payload_bytes: int_value(&values[10]),
        committed_size_bytes: opt_int_value(&values[11]),
        started_at: text_value(&values[12]),
        ended_at: opt_text_value(&values[13]),
        duration_ms: opt_int_value(&values[14]),
        outcome: text_value(&values[15]),
        error_code: opt_text_value(&values[16]),
        error_message: opt_text_value(&values[17]),
        retry_count: int_value(&values[18]),
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
        Value::Int64(v) => *v,
        _ => 0,
    }
}

fn opt_int_value(value: &Value) -> Option<i64> {
    match value {
        Value::Int64(v) => Some(*v),
        Value::Null => None,
        _ => None,
    }
}

fn bool_value(value: &Value) -> bool {
    match value {
        Value::Bool(v) => *v,
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

    #[test]
    fn insert_and_list_file_areas() {
        let db = test_db();

        let area = FileAreaRecord {
            id: String::new(),
            key: "utilities".into(),
            name: "Utilities".into(),
            description: "Useful tools".into(),
            root_path: "/files/utilities".into(),
            read_security_level: 0,
            download_security_level: 10,
            upload_security_level: 50,
            max_upload_bytes: Some(1_048_576),
            enabled: true,
            created_at: String::new(),
            updated_at: String::new(),
        };

        insert_file_area(&db, &area).expect("insert file area");

        let areas = list_file_areas(&db).expect("list file areas");
        assert_eq!(areas.len(), 1);
        assert_eq!(areas[0].key, "utilities");
        assert_eq!(areas[0].name, "Utilities");
        assert!(areas[0].enabled);
    }

    #[test]
    fn test_find_file_area_by_key() {
        let db = test_db();

        let area = FileAreaRecord {
            id: String::new(),
            key: "games".into(),
            name: "Games".into(),
            description: String::new(),
            root_path: "/files/games".into(),
            read_security_level: 0,
            download_security_level: 10,
            upload_security_level: 0,
            max_upload_bytes: None,
            enabled: true,
            created_at: String::new(),
            updated_at: String::new(),
        };

        insert_file_area(&db, &area).expect("insert file area");

        let found = find_file_area_by_key(&db, "games").expect("find");
        assert!(found.is_some());
        assert_eq!(found.unwrap().key, "games");

        let not_found = find_file_area_by_key(&db, "nonexistent").expect("find");
        assert!(not_found.is_none());
    }

    #[test]
    fn insert_and_list_file_entries() {
        let db = test_db();

        let area = FileAreaRecord {
            id: String::new(),
            key: "utils".into(),
            name: "Utils".into(),
            description: String::new(),
            root_path: "/files/utils".into(),
            read_security_level: 0,
            download_security_level: 10,
            upload_security_level: 50,
            max_upload_bytes: None,
            enabled: true,
            created_at: String::new(),
            updated_at: String::new(),
        };
        insert_file_area(&db, &area).expect("insert area");

        let areas = list_file_areas(&db).expect("list areas");
        let area_id = &areas[0].id;

        let entry = FileEntryRecord {
            id: String::new(),
            area_id: area_id.clone(),
            storage_name: "PKZ204G.EXE".into(),
            display_name: "PKZIP 2.04g".into(),
            original_name: Some("pkz204g.exe".into()),
            size_bytes: 42166,
            content_crc32: Some("A1B2C3D4".into()),
            description: "Compression utility".into(),
            uploader_user_id: None,
            download_count: 0,
            approved: true,
            created_at: String::new(),
            updated_at: String::new(),
        };

        insert_file_entry(&db, &entry).expect("insert file entry");

        let entries = list_file_entries(&db).expect("list entries");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].storage_name, "PKZ204G.EXE");
        assert_eq!(entries[0].size_bytes, 42166);
        assert!(entries[0].approved);
    }

    #[test]
    fn insert_file_transfer_rejects_invalid_user_fk() {
        let db = test_db();

        let xfer = FileTransferRecord {
            id: String::new(),
            node_number: 1,
            user_id: String::new(),
            area_id: None,
            file_entry_id: None,
            direction: "download".into(),
            protocol: "zmodem".into(),
            requested_name: None,
            storage_name: None,
            declared_size_bytes: None,
            transferred_payload_bytes: 0,
            committed_size_bytes: None,
            started_at: "2026-06-03T12:00:00.000000Z".into(),
            ended_at: None,
            duration_ms: None,
            outcome: "started".into(),
            error_code: None,
            error_message: None,
            retry_count: 0,
        };

        let result = insert_file_transfer(&db, &xfer);
        assert!(result.is_err());
    }
}
