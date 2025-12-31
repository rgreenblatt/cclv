//! Tests for expand_handler - Handler Integration with HeightIndex

use super::*;
use crate::model::{
    ConversationEntry, EntryMetadata, EntryType, EntryUuid, LogEntry, Message, MessageContent,
    Role, SessionId,
};
use crate::view_state::types::EntryIndex;

// ===== Test Helpers =====

fn make_session_id(s: &str) -> SessionId {
    SessionId::new(s).expect("valid session id")
}

fn make_entry_uuid(s: &str) -> EntryUuid {
    EntryUuid::new(s).expect("valid uuid")
}

fn make_timestamp() -> chrono::DateTime<chrono::Utc> {
    "2025-12-28T10:00:00Z".parse().expect("valid timestamp")
}

fn make_message(text: &str) -> Message {
    Message::new(Role::User, MessageContent::Text(text.to_string()))
}

fn make_valid_entry(uuid: &str) -> ConversationEntry {
    let log_entry = LogEntry::new(
        make_entry_uuid(uuid),
        None,
        make_session_id("session-1"),
        None,
        make_timestamp(),
        EntryType::User,
        make_message("Test message"),
        EntryMetadata::default(),
    );
    ConversationEntry::Valid(Box::new(log_entry))
}

// ===== Handler Integration Tests =====

/// Test that toggle_entry_expanded is called atomically on the ConversationViewState.
///
/// This verifies that the handler properly delegates to the new HeightIndex-integrated
/// method rather than the old toggle_expand API.
#[test]
fn test_toggle_expand_calls_toggle_entry_expanded() {
    // Create AppState with entries
    let entries = vec![
        make_valid_entry("uuid-1"),
        make_valid_entry("uuid-2"),
        make_valid_entry("uuid-3"),
    ];

    let mut state = AppState::new();
    state.add_entries(entries);
    state.focus = FocusPane::Main;

    // Initialize HeightIndex by calling relayout
    if let Some(view) = state.main_conversation_view_mut() {
        view.relayout(80, crate::state::WrapMode::Wrap);
        view.set_focused_message(Some(EntryIndex::new(1)));
    }

    // Invoke handler
    let result = handle_expand_action(state, crate::model::KeyAction::ToggleExpand, 80);

    // Verify: Entry 1 should be toggled (expanded)
    if let Some(view) = result.main_conversation_view() {
        let entry = view.get(EntryIndex::new(1)).expect("entry exists");
        assert!(
            entry.is_expanded(),
            "Entry 1 should be expanded after toggle"
        );
    } else {
        panic!("Expected main conversation view");
    }
}

/// Test that expand/collapse all operations use toggle_entry_expanded.
#[test]
fn test_expand_all_uses_toggle_entry_expanded() {
    let entries = vec![
        make_valid_entry("uuid-1"),
        make_valid_entry("uuid-2"),
        make_valid_entry("uuid-3"),
    ];

    let mut state = AppState::new();
    state.add_entries(entries);
    state.focus = FocusPane::Main;

    // Initialize HeightIndex
    if let Some(view) = state.main_conversation_view_mut() {
        view.relayout(80, crate::state::WrapMode::Wrap);
    }

    // Expand all
    let result = handle_expand_action(state, crate::model::KeyAction::ExpandMessage, 80);

    // Verify: All entries should be expanded
    if let Some(view) = result.main_conversation_view() {
        for i in 0..3 {
            let entry = view.get(EntryIndex::new(i)).expect("entry exists");
            assert!(entry.is_expanded(), "Entry {} should be expanded", i);
        }
    } else {
        panic!("Expected main conversation view");
    }
}

/// Test that HeightIndex invariant holds after toggle operations.
///
/// Verifies that height_index[i] == entries[i].rendered_lines.len() after handler operations.
#[test]
fn test_toggle_maintains_height_index_invariant() {
    let entries = vec![make_valid_entry("uuid-1"), make_valid_entry("uuid-2")];

    let mut state = AppState::new();
    state.add_entries(entries);
    state.focus = FocusPane::Main;

    // Initialize HeightIndex and set focused message
    if let Some(view) = state.main_conversation_view_mut() {
        view.relayout(80, crate::state::WrapMode::Wrap);
        view.set_focused_message(Some(EntryIndex::new(0)));
    }

    // Toggle expand
    let result = handle_expand_action(state, crate::model::KeyAction::ToggleExpand, 80);

    // Verify HeightIndex invariant
    if let Some(view) = result.main_conversation_view() {
        for i in 0..view.len() {
            let entry = view.get(EntryIndex::new(i)).expect("entry exists");
            let entry_height = entry.height().get() as usize;

            // Extract height from HeightIndex
            let index_height = if i == 0 {
                view.height_index.prefix_sum(0)
            } else {
                view.height_index.prefix_sum(i) - view.height_index.prefix_sum(i - 1)
            };

            assert_eq!(
                index_height, entry_height,
                "HeightIndex invariant violated at entry {}: index={}, entry={}",
                i, index_height, entry_height
            );
        }
    }
}
