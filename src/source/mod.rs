//! Log input sources.
//!
//! This module provides input sources for JSONL log data:
//! - File loading for read-once file input
//! - Stdin for piped input (live streaming)
//! - Unified InputSource enum for both

use crate::model::LogEntry;
use crate::model::error::InputError;
use std::path::PathBuf;

pub mod file;
pub mod stdin;

pub use file::FileSource;
pub use stdin::StdinSource;

/// Unified input source for JSONL log data.
///
/// Abstracts over file loading and stdin sources with a common interface.
/// Sum type enforces exactly one variant.
#[derive(Debug)]
pub enum InputSource {
    /// File source - read-once loading (FR-007)
    File(FileSource),
    /// Stdin source - reads from piped stdin (live streaming)
    Stdin(StdinSource),
}

impl InputSource {
    /// Poll for new entries from the input source.
    ///
    /// Returns parsed LogEntry, not raw strings (parse at boundary).
    /// Non-blocking - returns immediately with available entries.
    ///
    /// # Behavior:
    /// - File: all entries on first call, empty vec after
    /// - Stdin: incremental as data arrives
    ///
    /// # Errors
    ///
    /// Returns `InputError` for I/O errors.
    pub fn poll(&mut self) -> Result<Vec<LogEntry>, InputError> {
        match self {
            InputSource::File(f) => f.drain_entries(),
            InputSource::Stdin(s) => s.poll_and_parse(),
        }
    }

    /// Check if the source is still live (can receive more data).
    ///
    /// # Behavior:
    /// - File: always false (static, read-once)
    /// - Stdin: true until EOF is reached
    pub fn is_live(&self) -> bool {
        match self {
            InputSource::File(_) => false,
            InputSource::Stdin(s) => !s.is_complete(),
        }
    }
}

/// Detect and create appropriate input source.
///
/// # Logic:
/// 1. If file path is provided: create FileSource (loads on construction)
/// 2. If stdin is piped: use StdinSource
/// 3. Else: return InputError::NoInput
///
/// # Arguments
///
/// * `file` - Optional file path
///
/// # Errors
///
/// Returns `InputError::NoInput` if no file is provided and stdin is not piped.
/// Returns `InputError::FileNotFound` if file does not exist.
/// Returns `InputError::Io` for I/O errors during file reading.
pub fn detect_input_source(file: Option<PathBuf>) -> Result<InputSource, InputError> {
    match file {
        Some(path) => Ok(InputSource::File(FileSource::new(path)?)),
        None => Ok(InputSource::Stdin(StdinSource::new()?)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::IsTerminal;
    use std::thread;
    use std::time::Duration;

    // ========================================================================
    // InputSource::poll() tests - File variant
    // ========================================================================

    #[test]
    fn poll_returns_all_entries_on_first_call_for_file() {
        use std::fs;

        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join("poll_test_file_first_call.jsonl");

        // Create file with 2 valid entries
        let content = r#"{"type":"user","message":{"role":"user","content":"First"},"sessionId":"s1","uuid":"u1","timestamp":"2025-12-27T10:00:00Z"}
{"type":"assistant","message":{"role":"assistant","content":"Second"},"sessionId":"s1","uuid":"u2","timestamp":"2025-12-27T10:00:01Z"}
"#;
        fs::write(&test_file, content).unwrap();

        let mut source = detect_input_source(Some(test_file.clone())).unwrap();

        // Cleanup
        let _ = fs::remove_file(&test_file);

        let result = source.poll().unwrap();

        assert_eq!(result.len(), 2, "Should return all 2 entries on first poll");
        assert_eq!(result[0].uuid().as_str(), "u1");
        assert_eq!(result[1].uuid().as_str(), "u2");
    }

    #[test]
    fn poll_returns_empty_vec_on_subsequent_calls_for_file() {
        use std::fs;

        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join("poll_test_file_subsequent.jsonl");

        // Create file with 1 entry
        let content = r#"{"type":"user","message":{"role":"user","content":"Entry"},"sessionId":"s1","uuid":"u1","timestamp":"2025-12-27T10:00:00Z"}
"#;
        fs::write(&test_file, content).unwrap();

        let mut source = detect_input_source(Some(test_file.clone())).unwrap();

        // Cleanup
        let _ = fs::remove_file(&test_file);

        // First poll drains all entries
        let first = source.poll().unwrap();
        assert_eq!(first.len(), 1);

        // Second poll should return empty
        let second = source.poll().unwrap();
        assert_eq!(second.len(), 0, "Second poll should return empty vec");

        // Third poll should also return empty
        let third = source.poll().unwrap();
        assert_eq!(third.len(), 0, "Third poll should return empty vec");
    }

    // ========================================================================
    // InputSource::poll() tests - Stdin variant
    // ========================================================================
    // Note: StdinSource has its own comprehensive tests in stdin.rs
    // These tests verify InputSource::poll() integration only

    #[test]
    fn poll_returns_parsed_entries_for_stdin() {
        let data = b"{\"type\":\"user\",\"message\":{\"role\":\"user\",\"content\":\"Test\"},\"sessionId\":\"s1\",\"uuid\":\"u1\",\"timestamp\":\"2025-12-27T10:00:00Z\"}\n";
        let stdin_source = StdinSource::from_reader(&data[..]);
        let mut source = InputSource::Stdin(stdin_source);

        // Give background thread time to read
        thread::sleep(Duration::from_millis(50));

        let result = source.poll().unwrap();

        assert_eq!(result.len(), 1, "Should parse 1 entry from stdin");
        assert_eq!(result[0].uuid().as_str(), "u1");
    }

    // ========================================================================
    // InputSource::is_live() tests
    // ========================================================================

    #[test]
    fn is_live_returns_false_for_file_sources() {
        use std::fs;

        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join("is_live_test_file.jsonl");

        // Create file
        let content = r#"{"type":"user","message":{"role":"user","content":"Test"},"sessionId":"s1","uuid":"u1","timestamp":"2025-12-27T10:00:00Z"}
"#;
        fs::write(&test_file, content).unwrap();

        let source = detect_input_source(Some(test_file.clone())).unwrap();

        // Cleanup
        let _ = fs::remove_file(&test_file);

        assert!(
            !source.is_live(),
            "File sources are never live (static, read-once)"
        );
    }

    #[test]
    fn is_live_returns_true_for_stdin_before_eof() {
        let data = b"{\"type\":\"user\",\"message\":{\"role\":\"user\",\"content\":\"Test\"},\"sessionId\":\"s1\",\"uuid\":\"u1\",\"timestamp\":\"2025-12-27T10:00:00Z\"}\n";
        let stdin_source = StdinSource::from_reader(&data[..]);
        let source = InputSource::Stdin(stdin_source);

        // Before polling or before EOF is detected
        assert!(source.is_live(), "Stdin source should be live before EOF");
    }

    #[test]
    fn is_live_returns_false_for_stdin_after_eof() {
        let data = b"{\"type\":\"user\",\"message\":{\"role\":\"user\",\"content\":\"Test\"},\"sessionId\":\"s1\",\"uuid\":\"u1\",\"timestamp\":\"2025-12-27T10:00:00Z\"}\n";
        let stdin_source = StdinSource::from_reader(&data[..]);
        let mut source = InputSource::Stdin(stdin_source);

        // Drain all data
        thread::sleep(Duration::from_millis(50));
        source.poll().unwrap();

        // Wait for EOF message
        thread::sleep(Duration::from_millis(50));
        source.poll().unwrap();

        assert!(
            !source.is_live(),
            "Stdin source should not be live after EOF"
        );
    }

    // ========================================================================
    // detect_input_source() tests
    // ========================================================================

    // ========================================================================
    // detect_input_source() tests - File variant
    // ========================================================================

    #[test]
    fn detect_returns_file_source_for_existing_file() {
        use std::fs;

        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join("detect_test_existing_file.jsonl");

        // Create a valid test file
        let content = r#"{"type":"user","message":{"role":"user","content":"Test"},"sessionId":"s1","uuid":"u1","timestamp":"2025-12-27T10:00:00Z"}
"#;
        fs::write(&test_file, content).unwrap();

        let result = detect_input_source(Some(test_file.clone()));

        // Cleanup
        let _ = fs::remove_file(&test_file);

        assert!(
            result.is_ok(),
            "Should return File variant for existing file"
        );
        let source = result.unwrap();
        assert!(
            matches!(source, InputSource::File(_)),
            "Should be File variant, got: {:?}",
            source
        );
    }

    #[test]
    fn detect_returns_file_not_found_for_missing_file() {
        let temp_dir = std::env::temp_dir();
        let missing_file = temp_dir.join("nonexistent_detect_test_12345.jsonl");

        let result = detect_input_source(Some(missing_file.clone()));

        assert!(
            matches!(result, Err(InputError::FileNotFound { .. })),
            "Should return FileNotFound for missing file, got: {:?}",
            result
        );
        if let Err(InputError::FileNotFound { path }) = result {
            assert_eq!(path, missing_file);
        }
    }

    #[test]
    fn detect_returns_no_input_when_no_file_and_stdin_is_tty() {
        // Note: This test assumes stdin IS a TTY in the test environment
        // If stdin is piped during tests, this test would need to be skipped
        // For now, we test the error case directly by simulating the condition

        // Calling with None should attempt to use stdin
        // In test environment, stdin is typically a TTY, so we expect NoInput
        let result = detect_input_source(None);

        // This test may not be reliable if tests are run with piped stdin
        // The behavior should be: None file + TTY stdin = NoInput error
        if std::io::stdin().is_terminal() {
            assert!(
                matches!(result, Err(InputError::NoInput)),
                "Expected NoInput error when no file and stdin is TTY, got: {:?}",
                result
            );
        }
    }

    #[test]
    fn detect_error_message_is_user_friendly() {
        let result = detect_input_source(None);

        if let Err(e) = result {
            let msg = e.to_string();
            // Should match the error message from InputError::NoInput
            assert!(
                msg.contains("No input source"),
                "Error message should mention 'No input source', got: {}",
                msg
            );
            assert!(
                msg.contains("file path") || msg.contains("pipe"),
                "Error message should mention file path or piping, got: {}",
                msg
            );
        }
    }
}
