use crate::SysopError;
use std::fs;
use std::path::Path;

pub struct LogService;

pub struct LogEntry {
    pub timestamp: String,
    pub level: String,
    pub target: String,
    pub message: String,
}

impl LogService {
    pub fn tail(log_path: &Path, lines: usize) -> Result<Vec<LogEntry>, SysopError> {
        if !log_path.exists() {
            return Ok(Vec::new());
        }
        let content = fs::read_to_string(log_path).map_err(SysopError::Io)?;
        let all_lines: Vec<&str> = content.lines().collect();
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
}
