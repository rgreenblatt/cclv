//! Tab routing tests for scroll_handler.
//!
//! These tests verify that scroll actions route to the correct conversation
//! based on selected_tab, NOT focus. This ensures scroll parity with rendering.
//!
//! Bug: scroll_handler.rs previously used state.focus to route scroll actions,
//! but layout.rs uses state.selected_tab to determine which conversation to render.
//! This caused scroll actions to target the wrong conversation when focus != selected_tab.

use super::*;
use crate::model::{KeyAction, SessionId};
use crate::state::{AppState, FocusPane};
use crate::view_state::scroll::ScrollPosition;
use crate::view_state::types::LineOffset;

/// Helper to create test AppState with main agent and multiple subagents
fn create_test_state_with_multiple_agents(
    main_entries: usize,
    subagent1_entries: usize,
    subagent2_entries: usize,
) -> AppState {
    let mut entries = Vec::new();

    // Add main agent entries
    for i in 0..main_entries {
        let entry = create_test_log_entry(format!("main-{}", i), None);
        entries.push(crate::model::ConversationEntry::Valid(Box::new(entry)));
    }

    // Add subagent 1 entries
    if subagent1_entries > 0 {
        let agent_id = crate::model::AgentId::new("subagent-1").unwrap();
        for i in 0..subagent1_entries {
            let entry = create_test_log_entry(format!("sub1-{}", i), Some(agent_id.clone()));
            entries.push(crate::model::ConversationEntry::Valid(Box::new(entry)));
        }
    }

    // Add subagent 2 entries
    if subagent2_entries > 0 {
        let agent_id = crate::model::AgentId::new("subagent-2").unwrap();
        for i in 0..subagent2_entries {
            let entry = create_test_log_entry(format!("sub2-{}", i), Some(agent_id.clone()));
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

    static COUNTER: AtomicU32 = AtomicU32::new(1000);
    let id = COUNTER.fetch_add(1, Ordering::Relaxed);

    let message = Message::new(Role::User, MessageContent::Text(content));

    crate::model::LogEntry::new(
        EntryUuid::new(format!("tab-routing-uuid-{}", id)).unwrap(),
        None,
        SessionId::new("test-session").unwrap(),
        agent_id,
        Utc::now(),
        EntryType::User,
        message,
        EntryMetadata::default(),
    )
}

// ===== Test: Scroll actions target selected_tab, not focus =====

#[test]
fn scroll_routes_to_selected_tab_0_main_agent() {
    let mut state = create_test_state_with_multiple_agents(5, 3, 3);

    // Setup: selected_tab = 0 (main agent), but focus = Subagent
    // This is the bug case: focus != selected_tab
    state.selected_tab = Some(0);
    state.focus = FocusPane::Subagent;

    // Set main agent scroll to line 10
    state
        .log_view_mut()
        .current_session_mut()
        .unwrap()
        .main_mut()
        .set_scroll(ScrollPosition::AtLine(LineOffset::new(10)));

    // Scroll down should target MAIN agent (selected_tab=0), NOT subagent (focus)
    let new_state = handle_scroll_action(state, KeyAction::ScrollDown, 10);

    // Verify: Main agent scroll position changed
    let main_scroll = new_state
        .log_view()
        .current_session()
        .unwrap()
        .main()
        .scroll();

    match main_scroll {
        ScrollPosition::AtLine(offset) => {
            assert_eq!(
                offset.get(),
                11,
                "Scroll should target selected_tab=0 (main agent), not focus pane"
            );
        }
        _ => panic!("Expected ScrollPosition::AtLine for main agent"),
    }
}

#[test]
fn scroll_routes_to_selected_tab_1_subagent() {
    let mut state = create_test_state_with_multiple_agents(5, 3, 3);

    // Setup: selected_tab = 1 (first subagent), but focus = Main
    state.selected_tab = Some(1);
    state.focus = FocusPane::Main;

    // Get the first subagent ID
    let subagent_ids: Vec<_> = state.session_view().subagent_ids().cloned().collect();
    let first_subagent_id = subagent_ids[0].clone();

    // Set first subagent scroll to line 5
    state
        .log_view_mut()
        .current_session_mut()
        .unwrap()
        .subagent_mut(&first_subagent_id)
        .set_scroll(ScrollPosition::AtLine(LineOffset::new(5)));

    // Scroll down should target FIRST subagent (selected_tab=1), NOT main (focus)
    let new_state = handle_scroll_action(state, KeyAction::ScrollDown, 10);

    // Verify: First subagent scroll position changed
    let subagent_scroll = new_state
        .session_view()
        .get_subagent(&first_subagent_id)
        .unwrap()
        .scroll();

    match subagent_scroll {
        ScrollPosition::AtLine(offset) => {
            assert_eq!(
                offset.get(),
                6,
                "Scroll should target selected_tab=1 (first subagent), not focus pane"
            );
        }
        _ => panic!("Expected ScrollPosition::AtLine for subagent"),
    }
}

#[test]
fn scroll_routes_to_selected_tab_2_second_subagent() {
    let mut state = create_test_state_with_multiple_agents(5, 3, 3);

    // Setup: selected_tab = 2 (second subagent)
    state.selected_tab = Some(2);
    state.focus = FocusPane::Main;

    // Get the second subagent ID (index 1 in subagent list)
    let subagent_ids: Vec<_> = state.session_view().subagent_ids().cloned().collect();
    let second_subagent_id = subagent_ids[1].clone();

    // Set second subagent scroll to line 8
    state
        .log_view_mut()
        .current_session_mut()
        .unwrap()
        .subagent_mut(&second_subagent_id)
        .set_scroll(ScrollPosition::AtLine(LineOffset::new(8)));

    // Scroll down should target SECOND subagent (selected_tab=2)
    let new_state = handle_scroll_action(state, KeyAction::ScrollDown, 10);

    // Verify: Second subagent scroll position changed
    let subagent_scroll = new_state
        .session_view()
        .get_subagent(&second_subagent_id)
        .unwrap()
        .scroll();

    match subagent_scroll {
        ScrollPosition::AtLine(offset) => {
            assert_eq!(
                offset.get(),
                9,
                "Scroll should target selected_tab=2 (second subagent)"
            );
        }
        _ => panic!("Expected ScrollPosition::AtLine for subagent"),
    }
}

// ===== Test: Scroll independence - scrolling one tab doesn't affect others =====

#[test]
fn scrolling_tab_0_does_not_affect_tab_1() {
    let mut state = create_test_state_with_multiple_agents(5, 3, 3);

    // Setup: selected_tab = 0 (main agent)
    state.selected_tab = Some(0);
    state.focus = FocusPane::Main;

    // Set initial scroll positions
    state
        .log_view_mut()
        .current_session_mut()
        .unwrap()
        .main_mut()
        .set_scroll(ScrollPosition::AtLine(LineOffset::new(10)));

    let subagent_ids: Vec<_> = state.session_view().subagent_ids().cloned().collect();
    let first_subagent_id = subagent_ids[0].clone();

    state
        .log_view_mut()
        .current_session_mut()
        .unwrap()
        .subagent_mut(&first_subagent_id)
        .set_scroll(ScrollPosition::AtLine(LineOffset::new(20)));

    // Scroll main agent (tab 0)
    let new_state = handle_scroll_action(state, KeyAction::ScrollDown, 10);

    // Verify: Main agent scrolled
    let main_scroll = new_state
        .log_view()
        .current_session()
        .unwrap()
        .main()
        .scroll();
    match main_scroll {
        ScrollPosition::AtLine(offset) => {
            assert_eq!(offset.get(), 11, "Main agent should have scrolled");
        }
        _ => panic!("Expected AtLine for main"),
    }

    // Verify: Subagent 1 scroll unchanged
    let sub1_scroll = new_state
        .session_view()
        .get_subagent(&first_subagent_id)
        .unwrap()
        .scroll();
    match sub1_scroll {
        ScrollPosition::AtLine(offset) => {
            assert_eq!(
                offset.get(),
                20,
                "Subagent 1 scroll should be unchanged when scrolling tab 0"
            );
        }
        _ => panic!("Expected AtLine for subagent 1"),
    }
}

#[test]
fn scrolling_tab_1_does_not_affect_tab_0() {
    let mut state = create_test_state_with_multiple_agents(5, 3, 3);

    // Setup: selected_tab = 1 (first subagent)
    state.selected_tab = Some(1);
    state.focus = FocusPane::Subagent;

    // Set initial scroll positions
    state
        .log_view_mut()
        .current_session_mut()
        .unwrap()
        .main_mut()
        .set_scroll(ScrollPosition::AtLine(LineOffset::new(15)));

    let subagent_ids: Vec<_> = state.session_view().subagent_ids().cloned().collect();
    let first_subagent_id = subagent_ids[0].clone();

    state
        .log_view_mut()
        .current_session_mut()
        .unwrap()
        .subagent_mut(&first_subagent_id)
        .set_scroll(ScrollPosition::AtLine(LineOffset::new(25)));

    // Scroll first subagent (tab 1)
    let new_state = handle_scroll_action(state, KeyAction::ScrollDown, 10);

    // Verify: Subagent scrolled
    let sub1_scroll = new_state
        .session_view()
        .get_subagent(&first_subagent_id)
        .unwrap()
        .scroll();
    match sub1_scroll {
        ScrollPosition::AtLine(offset) => {
            assert_eq!(offset.get(), 26, "Subagent should have scrolled");
        }
        _ => panic!("Expected AtLine for subagent"),
    }

    // Verify: Main agent scroll unchanged
    let main_scroll = new_state
        .log_view()
        .current_session()
        .unwrap()
        .main()
        .scroll();
    match main_scroll {
        ScrollPosition::AtLine(offset) => {
            assert_eq!(
                offset.get(),
                15,
                "Main agent scroll should be unchanged when scrolling tab 1"
            );
        }
        _ => panic!("Expected AtLine for main"),
    }
}

// ===== Test: Scroll preservation when switching tabs =====

#[test]
fn scroll_position_preserved_when_switching_tabs() {
    let mut state = create_test_state_with_multiple_agents(5, 3, 3);

    // Setup: Start at tab 0, scroll it
    state.selected_tab = Some(0);
    state.focus = FocusPane::Main;

    state
        .log_view_mut()
        .current_session_mut()
        .unwrap()
        .main_mut()
        .set_scroll(ScrollPosition::AtLine(LineOffset::new(10)));

    // Scroll main agent
    let state = handle_scroll_action(state, KeyAction::ScrollDown, 10);

    // Verify main scrolled to 11
    let main_scroll = state
        .log_view()
        .current_session()
        .unwrap()
        .main()
        .scroll();
    match main_scroll {
        ScrollPosition::AtLine(offset) => {
            assert_eq!(offset.get(), 11);
        }
        _ => panic!("Expected AtLine"),
    }

    // Switch to tab 1 (simulate user pressing Tab key)
    let mut state = state;
    state.selected_tab = Some(1);

    // Scroll tab 1
    let state = handle_scroll_action(state, KeyAction::ScrollDown, 10);

    // Switch back to tab 0
    let mut state = state;
    state.selected_tab = Some(0);

    // Verify: Main agent scroll position is STILL 11 (preserved)
    let main_scroll = state
        .log_view()
        .current_session()
        .unwrap()
        .main()
        .scroll();
    match main_scroll {
        ScrollPosition::AtLine(offset) => {
            assert_eq!(
                offset.get(),
                11,
                "Main agent scroll should be preserved when switching tabs"
            );
        }
        _ => panic!("Expected AtLine for main"),
    }
}

// ===== Test: Edge case - selected_tab is None =====

#[test]
fn scroll_defaults_to_main_when_selected_tab_is_none() {
    let mut state = create_test_state_with_multiple_agents(5, 3, 3);

    // Setup: selected_tab = None (should default to tab 0 = main agent)
    state.selected_tab = None;
    state.focus = FocusPane::Main;

    // Set main agent scroll to line 5
    state
        .log_view_mut()
        .current_session_mut()
        .unwrap()
        .main_mut()
        .set_scroll(ScrollPosition::AtLine(LineOffset::new(5)));

    // Scroll down
    let new_state = handle_scroll_action(state, KeyAction::ScrollDown, 10);

    // Verify: Main agent scroll changed (default behavior)
    let main_scroll = new_state
        .log_view()
        .current_session()
        .unwrap()
        .main()
        .scroll();

    match main_scroll {
        ScrollPosition::AtLine(offset) => {
            assert_eq!(
                offset.get(),
                6,
                "When selected_tab is None, should default to main agent (tab 0)"
            );
        }
        _ => panic!("Expected ScrollPosition::AtLine for main agent"),
    }
}
