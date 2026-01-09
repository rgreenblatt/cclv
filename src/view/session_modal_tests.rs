//! Tests for session modal rendering.

use ratatui::backend::TestBackend;
use ratatui::Terminal;

use crate::model::{
    ConversationEntry, EntryMetadata, EntryType, EntryUuid, LogEntry, Message, MessageContent,
    Role, SessionId,
};
use crate::state::AppState;
use crate::view::render_session_modal;
use crate::view_state::types::SessionIndex;
use chrono::Utc;

/// Helper to create a test AppState with multiple sessions.
fn create_test_state_with_sessions(session_count: usize) -> AppState {
    let mut state = AppState::new();

    for i in 0..session_count {
        let session_id = SessionId::new(format!(
            "550e8400-e29b-41d4-a716-44665544000{}",
            i
        ).as_str())
        .unwrap();

        // Add a message to create the session
        let entry = LogEntry::new(
            EntryUuid::new(format!("uuid-session-{}", i)).unwrap(),
            None, // parent_uuid
            session_id,
            None, // main agent
            Utc::now(),
            EntryType::User,
            Message::new(
                Role::User,
                MessageContent::Text(format!("Message in session {}", i + 1)),
            ),
            EntryMetadata::default(),
        );
        state.add_entries(vec![ConversationEntry::Valid(Box::new(entry))]);
    }

    state
}

#[test]
fn render_session_modal_does_not_render_when_invisible() {
    let state = create_test_state_with_sessions(3);
    assert!(!state.session_modal.is_visible());

    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal
        .draw(|frame| {
            render_session_modal(frame, &state);
        })
        .unwrap();

    let buffer = terminal.backend().buffer();

    // When modal is invisible, the buffer should be empty (all default cells)
    // Check that no modal border or content is rendered
    let content = buffer.content().iter()
        .filter(|cell| !cell.symbol().trim().is_empty())
        .count();

    assert_eq!(content, 0, "Modal should not render any content when invisible");
}

#[test]
fn render_session_modal_shows_modal_when_visible() {
    let mut state = create_test_state_with_sessions(3);
    state.session_modal.open(0); // Open modal, select first session
    assert!(state.session_modal.is_visible());

    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal
        .draw(|frame| {
            render_session_modal(frame, &state);
        })
        .unwrap();

    let buffer = terminal.backend().buffer();
    let rendered = buffer.content().iter()
        .map(|cell| cell.symbol())
        .collect::<String>();

    // Modal should render with title
    assert!(
        rendered.contains("Session List"),
        "Modal should display title 'Session List'"
    );
}

#[test]
fn render_session_modal_displays_all_sessions() {
    let mut state = create_test_state_with_sessions(3);
    state.session_modal.open(0);

    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal
        .draw(|frame| {
            render_session_modal(frame, &state);
        })
        .unwrap();

    let buffer = terminal.backend().buffer();
    let rendered = buffer.content().iter()
        .map(|cell| cell.symbol())
        .collect::<String>();

    // All sessions should be listed
    assert!(
        rendered.contains("Session 1:"),
        "Should display Session 1"
    );
    assert!(
        rendered.contains("Session 2:"),
        "Should display Session 2"
    );
    assert!(
        rendered.contains("Session 3:"),
        "Should display Session 3"
    );
}

#[test]
fn render_session_modal_marks_current_session() {
    let mut state = create_test_state_with_sessions(3);

    // Select session 2 (index 1) as current
    let session_idx = SessionIndex::new(1, 3).unwrap();
    state.viewed_session = crate::state::ViewedSession::Pinned(session_idx);

    // Open modal
    state.session_modal.open(1);

    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal
        .draw(|frame| {
            render_session_modal(frame, &state);
        })
        .unwrap();

    let buffer = terminal.backend().buffer();
    let rendered = buffer.content().iter()
        .map(|cell| cell.symbol())
        .collect::<String>();

    // Current session should have [CURRENT] marker
    assert!(
        rendered.contains("[CURRENT]"),
        "Current session should be marked with [CURRENT]"
    );
}

#[test]
fn render_session_modal_shows_footer_with_keybindings() {
    let mut state = create_test_state_with_sessions(2);
    state.session_modal.open(0);

    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal
        .draw(|frame| {
            render_session_modal(frame, &state);
        })
        .unwrap();

    let buffer = terminal.backend().buffer();
    let rendered = buffer.content().iter()
        .map(|cell| cell.symbol())
        .collect::<String>();

    // Footer should show key hints
    // Contract specifies: "↑/↓: Navigate  Enter: Select  Esc: Cancel  S: Close"
    assert!(
        rendered.contains("Navigate") || rendered.contains("↑") || rendered.contains("↓"),
        "Footer should show navigation hints"
    );
    assert!(
        rendered.contains("Enter") || rendered.contains("Select"),
        "Footer should show Enter/Select hint"
    );
    assert!(
        rendered.contains("Esc") || rendered.contains("Cancel"),
        "Footer should show Esc/Cancel hint"
    );
}

#[test]
fn render_session_modal_centers_modal() {
    let mut state = create_test_state_with_sessions(2);
    state.session_modal.open(0);

    let backend = TestBackend::new(100, 30);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal
        .draw(|frame| {
            render_session_modal(frame, &state);
        })
        .unwrap();

    let buffer = terminal.backend().buffer();
    let rendered = buffer.content().iter()
        .map(|cell| cell.symbol())
        .collect::<String>();

    // Modal should be centered and contain the title
    assert!(
        rendered.contains("Session List"),
        "Modal should display centered title 'Session List'"
    );

    // Modal should not be at the very edges - check for border characters in middle region
    // With 60 column width and 100 column terminal, border should be around x=20 to x=80
    let mut found_border_in_middle = false;
    for y in 0..buffer.area.height {
        for x in 15..50 {  // Check middle-ish region
            let cell = buffer.get(x, y);
            let symbol = cell.symbol();
            // Check for border characters (box drawing)
            if symbol == "┌" || symbol == "─" || symbol == "│" || symbol == "└" || symbol == "┐" || symbol == "┘" {
                found_border_in_middle = true;
                break;
            }
        }
        if found_border_in_middle {
            break;
        }
    }

    assert!(found_border_in_middle, "Modal should have borders in middle region (not at edges)");
}
