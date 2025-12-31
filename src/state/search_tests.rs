//! Tests for SearchState and SearchQuery.

use super::*;

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
