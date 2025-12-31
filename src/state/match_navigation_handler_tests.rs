//! Tests for match navigation handler.
//!
//! All tests written BEFORE implementation (TDD).
//! Tests verify runtime behavior of match navigation.

use super::*;
use crate::model::{AgentId, EntryUuid, Session, SessionId};
use crate::state::{FocusPane, SearchQuery, SearchState};

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

fn make_test_session() -> Session {
    Session::new(make_session_id("test-session"))
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
    let session = make_test_session();
    let mut state = AppState::new(session);
    state.search = SearchState::Inactive;
    state.focus = FocusPane::Main;

    let result = next_match(state.clone());

    assert!(
        matches!(result.search, SearchState::Inactive),
        "Search should remain Inactive"
    );
    assert_eq!(result.focus, FocusPane::Main, "Focus should be unchanged");
}

#[test]
fn next_match_when_typing_does_nothing() {
    let session = make_test_session();
    let mut state = AppState::new(session);
    state.search = SearchState::Typing {
        query: "test".to_string(),
        cursor: 2,
    };
    state.focus = FocusPane::Search;

    let result = next_match(state.clone());

    match result.search {
        SearchState::Typing { query, cursor } => {
            assert_eq!(query, "test", "Query should be unchanged");
            assert_eq!(cursor, 2, "Cursor should be unchanged");
        }
        _ => panic!("Expected Typing state to remain unchanged"),
    }
}

#[test]
fn next_match_increments_current_match() {
    let session = make_test_session();
    let mut state = AppState::new(session);
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

    let result = next_match(state);

    match result.search {
        SearchState::Active { current_match, .. } => {
            assert_eq!(current_match, 1, "Should increment from 0 to 1");
        }
        _ => panic!("Expected Active search state"),
    }
}

#[test]
fn next_match_wraps_from_last_to_first() {
    let session = make_test_session();
    let mut state = AppState::new(session);
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

    let result = next_match(state);

    match result.search {
        SearchState::Active { current_match, .. } => {
            assert_eq!(current_match, 0, "Should wrap from 2 to 0");
        }
        _ => panic!("Expected Active search state"),
    }
}

#[test]
fn next_match_with_single_match_stays_at_zero() {
    let session = make_test_session();
    let mut state = AppState::new(session);
    let query = SearchQuery::new("test").expect("valid query");

    state.search = SearchState::Active {
        query,
        matches: vec![make_search_match(None, "uuid-1")],
        current_match: 0,
    };

    let result = next_match(state);

    match result.search {
        SearchState::Active { current_match, .. } => {
            assert_eq!(current_match, 0, "Single match should wrap to 0");
        }
        _ => panic!("Expected Active search state"),
    }
}

#[test]
fn next_match_switches_to_main_pane_when_match_in_main_agent() {
    let session = make_test_session();
    let mut state = AppState::new(session);
    let query = SearchQuery::new("test").expect("valid query");

    state.search = SearchState::Active {
        query,
        matches: vec![
            make_search_match(None, "uuid-1"), // Main agent (agent_id = None)
        ],
        current_match: 0,
    };
    state.focus = FocusPane::Stats; // Start in different pane

    let result = next_match(state);

    assert_eq!(
        result.focus,
        FocusPane::Main,
        "Should switch to Main pane when match is in main agent"
    );
}

#[test]
fn next_match_switches_to_subagent_pane_when_match_in_subagent() {
    use crate::model::{EntryMetadata, EntryType, LogEntry, Message, MessageContent, Role};
    use chrono::Utc;

    let mut session = Session::new(make_session_id("test-session"));
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
    session.add_entry(entry);

    let mut state = AppState::new(session);
    let query = SearchQuery::new("test").expect("valid query");

    state.search = SearchState::Active {
        query,
        matches: vec![
            make_search_match(Some(agent_id.clone()), "entry-1"), // Subagent
        ],
        current_match: 0,
    };
    state.focus = FocusPane::Main; // Start in Main pane

    let result = next_match(state);

    assert_eq!(
        result.focus,
        FocusPane::Subagent,
        "Should switch to Subagent pane when match is in subagent"
    );
}

#[test]
fn next_match_selects_correct_subagent_tab() {
    use crate::model::{EntryMetadata, EntryType, LogEntry, Message, MessageContent, Role};
    use chrono::Utc;

    let mut session = Session::new(make_session_id("test-session"));
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
        session.add_entry(entry);
    }

    let mut state = AppState::new(session);
    let query = SearchQuery::new("test").expect("valid query");

    state.search = SearchState::Active {
        query,
        matches: vec![
            make_search_match(Some(agent2.clone()), "entry-1"), // Second subagent
        ],
        current_match: 0,
    };
    state.focus = FocusPane::Main;
    state.selected_tab = Some(0); // Start at first tab

    let result = next_match(state);

    // Agent order in Session::subagents() is deterministic (BTreeMap/sorted)
    // We need to find which tab index agent2 is at
    let expected_tab = result
        .session()
        .subagents()
        .keys()
        .enumerate()
        .find(|(_, aid)| *aid == &agent2)
        .map(|(idx, _)| idx)
        .expect("agent2 should exist in subagents");

    assert_eq!(
        result.selected_tab,
        Some(expected_tab),
        "Should select tab for agent2"
    );
}

// ===== prev_match Tests =====

#[test]
fn prev_match_when_inactive_does_nothing() {
    let session = make_test_session();
    let mut state = AppState::new(session);
    state.search = SearchState::Inactive;
    state.focus = FocusPane::Main;

    let result = prev_match(state.clone());

    assert!(
        matches!(result.search, SearchState::Inactive),
        "Search should remain Inactive"
    );
    assert_eq!(result.focus, FocusPane::Main, "Focus should be unchanged");
}

#[test]
fn prev_match_when_typing_does_nothing() {
    let session = make_test_session();
    let mut state = AppState::new(session);
    state.search = SearchState::Typing {
        query: "test".to_string(),
        cursor: 2,
    };
    state.focus = FocusPane::Search;

    let result = prev_match(state.clone());

    match result.search {
        SearchState::Typing { query, cursor } => {
            assert_eq!(query, "test", "Query should be unchanged");
            assert_eq!(cursor, 2, "Cursor should be unchanged");
        }
        _ => panic!("Expected Typing state to remain unchanged"),
    }
}

#[test]
fn prev_match_decrements_current_match() {
    let session = make_test_session();
    let mut state = AppState::new(session);
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

    let result = prev_match(state);

    match result.search {
        SearchState::Active { current_match, .. } => {
            assert_eq!(current_match, 1, "Should decrement from 2 to 1");
        }
        _ => panic!("Expected Active search state"),
    }
}

#[test]
fn prev_match_wraps_from_first_to_last() {
    let session = make_test_session();
    let mut state = AppState::new(session);
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

    let result = prev_match(state);

    match result.search {
        SearchState::Active { current_match, .. } => {
            assert_eq!(current_match, 2, "Should wrap from 0 to 2 (last)");
        }
        _ => panic!("Expected Active search state"),
    }
}

#[test]
fn prev_match_with_single_match_stays_at_zero() {
    let session = make_test_session();
    let mut state = AppState::new(session);
    let query = SearchQuery::new("test").expect("valid query");

    state.search = SearchState::Active {
        query,
        matches: vec![make_search_match(None, "uuid-1")],
        current_match: 0,
    };

    let result = prev_match(state);

    match result.search {
        SearchState::Active { current_match, .. } => {
            assert_eq!(current_match, 0, "Single match should wrap to 0");
        }
        _ => panic!("Expected Active search state"),
    }
}

#[test]
fn prev_match_switches_to_main_pane_when_match_in_main_agent() {
    let session = make_test_session();
    let mut state = AppState::new(session);
    let query = SearchQuery::new("test").expect("valid query");

    state.search = SearchState::Active {
        query,
        matches: vec![
            make_search_match(None, "uuid-1"), // Main agent (agent_id = None)
        ],
        current_match: 0,
    };
    state.focus = FocusPane::Stats; // Start in different pane

    let result = prev_match(state);

    assert_eq!(
        result.focus,
        FocusPane::Main,
        "Should switch to Main pane when match is in main agent"
    );
}

#[test]
fn prev_match_switches_to_subagent_pane_when_match_in_subagent() {
    use crate::model::{EntryMetadata, EntryType, LogEntry, Message, MessageContent, Role};
    use chrono::Utc;

    let mut session = Session::new(make_session_id("test-session"));
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
    session.add_entry(entry);

    let mut state = AppState::new(session);
    let query = SearchQuery::new("test").expect("valid query");

    state.search = SearchState::Active {
        query,
        matches: vec![
            make_search_match(Some(agent_id.clone()), "entry-1"), // Subagent
        ],
        current_match: 0,
    };
    state.focus = FocusPane::Main; // Start in Main pane

    let result = prev_match(state);

    assert_eq!(
        result.focus,
        FocusPane::Subagent,
        "Should switch to Subagent pane when match is in subagent"
    );
}
