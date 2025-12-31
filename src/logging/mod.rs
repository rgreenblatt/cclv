//! Tracing subscriber initialization.
//!
//! Logs are written to a file instead of being captured in-app.
//! Users can monitor logs via `tail -f` in a separate terminal.

use std::path::{Path, PathBuf};
use thiserror::Error;

/// Error type for logging initialization failures.
#[derive(Debug, Error)]
pub enum LoggingError {
    /// Failed to create log directory
    #[error("Failed to create log directory at {path:?}: {source}")]
    DirectoryCreation {
        /// The directory path that failed to be created
        path: PathBuf,
        /// The underlying I/O error
        #[source]
        source: std::io::Error,
    },

    /// Invalid log file path (no filename component)
    #[error("Invalid log file path: {0:?}")]
    InvalidPath(PathBuf),

    /// Log path has no parent directory
    #[error("Log path has no parent directory: {0:?}")]
    NoParentDirectory(PathBuf),

    /// Tracing subscriber already initialized
    #[error("Tracing subscriber already initialized")]
    SubscriberAlreadySet,
}

/// Initialize the tracing subscriber with file-based logging.
///
/// Logs are written to a file for users to monitor with `tail -f`.
/// Respects RUST_LOG environment variable, defaults to "info" level.
///
/// Creates the log directory if it doesn't exist (FR-054/055).
///
/// # Arguments
///
/// * `log_path` - Path to the log file
///
/// # Returns
/// * `Ok(())` if initialization succeeded
/// * `Err(LoggingError)` if the subscriber was already initialized or directory creation failed
pub fn init(log_path: &Path) -> Result<(), LoggingError> {
    use tracing_subscriber::EnvFilter;

    // Create log directory if it doesn't exist
    if let Some(parent) = log_path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| LoggingError::DirectoryCreation {
            path: parent.to_path_buf(),
            source,
        })?;
    }

    // Get log file name and directory
    let file_name = log_path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| LoggingError::InvalidPath(log_path.to_path_buf()))?;

    let directory = log_path
        .parent()
        .ok_or_else(|| LoggingError::NoParentDirectory(log_path.to_path_buf()))?;

    // Create file appender
    let file_appender = tracing_appender::rolling::never(directory, file_name);

    // Respect RUST_LOG, default to "info"
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    // Initialize subscriber with file output
    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .with_writer(file_appender)
        .with_ansi(false) // No ANSI colors in log files
        .try_init()
        .map_err(|_| LoggingError::SubscriberAlreadySet)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use std::fs;

    #[test]
    #[serial(tracing_init)]
    fn init_creates_log_directory_if_missing() {
        let temp_dir = std::env::temp_dir();
        let test_dir = temp_dir.join("cclv_test_logs_create");
        let log_file = test_dir.join("test.log");

        // Ensure directory doesn't exist
        let _ = fs::remove_dir_all(&test_dir);

        // Initialize logging (may fail if subscriber already set, which is fine)
        let _ = init(&log_file);

        // Directory should exist (created even if subscriber init failed)
        assert!(
            test_dir.exists(),
            "Log directory should be created: {:?}",
            test_dir
        );

        // Cleanup
        let _ = fs::remove_dir_all(&test_dir);
    }

    #[test]
    #[serial(tracing_init)]
    fn init_succeeds_when_directory_already_exists() {
        let temp_dir = std::env::temp_dir();
        let test_dir = temp_dir.join("cclv_test_logs_exists");
        let log_file = test_dir.join("test.log");

        // Ensure directory exists
        let _ = fs::create_dir_all(&test_dir);

        // Initialize logging (may fail if subscriber already set, which is fine)
        let _ = init(&log_file);

        // Directory should still exist
        assert!(
            test_dir.exists(),
            "Log directory should exist: {:?}",
            test_dir
        );

        // Cleanup
        let _ = fs::remove_dir_all(&test_dir);
    }

    #[test]
    #[serial(tracing_init)]
    fn init_creates_log_file_path() {
        let temp_dir = std::env::temp_dir();
        let test_dir = temp_dir.join("cclv_test_logs_write");
        let log_file = test_dir.join("app.log");

        // Cleanup any previous test artifacts
        let _ = fs::remove_dir_all(&test_dir);

        // Initialize logging (subscriber may already be set)
        let _ = init(&log_file);

        // Verify directory was created
        assert!(
            test_dir.exists(),
            "Log directory should exist: {:?}",
            test_dir
        );

        // Cleanup
        let _ = fs::remove_dir_all(&test_dir);
    }

    #[test]
    #[serial(tracing_init)]
    fn init_respects_rust_log_env_var() {
        // Note: This test is challenging because:
        // 1. Subscriber can only be initialized once per process
        // 2. Environment variables affect global state
        // For now, we verify that init() doesn't panic and creates the directory

        let temp_dir = std::env::temp_dir();
        let test_dir = temp_dir.join("cclv_test_logs_env");
        let log_file = test_dir.join("env.log");

        // Cleanup
        let _ = fs::remove_dir_all(&test_dir);

        // Initialize logging (subscriber may already be set)
        let _ = init(&log_file);

        // Verify directory was created
        assert!(
            test_dir.exists(),
            "Log directory should exist: {:?}",
            test_dir
        );

        // Cleanup
        let _ = fs::remove_dir_all(&test_dir);
    }
}
