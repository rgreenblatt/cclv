//! Tests for scroll handler.
//!
//! Tests verify focus-aware scroll action dispatching:
//! - ScrollUp/ScrollDown modify vertical_offset
//! - PageUp/PageDown move by viewport_height
//! - ScrollToTop/ScrollToBottom jump to bounds
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

// ===== ScrollUp tests =====

#[test]
fn scroll_up_decrements_vertical_offset() {
    let mut state = create_test_state_with_entries(5, 0);
    state.focus = FocusPane::Main;
    state.main_scroll.vertical_offset = 3;

    let new_state = handle_scroll_action(state, KeyAction::ScrollUp, 10);

    assert_eq!(
        new_state.main_scroll.vertical_offset, 2,
        "ScrollUp should decrement vertical_offset by 1"
    );
}

#[test]
fn scroll_up_saturates_at_zero() {
    let mut state = create_test_state_with_entries(5, 0);
    state.focus = FocusPane::Main;
    state.main_scroll.vertical_offset = 0;

    let new_state = handle_scroll_action(state, KeyAction::ScrollUp, 10);

    assert_eq!(
        new_state.main_scroll.vertical_offset, 0,
        "ScrollUp from 0 should stay at 0"
    );
}

#[test]
fn scroll_up_targets_subagent_when_focused() {
    let mut state = create_test_state_with_entries(5, 5);
    state.focus = FocusPane::Subagent;
    state.selected_tab = Some(0); // Select first subagent tab
    state.main_scroll.vertical_offset = 3;
    state.subagent_scroll.vertical_offset = 4;

    let new_state = handle_scroll_action(state, KeyAction::ScrollUp, 10);

    assert_eq!(
        new_state.main_scroll.vertical_offset, 3,
        "Main scroll should be unchanged when Subagent focused"
    );
    assert_eq!(
        new_state.subagent_scroll.vertical_offset, 3,
        "Subagent scroll should decrement when Subagent focused"
    );
}

// ===== ScrollDown tests =====

#[test]
fn scroll_down_increments_vertical_offset() {
    let mut state = create_test_state_with_entries(10, 0);
    state.focus = FocusPane::Main;
    state.main_scroll.vertical_offset = 2;

    let new_state = handle_scroll_action(state, KeyAction::ScrollDown, 10);

    assert_eq!(
        new_state.main_scroll.vertical_offset, 3,
        "ScrollDown should increment vertical_offset by 1"
    );
}

#[test]
fn scroll_down_clamps_to_max_entries() {
    let mut state = create_test_state_with_entries(5, 0);
    state.focus = FocusPane::Main;
    state.main_scroll.vertical_offset = 4; // max is len() - 1 = 4

    let new_state = handle_scroll_action(state, KeyAction::ScrollDown, 10);

    assert_eq!(
        new_state.main_scroll.vertical_offset, 4,
        "ScrollDown should clamp to max_entries (len - 1)"
    );
}

#[test]
fn scroll_down_targets_subagent_when_focused() {
    let mut state = create_test_state_with_entries(5, 5);
    state.focus = FocusPane::Subagent;
    state.selected_tab = Some(0); // Select first subagent tab
    state.main_scroll.vertical_offset = 2;
    state.subagent_scroll.vertical_offset = 1;

    let new_state = handle_scroll_action(state, KeyAction::ScrollDown, 10);

    assert_eq!(
        new_state.main_scroll.vertical_offset, 2,
        "Main scroll should be unchanged when Subagent focused"
    );
    assert_eq!(
        new_state.subagent_scroll.vertical_offset, 2,
        "Subagent scroll should increment when Subagent focused"
    );
}

// ===== PageDown tests =====

#[test]
fn page_down_moves_by_viewport_height() {
    let mut state = create_test_state_with_entries(50, 0);
    state.focus = FocusPane::Main;
    state.main_scroll.vertical_offset = 5;

    let new_state = handle_scroll_action(state, KeyAction::PageDown, 20);

    assert_eq!(
        new_state.main_scroll.vertical_offset, 25,
        "PageDown should move by viewport_height (5 + 20 = 25)"
    );
}

#[test]
fn page_down_clamps_to_max_entries() {
    let mut state = create_test_state_with_entries(10, 0);
    state.focus = FocusPane::Main;
    state.main_scroll.vertical_offset = 5;

    let new_state = handle_scroll_action(state, KeyAction::PageDown, 20);

    assert_eq!(
        new_state.main_scroll.vertical_offset, 9,
        "PageDown should clamp to max_entries (len - 1 = 9)"
    );
}

// ===== PageUp tests =====

#[test]
fn page_up_moves_by_viewport_height() {
    let mut state = create_test_state_with_entries(50, 0);
    state.focus = FocusPane::Main;
    state.main_scroll.vertical_offset = 25;

    let new_state = handle_scroll_action(state, KeyAction::PageUp, 20);

    assert_eq!(
        new_state.main_scroll.vertical_offset, 5,
        "PageUp should move by viewport_height (25 - 20 = 5)"
    );
}

#[test]
fn page_up_saturates_at_zero() {
    let mut state = create_test_state_with_entries(50, 0);
    state.focus = FocusPane::Main;
    state.main_scroll.vertical_offset = 10;

    let new_state = handle_scroll_action(state, KeyAction::PageUp, 20);

    assert_eq!(
        new_state.main_scroll.vertical_offset, 0,
        "PageUp should saturate at 0 when moving past start"
    );
}

// ===== ScrollToTop tests =====

#[test]
fn scroll_to_top_sets_offset_to_zero() {
    let mut state = create_test_state_with_entries(20, 0);
    state.focus = FocusPane::Main;
    state.main_scroll.vertical_offset = 15;

    let new_state = handle_scroll_action(state, KeyAction::ScrollToTop, 10);

    assert_eq!(
        new_state.main_scroll.vertical_offset, 0,
        "ScrollToTop should set vertical_offset to 0"
    );
}

#[test]
fn scroll_to_top_targets_subagent_when_focused() {
    let mut state = create_test_state_with_entries(5, 5);
    state.focus = FocusPane::Subagent;
    state.selected_tab = Some(0); // Select first subagent tab
    state.main_scroll.vertical_offset = 4;
    state.subagent_scroll.vertical_offset = 4;

    let new_state = handle_scroll_action(state, KeyAction::ScrollToTop, 10);

    assert_eq!(
        new_state.main_scroll.vertical_offset, 4,
        "Main scroll should be unchanged when Subagent focused"
    );
    assert_eq!(
        new_state.subagent_scroll.vertical_offset, 0,
        "Subagent scroll should be set to 0 when Subagent focused"
    );
}

// ===== ScrollToBottom tests =====

#[test]
fn scroll_to_bottom_sets_offset_to_max() {
    let mut state = create_test_state_with_entries(20, 0);
    state.focus = FocusPane::Main;
    state.main_scroll.vertical_offset = 5;

    let new_state = handle_scroll_action(state, KeyAction::ScrollToBottom, 10);

    assert_eq!(
        new_state.main_scroll.vertical_offset, 19,
        "ScrollToBottom should set vertical_offset to max_entries (len - 1)"
    );
}

#[test]
fn scroll_to_bottom_targets_subagent_when_focused() {
    let mut state = create_test_state_with_entries(5, 10);
    state.focus = FocusPane::Subagent;
    state.selected_tab = Some(0); // Select first subagent tab
    state.main_scroll.vertical_offset = 0;
    state.subagent_scroll.vertical_offset = 0;

    let new_state = handle_scroll_action(state, KeyAction::ScrollToBottom, 10);

    assert_eq!(
        new_state.main_scroll.vertical_offset, 0,
        "Main scroll should be unchanged when Subagent focused"
    );
    // Subagent has 10 entries, so max is 9
    assert_eq!(
        new_state.subagent_scroll.vertical_offset, 9,
        "Subagent scroll should be set to max when Subagent focused"
    );
}

// ===== Focus-awareness tests =====

#[test]
fn scroll_actions_ignored_when_focus_on_stats() {
    let mut state = create_test_state_with_entries(10, 0);
    state.focus = FocusPane::Stats;
    state.main_scroll.vertical_offset = 5;

    let new_state = handle_scroll_action(state, KeyAction::ScrollDown, 10);

    assert_eq!(
        new_state.main_scroll.vertical_offset, 5,
        "ScrollDown should be ignored when Stats pane has focus"
    );
}

#[test]
fn scroll_actions_ignored_when_focus_on_search() {
    let mut state = create_test_state_with_entries(10, 0);
    state.focus = FocusPane::Search;
    state.main_scroll.vertical_offset = 5;

    let new_state = handle_scroll_action(state, KeyAction::ScrollDown, 10);

    assert_eq!(
        new_state.main_scroll.vertical_offset, 5,
        "ScrollDown should be ignored when Search pane has focus"
    );
}

// ===== Non-scroll action tests =====

#[test]
fn non_scroll_actions_return_state_unchanged() {
    let state = create_test_state_with_entries(10, 0);
    let original_offset = state.main_scroll.vertical_offset;

    let new_state = handle_scroll_action(state, KeyAction::Quit, 10);

    assert_eq!(
        new_state.main_scroll.vertical_offset, original_offset,
        "Non-scroll actions should return state unchanged"
    );
}
