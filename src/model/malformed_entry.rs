//! Malformed entry types for unparseable JSONL lines.
//!
//! When a JSONL line cannot be parsed into a valid LogEntry,
//! we create a MalformedEntry to display the error inline.

use crate::model::SessionId;

/// A malformed JSONL line that could not be parsed.
///
/// This allows the viewer to display errors inline and continue
/// parsing subsequent lines, satisfying FR-010.
#[derive(Debug, Clone)]
pub struct MalformedEntry {
    line_number: usize,
    raw_line: String,
    error_message: String,
    session_id: Option<SessionId>,
}

impl MalformedEntry {
    /// Create a new malformed entry.
    ///
    /// # Arguments
    ///
    /// * `line_number` - The line number in the JSONL file (1-indexed)
    /// * `raw_line` - The raw line content that failed to parse
    /// * `error_message` - Human-readable error message
    /// * `session_id` - Optional session ID if extractable from partial parse
    pub fn new(
        line_number: usize,
        raw_line: impl Into<String>,
        error_message: impl Into<String>,
        session_id: Option<SessionId>,
    ) -> Self {
        Self {
            line_number,
            raw_line: raw_line.into(),
            error_message: error_message.into(),
            session_id,
        }
    }

    /// Get the line number where the error occurred.
    pub fn line_number(&self) -> usize {
        self.line_number
    }

    /// Get the raw line content.
    pub fn raw_line(&self) -> &str {
        &self.raw_line
    }

    /// Get the error message.
    pub fn error_message(&self) -> &str {
        &self.error_message
    }

    /// Get the session ID if available.
    pub fn session_id(&self) -> Option<&SessionId> {
        self.session_id.as_ref()
    }
}
