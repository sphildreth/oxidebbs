use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::Read;
use std::path::{Component, Path, PathBuf};

use hex;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use zip::ZipArchive;

use crate::DoorError;

pub const OXDOOR_MANIFEST_FILE: &str = "oxide-door.toml";
pub const OXDOOR_CHECKSUM_FILE: &str = "checksums.sha256";
pub const OXDOOR_PACKAGE_FORMAT: &str = "oxide-door-package-v1";
pub const OXDOOR_PACKAGE_KIND_FULL: &str = "full";
pub const OXDOOR_SUPPORTED_DROPFILES: [&str; 6] = [
    "DOOR.SYS",
    "DORINFO1.DEF",
    "CHAIN.TXT",
    "DOORFILE.SR",
    "PCBOARD.SYS",
    "CALLINFO.BBS",
];

const FILES_DIRECTORY: &str = "files/";

#[derive(Debug, Clone, Serialize)]
pub struct OxDoorPackageSummary {
    pub package_name: String,
    pub package_id: String,
    pub package_version: String,
    pub package_kind: String,
    pub legal_status: String,
    pub requires_key: bool,
    pub source_url: Option<String>,
    pub door_id: String,
    pub door_name: String,
    pub door_category: String,
    pub runner: String,
    pub command: String,
    pub working_directory: String,
    pub preferred_drop_file: String,
    pub supported_drop_files: Vec<String>,
    pub exclusive: bool,
    pub timeout_seconds: u64,
    pub min_security_level: i32,
    pub enabled_after_import_request: bool,
    pub file_count: usize,
    pub total_unpacked_size: u64,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct OxDoorPackageInspection {
    pub summary: OxDoorPackageSummary,
}

#[derive(Debug, Clone, Deserialize)]
struct RawDoorPackageManifest {
    pub package: PackageSection,
    pub legal: LegalSection,
    #[serde(default)]
    pub source: Option<SourceSection>,
    pub door: DoorSection,
    #[serde(default)]
    pub access: AccessSection,
    #[serde(default)]
    pub persistence: PersistenceSection,
    #[serde(default)]
    pub test: TestSection,
    #[serde(default)]
    pub menu: MenuSection,
}

#[derive(Debug, Clone, Deserialize)]
struct PackageSection {
    pub format: String,
    pub kind: String,
    pub id: String,
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub requires_key: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
struct LegalSection {
    pub status: String,
    #[serde(default)]
    pub requires_key: Option<bool>,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct SourceSection {
    #[serde(default)]
    pub url: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct DoorSection {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub category: Option<String>,
    pub runner: String,
    pub command: String,
    #[serde(default, alias = "working_directory")]
    pub working_directory: Option<String>,
    #[serde(default, alias = "working_dir")]
    pub working_dir: Option<String>,
    #[serde(default, alias = "preferred_dropfile")]
    #[serde(alias = "preferred_drop_file")]
    pub preferred_drop_file: Option<String>,
    #[serde(default, alias = "supported_dropfiles")]
    #[serde(alias = "supported_drop_files")]
    pub supported_drop_files: Vec<String>,
    #[serde(default)]
    pub exclusive: Option<bool>,
    #[serde(default)]
    pub timeout_seconds: Option<u64>,
    #[serde(default, alias = "enabled_after_import")]
    pub enabled_after_import: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AccessSection {
    #[serde(default)]
    pub min_security_level: i32,
    #[serde(default)]
    pub preferred_drop_file: Option<String>,
    #[serde(default)]
    pub supported_drop_files: Vec<String>,
    #[serde(default)]
    pub exclusive: Option<bool>,
    #[serde(default)]
    pub timeout_seconds: Option<u64>,
}

impl Default for AccessSection {
    fn default() -> Self {
        Self {
            min_security_level: 0,
            preferred_drop_file: None,
            supported_drop_files: Vec::new(),
            exclusive: None,
            timeout_seconds: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PersistenceSection {
    #[serde(default)]
    pub enabled_after_import_request: Option<bool>,
    #[serde(default)]
    pub timeout_seconds: Option<u64>,
}

impl Default for PersistenceSection {
    fn default() -> Self {
        Self {
            enabled_after_import_request: None,
            timeout_seconds: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TestSection {
    #[serde(default)]
    pub timeout_seconds: Option<u64>,
}

impl Default for TestSection {
    fn default() -> Self {
        Self {
            timeout_seconds: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MenuSection {
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub exclusive: Option<bool>,
    #[serde(default)]
    pub timeout_seconds: Option<u64>,
}

impl Default for MenuSection {
    fn default() -> Self {
        Self {
            category: None,
            exclusive: None,
            timeout_seconds: None,
        }
    }
}

impl DoorSection {
    fn working_directory(&self) -> Option<&str> {
        self.working_directory
            .as_deref()
            .or(self.working_dir.as_deref())
    }
}

impl DoorSection {
    fn supported_drop_files(&self) -> Vec<String> {
        self.supported_drop_files.clone()
    }
}

pub fn inspect_oxide_door_package(
    package_path: impl AsRef<Path>,
) -> Result<OxDoorPackageSummary, DoorError> {
    let package_path = package_path.as_ref();
    let file = File::open(package_path).map_err(|source| DoorError::ReadDoorPackage {
        path: package_path.to_path_buf(),
        source,
    })?;

    let mut archive = ZipArchive::new(file).map_err(|source| DoorError::InvalidDoorPackage {
        path: package_path.to_path_buf(),
        message: format!("not a valid ZIP archive: {source}"),
    })?;

    let manifest = read_manifest(&mut archive, package_path)?;
    let mut summary = validate_manifest(package_path, &manifest)?;
    let checksums = read_checksums(&mut archive, package_path)?;
    let (file_count, total_unpacked_size) = verify_files(package_path, &mut archive, &checksums)?;

    summary.file_count = file_count;
    summary.total_unpacked_size = total_unpacked_size;

    Ok(summary)
}

fn read_manifest(
    archive: &mut ZipArchive<File>,
    package_path: &Path,
) -> Result<RawDoorPackageManifest, DoorError> {
    let contents = read_text_entry(archive, package_path, OXDOOR_MANIFEST_FILE)?;
    toml::from_str(&contents).map_err(|source| DoorError::ParseDoorPackage {
        path: package_path.to_path_buf(),
        source,
    })
}

fn read_checksums(
    archive: &mut ZipArchive<File>,
    package_path: &Path,
) -> Result<HashMap<String, String>, DoorError> {
    let contents = read_text_entry(archive, package_path, OXDOOR_CHECKSUM_FILE)?;
    parse_checksums(package_path, &contents)
}

fn parse_checksums(
    package_path: &Path,
    contents: &str,
) -> Result<HashMap<String, String>, DoorError> {
    let mut checksums = HashMap::new();

    for (line_no, raw_line) in contents.lines().enumerate() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }
        let mut parts = line.split_whitespace();
        let checksum = parts.next().unwrap_or_default();
        let candidate_path = parts.next().unwrap_or_default();
        if checksum.is_empty() || candidate_path.is_empty() || parts.next().is_some() {
            return Err(DoorError::InvalidDoorPackage {
                path: package_path.to_path_buf(),
                message: format!(
                    "invalid checksum line {}: expected '<sha256> <path>'",
                    line_no + 1
                ),
            });
        }
        if checksum.len() != 64 || hex::decode(checksum).is_err() {
            return Err(DoorError::InvalidDoorPackage {
                path: package_path.to_path_buf(),
                message: format!("invalid SHA-256 checksum on line {}", line_no + 1),
            });
        }
        let normalized_path = normalize_and_validate_checksum_path(package_path, candidate_path)?;
        let exists = checksums.insert(normalized_path.clone(), checksum.to_ascii_lowercase());
        if exists.is_some() {
            return Err(DoorError::InvalidDoorPackage {
                path: package_path.to_path_buf(),
                message: format!("duplicate checksum entry for {candidate_path}"),
            });
        }
    }

    if checksums.is_empty() {
        return Err(DoorError::InvalidDoorPackage {
            path: package_path.to_path_buf(),
            message: format!("{} is empty", OXDOOR_CHECKSUM_FILE),
        });
    }

    Ok(checksums)
}

fn validate_manifest(
    package_path: &Path,
    manifest: &RawDoorPackageManifest,
) -> Result<OxDoorPackageSummary, DoorError> {
    if manifest.package.format != OXDOOR_PACKAGE_FORMAT {
        return Err(DoorError::InvalidDoorPackage {
            path: package_path.to_path_buf(),
            message: format!(
                "unsupported package format {:?}; expected {:?}",
                manifest.package.format, OXDOOR_PACKAGE_FORMAT
            ),
        });
    }
    if manifest.package.kind.to_ascii_lowercase() != OXDOOR_PACKAGE_KIND_FULL {
        return Err(DoorError::InvalidDoorPackage {
            path: package_path.to_path_buf(),
            message: if manifest.package.kind.trim().is_empty() {
                "package.kind is required".to_string()
            } else {
                format!(
                    "unsupported package kind {:?}; only \"{}\" is supported now",
                    manifest.package.kind, OXDOOR_PACKAGE_KIND_FULL
                )
            },
        });
    }

    let package_name = trim_required(&manifest.package.name)
        .ok_or_else(|| DoorError::InvalidDoorPackage {
            path: package_path.to_path_buf(),
            message: "package.name is required".to_string(),
        })?;
    let package_id = trim_required(&manifest.package.id).ok_or_else(|| {
        DoorError::InvalidDoorPackage {
            path: package_path.to_path_buf(),
            message: "package.id is required".to_string(),
        }
    })?;
    let package_version = trim_required(&manifest.package.version).ok_or_else(|| {
        DoorError::InvalidDoorPackage {
            path: package_path.to_path_buf(),
            message: "package.version is required".to_string(),
        }
    })?;
    validate_id_characters(package_path, "package.id", &package_id)?;

    let legal_status = trim_required(&manifest.legal.status).ok_or_else(|| {
        DoorError::InvalidDoorPackage {
            path: package_path.to_path_buf(),
            message: "legal.status is required".to_string(),
        }
    })?;
    let requires_key = manifest
        .legal
        .requires_key
        .or(manifest.package.requires_key)
        .unwrap_or(false);

    let door_id = trim_required(&manifest.door.id).ok_or_else(|| {
        DoorError::InvalidDoorPackage {
            path: package_path.to_path_buf(),
            message: "door.id is required".to_string(),
        }
    })?;
    let door_name = trim_required(&manifest.door.name).ok_or_else(|| {
        DoorError::InvalidDoorPackage {
            path: package_path.to_path_buf(),
            message: "door.name is required".to_string(),
        }
    })?;
    validate_id_characters(package_path, "door.id", &door_id)?;

    if manifest.door.runner.trim().is_empty() {
        return Err(DoorError::InvalidDoorPackage {
            path: package_path.to_path_buf(),
            message: "door.runner is required".to_string(),
        });
    }
    if !manifest.door.runner.trim().eq_ignore_ascii_case("local:dosemu2") {
        return Err(DoorError::InvalidDoorPackage {
            path: package_path.to_path_buf(),
            message: "unsupported runner; v1 packages must set door.runner = \"local:dosemu2\""
                .to_string(),
        });
    }

    let command = trim_required(&manifest.door.command).ok_or_else(|| {
        DoorError::InvalidDoorPackage {
            path: package_path.to_path_buf(),
            message: "door.command is required".to_string(),
        }
    })?;

    let working_directory = manifest
        .door
        .working_directory()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| DoorError::InvalidDoorPackage {
            path: package_path.to_path_buf(),
            message: "door.working_directory is required".to_string(),
        })?;
    validate_relative_directory(package_path, "door.working_directory", working_directory)?;

    let preferred_drop_file = manifest
        .door
        .preferred_drop_file
        .as_ref()
        .or(manifest.access.preferred_drop_file.as_ref())
        .map(|value| normalize_drop_file(package_path, value))
        .transpose()?
        .or_else(|| {
            manifest.access
                .supported_drop_files
                .first()
                .map(|value| normalize_drop_file(package_path, value))
                .transpose()
                .ok()
                .flatten()
        })
        .ok_or_else(|| {
            DoorError::InvalidDoorPackage {
                path: package_path.to_path_buf(),
                message: "preferred drop-file format is required".to_string(),
            }
        })?;

    let mut supported_drop_files = Vec::new();
    for value in manifest
        .door
        .supported_drop_files()
        .into_iter()
        .chain(manifest.access.supported_drop_files.iter().cloned())
    {
        let normalized = normalize_drop_file(package_path, &value)?;
        supported_drop_files.push(normalized);
    }
    supported_drop_files.sort();
    supported_drop_files.dedup();
    if manifest.door.supported_drop_files.is_empty() && manifest.access.supported_drop_files.is_empty() {
        return Err(DoorError::InvalidDoorPackage {
            path: package_path.to_path_buf(),
            message: "door-support section requires supported drop-file formats".to_string(),
        });
    }
    if !supported_drop_files.iter().any(|value| value == &preferred_drop_file) {
        return Err(DoorError::InvalidDoorPackage {
            path: package_path.to_path_buf(),
            message: format!(
                "preferred drop-file format {:?} is not listed in supported_drop_files",
                preferred_drop_file
            ),
        });
    }

    let timeout_seconds = manifest
        .door
        .timeout_seconds
        .or(manifest.access.timeout_seconds)
        .or(manifest.persistence.timeout_seconds)
        .or(manifest.test.timeout_seconds)
        .or(manifest.menu.timeout_seconds)
        .unwrap_or(30 * 60);
    if timeout_seconds == 0 {
        return Err(DoorError::InvalidDoorPackage {
            path: package_path.to_path_buf(),
            message: "timeout_seconds must be greater than 0".to_string(),
        });
    }

    let min_security_level = manifest.access.min_security_level;
    if min_security_level < 0 {
        return Err(DoorError::InvalidDoorPackage {
            path: package_path.to_path_buf(),
            message: "access.min_security_level must be >= 0".to_string(),
        });
    }

    let enabled_after_import_request = manifest
        .door
        .enabled_after_import
        .or(manifest.persistence.enabled_after_import_request)
        .unwrap_or(false);

    let mut warnings = Vec::new();
    if manifest
        .source
        .as_ref()
        .and_then(|source| source.url.as_ref())
        .is_none()
    {
        warnings.push("source.url is not present".to_string());
    }
    Ok(OxDoorPackageSummary {
        package_name,
        package_id,
        package_version,
        package_kind: manifest.package.kind.trim().to_string(),
        legal_status,
        requires_key,
        source_url: manifest
            .source
            .as_ref()
            .and_then(|source| source.url.as_ref())
            .map(|url| url.trim().to_string()),
        door_id,
        door_name,
        door_category: manifest
            .menu
            .category
            .clone()
            .or_else(|| manifest.door.category.clone())
            .unwrap_or_else(|| "uncategorized".to_string()),
        runner: manifest.door.runner.trim().to_string(),
        command,
        working_directory: working_directory.to_string(),
        preferred_drop_file,
        supported_drop_files,
        exclusive: manifest
            .menu
            .exclusive
            .or(manifest.door.exclusive)
            .or(manifest.access.exclusive)
            .unwrap_or(false),
        timeout_seconds,
        min_security_level,
        enabled_after_import_request,
        file_count: 0,
        total_unpacked_size: 0,
        warnings,
    })
}

fn verify_files(
    package_path: &Path,
    archive: &mut ZipArchive<File>,
    checksums: &HashMap<String, String>,
) -> Result<(usize, u64), DoorError> {
    let mut file_count = 0usize;
    let mut total_unpacked_size = 0u64;
    let mut verified = HashSet::new();

    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(|source| DoorError::InvalidDoorPackage {
            path: package_path.to_path_buf(),
            message: format!("invalid archive entry at index {index}: {source}"),
        })?;
        let entry_name = entry.name().to_string();
        validate_entry_name(package_path, &entry_name)?;
        validate_entry_mode(package_path, &entry_name, &entry)?;

        if entry_name == OXDOOR_MANIFEST_FILE || entry_name == OXDOOR_CHECKSUM_FILE {
            continue;
        }
        if entry.is_dir() || entry_name.ends_with('/') {
            continue;
        }
        if !entry_name.starts_with(FILES_DIRECTORY) {
            continue;
        }
        let entry_relative = entry_name.trim_start_matches(FILES_DIRECTORY);
        if entry_relative.is_empty() {
            return Err(DoorError::InvalidDoorPackage {
                path: package_path.to_path_buf(),
                message: "files/ entry path is invalid".to_string(),
            });
        }
        let expected = checksums
            .get(&entry_name)
            .or_else(|| checksums.get(&format!("./{entry_name}")))
            .or_else(|| checksums.get(entry_relative))
            .or_else(|| checksums.get(&format!("./{entry_relative}")));
        let expected = expected.ok_or_else(|| {
            DoorError::InvalidDoorPackage {
                path: package_path.to_path_buf(),
                message: format!("missing checksum for file {entry_relative:?}"),
            }
        })?;
        verify_file_checksum(package_path, &mut entry, expected)?;
        verified.insert(format!("files/{entry_relative}"));
        file_count += 1;
        total_unpacked_size = total_unpacked_size.saturating_add(entry.size());
    }

    for expected_path in checksums.keys() {
        let normalized = normalize_for_lookup(expected_path);
        if normalized.starts_with(FILES_DIRECTORY) && !verified.contains(&normalized) {
            return Err(DoorError::InvalidDoorPackage {
                path: package_path.to_path_buf(),
                message: format!("checksum entry has no matching files/ payload: {expected_path}"),
            });
        }
    }

    if file_count == 0 {
        return Err(DoorError::InvalidDoorPackage {
            path: package_path.to_path_buf(),
            message: "package.files directory must contain at least one regular file".to_string(),
        });
    }

    Ok((file_count, total_unpacked_size))
}

fn read_text_entry(
    archive: &mut ZipArchive<File>,
    package_path: &Path,
    entry_name: &str,
) -> Result<String, DoorError> {
    let mut entry = archive.by_name(entry_name).map_err(|_| DoorError::InvalidDoorPackage {
        path: package_path.to_path_buf(),
        message: format!("missing required entry: {entry_name}"),
    })?;
    let mut text = String::new();
    entry.read_to_string(&mut text).map_err(|source| {
        DoorError::ReadDoorPackage {
            path: package_path.to_path_buf(),
            source,
        }
    })?;
    Ok(text)
}

fn normalize_for_lookup(value: &str) -> String {
    value
        .trim_start_matches("./")
        .trim_end_matches('/')
        .to_string()
}

fn normalize_and_validate_checksum_path(
    package_path: &Path,
    value: &str,
) -> Result<String, DoorError> {
    if value.contains('\\') {
        return Err(DoorError::InvalidDoorPackage {
            path: package_path.to_path_buf(),
            message: format!("invalid checksum path {value:?}: backslashes are not allowed"),
        });
    }
    let normalized = normalize_for_lookup(value);
    validate_entry_name(package_path, &normalized)?;
    if normalized.is_empty() {
        return Err(DoorError::InvalidDoorPackage {
            path: package_path.to_path_buf(),
            message: "empty checksum path".to_string(),
        });
    }
    Ok(normalized)
}

fn normalize_drop_file(package_path: &Path, value: &str) -> Result<String, DoorError> {
    let normalized = value.trim().to_ascii_uppercase();
    if normalized.is_empty() {
        return Err(DoorError::InvalidDoorPackage {
            path: package_path.to_path_buf(),
            message: "drop-file format must not be blank".to_string(),
        });
    }
    if !OXDOOR_SUPPORTED_DROPFILES.contains(&normalized.as_str()) {
        return Err(DoorError::InvalidDoorPackage {
            path: package_path.to_path_buf(),
            message: format!("unsupported drop-file format: {value}"),
        });
    }
    Ok(normalized)
}

fn validate_entry_name(package_path: &Path, entry_name: &str) -> Result<(), DoorError> {
    if entry_name.contains('\\') {
        return Err(DoorError::InvalidDoorPackage {
            path: package_path.to_path_buf(),
            message: format!("invalid path {entry_name:?}: backslashes are not allowed"),
        });
    }
    if entry_name.starts_with('/') {
        return Err(DoorError::InvalidDoorPackage {
            path: package_path.to_path_buf(),
            message: format!("invalid path {entry_name:?}: absolute paths are not allowed"),
        });
    }
    if is_windows_drive_path(entry_name) {
        return Err(DoorError::InvalidDoorPackage {
            path: package_path.to_path_buf(),
            message: format!(
                "invalid path {entry_name:?}: windows drive-style paths are not allowed"
            ),
        });
    }

    for component in Path::new(entry_name).components() {
        match component {
            Component::ParentDir | Component::CurDir | Component::RootDir => {
                return Err(DoorError::InvalidDoorPackage {
                    path: package_path.to_path_buf(),
                    message: format!("invalid path {entry_name:?}: traversal is not allowed"),
                });
            }
            Component::Normal(_) | Component::Prefix(_) => {}
        }
    }

    Ok(())
}

fn validate_relative_directory(
    package_path: &Path,
    field_name: &str,
    value: &str,
) -> Result<(), DoorError> {
    if value.starts_with('/') {
        return Err(DoorError::InvalidDoorPackage {
            path: package_path.to_path_buf(),
            message: format!("{field_name} must be relative, got {value:?}"),
        });
    }
    if value.contains('\\') || value.contains("..") || is_windows_drive_path(value) {
        return Err(DoorError::InvalidDoorPackage {
            path: package_path.to_path_buf(),
            message: format!("{field_name} contains unsupported path characters: {value:?}"),
        });
    }
    Ok(())
}

fn validate_entry_mode(
    package_path: &Path,
    entry_name: &str,
    entry: &zip::read::ZipFile<'_, File>,
) -> Result<(), DoorError> {
    let Some(mode) = entry.unix_mode() else {
        return Ok(());
    };
    let kind = mode & 0o170000;
    if kind == 0o120000 {
        return Err(DoorError::InvalidDoorPackage {
            path: package_path.to_path_buf(),
            message: format!("archive entry {entry_name:?} is a symlink"),
        });
    }
    if kind != 0o100000 && kind != 0o040000 {
        return Err(DoorError::InvalidDoorPackage {
            path: package_path.to_path_buf(),
            message: format!(
                "archive entry {entry_name:?} is unsupported file type (mode {mode:o})"
            ),
        });
    }
    Ok(())
}

fn verify_file_checksum(
    package_path: &Path,
    entry: &mut zip::read::ZipFile<'_, File>,
    expected_hex: &str,
) -> Result<(), DoorError> {
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 8192];
    loop {
        let count = entry.read(&mut buffer).map_err(|source| {
            DoorError::ReadDoorPackage {
                path: package_path.to_path_buf(),
                source,
            }
        })?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    let actual = hex::encode(hasher.finalize());
    if actual != expected_hex.to_ascii_lowercase() {
        return Err(DoorError::InvalidDoorPackage {
            path: package_path.to_path_buf(),
            message: format!(
                "checksum mismatch for {:?}: expected {expected_hex}, got {actual}",
                entry.name()
            ),
        });
    }
    Ok(())
}

fn is_windows_drive_path(value: &str) -> bool {
    let mut chars = value.chars();
    matches!((chars.next(), chars.next()), (Some(drive), Some(':')) if drive.is_ascii_alphabetic())
}

fn validate_id_characters(package_path: &Path, field: &str, value: &str) -> Result<(), DoorError> {
    if value.chars().any(|ch| ch == '/' || ch == '\\' || ch == ':') {
        return Err(DoorError::InvalidDoorPackage {
            path: package_path.to_path_buf(),
            message: format!("{field} must not contain path separators"),
        });
    }
    Ok(())
}

fn trim_required(value: &str) -> Option<String> {
    let value = value.trim().to_string();
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::io;
    use std::time::{SystemTime, UNIX_EPOCH};
    use zip::write::{FileOptions, SimpleFileOptions};
    use zip::ZipWriter;

    use crate::OXDOOR_PACKAGE_FORMAT;

    fn build_manifest(override_runner: &str, preferred_drop_file: &str, kind: &str) -> String {
        format!(
            r#"
[package]
format = "{OXDOOR_PACKAGE_FORMAT}"
kind = "{kind}"
id = "sample-package"
name = "Sample Package"
version = "1.0.0"
requires_key = false

[legal]
status = "freeware"
requires_key = false

[source]
url = "https://example.invalid/sample"

[door]
id = "sample-door"
name = "Sample Door"
runner = "{override_runner}"
command = "START.BAT"
working_directory = "sample"
category = "game"
preferred_drop_file = "{preferred_drop_file}"
supported_drop_files = ["DOOR.SYS", "DORINFO1.DEF", "CHAIN.TXT"]
exclusive = true
timeout_seconds = 120
enabled_after_import = true

[access]
min_security_level = 10
preferred_drop_file = "{preferred_drop_file}"
supported_drop_files = ["DOOR.SYS", "DORINFO1.DEF", "CHAIN.TXT"]
exclusive = true
timeout_seconds = 120

[persistence]
enabled_after_import_request = true

[test]
timeout_seconds = 120

[menu]
category = "doors"
exclusive = true
timeout_seconds = 120
"#
        )
    }

    fn write_fixture(
        path: &Path,
        manifest: &str,
        files: &[(&str, &[u8])],
        include_checksums: bool,
        include_traversal: bool,
        checksum_overrides: Option<&HashMap<String, String>>,
    ) -> io::Result<()> {
        let file = File::create(path)?;
        let mut writer = ZipWriter::new(file);
        let options: FileOptions<'_, ()> =
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);

        writer.start_file(OXDOOR_MANIFEST_FILE, options)?;
        writer.write_all(manifest.as_bytes())?;

        let mut checksums = HashMap::new();
        for (name, bytes) in files {
            writer.start_file(format!("{FILES_DIRECTORY}{name}"), options)?;
            writer.write_all(bytes)?;
            checksums.insert(
                format!("{FILES_DIRECTORY}{name}"),
                hex::encode(Sha256::digest(bytes)),
            );
        }

        if include_traversal {
            let name = "files/../outside.bin";
            writer.start_file(name, options)?;
            writer.write_all(b"bad")?;
            checksums.insert(name.to_string(), hex::encode(Sha256::digest(b"bad")));
        }

        if include_checksums {
            let mut entries = checksums;
            if let Some(overrides) = checksum_overrides {
                for (name, digest) in overrides {
                    entries.insert(name.clone(), digest.clone());
                }
            }
            let checksum_contents = entries
                .into_iter()
                .map(|(name, digest)| format!("{digest}  {name}\n"))
                .collect::<String>();
            writer.start_file(OXDOOR_CHECKSUM_FILE, options)?;
            writer.write_all(checksum_contents.as_bytes())?;
        }

        writer.finish()?;
        Ok(())
    }

    fn temp_dir() -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "oxidebbs-oxdoor-test-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("timestamp")
                .as_nanos()
        ));
        std::fs::create_dir_all(&path).expect("create temp dir");
        path
    }

    fn cleanup(path: &Path) {
        let _ = std::fs::remove_dir_all(path);
    }

    #[test]
    fn inspect_valid_oxide_door_package() {
        let temp = temp_dir();
        let package_path = temp.join("valid.oxdoor");

        write_fixture(
            &package_path,
            &build_manifest("local:dosemu2", "DOOR.SYS", "full"),
            &[("readme.txt", b"hello\n"), ("readme2.txt", b"world\n")],
            true,
            false,
            None,
        )
        .expect("write fixture");

        let summary = inspect_oxide_door_package(&package_path).expect("inspect");
        assert_eq!(summary.package_name, "Sample Package");
        assert_eq!(summary.package_id, "sample-package");
        assert_eq!(summary.legal_status, "freeware");
        assert_eq!(summary.door_category, "doors");
        assert_eq!(summary.runner, "local:dosemu2");
        assert_eq!(summary.timeout_seconds, 120);
        assert_eq!(summary.file_count, 2);
        assert_eq!(summary.total_unpacked_size, 12);
        assert_eq!(summary.preferred_drop_file, "DOOR.SYS");
        assert_eq!(summary.supported_drop_files, vec!["CHAIN.TXT","DORINFO1.DEF","DOOR.SYS"]);
        cleanup(&temp);
    }

    #[test]
    fn inspect_package_requires_manifest() {
        let temp = temp_dir();
        let package_path = temp.join("missing-manifest.oxdoor");
        write_fixture(
            &package_path,
            &build_manifest("local:dosemu2", "DOOR.SYS", "full"),
            &[],
            true,
            false,
            None,
        )
        .expect("write");
        {
            let file = File::create(&package_path).expect("open");
            let mut writer = ZipWriter::new(file);
            let options: FileOptions<'_, ()> =
                SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
            writer.start_file(OXDOOR_CHECKSUM_FILE, options).unwrap();
            writer
                .write_all(b"e3b0c44298fc1c149afb...  files/readme.txt\n")
                .unwrap();
            writer.finish().unwrap();
        }
        let error = inspect_oxide_door_package(&package_path).expect_err("missing manifest");
        assert!(error.to_string().contains("missing required entry"));
        cleanup(&temp);
    }

    #[test]
    fn inspect_package_requires_checksums() {
        let temp = temp_dir();
        let package_path = temp.join("missing-checksums.oxdoor");
        write_fixture(
            &package_path,
            &build_manifest("local:dosemu2", "DOOR.SYS", "full"),
            &[("readme.txt", b"hello")],
            false,
            false,
            None,
        )
        .expect("write");
        let error = inspect_oxide_door_package(&package_path).expect_err("missing checksums");
        assert!(error.to_string().contains("missing required entry"));
        cleanup(&temp);
    }

    #[test]
    fn inspect_package_rejects_invalid_kind() {
        let temp = temp_dir();
        let package_path = temp.join("bad-kind.oxdoor");
        write_fixture(
            &package_path,
            &build_manifest("local:dosemu2", "DOOR.SYS", "recipe"),
            &[("readme.txt", b"hello")],
            true,
            false,
            None,
        )
        .expect("write");
        let error = inspect_oxide_door_package(&package_path).expect_err("bad kind");
        assert!(error.to_string().contains("unsupported package kind"));
        cleanup(&temp);
    }

    #[test]
    fn inspect_package_rejects_unsupported_runner() {
        let temp = temp_dir();
        let package_path = temp.join("bad-runner.oxdoor");
        write_fixture(
            &package_path,
            &build_manifest("remote:doorparty", "DOOR.SYS", "full"),
            &[("readme.txt", b"hello")],
            true,
            false,
            None,
        )
        .expect("write");
        let error = inspect_oxide_door_package(&package_path).expect_err("bad runner");
        assert!(error.to_string().contains("unsupported runner"));
        cleanup(&temp);
    }

    #[test]
    fn inspect_package_rejects_unsupported_drop_file() {
        let temp = temp_dir();
        let package_path = temp.join("bad-drop.oxdoor");
        write_fixture(
            &package_path,
            &build_manifest("local:dosemu2", "BAD.DROP", "full"),
            &[("readme.txt", b"hello")],
            true,
            false,
            None,
        )
        .expect("write");
        let error = inspect_oxide_door_package(&package_path).expect_err("bad drop file");
        assert!(error.to_string().contains("unsupported drop-file format"));
        cleanup(&temp);
    }

    #[test]
    fn inspect_package_rejects_checksum_mismatch() {
        let temp = temp_dir();
        let package_path = temp.join("bad-checksum.oxdoor");
        let overrides =
            HashMap::from([("files/readme.txt".to_string(), "ff".repeat(32) + "11")]);
        write_fixture(
            &package_path,
            &build_manifest("local:dosemu2", "DOOR.SYS", "full"),
            &[("readme.txt", b"hello")],
            true,
            false,
            Some(&overrides),
        )
        .expect("write");
        let error = inspect_oxide_door_package(&package_path).expect_err("checksum mismatch");
        assert!(error.to_string().contains("checksum mismatch"));
        cleanup(&temp);
    }

    #[test]
    fn inspect_package_rejects_path_traversal_in_files() {
        let temp = temp_dir();
        let package_path = temp.join("bad-path.oxdoor");
        write_fixture(
            &package_path,
            &build_manifest("local:dosemu2", "DOOR.SYS", "full"),
            &[("readme.txt", b"hello")],
            true,
            true,
            None,
        )
        .expect("write");
        let error = inspect_oxide_door_package(&package_path).expect_err("path traversal");
        assert!(
            error.to_string().contains("traversal") || error.to_string().contains("not allowed")
        );
        cleanup(&temp);
    }
}
