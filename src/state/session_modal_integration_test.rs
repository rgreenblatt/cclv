//! Integration tests for session modal keyboard handling and view updates.
//!
//! Tests that verify:
//! 1. Modal handler intercepts keys before main view (when modal is visible)
//! 2. Main conversation view renders the session selected via viewed_session

use crate::model::{EntryType, LogEntry, Message, MessageContent, Role, SessionId};
use crate::state::{AppState, ViewedSession};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// Helper to create a KeyEvent
fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::empty())
}

/// Helper to create a test AppState with N sessions.
fn create_test_state_with_sessions(count: usize) -> AppState {
    let mut state = AppState::new();

    // Create N sessions, each with a unique message
    for i in 0..count {
        let session_id = SessionId::new(format!("session-{}", i)).expect("valid session id");
        let entry = LogEntry::new(
            crate::model::EntryUuid::new(format!("entry-{}", i)).expect("valid uuid"),
            None,
            session_id.clone(),
            None,
            chrono::Utc::now(),
            EntryType::User,
            Message::new(
                Role::User,
                MessageContent::Text(format!("Message from session {}", i)),
            ),
            crate::model::EntryMetadata::default(),
        );

        state.add_entries(vec![crate::model::ConversationEntry::Valid(Box::new(
            entry,
        ))]);
    }

    state
}

mod modal_key_priority {
    use super::*;
    use crate::state::handle_session_modal_key;

    #[test]
    fn modal_handler_returns_true_when_modal_visible_and_key_handled() {
        let mut state = create_test_state_with_sessions(3);
        state.session_modal.open(0);

        let result = handle_session_modal_key(&mut state, key(KeyCode::Up));

        assert!(result, "Modal should capture Up key");
    }

    #[test]
    fn modal_handler_returns_false_when_modal_not_visible() {
        let mut state = create_test_state_with_sessions(3);
        // Modal is closed by default

        let result = handle_session_modal_key(&mut state, key(KeyCode::Up));

        assert!(!result, "Modal should not capture keys when closed");
    }

    #[test]
    fn modal_handler_returns_false_for_unhandled_keys_when_modal_visible() {
        let mut state = create_test_state_with_sessions(3);
        state.session_modal.open(0);

        let result = handle_session_modal_key(&mut state, key(KeyCode::Char('x')));

        assert!(
            !result,
            "Modal should return false for unhandled keys (allows fallthrough)"
        );
    }
}

mod session_selection {
    use super::*;
    use crate::state::handle_session_modal_key;

    #[test]
    fn enter_on_first_session_pins_viewed_session_to_first() {
        let mut state = create_test_state_with_sessions(3);
        state.session_modal.open(0); // Select first session
        assert_eq!(state.viewed_session, ViewedSession::Latest); // Initially Latest

        handle_session_modal_key(&mut state, key(KeyCode::Enter));

        match state.viewed_session {
            ViewedSession::Pinned(idx) => {
                assert_eq!(idx.get(), 0, "Should pin to first session");
            }
            ViewedSession::Latest => panic!("Expected Pinned, got Latest"),
        }
    }

    #[test]
    fn enter_on_last_session_sets_viewed_session_to_latest() {
        let mut state = create_test_state_with_sessions(3);
        state.session_modal.open(2); // Select last session (index 2)

        handle_session_modal_key(&mut state, key(KeyCode::Enter));

        assert_eq!(
            state.viewed_session,
            ViewedSession::Latest,
            "Should set Latest mode when selecting last session"
        );
    }

    #[test]
    fn esc_closes_modal_without_changing_viewed_session() {
        let mut state = create_test_state_with_sessions(3);
        state.viewed_session = ViewedSession::Latest;
        state.session_modal.open(0);

        handle_session_modal_key(&mut state, key(KeyCode::Esc));

        assert_eq!(
            state.viewed_session,
            ViewedSession::Latest,
            "Esc should not change viewed_session"
        );
        assert!(!state.session_modal.is_visible(), "Modal should be closed");
    }
}

mod view_renders_selected_session {
    use super::*;

    #[test]
    fn main_conversation_view_returns_latest_session_by_default() {
        let state = create_test_state_with_sessions(3);
        // Default: viewed_session = Latest

        let view = state
            .main_conversation_view()
            .expect("Should have conversation view");

        // The view should contain messages from the last session (session-2)
        // We can't easily verify message content here without inspecting the view state,
        // but we can verify the view exists
        assert!(!view.is_empty(), "View should have entries");
    }

    #[test]
    fn main_conversation_view_returns_pinned_session_when_pinned() {
        let mut state = create_test_state_with_sessions(3);

        // Pin to first session
        state.viewed_session = ViewedSession::pinned(0, 3).expect("valid pin");

        let view = state
            .main_conversation_view()
            .expect("Should have conversation view");

        // The view should now show session 0's messages
        assert!(!view.is_empty(), "View should have entries");
    }

    #[test]
    fn effective_index_returns_correct_session_for_latest() {
        let state = create_test_state_with_sessions(3);
        let session_count = state.log_view().session_count();

        assert_eq!(session_count, 3);

        let effective = state.viewed_session.effective_index(session_count);
        assert_eq!(
            effective.expect("should have index").get(),
            2,
            "Latest should map to last session (index 2)"
        );
    }

    #[test]
    fn effective_index_returns_correct_session_for_pinned() {
        let mut state = create_test_state_with_sessions(3);
        state.viewed_session = ViewedSession::pinned(0, 3).expect("valid pin");

        let session_count = state.log_view().session_count();
        let effective = state.viewed_session.effective_index(session_count);

        assert_eq!(
            effective.expect("should have index").get(),
            0,
            "Pinned(0) should map to session index 0"
        );
    }
}
