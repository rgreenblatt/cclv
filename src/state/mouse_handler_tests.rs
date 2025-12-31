//! Tests for mouse event handling.

use super::*;
use crate::model::{
    AgentId, ConversationEntry, EntryMetadata, EntryType, EntryUuid, LogEntry, Message,
    MessageContent, Role, Session, SessionId,
};
use crate::state::{AppState, FocusPane};
use chrono::Utc;
use ratatui::layout::Rect;

// ===== Test Helpers =====

fn agent_id(s: &str) -> AgentId {
    AgentId::new(s).unwrap()
}

fn make_session_id(s: &str) -> SessionId {
    SessionId::new(s).expect("valid session id")
}

fn make_entry_uuid(s: &str) -> EntryUuid {
    EntryUuid::new(s).expect("valid uuid")
}

fn make_subagent_entry(agent_id: &str) -> ConversationEntry {
    let log_entry = LogEntry::new(
        make_entry_uuid(&format!("entry-{}", agent_id)),
        None,
        make_session_id("test-session"),
        Some(AgentId::new(agent_id).expect("valid agent id")),
        Utc::now(),
        EntryType::Assistant,
        Message::new(
            Role::Assistant,
            MessageContent::Text("Test message".to_string()),
        ),
        EntryMetadata::default(),
    );

    ConversationEntry::Valid(Box::new(log_entry))
}

fn create_app_state_with_tabs(agent_ids: Vec<&str>) -> AppState {
    let mut session = Session::new(make_session_id("test-session"));

    // Add a conversation entry for each subagent (this creates the subagent)
    for id in agent_ids {
        session.add_conversation_entry(make_subagent_entry(id));
    }

    AppState::new(session)
}

// ===== detect_tab_click Tests =====

#[test]
fn detect_tab_click_returns_no_tab_when_click_outside_bounds() {
    let agent1 = agent_id("agent-1");
    let agent_ids = vec![&agent1];

    // Tab area at (0, 0) with width 20, height 3
    let tab_area = Rect::new(0, 0, 20, 3);

    // Click outside the area (x=25, y=5)
    let result = detect_tab_click(25, 5, tab_area, &agent_ids);

    assert_eq!(
        result,
        TabClickResult::NoTab,
        "Click outside tab area should return NoTab"
    );
}

#[test]
fn detect_tab_click_returns_no_tab_when_click_before_tab_area() {
    let agent1 = agent_id("agent-1");
    let agent_ids = vec![&agent1];

    // Tab area starts at x=10
    let tab_area = Rect::new(10, 0, 20, 3);

    // Click before the area (x=5, y=1)
    let result = detect_tab_click(5, 1, tab_area, &agent_ids);

    assert_eq!(
        result,
        TabClickResult::NoTab,
        "Click before tab area should return NoTab"
    );
}

#[test]
fn detect_tab_click_detects_first_tab_in_single_tab_scenario() {
    let agent1 = agent_id("agent-1");
    let agent_ids = vec![&agent1];

    // Tab area at (0, 0) with width 40, height 3
    let tab_area = Rect::new(0, 0, 40, 3);

    // Click in the middle of the tab bar (x=20, y=1)
    // With only one tab, any click within bounds should hit it
    let result = detect_tab_click(20, 1, tab_area, &agent_ids);

    assert_eq!(
        result,
        TabClickResult::TabClicked(0),
        "Click within single tab should return index 0"
    );
}

#[test]
fn detect_tab_click_detects_second_tab_when_multiple_tabs() {
    let agent1 = agent_id("agent-1");
    let agent2 = agent_id("agent-2");
    let agent3 = agent_id("agent-3");
    let agent_ids = vec![&agent1, &agent2, &agent3];

    // Tab area with width 60 (each tab gets ~20 chars)
    let tab_area = Rect::new(0, 0, 60, 3);

    // Click in the second tab's area (roughly x=25, y=1)
    // This assumes each tab takes equal width
    let result = detect_tab_click(25, 1, tab_area, &agent_ids);

    assert_eq!(
        result,
        TabClickResult::TabClicked(1),
        "Click in second tab area should return index 1"
    );
}

#[test]
fn detect_tab_click_detects_first_tab_at_left_edge() {
    let agent1 = agent_id("agent-1");
    let agent2 = agent_id("agent-2");
    let agent_ids = vec![&agent1, &agent2];

    let tab_area = Rect::new(0, 0, 40, 3);

    // Click at the very start of first tab (x=1, y=1)
    // Note: x=0 might be the border
    let result = detect_tab_click(1, 1, tab_area, &agent_ids);

    assert_eq!(
        result,
        TabClickResult::TabClicked(0),
        "Click at left edge should hit first tab"
    );
}

#[test]
fn detect_tab_click_detects_last_tab_at_right_edge() {
    let agent1 = agent_id("agent-1");
    let agent2 = agent_id("agent-2");
    let agent_ids = vec![&agent1, &agent2];

    let tab_area = Rect::new(0, 0, 40, 3);

    // Click near the right edge (x=38, y=1)
    let result = detect_tab_click(38, 1, tab_area, &agent_ids);

    assert_eq!(
        result,
        TabClickResult::TabClicked(1),
        "Click at right edge should hit last tab"
    );
}

#[test]
fn detect_tab_click_works_with_empty_agent_list() {
    let agent_ids: Vec<&AgentId> = vec![];

    let tab_area = Rect::new(0, 0, 40, 3);

    // Any click with no tabs should return NoTab
    let result = detect_tab_click(20, 1, tab_area, &agent_ids);

    assert_eq!(
        result,
        TabClickResult::NoTab,
        "Click with no tabs should return NoTab"
    );
}

#[test]
fn detect_tab_click_respects_vertical_bounds() {
    let agent1 = agent_id("agent-1");
    let agent_ids = vec![&agent1];

    // Tab area height is 3, from y=0 to y=2
    let tab_area = Rect::new(0, 0, 40, 3);

    // Click below the tab area (y=5)
    let result = detect_tab_click(20, 5, tab_area, &agent_ids);

    assert_eq!(
        result,
        TabClickResult::NoTab,
        "Click below tab area should return NoTab"
    );
}

// ===== handle_mouse_click Tests =====

#[test]
fn handle_mouse_click_switches_to_clicked_tab() {
    let state = create_app_state_with_tabs(vec!["agent-1", "agent-2", "agent-3"]);

    // Initially select first tab (need to set focus first)
    let mut state = state;
    state.focus = crate::state::FocusPane::Subagent;
    state.select_tab(1); // 1-indexed
    assert_eq!(state.selected_tab, Some(0));

    // Tab area
    let tab_area = Rect::new(0, 0, 60, 3);

    // Click on second tab (assume x=25 is in second tab)
    let updated_state = handle_mouse_click(state, 25, 1, tab_area);

    assert_eq!(
        updated_state.selected_tab,
        Some(1),
        "Clicking second tab should switch selection to index 1"
    );
}

#[test]
fn handle_mouse_click_preserves_state_when_clicking_outside_tabs() {
    let state = create_app_state_with_tabs(vec!["agent-1", "agent-2"]);
    let mut state = state;
    state.focus = crate::state::FocusPane::Subagent;
    state.select_tab(1); // 1-indexed

    let tab_area = Rect::new(0, 0, 40, 3);

    // Click outside the tab area
    let updated_state = handle_mouse_click(state.clone(), 100, 1, tab_area);

    assert_eq!(
        updated_state.selected_tab, state.selected_tab,
        "Clicking outside tabs should preserve selection"
    );
}

#[test]
fn handle_mouse_click_switches_from_none_to_first_tab() {
    let state = create_app_state_with_tabs(vec!["agent-1", "agent-2"]);

    // Start with no tab selected
    let mut state = state;
    state.selected_tab = None;

    let tab_area = Rect::new(0, 0, 40, 3);

    // Click on first tab
    let updated_state = handle_mouse_click(state, 5, 1, tab_area);

    assert_eq!(
        updated_state.selected_tab,
        Some(0),
        "Clicking first tab when none selected should select it"
    );
}

#[test]
fn handle_mouse_click_can_switch_to_last_tab() {
    let state = create_app_state_with_tabs(vec!["agent-1", "agent-2", "agent-3"]);
    let mut state = state;
    state.focus = crate::state::FocusPane::Subagent;
    state.select_tab(1); // 1-indexed

    let tab_area = Rect::new(0, 0, 60, 3);

    // Click on third/last tab (x=50)
    let updated_state = handle_mouse_click(state, 50, 1, tab_area);

    assert_eq!(
        updated_state.selected_tab,
        Some(2),
        "Should switch to third tab"
    );
}

#[test]
fn handle_mouse_click_with_no_tabs_preserves_state() {
    let state = create_app_state_with_tabs(vec![]);

    let tab_area = Rect::new(0, 0, 40, 3);

    // Click anywhere
    let updated_state = handle_mouse_click(state.clone(), 20, 1, tab_area);

    assert_eq!(
        updated_state.selected_tab, state.selected_tab,
        "With no tabs, state should be unchanged"
    );
}

#[test]
fn handle_mouse_click_clicking_same_tab_is_idempotent() {
    let state = create_app_state_with_tabs(vec!["agent-1", "agent-2"]);
    let mut state = state;
    state.focus = crate::state::FocusPane::Subagent;
    state.select_tab(2); // 1-indexed, select second tab

    let tab_area = Rect::new(0, 0, 40, 3);

    // Click on the already-selected second tab
    let updated_state = handle_mouse_click(state.clone(), 30, 1, tab_area);

    assert_eq!(
        updated_state.selected_tab,
        Some(1),
        "Clicking already-selected tab should keep it selected"
    );
}

// ===== Division by Zero Guard Tests =====

#[test]
fn detect_tab_click_returns_no_tab_when_tab_area_width_is_zero() {
    let agent1 = agent_id("agent-1");
    let agent_ids = vec![&agent1];

    // Tab area with width == 0
    let tab_area = Rect::new(0, 0, 0, 1);

    // Click anywhere - should not panic
    let result = detect_tab_click(5, 0, tab_area, &agent_ids);

    assert_eq!(
        result,
        TabClickResult::NoTab,
        "Zero width tab area should return NoTab (not panic)"
    );
}

#[test]
fn detect_tab_click_returns_no_tab_when_tab_width_rounds_to_zero() {
    let agent1 = agent_id("agent-1");
    let agent2 = agent_id("agent-2");
    let agent_ids = vec![&agent1, &agent2];

    // Tab area width 1 with 2 tabs = tab_width 0
    let tab_area = Rect::new(0, 0, 1, 1);

    // Click at x=0 (within bounds) - should not panic
    let result = detect_tab_click(0, 0, tab_area, &agent_ids);

    assert_eq!(
        result,
        TabClickResult::NoTab,
        "Tab width rounding to zero should return NoTab (not panic)"
    );
}

// ===== detect_entry_click Tests =====

#[test]
fn detect_entry_click_returns_no_entry_when_click_outside_bounds() {
    let state = create_app_state_with_tabs(vec![]);

    // Main pane at (0, 0) with width 40, height 20
    let main_area = Rect::new(0, 0, 40, 20);

    // Click outside the area (x=50, y=25)
    let result = detect_entry_click(50, 25, main_area, None, &state);

    assert_eq!(
        result,
        EntryClickResult::NoEntry,
        "Click outside pane area should return NoEntry"
    );
}

#[test]
fn detect_entry_click_detects_main_pane_first_entry() {
    let mut session = Session::new(make_session_id("test-session"));

    // Add entries to main agent
    let uuid1 = make_entry_uuid("entry-1");
    let log_entry = LogEntry::new(
        uuid1.clone(),
        None,
        make_session_id("test-session"),
        None,
        Utc::now(),
        EntryType::User,
        Message::new(
            Role::User,
            MessageContent::Text("First message".to_string()),
        ),
        EntryMetadata::default(),
    );
    session.add_conversation_entry(ConversationEntry::Valid(Box::new(log_entry)));

    let state = AppState::new(session);

    // Main pane at (0, 0) with width 40, height 20
    // Click on first entry (inside border: y=1)
    let main_area = Rect::new(0, 0, 40, 20);
    let result = detect_entry_click(5, 1, main_area, None, &state);

    assert_eq!(
        result,
        EntryClickResult::MainPaneEntry(0),
        "Click on first entry should return MainPaneEntry(0)"
    );
}

#[test]
fn detect_entry_click_detects_subagent_pane_entry() {
    let mut state = create_app_state_with_tabs(vec!["agent-1"]);

    // Set focus and select the first subagent tab
    state.focus = crate::state::FocusPane::Subagent;
    state.select_tab(1); // 1-indexed

    // Main pane area
    let main_area = Rect::new(0, 0, 40, 20);

    // Subagent pane area
    let subagent_area = Rect::new(41, 0, 40, 20);

    // Click in subagent pane (inside border: y=1)
    let result = detect_entry_click(45, 1, main_area, Some(subagent_area), &state);

    assert_eq!(
        result,
        EntryClickResult::SubagentPaneEntry(0),
        "Click in subagent pane should return SubagentPaneEntry"
    );
}

#[test]
fn detect_entry_click_accounts_for_border() {
    let mut session = Session::new(make_session_id("test-session"));

    // Add entry
    let uuid1 = make_entry_uuid("entry-1");
    let log_entry = LogEntry::new(
        uuid1.clone(),
        None,
        make_session_id("test-session"),
        None,
        Utc::now(),
        EntryType::User,
        Message::new(Role::User, MessageContent::Text("Test".to_string())),
        EntryMetadata::default(),
    );
    session.add_conversation_entry(ConversationEntry::Valid(Box::new(log_entry)));

    let state = AppState::new(session);
    let main_area = Rect::new(0, 0, 40, 20);

    // Click on border (y=0) should return NoEntry
    let result = detect_entry_click(5, 0, main_area, None, &state);
    assert_eq!(
        result,
        EntryClickResult::NoEntry,
        "Click on border should return NoEntry"
    );
}

#[test]
fn detect_entry_click_handles_empty_conversation() {
    let state = create_app_state_with_tabs(vec![]);

    let main_area = Rect::new(0, 0, 40, 20);

    // Click anywhere in empty pane
    let result = detect_entry_click(5, 5, main_area, None, &state);

    assert_eq!(
        result,
        EntryClickResult::NoEntry,
        "Click in empty pane should return NoEntry"
    );
}

// ===== handle_entry_click Tests =====

#[test]
fn handle_entry_click_toggles_main_pane_entry_expansion() {
    let mut session = Session::new(make_session_id("test-session"));

    // Add entry
    let uuid1 = make_entry_uuid("entry-1");
    let log_entry = LogEntry::new(
        uuid1.clone(),
        None,
        make_session_id("test-session"),
        None,
        Utc::now(),
        EntryType::User,
        Message::new(Role::User, MessageContent::Text("Test".to_string())),
        EntryMetadata::default(),
    );
    session.add_conversation_entry(ConversationEntry::Valid(Box::new(log_entry)));

    let state = AppState::new(session);

    // Initially not expanded
    assert!(!state.main_scroll.is_expanded(&uuid1));

    // Handle click on first entry
    let click_result = EntryClickResult::MainPaneEntry(0);
    let updated_state = handle_entry_click(state, click_result);

    // Should now be expanded
    assert!(
        updated_state.main_scroll.is_expanded(&uuid1),
        "Clicking entry should toggle it to expanded"
    );
}

#[test]
fn handle_entry_click_toggles_expanded_entry_to_collapsed() {
    let mut session = Session::new(make_session_id("test-session"));

    // Add entry
    let uuid1 = make_entry_uuid("entry-1");
    let log_entry = LogEntry::new(
        uuid1.clone(),
        None,
        make_session_id("test-session"),
        None,
        Utc::now(),
        EntryType::User,
        Message::new(Role::User, MessageContent::Text("Test".to_string())),
        EntryMetadata::default(),
    );
    session.add_conversation_entry(ConversationEntry::Valid(Box::new(log_entry)));

    let mut state = AppState::new(session);

    // Expand the entry first
    state.main_scroll.toggle_expand(&uuid1);
    assert!(state.main_scroll.is_expanded(&uuid1));

    // Handle click on same entry
    let click_result = EntryClickResult::MainPaneEntry(0);
    let updated_state = handle_entry_click(state, click_result);

    // Should now be collapsed
    assert!(
        !updated_state.main_scroll.is_expanded(&uuid1),
        "Clicking expanded entry should collapse it"
    );
}

#[test]
fn handle_entry_click_toggles_subagent_pane_entry() {
    let mut state = create_app_state_with_tabs(vec!["agent-1"]);

    // Set focus and select the first subagent tab
    state.focus = crate::state::FocusPane::Subagent;
    state.select_tab(1); // 1-indexed

    // Get the UUID of the subagent entry (created by create_app_state_with_tabs)
    let agent_id_ref = agent_id("agent-1");
    let conversation = state.session().subagents().get(&agent_id_ref).unwrap();
    let entry = &conversation.entries()[0];
    let uuid = entry.uuid().unwrap().clone();

    // Initially not expanded
    assert!(!state.subagent_scroll.is_expanded(&uuid));

    // Handle click on first subagent entry
    let click_result = EntryClickResult::SubagentPaneEntry(0);
    let updated_state = handle_entry_click(state, click_result);

    // Should now be expanded
    assert!(
        updated_state.subagent_scroll.is_expanded(&uuid),
        "Clicking subagent entry should toggle it to expanded"
    );
}

#[test]
fn handle_entry_click_preserves_state_when_no_entry() {
    let mut session = Session::new(make_session_id("test-session"));

    // Add entry
    let uuid1 = make_entry_uuid("entry-1");
    let log_entry = LogEntry::new(
        uuid1.clone(),
        None,
        make_session_id("test-session"),
        None,
        Utc::now(),
        EntryType::User,
        Message::new(Role::User, MessageContent::Text("Test".to_string())),
        EntryMetadata::default(),
    );
    session.add_conversation_entry(ConversationEntry::Valid(Box::new(log_entry)));

    let mut state = AppState::new(session);

    // Expand the entry
    state.main_scroll.toggle_expand(&uuid1);
    let was_expanded = state.main_scroll.is_expanded(&uuid1);

    // Handle click outside entries
    let click_result = EntryClickResult::NoEntry;
    let updated_state = handle_entry_click(state, click_result);

    // State should be unchanged
    assert_eq!(
        updated_state.main_scroll.is_expanded(&uuid1),
        was_expanded,
        "NoEntry click should preserve expansion state"
    );
}

// ===== Y-to-Entry Mapping Tests =====

#[test]
fn detect_entry_click_maps_different_y_positions_to_different_entries() {
    let mut session = Session::new(make_session_id("test-session"));

    // Add three entries to main agent
    for i in 0..3 {
        let uuid = make_entry_uuid(&format!("entry-{}", i));
        let log_entry = LogEntry::new(
            uuid,
            None,
            make_session_id("test-session"),
            None,
            Utc::now(),
            EntryType::User,
            Message::new(Role::User, MessageContent::Text(format!("Message {}", i))),
            EntryMetadata::default(),
        );
        session.add_conversation_entry(ConversationEntry::Valid(Box::new(log_entry)));
    }

    let state = AppState::new(session);

    // Main pane at (0, 0) with width 40, height 20
    // Inner area: (1, 1) to (39, 19) - 18 lines tall
    let main_area = Rect::new(0, 0, 40, 20);

    // Click near top of inner area (y=2) - should hit first entry
    let result_top = detect_entry_click(5, 2, main_area, None, &state);
    assert_eq!(
        result_top,
        EntryClickResult::MainPaneEntry(0),
        "Click near top should hit first entry"
    );

    // Click in middle of inner area (y=10) - should hit second or third entry
    let result_mid = detect_entry_click(5, 10, main_area, None, &state);
    assert!(
        matches!(
            result_mid,
            EntryClickResult::MainPaneEntry(1) | EntryClickResult::MainPaneEntry(2)
        ),
        "Click in middle should hit entry 1 or 2, got {:?}",
        result_mid
    );

    // Click near bottom of inner area (y=18) - should hit last entry
    let result_bottom = detect_entry_click(5, 18, main_area, None, &state);
    assert_eq!(
        result_bottom,
        EntryClickResult::MainPaneEntry(2),
        "Click near bottom should hit last entry"
    );
}

#[test]
fn detect_entry_click_with_single_entry_always_returns_index_0() {
    let mut session = Session::new(make_session_id("test-session"));

    // Add single entry
    let uuid = make_entry_uuid("entry-0");
    let log_entry = LogEntry::new(
        uuid,
        None,
        make_session_id("test-session"),
        None,
        Utc::now(),
        EntryType::User,
        Message::new(
            Role::User,
            MessageContent::Text("Single message".to_string()),
        ),
        EntryMetadata::default(),
    );
    session.add_conversation_entry(ConversationEntry::Valid(Box::new(log_entry)));

    let state = AppState::new(session);
    let main_area = Rect::new(0, 0, 40, 20);

    // Any click within inner area should hit entry 0
    let results = vec![
        detect_entry_click(5, 1, main_area, None, &state),
        detect_entry_click(5, 5, main_area, None, &state),
        detect_entry_click(5, 10, main_area, None, &state),
        detect_entry_click(5, 18, main_area, None, &state),
    ];

    for result in results {
        assert_eq!(
            result,
            EntryClickResult::MainPaneEntry(0),
            "All clicks on single entry should return index 0"
        );
    }
}

// ===== Mouse Scroll Tests =====

#[test]
fn handle_mouse_scroll_up_scrolls_main_pane_when_focused() {
    let mut session = Session::new(make_session_id("test-session"));

    // Add multiple entries to enable scrolling
    for i in 0..10 {
        let uuid = make_entry_uuid(&format!("entry-{}", i));
        let log_entry = LogEntry::new(
            uuid,
            None,
            make_session_id("test-session"),
            None,
            Utc::now(),
            EntryType::User,
            Message::new(Role::User, MessageContent::Text(format!("Message {}", i))),
            EntryMetadata::default(),
        );
        session.add_conversation_entry(ConversationEntry::Valid(Box::new(log_entry)));
    }

    let mut state = AppState::new(session);

    // Focus on Main pane and scroll down first
    state.focus = FocusPane::Main;
    state.main_scroll.vertical_offset = 5;

    // Scroll up
    let updated_state = handle_mouse_scroll(state, true, 20);

    assert_eq!(
        updated_state.main_scroll.vertical_offset, 4,
        "Mouse scroll up should decrement main pane vertical offset when focused"
    );
}

#[test]
fn handle_mouse_scroll_down_scrolls_main_pane_when_focused() {
    let mut session = Session::new(make_session_id("test-session"));

    // Add multiple entries
    for i in 0..10 {
        let uuid = make_entry_uuid(&format!("entry-{}", i));
        let log_entry = LogEntry::new(
            uuid,
            None,
            make_session_id("test-session"),
            None,
            Utc::now(),
            EntryType::User,
            Message::new(Role::User, MessageContent::Text(format!("Message {}", i))),
            EntryMetadata::default(),
        );
        session.add_conversation_entry(ConversationEntry::Valid(Box::new(log_entry)));
    }

    let mut state = AppState::new(session);

    // Focus on Main pane at offset 0
    state.focus = FocusPane::Main;
    state.main_scroll.vertical_offset = 0;

    // Scroll down
    let updated_state = handle_mouse_scroll(state, false, 20);

    assert_eq!(
        updated_state.main_scroll.vertical_offset, 1,
        "Mouse scroll down should increment main pane vertical offset when focused"
    );
}

#[test]
fn handle_mouse_scroll_up_scrolls_subagent_pane_when_focused() {
    let mut session = Session::new(make_session_id("test-session"));

    // Add multiple subagent entries
    for i in 0..10 {
        let uuid = make_entry_uuid(&format!("entry-{}", i));
        let agent_id = AgentId::new("test-agent").unwrap();
        let log_entry = LogEntry::new(
            uuid,
            None,
            make_session_id("test-session"),
            Some(agent_id),
            Utc::now(),
            EntryType::Assistant,
            Message::new(
                Role::Assistant,
                MessageContent::Text(format!("Message {}", i)),
            ),
            EntryMetadata::default(),
        );
        session.add_conversation_entry(ConversationEntry::Valid(Box::new(log_entry)));
    }

    let mut state = AppState::new(session);

    // Focus on Subagent pane and select tab
    state.focus = FocusPane::Subagent;
    state.selected_tab = Some(0);
    state.subagent_scroll.vertical_offset = 5;

    // Scroll up
    let updated_state = handle_mouse_scroll(state, true, 20);

    assert_eq!(
        updated_state.subagent_scroll.vertical_offset, 4,
        "Mouse scroll up should decrement subagent pane vertical offset when focused"
    );
}

#[test]
fn handle_mouse_scroll_down_scrolls_subagent_pane_when_focused() {
    let mut session = Session::new(make_session_id("test-session"));

    // Add multiple subagent entries
    for i in 0..10 {
        let uuid = make_entry_uuid(&format!("entry-{}", i));
        let agent_id = AgentId::new("test-agent").unwrap();
        let log_entry = LogEntry::new(
            uuid,
            None,
            make_session_id("test-session"),
            Some(agent_id),
            Utc::now(),
            EntryType::Assistant,
            Message::new(
                Role::Assistant,
                MessageContent::Text(format!("Message {}", i)),
            ),
            EntryMetadata::default(),
        );
        session.add_conversation_entry(ConversationEntry::Valid(Box::new(log_entry)));
    }

    let mut state = AppState::new(session);

    // Focus on Subagent pane
    state.focus = FocusPane::Subagent;
    state.selected_tab = Some(0);
    state.subagent_scroll.vertical_offset = 0;

    // Scroll down
    let updated_state = handle_mouse_scroll(state, false, 20);

    assert_eq!(
        updated_state.subagent_scroll.vertical_offset, 1,
        "Mouse scroll down should increment subagent pane vertical offset when focused"
    );
}

#[test]
fn handle_mouse_scroll_ignores_scroll_when_stats_focused() {
    let session = Session::new(make_session_id("test-session"));
    let mut state = AppState::new(session);

    // Focus on Stats pane
    state.focus = FocusPane::Stats;
    state.main_scroll.vertical_offset = 5;
    state.subagent_scroll.vertical_offset = 3;

    // Try to scroll up
    let updated_state = handle_mouse_scroll(state.clone(), true, 20);

    assert_eq!(
        updated_state.main_scroll.vertical_offset, 5,
        "Mouse scroll should not affect main pane when Stats is focused"
    );
    assert_eq!(
        updated_state.subagent_scroll.vertical_offset, 3,
        "Mouse scroll should not affect subagent pane when Stats is focused"
    );

    // Try to scroll down
    let updated_state = handle_mouse_scroll(state, false, 20);

    assert_eq!(
        updated_state.main_scroll.vertical_offset, 5,
        "Mouse scroll should not affect main pane when Stats is focused"
    );
    assert_eq!(
        updated_state.subagent_scroll.vertical_offset, 3,
        "Mouse scroll should not affect subagent pane when Stats is focused"
    );
}

#[test]
fn handle_mouse_scroll_ignores_scroll_when_search_focused() {
    let session = Session::new(make_session_id("test-session"));
    let mut state = AppState::new(session);

    // Focus on Search pane
    state.focus = FocusPane::Search;
    state.main_scroll.vertical_offset = 5;
    state.subagent_scroll.vertical_offset = 3;

    // Try to scroll up
    let updated_state = handle_mouse_scroll(state.clone(), true, 20);

    assert_eq!(
        updated_state.main_scroll.vertical_offset, 5,
        "Mouse scroll should not affect main pane when Search is focused"
    );
    assert_eq!(
        updated_state.subagent_scroll.vertical_offset, 3,
        "Mouse scroll should not affect subagent pane when Search is focused"
    );

    // Try to scroll down
    let updated_state = handle_mouse_scroll(state, false, 20);

    assert_eq!(
        updated_state.main_scroll.vertical_offset, 5,
        "Mouse scroll should not affect main pane when Search is focused"
    );
    assert_eq!(
        updated_state.subagent_scroll.vertical_offset, 3,
        "Mouse scroll should not affect subagent pane when Search is focused"
    );
}
