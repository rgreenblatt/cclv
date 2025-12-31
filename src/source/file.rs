//! File-based log source for read-once loading.
//!
//! Provides FileSource for loading complete JSONL files into memory.
//! File watching removed - users leverage `tail -f file | cclv` pattern instead.

use crate::model::error::InputError;
use crate::model::{LogEntry, Session, SessionId};
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};

// ===== FileSource =====

/// File source for loading complete JSONL log files.
///
/// Reads entire file into memory and parses into Session.
/// Tracks line count for progress indication.
#[derive(Debug)]
pub struct FileSource {
    path: PathBuf,
    line_count: usize,
}

impl FileSource {
    /// Create a new FileSource for the given path.
    ///
    /// Does not read the file yet - use `initial_load()` to load.
    ///
    /// # Errors
    ///
    /// Returns `InputError::FileNotFound` if the file does not exist.
    pub fn new(path: impl AsRef<Path>) -> Result<Self, InputError> {
        let path = path.as_ref();

        // Verify file exists
        if !path.exists() {
            return Err(InputError::FileNotFound {
                path: path.to_path_buf(),
            });
        }

        Ok(Self {
            path: path.to_path_buf(),
            line_count: 0,
        })
    }

    /// Load complete file into Session.
    ///
    /// Reads entire file line by line, parsing each JSONL entry.
    /// Malformed lines are logged but do not stop parsing (FR-010).
    /// All valid entries are added to the returned Session.
    ///
    /// # Errors
    ///
    /// Returns `InputError::FileNotFound` if file does not exist.
    /// Returns `InputError::Io` for I/O errors during reading.
    pub fn initial_load(&mut self) -> Result<Session, InputError> {
        use std::io::BufRead;

        // Open file for reading (single pass)
        let file = File::open(&self.path)?;
        let reader = BufReader::new(file);

        // Parse entries in single pass
        let mut session: Option<Session> = None;
        let mut total_lines = 0;

        for line in reader.lines() {
            let line = line?;
            total_lines += 1;

            if line.trim().is_empty() {
                continue;
            }

            // Parse entry and add to session
            // Malformed lines are silently skipped (FR-010)
            match LogEntry::parse(&line) {
                Ok(entry) => {
                    // Initialize session from first valid entry
                    let sess =
                        session.get_or_insert_with(|| Session::new(entry.session_id().clone()));
                    sess.add_entry(entry);
                }
                Err(_parse_error) => {
                    // FR-010: Malformed lines do not stop parsing
                    // TODO: Add tracing for debugging malformed lines
                    continue;
                }
            }
        }

        // Update line count
        self.line_count = total_lines;

        // If no valid entries were found, create default session
        let session = session.unwrap_or_else(|| Session::new(SessionId::unknown()));

        Ok(session)
    }

    /// Number of lines processed during last `initial_load()`.
    ///
    /// Returns 0 if `initial_load()` has not been called yet.
    pub fn line_count(&self) -> usize {
        self.line_count
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    // ===== FileSource Tests =====

    #[test]
    fn file_source_new_succeeds_for_existing_file() {
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join("test_file_source_new_exists.jsonl");

        // Create test file
        fs::write(&test_file, "").unwrap();

        let result = FileSource::new(&test_file);

        // Cleanup
        let _ = fs::remove_file(&test_file);

        assert!(result.is_ok(), "Should create FileSource for existing file");
    }

    #[test]
    fn file_source_new_fails_for_missing_file() {
        let temp_dir = std::env::temp_dir();
        let missing_file = temp_dir.join("nonexistent_file_source_12345.jsonl");

        let result = FileSource::new(&missing_file);

        assert!(
            matches!(result, Err(InputError::FileNotFound { .. })),
            "Should return FileNotFound for missing file"
        );
    }

    #[test]
    fn file_source_initial_load_parses_valid_entries() {
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join("test_file_source_valid_entries.jsonl");

        // Create file with valid JSONL entries
        let content = r#"{"type":"user","message":{"role":"user","content":"Hello"},"sessionId":"session-1","uuid":"uuid-1","timestamp":"2025-12-25T10:00:00Z"}
{"type":"assistant","message":{"role":"assistant","content":"Hi"},"sessionId":"session-1","uuid":"uuid-2","timestamp":"2025-12-25T10:00:01Z"}
"#;
        fs::write(&test_file, content).unwrap();

        let mut source = FileSource::new(&test_file).unwrap();
        let result = source.initial_load();

        // Cleanup
        let _ = fs::remove_file(&test_file);

        assert!(result.is_ok(), "Should parse valid entries");
        let session = result.unwrap();
        assert_eq!(
            session.main_agent().len(),
            2,
            "Should have 2 entries in main agent"
        );
    }

    #[test]
    fn file_source_initial_load_skips_malformed_lines() {
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

        let mut source = FileSource::new(&test_file).unwrap();
        let result = source.initial_load();

        // Cleanup
        let _ = fs::remove_file(&test_file);

        assert!(result.is_ok(), "Should succeed despite malformed lines");
        let session = result.unwrap();
        assert_eq!(
            session.main_agent().len(),
            3,
            "Should parse 3 valid entries, skipping 2 malformed"
        );
    }

    #[test]
    fn file_source_initial_load_handles_empty_file() {
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join("test_file_source_empty.jsonl");

        // Empty file
        fs::write(&test_file, "").unwrap();

        let mut source = FileSource::new(&test_file).unwrap();
        let result = source.initial_load();

        // Cleanup
        let _ = fs::remove_file(&test_file);

        assert!(result.is_ok(), "Should handle empty file");
        let session = result.unwrap();
        assert_eq!(session.main_agent().len(), 0, "Should have no entries");
    }

    #[test]
    fn file_source_initial_load_routes_to_subagents() {
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join("test_file_source_subagents.jsonl");

        // Entries with subagent
        let content = r#"{"type":"user","message":{"role":"user","content":"Main"},"sessionId":"s1","uuid":"u1","timestamp":"2025-12-25T10:00:00Z"}
{"type":"user","message":{"role":"user","content":"Sub1"},"sessionId":"s1","uuid":"u2","agentId":"agent-abc","timestamp":"2025-12-25T10:00:01Z"}
{"type":"assistant","message":{"role":"assistant","content":"Sub2"},"sessionId":"s1","uuid":"u3","agentId":"agent-abc","timestamp":"2025-12-25T10:00:02Z"}
{"type":"assistant","message":{"role":"assistant","content":"Main2"},"sessionId":"s1","uuid":"u4","timestamp":"2025-12-25T10:00:03Z"}
"#;
        fs::write(&test_file, content).unwrap();

        let mut source = FileSource::new(&test_file).unwrap();
        let result = source.initial_load();

        // Cleanup
        let _ = fs::remove_file(&test_file);

        assert!(result.is_ok(), "Should parse entries with subagents");
        let session = result.unwrap();
        assert_eq!(
            session.main_agent().len(),
            2,
            "Should have 2 main agent entries"
        );
        assert_eq!(session.subagents().len(), 1, "Should have 1 subagent");

        let agent_id = crate::model::AgentId::new("agent-abc").unwrap();
        assert!(
            session.subagents().contains_key(&agent_id),
            "Should have agent-abc subagent"
        );
        assert_eq!(
            session.subagents()[&agent_id].len(),
            2,
            "Subagent should have 2 entries"
        );
    }

    #[test]
    fn file_source_line_count_returns_zero_initially() {
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join("test_file_source_line_count_initial.jsonl");

        fs::write(&test_file, "").unwrap();

        let source = FileSource::new(&test_file).unwrap();

        // Cleanup
        let _ = fs::remove_file(&test_file);

        assert_eq!(
            source.line_count(),
            0,
            "Should return 0 before initial_load"
        );
    }

    #[test]
    fn file_source_line_count_tracks_all_lines() {
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join("test_file_source_line_count_tracks.jsonl");

        // 3 valid + 2 malformed = 5 total lines
        let content = r#"{"type":"user","message":{"role":"user","content":"1"},"sessionId":"s1","uuid":"u1","timestamp":"2025-12-25T10:00:00Z"}
invalid line
{"type":"user","message":{"role":"user","content":"2"},"sessionId":"s1","uuid":"u2","timestamp":"2025-12-25T10:00:01Z"}
another malformed line
{"type":"user","message":{"role":"user","content":"3"},"sessionId":"s1","uuid":"u3","timestamp":"2025-12-25T10:00:02Z"}
"#;
        fs::write(&test_file, content).unwrap();

        let mut source = FileSource::new(&test_file).unwrap();
        let _ = source.initial_load().unwrap();

        // Cleanup
        let _ = fs::remove_file(&test_file);

        assert_eq!(
            source.line_count(),
            5,
            "Should track all lines including malformed"
        );
    }

    #[test]
    fn file_source_line_count_handles_empty_file() {
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join("test_file_source_line_count_empty.jsonl");

        fs::write(&test_file, "").unwrap();

        let mut source = FileSource::new(&test_file).unwrap();
        let _ = source.initial_load().unwrap();

        // Cleanup
        let _ = fs::remove_file(&test_file);

        assert_eq!(source.line_count(), 0, "Should return 0 for empty file");
    }
}
