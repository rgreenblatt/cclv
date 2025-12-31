//! Tests for mouse event handling.

use super::*;
use crate::model::{
    AgentId, ConversationEntry, EntryMetadata, EntryType, EntryUuid, LogEntry, Message,
    MessageContent, Role, SessionId,
};
use crate::state::AppState;
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

fn make_main_entry() -> ConversationEntry {
    let log_entry = LogEntry::new(
        make_entry_uuid("main-entry"),
        None,
        make_session_id("test-session"),
        None, // No agent_id = main agent
        Utc::now(),
        EntryType::User,
        Message::new(Role::User, MessageContent::Text("Main message".to_string())),
        EntryMetadata::default(),
    );

    ConversationEntry::Valid(Box::new(log_entry))
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
    let mut entries = Vec::new();

    // Add a conversation entry for each subagent (this creates the subagent)
    for id in agent_ids {
        entries.push(make_subagent_entry(id));
    }

    // If no entries, add a dummy main entry to ensure session_view exists
    if entries.is_empty() {
        entries.push(make_main_entry());
    }

    let mut state = AppState::new();
    state.add_entries(entries);
    state
}

/// Initialize layout for all conversations in state using actual rendering.
/// Required for tests that use hit_test-based entry detection.
fn init_layout_for_state(state: &mut AppState) {
    use crate::state::WrapMode;
    use crate::view_state::layout_params::LayoutParams;

    let params = LayoutParams::new(80, WrapMode::Wrap);

    // Initialize main conversation layout
    if let Some(session_view) = state.log_view_mut().current_session_mut() {
        session_view.main_mut().recompute_layout(params);

        // Initialize subagent layouts
        let agent_ids: Vec<_> = session_view.subagent_ids().cloned().collect();
        for agent_id in agent_ids {
            session_view
                .subagent_mut(&agent_id)
                .recompute_layout(params);
        }
    }
}

/// Get the measured height of an entry after rendering.
fn get_entry_height(state: &AppState, entry_idx: usize) -> u16 {
    use crate::view_state::types::EntryIndex;

    state
        .log_view()
        .current_session()
        .expect("session exists")
        .main()
        .get(EntryIndex::new(entry_idx))
        .expect("entry exists")
        .height()
        .get()
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
    let mut entries = Vec::new();

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
    entries.push(ConversationEntry::Valid(Box::new(log_entry)));

    let mut state = AppState::new();
    state.add_entries(entries);
    init_layout_for_state(&mut state);

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
    init_layout_for_state(&mut state);

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
    let mut entries = Vec::new();

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
    entries.push(ConversationEntry::Valid(Box::new(log_entry)));

    let mut state = AppState::new();
    state.add_entries(entries);
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
    let mut state = create_app_state_with_tabs(vec![]);
    init_layout_for_state(&mut state);

    let main_area = Rect::new(0, 0, 40, 20);

    // Note: create_app_state_with_tabs adds a dummy main entry when vec is empty
    // to ensure session_view exists, so conversation has 1 entry
    let entry_height = get_entry_height(&state, 0);

    // Click inside the entry (accounting for border at y=1)
    let click_y = 1 + (entry_height / 2);
    let result = detect_entry_click(5, click_y, main_area, None, &state);

    assert_eq!(
        result,
        EntryClickResult::MainPaneEntry(0),
        "Click should detect the single main entry"
    );
}

// ===== handle_entry_click Tests =====
// Tests removed during expand state migration to view-state layer

// ===== Y-to-Entry Mapping Tests =====

#[test]
fn detect_entry_click_maps_different_y_positions_to_different_entries() {
    let mut entries = Vec::new();

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
        entries.push(ConversationEntry::Valid(Box::new(log_entry)));
    }

    let mut state = AppState::new();
    state.add_entries(entries);
    init_layout_for_state(&mut state);

    let main_area = Rect::new(0, 0, 40, 20);

    // Get actual entry positions
    let h0 = get_entry_height(&state, 0);
    let h1 = get_entry_height(&state, 1);
    let pos0 = 0;
    let pos1 = h0;

    // Click inside entry 0 (border starts at y=1)
    let click_y0 = 1 + pos0 + (h0 / 2);
    let result_top = detect_entry_click(5, click_y0, main_area, None, &state);
    assert_eq!(
        result_top,
        EntryClickResult::MainPaneEntry(0),
        "Click inside entry 0 should hit entry 0"
    );

    // Click at start of entry 1
    let click_y1_start = 1 + pos1;
    let result_mid = detect_entry_click(5, click_y1_start, main_area, None, &state);
    assert_eq!(
        result_mid,
        EntryClickResult::MainPaneEntry(1),
        "Click at start of entry 1 should hit entry 1"
    );

    // Click in middle of entry 1
    let click_y1_mid = 1 + pos1 + (h1 / 2);
    let result_bottom = detect_entry_click(5, click_y1_mid, main_area, None, &state);
    assert_eq!(
        result_bottom,
        EntryClickResult::MainPaneEntry(1),
        "Click in middle of entry 1 should hit entry 1"
    );
}

#[test]
fn detect_entry_click_with_single_entry_always_returns_index_0() {
    let mut entries = Vec::new();

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
    entries.push(ConversationEntry::Valid(Box::new(log_entry)));

    let mut state = AppState::new();
    state.add_entries(entries);
    init_layout_for_state(&mut state);
    let main_area = Rect::new(0, 0, 40, 20);

    let entry_height = get_entry_height(&state, 0);

    // Click at start of entry (border at y=1)
    assert_eq!(
        detect_entry_click(5, 1, main_area, None, &state),
        EntryClickResult::MainPaneEntry(0)
    );

    // Click in middle of entry
    let click_mid = 1 + (entry_height / 2);
    assert_eq!(
        detect_entry_click(5, click_mid, main_area, None, &state),
        EntryClickResult::MainPaneEntry(0)
    );

    // Click at last line of entry
    let click_last = 1 + entry_height - 1;
    assert_eq!(
        detect_entry_click(5, click_last, main_area, None, &state),
        EntryClickResult::MainPaneEntry(0)
    );

    // Click beyond entry height
    let click_beyond = 1 + entry_height + 5;
    assert_eq!(
        detect_entry_click(5, click_beyond, main_area, None, &state),
        EntryClickResult::NoEntry,
        "Click beyond entry height should return NoEntry"
    );
}

// ===== Hit Test Integration Tests =====
// These tests verify that detect_entry_click uses ConversationViewState.hit_test()
// rather than a fixed-height linear scan approach.

#[test]
fn detect_entry_click_uses_actual_entry_heights_from_layout() {
    // Create three entries
    let mut entries = Vec::new();
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
        entries.push(ConversationEntry::Valid(Box::new(log_entry)));
    }

    let mut state = AppState::new();
    state.add_entries(entries);
    init_layout_for_state(&mut state);

    let main_area = Rect::new(0, 0, 40, 30);

    // Get actual measured heights
    let h0 = get_entry_height(&state, 0);
    let h1 = get_entry_height(&state, 1);
    let pos0 = 0;
    let pos1 = h0;
    let pos2 = h0 + h1;

    // Click in middle of entry 0 (border at y=1)
    let click_e0 = 1 + pos0 + (h0 / 2);
    let result = detect_entry_click(5, click_e0, main_area, None, &state);
    assert_eq!(
        result,
        EntryClickResult::MainPaneEntry(0),
        "Click in entry 0 should hit entry 0"
    );

    // Click at start of entry 1
    let click_e1 = 1 + pos1;
    let result = detect_entry_click(5, click_e1, main_area, None, &state);
    assert_eq!(
        result,
        EntryClickResult::MainPaneEntry(1),
        "Click at start of entry 1 should hit entry 1"
    );

    // Click at start of entry 2
    let click_e2 = 1 + pos2;
    let result = detect_entry_click(5, click_e2, main_area, None, &state);
    assert_eq!(
        result,
        EntryClickResult::MainPaneEntry(2),
        "Click at start of entry 2 should hit entry 2"
    );
}

#[test]
fn detect_entry_click_accounts_for_scroll_offset() {
    use crate::view_state::scroll::ScrollPosition;
    use crate::view_state::types::LineOffset;

    // Create 10 entries with longer text to force scrolling
    let mut entries = Vec::new();
    for i in 0..10 {
        let uuid = make_entry_uuid(&format!("entry-{}", i));
        let long_text = format!(
            "This is a longer message {} that will take multiple lines when wrapped at 80 characters width",
            i
        );
        let log_entry = LogEntry::new(
            uuid,
            None,
            make_session_id("test-session"),
            None,
            Utc::now(),
            EntryType::User,
            Message::new(Role::User, MessageContent::Text(long_text)),
            EntryMetadata::default(),
        );
        entries.push(ConversationEntry::Valid(Box::new(log_entry)));
    }

    let mut state = AppState::new();
    state.add_entries(entries);
    init_layout_for_state(&mut state);

    // Use smaller viewport to ensure scrolling is needed
    let main_area = Rect::new(0, 0, 40, 15); // Smaller height

    // Get cumulative heights to find scroll position
    let h0 = get_entry_height(&state, 0);
    let h1 = get_entry_height(&state, 1);
    let h2 = get_entry_height(&state, 2);
    let scroll_to = h0 + h1; // Skip entries 0 and 1

    // Set scroll position
    if let Some(session_view) = state.log_view_mut().current_session_mut() {
        session_view
            .main_mut()
            .set_scroll(ScrollPosition::AtLine(LineOffset::new(scroll_to as usize)));
    }

    // Click near top of viewport (border at y=1) should hit entry 2
    let result = detect_entry_click(5, 2, main_area, None, &state);
    assert_eq!(
        result,
        EntryClickResult::MainPaneEntry(2),
        "Click at viewport top with scroll should hit entry 2"
    );

    // Click further down should hit entry 3
    let click_e3 = 1 + h2 + 1; // border + entry2_height + into entry3
    let result = detect_entry_click(5, click_e3, main_area, None, &state);
    assert_eq!(
        result,
        EntryClickResult::MainPaneEntry(3),
        "Click further down with scroll should hit entry 3"
    );
}

#[test]
fn detect_entry_click_returns_no_entry_when_clicking_beyond_content() {
    // Create single entry
    let uuid = make_entry_uuid("entry-0");
    let log_entry = LogEntry::new(
        uuid,
        None,
        make_session_id("test-session"),
        None,
        Utc::now(),
        EntryType::User,
        Message::new(Role::User, MessageContent::Text("Message".to_string())),
        EntryMetadata::default(),
    );

    let mut state = AppState::new();
    state.add_entries(vec![ConversationEntry::Valid(Box::new(log_entry))]);
    init_layout_for_state(&mut state);

    let main_area = Rect::new(0, 0, 40, 30);

    // Click well beyond entry height
    let entry_height = get_entry_height(&state, 0);
    let click_beyond = 1 + entry_height + 10;
    let result = detect_entry_click(5, click_beyond, main_area, None, &state);
    assert_eq!(
        result,
        EntryClickResult::NoEntry,
        "Click beyond entry content should return NoEntry"
    );
}

// ===== Mouse Scroll Tests =====
// NOTE: Tests that verified vertical_offset behavior have been deleted.
// Vertical scrolling is now managed by ConversationViewState via ScrollPosition.
// Mouse scroll functionality will be verified by integration tests once migration is complete.

// ===== HeightIndex Integration Tests =====

/// Test that mouse expand maintains HeightIndex invariant.
///
/// Verifies that height_index[i] == entries[i].rendered_lines.len() after mouse click toggle.
/// Pattern from expand_handler_tests.rs::test_toggle_maintains_height_index_invariant
#[test]
fn test_mouse_expand_maintains_height_index_invariant() {
    let entries = vec![make_main_entry(), make_main_entry()];

    let mut state = AppState::new();
    state.add_entries(entries);
    state.focus = crate::state::FocusPane::Main;

    // Initialize HeightIndex via relayout
    if let Some(view) = state.main_conversation_view_mut() {
        view.relayout(80, crate::state::WrapMode::Wrap);
    }

    // Simulate mouse click on entry 0 to toggle expand
    let entry_click = EntryClickResult::MainPaneEntry(0);
    let result = handle_entry_click(state, entry_click, 80);

    // Verify HeightIndex invariant holds
    if let Some(view) = result.main_conversation_view() {
        for i in 0..view.len() {
            let entry = view
                .get(crate::view_state::types::EntryIndex::new(i))
                .expect("entry exists");
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
    } else {
        panic!("Expected main conversation view");
    }
}
