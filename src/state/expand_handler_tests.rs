//! Tests for expand/collapse handler.
//!
//! Tests verify focus-aware expand/collapse action dispatching:
//! - ToggleExpand toggles the focused message
//! - ExpandMessage expands all messages in current pane
//! - CollapseMessage collapses all messages in current pane
//! - Actions target correct scroll state based on focus

use super::*;
use crate::model::{KeyAction, Session, SessionId};
use crate::state::{AppState, FocusPane};

/// Helper to create test AppState with known entry count
fn create_test_state_with_entries(main_entries: usize, subagent_entries: usize) -> AppState {
    let session_id = SessionId::new("test-session").unwrap();
    let mut session = Session::new(session_id);

    // Add main agent entries
    for i in 0..main_entries {
        let entry = create_test_log_entry(format!("main-{}", i), None);
        session.add_conversation_entry(crate::model::ConversationEntry::Valid(Box::new(entry)));
    }

    // Add subagent entries
    if subagent_entries > 0 {
        let agent_id = crate::model::AgentId::new("test-agent").unwrap();
        for i in 0..subagent_entries {
            let entry = create_test_log_entry(format!("sub-{}", i), Some(agent_id.clone()));
            session.add_conversation_entry(crate::model::ConversationEntry::Valid(Box::new(entry)));
        }
    }

    AppState::new(session)
}

/// Helper to create a test log entry
fn create_test_log_entry(
    content: String,
    agent_id: Option<crate::model::AgentId>,
) -> crate::model::LogEntry {
    use crate::model::{EntryMetadata, EntryType, EntryUuid, Message, MessageContent, Role};
    use chrono::Utc;
    use std::sync::atomic::{AtomicU32, Ordering};

    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let id = COUNTER.fetch_add(1, Ordering::Relaxed);

    let message = Message::new(Role::User, MessageContent::Text(content));

    crate::model::LogEntry::new(
        EntryUuid::new(format!("uuid-{}", id)).unwrap(),
        None,
        SessionId::new("test-session").unwrap(),
        agent_id,
        Utc::now(),
        EntryType::User,
        message,
        EntryMetadata::default(),
    )
}

// ===== ToggleExpand tests =====

#[test]
fn toggle_expand_expands_collapsed_message() {
    let mut state = create_test_state_with_entries(3, 0);
    state.focus = FocusPane::Main;
    state.main_scroll.set_focused_message(Some(1));

    // Initially all messages are collapsed
    let entry_uuid = state
        .session()
        .main_agent()
        .entries()
        .get(1)
        .and_then(|e| e.as_valid())
        .unwrap()
        .uuid()
        .clone();

    assert!(
        !state.main_scroll.is_expanded(&entry_uuid),
        "Message should start collapsed"
    );

    let new_state = handle_expand_action(state, KeyAction::ToggleExpand);

    assert!(
        new_state.main_scroll.is_expanded(&entry_uuid),
        "ToggleExpand should expand the focused message"
    );
}

#[test]
fn toggle_expand_collapses_expanded_message() {
    let mut state = create_test_state_with_entries(3, 0);
    state.focus = FocusPane::Main;
    state.main_scroll.set_focused_message(Some(1));

    // Get the entry UUID
    let entry_uuid = state
        .session()
        .main_agent()
        .entries()
        .get(1)
        .and_then(|e| e.as_valid())
        .unwrap()
        .uuid()
        .clone();

    // Manually expand it first
    state.main_scroll.toggle_expand(&entry_uuid);
    assert!(
        state.main_scroll.is_expanded(&entry_uuid),
        "Setup: message should be expanded"
    );

    let new_state = handle_expand_action(state, KeyAction::ToggleExpand);

    assert!(
        !new_state.main_scroll.is_expanded(&entry_uuid),
        "ToggleExpand should collapse the focused message"
    );
}

#[test]
fn toggle_expand_does_nothing_when_no_focused_message() {
    let mut state = create_test_state_with_entries(3, 0);
    state.focus = FocusPane::Main;
    state.main_scroll.set_focused_message(None);

    let new_state = handle_expand_action(state.clone(), KeyAction::ToggleExpand);

    assert_eq!(
        new_state.main_scroll.expanded_messages.len(),
        state.main_scroll.expanded_messages.len(),
        "ToggleExpand with no focused message should not change expanded set"
    );
}

#[test]
fn toggle_expand_works_on_subagent_pane() {
    let mut state = create_test_state_with_entries(0, 3);
    state.focus = FocusPane::Subagent;
    state.selected_tab = Some(0);
    state.subagent_scroll.set_focused_message(Some(0));

    // Get the subagent entry UUID
    let agent_id = state.session().subagent_ids_ordered()[0];
    let entry_uuid = state
        .session()
        .subagents()
        .get(agent_id)
        .unwrap()
        .entries()
        .first()
        .and_then(|e| e.as_valid())
        .unwrap()
        .uuid()
        .clone();

    assert!(
        !state.subagent_scroll.is_expanded(&entry_uuid),
        "Message should start collapsed"
    );

    let new_state = handle_expand_action(state, KeyAction::ToggleExpand);

    assert!(
        new_state.subagent_scroll.is_expanded(&entry_uuid),
        "ToggleExpand should work on subagent pane"
    );
}

// ===== ExpandMessage tests =====

#[test]
fn expand_message_expands_all_main_pane_messages() {
    let mut state = create_test_state_with_entries(3, 0);
    state.focus = FocusPane::Main;

    // Collect UUIDs before moving state
    let uuids: Vec<_> = state
        .session()
        .main_agent()
        .entries()
        .iter()
        .filter_map(|e| e.as_valid().map(|log| log.uuid().clone()))
        .collect();

    let new_state = handle_expand_action(state, KeyAction::ExpandMessage);

    // Check all main pane messages are expanded
    for uuid in uuids {
        assert!(
            new_state.main_scroll.is_expanded(&uuid),
            "ExpandMessage should expand all messages in main pane"
        );
    }
}

#[test]
fn expand_message_expands_all_subagent_pane_messages() {
    let mut state = create_test_state_with_entries(0, 3);
    state.focus = FocusPane::Subagent;
    state.selected_tab = Some(0);

    // Collect UUIDs before moving state
    let agent_id = state.session().subagent_ids_ordered()[0];
    let uuids: Vec<_> = state
        .session()
        .subagents()
        .get(agent_id)
        .unwrap()
        .entries()
        .iter()
        .filter_map(|e| e.as_valid().map(|log| log.uuid().clone()))
        .collect();

    let new_state = handle_expand_action(state, KeyAction::ExpandMessage);

    // Check all subagent pane messages are expanded
    for uuid in uuids {
        assert!(
            new_state.subagent_scroll.is_expanded(&uuid),
            "ExpandMessage should expand all messages in subagent pane"
        );
    }
}

#[test]
fn expand_message_does_nothing_on_stats_pane() {
    let mut state = create_test_state_with_entries(3, 0);
    state.focus = FocusPane::Stats;

    let new_state = handle_expand_action(state.clone(), KeyAction::ExpandMessage);

    assert_eq!(
        new_state.main_scroll.expanded_messages.len(),
        state.main_scroll.expanded_messages.len(),
        "ExpandMessage on Stats pane should not modify main scroll"
    );
}

#[test]
fn expand_message_does_nothing_on_search_pane() {
    let mut state = create_test_state_with_entries(3, 0);
    state.focus = FocusPane::Search;

    let new_state = handle_expand_action(state.clone(), KeyAction::ExpandMessage);

    assert_eq!(
        new_state.main_scroll.expanded_messages.len(),
        state.main_scroll.expanded_messages.len(),
        "ExpandMessage on Search pane should not modify main scroll"
    );
}

// ===== CollapseMessage tests =====

#[test]
fn collapse_message_collapses_all_main_pane_messages() {
    let mut state = create_test_state_with_entries(3, 0);
    state.focus = FocusPane::Main;

    // Expand all messages first
    let uuids: Vec<_> = state
        .session()
        .main_agent()
        .entries()
        .iter()
        .filter_map(|e| e.as_valid().map(|log| log.uuid().clone()))
        .collect();

    for uuid in uuids {
        state.main_scroll.toggle_expand(&uuid);
    }

    assert!(
        !state.main_scroll.expanded_messages.is_empty(),
        "Setup: should have expanded messages"
    );

    let new_state = handle_expand_action(state, KeyAction::CollapseMessage);

    assert!(
        new_state.main_scroll.expanded_messages.is_empty(),
        "CollapseMessage should clear all expanded messages in main pane"
    );
}

#[test]
fn collapse_message_collapses_all_subagent_pane_messages() {
    let mut state = create_test_state_with_entries(0, 3);
    state.focus = FocusPane::Subagent;
    state.selected_tab = Some(0);

    // Expand all subagent messages first
    let agent_id = state.session().subagent_ids_ordered()[0];
    let uuids: Vec<_> = state
        .session()
        .subagents()
        .get(agent_id)
        .unwrap()
        .entries()
        .iter()
        .filter_map(|e| e.as_valid().map(|log| log.uuid().clone()))
        .collect();

    for uuid in uuids {
        state.subagent_scroll.toggle_expand(&uuid);
    }

    assert!(
        !state.subagent_scroll.expanded_messages.is_empty(),
        "Setup: should have expanded messages"
    );

    let new_state = handle_expand_action(state, KeyAction::CollapseMessage);

    assert!(
        new_state.subagent_scroll.expanded_messages.is_empty(),
        "CollapseMessage should clear all expanded messages in subagent pane"
    );
}

#[test]
fn collapse_message_does_nothing_on_stats_pane() {
    let mut state = create_test_state_with_entries(3, 0);
    state.focus = FocusPane::Stats;

    // Expand some messages
    let uuids: Vec<_> = state
        .session()
        .main_agent()
        .entries()
        .iter()
        .filter_map(|e| e.as_valid().map(|log| log.uuid().clone()))
        .collect();

    for uuid in uuids {
        state.main_scroll.toggle_expand(&uuid);
    }

    let expanded_count = state.main_scroll.expanded_messages.len();

    let new_state = handle_expand_action(state, KeyAction::CollapseMessage);

    assert_eq!(
        new_state.main_scroll.expanded_messages.len(),
        expanded_count,
        "CollapseMessage on Stats pane should not modify main scroll"
    );
}

// ===== Non-expand actions =====

#[test]
fn non_expand_actions_are_no_ops() {
    let mut state = create_test_state_with_entries(3, 0);
    state.focus = FocusPane::Main;

    let new_state = handle_expand_action(state.clone(), KeyAction::ScrollDown);

    assert_eq!(
        new_state.main_scroll.expanded_messages.len(),
        state.main_scroll.expanded_messages.len(),
        "Non-expand actions should be no-ops"
    );
}
