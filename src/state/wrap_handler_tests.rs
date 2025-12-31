//! Tests for wrap_handler module.
//!
//! Integration tests for wrap toggle behavior are in src/view/mod.rs
//! These tests verify the pure handler function in isolation.

use super::*;
use crate::model::{
    AgentId, ConversationEntry, EntryMetadata, EntryType, EntryUuid, LogEntry, Message,
    MessageContent, Role, SessionId,
};
use crate::state::ConversationSelection;
use crate::view_state::types::EntryIndex;
use chrono::Utc;

#[test]
fn handle_toggle_wrap_returns_unchanged_state_when_no_focused_message() {
    let mut state = AppState::new();

    // Focus on Main pane but no focused message in view-state
    state.focus = FocusPane::Main;
    if let Some(view) = state.main_conversation_view_mut() {
        view.set_focused_message(None);
    }

    let result = handle_toggle_wrap(state.clone(), 80);

    // Verify no wrap override was added (should be no-op)
    // Since there's no entry, there's nothing to check - just verify no panic
    assert_eq!(result.focus, FocusPane::Main);
}

#[test]
fn handle_toggle_wrap_adds_override_on_first_toggle() {
    let mut state = AppState::new();

    // Add an entry to main pane
    let message = Message::new(Role::User, MessageContent::Text("test message".to_string()));
    let uuid = EntryUuid::new("test-uuid-wrap").unwrap();
    let entry = LogEntry::new(
        uuid.clone(),
        None,
        SessionId::new("test-session").unwrap(),
        None,
        Utc::now(),
        EntryType::User,
        message,
        EntryMetadata::default(),
    );
    state.add_entries(vec![ConversationEntry::Valid(Box::new(entry))]);

    // Focus on Main pane and set focused message in view-state
    state.focus = FocusPane::Main;
    if let Some(view) = state.main_conversation_view_mut() {
        view.relayout(80, WrapMode::Wrap, &crate::state::SearchState::Inactive); // Initialize HeightIndex
        view.set_focused_message(Some(EntryIndex::new(0)));
    }

    // Initially no override
    let initial_override = state
        .main_conversation_view()
        .and_then(|view| view.get(EntryIndex::new(0)))
        .and_then(|e| e.wrap_override());
    assert_eq!(
        initial_override, None,
        "Should have no wrap override initially"
    );

    // First toggle
    let result = handle_toggle_wrap(state, 80);

    // Should have override set to opposite of global (global is Wrap by default, so NoWrap)
    let final_override = result
        .main_conversation_view()
        .and_then(|view| view.get(EntryIndex::new(0)))
        .and_then(|e| e.wrap_override());
    assert_eq!(
        final_override,
        Some(WrapMode::NoWrap),
        "First toggle should set override to NoWrap (opposite of default Wrap)"
    );
}

#[test]
fn handle_toggle_wrap_clears_override_on_second_toggle() {
    let mut state = AppState::new();

    // Add an entry to main pane
    let message = Message::new(Role::User, MessageContent::Text("test message".to_string()));
    let uuid = EntryUuid::new("test-uuid-wrap").unwrap();
    let entry = LogEntry::new(
        uuid.clone(),
        None,
        SessionId::new("test-session").unwrap(),
        None,
        Utc::now(),
        EntryType::User,
        message,
        EntryMetadata::default(),
    );
    state.add_entries(vec![ConversationEntry::Valid(Box::new(entry))]);

    // Focus on Main pane and set focused message in view-state
    state.focus = FocusPane::Main;
    if let Some(view) = state.main_conversation_view_mut() {
        view.relayout(80, WrapMode::Wrap, &crate::state::SearchState::Inactive); // Initialize HeightIndex
        view.set_focused_message(Some(EntryIndex::new(0)));
    }

    // First toggle - sets override
    let state = handle_toggle_wrap(state, 80);
    let after_first = state
        .main_conversation_view()
        .and_then(|view| view.get(EntryIndex::new(0)))
        .and_then(|e| e.wrap_override());
    assert_eq!(
        after_first,
        Some(WrapMode::NoWrap),
        "First toggle should set override"
    );

    // Second toggle - clears override
    let result = handle_toggle_wrap(state, 80);
    let after_second = result
        .main_conversation_view()
        .and_then(|view| view.get(EntryIndex::new(0)))
        .and_then(|e| e.wrap_override());
    assert_eq!(
        after_second, None,
        "Second toggle should clear override (return to global)"
    );
}

/// Test that set_entry_wrap_override is called and maintains HeightIndex invariant.
///
/// This verifies that the handler properly delegates to the new HeightIndex-integrated
/// method rather than the old set_wrap_override API.
#[test]
fn test_toggle_wrap_maintains_height_index_invariant() {
    let mut state = AppState::new();

    // Add entries to main pane
    let entries = vec![
        {
            let message = Message::new(Role::User, MessageContent::Text("test 1".to_string()));
            let uuid = EntryUuid::new("uuid-1").unwrap();
            ConversationEntry::Valid(Box::new(LogEntry::new(
                uuid,
                None,
                SessionId::new("session-1").unwrap(),
                None,
                Utc::now(),
                EntryType::User,
                message,
                EntryMetadata::default(),
            )))
        },
        {
            let message = Message::new(Role::User, MessageContent::Text("test 2".to_string()));
            let uuid = EntryUuid::new("uuid-2").unwrap();
            ConversationEntry::Valid(Box::new(LogEntry::new(
                uuid,
                None,
                SessionId::new("session-1").unwrap(),
                None,
                Utc::now(),
                EntryType::User,
                message,
                EntryMetadata::default(),
            )))
        },
    ];
    state.add_entries(entries);

    // Focus and set focused message
    state.focus = FocusPane::Main;
    if let Some(view) = state.main_conversation_view_mut() {
        view.relayout(80, WrapMode::Wrap, &crate::state::SearchState::Inactive);
        view.set_focused_message(Some(EntryIndex::new(0)));
    }

    // Toggle wrap
    let result = handle_toggle_wrap(state, 80);

    // Verify HeightIndex invariant: height_index[i] == entries[i].rendered_lines.len()
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

/// Test that wrap toggle on subagent tab targets the CORRECT subagent.
///
/// BUG: cclv-5ur.46 - Line 45 passes selected_tab directly to subagent_conversation_view_mut,
/// but should convert using (selected_tab - 1) pattern.
///
/// This test creates two subagents (alpha, beta) and verifies that toggling wrap on tab 1
/// (first subagent tab) targets the first subagent (index 0 in sorted list).
#[test]
fn test_toggle_wrap_subagent_correct_indexing() {
    let mut state = AppState::new();

    // Create entries for two different subagents (alpha, beta - sorted alphabetically)
    let agent_alpha = AgentId::new("alpha").unwrap();
    let agent_beta = AgentId::new("beta").unwrap();

    // Add entry for subagent "alpha" (will be index 0 in sorted list)
    let message_alpha = Message::new(
        Role::User,
        MessageContent::Text("alpha message".to_string()),
    );
    let uuid_alpha = EntryUuid::new("uuid-alpha").unwrap();
    let entry_alpha = LogEntry::new(
        uuid_alpha.clone(),
        None,
        SessionId::new("session-1").unwrap(),
        Some(agent_alpha.clone()),
        Utc::now(),
        EntryType::User,
        message_alpha,
        EntryMetadata::default(),
    );

    // Add entry for subagent "beta" (will be index 1 in sorted list)
    let message_beta = Message::new(Role::User, MessageContent::Text("beta message".to_string()));
    let uuid_beta = EntryUuid::new("uuid-beta").unwrap();
    let entry_beta = LogEntry::new(
        uuid_beta.clone(),
        None,
        SessionId::new("session-1").unwrap(),
        Some(agent_beta.clone()),
        Utc::now(),
        EntryType::User,
        message_beta,
        EntryMetadata::default(),
    );

    state.add_entries(vec![
        ConversationEntry::Valid(Box::new(entry_alpha)),
        ConversationEntry::Valid(Box::new(entry_beta)),
    ]);

    // Initialize view states for both subagents
    if let Some(view) = state.subagent_conversation_view_mut(0) {
        view.relayout(80, WrapMode::Wrap, &crate::state::SearchState::Inactive);
        view.set_focused_message(Some(EntryIndex::new(0)));
    }
    if let Some(view) = state.subagent_conversation_view_mut(1) {
        view.relayout(80, WrapMode::Wrap, &crate::state::SearchState::Inactive);
        view.set_focused_message(Some(EntryIndex::new(0)));
    }

    // Select tab 1 (first subagent tab = subagent index 0 = "alpha")
    state.selected_conversation = ConversationSelection::Subagent(agent_alpha.clone());
    state.focus = FocusPane::Subagent;

    // Verify initial state: no wrap override on either subagent
    let alpha_before = state
        .subagent_conversation_view_mut(0)
        .and_then(|view| view.get(EntryIndex::new(0)))
        .and_then(|e| e.wrap_override());
    let beta_before = state
        .subagent_conversation_view_mut(1)
        .and_then(|view| view.get(EntryIndex::new(0)))
        .and_then(|e| e.wrap_override());
    assert_eq!(
        alpha_before, None,
        "alpha should have no wrap override initially"
    );
    assert_eq!(
        beta_before, None,
        "beta should have no wrap override initially"
    );

    // Toggle wrap on selected tab (tab 1 = should target subagent index 0 = "alpha")
    let mut result = handle_toggle_wrap(state, 80);

    // ASSERTION: Wrap override should be set on "alpha" (subagent index 0), NOT "beta"
    let alpha_after = result
        .subagent_conversation_view_mut(0)
        .and_then(|view| view.get(EntryIndex::new(0)))
        .and_then(|e| e.wrap_override());
    let beta_after = result
        .subagent_conversation_view_mut(1)
        .and_then(|view| view.get(EntryIndex::new(0)))
        .and_then(|e| e.wrap_override());

    assert_eq!(
        alpha_after,
        Some(WrapMode::NoWrap),
        "alpha (subagent index 0, tab 1) should have wrap override set"
    );
    assert_eq!(
        beta_after, None,
        "beta (subagent index 1, tab 2) should NOT have wrap override (bug would set this)"
    );
}
