//! Tracing subscriber initialization.
//!
//! Logs are written to a file instead of being captured in-app.
//! Users can monitor logs via `tail -f` in a separate terminal.

use std::path::Path;

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
/// * `Err(msg)` if the subscriber was already initialized or directory creation failed
pub fn init(_log_path: &Path) -> Result<(), String> {
    todo!("logging::init")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn init_creates_log_directory_if_missing() {
        let temp_dir = std::env::temp_dir();
        let test_dir = temp_dir.join("cclv_test_logs_create");
        let log_file = test_dir.join("test.log");

        // Ensure directory doesn't exist
        let _ = fs::remove_dir_all(&test_dir);

        // Initialize logging
        let result = init(&log_file);

        // Should succeed
        assert!(
            result.is_ok(),
            "init should succeed and create directory: {:?}",
            result
        );

        // Directory should exist
        assert!(
            test_dir.exists(),
            "Log directory should be created: {:?}",
            test_dir
        );

        // Cleanup
        let _ = fs::remove_dir_all(&test_dir);
    }

    #[test]
    fn init_succeeds_when_directory_already_exists() {
        let temp_dir = std::env::temp_dir();
        let test_dir = temp_dir.join("cclv_test_logs_exists");
        let log_file = test_dir.join("test.log");

        // Ensure directory exists
        let _ = fs::create_dir_all(&test_dir);

        // Initialize logging
        let result = init(&log_file);

        // Should succeed
        assert!(
            result.is_ok(),
            "init should succeed when directory exists: {:?}",
            result
        );

        // Cleanup
        let _ = fs::remove_dir_all(&test_dir);
    }

    #[test]
    fn init_writes_to_configured_file() {
        let temp_dir = std::env::temp_dir();
        let test_dir = temp_dir.join("cclv_test_logs_write");
        let log_file = test_dir.join("app.log");

        // Cleanup any previous test artifacts
        let _ = fs::remove_dir_all(&test_dir);

        // Initialize logging
        let _ = init(&log_file);

        // Write a test log entry using tracing
        tracing::info!("test log entry");

        // Give async writer time to flush (tracing-appender is async)
        std::thread::sleep(std::time::Duration::from_millis(100));

        // Log file should exist
        assert!(
            log_file.exists(),
            "Log file should exist after logging: {:?}",
            log_file
        );

        // Log file should contain content
        let contents = fs::read_to_string(&log_file).expect("Failed to read log file");
        assert!(
            contents.contains("test log entry"),
            "Log file should contain the test entry, got: {}",
            contents
        );

        // Cleanup
        let _ = fs::remove_dir_all(&test_dir);
    }

    #[test]
    fn init_respects_rust_log_env_var() {
        let temp_dir = std::env::temp_dir();
        let test_dir = temp_dir.join("cclv_test_logs_env");
        let log_file = test_dir.join("env.log");

        // Cleanup
        let _ = fs::remove_dir_all(&test_dir);

        // Set RUST_LOG to debug level
        std::env::set_var("RUST_LOG", "debug");

        // Initialize logging
        let _ = init(&log_file);

        // Write debug and trace entries
        tracing::debug!("debug entry");
        tracing::trace!("trace entry");

        std::thread::sleep(std::time::Duration::from_millis(100));

        // Read log file
        let contents = fs::read_to_string(&log_file).expect("Failed to read log file");

        // Debug should be present (level=debug)
        assert!(
            contents.contains("debug entry"),
            "Log should contain debug entry when RUST_LOG=debug, got: {}",
            contents
        );

        // Trace should NOT be present (level < debug)
        assert!(
            !contents.contains("trace entry"),
            "Log should NOT contain trace entry when RUST_LOG=debug, got: {}",
            contents
        );

        // Cleanup
        std::env::remove_var("RUST_LOG");
        let _ = fs::remove_dir_all(&test_dir);
    }
}
