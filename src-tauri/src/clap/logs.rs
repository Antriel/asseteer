//! CLAP server log file management
//!
//! Captures Python server stdout/stderr to timestamped log files in the app
//! data directory. Old logs are cleaned up on each server start.

use std::fs::{self, File};
use std::path::PathBuf;
use std::process::Stdio;

use super::uv;

/// Maximum number of log files to keep
const MAX_LOG_FILES: usize = 5;

/// Directory where CLAP server logs are stored.
pub fn log_dir() -> PathBuf {
    uv::app_data_dir().join("clap-logs")
}

/// Creates a new log file and returns Stdio handles for stdout and stderr.
/// Also cleans up old log files beyond the retention limit.
pub fn create_log_file() -> Result<(Stdio, Stdio, PathBuf), String> {
    let dir = log_dir();
    fs::create_dir_all(&dir).map_err(|e| format!("Failed to create log directory: {}", e))?;

    cleanup_old_logs(&dir);

    let timestamp = chrono::Local::now().format("%Y-%m-%dT%H-%M-%S");
    let log_path = dir.join(format!("clap-server-{}.log", timestamp));

    let stdout_file =
        File::create(&log_path).map_err(|e| format!("Failed to create log file: {}", e))?;
    let stderr_file = stdout_file
        .try_clone()
        .map_err(|e| format!("Failed to clone log file handle: {}", e))?;

    println!("[CLAP] Logging server output to {:?}", log_path);

    Ok((Stdio::from(stdout_file), Stdio::from(stderr_file), log_path))
}

/// Remove old log files, keeping only the most recent `MAX_LOG_FILES`.
fn cleanup_old_logs(dir: &std::path::Path) {
    let mut logs: Vec<_> = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .map(|ext| ext == "log")
                .unwrap_or(false)
        })
        .collect();

    if logs.len() < MAX_LOG_FILES {
        return;
    }

    // Sort by name (timestamp-based names sort chronologically)
    logs.sort_by_key(|e| e.file_name());

    let to_remove = logs.len() - MAX_LOG_FILES + 1; // +1 to make room for the new one
    for entry in logs.into_iter().take(to_remove) {
        let path = entry.path();
        if let Err(e) = fs::remove_file(&path) {
            println!("[CLAP] Failed to remove old log {:?}: {}", path, e);
        }
    }
}
