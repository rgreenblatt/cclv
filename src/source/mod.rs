//! Log input sources.
//!
//! This module provides input sources for JSONL log data:
//! - File tailing for live log following
//! - Stdin for piped input
//! - Unified InputSource enum for both

use crate::model::error::InputError;
use std::path::PathBuf;

pub mod file;
pub mod stdin;

pub use file::FileTailer;
pub use stdin::StdinSource;

/// Unified input source for JSONL log data.
///
/// Abstracts over file tailing and stdin sources with a common interface.
#[derive(Debug)]
pub enum InputSource {
    /// File tailing source - can read incrementally from a file
    File(FileTailer),
    /// Stdin source - reads from piped stdin
    Stdin(StdinSource),
}

impl InputSource {
    /// Poll for new lines from the input source.
    ///
    /// Non-blocking - returns immediately with available lines.
    ///
    /// # Behavior by variant:
    /// - File: checks for file changes, reads new lines if available
    /// - Stdin: drains all available lines from the channel
    ///
    /// # Errors
    ///
    /// Returns `InputError` for I/O errors or file deletion.
    pub fn poll(&mut self) -> Result<Vec<String>, InputError> {
        todo!("InputSource::poll")
    }

    /// Check if the source is still live (can receive more data).
    ///
    /// # Behavior by variant:
    /// - File: always true (can tail indefinitely)
    /// - Stdin: true until EOF is reached
    pub fn is_live(&self) -> bool {
        todo!("InputSource::is_live")
    }
}

/// Detect and create appropriate input source.
///
/// # Logic:
/// 1. If file path is provided: open with FileTailer
/// 2. Else if stdin is piped: use StdinSource
/// 3. Else: return InputError::NoInput
///
/// # Arguments
///
/// * `file` - Optional file path to tail
///
/// # Errors
///
/// Returns `InputError::NoInput` if no file is provided and stdin is not piped.
/// Returns `InputError::FileNotFound` if file path is provided but doesn't exist.
/// Returns `InputError::Io` for other I/O errors.
pub fn detect_input_source(_file: Option<PathBuf>) -> Result<InputSource, InputError> {
    todo!("detect_input_source")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::{IsTerminal, Write};
    use std::thread;
    use std::time::Duration;

    // ========================================================================
    // InputSource::poll() tests - File variant
    // ========================================================================

    #[test]
    fn poll_returns_empty_vec_when_file_has_no_new_data() {
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join("test_poll_file_no_new_data.jsonl");

        // Create file with initial content
        fs::write(&test_file, "{\"line\": 1}\n").unwrap();

        let tailer = FileTailer::new(&test_file).unwrap();
        let mut source = InputSource::File(tailer);

        // Read initial content
        let initial = source.poll().unwrap();
        assert_eq!(initial.len(), 1);

        // Poll again without changes
        let result = source.poll().unwrap();

        // Cleanup
        let _ = fs::remove_file(&test_file);

        assert_eq!(result.len(), 0, "Should return empty vec when no new data");
    }

    #[test]
    fn poll_returns_new_lines_when_file_is_modified() {
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join("test_poll_file_modified.jsonl");

        // Create file with initial content
        fs::write(&test_file, "{\"line\": 1}\n").unwrap();

        let tailer = FileTailer::new(&test_file).unwrap();
        let mut source = InputSource::File(tailer);

        // Read initial content
        source.poll().unwrap();

        // Append more content
        let mut file = fs::OpenOptions::new()
            .append(true)
            .open(&test_file)
            .unwrap();
        writeln!(file, "{{\"line\": 2}}").unwrap();
        writeln!(file, "{{\"line\": 3}}").unwrap();
        drop(file);

        // Wait for file system event
        thread::sleep(Duration::from_millis(200));

        // Poll should return new lines
        let result = source.poll().unwrap();

        // Cleanup
        let _ = fs::remove_file(&test_file);

        assert_eq!(result.len(), 2);
        assert_eq!(result[0], "{\"line\": 2}");
        assert_eq!(result[1], "{\"line\": 3}");
    }

    #[test]
    fn poll_returns_initial_file_content_on_first_call() {
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join("test_poll_file_initial.jsonl");

        // Create file with content
        fs::write(&test_file, "{\"line\": 1}\n{\"line\": 2}\n").unwrap();

        let tailer = FileTailer::new(&test_file).unwrap();
        let mut source = InputSource::File(tailer);

        let result = source.poll().unwrap();

        // Cleanup
        let _ = fs::remove_file(&test_file);

        assert_eq!(result.len(), 2);
        assert_eq!(result[0], "{\"line\": 1}");
        assert_eq!(result[1], "{\"line\": 2}");
    }

    // ========================================================================
    // InputSource::poll() tests - Stdin variant
    // ========================================================================

    #[test]
    fn poll_drains_all_available_stdin_lines() {
        let data = b"{\"line\": 1}\n{\"line\": 2}\n{\"line\": 3}\n";
        let stdin_source = StdinSource::from_reader(&data[..]);
        let mut source = InputSource::Stdin(stdin_source);

        // Give background thread time to read
        thread::sleep(Duration::from_millis(50));

        let result = source.poll().unwrap();

        // Should drain all available lines in one poll
        assert_eq!(result.len(), 3);
        assert_eq!(result[0], "{\"line\": 1}");
        assert_eq!(result[1], "{\"line\": 2}");
        assert_eq!(result[2], "{\"line\": 3}");
    }

    #[test]
    fn poll_returns_empty_vec_when_stdin_has_no_data_yet() {
        let data = b"";
        let stdin_source = StdinSource::from_reader(&data[..]);
        let mut source = InputSource::Stdin(stdin_source);

        // Poll immediately - no data ready yet
        let result = source.poll().unwrap();

        // Could be empty or could have received EOF already
        assert!(result.is_empty());
    }

    #[test]
    fn poll_returns_partial_stdin_data_when_not_all_available() {
        let data = b"{\"line\": 1}\n";
        let stdin_source = StdinSource::from_reader(&data[..]);
        let mut source = InputSource::Stdin(stdin_source);

        // Give thread time to read first line
        thread::sleep(Duration::from_millis(50));

        let result = source.poll().unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0], "{\"line\": 1}");
    }

    // ========================================================================
    // InputSource::is_live() tests
    // ========================================================================

    #[test]
    fn is_live_returns_true_for_file_source() {
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join("test_is_live_file.jsonl");

        fs::write(&test_file, "{\"line\": 1}\n").unwrap();

        let tailer = FileTailer::new(&test_file).unwrap();
        let source = InputSource::File(tailer);

        let is_live = source.is_live();

        // Cleanup
        let _ = fs::remove_file(&test_file);

        assert!(is_live, "File source should always be live");
    }

    #[test]
    fn is_live_returns_true_for_stdin_before_eof() {
        let data = b"{\"line\": 1}\n";
        let stdin_source = StdinSource::from_reader(&data[..]);
        let source = InputSource::Stdin(stdin_source);

        // Before polling or before EOF is detected
        assert!(source.is_live(), "Stdin source should be live before EOF");
    }

    #[test]
    fn is_live_returns_false_for_stdin_after_eof() {
        let data = b"{\"line\": 1}\n";
        let stdin_source = StdinSource::from_reader(&data[..]);
        let mut source = InputSource::Stdin(stdin_source);

        // Drain all data
        thread::sleep(Duration::from_millis(50));
        source.poll().unwrap();

        // Wait for EOF message
        thread::sleep(Duration::from_millis(50));
        source.poll().unwrap();

        assert!(!source.is_live(), "Stdin source should not be live after EOF");
    }

    // ========================================================================
    // detect_input_source() tests
    // ========================================================================

    #[test]
    fn detect_creates_file_source_when_path_provided() {
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join("test_detect_file.jsonl");

        fs::write(&test_file, "{\"test\": \"data\"}\n").unwrap();

        let result = detect_input_source(Some(test_file.clone()));

        // Cleanup
        let _ = fs::remove_file(&test_file);

        assert!(result.is_ok());
        assert!(matches!(result.unwrap(), InputSource::File(_)));
    }

    #[test]
    fn detect_returns_file_not_found_for_missing_file() {
        let temp_dir = std::env::temp_dir();
        let missing_file = temp_dir.join("nonexistent_detect_test_12345.jsonl");

        let result = detect_input_source(Some(missing_file.clone()));

        assert!(matches!(result, Err(InputError::FileNotFound { .. })));
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
