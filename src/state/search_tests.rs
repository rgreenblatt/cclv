//! Tests for SearchState and SearchQuery.

use super::*;
use crate::model::{
    EntryMetadata, EntryType, EntryUuid, LogEntry, Message, MessageContent, Role, Session,
    SessionId, ContentBlock,
};
use chrono::Utc;

// ===== SearchQuery::new Tests =====

#[test]
fn search_query_new_accepts_non_empty_string() {
    let query = SearchQuery::new("test");

    assert!(query.is_some());
}

#[test]
fn search_query_new_rejects_empty_string() {
    let query = SearchQuery::new("");

    assert!(query.is_none());
}

#[test]
fn search_query_new_rejects_whitespace_only() {
    let query = SearchQuery::new("   ");

    assert!(query.is_none());
}

#[test]
fn search_query_new_rejects_tabs_only() {
    let query = SearchQuery::new("\t\t");

    assert!(query.is_none());
}

#[test]
fn search_query_new_accepts_string_with_leading_trailing_whitespace() {
    let query = SearchQuery::new("  test  ");

    assert!(query.is_some());
}

#[test]
fn search_query_as_str_returns_original_string() {
    let query = SearchQuery::new("search term").expect("valid query");

    assert_eq!(query.as_str(), "search term");
}

#[test]
fn search_query_preserves_whitespace() {
    let query = SearchQuery::new("  test  ").expect("valid query");

    assert_eq!(query.as_str(), "  test  ");
}

// ===== SearchState Variant Tests =====

#[test]
fn search_state_inactive_variant_exists() {
    let state = SearchState::Inactive;

    matches!(state, SearchState::Inactive);
}

#[test]
fn search_state_typing_variant_with_empty_query() {
    let state = SearchState::Typing {
        query: String::new(),
        cursor: 0,
    };

    match state {
        SearchState::Typing { query, cursor } => {
            assert_eq!(query, "");
            assert_eq!(cursor, 0);
        }
        _ => panic!("Expected Typing variant"),
    }
}

#[test]
fn search_state_typing_variant_with_cursor_position() {
    let state = SearchState::Typing {
        query: "test".to_string(),
        cursor: 2,
    };

    match state {
        SearchState::Typing { query, cursor } => {
            assert_eq!(query, "test");
            assert_eq!(cursor, 2);
        }
        _ => panic!("Expected Typing variant"),
    }
}

#[test]
fn search_state_active_variant_with_no_matches() {
    let query = SearchQuery::new("test").expect("valid query");
    let state = SearchState::Active {
        query,
        matches: vec![],
        current_match: 0,
    };

    match state {
        SearchState::Active {
            query,
            matches,
            current_match,
        } => {
            assert_eq!(query.as_str(), "test");
            assert_eq!(matches.len(), 0);
            assert_eq!(current_match, 0);
        }
        _ => panic!("Expected Active variant"),
    }
}

#[test]
fn search_state_active_variant_with_matches() {
    use crate::model::{AgentId, EntryUuid};

    let make_uuid = |s: &str| EntryUuid::new(s).expect("valid uuid");
    let make_agent_id = |s: &str| AgentId::new(s).expect("valid agent id");

    let query = SearchQuery::new("error").expect("valid query");
    let matches = vec![
        SearchMatch {
            agent_id: None,
            entry_uuid: make_uuid("entry-1"),
            block_index: 0,
            char_offset: 10,
            length: 5,
        },
        SearchMatch {
            agent_id: Some(make_agent_id("agent-1")),
            entry_uuid: make_uuid("entry-2"),
            block_index: 1,
            char_offset: 25,
            length: 5,
        },
    ];

    let state = SearchState::Active {
        query,
        matches: matches.clone(),
        current_match: 1,
    };

    match state {
        SearchState::Active {
            query,
            matches: result_matches,
            current_match,
        } => {
            assert_eq!(query.as_str(), "error");
            assert_eq!(result_matches.len(), 2);
            assert_eq!(current_match, 1);
        }
        _ => panic!("Expected Active variant"),
    }
}

// ===== Test Helpers for execute_search =====

fn make_session_id(s: &str) -> SessionId {
    SessionId::new(s).expect("valid session id")
}

fn make_entry_uuid(s: &str) -> EntryUuid {
    EntryUuid::new(s).expect("valid uuid")
}

fn make_agent_id(s: &str) -> AgentId {
    AgentId::new(s).expect("valid agent id")
}

fn make_timestamp() -> chrono::DateTime<Utc> {
    "2025-12-25T10:00:00Z".parse().expect("valid timestamp")
}

fn make_text_entry(uuid: &str, agent_id: Option<AgentId>, text: &str) -> LogEntry {
    LogEntry::new(
        make_entry_uuid(uuid),
        None,
        make_session_id("session-1"),
        agent_id,
        make_timestamp(),
        EntryType::User,
        Message::new(Role::User, MessageContent::Text(text.to_string())),
        EntryMetadata::default(),
    )
}

fn make_blocks_entry(uuid: &str, agent_id: Option<AgentId>, blocks: Vec<ContentBlock>) -> LogEntry {
    LogEntry::new(
        make_entry_uuid(uuid),
        None,
        make_session_id("session-1"),
        agent_id,
        make_timestamp(),
        EntryType::Assistant,
        Message::new(Role::Assistant, MessageContent::Blocks(blocks)),
        EntryMetadata::default(),
    )
}

// ===== execute_search Tests =====

#[test]
fn execute_search_finds_match_in_main_agent_text() {
    let mut session = Session::new(make_session_id("session-1"));
    session.add_entry(make_text_entry("entry-1", None, "This is an error message"));

    let query = SearchQuery::new("error").expect("valid query");
    let matches = execute_search(&session, &query);

    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].agent_id, None);
    assert_eq!(matches[0].entry_uuid.as_str(), "entry-1");
    assert_eq!(matches[0].block_index, 0);
    assert_eq!(matches[0].char_offset, 11);
    assert_eq!(matches[0].length, 5);
}

#[test]
fn execute_search_is_case_insensitive() {
    let mut session = Session::new(make_session_id("session-1"));
    session.add_entry(make_text_entry("entry-1", None, "ERROR in uppercase"));

    let query = SearchQuery::new("error").expect("valid query");
    let matches = execute_search(&session, &query);

    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].char_offset, 0);
}

#[test]
fn execute_search_finds_multiple_matches_in_single_entry() {
    let mut session = Session::new(make_session_id("session-1"));
    session.add_entry(make_text_entry(
        "entry-1",
        None,
        "error at start and error at end",
    ));

    let query = SearchQuery::new("error").expect("valid query");
    let matches = execute_search(&session, &query);

    assert_eq!(matches.len(), 2);
    assert_eq!(matches[0].char_offset, 0);
    assert_eq!(matches[1].char_offset, 20);
}

#[test]
fn execute_search_finds_matches_across_multiple_entries() {
    let mut session = Session::new(make_session_id("session-1"));
    session.add_entry(make_text_entry("entry-1", None, "first error"));
    session.add_entry(make_text_entry("entry-2", None, "second error"));

    let query = SearchQuery::new("error").expect("valid query");
    let matches = execute_search(&session, &query);

    assert_eq!(matches.len(), 2);
    assert_eq!(matches[0].entry_uuid.as_str(), "entry-1");
    assert_eq!(matches[1].entry_uuid.as_str(), "entry-2");
}

#[test]
fn execute_search_finds_match_in_subagent() {
    let mut session = Session::new(make_session_id("session-1"));
    let agent_id = make_agent_id("agent-123");
    session.add_entry(make_text_entry("entry-1", Some(agent_id.clone()), "subagent error"));

    let query = SearchQuery::new("error").expect("valid query");
    let matches = execute_search(&session, &query);

    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].agent_id, Some(agent_id));
    assert_eq!(matches[0].entry_uuid.as_str(), "entry-1");
}

#[test]
fn execute_search_finds_matches_in_main_and_subagent() {
    let mut session = Session::new(make_session_id("session-1"));
    let agent_id = make_agent_id("agent-abc");

    session.add_entry(make_text_entry("entry-1", None, "main error"));
    session.add_entry(make_text_entry("entry-2", Some(agent_id.clone()), "sub error"));

    let query = SearchQuery::new("error").expect("valid query");
    let matches = execute_search(&session, &query);

    assert_eq!(matches.len(), 2);
    assert_eq!(matches[0].agent_id, None);
    assert_eq!(matches[1].agent_id, Some(agent_id));
}

#[test]
fn execute_search_searches_all_text_blocks_in_blocks_content() {
    let mut session = Session::new(make_session_id("session-1"));
    let blocks = vec![
        ContentBlock::Text {
            text: "first error".to_string(),
        },
        ContentBlock::Thinking {
            thinking: "skip this".to_string(),
        },
        ContentBlock::Text {
            text: "second error".to_string(),
        },
    ];
    session.add_entry(make_blocks_entry("entry-1", None, blocks));

    let query = SearchQuery::new("error").expect("valid query");
    let matches = execute_search(&session, &query);

    assert_eq!(matches.len(), 2);
    assert_eq!(matches[0].block_index, 0);
    assert_eq!(matches[0].char_offset, 6);
    assert_eq!(matches[1].block_index, 2);
    assert_eq!(matches[1].char_offset, 7);
}

#[test]
fn execute_search_returns_empty_when_no_matches() {
    let mut session = Session::new(make_session_id("session-1"));
    session.add_entry(make_text_entry("entry-1", None, "no matching text"));

    let query = SearchQuery::new("error").expect("valid query");
    let matches = execute_search(&session, &query);

    assert_eq!(matches.len(), 0);
}

#[test]
fn execute_search_returns_empty_for_empty_session() {
    let session = Session::new(make_session_id("session-1"));

    let query = SearchQuery::new("error").expect("valid query");
    let matches = execute_search(&session, &query);

    assert_eq!(matches.len(), 0);
}

#[test]
fn execute_search_handles_overlapping_matches() {
    let mut session = Session::new(make_session_id("session-1"));
    session.add_entry(make_text_entry("entry-1", None, "aaa"));

    let query = SearchQuery::new("aa").expect("valid query");
    let matches = execute_search(&session, &query);

    // Should find "aa" at position 0 and position 1 (overlapping)
    assert_eq!(matches.len(), 2);
    assert_eq!(matches[0].char_offset, 0);
    assert_eq!(matches[1].char_offset, 1);
}

#[test]
fn execute_search_stores_correct_match_length() {
    let mut session = Session::new(make_session_id("session-1"));
    session.add_entry(make_text_entry("entry-1", None, "find this pattern"));

    let query = SearchQuery::new("pattern").expect("valid query");
    let matches = execute_search(&session, &query);

    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].length, 7); // "pattern" is 7 chars
}

#[test]
fn execute_search_searches_thinking_blocks() {
    let mut session = Session::new(make_session_id("session-1"));
    let blocks = vec![
        ContentBlock::Thinking {
            thinking: "I'm thinking about the error".to_string(),
        },
    ];
    session.add_entry(make_blocks_entry("entry-1", None, blocks));

    let query = SearchQuery::new("error").expect("valid query");
    let matches = execute_search(&session, &query);

    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].block_index, 0);
}

#[test]
fn execute_search_searches_tool_result_blocks() {
    use crate::model::ToolUseId;

    let mut session = Session::new(make_session_id("session-1"));
    let blocks = vec![
        ContentBlock::ToolResult {
            tool_use_id: ToolUseId::new("tool-1").expect("valid id"),
            content: "command failed with error".to_string(),
            is_error: true,
        },
    ];
    session.add_entry(make_blocks_entry("entry-1", None, blocks));

    let query = SearchQuery::new("error").expect("valid query");
    let matches = execute_search(&session, &query);

    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].block_index, 0);
}
