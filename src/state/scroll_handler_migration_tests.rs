//! Migration tests for ScrollPosition-based scrolling.
//!
//! These tests verify that scroll_handler uses ConversationViewState.set_scroll()
//! with ScrollPosition variants instead of directly modifying vertical_offset.
//!
//! This is the NEW behavior after removing vertical_offset from ScrollState.

use super::*;
use crate::model::{KeyAction, SessionId};
use crate::state::{AppState, FocusPane};
use crate::view_state::scroll::ScrollPosition;
use crate::view_state::types::LineOffset;

/// Helper to create test AppState with populated log_view
fn create_test_state_with_log_view(main_entries: usize, subagent_entries: usize) -> AppState {
    let mut entries = Vec::new();

    // Add main agent entries
    for i in 0..main_entries {
        let entry = create_test_log_entry(format!("main-{}", i), None);
        entries.push(crate::model::ConversationEntry::Valid(Box::new(entry)));
    }

    // Add subagent entries
    if subagent_entries > 0 {
        let agent_id = crate::model::AgentId::new("test-agent").unwrap();
        for i in 0..subagent_entries {
            let entry = create_test_log_entry(format!("sub-{}", i), Some(agent_id.clone()));
            entries.push(crate::model::ConversationEntry::Valid(Box::new(entry)));
        }
    }

    let mut state = AppState::new();
    state.add_entries(entries);
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

// ===== ScrollUp tests (ScrollPosition-based) =====

#[test]
fn scroll_up_uses_scroll_position_at_line() {
    let mut state = create_test_state_with_log_view(5, 0);
    state.focus = FocusPane::Main;

    // Set initial scroll position to line 10
    state
        .log_view_mut()
        .current_session_mut()
        .unwrap()
        .main_mut()
        .set_scroll(ScrollPosition::AtLine(LineOffset::new(10)));

    let viewport = crate::view_state::types::ViewportDimensions::new(80, 10);
    handle_scroll_action(&mut state, KeyAction::ScrollUp, viewport);

    // Should scroll up by 1 line (10 -> 9)
    let scroll = state
        .log_view()
        .current_session()
        .unwrap()
        .main()
        .scroll();

    match scroll {
        ScrollPosition::AtLine(offset) => {
            assert_eq!(offset.get(), 9, "ScrollUp should decrement by 1 line");
        }
        _ => panic!("Expected ScrollPosition::AtLine after ScrollUp"),
    }
}

#[test]
fn scroll_up_saturates_at_top() {
    let mut state = create_test_state_with_log_view(5, 0);
    state.focus = FocusPane::Main;

    // Set initial scroll position to line 0
    state
        .log_view_mut()
        .current_session_mut()
        .unwrap()
        .main_mut()
        .set_scroll(ScrollPosition::AtLine(LineOffset::new(0)));

    let viewport = crate::view_state::types::ViewportDimensions::new(80, 10);
    handle_scroll_action(&mut state, KeyAction::ScrollUp, viewport);

    // Should stay at line 0
    let scroll = state
        .log_view()
        .current_session()
        .unwrap()
        .main()
        .scroll();

    match scroll {
        ScrollPosition::AtLine(offset) => {
            assert_eq!(offset.get(), 0, "ScrollUp from 0 should stay at 0");
        }
        _ => panic!("Expected ScrollPosition::AtLine"),
    }
}

// ===== ScrollDown tests (ScrollPosition-based) =====

#[test]
fn scroll_down_uses_scroll_position_at_line() {
    let mut state = create_test_state_with_log_view(10, 0);
    state.focus = FocusPane::Main;

    // Set initial scroll position to line 5
    state
        .log_view_mut()
        .current_session_mut()
        .unwrap()
        .main_mut()
        .set_scroll(ScrollPosition::AtLine(LineOffset::new(5)));

    let viewport = crate::view_state::types::ViewportDimensions::new(80, 10);
    handle_scroll_action(&mut state, KeyAction::ScrollDown, viewport);

    // Should scroll down by 1 line (5 -> 6)
    let scroll = state
        .log_view()
        .current_session()
        .unwrap()
        .main()
        .scroll();

    match scroll {
        ScrollPosition::AtLine(offset) => {
            assert_eq!(offset.get(), 6, "ScrollDown should increment by 1 line");
        }
        _ => panic!("Expected ScrollPosition::AtLine after ScrollDown"),
    }
}

// ===== PageDown tests (ScrollPosition-based) =====

#[test]
fn page_down_uses_scroll_position_with_viewport_offset() {
    let mut state = create_test_state_with_log_view(50, 0);
    state.focus = FocusPane::Main;

    // Set initial scroll position to line 5
    state
        .log_view_mut()
        .current_session_mut()
        .unwrap()
        .main_mut()
        .set_scroll(ScrollPosition::AtLine(LineOffset::new(5)));

    let viewport = crate::view_state::types::ViewportDimensions::new(80, 20);
    handle_scroll_action(&mut state, KeyAction::PageDown, viewport);

    // Should move by viewport_height (5 + 20 = 25)
    let scroll = state
        .log_view()
        .current_session()
        .unwrap()
        .main()
        .scroll();

    match scroll {
        ScrollPosition::AtLine(offset) => {
            assert_eq!(offset.get(), 25, "PageDown should move by viewport_height");
        }
        _ => panic!("Expected ScrollPosition::AtLine after PageDown"),
    }
}

// ===== PageUp tests (ScrollPosition-based) =====

#[test]
fn page_up_uses_scroll_position_with_viewport_offset() {
    let mut state = create_test_state_with_log_view(50, 0);
    state.focus = FocusPane::Main;

    // Set initial scroll position to line 25
    state
        .log_view_mut()
        .current_session_mut()
        .unwrap()
        .main_mut()
        .set_scroll(ScrollPosition::AtLine(LineOffset::new(25)));

    let viewport = crate::view_state::types::ViewportDimensions::new(80, 20);
    handle_scroll_action(&mut state, KeyAction::PageUp, viewport);

    // Should move by viewport_height (25 - 20 = 5)
    let scroll = state
        .log_view()
        .current_session()
        .unwrap()
        .main()
        .scroll();

    match scroll {
        ScrollPosition::AtLine(offset) => {
            assert_eq!(offset.get(), 5, "PageUp should move by viewport_height");
        }
        _ => panic!("Expected ScrollPosition::AtLine after PageUp"),
    }
}

// ===== ScrollToTop tests (ScrollPosition-based) =====

#[test]
fn scroll_to_top_uses_scroll_position_top() {
    let mut state = create_test_state_with_log_view(20, 0);
    state.focus = FocusPane::Main;

    // Set initial scroll position to line 15
    state
        .log_view_mut()
        .current_session_mut()
        .unwrap()
        .main_mut()
        .set_scroll(ScrollPosition::AtLine(LineOffset::new(15)));

    let viewport = crate::view_state::types::ViewportDimensions::new(80, 10);
    handle_scroll_action(&mut state, KeyAction::ScrollToTop, viewport);

    // Should use ScrollPosition::Top
    let scroll = state
        .log_view()
        .current_session()
        .unwrap()
        .main()
        .scroll();

    assert_eq!(
        *scroll,
        ScrollPosition::Top,
        "ScrollToTop should use ScrollPosition::Top"
    );
}

// ===== ScrollToBottom tests (ScrollPosition-based) =====

#[test]
fn scroll_to_bottom_uses_scroll_position_bottom() {
    let mut state = create_test_state_with_log_view(20, 0);
    state.focus = FocusPane::Main;

    // Set initial scroll position to line 5
    state
        .log_view_mut()
        .current_session_mut()
        .unwrap()
        .main_mut()
        .set_scroll(ScrollPosition::AtLine(LineOffset::new(5)));

    let viewport = crate::view_state::types::ViewportDimensions::new(80, 10);
    handle_scroll_action(&mut state, KeyAction::ScrollToBottom, viewport);

    // Should use ScrollPosition::Bottom
    let scroll = state
        .log_view()
        .current_session()
        .unwrap()
        .main()
        .scroll();

    assert_eq!(
        *scroll,
        ScrollPosition::Bottom,
        "ScrollToBottom should use ScrollPosition::Bottom"
    );
}

// ===== Critical: No max_entries calculations =====

#[test]
fn scroll_position_not_bounded_by_entry_count() {
    let mut state = create_test_state_with_log_view(5, 0); // Only 5 entries
    state.focus = FocusPane::Main;

    // Set initial scroll position to line 100 (way beyond entry count)
    // This would have been clamped by old max_entries logic (the BUG)
    state
        .log_view_mut()
        .current_session_mut()
        .unwrap()
        .main_mut()
        .set_scroll(ScrollPosition::AtLine(LineOffset::new(100)));

    let viewport = crate::view_state::types::ViewportDimensions::new(80, 10);
    handle_scroll_action(&mut state, KeyAction::ScrollDown, viewport);

    // Should still use line-based offset, NOT clamped to entry count
    let scroll = state
        .log_view()
        .current_session()
        .unwrap()
        .main()
        .scroll();

    match scroll {
        ScrollPosition::AtLine(offset) => {
            assert_eq!(
                offset.get(),
                101,
                "Scroll should use LINE offset, not entry count"
            );
        }
        _ => panic!("Expected ScrollPosition::AtLine"),
    }
}
