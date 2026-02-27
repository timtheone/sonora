use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeLogEntry {
    pub timestamp_unix_ms: u128,
    pub level: String,
    pub event: String,
    pub message: String,
}

pub fn default_log_path() -> PathBuf {
    let base = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    base.join("sonora-dictation").join("runtime.log")
}

pub fn append(path: &Path, level: &str, event: &str, message: &str) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "log path has no parent directory".to_string())?;
    fs::create_dir_all(parent).map_err(io_to_string)?;

    let timestamp_unix_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_millis();

    let line = serde_json::to_string(&RuntimeLogEntry {
        timestamp_unix_ms,
        level: level.to_string(),
        event: event.to_string(),
        message: message.to_string(),
    })
    .map_err(|error| error.to_string())?;

    let mut existing = String::new();
    if let Ok(contents) = fs::read_to_string(path) {
        existing = contents;
    }
    existing.push_str(&line);
    existing.push('\n');
    fs::write(path, existing).map_err(io_to_string)
}

pub fn read_recent(path: &Path, limit: usize) -> Result<Vec<String>, String> {
    if !path.exists() {
        return Ok(Vec::new());
    }

    let contents = fs::read_to_string(path).map_err(io_to_string)?;
    let lines = contents.lines().collect::<Vec<_>>();
    let take = lines.len().min(limit);
    Ok(lines[lines.len() - take..]
        .iter()
        .map(|line| (*line).to_string())
        .collect())
}

pub fn clear(path: &Path) -> Result<(), String> {
    if !path.exists() {
        return Ok(());
    }
    fs::remove_file(path).map_err(io_to_string)
}

fn io_to_string(error: io::Error) -> String {
    error.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_file(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be set")
            .as_nanos();
        std::env::temp_dir().join(format!("sonora-runtime-{name}-{nanos}.log"))
    }

    #[test]
    fn appends_and_reads_recent_logs() {
        let path = temp_file("append");
        append(&path, "info", "start", "app started").expect("first log should write");
        append(&path, "info", "tick", "heartbeat").expect("second log should write");

        let recent = read_recent(&path, 1).expect("recent logs should read");
        assert_eq!(recent.len(), 1);
        assert!(recent[0].contains("heartbeat"));

        let _ = clear(&path);
    }

    #[test]
    fn clear_removes_log_file() {
        let path = temp_file("clear");
        append(&path, "info", "start", "app started").expect("log should write");
        clear(&path).expect("clear should remove file");
        assert!(!path.exists());
    }
}
