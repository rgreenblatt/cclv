//! Tests for session navigation type wiring in AppState.
//!
//! These tests verify that:
//! 1. AppState has the required fields for session navigation
//! 2. The fields are initialized with sensible defaults
//! 3. The types are accessible through public module paths

use super::*;
use crate::state::ViewedSession;

#[test]
fn appstate_has_session_modal_field() {
    let state = AppState::new();

    // Verify session_modal field exists and has sensible default
    assert!(!state.session_modal.is_visible());
}

#[test]
fn appstate_has_viewed_session_field() {
    let state = AppState::new();

    // Verify viewed_session field exists and defaults to Latest
    assert_eq!(state.viewed_session, ViewedSession::Latest);
}

#[test]
fn can_import_session_types_from_state_module() {
    // Test that we can access types through public paths
    use crate::state::{SessionModalState, ViewedSession};

    let _modal = SessionModalState::new();
    let _session = ViewedSession::default();

    // If this compiles, the types are properly exported
}

#[test]
fn can_import_view_state_types_from_view_state_module() {
    // Test that we can access view-state types through public paths
    use crate::view_state::{SessionIndex, SessionSummary};

    // Create a validated session index
    let index = SessionIndex::new(0, 3).expect("Valid session index");

    // Create a session summary
    use crate::model::SessionId;
    let session_id = SessionId::new("550e8400-e29b-41d4-a716-446655440000").unwrap();
    let _summary = SessionSummary::new(index, session_id, 10, None, 2);

    // If this compiles, the types are properly exported
}

#[test]
fn session_modal_default_is_closed() {
    let state = AppState::new();

    // Verify the modal starts in a closed state
    assert!(!state.session_modal.is_visible());
}

#[test]
fn viewed_session_default_is_latest() {
    let state = AppState::new();

    // Verify we default to viewing the latest session
    assert_eq!(state.viewed_session, ViewedSession::Latest);
}

#[test]
fn is_tailing_enabled_returns_false_when_auto_scroll_is_false() {
    let mut state = AppState::new();
    state.auto_scroll = false;

    // Even if viewing last session, auto_scroll=false means no tailing
    let result = state.is_tailing_enabled();

    assert!(!result);
}

#[test]
fn is_tailing_enabled_returns_false_when_viewing_historical_session() {
    // Create a state with multiple sessions and viewing a historical one
    use crate::model::ConversationEntry;
    use crate::parser;

    let mut state = AppState::new();
    state.auto_scroll = true;

    // Add entries from two different sessions to create multiple sessions
    let session1 = "550e8400-e29b-41d4-a716-446655440000";
    let session2 = "550e8400-e29b-41d4-a716-446655440001";

    // Add entries from session 1
    for i in 0..3 {
        let json = format!(
            r#"{{"timestamp":"2024-01-01T00:00:0{}Z","type":"user_message","role":"user","content":[{{"type":"text","text":"Session 1 message {}"}}],"session_id":"{}","agent_id":"main","uuid":"uuid-1-{}"}}"#,
            i, i, session1, i
        );
        let entry = ConversationEntry::from(parser::parse_entry_graceful(&json, i + 1));
        state.add_entries(vec![entry]);
    }

    // Add entries from session 2
    for i in 0..3 {
        let json = format!(
            r#"{{"timestamp":"2024-01-01T00:01:0{}Z","type":"user_message","role":"user","content":[{{"type":"text","text":"Session 2 message {}"}}],"session_id":"{}","agent_id":"main","uuid":"uuid-2-{}"}}"#,
            i, i, session2, i
        );
        let entry = ConversationEntry::from(parser::parse_entry_graceful(&json, 10 + i));
        state.add_entries(vec![entry]);
    }

    // Pin to first session (historical)
    let session_count = state.log_view().session_count();
    state.viewed_session = ViewedSession::pinned(0, session_count).unwrap();

    // Even though auto_scroll=true, we're viewing historical session
    let result = state.is_tailing_enabled();

    assert!(!result);
}

#[test]
fn is_tailing_enabled_returns_true_when_auto_scroll_and_viewing_last_session() {
    // Create a state with multiple sessions and viewing the last one
    use crate::model::ConversationEntry;
    use crate::parser;

    let mut state = AppState::new();
    state.auto_scroll = true;

    // Add entries from two different sessions to create multiple sessions
    let session1 = "550e8400-e29b-41d4-a716-446655440000";
    let session2 = "550e8400-e29b-41d4-a716-446655440001";

    // Add entries from session 1
    for i in 0..3 {
        let json = format!(
            r#"{{"timestamp":"2024-01-01T00:00:0{}Z","type":"user_message","role":"user","content":[{{"type":"text","text":"Session 1 message {}"}}],"session_id":"{}","agent_id":"main","uuid":"uuid-1-{}"}}"#,
            i, i, session1, i
        );
        let entry = ConversationEntry::from(parser::parse_entry_graceful(&json, i + 1));
        state.add_entries(vec![entry]);
    }

    // Add entries from session 2
    for i in 0..3 {
        let json = format!(
            r#"{{"timestamp":"2024-01-01T00:01:0{}Z","type":"user_message","role":"user","content":[{{"type":"text","text":"Session 2 message {}"}}],"session_id":"{}","agent_id":"main","uuid":"uuid-2-{}"}}"#,
            i, i, session2, i
        );
        let entry = ConversationEntry::from(parser::parse_entry_graceful(&json, 10 + i));
        state.add_entries(vec![entry]);
    }

    // Default state should be viewing latest (last) session
    assert_eq!(state.viewed_session, ViewedSession::Latest);

    // Both conditions met: auto_scroll=true AND viewing last session
    let result = state.is_tailing_enabled();

    assert!(result);
}
