//! Tests for SearchState and SearchQuery.

use super::*;
use crate::model::{
    AgentId, ContentBlock, EntryMetadata, EntryType, EntryUuid, LogEntry, Message, MessageContent,
    Role, SessionId,
};
use crate::state::AppState;
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
    let mut entries = Vec::new();
    entries.push(crate::model::ConversationEntry::Valid(Box::new(make_text_entry("entry-1", None, "This is an error message"))));

    let mut state = AppState::new();
    state.add_entries(entries);

    let query = SearchQuery::new("error").expect("valid query");
    let matches = execute_search(state.session_view(), &query);

    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].agent_id, None);
    assert_eq!(matches[0].entry_uuid.as_str(), "entry-1");
    assert_eq!(matches[0].block_index, 0);
    assert_eq!(matches[0].char_offset, 11);
    assert_eq!(matches[0].length, 5);
}

#[test]
fn execute_search_is_case_insensitive() {
    let mut entries = Vec::new();
    entries.push(crate::model::ConversationEntry::Valid(Box::new(make_text_entry("entry-1", None, "ERROR in uppercase"))));

    let mut state = AppState::new();
    state.add_entries(entries);

    let query = SearchQuery::new("error").expect("valid query");
    let matches = execute_search(state.session_view(), &query);

    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].char_offset, 0);
}

#[test]
fn execute_search_finds_multiple_matches_in_single_entry() {
    let mut entries = Vec::new();
    entries.push(crate::model::ConversationEntry::Valid(Box::new(make_text_entry(
        "entry-1",
        None,
        "error at start and error at end",
    ))));

    let mut state = AppState::new();
    state.add_entries(entries);

    let query = SearchQuery::new("error").expect("valid query");
    let matches = execute_search(state.session_view(), &query);

    assert_eq!(matches.len(), 2);
    assert_eq!(matches[0].char_offset, 0);
    assert_eq!(matches[1].char_offset, 19);
}

#[test]
fn execute_search_finds_matches_across_multiple_entries() {
    let mut entries = Vec::new();
    entries.push(crate::model::ConversationEntry::Valid(Box::new(make_text_entry("entry-1", None, "first error"))));
    entries.push(crate::model::ConversationEntry::Valid(Box::new(make_text_entry("entry-2", None, "second error"))));

    let mut state = AppState::new();
    state.add_entries(entries);

    let query = SearchQuery::new("error").expect("valid query");
    let matches = execute_search(state.session_view(), &query);

    assert_eq!(matches.len(), 2);
    assert_eq!(matches[0].entry_uuid.as_str(), "entry-1");
    assert_eq!(matches[1].entry_uuid.as_str(), "entry-2");
}

#[test]
fn execute_search_finds_match_in_subagent() {
    let mut entries = Vec::new();
    let agent_id = make_agent_id("agent-123");
    entries.push(crate::model::ConversationEntry::Valid(Box::new(make_text_entry(
        "entry-1",
        Some(agent_id.clone()),
        "subagent error",
    ))));

    let mut state = AppState::new();
    state.add_entries(entries);

    let query = SearchQuery::new("error").expect("valid query");
    let matches = execute_search(state.session_view(), &query);

    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].agent_id, Some(agent_id));
    assert_eq!(matches[0].entry_uuid.as_str(), "entry-1");
}

#[test]
fn execute_search_finds_matches_in_main_and_subagent() {
    let mut entries = Vec::new();
    let agent_id = make_agent_id("agent-abc");

    entries.push(crate::model::ConversationEntry::Valid(Box::new(make_text_entry("entry-1", None, "main error"))));
    entries.push(crate::model::ConversationEntry::Valid(Box::new(make_text_entry(
        "entry-2",
        Some(agent_id.clone()),
        "sub error",
    ))));

    let mut state = AppState::new();
    state.add_entries(entries);

    let query = SearchQuery::new("error").expect("valid query");
    let matches = execute_search(state.session_view(), &query);

    assert_eq!(matches.len(), 2);
    assert_eq!(matches[0].agent_id, None);
    assert_eq!(matches[1].agent_id, Some(agent_id));
}

#[test]
fn execute_search_searches_all_text_blocks_in_blocks_content() {
    let mut entries = Vec::new();
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
    entries.push(crate::model::ConversationEntry::Valid(Box::new(make_blocks_entry("entry-1", None, blocks))));

    let mut state = AppState::new();
    state.add_entries(entries);

    let query = SearchQuery::new("error").expect("valid query");
    let matches = execute_search(state.session_view(), &query);

    assert_eq!(matches.len(), 2);
    assert_eq!(matches[0].block_index, 0);
    assert_eq!(matches[0].char_offset, 6);
    assert_eq!(matches[1].block_index, 2);
    assert_eq!(matches[1].char_offset, 7);
}

#[test]
fn execute_search_returns_empty_when_no_matches() {
    let mut entries = Vec::new();
    entries.push(crate::model::ConversationEntry::Valid(Box::new(make_text_entry("entry-1", None, "no matching text"))));

    let mut state = AppState::new();
    state.add_entries(entries);

    let query = SearchQuery::new("error").expect("valid query");
    let matches = execute_search(state.session_view(), &query);

    assert_eq!(matches.len(), 0);
}

#[test]
fn execute_search_returns_empty_for_empty_session() {
    let mut state = AppState::new();
    // Add a single entry with no searchable content (empty text)
    let mut entries = Vec::new();
    entries.push(crate::model::ConversationEntry::Valid(Box::new(make_text_entry("entry-1", None, ""))));
    state.add_entries(entries);

    let query = SearchQuery::new("error").expect("valid query");
    let matches = execute_search(state.session_view(), &query);

    assert_eq!(matches.len(), 0);
}

#[test]
fn execute_search_handles_overlapping_matches() {
    let mut entries = Vec::new();
    entries.push(crate::model::ConversationEntry::Valid(Box::new(make_text_entry("entry-1", None, "aaa"))));

    let mut state = AppState::new();
    state.add_entries(entries);

    let query = SearchQuery::new("aa").expect("valid query");
    let matches = execute_search(state.session_view(), &query);

    // Should find "aa" at position 0 and position 1 (overlapping)
    assert_eq!(matches.len(), 2);
    assert_eq!(matches[0].char_offset, 0);
    assert_eq!(matches[1].char_offset, 1);
}

#[test]
fn execute_search_stores_correct_match_length() {
    let mut entries = Vec::new();
    entries.push(crate::model::ConversationEntry::Valid(Box::new(make_text_entry("entry-1", None, "find this pattern"))));

    let mut state = AppState::new();
    state.add_entries(entries);

    let query = SearchQuery::new("pattern").expect("valid query");
    let matches = execute_search(state.session_view(), &query);

    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].length, 7); // "pattern" is 7 chars
}

#[test]
fn execute_search_searches_thinking_blocks() {
    let mut entries = Vec::new();
    let blocks = vec![ContentBlock::Thinking {
        thinking: "I'm thinking about the error".to_string(),
    }];
    entries.push(crate::model::ConversationEntry::Valid(Box::new(make_blocks_entry("entry-1", None, blocks))));

    let mut state = AppState::new();
    state.add_entries(entries);

    let query = SearchQuery::new("error").expect("valid query");
    let matches = execute_search(state.session_view(), &query);

    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].block_index, 0);
}

#[test]
fn execute_search_searches_tool_result_blocks() {
    use crate::model::ToolUseId;

    let mut entries = Vec::new();
    let blocks = vec![ContentBlock::ToolResult {
        tool_use_id: ToolUseId::new("tool-1").expect("valid id"),
        content: "command failed with error".to_string(),
        is_error: true,
    }];
    entries.push(crate::model::ConversationEntry::Valid(Box::new(make_blocks_entry("entry-1", None, blocks))));

    let mut state = AppState::new();
    state.add_entries(entries);

    let query = SearchQuery::new("error").expect("valid query");
    let matches = execute_search(state.session_view(), &query);

    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].block_index, 0);
}

// ===== agent_ids_with_matches Tests =====

#[test]
fn agent_ids_with_matches_returns_empty_for_empty_matches() {
    let matches: Vec<SearchMatch> = vec![];
    let agent_ids = agent_ids_with_matches(&matches);

    assert!(
        agent_ids.is_empty(),
        "Should return empty set for no matches"
    );
}

#[test]
fn agent_ids_with_matches_ignores_main_agent_matches() {
    let matches = vec![
        SearchMatch {
            agent_id: None, // Main agent
            entry_uuid: make_entry_uuid("entry-1"),
            block_index: 0,
            char_offset: 0,
            length: 5,
        },
        SearchMatch {
            agent_id: None, // Main agent
            entry_uuid: make_entry_uuid("entry-2"),
            block_index: 0,
            char_offset: 10,
            length: 5,
        },
    ];

    let agent_ids = agent_ids_with_matches(&matches);

    assert!(
        agent_ids.is_empty(),
        "Should ignore main agent matches (agent_id = None)"
    );
}

#[test]
fn agent_ids_with_matches_returns_single_agent() {
    let agent = make_agent_id("agent-123");
    let matches = vec![SearchMatch {
        agent_id: Some(agent.clone()),
        entry_uuid: make_entry_uuid("entry-1"),
        block_index: 0,
        char_offset: 0,
        length: 5,
    }];

    let agent_ids = agent_ids_with_matches(&matches);

    assert_eq!(agent_ids.len(), 1, "Should contain exactly one agent");
    assert!(agent_ids.contains(&agent), "Should contain agent-123");
}

#[test]
fn agent_ids_with_matches_deduplicates_same_agent() {
    let agent = make_agent_id("agent-abc");
    let matches = vec![
        SearchMatch {
            agent_id: Some(agent.clone()),
            entry_uuid: make_entry_uuid("entry-1"),
            block_index: 0,
            char_offset: 0,
            length: 5,
        },
        SearchMatch {
            agent_id: Some(agent.clone()),
            entry_uuid: make_entry_uuid("entry-2"),
            block_index: 0,
            char_offset: 10,
            length: 5,
        },
        SearchMatch {
            agent_id: Some(agent.clone()),
            entry_uuid: make_entry_uuid("entry-3"),
            block_index: 1,
            char_offset: 20,
            length: 5,
        },
    ];

    let agent_ids = agent_ids_with_matches(&matches);

    assert_eq!(agent_ids.len(), 1, "Should deduplicate to single agent");
    assert!(agent_ids.contains(&agent), "Should contain agent-abc");
}

#[test]
fn agent_ids_with_matches_returns_multiple_agents() {
    let agent1 = make_agent_id("agent-1");
    let agent2 = make_agent_id("agent-2");
    let agent3 = make_agent_id("agent-3");

    let matches = vec![
        SearchMatch {
            agent_id: Some(agent1.clone()),
            entry_uuid: make_entry_uuid("entry-1"),
            block_index: 0,
            char_offset: 0,
            length: 5,
        },
        SearchMatch {
            agent_id: Some(agent2.clone()),
            entry_uuid: make_entry_uuid("entry-2"),
            block_index: 0,
            char_offset: 0,
            length: 5,
        },
        SearchMatch {
            agent_id: Some(agent3.clone()),
            entry_uuid: make_entry_uuid("entry-3"),
            block_index: 0,
            char_offset: 0,
            length: 5,
        },
    ];

    let agent_ids = agent_ids_with_matches(&matches);

    assert_eq!(agent_ids.len(), 3, "Should contain all three agents");
    assert!(agent_ids.contains(&agent1), "Should contain agent-1");
    assert!(agent_ids.contains(&agent2), "Should contain agent-2");
    assert!(agent_ids.contains(&agent3), "Should contain agent-3");
}

#[test]
fn agent_ids_with_matches_mixed_main_and_subagent_matches() {
    let agent1 = make_agent_id("agent-sub1");
    let agent2 = make_agent_id("agent-sub2");

    let matches = vec![
        SearchMatch {
            agent_id: None, // Main agent - should be ignored
            entry_uuid: make_entry_uuid("entry-1"),
            block_index: 0,
            char_offset: 0,
            length: 5,
        },
        SearchMatch {
            agent_id: Some(agent1.clone()),
            entry_uuid: make_entry_uuid("entry-2"),
            block_index: 0,
            char_offset: 0,
            length: 5,
        },
        SearchMatch {
            agent_id: None, // Main agent - should be ignored
            entry_uuid: make_entry_uuid("entry-3"),
            block_index: 0,
            char_offset: 10,
            length: 5,
        },
        SearchMatch {
            agent_id: Some(agent2.clone()),
            entry_uuid: make_entry_uuid("entry-4"),
            block_index: 0,
            char_offset: 0,
            length: 5,
        },
    ];

    let agent_ids = agent_ids_with_matches(&matches);

    assert_eq!(agent_ids.len(), 2, "Should contain only subagents");
    assert!(agent_ids.contains(&agent1), "Should contain agent-sub1");
    assert!(agent_ids.contains(&agent2), "Should contain agent-sub2");
}

// ===== Unicode/Emoji Tests =====
// These tests verify that char_offset and length fields use BYTE offsets (not character offsets).
// This is correct for Rust string slicing which is byte-indexed.

#[test]
fn execute_search_handles_emoji_in_content_before_match() {
    // Content: "🦀 error" - emoji is 4 bytes, then space (1 byte), then "error" at byte 5
    let mut entries = Vec::new();
    entries.push(crate::model::ConversationEntry::Valid(Box::new(make_text_entry("entry-1", None, "🦀 error"))));

    let mut state = AppState::new();
    state.add_entries(entries);

    let query = SearchQuery::new("error").expect("valid query");
    let matches = execute_search(state.session_view(), &query);

    assert_eq!(matches.len(), 1);
    // char_offset should be 5 (byte offset after "🦀 ")
    // If it were character offset, it would be 2 (crab + space)
    assert_eq!(
        matches[0].char_offset, 5,
        "Should use byte offset, not char offset"
    );
    assert_eq!(matches[0].length, 5, "Length of 'error' in bytes");
}

#[test]
fn execute_search_finds_emoji_in_content() {
    // Search for emoji within content
    let mut entries = Vec::new();
    entries.push(crate::model::ConversationEntry::Valid(Box::new(make_text_entry("entry-1", None, "Rust 🦀 rocks"))));

    let mut state = AppState::new();
    state.add_entries(entries);

    let query = SearchQuery::new("🦀").expect("valid query");
    let matches = execute_search(state.session_view(), &query);

    assert_eq!(matches.len(), 1);
    // Emoji starts at byte 5 (after "Rust ")
    assert_eq!(matches[0].char_offset, 5, "Emoji at byte offset 5");
    // Crab emoji is 4 bytes
    assert_eq!(matches[0].length, 4, "Crab emoji is 4 bytes");
}

#[test]
fn execute_search_handles_multibyte_unicode_characters() {
    // Japanese characters (3 bytes each in UTF-8)
    let mut entries = Vec::new();
    entries.push(crate::model::ConversationEntry::Valid(Box::new(make_text_entry("entry-1", None, "Hello 日本語 world"))));

    let mut state = AppState::new();
    state.add_entries(entries);

    let query = SearchQuery::new("world").expect("valid query");
    let matches = execute_search(state.session_view(), &query);

    assert_eq!(matches.len(), 1);
    // "Hello " = 6 bytes, "日本語" = 9 bytes (3 chars × 3 bytes), " " = 1 byte
    // "world" starts at byte 16
    assert_eq!(matches[0].char_offset, 16, "Should use byte offset");
    assert_eq!(matches[0].length, 5);
}

#[test]
fn execute_search_finds_japanese_text() {
    let mut entries = Vec::new();
    entries.push(crate::model::ConversationEntry::Valid(Box::new(make_text_entry(
        "entry-1",
        None,
        "Searching for 日本語 here",
    ))));

    let mut state = AppState::new();
    state.add_entries(entries);

    let query = SearchQuery::new("日本語").expect("valid query");
    let matches = execute_search(state.session_view(), &query);

    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].char_offset, 14, "Japanese at byte 14");
    assert_eq!(matches[0].length, 9, "日本語 is 9 bytes");
}

#[test]
fn execute_search_multiple_emojis_in_text() {
    let mut entries = Vec::new();
    entries.push(crate::model::ConversationEntry::Valid(Box::new(make_text_entry("entry-1", None, "🔥🦀🚀 test 🎉"))));

    let mut state = AppState::new();
    state.add_entries(entries);

    let query = SearchQuery::new("test").expect("valid query");
    let matches = execute_search(state.session_view(), &query);

    assert_eq!(matches.len(), 1);
    // 🔥 = 4 bytes, 🦀 = 4 bytes, 🚀 = 4 bytes, space = 1 byte
    assert_eq!(matches[0].char_offset, 13, "After 3 emojis and space");
    assert_eq!(matches[0].length, 4);
}

#[test]
fn execute_search_emoji_case_insensitive_ascii_only() {
    // Case insensitivity should work for ASCII parts, emoji stays as-is
    let mut entries = Vec::new();
    entries.push(crate::model::ConversationEntry::Valid(Box::new(make_text_entry("entry-1", None, "ERROR 🔥 here"))));

    let mut state = AppState::new();
    state.add_entries(entries);

    let query = SearchQuery::new("error").expect("valid query");
    let matches = execute_search(state.session_view(), &query);

    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].char_offset, 0);
    assert_eq!(matches[0].length, 5);
}

#[test]
fn execute_search_overlapping_matches_with_unicode() {
    // "ää" where ä is 2 bytes each in UTF-8
    let mut entries = Vec::new();
    entries.push(crate::model::ConversationEntry::Valid(Box::new(make_text_entry("entry-1", None, "ääää"))));

    let mut state = AppState::new();
    state.add_entries(entries);

    let query = SearchQuery::new("ää").expect("valid query");
    let matches = execute_search(state.session_view(), &query);

    // Should find overlapping matches at byte positions
    assert_eq!(matches.len(), 3);
    assert_eq!(matches[0].char_offset, 0); // First ää
    assert_eq!(matches[0].length, 4); // 2 chars × 2 bytes
    assert_eq!(matches[1].char_offset, 2); // Second overlapping match
    assert_eq!(matches[2].char_offset, 4); // Third overlapping match
}

#[test]
fn execute_search_unicode_at_match_boundary() {
    // Emoji right at the end of a match
    let mut entries = Vec::new();
    entries.push(crate::model::ConversationEntry::Valid(Box::new(make_text_entry("entry-1", None, "test🦀 more test🦀"))));

    let mut state = AppState::new();
    state.add_entries(entries);

    let query = SearchQuery::new("test").expect("valid query");
    let matches = execute_search(state.session_view(), &query);

    assert_eq!(matches.len(), 2);
    assert_eq!(matches[0].char_offset, 0);
    assert_eq!(matches[0].length, 4);
    // Second "test" is at: "test"(4) + "🦀"(4) + " more "(6) = byte 14
    assert_eq!(matches[1].char_offset, 14);
}

#[test]
fn execute_search_stores_correct_match_length_for_unicode_query() {
    let mut entries = Vec::new();
    entries.push(crate::model::ConversationEntry::Valid(Box::new(make_text_entry("entry-1", None, "Find the 🚀 emoji"))));

    let mut state = AppState::new();
    state.add_entries(entries);

    let query = SearchQuery::new("🚀").expect("valid query");
    let matches = execute_search(state.session_view(), &query);

    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].length, 4, "Rocket emoji is 4 bytes, not 1 char");
}
