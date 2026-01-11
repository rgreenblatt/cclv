//! JSONL parser for Claude Code log entries.
//!
//! This module provides pure parsing functions for converting JSONL lines
//! into validated LogEntry structs.

use crate::model::{
    AgentId, ContentBlock, EntryMetadata, EntryType, EntryUuid, LogEntry, MalformedEntry, Message,
    MessageContent, ModelInfo, ParseError, ResultMetadata, Role, SessionId, SystemMetadata,
    TokenUsage, ToolCall, ToolName, ToolUseId,
};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use std::path::PathBuf;

// Entry type string constants
const ENTRY_TYPE_USER: &str = "user";
const ENTRY_TYPE_ASSISTANT: &str = "assistant";
const ENTRY_TYPE_SUMMARY: &str = "summary";
const ENTRY_TYPE_SYSTEM: &str = "system";
const ENTRY_TYPE_RESULT: &str = "result";

// Role string constants
const ROLE_USER: &str = "user";
const ROLE_ASSISTANT: &str = "assistant";

// Session ID constants
pub(crate) const UNKNOWN_SESSION_ID: &str = "unknown-session";

/// Raw JSON structure for deserializing log entries.
#[derive(Debug, Deserialize)]
struct RawLogEntry {
    #[serde(rename = "type")]
    entry_type: String,
    #[serde(default)]
    message: Option<RawMessage>,
    #[serde(default)]
    session_id: Option<String>,
    uuid: String,
    #[serde(default)]
    parent_tool_use_id: Option<String>,
    #[serde(default, rename = "agentId")]
    agent_id: Option<String>,
    #[serde(default)]
    timestamp: Option<String>,
    #[serde(default)]
    cwd: Option<String>,
    #[serde(default, rename = "gitBranch")]
    git_branch: Option<String>,
    #[serde(default)]
    version: Option<String>,
    #[serde(default, rename = "isSidechain")]
    is_sidechain: bool,
    // System entry fields (FMT-006)
    #[serde(default)]
    subtype: Option<String>,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    tools: Option<Vec<String>>,
    #[serde(default)]
    agents: Option<Vec<String>>,
    #[serde(default)]
    skills: Option<Vec<String>>,
    // Result entry fields (FMT-007)
    #[serde(default)]
    is_error: Option<bool>,
    #[serde(default)]
    duration_ms: Option<u64>,
    #[serde(default)]
    num_turns: Option<u32>,
    #[serde(default)]
    total_cost_usd: Option<f64>,
    #[serde(default)]
    result: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawMessage {
    role: String,
    content: RawMessageContent,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    usage: Option<RawTokenUsage>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum RawMessageContent {
    Text(String),
    Blocks(Vec<RawContentBlock>),
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum RawContentBlock {
    Text {
        text: String,
    },
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    ToolResult {
        tool_use_id: String,
        content: String,
        #[serde(default)]
        is_error: bool,
    },
    Thinking {
        thinking: String,
    },
}

/// Nested cache_creation object from usage field.
#[derive(Debug, Deserialize)]
struct RawCacheCreation {
    #[serde(default)]
    ephemeral_5m_input_tokens: u64,
    #[serde(default)]
    ephemeral_1h_input_tokens: u64,
}

#[derive(Debug, Deserialize)]
struct RawTokenUsage {
    input_tokens: u64,
    output_tokens: u64,
    #[serde(default)]
    cache_creation_input_tokens: u64,
    #[serde(default)]
    cache_read_input_tokens: u64,
    #[serde(default)]
    cache_creation: Option<RawCacheCreation>,
}

/// Result of parsing a JSONL line with graceful error handling.
///
/// This allows the parser to continue processing subsequent lines
/// even when encountering malformed JSON.
#[derive(Debug, Clone)]
pub enum ParseResult {
    /// Successfully parsed a valid log entry.
    Valid(Box<LogEntry>),
    /// Encountered a malformed line that could not be parsed.
    Malformed(MalformedEntry),
}

/// Parse a single JSONL line gracefully.
///
/// Unlike `parse_entry()`, this function never returns an error.
/// Instead, it returns either a valid LogEntry or a MalformedEntry
/// that can be displayed inline.
///
/// This satisfies FR-010: handle malformed JSON lines gracefully.
///
/// # Arguments
///
/// * `raw` - The raw JSONL line to parse
/// * `line_number` - The line number (1-indexed) for error reporting
pub fn parse_entry_graceful(raw: &str, line_number: usize) -> ParseResult {
    // Attempt to parse the entry
    match parse_entry(raw, line_number) {
        Ok(entry) => ParseResult::Valid(Box::new(entry)),
        Err(parse_error) => {
            // Parsing failed - create a malformed entry
            // Try to extract session_id if the JSON is partially parseable
            let session_id = extract_session_id_best_effort(raw);

            ParseResult::Malformed(MalformedEntry::new(
                line_number,
                raw,
                parse_error.to_string(),
                session_id,
            ))
        }
    }
}

/// Attempt to extract session_id from malformed JSON on a best-effort basis.
///
/// This is used when a line fails to parse but we want to associate it with
/// a session if possible. Returns None if extraction fails.
fn extract_session_id_best_effort(raw: &str) -> Option<SessionId> {
    // Try to deserialize just enough to get the session_id field
    #[derive(Deserialize)]
    struct PartialEntry {
        session_id: Option<String>,
    }

    serde_json::from_str::<PartialEntry>(raw)
        .ok()
        .and_then(|partial| partial.session_id)
        .and_then(|id| SessionId::new(id).ok())
}

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
    // Deserialize JSON
    let raw_entry: RawLogEntry =
        serde_json::from_str(raw).map_err(|e| ParseError::InvalidJson {
            line: line_number,
            message: e.to_string(),
        })?;

    // Parse entry type
    let entry_type = parse_entry_type(&raw_entry.entry_type).ok_or(ParseError::MissingField {
        line: line_number,
        field: "type",
    })?;

    // Validate and construct UUIDs
    let uuid = EntryUuid::new(&raw_entry.uuid).map_err(|_| ParseError::MissingField {
        line: line_number,
        field: "uuid",
    })?;

    let parent_uuid = raw_entry
        .parent_tool_use_id
        .as_ref()
        .map(|s| {
            EntryUuid::new(s.as_str()).map_err(|_| ParseError::MissingField {
                line: line_number,
                field: "parent_tool_use_id",
            })
        })
        .transpose()?;

    // Validate and construct session ID (use unknown as fallback)
    let session_id = match &raw_entry.session_id {
        Some(id) if !id.is_empty() => {
            SessionId::new(id.as_str()).map_err(|_| ParseError::MissingField {
                line: line_number,
                field: "sessionId",
            })?
        }
        _ => SessionId::unknown(),
    };

    // Validate and construct agent ID (optional)
    // Use parent_tool_use_id as the agent identifier (subagent entries)
    // Fall back to agentId field if present (future compatibility)
    let agent_id = match (&raw_entry.parent_tool_use_id, &raw_entry.agent_id) {
        (Some(parent_id), _) if !parent_id.is_empty() => {
            // Entry has parent_tool_use_id -> this is a subagent entry
            Some(
                AgentId::new(parent_id.as_str()).map_err(|_| ParseError::MissingField {
                    line: line_number,
                    field: "parent_tool_use_id (used as agent_id)",
                })?,
            )
        }
        (_, Some(agent_id_str)) if !agent_id_str.is_empty() => {
            // Entry has explicit agentId field (future compatibility)
            Some(
                AgentId::new(agent_id_str.as_str()).map_err(|_| ParseError::MissingField {
                    line: line_number,
                    field: "agentId",
                })?,
            )
        }
        _ => None, // Main agent entry (no parent_tool_use_id, no agentId)
    };

    // Parse timestamp (optional - use epoch if missing)
    let timestamp: DateTime<Utc> = match &raw_entry.timestamp {
        Some(ts) => ts.parse().map_err(|_| ParseError::InvalidTimestamp {
            line: line_number,
            raw: ts.clone(),
        })?,
        None => DateTime::UNIX_EPOCH,
    };

    // Parse system metadata for System entries BEFORE consuming raw_entry (FMT-006)
    let system_metadata = if entry_type == EntryType::System {
        parse_system_metadata(&raw_entry)
    } else {
        None
    };

    // Parse result metadata for Result entries BEFORE consuming raw_entry (FMT-007)
    let result_metadata = if entry_type == EntryType::Result {
        parse_result_metadata(&raw_entry)
    } else {
        None
    };

    // Parse message (optional for system and result entries)
    let message = match raw_entry.message {
        Some(raw_msg) => parse_message(raw_msg)?,
        None => {
            // System and Result entries may not have a message - create empty assistant message as placeholder
            Message::new(Role::Assistant, MessageContent::Text(String::new()))
        }
    };

    // Construct metadata
    let metadata = EntryMetadata {
        cwd: raw_entry.cwd.map(PathBuf::from),
        git_branch: raw_entry.git_branch,
        version: raw_entry.version,
        is_sidechain: raw_entry.is_sidechain,
    };

    // Use appropriate constructor based on entry type
    if result_metadata.is_some() {
        Ok(LogEntry::new_with_result_metadata(
            uuid,
            parent_uuid,
            session_id,
            agent_id,
            timestamp,
            entry_type,
            message,
            metadata,
            result_metadata,
        ))
    } else {
        Ok(LogEntry::new_with_system_metadata(
            uuid,
            parent_uuid,
            session_id,
            agent_id,
            timestamp,
            entry_type,
            message,
            metadata,
            system_metadata,
        ))
    }
}

/// Parse the "type" field into EntryType enum.
fn parse_entry_type(type_str: &str) -> Option<EntryType> {
    match type_str {
        ENTRY_TYPE_USER => Some(EntryType::User),
        ENTRY_TYPE_ASSISTANT => Some(EntryType::Assistant),
        ENTRY_TYPE_SUMMARY => Some(EntryType::Summary),
        ENTRY_TYPE_SYSTEM => Some(EntryType::System),
        ENTRY_TYPE_RESULT => Some(EntryType::Result),
        _ => None,
    }
}

/// Parse system metadata from a RawLogEntry for System entries.
///
/// Extracts subtype, cwd, model, tools, agents, and skills fields.
/// Returns None if subtype is missing (required for SystemMetadata).
fn parse_system_metadata(raw: &RawLogEntry) -> Option<SystemMetadata> {
    // Subtype is required for system metadata
    let subtype = raw.subtype.as_ref()?.clone();

    Some(SystemMetadata {
        subtype,
        cwd: raw.cwd.as_ref().map(PathBuf::from),
        model: raw.model.clone(),
        tools: raw.tools.clone().unwrap_or_default(),
        agents: raw.agents.clone().unwrap_or_default(),
        skills: raw.skills.clone().unwrap_or_default(),
    })
}

/// Parse result metadata from a RawLogEntry for Result entries.
///
/// Extracts is_error, duration_ms, num_turns, total_cost_usd, and result text.
/// Returns None if required fields are missing.
fn parse_result_metadata(raw: &RawLogEntry) -> Option<ResultMetadata> {
    // All fields are required for result metadata
    let is_error = raw.is_error?;
    let duration_ms = raw.duration_ms?;
    let num_turns = raw.num_turns?;
    let total_cost_usd = raw.total_cost_usd?;
    let result_text = raw.result.as_ref()?.clone();

    Some(ResultMetadata {
        is_error,
        duration_ms,
        num_turns,
        total_cost_usd,
        result_text,
    })
}

/// Parse a raw message into a Message.
fn parse_message(raw: RawMessage) -> Result<Message, ParseError> {
    // Parse role
    let role = match raw.role.as_str() {
        ROLE_USER => Role::User,
        ROLE_ASSISTANT => Role::Assistant,
        _ => Role::Assistant, // Default to assistant for unknown roles
    };

    // Parse content
    let content = match raw.content {
        RawMessageContent::Text(text) => MessageContent::Text(text),
        RawMessageContent::Blocks(blocks) => {
            let parsed_blocks: Vec<ContentBlock> = blocks
                .into_iter()
                .map(parse_content_block)
                .collect::<Result<_, _>>()?;
            MessageContent::Blocks(parsed_blocks)
        }
    };

    // Create message
    let mut message = Message::new(role, content);

    // Add model if present
    if let Some(model_str) = raw.model {
        message = message.with_model(ModelInfo::new(model_str));
    }

    // Add usage if present
    if let Some(raw_usage) = raw.usage {
        // Extract ephemeral breakdown from nested cache_creation object
        let (ephemeral_5m, ephemeral_1h) = raw_usage
            .cache_creation
            .as_ref()
            .map(|cc| (cc.ephemeral_5m_input_tokens, cc.ephemeral_1h_input_tokens))
            .unwrap_or((0, 0));

        let usage = TokenUsage {
            input_tokens: raw_usage.input_tokens,
            output_tokens: raw_usage.output_tokens,
            cache_creation_input_tokens: raw_usage.cache_creation_input_tokens,
            cache_read_input_tokens: raw_usage.cache_read_input_tokens,
            ephemeral_5m_input_tokens: ephemeral_5m,
            ephemeral_1h_input_tokens: ephemeral_1h,
        };
        message = message.with_usage(usage);
    }

    Ok(message)
}

/// Parse a raw content block into a ContentBlock.
fn parse_content_block(raw: RawContentBlock) -> Result<ContentBlock, ParseError> {
    match raw {
        RawContentBlock::Text { text } => Ok(ContentBlock::Text { text }),
        RawContentBlock::ToolUse { id, name, input } => {
            let tool_use_id = ToolUseId::new(id).map_err(|_| ParseError::MissingField {
                line: 0, // Line number not available at this level
                field: "tool_use.id",
            })?;
            let tool_name = ToolName::parse(&name);
            Ok(ContentBlock::ToolUse(ToolCall::new(
                tool_use_id,
                tool_name,
                input,
            )))
        }
        RawContentBlock::ToolResult {
            tool_use_id,
            content,
            is_error,
        } => {
            let id = ToolUseId::new(tool_use_id).map_err(|_| ParseError::MissingField {
                line: 0,
                field: "tool_result.tool_use_id",
            })?;
            Ok(ContentBlock::ToolResult {
                tool_use_id: id,
                content,
                is_error,
            })
        }
        RawContentBlock::Thinking { thinking } => Ok(ContentBlock::Thinking { thinking }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{EntryType, MessageContent, Role};

    // ===== Successful Parsing Tests =====

    #[test]
    fn parse_entry_minimal_user_message() {
        let raw = r#"{"type":"user","message":{"role":"user","content":"Hello"},"session_id":"session-123","uuid":"uuid-001"}"#;
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
        let raw = r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"Hi there"}],"model":"claude-opus-4-5-20251101","usage":{"input_tokens":100,"output_tokens":50,"cache_creation_input_tokens":20,"cache_read_input_tokens":10}},"session_id":"session-123","uuid":"uuid-002"}"#;
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
        let raw = r#"{"type":"user","message":{"role":"user","content":"Simple text"},"session_id":"s1","uuid":"u1"}"#;
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
        let raw = r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"Block text"}]},"session_id":"s1","uuid":"u1"}"#;
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
        let raw = r#"{"type":"user","message":{"role":"user","content":"Test"},"session_id":"s1","uuid":"u1","agentId":"agent-abc"}"#;
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
        // Updated to use parent_tool_use_id (actual Claude Code format)
        let raw = r#"{"type":"assistant","message":{"role":"assistant","content":"Test"},"session_id":"s1","uuid":"u2","parent_tool_use_id":"u1"}"#;
        let result = parse_entry(raw, 1);

        assert!(result.is_ok());
        let entry = result.unwrap();
        assert_eq!(
            entry.parent_uuid().unwrap().as_str(),
            "u1",
            "Should have parent_uuid"
        );
    }

    // ===== Bug Fix: FMT-004 - parent_tool_use_id field =====

    #[test]
    fn parse_entry_with_parent_tool_use_id() {
        // RED TEST: Actual Claude Code format uses parent_tool_use_id, not parentUuid
        // This test expects the actual field name used in Claude Code JSONL
        let raw = r#"{"type":"assistant","message":{"role":"assistant","content":"Test"},"session_id":"s1","uuid":"u2","parent_tool_use_id":"tool-123"}"#;
        let result = parse_entry(raw, 1);

        assert!(
            result.is_ok(),
            "Should parse entry with parent_tool_use_id field (actual Claude Code format)"
        );
        let entry = result.unwrap();
        assert_eq!(
            entry.parent_uuid().unwrap().as_str(),
            "tool-123",
            "Should parse parent_tool_use_id into parent_uuid field"
        );
    }

    #[test]
    fn parse_entry_with_null_parent_tool_use_id() {
        // parent_tool_use_id is null for top-level entries
        let raw = r#"{"type":"user","message":{"role":"user","content":"Test"},"session_id":"s1","uuid":"u1","parent_tool_use_id":null}"#;
        let result = parse_entry(raw, 1);

        assert!(
            result.is_ok(),
            "Should parse entry with null parent_tool_use_id"
        );
        let entry = result.unwrap();
        assert!(
            entry.parent_uuid().is_none(),
            "Should have None for null parent_tool_use_id"
        );
    }

    #[test]
    fn parse_entry_summary_type() {
        let raw = r#"{"type":"summary","message":{"role":"assistant","content":"Summary"},"session_id":"s1","uuid":"u1"}"#;
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
                r#"{{"type":"user","message":{{"role":"user","content":"Test"}},"session_id":"s1","uuid":"u1","timestamp":"{}"}}"#,
                ts
            );
            let result = parse_entry(&raw, 1);
            assert!(result.is_ok(), "Should parse timestamp format: {}", ts);
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
                assert!(!message.is_empty(), "Should have error message");
            }
            _ => panic!("Expected InvalidJson error"),
        }
    }

    #[test]
    fn parse_entry_missing_uuid() {
        let raw = r#"{"type":"user","message":{"role":"user","content":"Test"},"session_id":"s1"}"#;
        let result = parse_entry(raw, 15);

        assert!(result.is_err(), "Should reject missing uuid");
        match result.unwrap_err() {
            ParseError::InvalidJson { line, message } => {
                assert_eq!(line, 15);
                assert!(
                    message.contains("uuid") || message.contains("missing field"),
                    "Error should mention uuid or missing field, got: {}",
                    message
                );
            }
            _ => panic!("Expected InvalidJson error for missing required field"),
        }
    }

    #[test]
    fn parse_entry_missing_session_id() {
        // Updated: missing sessionId now defaults to unknown instead of error
        let raw = r#"{"type":"user","message":{"role":"user","content":"Test"},"uuid":"u1"}"#;
        let result = parse_entry(raw, 20);

        assert!(
            result.is_ok(),
            "Should parse successfully with missing sessionId"
        );
        let entry = result.unwrap();
        assert_eq!(
            entry.session_id().as_str(),
            "unknown-session",
            "Should use unknown when sessionId is missing"
        );
    }

    #[test]
    fn parse_entry_missing_timestamp() {
        // Updated: timestamp is now optional, missing timestamp should parse successfully
        let raw = r#"{"type":"user","message":{"role":"user","content":"Test"},"session_id":"s1","uuid":"u1"}"#;
        let result = parse_entry(raw, 8);

        assert!(
            result.is_ok(),
            "Should accept missing timestamp (now optional)"
        );
        let entry = result.unwrap();
        // Should use epoch fallback when timestamp is missing
        assert_eq!(entry.timestamp(), DateTime::UNIX_EPOCH);
    }

    #[test]
    fn parse_entry_invalid_timestamp() {
        let raw = r#"{"type":"user","message":{"role":"user","content":"Test"},"session_id":"s1","uuid":"u1","timestamp":"not-a-timestamp"}"#;
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
        let raw = r#"{"type":"user","message":{"role":"user","content":"Test"},"session_id":"s1","uuid":""}"#;
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
        // Updated: empty sessionId now defaults to unknown instead of error
        let raw = r#"{"type":"user","message":{"role":"user","content":"Test"},"session_id":"","uuid":"u1"}"#;
        let result = parse_entry(raw, 7);

        assert!(
            result.is_ok(),
            "Should parse successfully with empty sessionId"
        );
        let entry = result.unwrap();
        assert_eq!(
            entry.session_id().as_str(),
            "unknown-session",
            "Should use unknown when sessionId is empty"
        );
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
    fn parse_entry_type_recognizes_system() {
        assert_eq!(parse_entry_type("system"), Some(EntryType::System));
    }

    #[test]
    fn parse_entry_type_recognizes_result() {
        assert_eq!(parse_entry_type("result"), Some(EntryType::Result));
    }

    #[test]
    fn parse_entry_type_rejects_unknown() {
        assert_eq!(parse_entry_type("unknown"), None);
        assert_eq!(parse_entry_type(""), None);
        assert_eq!(parse_entry_type("USER"), None); // Case sensitive
    }

    // ===== Graceful Parsing Tests =====

    #[test]
    fn parse_entry_graceful_returns_valid_for_correct_json() {
        let raw = r#"{"type":"user","message":{"role":"user","content":"Hello"},"session_id":"session-123","uuid":"uuid-001"}"#;
        let result = parse_entry_graceful(raw, 1);

        match result {
            ParseResult::Valid(entry) => {
                assert_eq!(entry.uuid().as_str(), "uuid-001");
                assert_eq!(entry.session_id().as_str(), "session-123");
            }
            ParseResult::Malformed(_) => panic!("Expected Valid, got Malformed"),
        }
    }

    #[test]
    fn parse_entry_graceful_returns_malformed_for_invalid_json() {
        let raw = r#"{"type":"user","message":{"role":"user""#;
        let result = parse_entry_graceful(raw, 42);

        match result {
            ParseResult::Malformed(malformed) => {
                assert_eq!(malformed.line_number(), 42, "Should preserve line number");
                assert_eq!(
                    malformed.raw_line(),
                    raw,
                    "Should preserve raw line content"
                );
                assert!(
                    !malformed.error_message().is_empty(),
                    "Should have error message"
                );
            }
            ParseResult::Valid(_) => panic!("Expected Malformed, got Valid"),
        }
    }

    #[test]
    fn parse_entry_graceful_returns_malformed_for_missing_required_field() {
        let raw = r#"{"type":"user","message":{"role":"user","content":"Test"},"session_id":"s1"}"#;
        let result = parse_entry_graceful(raw, 15);

        match result {
            ParseResult::Malformed(malformed) => {
                assert_eq!(malformed.line_number(), 15);
                assert_eq!(malformed.raw_line(), raw);
                assert!(
                    malformed.error_message().contains("uuid")
                        || malformed.error_message().contains("missing"),
                    "Error message should mention missing field, got: {}",
                    malformed.error_message()
                );
            }
            ParseResult::Valid(_) => panic!("Expected Malformed for missing uuid"),
        }
    }

    #[test]
    fn parse_entry_graceful_returns_malformed_for_invalid_timestamp() {
        let raw = r#"{"type":"user","message":{"role":"user","content":"Test"},"session_id":"s1","uuid":"u1","timestamp":"not-a-timestamp"}"#;
        let result = parse_entry_graceful(raw, 99);

        match result {
            ParseResult::Malformed(malformed) => {
                assert_eq!(malformed.line_number(), 99);
                assert!(
                    malformed.error_message().contains("timestamp")
                        || malformed.error_message().contains("not-a-timestamp"),
                    "Error should mention timestamp issue, got: {}",
                    malformed.error_message()
                );
            }
            ParseResult::Valid(_) => panic!("Expected Malformed for invalid timestamp"),
        }
    }

    #[test]
    fn parse_entry_graceful_preserves_raw_line_exactly() {
        let raw = r#"{"malformed":true, "weird": "spacing"   }"#;
        let result = parse_entry_graceful(raw, 5);

        match result {
            ParseResult::Malformed(malformed) => {
                assert_eq!(
                    malformed.raw_line(),
                    raw,
                    "Should preserve exact raw line including spacing"
                );
            }
            ParseResult::Valid(_) => panic!("Expected Malformed"),
        }
    }

    #[test]
    fn parse_entry_graceful_extracts_session_id_when_possible() {
        // Malformed due to missing uuid, but session_id is present and extractable
        let raw = r#"{"type":"user","message":{"role":"user","content":"Test"},"session_id":"extractable-session"}"#;
        let result = parse_entry_graceful(raw, 10);

        match result {
            ParseResult::Malformed(malformed) => {
                // Session ID extraction is optional/best-effort
                // This test documents the behavior but doesn't require it
                let _session_id = malformed.session_id();
                // Test passes as long as it's Malformed (session_id extraction is optional)
            }
            ParseResult::Valid(_) => panic!("Expected Malformed for missing uuid"),
        }
    }

    #[test]
    fn parse_entry_graceful_handles_empty_line() {
        let raw = "";
        let result = parse_entry_graceful(raw, 1);

        match result {
            ParseResult::Malformed(malformed) => {
                assert_eq!(malformed.line_number(), 1);
                assert_eq!(malformed.raw_line(), "");
            }
            ParseResult::Valid(_) => panic!("Expected Malformed for empty line"),
        }
    }

    #[test]
    fn parse_entry_graceful_handles_whitespace_only() {
        let raw = "   \t  \n  ";
        let result = parse_entry_graceful(raw, 7);

        match result {
            ParseResult::Malformed(malformed) => {
                assert_eq!(malformed.line_number(), 7);
                assert_eq!(malformed.raw_line(), raw);
            }
            ParseResult::Valid(_) => panic!("Expected Malformed for whitespace"),
        }
    }

    #[test]
    fn parse_entry_graceful_handles_non_json_text() {
        let raw = "This is just plain text, not JSON at all";
        let result = parse_entry_graceful(raw, 23);

        match result {
            ParseResult::Malformed(malformed) => {
                assert_eq!(malformed.line_number(), 23);
                assert_eq!(malformed.raw_line(), raw);
                assert!(!malformed.error_message().is_empty());
            }
            ParseResult::Valid(_) => panic!("Expected Malformed for non-JSON text"),
        }
    }

    // ===== MalformedEntry Tests =====

    #[test]
    fn malformed_entry_stores_all_fields() {
        let malformed = MalformedEntry::new(42, "bad json", "Parse error: unexpected token", None);

        assert_eq!(malformed.line_number(), 42);
        assert_eq!(malformed.raw_line(), "bad json");
        assert_eq!(malformed.error_message(), "Parse error: unexpected token");
        assert!(malformed.session_id().is_none());
    }

    #[test]
    fn malformed_entry_stores_session_id_when_provided() {
        let session_id = SessionId::new("session-123").unwrap();
        let malformed = MalformedEntry::new(
            10,
            "partial json",
            "Missing field",
            Some(session_id.clone()),
        );

        assert_eq!(malformed.session_id(), Some(&session_id));
    }

    #[test]
    fn malformed_entry_accepts_string_types() {
        let malformed = MalformedEntry::new(
            1,
            String::from("owned string"),
            String::from("owned error"),
            None,
        );

        assert_eq!(malformed.raw_line(), "owned string");
        assert_eq!(malformed.error_message(), "owned error");
    }

    // ===== ContentBlock Variant Tests =====

    #[test]
    fn parse_entry_with_tool_use_block() {
        let raw = r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","id":"tool-123","name":"Read","input":{"file_path":"test.txt"}}]},"session_id":"s1","uuid":"u1"}"#;
        let result = parse_entry(raw, 1);

        assert!(result.is_ok(), "Should parse entry with ToolUse block");
        let entry = result.unwrap();
        match entry.message().content() {
            MessageContent::Blocks(blocks) => {
                assert_eq!(blocks.len(), 1);
                match &blocks[0] {
                    ContentBlock::ToolUse(call) => {
                        assert_eq!(call.id().as_str(), "tool-123");
                        assert_eq!(call.name(), &ToolName::Read);
                        assert_eq!(call.input()["file_path"], "test.txt");
                    }
                    _ => panic!("Expected ToolUse block"),
                }
            }
            _ => panic!("Expected Blocks content"),
        }
    }

    #[test]
    fn parse_entry_with_tool_result_block_success() {
        let raw = r#"{"type":"user","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"tool-456","content":"file contents here","is_error":false}]},"session_id":"s1","uuid":"u1"}"#;
        let result = parse_entry(raw, 1);

        assert!(result.is_ok(), "Should parse entry with ToolResult block");
        let entry = result.unwrap();
        match entry.message().content() {
            MessageContent::Blocks(blocks) => {
                assert_eq!(blocks.len(), 1);
                match &blocks[0] {
                    ContentBlock::ToolResult {
                        tool_use_id,
                        content,
                        is_error,
                    } => {
                        assert_eq!(tool_use_id.as_str(), "tool-456");
                        assert_eq!(content, "file contents here");
                        assert!(!is_error, "Should not be error");
                    }
                    _ => panic!("Expected ToolResult block"),
                }
            }
            _ => panic!("Expected Blocks content"),
        }
    }

    #[test]
    fn parse_entry_with_tool_result_block_error() {
        let raw = r#"{"type":"user","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"tool-789","content":"Error: file not found","is_error":true}]},"session_id":"s1","uuid":"u1"}"#;
        let result = parse_entry(raw, 1);

        assert!(result.is_ok(), "Should parse entry with ToolResult error");
        let entry = result.unwrap();
        match entry.message().content() {
            MessageContent::Blocks(blocks) => {
                assert_eq!(blocks.len(), 1);
                match &blocks[0] {
                    ContentBlock::ToolResult {
                        tool_use_id,
                        content,
                        is_error,
                    } => {
                        assert_eq!(tool_use_id.as_str(), "tool-789");
                        assert_eq!(content, "Error: file not found");
                        assert!(is_error, "Should be error");
                    }
                    _ => panic!("Expected ToolResult block"),
                }
            }
            _ => panic!("Expected Blocks content"),
        }
    }

    #[test]
    fn parse_entry_with_tool_result_defaults_is_error_to_false() {
        let raw = r#"{"type":"user","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"tool-999","content":"output"}]},"session_id":"s1","uuid":"u1"}"#;
        let result = parse_entry(raw, 1);

        assert!(
            result.is_ok(),
            "Should parse ToolResult without is_error field"
        );
        let entry = result.unwrap();
        match entry.message().content() {
            MessageContent::Blocks(blocks) => match &blocks[0] {
                ContentBlock::ToolResult { is_error, .. } => {
                    assert!(!is_error, "is_error should default to false");
                }
                _ => panic!("Expected ToolResult block"),
            },
            _ => panic!("Expected Blocks content"),
        }
    }

    #[test]
    fn parse_entry_with_thinking_block() {
        let raw = r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"thinking","thinking":"Let me analyze this problem..."}]},"session_id":"s1","uuid":"u1"}"#;
        let result = parse_entry(raw, 1);

        assert!(result.is_ok(), "Should parse entry with Thinking block");
        let entry = result.unwrap();
        match entry.message().content() {
            MessageContent::Blocks(blocks) => {
                assert_eq!(blocks.len(), 1);
                match &blocks[0] {
                    ContentBlock::Thinking { thinking } => {
                        assert_eq!(thinking, "Let me analyze this problem...");
                    }
                    _ => panic!("Expected Thinking block"),
                }
            }
            _ => panic!("Expected Blocks content"),
        }
    }

    #[test]
    fn parse_entry_with_mixed_content_blocks() {
        let raw = r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"I'll read the file"},{"type":"thinking","thinking":"Using Read tool"},{"type":"tool_use","id":"t1","name":"Read","input":{"file":"a.txt"}},{"type":"text","text":"Done"}]},"session_id":"s1","uuid":"u1"}"#;
        let result = parse_entry(raw, 1);

        assert!(result.is_ok(), "Should parse entry with mixed blocks");
        let entry = result.unwrap();
        match entry.message().content() {
            MessageContent::Blocks(blocks) => {
                assert_eq!(blocks.len(), 4);
                assert!(matches!(blocks[0], ContentBlock::Text { .. }));
                assert!(matches!(blocks[1], ContentBlock::Thinking { .. }));
                assert!(matches!(blocks[2], ContentBlock::ToolUse(_)));
                assert!(matches!(blocks[3], ContentBlock::Text { .. }));
            }
            _ => panic!("Expected Blocks content"),
        }
    }

    // ===== Bug Fix: cclv-07v.9.16 - Optional sessionId =====

    #[test]
    fn parse_entry_missing_session_id_uses_unknown() {
        // Entry with no sessionId field should parse successfully
        // using SessionId::unknown() ("unknown-session")
        let raw = r#"{"type":"user","message":{"role":"user","content":"hello"},"uuid":"abc-123"}"#;

        let result = parse_entry(raw, 1);
        assert!(
            result.is_ok(),
            "Should parse successfully when sessionId is missing"
        );
        let entry = result.unwrap();
        assert_eq!(
            entry.session_id().as_str(),
            "unknown-session",
            "Should use SessionId::unknown() when sessionId is missing"
        );
    }

    #[test]
    fn parse_entry_empty_session_id_uses_unknown() {
        // Entry with empty sessionId field should use unknown
        let raw = r#"{"type":"user","message":{"role":"user","content":"hello"},"uuid":"abc-123","session_id":""}"#;

        let result = parse_entry(raw, 1);
        assert!(
            result.is_ok(),
            "Should parse successfully when sessionId is empty"
        );
        let entry = result.unwrap();
        assert_eq!(
            entry.session_id().as_str(),
            "unknown-session",
            "Should use SessionId::unknown() when sessionId is empty"
        );
    }

    #[test]
    fn parse_entry_null_session_id_uses_unknown() {
        // Entry with null sessionId field should use unknown
        let raw = r#"{"type":"user","message":{"role":"user","content":"hello"},"uuid":"abc-123","session_id":null}"#;

        let result = parse_entry(raw, 1);
        assert!(
            result.is_ok(),
            "Should parse successfully when sessionId is null"
        );
        let entry = result.unwrap();
        assert_eq!(
            entry.session_id().as_str(),
            "unknown-session",
            "Should use SessionId::unknown() when sessionId is null"
        );
    }

    #[test]
    fn parse_entry_valid_session_id_preserved() {
        // Entry with valid sessionId should still work
        let raw = r#"{"type":"user","message":{"role":"user","content":"hello"},"uuid":"abc-123","session_id":"my-session"}"#;

        let result = parse_entry(raw, 1);
        assert!(result.is_ok(), "Should parse with valid sessionId");
        let entry = result.unwrap();
        assert_eq!(
            entry.session_id().as_str(),
            "my-session",
            "Should preserve valid sessionId"
        );
    }

    // ===== Bug Fix: FMT-005 - Nested cache_creation structure =====

    #[test]
    fn parse_entry_with_nested_cache_creation() {
        // RED TEST: Parse usage with nested cache_creation object containing ephemeral breakdowns
        let raw = r#"{"type":"assistant","message":{"role":"assistant","content":"Test","usage":{"input_tokens":100,"output_tokens":50,"cache_creation_input_tokens":24337,"cache_read_input_tokens":0,"cache_creation":{"ephemeral_5m_input_tokens":24337,"ephemeral_1h_input_tokens":0}}},"session_id":"s1","uuid":"u1"}"#;
        let result = parse_entry(raw, 1);

        assert!(
            result.is_ok(),
            "Should parse entry with nested cache_creation structure"
        );
        let entry = result.unwrap();
        let usage = entry.message().usage().expect("Should have usage");

        // Verify flat fields still work
        assert_eq!(usage.input_tokens, 100);
        assert_eq!(usage.output_tokens, 50);
        assert_eq!(usage.cache_creation_input_tokens, 24337);
        assert_eq!(usage.cache_read_input_tokens, 0);

        // Verify ephemeral breakdown extracted from nested object
        assert_eq!(
            usage.ephemeral_5m_input_tokens, 24337,
            "Should extract ephemeral_5m_input_tokens from cache_creation"
        );
        assert_eq!(
            usage.ephemeral_1h_input_tokens, 0,
            "Should extract ephemeral_1h_input_tokens from cache_creation"
        );
    }

    #[test]
    fn parse_entry_with_missing_cache_creation_defaults_to_zero() {
        // Entry without cache_creation nested object should default ephemeral fields to 0
        let raw = r#"{"type":"assistant","message":{"role":"assistant","content":"Test","usage":{"input_tokens":100,"output_tokens":50,"cache_creation_input_tokens":20,"cache_read_input_tokens":10}},"session_id":"s1","uuid":"u1"}"#;
        let result = parse_entry(raw, 1);

        assert!(result.is_ok(), "Should parse entry without cache_creation");
        let entry = result.unwrap();
        let usage = entry.message().usage().expect("Should have usage");

        // Flat fields should still parse
        assert_eq!(usage.input_tokens, 100);
        assert_eq!(usage.cache_creation_input_tokens, 20);

        // Ephemeral fields should default to 0 when cache_creation is missing
        assert_eq!(
            usage.ephemeral_5m_input_tokens, 0,
            "Should default to 0 when cache_creation is missing"
        );
        assert_eq!(
            usage.ephemeral_1h_input_tokens, 0,
            "Should default to 0 when cache_creation is missing"
        );
    }

    #[test]
    fn parse_entry_with_both_ephemeral_fields() {
        // Entry with both 5m and 1h ephemeral tokens
        let raw = r#"{"type":"assistant","message":{"role":"assistant","content":"Test","usage":{"input_tokens":50,"output_tokens":25,"cache_creation_input_tokens":10000,"cache_read_input_tokens":5000,"cache_creation":{"ephemeral_5m_input_tokens":6000,"ephemeral_1h_input_tokens":4000}}},"session_id":"s1","uuid":"u1"}"#;
        let result = parse_entry(raw, 1);

        assert!(
            result.is_ok(),
            "Should parse entry with both ephemeral fields"
        );
        let entry = result.unwrap();
        let usage = entry.message().usage().expect("Should have usage");

        assert_eq!(usage.ephemeral_5m_input_tokens, 6000);
        assert_eq!(usage.ephemeral_1h_input_tokens, 4000);
    }

    // ===== Bug Fix: cclv-07v.11.1 - session_id snake_case format =====

    #[test]
    fn parse_entry_session_id_snake_case() {
        // RED TEST: Actual Claude Code format uses snake_case session_id
        // This test expects snake_case and will FAIL until we remove camelCase rename
        let raw = r#"{"type":"user","message":{"role":"user","content":"hello"},"uuid":"abc-123","session_id":"snake-case-session"}"#;

        let result = parse_entry(raw, 1);
        assert!(
            result.is_ok(),
            "Should parse session_id in snake_case format (actual Claude Code format)"
        );
        let entry = result.unwrap();
        assert_eq!(
            entry.session_id().as_str(),
            "snake-case-session",
            "Should accept snake_case session_id field"
        );
    }

    // ===== Bug Fix: cclv-07v.11.2 - Optional timestamp field =====

    #[test]
    fn parse_entry_without_timestamp_succeeds() {
        // RED TEST: Actual Claude Code entries do NOT have timestamp field
        // Parser should accept entries without timestamp and use entry order for sequencing
        let raw = r#"{"type":"user","message":{"role":"user","content":"hello"},"uuid":"abc-123","session_id":"test-session"}"#;

        let result = parse_entry(raw, 1);
        assert!(
            result.is_ok(),
            "Should parse entry without timestamp field (actual Claude Code format)"
        );
        let entry = result.unwrap();
        assert_eq!(entry.uuid().as_str(), "abc-123");
        // Timestamp should exist (with some fallback value) even though field was missing
        let _timestamp = entry.timestamp(); // Should not panic
    }

    #[test]
    fn parse_entry_with_null_timestamp_succeeds() {
        // Entries with explicit null timestamp should also work
        let raw = r#"{"type":"user","message":{"role":"user","content":"hello"},"uuid":"abc-456","session_id":"test-session","timestamp":null}"#;

        let result = parse_entry(raw, 1);
        assert!(
            result.is_ok(),
            "Should parse entry with null timestamp field"
        );
        let entry = result.unwrap();
        assert_eq!(entry.uuid().as_str(), "abc-456");
    }

    // ===== Bug Fix: FMT-006 - SystemMetadata for system entries =====

    #[test]
    fn parse_entry_system_init_with_metadata() {
        // RED TEST: Parse system:init entry with all SystemMetadata fields
        let raw = r#"{"type":"system","subtype":"init","message":{"role":"assistant","content":"Initialized"},"uuid":"sys-001","session_id":"test-session","cwd":"/home/user","model":"claude-opus-4-5-20251101","tools":["Read","Write","Bash"],"agents":["general-purpose"],"skills":["commit","test-driven-development"]}"#;

        let result = parse_entry(raw, 1);
        assert!(
            result.is_ok(),
            "Should parse system:init entry with metadata"
        );
        let entry = result.unwrap();
        assert_eq!(entry.entry_type(), EntryType::System);

        let sys_meta = entry
            .system_metadata()
            .expect("System entry should have system_metadata");
        assert_eq!(sys_meta.subtype, "init");
        assert_eq!(sys_meta.cwd, Some(std::path::PathBuf::from("/home/user")));
        assert_eq!(sys_meta.model, Some("claude-opus-4-5-20251101".to_string()));
        assert_eq!(sys_meta.tools.len(), 3);
        assert!(sys_meta.tools.contains(&"Read".to_string()));
        assert!(sys_meta.tools.contains(&"Write".to_string()));
        assert!(sys_meta.tools.contains(&"Bash".to_string()));
        assert_eq!(sys_meta.agents.len(), 1);
        assert_eq!(sys_meta.agents[0], "general-purpose");
        assert_eq!(sys_meta.skills.len(), 2);
        assert!(sys_meta.skills.contains(&"commit".to_string()));
        assert!(
            sys_meta
                .skills
                .contains(&"test-driven-development".to_string())
        );
    }

    #[test]
    fn parse_entry_system_hook_response() {
        // System:hook_response entry (minimal metadata)
        let raw = r#"{"type":"system","subtype":"hook_response","message":{"role":"assistant","content":"Hook executed"},"uuid":"sys-002","session_id":"test-session"}"#;

        let result = parse_entry(raw, 1);
        assert!(result.is_ok(), "Should parse system:hook_response entry");
        let entry = result.unwrap();
        assert_eq!(entry.entry_type(), EntryType::System);

        let sys_meta = entry
            .system_metadata()
            .expect("System entry should have system_metadata");
        assert_eq!(sys_meta.subtype, "hook_response");
        assert!(sys_meta.cwd.is_none());
        assert!(sys_meta.model.is_none());
        assert!(sys_meta.tools.is_empty());
        assert!(sys_meta.agents.is_empty());
        assert!(sys_meta.skills.is_empty());
    }

    #[test]
    fn parse_entry_non_system_has_no_system_metadata() {
        // User entry should not have system_metadata even if fields are present
        let raw = r#"{"type":"user","message":{"role":"user","content":"hello"},"uuid":"u-001","session_id":"test-session"}"#;

        let result = parse_entry(raw, 1);
        assert!(result.is_ok(), "Should parse user entry");
        let entry = result.unwrap();
        assert_eq!(entry.entry_type(), EntryType::User);
        assert!(
            entry.system_metadata().is_none(),
            "Non-system entry should not have system_metadata"
        );
    }

    #[test]
    fn parse_entry_system_with_empty_tools_agents_skills() {
        // System entry with explicit empty arrays
        let raw = r#"{"type":"system","subtype":"init","message":{"role":"assistant","content":"Init"},"uuid":"sys-003","session_id":"test-session","tools":[],"agents":[],"skills":[]}"#;

        let result = parse_entry(raw, 1);
        assert!(
            result.is_ok(),
            "Should parse system entry with empty arrays"
        );
        let entry = result.unwrap();

        let sys_meta = entry
            .system_metadata()
            .expect("Should have system_metadata");
        assert!(sys_meta.tools.is_empty(), "tools should be empty");
        assert!(sys_meta.agents.is_empty(), "agents should be empty");
        assert!(sys_meta.skills.is_empty(), "skills should be empty");
    }

    #[test]
    fn parse_entry_real_system_init_from_claude_code() {
        // Integration test: Real system:init entry from actual Claude Code JSONL output
        // This tests the complete format as seen in production logs
        let raw = r#"{"type":"system","subtype":"init","session_id":"e9bc0c98-6abc-4fe7-abc7-123456789abc","uuid":"38df9820-95f4-4a8f-ba30-abcdef123456","cwd":"/home/claude/cclv","model":"claude-opus-4-5-20251101","tools":["Task","Read","Write","Edit","Bash","Grep","Glob"],"agents":["general-purpose"],"skills":["commit","test-driven-development","typed-domain-modeling"]}"#;

        let result = parse_entry(raw, 1);
        assert!(
            result.is_ok(),
            "Should parse real Claude Code system:init entry"
        );
        let entry = result.unwrap();

        // Verify basic entry properties
        assert_eq!(entry.entry_type(), EntryType::System);
        assert_eq!(
            entry.session_id().as_str(),
            "e9bc0c98-6abc-4fe7-abc7-123456789abc"
        );
        assert_eq!(
            entry.uuid().as_str(),
            "38df9820-95f4-4a8f-ba30-abcdef123456"
        );

        // Verify system metadata
        let sys_meta = entry
            .system_metadata()
            .expect("Real system:init should have system_metadata");

        assert_eq!(sys_meta.subtype, "init");
        assert_eq!(
            sys_meta.cwd,
            Some(std::path::PathBuf::from("/home/claude/cclv"))
        );
        assert_eq!(sys_meta.model, Some("claude-opus-4-5-20251101".to_string()));

        // Verify tools array
        assert_eq!(sys_meta.tools.len(), 7);
        let expected_tools = vec!["Task", "Read", "Write", "Edit", "Bash", "Grep", "Glob"];
        for tool in &expected_tools {
            assert!(
                sys_meta.tools.contains(&tool.to_string()),
                "Should contain tool: {}",
                tool
            );
        }

        // Verify agents array
        assert_eq!(sys_meta.agents.len(), 1);
        assert_eq!(sys_meta.agents[0], "general-purpose");

        // Verify skills array
        assert_eq!(sys_meta.skills.len(), 3);
        let expected_skills = vec!["commit", "test-driven-development", "typed-domain-modeling"];
        for skill in &expected_skills {
            assert!(
                sys_meta.skills.contains(&skill.to_string()),
                "Should contain skill: {}",
                skill
            );
        }
    }

    // ===== Bug Fix: FMT-007 - ResultMetadata for result entries =====

    #[test]
    fn parse_entry_result_with_metadata() {
        // RED TEST: Parse type:result entry with all ResultMetadata fields
        let raw = r#"{"type":"result","subtype":"success","is_error":false,"duration_ms":306681,"num_turns":36,"result":"Session complete","session_id":"test-session","total_cost_usd":1.3874568,"uuid":"res-001"}"#;

        let result = parse_entry(raw, 1);
        assert!(result.is_ok(), "Should parse result entry with metadata");
        let entry = result.unwrap();
        assert_eq!(entry.entry_type(), EntryType::Result);

        let res_meta = entry
            .result_metadata()
            .expect("Result entry should have result_metadata");
        assert!(!res_meta.is_error);
        assert_eq!(res_meta.duration_ms, 306681);
        assert_eq!(res_meta.num_turns, 36);
        assert_eq!(res_meta.total_cost_usd, 1.3874568);
        assert_eq!(res_meta.result_text, "Session complete");
    }

    #[test]
    fn parse_entry_result_with_error_status() {
        // Result entry with is_error=true
        let raw = r#"{"type":"result","subtype":"error","is_error":true,"duration_ms":12345,"num_turns":5,"result":"Error occurred","session_id":"test-session","total_cost_usd":0.05,"uuid":"res-002"}"#;

        let result = parse_entry(raw, 1);
        assert!(result.is_ok(), "Should parse error result entry");
        let entry = result.unwrap();

        let res_meta = entry
            .result_metadata()
            .expect("Error result should have result_metadata");
        assert!(res_meta.is_error, "is_error should be true");
        assert_eq!(res_meta.duration_ms, 12345);
        assert_eq!(res_meta.num_turns, 5);
        assert_eq!(res_meta.total_cost_usd, 0.05);
        assert_eq!(res_meta.result_text, "Error occurred");
    }

    #[test]
    fn parse_entry_non_result_has_no_result_metadata() {
        // User entry should not have result_metadata
        let raw = r#"{"type":"user","message":{"role":"user","content":"hello"},"uuid":"u-001","session_id":"test-session"}"#;

        let result = parse_entry(raw, 1);
        assert!(result.is_ok(), "Should parse user entry");
        let entry = result.unwrap();
        assert_eq!(entry.entry_type(), EntryType::User);
        assert!(
            entry.result_metadata().is_none(),
            "Non-result entry should not have result_metadata"
        );
    }

    #[test]
    fn parse_entry_real_result_from_claude_code() {
        // Integration test: Real type:result entry from actual Claude Code JSONL output
        // This tests the complete format as seen in production logs
        let raw = r#"{"type":"result","subtype":"success","is_error":false,"duration_ms":306681,"duration_api_ms":336809,"num_turns":36,"result":"---\n\n## Session Complete ✓\n\n**Bead Completed**: `cclv-07v.1.3` - Initialize Cargo.toml with project metadata\n\n**Summary**:\n- Updated Cargo.toml with rust-version = \"1.83\"\n- Added 7 core dependencies: ratatui, crossterm, serde, serde_json, clap, thiserror, chrono\n- Added 2 dev-dependencies: proptest, insta\n- cargo check passes successfully\n- Review: PASS\n\n**Commit**: `f8b5dd1 chore(deps): add core dependencies for TUI implementation`\n\n**Now Unblocked**: `cclv-07v.1.4` (Create minimal src/main.rs and src/lib.rs)\n\n**STOPPING** as instructed - wrapper script handles next iteration.","session_id":"c297ee1e-885f-4261-9046-1e5fb4628313","total_cost_usd":1.3874568,"uuid":"9cafe6c3-d683-4670-bda2-8bed22efbe84"}"#;

        let result = parse_entry(raw, 1);
        assert!(result.is_ok(), "Should parse real Claude Code result entry");
        let entry = result.unwrap();

        // Verify basic entry properties
        assert_eq!(entry.entry_type(), EntryType::Result);
        assert_eq!(
            entry.session_id().as_str(),
            "c297ee1e-885f-4261-9046-1e5fb4628313"
        );
        assert_eq!(
            entry.uuid().as_str(),
            "9cafe6c3-d683-4670-bda2-8bed22efbe84"
        );

        // Verify result metadata
        let res_meta = entry
            .result_metadata()
            .expect("Real result entry should have result_metadata");

        assert!(!res_meta.is_error, "Success result should not be error");
        assert_eq!(res_meta.duration_ms, 306681);
        assert_eq!(res_meta.num_turns, 36);
        assert_eq!(res_meta.total_cost_usd, 1.3874568);

        // Verify result text contains expected content
        assert!(
            res_meta.result_text.contains("Session Complete"),
            "Result text should contain 'Session Complete'"
        );
        assert!(
            res_meta.result_text.contains("cclv-07v.1.3"),
            "Result text should contain bead ID"
        );
    }
}
