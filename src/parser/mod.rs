//! JSONL parser for Claude Code log entries.
//!
//! This module provides pure parsing functions for converting JSONL lines
//! into validated LogEntry structs.

use crate::model::{EntryType, LogEntry, ParseError};

/// Parse a single JSONL line into a LogEntry.
///
/// This is the main entry point for parsing. It:
/// - Deserializes the JSON
/// - Validates required fields
/// - Constructs validated newtypes
/// - Returns a fully validated LogEntry
///
/// # Errors
///
/// Returns `ParseError` if:
/// - JSON is malformed
/// - Required fields are missing
/// - Timestamps are invalid
/// - UUIDs or IDs are empty
pub fn parse_entry(raw: &str, line_number: usize) -> Result<LogEntry, ParseError> {
    todo!("parse_entry")
}

/// Parse the "type" field into EntryType enum.
fn parse_entry_type(type_str: &str) -> Option<EntryType> {
    todo!("parse_entry_type")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{EntryType, MessageContent, Role};

    // ===== Successful Parsing Tests =====

    #[test]
    fn parse_entry_minimal_user_message() {
        let raw = r#"{"type":"user","message":{"role":"user","content":"Hello"},"sessionId":"session-123","uuid":"uuid-001","timestamp":"2025-12-25T10:00:00Z"}"#;
        let result = parse_entry(raw, 1);

        assert!(result.is_ok(), "Should parse valid user message");
        let entry = result.unwrap();
        assert_eq!(entry.uuid().as_str(), "uuid-001");
        assert_eq!(entry.session_id().as_str(), "session-123");
        assert_eq!(entry.entry_type(), EntryType::User);
        assert_eq!(entry.message().role(), Role::User);
        assert!(entry.agent_id().is_none(), "Main agent has no agent_id");
    }

    #[test]
    fn parse_entry_assistant_with_usage() {
        let raw = r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"Hi there"}],"model":"claude-opus-4-5-20251101","usage":{"input_tokens":100,"output_tokens":50,"cache_creation_input_tokens":20,"cache_read_input_tokens":10}},"sessionId":"session-123","uuid":"uuid-002","timestamp":"2025-12-25T10:00:01Z"}"#;
        let result = parse_entry(raw, 1);

        assert!(result.is_ok(), "Should parse assistant message with usage");
        let entry = result.unwrap();
        assert_eq!(entry.entry_type(), EntryType::Assistant);
        assert_eq!(entry.message().role(), Role::Assistant);

        let usage = entry.message().usage().expect("Should have usage");
        assert_eq!(usage.input_tokens, 100);
        assert_eq!(usage.output_tokens, 50);
        assert_eq!(usage.cache_creation_input_tokens, 20);
        assert_eq!(usage.cache_read_input_tokens, 10);

        let model = entry.message().model().expect("Should have model");
        assert_eq!(model.id(), "claude-opus-4-5-20251101");
    }

    #[test]
    fn parse_entry_text_content_as_string() {
        let raw = r#"{"type":"user","message":{"role":"user","content":"Simple text"},"sessionId":"s1","uuid":"u1","timestamp":"2025-12-25T10:00:00Z"}"#;
        let result = parse_entry(raw, 1);

        assert!(result.is_ok());
        let entry = result.unwrap();
        match entry.message().content() {
            MessageContent::Text(text) => assert_eq!(text, "Simple text"),
            _ => panic!("Expected Text content"),
        }
    }

    #[test]
    fn parse_entry_text_content_as_blocks() {
        let raw = r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"Block text"}]},"sessionId":"s1","uuid":"u1","timestamp":"2025-12-25T10:00:00Z"}"#;
        let result = parse_entry(raw, 1);

        assert!(result.is_ok());
        let entry = result.unwrap();
        match entry.message().content() {
            MessageContent::Blocks(blocks) => {
                assert_eq!(blocks.len(), 1);
            }
            _ => panic!("Expected Blocks content"),
        }
    }

    #[test]
    fn parse_entry_with_subagent() {
        let raw = r#"{"type":"user","message":{"role":"user","content":"Test"},"sessionId":"s1","uuid":"u1","agentId":"agent-abc","timestamp":"2025-12-25T10:00:00Z"}"#;
        let result = parse_entry(raw, 1);

        assert!(result.is_ok());
        let entry = result.unwrap();
        assert!(entry.is_subagent(), "Should be subagent entry");
        assert_eq!(
            entry.agent_id().unwrap().as_str(),
            "agent-abc",
            "Should have correct agent_id"
        );
    }

    #[test]
    fn parse_entry_with_parent_uuid() {
        let raw = r#"{"type":"assistant","message":{"role":"assistant","content":"Test"},"sessionId":"s1","uuid":"u2","parentUuid":"u1","timestamp":"2025-12-25T10:00:00Z"}"#;
        let result = parse_entry(raw, 1);

        assert!(result.is_ok());
        let entry = result.unwrap();
        assert_eq!(
            entry.parent_uuid().unwrap().as_str(),
            "u1",
            "Should have parent_uuid"
        );
    }

    #[test]
    fn parse_entry_summary_type() {
        let raw = r#"{"type":"summary","message":{"role":"assistant","content":"Summary"},"sessionId":"s1","uuid":"u1","timestamp":"2025-12-25T10:00:00Z"}"#;
        let result = parse_entry(raw, 1);

        assert!(result.is_ok());
        let entry = result.unwrap();
        assert_eq!(entry.entry_type(), EntryType::Summary);
    }

    #[test]
    fn parse_entry_timestamp_formats() {
        let timestamps = vec![
            "2025-12-25T10:00:00Z",
            "2025-12-25T10:00:00.123Z",
            "2025-12-25T10:00:00+00:00",
        ];

        for ts in timestamps {
            let raw = format!(
                r#"{{"type":"user","message":{{"role":"user","content":"Test"}},"sessionId":"s1","uuid":"u1","timestamp":"{}"}}"#,
                ts
            );
            let result = parse_entry(&raw, 1);
            assert!(
                result.is_ok(),
                "Should parse timestamp format: {}",
                ts
            );
        }
    }

    // ===== Error Handling Tests =====

    #[test]
    fn parse_entry_malformed_json() {
        let raw = r#"{"type":"user","message":{"role":"user""#;
        let result = parse_entry(raw, 42);

        assert!(result.is_err(), "Should reject malformed JSON");
        match result.unwrap_err() {
            ParseError::InvalidJson { line, message } => {
                assert_eq!(line, 42, "Should preserve line number");
                assert!(
                    !message.is_empty(),
                    "Should have error message"
                );
            }
            _ => panic!("Expected InvalidJson error"),
        }
    }

    #[test]
    fn parse_entry_missing_uuid() {
        let raw = r#"{"type":"user","message":{"role":"user","content":"Test"},"sessionId":"s1","timestamp":"2025-12-25T10:00:00Z"}"#;
        let result = parse_entry(raw, 15);

        assert!(result.is_err(), "Should reject missing uuid");
        match result.unwrap_err() {
            ParseError::MissingField { line, field } => {
                assert_eq!(line, 15);
                assert_eq!(field, "uuid");
            }
            _ => panic!("Expected MissingField error"),
        }
    }

    #[test]
    fn parse_entry_missing_session_id() {
        let raw = r#"{"type":"user","message":{"role":"user","content":"Test"},"uuid":"u1","timestamp":"2025-12-25T10:00:00Z"}"#;
        let result = parse_entry(raw, 20);

        assert!(result.is_err(), "Should reject missing sessionId");
        match result.unwrap_err() {
            ParseError::MissingField { line, field } => {
                assert_eq!(line, 20);
                assert_eq!(field, "sessionId");
            }
            _ => panic!("Expected MissingField error"),
        }
    }

    #[test]
    fn parse_entry_missing_timestamp() {
        let raw = r#"{"type":"user","message":{"role":"user","content":"Test"},"sessionId":"s1","uuid":"u1"}"#;
        let result = parse_entry(raw, 8);

        assert!(result.is_err(), "Should reject missing timestamp");
        match result.unwrap_err() {
            ParseError::MissingField { line, field } => {
                assert_eq!(line, 8);
                assert_eq!(field, "timestamp");
            }
            _ => panic!("Expected MissingField error"),
        }
    }

    #[test]
    fn parse_entry_invalid_timestamp() {
        let raw = r#"{"type":"user","message":{"role":"user","content":"Test"},"sessionId":"s1","uuid":"u1","timestamp":"not-a-timestamp"}"#;
        let result = parse_entry(raw, 99);

        assert!(result.is_err(), "Should reject invalid timestamp");
        match result.unwrap_err() {
            ParseError::InvalidTimestamp { line, raw } => {
                assert_eq!(line, 99);
                assert_eq!(raw, "not-a-timestamp");
            }
            _ => panic!("Expected InvalidTimestamp error"),
        }
    }

    #[test]
    fn parse_entry_empty_uuid() {
        let raw = r#"{"type":"user","message":{"role":"user","content":"Test"},"sessionId":"s1","uuid":"","timestamp":"2025-12-25T10:00:00Z"}"#;
        let result = parse_entry(raw, 5);

        assert!(result.is_err(), "Should reject empty uuid");
        // Empty UUID validation happens during newtype construction
        match result.unwrap_err() {
            ParseError::MissingField { line, field } => {
                assert_eq!(line, 5);
                assert_eq!(field, "uuid");
            }
            _ => panic!("Expected MissingField error for empty uuid"),
        }
    }

    #[test]
    fn parse_entry_empty_session_id() {
        let raw = r#"{"type":"user","message":{"role":"user","content":"Test"},"sessionId":"","uuid":"u1","timestamp":"2025-12-25T10:00:00Z"}"#;
        let result = parse_entry(raw, 7);

        assert!(result.is_err(), "Should reject empty sessionId");
        match result.unwrap_err() {
            ParseError::MissingField { line, field } => {
                assert_eq!(line, 7);
                assert_eq!(field, "sessionId");
            }
            _ => panic!("Expected MissingField error for empty sessionId"),
        }
    }

    // ===== Entry Type Parsing Tests =====

    #[test]
    fn parse_entry_type_recognizes_user() {
        assert_eq!(parse_entry_type("user"), Some(EntryType::User));
    }

    #[test]
    fn parse_entry_type_recognizes_assistant() {
        assert_eq!(parse_entry_type("assistant"), Some(EntryType::Assistant));
    }

    #[test]
    fn parse_entry_type_recognizes_summary() {
        assert_eq!(parse_entry_type("summary"), Some(EntryType::Summary));
    }

    #[test]
    fn parse_entry_type_rejects_unknown() {
        assert_eq!(parse_entry_type("unknown"), None);
        assert_eq!(parse_entry_type(""), None);
        assert_eq!(parse_entry_type("USER"), None); // Case sensitive
    }
}
