//! Pure core integration functions.
//!
//! This module contains pure functions that integrate the parsing and session
//! management for the main event loop. These functions are testable without
//! needing actual I/O.

use crate::model::{ConversationEntry, LogEntry, ParseError};
use crate::parser;

/// Convert parsed LogEntry vector into ConversationEntry vector.
///
/// This is a pure function that wraps LogEntry in ConversationEntry::Valid.
/// Used when entries are already parsed (e.g., from FileSource or StdinSource).
///
/// # Arguments
///
/// * `entries` - Parsed LogEntry vector
///
/// # Returns
///
/// Vector of ConversationEntry::Valid
pub fn process_entries(entries: Vec<LogEntry>) -> Vec<ConversationEntry> {
    entries
        .into_iter()
        .map(|entry| ConversationEntry::Valid(Box::new(entry)))
        .collect()
}

/// Process new JSONL lines into conversation entries (valid or malformed).
///
/// This is a pure function that:
/// - Attempts to parse each line into a LogEntry
/// - On success, returns ConversationEntry::Valid
/// - On failure, returns ConversationEntry::Malformed
/// - ALL lines produce an entry (graceful degradation)
///
/// # Arguments
///
/// * `lines` - Raw JSONL lines to process
/// * `starting_line_number` - Line number of the first line (for error reporting)
///
/// # Returns
///
/// Vector of ConversationEntry (both valid and malformed entries)
pub fn process_lines(lines: Vec<String>, starting_line_number: usize) -> Vec<ConversationEntry> {
    lines
        .into_iter()
        .enumerate()
        .map(|(index, line)| {
            let line_number = starting_line_number + index;
            parser::parse_entry_graceful(&line, line_number).into()
        })
        .collect()
}

/// Legacy function for backwards compatibility.
/// Returns (valid entries, errors) tuple.
/// New code should use process_lines() instead.
#[deprecated(note = "Use process_lines() which returns ConversationEntry instead")]
pub fn process_lines_legacy(
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
    use crate::state::AppState;

    // ===== process_lines Tests =====

    #[test]
    fn process_lines_adds_valid_entry_to_session() {
        let lines = vec![
            r#"{"type":"user","message":{"role":"user","content":"Hello"},"sessionId":"session-1","uuid":"uuid-1","timestamp":"2025-12-25T10:00:00Z"}"#.to_string(),
        ];

        let entries = process_lines(lines, 1);

        assert_eq!(entries.len(), 1, "Should have parsed one entry");
        assert!(
            entries[0].is_valid(),
            "Entry should be valid for well-formed JSON"
        );

        let mut state = AppState::new();
        state.add_entries(entries);
        assert_eq!(
            state.session_view().main().len(),
            1,
            "Should have added entry to main conversation"
        );
    }

    #[test]
    fn process_lines_adds_multiple_valid_entries() {
        let lines = vec![
            r#"{"type":"user","message":{"role":"user","content":"First"},"sessionId":"session-1","uuid":"uuid-1","timestamp":"2025-12-25T10:00:00Z"}"#.to_string(),
            r#"{"type":"assistant","message":{"role":"assistant","content":"Second"},"sessionId":"session-1","uuid":"uuid-2","timestamp":"2025-12-25T10:00:01Z"}"#.to_string(),
        ];

        let entries = process_lines(lines, 1);

        assert_eq!(entries.len(), 2);
        assert!(entries[0].is_valid());
        assert!(entries[1].is_valid());

        let mut state = AppState::new();
        state.add_entries(entries);
        assert_eq!(state.session_view().main().len(), 2);
    }

    #[test]
    fn process_lines_creates_malformed_entry_for_bad_json() {
        let lines = vec![r#"{"type":"user","malformed"#.to_string()];

        let entries = process_lines(lines, 42);

        assert_eq!(entries.len(), 1, "Should have one entry (malformed)");
        assert!(
            entries[0].is_malformed(),
            "Entry should be malformed for bad JSON"
        );

        // Verify the malformed entry has correct line number
        if let Some(malformed) = entries[0].as_malformed() {
            assert_eq!(
                malformed.line_number(),
                42,
                "Should preserve line number in malformed entry"
            );
        } else {
            panic!("Expected malformed entry");
        }
    }

    #[test]
    fn process_lines_continues_after_parse_error() {
        let lines = vec![
            r#"{"type":"user","message":{"role":"user","content":"Good"},"sessionId":"session-1","uuid":"uuid-1","timestamp":"2025-12-25T10:00:00Z"}"#.to_string(),
            r#"{"malformed"#.to_string(),
            r#"{"type":"user","message":{"role":"user","content":"Also good"},"sessionId":"session-1","uuid":"uuid-2","timestamp":"2025-12-25T10:00:01Z"}"#.to_string(),
        ];

        let entries = process_lines(lines, 1);

        assert_eq!(entries.len(), 3, "Should have 3 entries total");
        assert!(entries[0].is_valid(), "First entry should be valid");
        assert!(
            entries[1].is_malformed(),
            "Second entry should be malformed"
        );
        assert!(entries[2].is_valid(), "Third entry should be valid");

        let mut state = AppState::new();
        state.add_entries(entries);
        assert_eq!(
            state.session_view().main().len(),
            3,
            "Should have added all 3 entries (2 valid, 1 malformed)"
        );
    }

    #[test]
    fn process_lines_routes_to_subagent() {
        let lines = vec![
            r#"{"type":"user","message":{"role":"user","content":"Test"},"sessionId":"session-1","uuid":"uuid-1","agentId":"agent-123","timestamp":"2025-12-25T10:00:00Z"}"#.to_string(),
        ];

        let entries = process_lines(lines, 1);

        assert_eq!(entries.len(), 1);
        assert!(entries[0].is_valid());

        let mut state = AppState::new();
        state.add_entries(entries);
        assert_eq!(
            state.session_view().main().len(),
            0,
            "Should not add to main agent"
        );
        assert!(
            state.session_view().has_subagents(),
            "Should create subagent (pending or materialized)"
        );
    }

    #[test]
    fn process_lines_returns_empty_for_empty_input() {
        let lines = vec![];

        let entries = process_lines(lines, 1);

        assert_eq!(entries.len(), 0);
    }

    #[test]
    fn process_lines_uses_line_numbers_correctly() {
        let lines = vec![r#"{"malformed1"#.to_string(), r#"{"malformed2"#.to_string()];

        let entries = process_lines(lines, 100);

        assert_eq!(entries.len(), 2, "Should have 2 malformed entries");
        assert!(entries[0].is_malformed());
        assert!(entries[1].is_malformed());

        // Check line numbers
        if let Some(malformed1) = entries[0].as_malformed() {
            assert_eq!(
                malformed1.line_number(),
                100,
                "First error should be at line 100"
            );
        } else {
            panic!("Expected malformed entry");
        }

        if let Some(malformed2) = entries[1].as_malformed() {
            assert_eq!(
                malformed2.line_number(),
                101,
                "Second error should be at line 101"
            );
        } else {
            panic!("Expected malformed entry");
        }
    }
}
