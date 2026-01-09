//! Tests for type-safe conversation selection (cclv-5ur.53).
//!
//! These tests verify that ConversationSelection provides stable,
//! identity-based tab selection that survives subagent additions/removals.

use super::*;
use crate::model::{AgentId, ConversationEntry};

// ===== Test Helpers =====

/// Create test state with main + 2 subagents (agent-a, agent-b alphabetically).
fn state_with_subagents() -> AppState {
    let mut state = AppState::new();

    // Add entries to create main and two subagents
    let main_entry = ConversationEntry::Valid(Box::new(create_test_log_entry(None)));
    let agent_a_entry = ConversationEntry::Valid(Box::new(create_test_log_entry(Some("agent-a"))));
    let agent_b_entry = ConversationEntry::Valid(Box::new(create_test_log_entry(Some("agent-b"))));

    state.add_entries(vec![main_entry, agent_a_entry, agent_b_entry]);
    state
}

fn create_test_log_entry(agent_id_str: Option<&str>) -> crate::model::LogEntry {
    use crate::model::{
        EntryMetadata, EntryType, EntryUuid, Message, MessageContent, Role, SessionId,
    };
    use chrono::Utc;

    let agent_id = agent_id_str.map(|s| AgentId::new(s).unwrap());

    crate::model::LogEntry::new(
        EntryUuid::new(format!("uuid-{}", agent_id_str.unwrap_or("main"))).unwrap(),
        None, // parent_uuid
        SessionId::new("session-1").unwrap(),
        agent_id,
        Utc::now(),
        EntryType::User,
        Message::new(Role::User, MessageContent::Text("test".to_string())),
        EntryMetadata::default(),
    )
}

// ===== Tests: selected_tab_index() =====

#[test]
fn selected_tab_index_main_returns_zero() {
    let state = state_with_subagents();
    // Default is Main
    assert_eq!(state.selected_tab_index(), Some(0));
}

#[test]
fn selected_tab_index_subagent_returns_position_plus_one() {
    let mut state = state_with_subagents();

    // Select agent-a (first subagent alphabetically)
    let agent_a = AgentId::new("agent-a").unwrap();
    state.selected_conversation = ConversationSelection::Subagent(agent_a);

    // Should map to tab 1 (0 is main, 1 is first subagent)
    assert_eq!(state.selected_tab_index(), Some(1));
}

#[test]
fn selected_tab_index_second_subagent_returns_two() {
    let mut state = state_with_subagents();

    // Select agent-b (second subagent alphabetically)
    let agent_b = AgentId::new("agent-b").unwrap();
    state.selected_conversation = ConversationSelection::Subagent(agent_b);

    // Should map to tab 2
    assert_eq!(state.selected_tab_index(), Some(2));
}

// ===== Tests: selected_conversation_view() respects viewed_session (cclv-463.3.6) =====

#[test]
fn selected_conversation_view_respects_viewed_session_latest() {
    use crate::model::SessionId;

    let state = create_test_state_with_sessions(3);
    // Default viewed_session is Latest

    let view = state.selected_conversation_view()
        .expect("Should have conversation view");

    // Should show latest session (session 2 with 0-indexed)
    let entries = view.entries();
    assert!(!entries.is_empty());

    // Verify the session ID matches the last session
    let expected_session_id = SessionId::new("550e8400-e29b-41d4-a716-446655440002").unwrap();
    let first_entry = entries.first().expect("Should have entry");
    let actual_session_id = first_entry.entry().session_id().expect("Should have session_id");
    assert_eq!(actual_session_id, &expected_session_id, "Expected latest session (index 2)");
}

#[test]
fn selected_conversation_view_respects_viewed_session_pinned_to_first() {
    use crate::model::SessionId;
    use crate::state::ViewedSession;

    let mut state = create_test_state_with_sessions(3);

    // Pin to first session (index 0)
    state.viewed_session = ViewedSession::pinned(0, 3).expect("valid pin");

    let view = state.selected_conversation_view()
        .expect("Should have conversation view");

    // Should show first session, NOT latest
    let entries = view.entries();
    assert!(!entries.is_empty());

    // Verify the session ID matches the FIRST session, not the last
    let expected_session_id = SessionId::new("550e8400-e29b-41d4-a716-446655440000").unwrap();
    let first_entry = entries.first().expect("Should have entry");
    let actual_session_id = first_entry.entry().session_id().expect("Should have session_id");
    assert_eq!(actual_session_id, &expected_session_id, "Expected first session (index 0), not latest");
}

#[test]
fn selected_conversation_view_respects_viewed_session_pinned_to_middle() {
    use crate::model::SessionId;
    use crate::state::ViewedSession;

    let mut state = create_test_state_with_sessions(3);

    // Pin to middle session (index 1)
    state.viewed_session = ViewedSession::pinned(1, 3).expect("valid pin");

    let view = state.selected_conversation_view()
        .expect("Should have conversation view");

    // Should show middle session
    let entries = view.entries();
    assert!(!entries.is_empty());

    // Verify the session ID matches the MIDDLE session
    let expected_session_id = SessionId::new("550e8400-e29b-41d4-a716-446655440001").unwrap();
    let first_entry = entries.first().expect("Should have entry");
    let actual_session_id = first_entry.entry().session_id().expect("Should have session_id");
    assert_eq!(actual_session_id, &expected_session_id, "Expected middle session (index 1)");
}

#[test]
fn selected_conversation_view_mut_respects_viewed_session_pinned() {
    use crate::model::SessionId;
    use crate::state::ViewedSession;

    let mut state = create_test_state_with_sessions(3);

    // Pin to first session
    state.viewed_session = ViewedSession::pinned(0, 3).expect("valid pin");

    let view = state.selected_conversation_view_mut()
        .expect("Should have conversation view");

    // Should show first session, NOT latest
    let entries = view.entries();
    assert!(!entries.is_empty());

    // Verify the session ID matches the FIRST session, not the last
    let expected_session_id = SessionId::new("550e8400-e29b-41d4-a716-446655440000").unwrap();
    let first_entry = entries.first().expect("Should have entry");
    let actual_session_id = first_entry.entry().session_id().expect("Should have session_id");
    assert_eq!(actual_session_id, &expected_session_id, "Expected first session (index 0), not latest");
}

#[test]
fn selected_tab_index_respects_viewed_session_pinned() {
    use crate::state::ViewedSession;

    let mut state = create_test_state_with_sessions(3);

    // Pin to first session
    state.viewed_session = ViewedSession::pinned(0, 3).expect("valid pin");

    // selected_tab_index should use the pinned session's subagents,
    // not the latest session's subagents
    let tab_index = state.selected_tab_index();

    // Should return Some(0) for Main, even when pinned to historical session
    assert_eq!(tab_index, Some(0));
}

// ===== Helper function (borrowed from session_modal_tests.rs) =====

/// Helper to create a test AppState with multiple sessions.
fn create_test_state_with_sessions(session_count: usize) -> AppState {
    use crate::model::{
        ConversationEntry, EntryMetadata, EntryType, EntryUuid, LogEntry, Message, MessageContent,
        Role, SessionId,
    };
    use chrono::Utc;

    let mut state = AppState::new();

    for i in 0..session_count {
        let session_id = SessionId::new(format!(
            "550e8400-e29b-41d4-a716-44665544000{}",
            i
        ).as_str())
        .unwrap();

        // Add a message to create the session
        let entry = LogEntry::new(
            EntryUuid::new(format!("uuid-session-{}", i)).unwrap(),
            None, // parent_uuid
            session_id,
            None, // main agent
            Utc::now(),
            EntryType::User,
            Message::new(
                Role::User,
                MessageContent::Text(format!("Message in session {}", i + 1)),
            ),
            EntryMetadata::default(),
        );
        state.add_entries(vec![ConversationEntry::Valid(Box::new(entry))]);
    }

    state
}

#[test]
fn selected_tab_index_nonexistent_subagent_returns_none() {
    let mut state = state_with_subagents();

    // Select agent that doesn't exist
    let agent_x = AgentId::new("agent-x").unwrap();
    state.selected_conversation = ConversationSelection::Subagent(agent_x);

    // Should return None (agent not found)
    assert_eq!(state.selected_tab_index(), None);
}

// ===== Tests: selected_agent_id() =====

#[test]
fn selected_agent_id_main_returns_none() {
    let state = state_with_subagents();
    // Default is Main, which has no AgentId
    assert_eq!(state.selected_agent_id(), None);
}

#[test]
fn selected_agent_id_subagent_returns_agent_id() {
    let mut state = state_with_subagents();

    let agent_a = AgentId::new("agent-a").unwrap();
    state.selected_conversation = ConversationSelection::Subagent(agent_a.clone());

    assert_eq!(state.selected_agent_id(), Some(agent_a));
}

// ===== Tests: selected_conversation_view() =====

#[test]
fn selected_conversation_view_main_returns_main_conversation() {
    let state = state_with_subagents();

    let view = state.selected_conversation_view();
    assert!(view.is_some(), "Main conversation should exist");
    // Can't deeply inspect without more setup, but existence proves routing
}

#[test]
fn selected_conversation_view_subagent_returns_subagent_conversation() {
    let mut state = state_with_subagents();

    let agent_a = AgentId::new("agent-a").unwrap();
    state.selected_conversation = ConversationSelection::Subagent(agent_a);

    let view = state.selected_conversation_view();
    assert!(view.is_some(), "Subagent conversation should exist");
}

#[test]
fn selected_conversation_view_nonexistent_subagent_returns_none() {
    let mut state = state_with_subagents();

    let agent_x = AgentId::new("agent-x").unwrap();
    state.selected_conversation = ConversationSelection::Subagent(agent_x);

    let view = state.selected_conversation_view();
    assert!(view.is_none(), "Nonexistent subagent should return None");
}

// ===== Tests: next_tab() =====

#[test]
fn next_tab_from_main_selects_first_subagent() {
    let mut state = state_with_subagents();
    // Start at Main
    assert_eq!(state.selected_conversation, ConversationSelection::Main);

    state.next_tab();

    // Should select agent-a (first alphabetically)
    let agent_a = AgentId::new("agent-a").unwrap();
    assert_eq!(
        state.selected_conversation,
        ConversationSelection::Subagent(agent_a)
    );
}

#[test]
fn next_tab_from_first_subagent_selects_second() {
    let mut state = state_with_subagents();
    let agent_a = AgentId::new("agent-a").unwrap();
    state.selected_conversation = ConversationSelection::Subagent(agent_a);

    state.next_tab();

    // Should select agent-b
    let agent_b = AgentId::new("agent-b").unwrap();
    assert_eq!(
        state.selected_conversation,
        ConversationSelection::Subagent(agent_b)
    );
}

#[test]
fn next_tab_from_last_subagent_wraps_to_main() {
    let mut state = state_with_subagents();
    let agent_b = AgentId::new("agent-b").unwrap();
    state.selected_conversation = ConversationSelection::Subagent(agent_b);

    state.next_tab();

    // Should wrap to Main
    assert_eq!(state.selected_conversation, ConversationSelection::Main);
}

#[test]
fn next_tab_with_search_active_is_noop() {
    let mut state = state_with_subagents();
    state.search = crate::state::SearchState::Typing {
        query: String::new(),
        cursor: 0,
    };
    let original = state.selected_conversation.clone();

    state.next_tab();

    // Should not change when search is active
    assert_eq!(state.selected_conversation, original);
}

// ===== Tests: prev_tab() =====

#[test]
fn prev_tab_from_main_wraps_to_last_subagent() {
    let mut state = state_with_subagents();
    // Start at Main

    state.prev_tab();

    // Should wrap to agent-b (last alphabetically)
    let agent_b = AgentId::new("agent-b").unwrap();
    assert_eq!(
        state.selected_conversation,
        ConversationSelection::Subagent(agent_b)
    );
}

#[test]
fn prev_tab_from_second_subagent_selects_first() {
    let mut state = state_with_subagents();
    let agent_b = AgentId::new("agent-b").unwrap();
    state.selected_conversation = ConversationSelection::Subagent(agent_b);

    state.prev_tab();

    // Should select agent-a
    let agent_a = AgentId::new("agent-a").unwrap();
    assert_eq!(
        state.selected_conversation,
        ConversationSelection::Subagent(agent_a)
    );
}

#[test]
fn prev_tab_from_first_subagent_selects_main() {
    let mut state = state_with_subagents();
    let agent_a = AgentId::new("agent-a").unwrap();
    state.selected_conversation = ConversationSelection::Subagent(agent_a);

    state.prev_tab();

    // Should select Main
    assert_eq!(state.selected_conversation, ConversationSelection::Main);
}

// ===== Tests: select_tab(n) =====

#[test]
fn select_tab_one_selects_main() {
    let mut state = state_with_subagents();

    state.select_tab(1); // 1-indexed: tab 1 = main

    assert_eq!(state.selected_conversation, ConversationSelection::Main);
}

#[test]
fn select_tab_two_selects_first_subagent() {
    let mut state = state_with_subagents();

    state.select_tab(2); // 1-indexed: tab 2 = first subagent

    let agent_a = AgentId::new("agent-a").unwrap();
    assert_eq!(
        state.selected_conversation,
        ConversationSelection::Subagent(agent_a)
    );
}

#[test]
fn select_tab_three_selects_second_subagent() {
    let mut state = state_with_subagents();

    state.select_tab(3); // 1-indexed: tab 3 = second subagent

    let agent_b = AgentId::new("agent-b").unwrap();
    assert_eq!(
        state.selected_conversation,
        ConversationSelection::Subagent(agent_b)
    );
}

#[test]
fn select_tab_zero_is_noop() {
    let mut state = state_with_subagents();
    let original = state.selected_conversation.clone();

    state.select_tab(0); // Invalid 1-indexed input

    // Should not change
    assert_eq!(state.selected_conversation, original);
}

#[test]
fn select_tab_too_high_clamps_to_last() {
    let mut state = state_with_subagents();

    state.select_tab(999); // Way beyond bounds

    // Should clamp to agent-b (last tab)
    let agent_b = AgentId::new("agent-b").unwrap();
    assert_eq!(
        state.selected_conversation,
        ConversationSelection::Subagent(agent_b)
    );
}

// ===== Tests: Stability across subagent changes =====

#[test]
fn selection_stable_when_subagent_added() {
    let mut state = state_with_subagents();

    // Select agent-b
    let agent_b = AgentId::new("agent-b").unwrap();
    state.selected_conversation = ConversationSelection::Subagent(agent_b.clone());

    // Add a new subagent agent-c
    let agent_c_entry = ConversationEntry::Valid(Box::new(create_test_log_entry(Some("agent-c"))));
    state.add_entries(vec![agent_c_entry]);

    // Selection should still be agent-b (unchanged)
    assert_eq!(
        state.selected_conversation,
        ConversationSelection::Subagent(agent_b)
    );
}

#[test]
fn tab_index_changes_when_earlier_subagent_added() {
    let mut state = state_with_subagents();

    // Select agent-b (currently at index 2)
    let agent_b = AgentId::new("agent-b").unwrap();
    state.selected_conversation = ConversationSelection::Subagent(agent_b.clone());
    assert_eq!(state.selected_tab_index(), Some(2));

    // Add agent-a1 (alphabetically between agent-a and agent-b)
    let agent_a1_entry =
        ConversationEntry::Valid(Box::new(create_test_log_entry(Some("agent-a1"))));
    state.add_entries(vec![agent_a1_entry]);

    // Selection is still agent-b (stable identity)
    assert_eq!(
        state.selected_conversation,
        ConversationSelection::Subagent(agent_b)
    );

    // But tab_index moved from 2 to 3 (agent-a1 inserted before agent-b)
    assert_eq!(state.selected_tab_index(), Some(3));
}
