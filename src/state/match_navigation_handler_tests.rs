//! Tests for match navigation handler.
//!
//! All tests written BEFORE implementation (TDD).
//! Tests verify runtime behavior of match navigation.

use super::*;
use crate::model::{AgentId, EntryUuid, SessionId};
use crate::state::{ConversationSelection, FocusPane, SearchQuery, SearchState};

// ===== Test Helpers =====

fn make_session_id(s: &str) -> SessionId {
    SessionId::new(s).expect("valid session id")
}

fn make_entry_uuid(s: &str) -> EntryUuid {
    EntryUuid::new(s).expect("valid uuid")
}

fn make_agent_id(s: &str) -> AgentId {
    AgentId::new(s).expect("valid agent id")
}

fn make_search_match(agent_id: Option<AgentId>, uuid: &str) -> crate::state::SearchMatch {
    crate::state::SearchMatch {
        agent_id,
        entry_uuid: make_entry_uuid(uuid),
        block_index: 0,
        char_offset: 0,
        length: 4,
    }
}

// ===== next_match Tests =====

#[test]
fn next_match_when_inactive_does_nothing() {
    let mut state = AppState::new();
    let entries = Vec::new();
    state.add_entries(entries);
    state.search = SearchState::Inactive;
    state.focus = FocusPane::Main;

    next_match(&mut state);

    assert!(
        matches!(state.search, SearchState::Inactive),
        "Search should remain Inactive"
    );
    assert_eq!(state.focus, FocusPane::Main, "Focus should be unchanged");
}

#[test]
fn next_match_when_typing_does_nothing() {
    let mut state = AppState::new();
    let entries = Vec::new();
    state.add_entries(entries);
    state.search = SearchState::Typing {
        query: "test".to_string(),
        cursor: 2,
    };
    state.focus = FocusPane::Search;

    next_match(&mut state);

    match state.search {
        SearchState::Typing { query, cursor } => {
            assert_eq!(query, "test", "Query should be unchanged");
            assert_eq!(cursor, 2, "Cursor should be unchanged");
        }
        _ => panic!("Expected Typing state to remain unchanged"),
    }
}

#[test]
fn next_match_increments_current_match() {
    let mut state = AppState::new();
    let query = SearchQuery::new("test").expect("valid query");

    state.search = SearchState::Active {
        query,
        matches: vec![
            make_search_match(None, "uuid-1"),
            make_search_match(None, "uuid-2"),
            make_search_match(None, "uuid-3"),
        ],
        current_match: 0,
    };

    next_match(&mut state);

    match state.search {
        SearchState::Active { current_match, .. } => {
            assert_eq!(current_match, 1, "Should increment from 0 to 1");
        }
        _ => panic!("Expected Active search state"),
    }
}

#[test]
fn next_match_wraps_from_last_to_first() {
    let mut state = AppState::new();
    let query = SearchQuery::new("test").expect("valid query");

    state.search = SearchState::Active {
        query,
        matches: vec![
            make_search_match(None, "uuid-1"),
            make_search_match(None, "uuid-2"),
            make_search_match(None, "uuid-3"),
        ],
        current_match: 2, // Last match (index 2)
    };

    next_match(&mut state);

    match state.search {
        SearchState::Active { current_match, .. } => {
            assert_eq!(current_match, 0, "Should wrap from 2 to 0");
        }
        _ => panic!("Expected Active search state"),
    }
}

#[test]
fn next_match_with_single_match_stays_at_zero() {
    let mut state = AppState::new();
    let query = SearchQuery::new("test").expect("valid query");

    state.search = SearchState::Active {
        query,
        matches: vec![make_search_match(None, "uuid-1")],
        current_match: 0,
    };

    next_match(&mut state);

    match state.search {
        SearchState::Active { current_match, .. } => {
            assert_eq!(current_match, 0, "Single match should wrap to 0");
        }
        _ => panic!("Expected Active search state"),
    }
}

#[test]
fn next_match_switches_to_main_pane_when_match_in_main_agent() {
    let mut state = AppState::new();
    let query = SearchQuery::new("test").expect("valid query");

    state.search = SearchState::Active {
        query,
        matches: vec![
            make_search_match(None, "uuid-1"), // Main agent (agent_id = None)
        ],
        current_match: 0,
    };
    state.focus = FocusPane::Stats; // Start in different pane

    next_match(&mut state);

    assert_eq!(
        state.focus,
        FocusPane::Main,
        "Should switch to Main pane when match is in main agent"
    );
}

#[test]
fn next_match_switches_to_subagent_pane_when_match_in_subagent() {
    use crate::model::{EntryMetadata, EntryType, LogEntry, Message, MessageContent, Role};
    use chrono::Utc;

    let mut entries = Vec::new();
    let agent_id = make_agent_id("agent-123");

    // Add an entry to create the subagent
    let timestamp: chrono::DateTime<Utc> = "2025-12-25T10:00:00Z".parse().unwrap();
    let entry = LogEntry::new(
        make_entry_uuid("entry-1"),
        None,
        make_session_id("test-session"),
        Some(agent_id.clone()),
        timestamp,
        EntryType::User,
        Message::new(Role::User, MessageContent::Text("test".to_string())),
        EntryMetadata::default(),
    );
    entries.push(crate::model::ConversationEntry::Valid(Box::new(entry)));

    let mut state = AppState::new();
    state.add_entries(entries);
    let query = SearchQuery::new("test").expect("valid query");

    state.search = SearchState::Active {
        query,
        matches: vec![
            make_search_match(Some(agent_id.clone()), "entry-1"), // Subagent
        ],
        current_match: 0,
    };
    state.focus = FocusPane::Main; // Start in Main pane

    next_match(&mut state);

    assert_eq!(
        state.focus,
        FocusPane::Subagent,
        "Should switch to Subagent pane when match is in subagent"
    );
}

#[test]
fn next_match_selects_correct_subagent_tab() {
    use crate::model::{EntryMetadata, EntryType, LogEntry, Message, MessageContent, Role};
    use chrono::Utc;

    let mut entries = Vec::new();
    let agent1 = make_agent_id("agent-aaa");
    let agent2 = make_agent_id("agent-bbb");
    let agent3 = make_agent_id("agent-ccc");

    // Add entries to create three subagents (order matters for tab index)
    let timestamp: chrono::DateTime<Utc> = "2025-12-25T10:00:00Z".parse().unwrap();
    for (idx, agent_id) in [&agent1, &agent2, &agent3].iter().enumerate() {
        let entry = LogEntry::new(
            make_entry_uuid(&format!("entry-{}", idx)),
            None,
            make_session_id("test-session"),
            Some((*agent_id).clone()),
            timestamp,
            EntryType::User,
            Message::new(Role::User, MessageContent::Text("test".to_string())),
            EntryMetadata::default(),
        );
        entries.push(crate::model::ConversationEntry::Valid(Box::new(entry)));
    }

    let mut state = AppState::new();
    state.add_entries(entries);
    let query = SearchQuery::new("test").expect("valid query");

    state.search = SearchState::Active {
        query,
        matches: vec![
            make_search_match(Some(agent2.clone()), "entry-1"), // Second subagent
        ],
        current_match: 0,
    };
    state.focus = FocusPane::Main;
    state.selected_conversation = ConversationSelection::Main; // Start at main conversation

    next_match(&mut state);

    // Agent order in tabs is sorted alphabetically
    // Unified tab model (FR-086): tab 0 = main, tab 1+ = subagents
    // So first subagent is at tab 1, second at tab 2, etc.
    let mut agent_ids: Vec<_> = state.session_view().subagent_ids().collect();
    agent_ids.sort_by(|a, b| a.as_str().cmp(b.as_str()));
    let subagent_position = agent_ids
        .iter()
        .enumerate()
        .find(|(_, aid)| **aid == &agent2)
        .map(|(idx, _)| idx)
        .expect("agent2 should exist in subagent_ids");
    let expected_tab = subagent_position + 1; // Convert to global tab index

    assert_eq!(
        state.selected_tab_index(),
        Some(expected_tab),
        "Should select tab for agent2 (unified tab model: tab 0 = main)"
    );
}

// ===== prev_match Tests =====

#[test]
fn prev_match_when_inactive_does_nothing() {
    let mut state = AppState::new();
    let entries = Vec::new();
    state.add_entries(entries);
    state.search = SearchState::Inactive;
    state.focus = FocusPane::Main;

    prev_match(&mut state);

    assert!(
        matches!(state.search, SearchState::Inactive),
        "Search should remain Inactive"
    );
    assert_eq!(state.focus, FocusPane::Main, "Focus should be unchanged");
}

#[test]
fn prev_match_when_typing_does_nothing() {
    let mut state = AppState::new();
    let entries = Vec::new();
    state.add_entries(entries);
    state.search = SearchState::Typing {
        query: "test".to_string(),
        cursor: 2,
    };
    state.focus = FocusPane::Search;

    prev_match(&mut state);

    match state.search {
        SearchState::Typing { query, cursor } => {
            assert_eq!(query, "test", "Query should be unchanged");
            assert_eq!(cursor, 2, "Cursor should be unchanged");
        }
        _ => panic!("Expected Typing state to remain unchanged"),
    }
}

#[test]
fn prev_match_decrements_current_match() {
    let mut state = AppState::new();
    let query = SearchQuery::new("test").expect("valid query");

    state.search = SearchState::Active {
        query,
        matches: vec![
            make_search_match(None, "uuid-1"),
            make_search_match(None, "uuid-2"),
            make_search_match(None, "uuid-3"),
        ],
        current_match: 2, // Start at third match
    };

    prev_match(&mut state);

    match state.search {
        SearchState::Active { current_match, .. } => {
            assert_eq!(current_match, 1, "Should decrement from 2 to 1");
        }
        _ => panic!("Expected Active search state"),
    }
}

#[test]
fn prev_match_wraps_from_first_to_last() {
    let mut state = AppState::new();
    let query = SearchQuery::new("test").expect("valid query");

    state.search = SearchState::Active {
        query,
        matches: vec![
            make_search_match(None, "uuid-1"),
            make_search_match(None, "uuid-2"),
            make_search_match(None, "uuid-3"),
        ],
        current_match: 0, // First match
    };

    prev_match(&mut state);

    match state.search {
        SearchState::Active { current_match, .. } => {
            assert_eq!(current_match, 2, "Should wrap from 0 to 2 (last)");
        }
        _ => panic!("Expected Active search state"),
    }
}

#[test]
fn prev_match_with_single_match_stays_at_zero() {
    let mut state = AppState::new();
    let query = SearchQuery::new("test").expect("valid query");

    state.search = SearchState::Active {
        query,
        matches: vec![make_search_match(None, "uuid-1")],
        current_match: 0,
    };

    prev_match(&mut state);

    match state.search {
        SearchState::Active { current_match, .. } => {
            assert_eq!(current_match, 0, "Single match should wrap to 0");
        }
        _ => panic!("Expected Active search state"),
    }
}

#[test]
fn prev_match_switches_to_main_pane_when_match_in_main_agent() {
    let mut state = AppState::new();
    let query = SearchQuery::new("test").expect("valid query");

    state.search = SearchState::Active {
        query,
        matches: vec![
            make_search_match(None, "uuid-1"), // Main agent (agent_id = None)
        ],
        current_match: 0,
    };
    state.focus = FocusPane::Stats; // Start in different pane

    prev_match(&mut state);

    assert_eq!(
        state.focus,
        FocusPane::Main,
        "Should switch to Main pane when match is in main agent"
    );
}

#[test]
fn prev_match_switches_to_subagent_pane_when_match_in_subagent() {
    use crate::model::{EntryMetadata, EntryType, LogEntry, Message, MessageContent, Role};
    use chrono::Utc;

    let mut entries = Vec::new();
    let agent_id = make_agent_id("agent-xyz");

    // Add an entry to create the subagent
    let timestamp: chrono::DateTime<Utc> = "2025-12-25T10:00:00Z".parse().unwrap();
    let entry = LogEntry::new(
        make_entry_uuid("entry-1"),
        None,
        make_session_id("test-session"),
        Some(agent_id.clone()),
        timestamp,
        EntryType::User,
        Message::new(Role::User, MessageContent::Text("test".to_string())),
        EntryMetadata::default(),
    );
    entries.push(crate::model::ConversationEntry::Valid(Box::new(entry)));

    let mut state = AppState::new();
    state.add_entries(entries);
    let query = SearchQuery::new("test").expect("valid query");

    state.search = SearchState::Active {
        query,
        matches: vec![
            make_search_match(Some(agent_id.clone()), "entry-1"), // Subagent
        ],
        current_match: 0,
    };
    state.focus = FocusPane::Main; // Start in Main pane

    prev_match(&mut state);

    assert_eq!(
        state.focus,
        FocusPane::Subagent,
        "Should switch to Subagent pane when match is in subagent"
    );
}

// ===== Scroll Position Tests (US5/FR-013) =====

#[test]
fn next_match_scrolls_to_match_entry_in_main_conversation() {
    use crate::model::{EntryMetadata, EntryType, LogEntry, Message, MessageContent, Role};
    use crate::view_state::scroll::ScrollPosition;
    use crate::view_state::types::EntryIndex;
    use chrono::Utc;

    // Create multiple entries in main conversation
    let mut entries = Vec::new();
    let timestamp: chrono::DateTime<Utc> = "2025-12-25T10:00:00Z".parse().unwrap();

    for i in 0..5 {
        let entry = LogEntry::new(
            make_entry_uuid(&format!("entry-{}", i)),
            None,
            make_session_id("test-session"),
            None, // Main agent
            timestamp,
            EntryType::User,
            Message::new(Role::User, MessageContent::Text(format!("message {}", i))),
            EntryMetadata::default(),
        );
        entries.push(crate::model::ConversationEntry::Valid(Box::new(entry)));
    }

    let mut state = AppState::new();
    state.add_entries(entries);

    // Compute layout so entries have positions
    let params = crate::view_state::layout_params::LayoutParams {
        width: 80,
        global_wrap: crate::state::app_state::WrapMode::Wrap,
    };
    state.log_view_mut().current_session_mut().unwrap().main_mut().recompute_layout(params);

    let query = SearchQuery::new("message").expect("valid query");

    // Match at entry index 3
    state.search = SearchState::Active {
        query,
        matches: vec![
            make_search_match(None, "entry-3"), // Main agent, entry 3
        ],
        current_match: 0,
    };
    state.focus = FocusPane::Main;

    next_match(&mut state);

    // Verify scroll position updated to show entry 3
    let main_conv = state.main_conversation_view().expect("should have main conversation");
    match main_conv.scroll() {
        ScrollPosition::AtEntry { entry_index, .. } => {
            assert_eq!(
                *entry_index,
                EntryIndex::new(3),
                "Should scroll to entry 3 where match is located"
            );
        }
        other => panic!("Expected ScrollPosition::AtEntry, got {:?}", other),
    }
}

#[test]
fn prev_match_scrolls_to_match_entry_in_main_conversation() {
    use crate::model::{EntryMetadata, EntryType, LogEntry, Message, MessageContent, Role};
    use crate::view_state::scroll::ScrollPosition;
    use crate::view_state::types::EntryIndex;
    use chrono::Utc;

    // Create multiple entries in main conversation
    let mut entries = Vec::new();
    let timestamp: chrono::DateTime<Utc> = "2025-12-25T10:00:00Z".parse().unwrap();

    for i in 0..5 {
        let entry = LogEntry::new(
            make_entry_uuid(&format!("entry-{}", i)),
            None,
            make_session_id("test-session"),
            None, // Main agent
            timestamp,
            EntryType::User,
            Message::new(Role::User, MessageContent::Text(format!("message {}", i))),
            EntryMetadata::default(),
        );
        entries.push(crate::model::ConversationEntry::Valid(Box::new(entry)));
    }

    let mut state = AppState::new();
    state.add_entries(entries);

    // Compute layout
    let params = crate::view_state::layout_params::LayoutParams {
        width: 80,
        global_wrap: crate::state::app_state::WrapMode::Wrap,
    };
    state.log_view_mut().current_session_mut().unwrap().main_mut().recompute_layout(params);

    let query = SearchQuery::new("message").expect("valid query");

    // Two matches: entry 1 and entry 4
    state.search = SearchState::Active {
        query,
        matches: vec![
            make_search_match(None, "entry-1"),
            make_search_match(None, "entry-4"),
        ],
        current_match: 1, // Start at second match (entry 4)
    };
    state.focus = FocusPane::Main;

    prev_match(&mut state);

    // Verify scroll position updated to show entry 1 (prev match)
    let main_conv = state.main_conversation_view().expect("should have main conversation");
    match main_conv.scroll() {
        ScrollPosition::AtEntry { entry_index, .. } => {
            assert_eq!(
                *entry_index,
                EntryIndex::new(1),
                "Should scroll to entry 1 (previous match)"
            );
        }
        other => panic!("Expected ScrollPosition::AtEntry, got {:?}", other),
    }
}
