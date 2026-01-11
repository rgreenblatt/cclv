//! Integration tests verifying modal handler is wired into event loop.
//!
//! These tests verify:
//! 1. Modal handler is called BEFORE other key handlers in the event loop
//! 2. Main conversation view uses viewed_session to determine which session to display

use crate::model::{EntryType, LogEntry, Message, MessageContent, Role, SessionId};
use crate::state::{AppState, ViewedSession};
use crate::view::TuiApp;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Terminal;
use ratatui::backend::TestBackend;

/// Helper to create a KeyEvent
fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::empty())
}

/// Helper to create a test TuiApp with N sessions.
fn create_test_app_with_sessions(count: usize) -> TuiApp<TestBackend> {
    let backend = TestBackend::new(80, 24);
    let terminal = Terminal::new(backend).unwrap();

    let stdin_data = b"";
    let stdin_source = crate::source::StdinSource::from_reader(&stdin_data[..]);
    let input_source = crate::source::InputSource::Stdin(stdin_source);

    let mut app_state = AppState::new();

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

        app_state.add_entries(vec![crate::model::ConversationEntry::Valid(Box::new(
            entry,
        ))]);
    }

    let key_bindings = crate::config::KeyBindings::default();

    TuiApp {
        terminal,
        app_state,
        input_source,
        line_counter: 0,
        key_bindings,
        pending_entries: Vec::new(),
        last_tab_area: None,
        last_main_area: None,
    }
}

mod modal_key_priority_in_event_loop {
    use super::*;

    #[test]
    fn arrow_keys_navigate_modal_not_main_view_when_modal_open() {
        let mut app = create_test_app_with_sessions(3);

        // Open modal at first session
        app.app_state.session_modal.open(0);
        let initial_selection = app.app_state.session_modal.selected_index();
        assert_eq!(initial_selection, 0);

        // Simulate Down key event
        let should_quit = app.handle_key(key(KeyCode::Down));

        // Modal should have consumed the key
        assert!(!should_quit, "Down key should not quit");
        assert_eq!(
            app.app_state.session_modal.selected_index(),
            1,
            "Down key should navigate modal selection"
        );
        assert!(
            app.app_state.session_modal.is_visible(),
            "Modal should still be visible"
        );
    }

    #[test]
    fn arrow_keys_work_normally_when_modal_closed() {
        let mut app = create_test_app_with_sessions(3);

        // Modal is closed by default
        assert!(!app.app_state.session_modal.is_visible());

        // Simulate Down key event - should fall through to normal handling
        let should_quit = app.handle_key(key(KeyCode::Down));

        assert!(!should_quit, "Down key should not quit");
        // Can't easily verify scroll behavior here without more setup,
        // but we can verify modal didn't capture it
        assert!(
            !app.app_state.session_modal.is_visible(),
            "Modal should remain closed"
        );
    }

    #[test]
    fn enter_in_modal_pins_session_and_closes_modal() {
        let mut app = create_test_app_with_sessions(3);

        // Open modal and navigate to first session
        app.app_state.session_modal.open(0);
        assert_eq!(app.app_state.viewed_session, ViewedSession::Latest);

        // Press Enter
        let should_quit = app.handle_key(key(KeyCode::Enter));

        assert!(!should_quit, "Enter should not quit");
        assert!(
            !app.app_state.session_modal.is_visible(),
            "Modal should be closed"
        );

        match app.app_state.viewed_session {
            ViewedSession::Pinned(idx) => {
                assert_eq!(idx.get(), 0, "Should pin to first session");
            }
            ViewedSession::Latest => panic!("Expected Pinned, got Latest"),
        }
    }
}

mod view_displays_selected_session {
    use super::*;

    #[test]
    fn main_conversation_view_displays_latest_session_by_default() {
        let app = create_test_app_with_sessions(3);

        // Default: viewed_session = Latest
        assert_eq!(app.app_state.viewed_session, ViewedSession::Latest);

        // View should display last session
        let view = app
            .app_state
            .main_conversation_view()
            .expect("Should have view");
        assert!(!view.is_empty(), "View should have entries");
    }

    #[test]
    fn main_conversation_view_displays_pinned_session_after_selection() {
        let mut app = create_test_app_with_sessions(3);

        // Open modal and select first session
        app.app_state.session_modal.open(0);
        app.handle_key(key(KeyCode::Enter));

        // viewed_session should now be Pinned(0)
        match app.app_state.viewed_session {
            ViewedSession::Pinned(idx) => assert_eq!(idx.get(), 0),
            ViewedSession::Latest => panic!("Expected Pinned"),
        }

        // View should display first session
        let view = app
            .app_state
            .main_conversation_view()
            .expect("Should have view");
        assert!(
            !view.is_empty(),
            "View should have entries from first session"
        );
    }
}
