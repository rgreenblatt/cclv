//! Tests for tab navigation handler.
//!
//! Tests verify that tab actions are correctly dispatched to AppState methods:
//! - NextTab moves to next tab (with wrapping)
//! - PrevTab moves to previous tab (with wrapping)
//! - SelectTab(n) selects tab by 1-indexed number
//! - All actions respect focus (only work when Subagent pane focused)
//! - All actions handle edge cases (no subagents, out of bounds)

use super::*;
use crate::model::{
    AgentId, ConversationEntry, EntryMetadata, EntryType, EntryUuid, LogEntry, Message,
    MessageContent, Role, SessionId,
};
use crate::state::{AppState, FocusPane};
use chrono::Utc;

// ===== Test Helpers =====

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

// ===== NextTab tests =====

#[test]
fn next_tab_moves_to_next_tab() {
    let mut entries = Vec::new();
    entries.push(make_subagent_entry("agent-1"));
    entries.push(make_subagent_entry("agent-2"));
    entries.push(make_subagent_entry("agent-3"));

    let mut state = AppState::new();
    state.add_entries(entries);
    state.focus = FocusPane::Subagent;
    state.selected_tab = Some(0);

    let new_state = handle_tab_action(state, KeyAction::NextTab);

    assert_eq!(
        new_state.selected_tab,
        Some(1),
        "NextTab should move from tab 0 to tab 1"
    );
}

#[test]
fn next_tab_wraps_from_last_to_first() {
    let mut entries = Vec::new();
    entries.push(make_subagent_entry("agent-1"));
    entries.push(make_subagent_entry("agent-2"));

    let mut state = AppState::new();
    state.add_entries(entries);
    state.focus = FocusPane::Subagent;
    state.selected_tab = Some(1); // Last tab

    let new_state = handle_tab_action(state, KeyAction::NextTab);

    assert_eq!(
        new_state.selected_tab,
        Some(0),
        "NextTab should wrap from last tab to first"
    );
}

#[test]
fn next_tab_initializes_to_first_when_none() {
    let mut entries = Vec::new();
    entries.push(make_subagent_entry("agent-1"));

    let mut state = AppState::new();
    state.add_entries(entries);
    state.focus = FocusPane::Subagent;
    state.selected_tab = None;

    let new_state = handle_tab_action(state, KeyAction::NextTab);

    assert_eq!(
        new_state.selected_tab,
        Some(0),
        "NextTab should initialize to first tab when None"
    );
}

#[test]
fn next_tab_does_nothing_when_focus_not_on_subagent() {
    let mut entries = Vec::new();
    entries.push(make_subagent_entry("agent-1"));
    entries.push(make_subagent_entry("agent-2"));

    let mut state = AppState::new();
    state.add_entries(entries);
    state.focus = FocusPane::Main;
    state.selected_tab = Some(0);

    let new_state = handle_tab_action(state, KeyAction::NextTab);

    assert_eq!(
        new_state.selected_tab,
        Some(0),
        "NextTab should not change tab when focus is not on Subagent"
    );
}

#[test]
fn next_tab_does_nothing_when_no_subagents() {
    let mut state = AppState::new();
    state.focus = FocusPane::Subagent;
    state.selected_tab = None;

    let new_state = handle_tab_action(state, KeyAction::NextTab);

    assert_eq!(
        new_state.selected_tab, None,
        "NextTab should not change tab when no subagents exist"
    );
}

// ===== PrevTab tests =====

#[test]
fn prev_tab_moves_to_previous_tab() {
    let mut entries = Vec::new();
    entries.push(make_subagent_entry("agent-1"));
    entries.push(make_subagent_entry("agent-2"));
    entries.push(make_subagent_entry("agent-3"));

    let mut state = AppState::new();
    state.add_entries(entries);
    state.focus = FocusPane::Subagent;
    state.selected_tab = Some(2); // Third tab

    let new_state = handle_tab_action(state, KeyAction::PrevTab);

    assert_eq!(
        new_state.selected_tab,
        Some(1),
        "PrevTab should move from tab 2 to tab 1"
    );
}

#[test]
fn prev_tab_wraps_from_first_to_last() {
    let mut entries = Vec::new();
    entries.push(make_subagent_entry("agent-1"));
    entries.push(make_subagent_entry("agent-2"));
    entries.push(make_subagent_entry("agent-3"));

    let mut state = AppState::new();
    state.add_entries(entries);
    state.focus = FocusPane::Subagent;
    state.selected_tab = Some(0); // First tab

    let new_state = handle_tab_action(state, KeyAction::PrevTab);

    assert_eq!(
        new_state.selected_tab,
        Some(2),
        "PrevTab should wrap from first tab to last (index 2)"
    );
}

#[test]
fn prev_tab_initializes_to_first_when_none() {
    let mut entries = Vec::new();
    entries.push(make_subagent_entry("agent-1"));

    let mut state = AppState::new();
    state.add_entries(entries);
    state.focus = FocusPane::Subagent;
    state.selected_tab = None;

    let new_state = handle_tab_action(state, KeyAction::PrevTab);

    assert_eq!(
        new_state.selected_tab,
        Some(0),
        "PrevTab should initialize to first tab when None"
    );
}

#[test]
fn prev_tab_does_nothing_when_focus_not_on_subagent() {
    let mut entries = Vec::new();
    entries.push(make_subagent_entry("agent-1"));
    entries.push(make_subagent_entry("agent-2"));

    let mut state = AppState::new();
    state.add_entries(entries);
    state.focus = FocusPane::Stats;
    state.selected_tab = Some(1);

    let new_state = handle_tab_action(state, KeyAction::PrevTab);

    assert_eq!(
        new_state.selected_tab,
        Some(1),
        "PrevTab should not change tab when focus is not on Subagent"
    );
}

#[test]
fn prev_tab_does_nothing_when_no_subagents() {
    let mut state = AppState::new();
    state.focus = FocusPane::Subagent;
    state.selected_tab = None;

    let new_state = handle_tab_action(state, KeyAction::PrevTab);

    assert_eq!(
        new_state.selected_tab, None,
        "PrevTab should not change tab when no subagents exist"
    );
}

// ===== SelectTab tests =====

#[test]
fn select_tab_sets_tab_by_one_indexed_number() {
    let mut entries = Vec::new();
    entries.push(make_subagent_entry("agent-1"));
    entries.push(make_subagent_entry("agent-2"));
    entries.push(make_subagent_entry("agent-3"));

    let mut state = AppState::new();
    state.add_entries(entries);
    state.focus = FocusPane::Subagent;
    state.selected_tab = Some(0);

    let new_state = handle_tab_action(state, KeyAction::SelectTab(2));

    assert_eq!(
        new_state.selected_tab,
        Some(1),
        "SelectTab(2) should select second tab (0-indexed as 1)"
    );
}

#[test]
fn select_tab_handles_tab_1() {
    let mut entries = Vec::new();
    entries.push(make_subagent_entry("agent-1"));

    let mut state = AppState::new();
    state.add_entries(entries);
    state.focus = FocusPane::Subagent;
    state.selected_tab = None;

    let new_state = handle_tab_action(state, KeyAction::SelectTab(1));

    assert_eq!(
        new_state.selected_tab,
        Some(0),
        "SelectTab(1) should select first tab (0-indexed as 0)"
    );
}

#[test]
fn select_tab_clamps_to_last_when_too_high() {
    let mut entries = Vec::new();
    entries.push(make_subagent_entry("agent-1"));
    entries.push(make_subagent_entry("agent-2"));

    let mut state = AppState::new();
    state.add_entries(entries);
    state.focus = FocusPane::Subagent;
    state.selected_tab = Some(0);

    let new_state = handle_tab_action(state, KeyAction::SelectTab(9));

    assert_eq!(
        new_state.selected_tab,
        Some(1),
        "SelectTab(9) should clamp to last tab when number is too high"
    );
}

#[test]
fn select_tab_ignores_zero() {
    let mut entries = Vec::new();
    entries.push(make_subagent_entry("agent-1"));
    entries.push(make_subagent_entry("agent-2"));

    let mut state = AppState::new();
    state.add_entries(entries);
    state.focus = FocusPane::Subagent;
    state.selected_tab = Some(1);

    let new_state = handle_tab_action(state, KeyAction::SelectTab(0));

    assert_eq!(
        new_state.selected_tab,
        Some(1),
        "SelectTab(0) should be ignored (invalid 1-indexed input)"
    );
}

#[test]
fn select_tab_does_nothing_when_focus_not_on_subagent() {
    let mut entries = Vec::new();
    entries.push(make_subagent_entry("agent-1"));
    entries.push(make_subagent_entry("agent-2"));

    let mut state = AppState::new();
    state.add_entries(entries);
    state.focus = FocusPane::Main;
    state.selected_tab = Some(0);

    let new_state = handle_tab_action(state, KeyAction::SelectTab(2));

    assert_eq!(
        new_state.selected_tab,
        Some(0),
        "SelectTab should not change tab when focus is not on Subagent"
    );
}

#[test]
fn select_tab_does_nothing_when_no_subagents() {
    let mut state = AppState::new();
    state.focus = FocusPane::Subagent;
    state.selected_tab = None;

    let new_state = handle_tab_action(state, KeyAction::SelectTab(1));

    assert_eq!(
        new_state.selected_tab, None,
        "SelectTab should not change tab when no subagents exist"
    );
}

// ===== Non-tab action tests =====

#[test]
fn non_tab_actions_return_state_unchanged() {
    let mut entries = Vec::new();
    entries.push(make_subagent_entry("agent-1"));
    entries.push(make_subagent_entry("agent-2"));

    let mut state = AppState::new();
    state.add_entries(entries);
    state.focus = FocusPane::Subagent;
    state.selected_tab = Some(1);

    let new_state = handle_tab_action(state, KeyAction::ScrollDown);

    assert_eq!(
        new_state.selected_tab,
        Some(1),
        "Non-tab actions should return state unchanged"
    );
}

#[test]
fn non_tab_actions_like_quit_return_state_unchanged() {
    let mut entries = Vec::new();
    entries.push(make_subagent_entry("agent-1"));

    let mut state = AppState::new();
    state.add_entries(entries);
    state.focus = FocusPane::Subagent;
    state.selected_tab = Some(0);

    let new_state = handle_tab_action(state, KeyAction::Quit);

    assert_eq!(
        new_state.selected_tab,
        Some(0),
        "Quit action should return state unchanged"
    );
}
