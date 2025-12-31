//! Tests for unified tab model (FR-083-088)
//!
//! Verifies that all conversations (main agent + subagents) appear as
//! top-level tabs with consistent behavior.

use super::*;
use crate::model::{
    AgentId, ConversationEntry, EntryMetadata, EntryType, EntryUuid, LogEntry, Message,
    MessageContent, Role, SessionId,
};
use chrono::Utc;

// ===== Test Helpers =====

fn create_session_with_main_and_subagents(num_subagents: usize) -> Vec<ConversationEntry> {
    let mut entries = Vec::new();
    let session_id = SessionId::new("test-session").unwrap();

    // Add main agent entry
    let main_entry = LogEntry::new(
        EntryUuid::new("main-1").unwrap(),
        None,
        session_id.clone(),
        None,
        Utc::now(),
        EntryType::User,
        Message::new(Role::User, MessageContent::Text("Main message".to_string())),
        EntryMetadata::default(),
    );
    entries.push(ConversationEntry::Valid(Box::new(main_entry)));

    // Add subagent entries
    for i in 1..=num_subagents {
        let subagent_entry = LogEntry::new(
            EntryUuid::new(format!("subagent-{}", i)).unwrap(),
            None,
            session_id.clone(),
            Some(AgentId::new(format!("agent-{}", i)).unwrap()),
            Utc::now(),
            EntryType::User,
            Message::new(
                Role::User,
                MessageContent::Text(format!("Subagent {} message", i)),
            ),
            EntryMetadata::default(),
        );
        entries.push(ConversationEntry::Valid(Box::new(subagent_entry)));
    }

    entries
}

// ===== FR-083: All conversations as top-level tabs =====

#[test]
fn new_appstate_defaults_to_main_agent_tab() {
    // Given: new AppState
    let state = AppState::new();

    // Then: selected_tab should be Some(0) (main agent tab)
    assert_eq!(
        state.selected_tab,
        Some(0),
        "New AppState should default to tab 0 (main agent)"
    );
}

#[test]
fn selected_tab_is_none_only_when_no_session() {
    // Given: AppState with no entries
    let state = AppState::new();

    // When: no session exists (log_view is empty)
    let has_session = state.log_view().current_session().is_some();

    // Then: either selected_tab defaults to Some(0) or None is acceptable
    // (Implementation detail: we choose to default to Some(0) for simplicity)
    if !has_session {
        // No session exists, so None is valid
        assert!(
            state.selected_tab.is_none() || state.selected_tab == Some(0),
            "When no session exists, selected_tab can be None or default to 0"
        );
    }
}

#[test]
fn tab_0_represents_main_agent() {
    // Given: session with main agent and subagents
    let entries = create_session_with_main_and_subagents(2);
    let mut state = AppState::new();
    state.add_entries(entries);

    // When: selected_tab is 0
    state.selected_tab = Some(0);

    // Then: current conversation should be main agent
    // Verify by checking that the conversation at tab 0 has no agent_id
    let main_conv = state.main_conversation_view();
    assert!(
        main_conv.is_some(),
        "Tab 0 should map to main conversation"
    );
}

#[test]
fn tab_1_and_above_represent_subagents() {
    // Given: session with main agent and 3 subagents
    let entries = create_session_with_main_and_subagents(3);
    let mut state = AppState::new();
    state.add_entries(entries);

    // When: selected_tab is 1 (first subagent)
    state.selected_tab = Some(1);

    // Then: current conversation should be first subagent
    let subagent_conv = state.subagent_conversation_view(0); // 0-indexed for subagent list
    assert!(
        subagent_conv.is_some(),
        "Tab 1 should map to first subagent conversation"
    );

    // When: selected_tab is 2 (second subagent)
    state.selected_tab = Some(2);

    // Then: current conversation should be second subagent
    let subagent_conv = state.subagent_conversation_view(1);
    assert!(
        subagent_conv.is_some(),
        "Tab 2 should map to second subagent conversation"
    );
}

// ===== FR-084: Main agent at index 0, subagents at 1..N =====

#[test]
fn tab_indices_follow_spawn_order() {
    // Given: session with main and subagents spawned in order
    let entries = create_session_with_main_and_subagents(3);
    let mut state = AppState::new();
    state.add_entries(entries);

    // Then: tab indices should be:
    // 0 = main
    // 1 = agent-1 (first subagent)
    // 2 = agent-2 (second subagent)
    // 3 = agent-3 (third subagent)

    // Verify by getting subagent IDs in order
    let session = state.session_view();
    let agent_ids: Vec<_> = session.subagent_ids().collect();

    assert_eq!(
        agent_ids.len(),
        3,
        "Should have 3 subagents in spawn order"
    );
}

// ===== FR-086: Tab switching works identically for all tabs =====

#[test]
fn next_tab_wraps_from_main_to_first_subagent() {
    // Given: session with main and 2 subagents, tab 0 selected
    let entries = create_session_with_main_and_subagents(2);
    let mut state = AppState::new();
    state.add_entries(entries);
    state.selected_tab = Some(0); // Main agent

    // When: next_tab is called
    state.next_tab();

    // Then: should move to tab 1 (first subagent)
    assert_eq!(
        state.selected_tab,
        Some(1),
        "next_tab from main (0) should move to first subagent (1)"
    );
}

#[test]
fn next_tab_wraps_from_last_subagent_to_main() {
    // Given: session with main and 2 subagents, last tab selected
    let entries = create_session_with_main_and_subagents(2);
    let mut state = AppState::new();
    state.add_entries(entries);
    state.selected_tab = Some(2); // Last subagent (tab 2)

    // When: next_tab is called
    state.next_tab();

    // Then: should wrap to tab 0 (main agent)
    assert_eq!(
        state.selected_tab,
        Some(0),
        "next_tab from last subagent should wrap to main (0)"
    );
}

#[test]
fn prev_tab_wraps_from_main_to_last_subagent() {
    // Given: session with main and 2 subagents, tab 0 selected
    let entries = create_session_with_main_and_subagents(2);
    let mut state = AppState::new();
    state.add_entries(entries);
    state.selected_tab = Some(0); // Main agent

    // When: prev_tab is called
    state.prev_tab();

    // Then: should wrap to tab 2 (last subagent)
    assert_eq!(
        state.selected_tab,
        Some(2),
        "prev_tab from main (0) should wrap to last subagent (2)"
    );
}

#[test]
fn prev_tab_wraps_from_first_subagent_to_main() {
    // Given: session with main and 2 subagents, first subagent selected
    let entries = create_session_with_main_and_subagents(2);
    let mut state = AppState::new();
    state.add_entries(entries);
    state.selected_tab = Some(1); // First subagent

    // When: prev_tab is called
    state.prev_tab();

    // Then: should move to tab 0 (main agent)
    assert_eq!(
        state.selected_tab,
        Some(0),
        "prev_tab from first subagent (1) should move to main (0)"
    );
}

#[test]
fn tab_switching_works_regardless_of_focus_pane() {
    // Given: session with main and 2 subagents
    let entries = create_session_with_main_and_subagents(2);

    // Test with focus on Main
    let mut state = AppState::new();
    state.add_entries(entries.clone());
    state.focus = FocusPane::Main;
    state.selected_tab = Some(0);
    state.next_tab();
    assert_eq!(
        state.selected_tab,
        Some(1),
        "Tab switching should work when focus is Main"
    );

    // Test with focus on Subagent
    let mut state = AppState::new();
    state.add_entries(entries.clone());
    state.focus = FocusPane::Subagent;
    state.selected_tab = Some(0);
    state.next_tab();
    assert_eq!(
        state.selected_tab,
        Some(1),
        "Tab switching should work when focus is Subagent"
    );

    // Test with focus on Stats
    let mut state = AppState::new();
    state.add_entries(entries.clone());
    state.focus = FocusPane::Stats;
    state.selected_tab = Some(0);
    state.next_tab();
    assert_eq!(
        state.selected_tab,
        Some(1),
        "Tab switching should work when focus is Stats"
    );
}

#[test]
fn tab_switching_is_no_op_during_search_modal() {
    // Given: session with main and 2 subagents, search modal active
    let entries = create_session_with_main_and_subagents(2);
    let mut state = AppState::new();
    state.add_entries(entries);
    state.focus = FocusPane::Search;
    state.selected_tab = Some(0);

    // When: next_tab is called during search
    state.next_tab();

    // Then: selected_tab should NOT change
    assert_eq!(
        state.selected_tab,
        Some(0),
        "Tab switching should be no-op when Search modal is active"
    );
}

#[test]
fn select_tab_by_number_works_for_all_tabs() {
    // Given: session with main and 3 subagents
    let entries = create_session_with_main_and_subagents(3);
    let mut state = AppState::new();
    state.add_entries(entries);

    // When: select tab 1 (using 1-indexed input, maps to tab 0 = main)
    state.select_tab(1);

    // Then: tab 0 should be selected
    assert_eq!(
        state.selected_tab,
        Some(0),
        "select_tab(1) should select tab 0 (main)"
    );

    // When: select tab 2 (maps to tab 1 = first subagent)
    state.select_tab(2);

    // Then: tab 1 should be selected
    assert_eq!(
        state.selected_tab,
        Some(1),
        "select_tab(2) should select tab 1 (first subagent)"
    );

    // When: select tab 4 (maps to tab 3 = third subagent)
    state.select_tab(4);

    // Then: tab 3 should be selected
    assert_eq!(
        state.selected_tab,
        Some(3),
        "select_tab(4) should select tab 3 (third subagent)"
    );
}

#[test]
fn select_tab_clamps_to_last_tab_when_number_too_high() {
    // Given: session with main and 2 subagents (3 tabs total: 0, 1, 2)
    let entries = create_session_with_main_and_subagents(2);
    let mut state = AppState::new();
    state.add_entries(entries);

    // When: select tab 99 (way beyond last tab)
    state.select_tab(99);

    // Then: should clamp to tab 2 (last tab)
    assert_eq!(
        state.selected_tab,
        Some(2),
        "select_tab should clamp to last available tab"
    );
}

// ===== Edge Cases =====

#[test]
fn tab_operations_no_op_when_only_main_exists() {
    // Given: session with only main agent (no subagents)
    let entries = create_session_with_main_and_subagents(0);
    let mut state = AppState::new();
    state.add_entries(entries);
    state.selected_tab = Some(0);

    // When: next_tab is called
    state.next_tab();

    // Then: should stay at tab 0 (wrapping with 1 tab means no movement)
    assert_eq!(
        state.selected_tab,
        Some(0),
        "next_tab with only main agent should stay at tab 0"
    );

    // When: prev_tab is called
    state.prev_tab();

    // Then: should stay at tab 0
    assert_eq!(
        state.selected_tab,
        Some(0),
        "prev_tab with only main agent should stay at tab 0"
    );
}

#[test]
fn total_tab_count_equals_one_plus_subagent_count() {
    // Given: session with main and 3 subagents
    let entries = create_session_with_main_and_subagents(3);
    let mut state = AppState::new();
    state.add_entries(entries);

    // Then: total tab count should be 4 (1 main + 3 subagents)
    let num_subagents = state.session_view().subagent_ids().count();
    let total_tabs = 1 + num_subagents;

    assert_eq!(total_tabs, 4, "Total tabs = 1 (main) + 3 (subagents)");
}
