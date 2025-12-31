//! File-based log source with live tailing support.
//!
//! Provides FileTailer for reading JSONL files with automatic file watching
//! for live updates.

use crate::model::error::InputError;
use notify_debouncer_mini::{new_debouncer, DebouncedEventKind, Debouncer};
use std::fs::File;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::mpsc::Receiver;
use std::time::Duration;

/// File tailer for live JSONL log following.
///
/// Tracks file position and watches for modifications to provide
/// incremental updates.
#[derive(Debug)]
pub struct FileTailer {
    path: PathBuf,
    position: u64,
    file: BufReader<File>,
    _debouncer: Debouncer<notify::RecommendedWatcher>,
    event_rx: Receiver<notify_debouncer_mini::DebounceEventResult>,
}

impl FileTailer {
    /// Create a new FileTailer for the given path.
    ///
    /// Opens the file, sets up file watching with 100ms debouncing,
    /// and positions at the start of the file.
    ///
    /// # Errors
    ///
    /// Returns `InputError::FileNotFound` if the file does not exist.
    /// Returns `InputError::Io` for other I/O errors.
    pub fn new(path: impl AsRef<Path>) -> Result<Self, InputError> {
        let path = path.as_ref();

        // Check if file exists before trying to open
        if !path.exists() {
            return Err(InputError::FileNotFound {
                path: path.to_path_buf(),
            });
        }

        // Open file for reading
        let file = File::open(path)?;
        let reader = BufReader::new(file);

        // Set up file watcher with 100ms debouncing
        let (tx, rx) = std::sync::mpsc::channel();
        let mut debouncer = new_debouncer(Duration::from_millis(100), tx)
            .map_err(std::io::Error::other)?;

        // Watch the file's parent directory (watching individual files may not work on all platforms)
        debouncer
            .watcher()
            .watch(path, notify::RecursiveMode::NonRecursive)
            .map_err(std::io::Error::other)?;

        Ok(Self {
            path: path.to_path_buf(),
            position: 0,
            file: reader,
            _debouncer: debouncer,
            event_rx: rx,
        })
    }

    /// Read new lines that have been appended since last read.
    ///
    /// Seeks to the last known position, reads all available content,
    /// and returns complete lines.
    ///
    /// # Errors
    ///
    /// Returns `InputError::FileDeleted` if the file has been deleted.
    /// Returns `InputError::Io` for other I/O errors.
    pub fn read_new_lines(&mut self) -> Result<Vec<String>, InputError> {
        // Seek to last known position
        self.file.seek(SeekFrom::Start(self.position))?;

        let mut lines = Vec::new();
        let mut buffer = String::new();

        // Read all complete lines
        loop {
            buffer.clear();
            let bytes_read = self.file.read_line(&mut buffer)?;

            if bytes_read == 0 {
                // EOF reached
                break;
            }

            // Only include complete lines (ending with newline)
            if buffer.ends_with('\n') {
                // Remove the trailing newline before storing
                let line = buffer.trim_end_matches('\n').to_string();
                lines.push(line);
                self.position += bytes_read as u64;
            } else {
                // Partial line - don't include it, don't update position
                break;
            }
        }

        Ok(lines)
    }

    /// Poll for file change events.
    ///
    /// Returns true if the file has been modified since last poll.
    /// Non-blocking - returns immediately.
    ///
    /// # Errors
    ///
    /// Returns `InputError::FileDeleted` if file deletion is detected.
    pub fn poll_changes(&mut self) -> Result<bool, InputError> {
        let mut has_changes = false;

        // Drain all pending events
        while let Ok(result) = self.event_rx.try_recv() {
            match result {
                Ok(events) => {
                    for event in events {
                        match event.kind {
                            DebouncedEventKind::Any => {
                                // Check if file still exists
                                if !self.path.exists() {
                                    return Err(InputError::FileDeleted);
                                }
                                has_changes = true;
                            }
                            _ => {
                                // Other event types - treat as changes
                                has_changes = true;
                            }
                        }
                    }
                }
                Err(error) => {
                    // Check for file deletion in error path
                    if let notify::ErrorKind::PathNotFound = error.kind {
                        return Err(InputError::FileDeleted);
                    }
                    // Other errors are logged but don't stop polling
                }
            }
        }

        Ok(has_changes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn new_opens_existing_file() {
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join("test_new_opens_existing_file.jsonl");

        // Create test file
        fs::write(&test_file, "{\"test\": \"data\"}\n").unwrap();

        let result = FileTailer::new(&test_file);

        // Cleanup
        let _ = fs::remove_file(&test_file);

        assert!(result.is_ok());
    }

    #[test]
    fn new_returns_file_not_found_for_missing_file() {
        let temp_dir = std::env::temp_dir();
        let missing_file = temp_dir.join("nonexistent_file_12345.jsonl");

        let result = FileTailer::new(&missing_file);

        assert!(matches!(result, Err(InputError::FileNotFound { .. })));
    }

    #[test]
    fn read_new_lines_returns_initial_content() {
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join("test_read_new_lines_initial.jsonl");

        // Create file with content
        fs::write(&test_file, "{\"line\": 1}\n{\"line\": 2}\n").unwrap();

        let mut tailer = FileTailer::new(&test_file).unwrap();
        let lines = tailer.read_new_lines().unwrap();

        // Cleanup
        let _ = fs::remove_file(&test_file);

        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0], "{\"line\": 1}");
        assert_eq!(lines[1], "{\"line\": 2}");
    }

    #[test]
    fn read_new_lines_returns_only_new_content_on_second_call() {
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join("test_read_new_lines_incremental.jsonl");

        // Create file with initial content
        fs::write(&test_file, "{\"line\": 1}\n").unwrap();

        let mut tailer = FileTailer::new(&test_file).unwrap();

        // Read initial content
        let lines1 = tailer.read_new_lines().unwrap();
        assert_eq!(lines1.len(), 1);

        // Append more content
        let mut file = fs::OpenOptions::new()
            .append(true)
            .open(&test_file)
            .unwrap();
        writeln!(file, "{{\"line\": 2}}").unwrap();
        drop(file);

        // Read only new content
        let lines2 = tailer.read_new_lines().unwrap();

        // Cleanup
        let _ = fs::remove_file(&test_file);

        assert_eq!(lines2.len(), 1);
        assert_eq!(lines2[0], "{\"line\": 2}");
    }

    #[test]
    fn read_new_lines_returns_empty_when_no_new_content() {
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join("test_read_new_lines_empty.jsonl");

        fs::write(&test_file, "{\"line\": 1}\n").unwrap();

        let mut tailer = FileTailer::new(&test_file).unwrap();

        // Read initial content
        tailer.read_new_lines().unwrap();

        // Read again without new content
        let lines = tailer.read_new_lines().unwrap();

        // Cleanup
        let _ = fs::remove_file(&test_file);

        assert_eq!(lines.len(), 0);
    }

    #[test]
    fn read_new_lines_handles_partial_lines() {
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join("test_read_new_lines_partial.jsonl");

        // Create file with complete line and partial line
        let mut file = fs::File::create(&test_file).unwrap();
        write!(file, "{{\"line\": 1}}\n{{\"line\": 2").unwrap();
        drop(file);

        let mut tailer = FileTailer::new(&test_file).unwrap();
        let lines = tailer.read_new_lines().unwrap();

        // Cleanup
        let _ = fs::remove_file(&test_file);

        // Should only return the complete line
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0], "{\"line\": 1}");
    }

    #[test]
    fn poll_changes_detects_file_modification() {
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join("test_poll_changes.jsonl");

        fs::write(&test_file, "{\"line\": 1}\n").unwrap();

        let mut tailer = FileTailer::new(&test_file).unwrap();

        // Initial poll should be false (no changes yet)
        thread::sleep(Duration::from_millis(50));
        let changed1 = tailer.poll_changes().unwrap();

        // Modify file
        let mut file = fs::OpenOptions::new()
            .append(true)
            .open(&test_file)
            .unwrap();
        writeln!(file, "{{\"line\": 2}}").unwrap();
        drop(file);

        // Wait for debouncer (100ms) + safety margin
        thread::sleep(Duration::from_millis(200));

        // Poll should detect change
        let changed2 = tailer.poll_changes().unwrap();

        // Cleanup
        let _ = fs::remove_file(&test_file);

        assert!(!changed1, "Initial poll should detect no changes");
        assert!(changed2, "Poll after modification should detect changes");
    }

    #[test]
    fn poll_changes_returns_false_when_no_changes() {
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join("test_poll_changes_no_change.jsonl");

        fs::write(&test_file, "{\"line\": 1}\n").unwrap();

        let mut tailer = FileTailer::new(&test_file).unwrap();

        thread::sleep(Duration::from_millis(50));

        let changed = tailer.poll_changes().unwrap();

        // Cleanup
        let _ = fs::remove_file(&test_file);

        assert!(!changed);
    }

    #[test]
    fn file_deletion_detected() {
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join("test_file_deletion.jsonl");

        fs::write(&test_file, "{\"line\": 1}\n").unwrap();

        let mut tailer = FileTailer::new(&test_file).unwrap();

        // Read initial content
        tailer.read_new_lines().unwrap();

        // Delete the file
        fs::remove_file(&test_file).unwrap();

        // Wait for file system event
        thread::sleep(Duration::from_millis(200));

        // Poll should detect deletion
        let result = tailer.poll_changes();

        assert!(
            matches!(result, Err(InputError::FileDeleted)),
            "Expected FileDeleted error, got: {:?}",
            result
        );
    }
}
