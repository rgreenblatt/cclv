//! Tests for FR-036: Auto-scroll pause when user scrolls away from bottom.
//!
//! These tests verify that:
//! 1. When user scrolls away from bottom (Up/Down/PgUp/PgDn/Home), auto_scroll becomes false
//! 2. When user is at bottom (or presses End), auto_scroll remains/becomes true
//! 3. This behavior enables proper "follow" mode in live streaming

use super::*;
use crate::model::{KeyAction, SessionId};
use crate::state::app_state::WrapMode;
use crate::state::{AppState, FocusPane};
use crate::view_state::layout_params::LayoutParams;
use crate::view_state::scroll::ScrollPosition;
use crate::view_state::types::LineOffset;

/// Helper to create test AppState with populated log_view
fn create_test_state_with_entries(num_entries: usize) -> AppState {
    let mut entries = Vec::new();

    for i in 0..num_entries {
        let entry = create_test_log_entry(format!("Entry {} content", i), None);
        entries.push(crate::model::ConversationEntry::Valid(Box::new(entry)));
    }

    let mut state = AppState::new();
    state.add_entries(entries);

    // Compute layout so we have real heights
    let params = LayoutParams::new(80, WrapMode::Wrap);
    state
        .log_view_mut()
        .current_session_mut()
        .unwrap()
        .main_mut()
        .recompute_layout(params);

    state
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

// ===== FR-036: Auto-scroll becomes false when scrolling away from bottom =====

#[test]
fn scroll_up_from_bottom_disables_auto_scroll() {
    let mut state = create_test_state_with_entries(20);
    state.focus = FocusPane::Main;
    state.auto_scroll = true;

    // Start at bottom
    state
        .log_view_mut()
        .current_session_mut()
        .unwrap()
        .main_mut()
        .set_scroll(ScrollPosition::Bottom);

    // Scroll up by 1 line
    let viewport = crate::view_state::types::ViewportDimensions::new(80, 24);
    let new_state = handle_scroll_action(state, KeyAction::ScrollUp, viewport);

    // FR-036: Auto-scroll should be disabled when user scrolls away from bottom
    assert!(
        !new_state.auto_scroll,
        "auto_scroll should be false after scrolling up from bottom"
    );
}

#[test]
fn scroll_up_from_middle_disables_auto_scroll() {
    let mut state = create_test_state_with_entries(50);
    state.focus = FocusPane::Main;
    state.auto_scroll = true;

    // Set scroll position to middle (not at bottom)
    state
        .log_view_mut()
        .current_session_mut()
        .unwrap()
        .main_mut()
        .set_scroll(ScrollPosition::AtLine(LineOffset::new(50)));

    // Scroll up by 1 line
    let viewport = crate::view_state::types::ViewportDimensions::new(80, 24);
    let new_state = handle_scroll_action(state, KeyAction::ScrollUp, viewport);

    // FR-036: Auto-scroll should be disabled when scrolling away from bottom
    assert!(
        !new_state.auto_scroll,
        "auto_scroll should be false after scrolling up from middle"
    );
}

#[test]
fn scroll_down_not_at_bottom_disables_auto_scroll() {
    let mut state = create_test_state_with_entries(50);
    state.focus = FocusPane::Main;
    state.auto_scroll = true;

    // Set scroll position to middle (not at bottom)
    state
        .log_view_mut()
        .current_session_mut()
        .unwrap()
        .main_mut()
        .set_scroll(ScrollPosition::AtLine(LineOffset::new(20)));

    // Scroll down by 1 line (still not at bottom)
    let viewport = crate::view_state::types::ViewportDimensions::new(80, 24);
    let new_state = handle_scroll_action(state, KeyAction::ScrollDown, viewport);

    // FR-036: Auto-scroll should be disabled when user scrolls (even down, if not at bottom)
    assert!(
        !new_state.auto_scroll,
        "auto_scroll should be false after scrolling down (not reaching bottom)"
    );
}

#[test]
fn page_up_disables_auto_scroll() {
    let mut state = create_test_state_with_entries(100);
    state.focus = FocusPane::Main;
    state.auto_scroll = true;

    // Start at bottom
    state
        .log_view_mut()
        .current_session_mut()
        .unwrap()
        .main_mut()
        .set_scroll(ScrollPosition::Bottom);

    // Page up
    let viewport = crate::view_state::types::ViewportDimensions::new(80, 24);
    let new_state = handle_scroll_action(state, KeyAction::PageUp, viewport);

    // FR-036: Auto-scroll should be disabled
    assert!(
        !new_state.auto_scroll,
        "auto_scroll should be false after PageUp"
    );
}

#[test]
fn page_down_not_reaching_bottom_disables_auto_scroll() {
    let mut state = create_test_state_with_entries(100);
    state.focus = FocusPane::Main;
    state.auto_scroll = true;

    // Start at top
    state
        .log_view_mut()
        .current_session_mut()
        .unwrap()
        .main_mut()
        .set_scroll(ScrollPosition::Top);

    // Page down (won't reach bottom with 100 entries)
    let viewport = crate::view_state::types::ViewportDimensions::new(80, 24);
    let new_state = handle_scroll_action(state, KeyAction::PageDown, viewport);

    // FR-036: Auto-scroll should be disabled
    assert!(
        !new_state.auto_scroll,
        "auto_scroll should be false after PageDown not reaching bottom"
    );
}

#[test]
fn home_key_disables_auto_scroll() {
    let mut state = create_test_state_with_entries(50);
    state.focus = FocusPane::Main;
    state.auto_scroll = true;

    // Start at bottom
    state
        .log_view_mut()
        .current_session_mut()
        .unwrap()
        .main_mut()
        .set_scroll(ScrollPosition::Bottom);

    // Press Home to jump to top
    let viewport = crate::view_state::types::ViewportDimensions::new(80, 24);
    let new_state = handle_scroll_action(state, KeyAction::ScrollToTop, viewport);

    // FR-036: Auto-scroll should be disabled
    assert!(
        !new_state.auto_scroll,
        "auto_scroll should be false after Home (jump to top)"
    );
}

// ===== FR-036: Auto-scroll remains/becomes true when at bottom =====

#[test]
fn end_key_enables_auto_scroll() {
    let mut state = create_test_state_with_entries(50);
    state.focus = FocusPane::Main;
    state.auto_scroll = false; // Start with auto_scroll disabled

    // Press End to jump to bottom
    let viewport = crate::view_state::types::ViewportDimensions::new(80, 24);
    let new_state = handle_scroll_action(state, KeyAction::ScrollToBottom, viewport);

    // FR-036: Auto-scroll should be re-enabled when user goes to bottom
    assert!(
        new_state.auto_scroll,
        "auto_scroll should be true after End (jump to bottom)"
    );
}

#[test]
fn scroll_down_reaching_bottom_enables_auto_scroll() {
    let mut state = create_test_state_with_entries(10); // Small number for easy "reach bottom"
    state.focus = FocusPane::Main;
    state.auto_scroll = false;

    // Get total height to position just above bottom
    let total_height = state
        .log_view()
        .current_session()
        .unwrap()
        .main()
        .total_height();
    let viewport_height = 24;

    // Set scroll position just above bottom (so one scroll down will reach it)
    let near_bottom = total_height
        .saturating_sub(viewport_height)
        .saturating_sub(1);
    state
        .log_view_mut()
        .current_session_mut()
        .unwrap()
        .main_mut()
        .set_scroll(ScrollPosition::AtLine(LineOffset::new(near_bottom)));

    // Scroll down to reach bottom
    let viewport = crate::view_state::types::ViewportDimensions::new(80, viewport_height as u16);
    let new_state = handle_scroll_action(state, KeyAction::ScrollDown, viewport);

    // FR-036: Auto-scroll should be enabled when reaching bottom
    assert!(
        new_state.auto_scroll,
        "auto_scroll should be true after scrolling down to bottom"
    );
}

#[test]
fn already_at_bottom_keeps_auto_scroll_true() {
    let mut state = create_test_state_with_entries(20);
    state.focus = FocusPane::Main;
    state.auto_scroll = true;

    // Start at bottom
    state
        .log_view_mut()
        .current_session_mut()
        .unwrap()
        .main_mut()
        .set_scroll(ScrollPosition::Bottom);

    // Try to scroll down (no-op since already at bottom)
    let viewport = crate::view_state::types::ViewportDimensions::new(80, 24);
    let new_state = handle_scroll_action(state, KeyAction::ScrollDown, viewport);

    // FR-036: Auto-scroll should remain true when already at bottom
    assert!(
        new_state.auto_scroll,
        "auto_scroll should remain true when already at bottom"
    );
}
