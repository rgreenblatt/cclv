//! Tests for wrap_handler module.
//!
//! Integration tests for wrap toggle behavior are in src/view/mod.rs
//! These tests verify the pure handler function in isolation.

use super::*;
use crate::model::{
    ConversationEntry, EntryMetadata, EntryType, EntryUuid, LogEntry, Message, MessageContent,
    Role, Session, SessionId,
};
use chrono::Utc;

#[test]
fn handle_toggle_wrap_returns_unchanged_state_when_no_focused_message() {
    let session = Session::new(SessionId::new("test-session").unwrap());
    let mut state = AppState::new(session);

    // Focus on Main pane but no focused message
    state.focus = FocusPane::Main;
    state.main_scroll.set_focused_message(None);

    let initial_overrides_count = state.main_scroll.wrap_overrides.len();

    let result = handle_toggle_wrap(state.clone());

    assert_eq!(
        result.main_scroll.wrap_overrides.len(),
        initial_overrides_count,
        "wrap_overrides should be unchanged when no message is focused"
    );
}

#[test]
fn handle_toggle_wrap_adds_uuid_to_overrides_on_first_toggle() {
    let session = Session::new(SessionId::new("test-session").unwrap());
    let mut state = AppState::new(session);

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

    // Focus on Main pane and set focused message
    state.focus = FocusPane::Main;
    state.main_scroll.set_focused_message(Some(0));

    // Initially no overrides
    assert!(
        !state.main_scroll.wrap_overrides.contains(&uuid),
        "UUID should not be in wrap_overrides initially"
    );

    let result = handle_toggle_wrap(state);

    assert!(
        result.main_scroll.wrap_overrides.contains(&uuid),
        "UUID should be added to wrap_overrides on first toggle"
    );
}

#[test]
fn handle_toggle_wrap_removes_uuid_from_overrides_on_second_toggle() {
    let session = Session::new(SessionId::new("test-session").unwrap());
    let mut state = AppState::new(session);

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

    // Focus on Main pane and set focused message
    state.focus = FocusPane::Main;
    state.main_scroll.set_focused_message(Some(0));

    // First toggle - adds UUID
    let state = handle_toggle_wrap(state);
    assert!(state.main_scroll.wrap_overrides.contains(&uuid));

    // Second toggle - removes UUID
    let result = handle_toggle_wrap(state);
    assert!(
        !result.main_scroll.wrap_overrides.contains(&uuid),
        "UUID should be removed from wrap_overrides on second toggle"
    );
}
