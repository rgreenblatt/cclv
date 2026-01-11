//! File-based log source for read-once loading.
//!
//! Provides FileSource for synchronous, read-once loading of JSONL files.
//! No file watching - users leverage `tail -f file | cclv` pattern instead.

use crate::model::LogEntry;
use crate::model::error::InputError;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

// ===== FileSource =====

/// File source for loading complete JSONL log files.
///
/// Read-once: loads all entries on construction, drains on first poll, empty after.
/// No async, no channels, no file watching, no callbacks.
#[derive(Debug)]
pub struct FileSource {
    /// Entries loaded from file. Some until drained, then None.
    entries: Option<Vec<LogEntry>>,
}

impl FileSource {
    /// Create new FileSource by loading all entries from file.
    ///
    /// Loads entire file on construction. Call drain_entries() to retrieve entries.
    ///
    /// # Errors
    ///
    /// Returns `InputError::FileNotFound` if file does not exist.
    /// Returns `InputError::Io` for I/O errors during reading.
    pub fn new(path: PathBuf) -> Result<Self, InputError> {
        let entries = Self::read_all(&path)?;
        Ok(Self {
            entries: Some(entries),
        })
    }

    /// Drain all entries from this source.
    ///
    /// Returns all entries on first call, empty vec on subsequent calls.
    pub fn drain_entries(&mut self) -> Result<Vec<LogEntry>, InputError> {
        Ok(self.entries.take().unwrap_or_default())
    }

    /// Read entire file and parse into LogEntry vector (private helper).
    ///
    /// Synchronously reads the file line by line, parsing each JSONL entry.
    /// Malformed lines are silently skipped (FR-010).
    ///
    /// # Errors
    ///
    /// Returns `InputError::FileNotFound` if file does not exist.
    /// Returns `InputError::Io` for I/O errors during reading.
    fn read_all(path: impl AsRef<Path>) -> Result<Vec<LogEntry>, InputError> {
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
    // FileSource tests are now in src/source/mod.rs
    // as part of InputSource integration tests
}
