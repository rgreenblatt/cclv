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
    use crate::state::{ViewedSession, SessionModalState};

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
