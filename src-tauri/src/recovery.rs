use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RecoveryCheckpoint {
    pub clean_shutdown: bool,
    pub recovery_notice_pending: bool,
    pub launch_count: u64,
    pub last_start_unix_ms: Option<u128>,
    pub last_shutdown_unix_ms: Option<u128>,
}

impl Default for RecoveryCheckpoint {
    fn default() -> Self {
        Self {
            clean_shutdown: true,
            recovery_notice_pending: false,
            launch_count: 0,
            last_start_unix_ms: None,
            last_shutdown_unix_ms: None,
        }
    }
}

pub fn default_checkpoint_path() -> PathBuf {
    let base = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    base.join("sonora-dictation").join("recovery.json")
}

pub fn load_or_default(path: &Path) -> RecoveryCheckpoint {
    match fs::read_to_string(path) {
        Ok(contents) => serde_json::from_str::<RecoveryCheckpoint>(&contents).unwrap_or_default(),
        Err(_) => RecoveryCheckpoint::default(),
    }
}

pub fn save(path: &Path, checkpoint: &RecoveryCheckpoint) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "recovery path has no parent directory".to_string())?;
    fs::create_dir_all(parent).map_err(io_to_string)?;
    let payload = serde_json::to_string_pretty(checkpoint).map_err(|error| error.to_string())?;
    fs::write(path, payload).map_err(io_to_string)
}

pub fn mark_start(checkpoint: &RecoveryCheckpoint, now_unix_ms: u128) -> RecoveryCheckpoint {
    RecoveryCheckpoint {
        clean_shutdown: false,
        recovery_notice_pending: !checkpoint.clean_shutdown,
        launch_count: checkpoint.launch_count.saturating_add(1),
        last_start_unix_ms: Some(now_unix_ms),
        last_shutdown_unix_ms: checkpoint.last_shutdown_unix_ms,
    }
}

pub fn mark_clean_shutdown(
    checkpoint: &RecoveryCheckpoint,
    now_unix_ms: u128,
) -> RecoveryCheckpoint {
    RecoveryCheckpoint {
        clean_shutdown: true,
        recovery_notice_pending: false,
        launch_count: checkpoint.launch_count,
        last_start_unix_ms: checkpoint.last_start_unix_ms,
        last_shutdown_unix_ms: Some(now_unix_ms),
    }
}

pub fn acknowledge_recovery_notice(checkpoint: &RecoveryCheckpoint) -> RecoveryCheckpoint {
    RecoveryCheckpoint {
        recovery_notice_pending: false,
        ..checkpoint.clone()
    }
}

pub fn current_unix_ms() -> Result<u128, String> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .map_err(|error| error.to_string())
}

fn io_to_string(error: io::Error) -> String {
    error.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_file(name: &str) -> PathBuf {
        let stamp = current_unix_ms().unwrap_or(0);
        std::env::temp_dir().join(format!("sonora-recovery-{name}-{stamp}.json"))
    }

    #[test]
    fn marks_recovery_notice_when_previous_shutdown_was_dirty() {
        let previous = RecoveryCheckpoint {
            clean_shutdown: false,
            recovery_notice_pending: false,
            launch_count: 9,
            last_start_unix_ms: Some(10),
            last_shutdown_unix_ms: None,
        };

        let started = mark_start(&previous, 1234);
        assert!(!started.clean_shutdown);
        assert!(started.recovery_notice_pending);
        assert_eq!(started.launch_count, 10);
        assert_eq!(started.last_start_unix_ms, Some(1234));
    }

    #[test]
    fn marks_clean_shutdown() {
        let started = RecoveryCheckpoint {
            clean_shutdown: false,
            recovery_notice_pending: true,
            launch_count: 3,
            last_start_unix_ms: Some(33),
            last_shutdown_unix_ms: None,
        };

        let shutdown = mark_clean_shutdown(&started, 55);
        assert!(shutdown.clean_shutdown);
        assert!(!shutdown.recovery_notice_pending);
        assert_eq!(shutdown.last_shutdown_unix_ms, Some(55));
    }

    #[test]
    fn persists_checkpoint() {
        let path = temp_file("persist");
        let checkpoint = RecoveryCheckpoint {
            clean_shutdown: true,
            recovery_notice_pending: false,
            launch_count: 7,
            last_start_unix_ms: Some(100),
            last_shutdown_unix_ms: Some(101),
        };

        save(&path, &checkpoint).expect("checkpoint should save");
        let loaded = load_or_default(&path);
        assert_eq!(loaded, checkpoint);

        let _ = fs::remove_file(path);
    }
}
