//! Pure core integration functions.
//!
//! This module contains pure functions that integrate the parsing and session
//! management for the main event loop. These functions are testable without
//! needing actual I/O.

use crate::model::ParseError;
use crate::parser;

/// Process new JSONL lines into log entries.
///
/// This is a pure function that:
/// - Parses each line into a LogEntry
/// - Collects successful parses and errors separately
///
/// # Arguments
///
/// * `lines` - Raw JSONL lines to process
/// * `starting_line_number` - Line number of the first line (for error reporting)
///
/// # Returns
///
/// Tuple of (successfully parsed entries, parse errors)
pub fn process_lines(
    lines: Vec<String>,
    starting_line_number: usize,
) -> (Vec<crate::model::LogEntry>, Vec<ParseError>) {
    let mut entries = Vec::new();
    let mut errors = Vec::new();

    for (index, line) in lines.into_iter().enumerate() {
        let line_number = starting_line_number + index;
        match parser::parse_entry(&line, line_number) {
            Ok(entry) => entries.push(entry),
            Err(err) => errors.push(err),
        }
    }

    (entries, errors)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Session, SessionId};

    // ===== Test Helpers =====

    fn make_session_id(s: &str) -> SessionId {
        SessionId::new(s).expect("valid session id")
    }

    // ===== process_lines Tests =====

    #[test]
    fn process_lines_adds_valid_entry_to_session() {
        let mut session = Session::new(make_session_id("session-1"));
        let lines = vec![
            r#"{"type":"user","message":{"role":"user","content":"Hello"},"sessionId":"session-1","uuid":"uuid-1","timestamp":"2025-12-25T10:00:00Z"}"#.to_string(),
        ];

        let (entries, errors) = process_lines(lines, 1);

        assert_eq!(errors.len(), 0, "Should have no parse errors");
        assert_eq!(entries.len(), 1, "Should have parsed one entry");

        for entry in entries {
            session.add_entry(entry);
        }
        assert_eq!(
            session.main_agent().len(),
            1,
            "Should have added entry to session"
        );
    }

    #[test]
    fn process_lines_adds_multiple_valid_entries() {
        let mut session = Session::new(make_session_id("session-1"));
        let lines = vec![
            r#"{"type":"user","message":{"role":"user","content":"First"},"sessionId":"session-1","uuid":"uuid-1","timestamp":"2025-12-25T10:00:00Z"}"#.to_string(),
            r#"{"type":"assistant","message":{"role":"assistant","content":"Second"},"sessionId":"session-1","uuid":"uuid-2","timestamp":"2025-12-25T10:00:01Z"}"#.to_string(),
        ];

        let (entries, errors) = process_lines(lines, 1);

        assert_eq!(errors.len(), 0);
        assert_eq!(entries.len(), 2);

        for entry in entries {
            session.add_entry(entry);
        }
        assert_eq!(session.main_agent().len(), 2);
    }

    #[test]
    fn process_lines_returns_error_for_malformed_json() {
        let lines = vec![
            r#"{"type":"user","malformed"#.to_string(),
        ];

        let (entries, errors) = process_lines(lines, 42);

        assert_eq!(entries.len(), 0, "Should have no valid entries");
        assert_eq!(errors.len(), 1, "Should have one parse error");
        match &errors[0] {
            ParseError::InvalidJson { line, .. } => {
                assert_eq!(*line, 42, "Should preserve line number in error");
            }
            _ => panic!("Expected InvalidJson error"),
        }
    }

    #[test]
    fn process_lines_continues_after_parse_error() {
        let mut session = Session::new(make_session_id("session-1"));
        let lines = vec![
            r#"{"type":"user","message":{"role":"user","content":"Good"},"sessionId":"session-1","uuid":"uuid-1","timestamp":"2025-12-25T10:00:00Z"}"#.to_string(),
            r#"{"malformed"#.to_string(),
            r#"{"type":"user","message":{"role":"user","content":"Also good"},"sessionId":"session-1","uuid":"uuid-2","timestamp":"2025-12-25T10:00:01Z"}"#.to_string(),
        ];

        let (entries, errors) = process_lines(lines, 1);

        assert_eq!(errors.len(), 1, "Should have one parse error");
        assert_eq!(entries.len(), 2, "Should have 2 valid entries");

        for entry in entries {
            session.add_entry(entry);
        }
        assert_eq!(
            session.main_agent().len(),
            2,
            "Should have added 2 valid entries despite error"
        );
    }

    #[test]
    fn process_lines_routes_to_subagent() {
        let mut session = Session::new(make_session_id("session-1"));
        let lines = vec![
            r#"{"type":"user","message":{"role":"user","content":"Test"},"sessionId":"session-1","uuid":"uuid-1","agentId":"agent-123","timestamp":"2025-12-25T10:00:00Z"}"#.to_string(),
        ];

        let (entries, errors) = process_lines(lines, 1);

        assert_eq!(errors.len(), 0);
        assert_eq!(entries.len(), 1);

        for entry in entries {
            session.add_entry(entry);
        }
        assert_eq!(session.main_agent().len(), 0, "Should not add to main agent");
        assert_eq!(session.subagents().len(), 1, "Should create subagent");
    }

    #[test]
    fn process_lines_returns_empty_errors_for_empty_input() {
        let lines = vec![];

        let (entries, errors) = process_lines(lines, 1);

        assert_eq!(entries.len(), 0);
        assert_eq!(errors.len(), 0);
    }

    #[test]
    fn process_lines_uses_line_numbers_correctly() {
        let lines = vec![
            r#"{"malformed1"#.to_string(),
            r#"{"malformed2"#.to_string(),
        ];

        let (entries, errors) = process_lines(lines, 100);

        assert_eq!(entries.len(), 0);
        assert_eq!(errors.len(), 2);
        match &errors[0] {
            ParseError::InvalidJson { line, .. } => {
                assert_eq!(*line, 100, "First error should be at line 100");
            }
            _ => panic!("Expected InvalidJson error"),
        }
        match &errors[1] {
            ParseError::InvalidJson { line, .. } => {
                assert_eq!(*line, 101, "Second error should be at line 101");
            }
            _ => panic!("Expected InvalidJson error"),
        }
    }
}
