use std::fs;
use std::path::{Path, PathBuf};

use clap::{Args, Subcommand};
use serde_json::{Value as JsonValue, json};

use oxidebbs_db::{
    FileAreaRecord, FileEntryRecord, insert_file_area, insert_file_entry, list_file_areas,
    list_file_entries, list_file_transfers, update_file_area, update_file_entry_approved,
};

use crate::sysop_cli::{
    AppContext, CliError, CliResult, audit, current_timestamp, emit_ok, generated_uuid,
    open_database, print_json, require_user,
};

#[derive(Subcommand)]
pub enum FilesCommand {
    /// Manage caller file areas
    Areas {
        #[command(subcommand)]
        command: FileAreasCommand,
    },
    /// List file entries
    List {
        #[arg(long)]
        area: Option<String>,
    },
    /// Import a local file into a file area
    Import(FileImportArgs),
    /// Safely remove a file entry by marking it unapproved
    Remove {
        file_id: String,
        #[arg(long)]
        reason: String,
    },
    /// Inspect transfer history
    Transfers {
        #[command(subcommand)]
        command: FileTransfersCommand,
    },
}

#[derive(Subcommand)]
pub enum FileAreasCommand {
    List,
    Add(FileAreaAddArgs),
    Edit(FileAreaEditArgs),
}

#[derive(Args, Debug, Clone)]
pub struct FileAreaAddArgs {
    pub key: String,
    #[arg(long)]
    pub name: String,
    #[arg(long)]
    pub root: PathBuf,
    #[arg(long, default_value = "")]
    pub description: String,
    #[arg(long, default_value_t = 0)]
    pub read_level: i64,
    #[arg(long, default_value_t = 10)]
    pub download_level: i64,
    #[arg(long, default_value_t = 10)]
    pub upload_level: i64,
    #[arg(long)]
    pub max_upload_bytes: Option<i64>,
    #[arg(long)]
    pub disabled: bool,
}

#[derive(Args, Debug, Clone)]
pub struct FileAreaEditArgs {
    pub key: String,
    #[arg(long)]
    pub new_key: Option<String>,
    #[arg(long)]
    pub name: Option<String>,
    #[arg(long)]
    pub root: Option<PathBuf>,
    #[arg(long)]
    pub description: Option<String>,
    #[arg(long)]
    pub read_level: Option<i64>,
    #[arg(long)]
    pub download_level: Option<i64>,
    #[arg(long)]
    pub upload_level: Option<i64>,
    #[arg(long)]
    pub max_upload_bytes: Option<i64>,
    #[arg(long)]
    pub clear_max_upload_bytes: bool,
    #[arg(long)]
    pub enabled: Option<bool>,
}

#[derive(Args, Debug, Clone)]
pub struct FileImportArgs {
    pub area: String,
    pub path: PathBuf,
    #[arg(long)]
    pub name: Option<String>,
    #[arg(long, default_value = "")]
    pub description: String,
    #[arg(long)]
    pub uploader: Option<String>,
    #[arg(long, default_value_t = true)]
    pub approved: bool,
}

#[derive(Subcommand)]
pub enum FileTransfersCommand {
    Recent {
        #[arg(short, long, default_value_t = 25)]
        limit: usize,
    },
}

pub fn run_files(command: FilesCommand, ctx: &AppContext) -> CliResult<()> {
    let db = open_database(&ctx.config)?;
    match command {
        FilesCommand::Areas { command } => run_file_areas(command, ctx, &db),
        FilesCommand::List { area } => {
            let area_record = area
                .as_deref()
                .map(|key| require_file_area(&db, key))
                .transpose()?;
            let entries: Vec<_> = list_file_entries(db.db())?
                .into_iter()
                .filter(|entry| {
                    area_record
                        .as_ref()
                        .is_none_or(|area| entry.area_id == area.id)
                })
                .collect();
            print_file_entries(&entries, ctx.json)
        }
        FilesCommand::Import(args) => import_file(args, ctx, &db),
        FilesCommand::Remove { file_id, reason } => {
            let entry = require_file_entry(&db, &file_id)?;
            update_file_entry_approved(db.db(), &entry.id, false)?;
            audit(
                &db,
                "file:remove",
                None,
                None,
                &format!(
                    "file entry {} ({}) marked unapproved; reason: {}",
                    entry.display_name, entry.id, reason
                ),
            )?;
            emit_ok(
                ctx.json,
                "file entry marked unapproved",
                json!({"file_id": entry.id, "reason": reason}),
            )
        }
        FilesCommand::Transfers { command } => match command {
            FileTransfersCommand::Recent { limit } => {
                let transfers: Vec<_> = list_file_transfers(db.db())?
                    .into_iter()
                    .take(limit)
                    .collect();
                print_file_transfers(&transfers, ctx.json)
            }
        },
    }
}

fn run_file_areas(
    command: FileAreasCommand,
    ctx: &AppContext,
    db: &oxidebbs_db::OxideDb,
) -> CliResult<()> {
    match command {
        FileAreasCommand::List => {
            let areas = list_file_areas(db.db())?;
            print_file_areas(&areas, ctx.json)
        }
        FileAreasCommand::Add(args) => {
            validate_security_level(args.read_level, "read-level")?;
            validate_security_level(args.download_level, "download-level")?;
            validate_security_level(args.upload_level, "upload-level")?;
            if let Some(max_upload_bytes) = args.max_upload_bytes
                && max_upload_bytes < 0
            {
                return Err(CliError::Message(
                    "--max-upload-bytes must not be negative".to_string(),
                ));
            }
            let area = FileAreaRecord {
                id: generated_uuid(db)?,
                key: args.key,
                name: args.name,
                description: args.description,
                root_path: args.root.display().to_string(),
                read_security_level: args.read_level,
                download_security_level: args.download_level,
                upload_security_level: args.upload_level,
                max_upload_bytes: args.max_upload_bytes,
                enabled: !args.disabled,
                created_at: current_timestamp(db)?,
                updated_at: current_timestamp(db)?,
            };
            insert_file_area(db.db(), &area)?;
            audit(
                db,
                "file_area:add",
                None,
                None,
                &format!("file area {} ({}) added", area.key, area.id),
            )?;
            emit_ok(ctx.json, "file area added", file_area_json(&area))
        }
        FileAreasCommand::Edit(args) => {
            let mut area = require_file_area(db, &args.key)?;
            if let Some(key) = args.new_key {
                area.key = key;
            }
            if let Some(name) = args.name {
                area.name = name;
            }
            if let Some(root) = args.root {
                area.root_path = root.display().to_string();
            }
            if let Some(description) = args.description {
                area.description = description;
            }
            if let Some(level) = args.read_level {
                validate_security_level(level, "read-level")?;
                area.read_security_level = level;
            }
            if let Some(level) = args.download_level {
                validate_security_level(level, "download-level")?;
                area.download_security_level = level;
            }
            if let Some(level) = args.upload_level {
                validate_security_level(level, "upload-level")?;
                area.upload_security_level = level;
            }
            if args.clear_max_upload_bytes {
                area.max_upload_bytes = None;
            } else if let Some(max_upload_bytes) = args.max_upload_bytes {
                if max_upload_bytes < 0 {
                    return Err(CliError::Message(
                        "--max-upload-bytes must not be negative".to_string(),
                    ));
                }
                area.max_upload_bytes = Some(max_upload_bytes);
            }
            if let Some(enabled) = args.enabled {
                area.enabled = enabled;
            }
            update_file_area(db.db(), &area)?;
            audit(
                db,
                "file_area:edit",
                None,
                None,
                &format!("file area {} ({}) edited", area.key, area.id),
            )?;
            emit_ok(ctx.json, "file area updated", file_area_json(&area))
        }
    }
}

fn import_file(args: FileImportArgs, ctx: &AppContext, db: &oxidebbs_db::OxideDb) -> CliResult<()> {
    let area = require_file_area(db, &args.area)?;
    if !area.enabled {
        return Err(CliError::Message(format!(
            "file area {:?} is disabled",
            area.key
        )));
    }
    let metadata = fs::metadata(&args.path)?;
    if !metadata.is_file() {
        return Err(CliError::Message(format!(
            "{:?} is not a regular file",
            args.path
        )));
    }
    let size_bytes = i64::try_from(metadata.len())
        .map_err(|_| CliError::Message("file is too large to import".to_string()))?;
    if let Some(max_upload_bytes) = area.max_upload_bytes
        && size_bytes > max_upload_bytes
    {
        return Err(CliError::Message(format!(
            "file exceeds area max upload size of {max_upload_bytes} bytes"
        )));
    }

    let original_name = args
        .path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| CliError::Message("import path must have a file name".to_string()))?
        .to_string();
    let display_name = args.name.unwrap_or_else(|| original_name.clone());
    if display_name.trim().is_empty() {
        return Err(CliError::Message(
            "file display name cannot be blank".to_string(),
        ));
    }

    let storage_name = storage_name_for(&args.path, &generated_uuid(db)?);
    let root = PathBuf::from(&area.root_path);
    fs::create_dir_all(&root)?;
    let destination = root.join(&storage_name);
    fs::copy(&args.path, &destination)?;

    let uploader = args
        .uploader
        .as_ref()
        .map(|alias| require_user(db, alias))
        .transpose()?;
    let entry = FileEntryRecord {
        id: generated_uuid(db)?,
        area_id: area.id.clone(),
        storage_name,
        display_name,
        original_name: Some(original_name),
        size_bytes,
        content_crc32: None,
        description: args.description,
        uploader_user_id: uploader.as_ref().map(|user| user.id.clone()),
        download_count: 0,
        approved: args.approved,
        created_at: current_timestamp(db)?,
        updated_at: current_timestamp(db)?,
    };
    insert_file_entry(db.db(), &entry)?;
    audit(
        db,
        "file:import",
        uploader.as_ref().map(|user| user.id.as_str()),
        None,
        &format!(
            "file {} ({}) imported into area {}; {} bytes",
            entry.display_name, entry.id, area.key, entry.size_bytes
        ),
    )?;
    emit_ok(ctx.json, "file imported", file_entry_json(&entry))
}

fn require_file_area(db: &oxidebbs_db::OxideDb, key: &str) -> CliResult<FileAreaRecord> {
    oxidebbs_db::find_file_area_by_key(db.db(), key)?
        .ok_or_else(|| CliError::Message(format!("file area {key:?} was not found")))
}

fn require_file_entry(db: &oxidebbs_db::OxideDb, id: &str) -> CliResult<FileEntryRecord> {
    oxidebbs_db::find_file_entry_by_id(db.db(), id)?
        .ok_or_else(|| CliError::Message(format!("file entry {id:?} was not found")))
}

fn validate_security_level(level: i64, name: &str) -> CliResult<()> {
    if !(0..=255).contains(&level) {
        return Err(CliError::Message(format!("--{name} must be in 0..=255")));
    }
    Ok(())
}

fn storage_name_for(path: &Path, id: &str) -> String {
    path.extension()
        .and_then(|extension| extension.to_str())
        .filter(|extension| {
            !extension.is_empty()
                && extension
                    .chars()
                    .all(|character| character.is_ascii_alphanumeric())
        })
        .map_or_else(|| id.to_string(), |extension| format!("{id}.{extension}"))
}

fn print_file_areas(areas: &[FileAreaRecord], json_output: bool) -> CliResult<()> {
    if json_output {
        print_json(&json!({"file_areas": areas.iter().map(file_area_json).collect::<Vec<_>>()}))
    } else {
        for area in areas {
            println!(
                "{}\t{}\troot={}\tread={}\tdownload={}\tupload={}\tenabled={}",
                area.key,
                area.name,
                area.root_path,
                area.read_security_level,
                area.download_security_level,
                area.upload_security_level,
                area.enabled
            );
        }
        Ok(())
    }
}

fn print_file_entries(entries: &[FileEntryRecord], json_output: bool) -> CliResult<()> {
    if json_output {
        print_json(&json!({"files": entries.iter().map(file_entry_json).collect::<Vec<_>>()}))
    } else {
        for entry in entries {
            println!(
                "{}\t{}\tsize={}\tapproved={}\tdownloads={}",
                entry.id,
                entry.display_name,
                entry.size_bytes,
                entry.approved,
                entry.download_count
            );
        }
        Ok(())
    }
}

fn print_file_transfers(
    transfers: &[oxidebbs_db::FileTransferRecord],
    json_output: bool,
) -> CliResult<()> {
    if json_output {
        print_json(
            &json!({"transfers": transfers.iter().map(file_transfer_json).collect::<Vec<_>>()}),
        )
    } else {
        for transfer in transfers {
            println!(
                "{}\t{}\t{}\tbytes={}\toutcome={}",
                transfer.id,
                transfer.direction,
                transfer.protocol,
                transfer.transferred_payload_bytes,
                transfer.outcome
            );
        }
        Ok(())
    }
}

fn file_area_json(area: &FileAreaRecord) -> JsonValue {
    json!({
        "id": area.id,
        "key": area.key,
        "name": area.name,
        "description": area.description,
        "root_path": area.root_path,
        "read_security_level": area.read_security_level,
        "download_security_level": area.download_security_level,
        "upload_security_level": area.upload_security_level,
        "max_upload_bytes": area.max_upload_bytes,
        "enabled": area.enabled,
        "created_at": area.created_at,
        "updated_at": area.updated_at
    })
}

fn file_entry_json(entry: &FileEntryRecord) -> JsonValue {
    json!({
        "id": entry.id,
        "area_id": entry.area_id,
        "storage_name": entry.storage_name,
        "display_name": entry.display_name,
        "original_name": entry.original_name,
        "size_bytes": entry.size_bytes,
        "content_crc32": entry.content_crc32,
        "description": entry.description,
        "uploader_user_id": entry.uploader_user_id,
        "download_count": entry.download_count,
        "approved": entry.approved,
        "created_at": entry.created_at,
        "updated_at": entry.updated_at
    })
}

fn file_transfer_json(transfer: &oxidebbs_db::FileTransferRecord) -> JsonValue {
    json!({
        "id": transfer.id,
        "node_number": transfer.node_number,
        "user_id": transfer.user_id,
        "area_id": transfer.area_id,
        "file_entry_id": transfer.file_entry_id,
        "direction": transfer.direction,
        "protocol": transfer.protocol,
        "requested_name": transfer.requested_name,
        "storage_name": transfer.storage_name,
        "declared_size_bytes": transfer.declared_size_bytes,
        "transferred_payload_bytes": transfer.transferred_payload_bytes,
        "committed_size_bytes": transfer.committed_size_bytes,
        "started_at": transfer.started_at,
        "ended_at": transfer.ended_at,
        "duration_ms": transfer.duration_ms,
        "outcome": transfer.outcome,
        "error_code": transfer.error_code,
        "error_message": transfer.error_message,
        "retry_count": transfer.retry_count
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn storage_name_keeps_safe_extension_only() {
        assert_eq!(
            storage_name_for(Path::new("demo.zip"), "id"),
            "id.zip".to_string()
        );
        assert_eq!(
            storage_name_for(Path::new("demo.bad-ext"), "id"),
            "id".to_string()
        );
    }

    #[test]
    fn file_area_json_shape_is_stable() {
        let area = FileAreaRecord {
            id: "area-id".to_string(),
            key: "main".to_string(),
            name: "Main".to_string(),
            description: String::new(),
            root_path: "./files/main".to_string(),
            read_security_level: 0,
            download_security_level: 10,
            upload_security_level: 20,
            max_upload_bytes: Some(1024),
            enabled: true,
            created_at: "now".to_string(),
            updated_at: "now".to_string(),
        };

        let value = file_area_json(&area);
        assert_eq!(value["key"], "main");
        assert_eq!(value["max_upload_bytes"], 1024);
    }
}
