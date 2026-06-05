use crate::SysopError;
use std::fs;
use std::path::{Path, PathBuf};

pub struct LogService;

pub struct LogEntry {
    pub timestamp: String,
    pub level: String,
    pub target: String,
    pub message: String,
}

impl LogService {
    pub fn tail(log_path: &Path, lines: usize) -> Result<Vec<LogEntry>, SysopError> {
        let files = log_files(log_path)?;
        if files.is_empty() {
            return Ok(Vec::new());
        }
        let mut all_lines = Vec::new();
        for file in files {
            let content = fs::read_to_string(file).map_err(SysopError::Io)?;
            all_lines.extend(content.lines().map(ToOwned::to_owned));
        }
        let entries: Vec<LogEntry> = all_lines
            .iter()
            .rev()
            .take(lines)
            .rev()
            .map(|line| {
                // Parse log line format: "TIMESTAMP LEVEL TARGET MESSAGE"
                let parts: Vec<&str> = line.splitn(4, ' ').collect();
                LogEntry {
                    timestamp: parts.first().copied().unwrap_or("").to_string(),
                    level: parts.get(1).copied().unwrap_or("").to_string(),
                    target: parts.get(2).copied().unwrap_or("").to_string(),
                    message: parts.get(3).copied().unwrap_or("").to_string(),
                }
            })
            .collect();
        Ok(entries)
    }

    pub fn export(entries: &[LogEntry], output: &Path) -> Result<(), SysopError> {
        if let Some(parent) = output.parent()
            && !parent.as_os_str().is_empty()
        {
            fs::create_dir_all(parent).map_err(SysopError::Io)?;
        }
        let mut text = String::new();
        for entry in entries {
            text.push_str(&format!(
                "{}\t{}\t{}\t{}\n",
                entry.timestamp, entry.level, entry.target, entry.message
            ));
        }
        fs::write(output, text).map_err(SysopError::Io)?;
        Ok(())
    }
}

fn log_files(log_path: &Path) -> Result<Vec<PathBuf>, SysopError> {
    if log_path.is_file() {
        return Ok(vec![log_path.to_path_buf()]);
    }
    if !log_path.exists() {
        return Ok(Vec::new());
    }
    let mut files = Vec::new();
    collect_log_files(log_path, &mut files)?;
    files.sort();
    Ok(files)
}

fn collect_log_files(path: &Path, files: &mut Vec<PathBuf>) -> Result<(), SysopError> {
    for entry in fs::read_dir(path).map_err(SysopError::Io)? {
        let path = entry.map_err(SysopError::Io)?.path();
        if path.is_dir() {
            collect_log_files(&path, files)?;
        } else if path.is_file() {
            files.push(path);
        }
    }
    Ok(())
}
