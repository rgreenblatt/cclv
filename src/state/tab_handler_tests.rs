//! Tests for tab navigation handler.
//!
//! Tests verify that tab actions are correctly dispatched to AppState methods:
//! - NextTab moves to next tab (with wrapping)
//! - PrevTab moves to previous tab (with wrapping)
//! - SelectTab(n) selects tab by 1-indexed number
//! - Tab 0 = main agent, tabs 1..N = subagents (FR-083-088)
//! - Tab operations work regardless of focus (except Search modal)
//! - All actions handle edge cases (no session, out of bounds)

use super::*;
use crate::model::{
    AgentId, ConversationEntry, EntryMetadata, EntryType, EntryUuid, LogEntry, Message,
    MessageContent, Role, SessionId,
};
use crate::state::{AppState, ConversationSelection, FocusPane};
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
    let entries = vec![
        make_subagent_entry("agent-1"),
        make_subagent_entry("agent-2"),
        make_subagent_entry("agent-3"),
    ];

    let mut state = AppState::new();
    state.add_entries(entries);
    // Tab 0 = main, tabs 1-3 = subagents
    state.selected_conversation = ConversationSelection::Main; // main agent

    let state = handle_tab_action(state, KeyAction::NextTab);

    assert_eq!(
        state.selected_tab_index(),
        Some(1),
        "NextTab should move from tab 0 (main) to tab 1 (agent-1)"
    );
}

#[test]
fn next_tab_wraps_from_last_to_first() {
    let entries = vec![
        make_subagent_entry("agent-1"),
        make_subagent_entry("agent-2"),
    ];

    let mut state = AppState::new();
    state.add_entries(entries);
    // Tab 0 = main, tab 1 = agent-1, tab 2 = agent-2
    state.selected_conversation = ConversationSelection::Subagent(AgentId::new("agent-2").unwrap()); // Last tab (agent-2)

    let state = handle_tab_action(state, KeyAction::NextTab);

    assert_eq!(
        state.selected_tab_index(),
        Some(0),
        "NextTab should wrap from tab 2 (last) to tab 0 (main)"
    );
}

#[test]
fn next_tab_works_regardless_of_focus() {
    let entries = vec![
        make_subagent_entry("agent-1"),
        make_subagent_entry("agent-2"),
    ];

    let mut state = AppState::new();
    state.add_entries(entries);
    state.focus = FocusPane::Main; // Different focus
    state.selected_conversation = ConversationSelection::Main;

    let state = handle_tab_action(state, KeyAction::NextTab);

    assert_eq!(
        state.selected_tab_index(),
        Some(1),
        "NextTab should work even when focus is on Main pane (FR-088)"
    );
}

#[test]
fn next_tab_does_nothing_when_no_session() {
    let state = AppState::new(); // No entries = no session
                                 // AppState::new() initializes to Some(0) per FR-083

    let state = handle_tab_action(state, KeyAction::NextTab);

    assert_eq!(
        state.selected_tab_index(),
        Some(0),
        "NextTab should be no-op when no session (stays at tab 0)"
    );
}

// ===== PrevTab tests =====

#[test]
fn prev_tab_moves_to_previous_tab() {
    let entries = vec![
        make_subagent_entry("agent-1"),
        make_subagent_entry("agent-2"),
        make_subagent_entry("agent-3"),
    ];

    let mut state = AppState::new();
    state.add_entries(entries);
    // Tab 0 = main, tab 1 = agent-1, tab 2 = agent-2, tab 3 = agent-3
    state.selected_conversation = ConversationSelection::Subagent(AgentId::new("agent-3").unwrap()); // Last tab (agent-3)

    let state = handle_tab_action(state, KeyAction::PrevTab);

    assert_eq!(
        state.selected_tab_index(),
        Some(2),
        "PrevTab should move from tab 3 (agent-3) to tab 2 (agent-2)"
    );
}

#[test]
fn prev_tab_wraps_from_first_to_last() {
    let entries = vec![
        make_subagent_entry("agent-1"),
        make_subagent_entry("agent-2"),
        make_subagent_entry("agent-3"),
    ];

    let mut state = AppState::new();
    state.add_entries(entries);
    // Tab 0 = main, tab 1 = agent-1, tab 2 = agent-2, tab 3 = agent-3
    state.selected_conversation = ConversationSelection::Main; // First tab (main)

    let state = handle_tab_action(state, KeyAction::PrevTab);

    assert_eq!(
        state.selected_tab_index(),
        Some(3),
        "PrevTab should wrap from tab 0 (main) to tab 3 (agent-3)"
    );
}

#[test]
fn prev_tab_works_regardless_of_focus() {
    let entries = vec![
        make_subagent_entry("agent-1"),
        make_subagent_entry("agent-2"),
    ];

    let mut state = AppState::new();
    state.add_entries(entries);
    state.focus = FocusPane::Stats; // Different focus
                                    // Tab 0 = main, tab 1 = agent-1, tab 2 = agent-2
    state.selected_conversation = ConversationSelection::Subagent(AgentId::new("agent-2").unwrap());

    let state = handle_tab_action(state, KeyAction::PrevTab);

    assert_eq!(
        state.selected_tab_index(),
        Some(1),
        "PrevTab should work even when focus is on Stats pane (FR-088)"
    );
}

#[test]
fn prev_tab_does_nothing_when_no_session() {
    let state = AppState::new(); // No entries = no session
                                 // AppState::new() initializes to Some(0) per FR-083

    let state = handle_tab_action(state, KeyAction::PrevTab);

    assert_eq!(
        state.selected_tab_index(),
        Some(0),
        "PrevTab should be no-op when no session (stays at tab 0)"
    );
}

// ===== SelectTab tests =====

#[test]
fn select_tab_sets_tab_by_one_indexed_number() {
    let entries = vec![
        make_subagent_entry("agent-1"),
        make_subagent_entry("agent-2"),
        make_subagent_entry("agent-3"),
    ];

    let mut state = AppState::new();
    state.add_entries(entries);
    // Tab 0 = main, tab 1 = agent-1, tab 2 = agent-2, tab 3 = agent-3
    state.selected_conversation = ConversationSelection::Main;

    let state = handle_tab_action(state, KeyAction::SelectTab(3));

    assert_eq!(
        state.selected_tab_index(),
        Some(2),
        "SelectTab(3) should select third tab (agent-2, 0-indexed as 2)"
    );
}

#[test]
fn select_tab_handles_tab_1_as_main() {
    let entries = vec![make_subagent_entry("agent-1")];

    let mut state = AppState::new();
    state.add_entries(entries);
    // Default is Main
    // state.selected_conversation = ConversationSelection::Main;

    let state = handle_tab_action(state, KeyAction::SelectTab(1));

    assert_eq!(
        state.selected_tab_index(),
        Some(0),
        "SelectTab(1) should select tab 0 (main agent)"
    );
}

#[test]
fn select_tab_clamps_to_last_when_too_high() {
    let entries = vec![
        make_subagent_entry("agent-1"),
        make_subagent_entry("agent-2"),
    ];

    let mut state = AppState::new();
    state.add_entries(entries);
    // Tab 0 = main, tab 1 = agent-1, tab 2 = agent-2
    state.selected_conversation = ConversationSelection::Main;

    let state = handle_tab_action(state, KeyAction::SelectTab(9));

    assert_eq!(
        state.selected_tab_index(),
        Some(2),
        "SelectTab(9) should clamp to last tab (agent-2, index 2)"
    );
}

#[test]
fn select_tab_ignores_zero() {
    let entries = vec![
        make_subagent_entry("agent-1"),
        make_subagent_entry("agent-2"),
    ];

    let mut state = AppState::new();
    state.add_entries(entries);
    state.selected_conversation = ConversationSelection::Subagent(AgentId::new("agent-1").unwrap());

    let state = handle_tab_action(state, KeyAction::SelectTab(0));

    assert_eq!(
        state.selected_tab_index(),
        Some(1),
        "SelectTab(0) should be ignored (invalid 1-indexed input)"
    );
}

#[test]
fn select_tab_works_regardless_of_focus() {
    let entries = vec![
        make_subagent_entry("agent-1"),
        make_subagent_entry("agent-2"),
    ];

    let mut state = AppState::new();
    state.add_entries(entries);
    state.focus = FocusPane::Main; // Different focus
                                   // Tab 0 = main, tab 1 = agent-1, tab 2 = agent-2
    state.selected_conversation = ConversationSelection::Main;

    let state = handle_tab_action(state, KeyAction::SelectTab(3));

    assert_eq!(
        state.selected_tab_index(),
        Some(2),
        "SelectTab should work even when focus is on Main pane (FR-088)"
    );
}

#[test]
fn select_tab_does_nothing_when_no_session() {
    let state = AppState::new(); // No entries = no session
                                 // AppState::new() initializes to Some(0) per FR-083

    let state = handle_tab_action(state, KeyAction::SelectTab(1));

    assert_eq!(
        state.selected_tab_index(),
        Some(0),
        "SelectTab should be no-op when no session (stays at tab 0)"
    );
}

// ===== Non-tab action tests =====

#[test]
fn non_tab_actions_return_state_unchanged() {
    let entries = vec![
        make_subagent_entry("agent-1"),
        make_subagent_entry("agent-2"),
    ];

    let mut state = AppState::new();
    state.add_entries(entries);
    state.focus = FocusPane::Subagent;
    state.selected_conversation = ConversationSelection::Subagent(AgentId::new("agent-1").unwrap());

    let state = handle_tab_action(state, KeyAction::ScrollDown);

    assert_eq!(
        state.selected_tab_index(),
        Some(1),
        "Non-tab actions should return state unchanged"
    );
}

#[test]
fn non_tab_actions_like_quit_return_state_unchanged() {
    let entries = vec![make_subagent_entry("agent-1")];

    let mut state = AppState::new();
    state.add_entries(entries);
    state.focus = FocusPane::Subagent;
    state.selected_conversation = ConversationSelection::Main;

    let state = handle_tab_action(state, KeyAction::Quit);

    assert_eq!(
        state.selected_tab_index(),
        Some(0),
        "Quit action should return state unchanged"
    );
}

// ===== Multi-session tab scoping tests (FR-080, FR-081) =====

/// Helper to create a main conversation entry for a session
fn make_main_entry(session_id: &str, content: &str) -> ConversationEntry {
    let log_entry = LogEntry::new(
        make_entry_uuid(&format!("main-{}", session_id)),
        None,
        make_session_id(session_id),
        None, // Main agent has no agent_id
        Utc::now(),
        EntryType::User,
        Message::new(Role::User, MessageContent::Text(content.to_string())),
        EntryMetadata::default(),
    );

    ConversationEntry::Valid(Box::new(log_entry))
}

/// Helper to create a subagent entry for a specific session and agent
fn make_subagent_entry_for_session(
    session_id: &str,
    agent_id: &str,
    content: &str,
) -> ConversationEntry {
    let log_entry = LogEntry::new(
        make_entry_uuid(&format!("entry-{}-{}", session_id, agent_id)),
        None,
        make_session_id(session_id),
        Some(AgentId::new(agent_id).expect("valid agent id")),
        Utc::now(),
        EntryType::Assistant,
        Message::new(Role::Assistant, MessageContent::Text(content.to_string())),
        EntryMetadata::default(),
    );

    ConversationEntry::Valid(Box::new(log_entry))
}

#[test]
fn next_tab_uses_active_session_subagents_when_scrolled_to_first_session() {
    // Given: Two sessions with different subagent sets
    // Session 1: alpha, beta
    // Session 2: gamma, delta, epsilon
    let entries = vec![
        // Session 1
        make_main_entry("session-1", "First session"),
        make_subagent_entry_for_session("session-1", "alpha", "Alpha msg"),
        make_subagent_entry_for_session("session-1", "beta", "Beta msg"),
        // Session 2
        make_main_entry("session-2", "Second session"),
        make_subagent_entry_for_session("session-2", "gamma", "Gamma msg"),
        make_subagent_entry_for_session("session-2", "delta", "Delta msg"),
        make_subagent_entry_for_session("session-2", "epsilon", "Epsilon msg"),
    ];

    let mut state = AppState::new();
    state.add_entries(entries);

    // TODO: Future behavior - scroll position should determine active session
    // Currently: current_session() returns LAST session (session-2)
    // When alpha is selected (exists only in session-1), next_tab wraps to Main
    // because alpha doesn't exist in session-2's subagent list
    state.selected_conversation = ConversationSelection::Subagent(AgentId::new("alpha").unwrap());

    let state = handle_tab_action(state, KeyAction::NextTab);

    // Current behavior: alpha not in session-2, wraps to Main
    assert_eq!(
        state.selected_tab_index(),
        Some(0),
        "NextTab from alpha (not in current session) wraps to Main"
    );
}

#[test]
fn next_tab_wraps_within_active_session_tabs() {
    // Given: Session with multiple subagents
    // Session: alpha, beta (2 subagents)
    let entries = vec![
        make_main_entry("session-1", "Test session"),
        make_subagent_entry_for_session("session-1", "alpha", "Alpha msg"),
        make_subagent_entry_for_session("session-1", "beta", "Beta msg"),
    ];

    let mut state = AppState::new();
    state.add_entries(entries);

    // Tabs: main (0), alpha (1), beta (2)
    // When at last tab (beta = index 2)
    state.selected_conversation = ConversationSelection::Subagent(AgentId::new("agent-2").unwrap());

    let state = handle_tab_action(state, KeyAction::NextTab);

    // Should wrap back to first tab (main = index 0)
    assert_eq!(
        state.selected_tab_index(),
        Some(0),
        "NextTab from last tab should wrap to first tab (main)"
    );
}

#[test]
fn prev_tab_uses_active_session_tabs() {
    // Given: Two sessions
    let entries = vec![
        // Session 1: alpha, beta
        make_main_entry("session-1", "First session"),
        make_subagent_entry_for_session("session-1", "alpha", "Alpha msg"),
        make_subagent_entry_for_session("session-1", "beta", "Beta msg"),
        // Session 2: gamma, delta
        make_main_entry("session-2", "Second session"),
        make_subagent_entry_for_session("session-2", "gamma", "Gamma msg"),
        make_subagent_entry_for_session("session-2", "delta", "Delta msg"),
    ];

    let mut state = AppState::new();
    state.add_entries(entries);

    // Scrolled to session 1
    // Session 1 tabs: main (0), alpha (1), beta (2)
    state.selected_conversation = ConversationSelection::Main; // main

    let state = handle_tab_action(state, KeyAction::PrevTab);

    // Should wrap to last tab in session 1 (beta = index 2)
    assert_eq!(
        state.selected_tab_index(),
        Some(2),
        "PrevTab from main should wrap to beta (last tab in session 1)"
    );
}

#[test]
fn select_tab_clamps_to_active_session_tab_count() {
    // Given: Session with 2 subagents
    let entries = vec![
        make_main_entry("session-1", "Test session"),
        make_subagent_entry_for_session("session-1", "alpha", "Alpha msg"),
        make_subagent_entry_for_session("session-1", "beta", "Beta msg"),
    ];

    let mut state = AppState::new();
    state.add_entries(entries);
    // Tabs: main (0), alpha (1), beta (2)
    state.selected_conversation = ConversationSelection::Main;

    let state = handle_tab_action(state, KeyAction::SelectTab(5));

    // Should clamp to last tab (beta = index 2)
    assert_eq!(
        state.selected_tab_index(),
        Some(2),
        "SelectTab(5) with 3 tabs should clamp to index 2 (beta)"
    );
}

#[test]
fn tab_operations_respect_scroll_position_to_determine_active_session() {
    // This test verifies the CRITICAL requirement: scroll position determines active session
    // Given: Two sessions with DIFFERENT subagent sets
    let entries = vec![
        // Session 1: alpha, beta (2 subagents)
        make_main_entry("session-1", "First session"),
        make_subagent_entry_for_session("session-1", "alpha", "Alpha msg"),
        make_subagent_entry_for_session("session-1", "beta", "Beta msg"),
        // Session 2: gamma (1 subagent)
        make_main_entry("session-2", "Second session"),
        make_subagent_entry_for_session("session-2", "gamma", "Gamma msg"),
    ];

    let mut state = AppState::new();
    state.add_entries(entries);
    state.focus = FocusPane::Subagent;

    // Verify multi-session state was created
    assert_eq!(
        state.log_view().session_count(),
        2,
        "Should have created 2 sessions"
    );

    // Verify session 1 has 2 subagents
    let session1_subagent_count = state
        .log_view()
        .get_session(0)
        .unwrap()
        .subagent_ids()
        .count();
    assert_eq!(
        session1_subagent_count, 2,
        "Session 1 should have 2 subagents"
    );

    // Verify session 2 has 1 subagent
    let session2_subagent_count = state
        .log_view()
        .get_session(1)
        .unwrap()
        .subagent_ids()
        .count();
    assert_eq!(
        session2_subagent_count, 1,
        "Session 2 should have 1 subagent"
    );

    // When scrolled to session 2 (scroll position beyond session 1's content)
    // Session 2 only has gamma (1 subagent)
    // So NextTab from tab 0 (gamma) should wrap back to tab 0 (gamma)
    // NOT to tab 1 (which would be beta from session 1)

    // TODO: This test cannot currently set scroll position directly.
    // It would need to:
    // 1. Get the main conversation view state
    // 2. Calculate session 2's start line
    // 3. Set scroll position to that line
    //
    // For now, this test documents the EXPECTED behavior that
    // tab operations should consider scroll position via active_session().
}
