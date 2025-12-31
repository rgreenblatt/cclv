//! File-based log source for read-once loading.
//!
//! Provides FileSource for synchronous, read-once loading of JSONL files.
//! No file watching - users leverage `tail -f file | cclv` pattern instead.

use crate::model::error::InputError;
use crate::model::LogEntry;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

// ===== FileSource =====

/// File source for loading complete JSONL log files.
///
/// Pure read-once function - synchronously reads entire file and returns parsed entries.
/// No async, no channels, no polling, no callbacks.
pub struct FileSource;

impl FileSource {
    /// Read entire file and parse into LogEntry vector.
    ///
    /// Synchronously reads the file line by line, parsing each JSONL entry.
    /// Malformed lines are silently skipped (FR-010).
    ///
    /// # Errors
    ///
    /// Returns `InputError::FileNotFound` if file does not exist.
    /// Returns `InputError::Io` for I/O errors during reading.
    pub fn read(path: impl AsRef<Path>) -> Result<Vec<LogEntry>, InputError> {
        let path = path.as_ref();

        // Verify file exists
        if !path.exists() {
            return Err(InputError::FileNotFound {
                path: path.to_path_buf(),
            });
        }

        // Open file for reading
        let file = File::open(path)?;
        let reader = BufReader::new(file);

        // Parse entries in single pass
        let mut entries = Vec::new();

        for line in reader.lines() {
            let line = line?;

            // Skip empty lines
            if line.trim().is_empty() {
                continue;
            }

            // Parse entry and add to collection
            // Malformed lines are silently skipped (FR-010)
            if let Ok(entry) = LogEntry::parse(&line) {
                entries.push(entry);
            }
        }

        Ok(entries)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    // ===== FileSource::read() Tests =====
    // Tests for read-once, synchronous file loading

    #[test]
    fn read_fails_for_missing_file() {
        let temp_dir = std::env::temp_dir();
        let missing_file = temp_dir.join("nonexistent_file_source_12345.jsonl");

        let result = FileSource::read(&missing_file);

        assert!(
            matches!(result, Err(InputError::FileNotFound { .. })),
            "Should return FileNotFound for missing file"
        );
    }

    #[test]
    fn read_returns_empty_vec_for_empty_file() {
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join("test_file_source_empty.jsonl");

        fs::write(&test_file, "").unwrap();

        let result = FileSource::read(&test_file);

        // Cleanup
        let _ = fs::remove_file(&test_file);

        assert!(result.is_ok(), "Should succeed for empty file");
        let entries = result.unwrap();
        assert_eq!(entries.len(), 0, "Should return empty vec for empty file");
    }

    #[test]
    fn read_parses_valid_jsonl_entries() {
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join("test_file_source_valid_entries.jsonl");

        // Create file with valid JSONL entries
        let content = r#"{"type":"user","message":{"role":"user","content":"Hello"},"sessionId":"session-1","uuid":"uuid-1","timestamp":"2025-12-25T10:00:00Z"}
{"type":"assistant","message":{"role":"assistant","content":"Hi"},"sessionId":"session-1","uuid":"uuid-2","timestamp":"2025-12-25T10:00:01Z"}
"#;
        fs::write(&test_file, content).unwrap();

        let result = FileSource::read(&test_file);

        // Cleanup
        let _ = fs::remove_file(&test_file);

        assert!(result.is_ok(), "Should parse valid entries");
        let entries = result.unwrap();
        assert_eq!(entries.len(), 2, "Should have 2 parsed entries");
    }

    #[test]
    fn read_skips_malformed_lines_per_fr_010() {
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join("test_file_source_malformed.jsonl");

        // Mix of valid and malformed lines
        let content = r#"{"type":"user","message":{"role":"user","content":"First"},"sessionId":"s1","uuid":"u1","timestamp":"2025-12-25T10:00:00Z"}
{"invalid json this should be skipped
{"type":"assistant","message":{"role":"assistant","content":"Second"},"sessionId":"s1","uuid":"u2","timestamp":"2025-12-25T10:00:01Z"}
malformed line without braces
{"type":"user","message":{"role":"user","content":"Third"},"sessionId":"s1","uuid":"u3","timestamp":"2025-12-25T10:00:02Z"}
"#;
        fs::write(&test_file, content).unwrap();

        let result = FileSource::read(&test_file);

        // Cleanup
        let _ = fs::remove_file(&test_file);

        assert!(result.is_ok(), "Should succeed despite malformed lines");
        let entries = result.unwrap();
        assert_eq!(
            entries.len(),
            3,
            "Should parse 3 valid entries, skipping 2 malformed"
        );
    }

    #[test]
    fn read_skips_empty_lines() {
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join("test_file_source_empty_lines.jsonl");

        // Valid entries with empty lines between
        let content = r#"{"type":"user","message":{"role":"user","content":"First"},"sessionId":"s1","uuid":"u1","timestamp":"2025-12-25T10:00:00Z"}

{"type":"user","message":{"role":"user","content":"Second"},"sessionId":"s1","uuid":"u2","timestamp":"2025-12-25T10:00:01Z"}

"#;
        fs::write(&test_file, content).unwrap();

        let result = FileSource::read(&test_file);

        // Cleanup
        let _ = fs::remove_file(&test_file);

        assert!(result.is_ok(), "Should skip empty lines");
        let entries = result.unwrap();
        assert_eq!(entries.len(), 2, "Should parse 2 entries, skipping blanks");
    }

    #[test]
    fn read_returns_entries_in_file_order() {
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join("test_file_source_ordering.jsonl");

        // Create file with entries in specific order
        let content = r#"{"type":"user","message":{"role":"user","content":"First"},"sessionId":"s1","uuid":"uuid-1","timestamp":"2025-12-25T10:00:00Z"}
{"type":"assistant","message":{"role":"assistant","content":"Second"},"sessionId":"s1","uuid":"uuid-2","timestamp":"2025-12-25T10:00:01Z"}
{"type":"user","message":{"role":"user","content":"Third"},"sessionId":"s1","uuid":"uuid-3","timestamp":"2025-12-25T10:00:02Z"}
"#;
        fs::write(&test_file, content).unwrap();

        let result = FileSource::read(&test_file);

        // Cleanup
        let _ = fs::remove_file(&test_file);

        assert!(result.is_ok(), "Should preserve file order");
        let entries = result.unwrap();
        assert_eq!(entries.len(), 3);

        // Verify UUIDs are in order
        assert_eq!(entries[0].uuid().as_str(), "uuid-1");
        assert_eq!(entries[1].uuid().as_str(), "uuid-2");
        assert_eq!(entries[2].uuid().as_str(), "uuid-3");
    }
}
