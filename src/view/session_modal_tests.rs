//! Tests for session modal rendering.

use ratatui::Terminal;
use ratatui::backend::TestBackend;

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
        let session_id =
            SessionId::new(format!("550e8400-e29b-41d4-a716-44665544000{}", i).as_str()).unwrap();

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
    let content = buffer
        .content()
        .iter()
        .filter(|cell| !cell.symbol().trim().is_empty())
        .count();

    assert_eq!(
        content, 0,
        "Modal should not render any content when invisible"
    );
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
    let rendered = buffer
        .content()
        .iter()
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
    let rendered = buffer
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect::<String>();

    // All sessions should be listed
    assert!(rendered.contains("Session 1:"), "Should display Session 1");
    assert!(rendered.contains("Session 2:"), "Should display Session 2");
    assert!(rendered.contains("Session 3:"), "Should display Session 3");
}

#[test]
fn render_session_modal_displays_message_and_subagent_counts() {
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
    let rendered = buffer
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect::<String>();

    // Verify format includes message and subagent counts
    // Contract: "Session N: X messages, Y subagents (HH:MM)"
    assert!(
        rendered.contains("messages"),
        "Should display message count with 'messages' label"
    );
    assert!(
        rendered.contains("subagents"),
        "Should display subagent count with 'subagents' label"
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
    let rendered = buffer
        .content()
        .iter()
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
    let rendered = buffer
        .content()
        .iter()
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
    let rendered = buffer
        .content()
        .iter()
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
        for x in 15..50 {
            // Check middle-ish region
            let cell = buffer.get(x, y);
            let symbol = cell.symbol();
            // Check for border characters (box drawing)
            if symbol == "┌"
                || symbol == "─"
                || symbol == "│"
                || symbol == "└"
                || symbol == "┐"
                || symbol == "┘"
            {
                found_border_in_middle = true;
                break;
            }
        }
        if found_border_in_middle {
            break;
        }
    }

    assert!(
        found_border_in_middle,
        "Modal should have borders in middle region (not at edges)"
    );
}

#[test]
fn render_session_modal_styles_current_marker_yellow_italic() {
    let mut state = create_test_state_with_sessions(3);

    // Select session 2 (index 1) as current
    let session_idx = SessionIndex::new(1, 3).unwrap();
    state.viewed_session = crate::state::ViewedSession::Pinned(session_idx);

    // Open modal with selection on a DIFFERENT session (session 1, index 0)
    // This way we can test the [CURRENT] marker styling without highlight override
    state.session_modal.open(0);

    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal
        .draw(|frame| {
            render_session_modal(frame, &state);
        })
        .unwrap();

    let buffer = terminal.backend().buffer();

    // Find the [CURRENT] marker in the buffer
    let mut found_current_marker = false;
    let mut correct_style = false;

    for y in 0..buffer.area.height {
        for x in 0..buffer.area.width.saturating_sub(9) {
            // Look for "[CURRENT]" text
            let text: String = (0..9)
                .map(|offset| buffer.get(x + offset, y).symbol())
                .collect();

            if text == "[CURRENT]" {
                found_current_marker = true;

                // Check styling of the '[' character (first character of marker)
                let cell = buffer.get(x, y);
                let style = cell.style();

                // Per contract line 110: Current marker | Yellow, italic
                let is_yellow = style.fg == Some(ratatui::style::Color::Yellow);
                let is_italic = style
                    .add_modifier
                    .contains(ratatui::style::Modifier::ITALIC);

                correct_style = is_yellow && is_italic;
                break;
            }
        }
        if found_current_marker {
            break;
        }
    }

    assert!(
        found_current_marker,
        "[CURRENT] marker should be present in buffer"
    );
    assert!(
        correct_style,
        "[CURRENT] marker should be styled as yellow and italic per contract line 110"
    );
}

#[test]
fn render_session_modal_calculates_height_per_contract() {
    // Contract line 30: Height = min(session_count + 4, terminal_height - 4)

    // Test case 1: Few sessions, should be session_count + 4
    let mut state1 = create_test_state_with_sessions(3);
    state1.session_modal.open(0);

    let backend1 = TestBackend::new(80, 30);
    let mut terminal1 = Terminal::new(backend1).unwrap();

    let mut actual_height1 = 0;
    terminal1
        .draw(|frame| {
            render_session_modal(frame, &state1);
            // We need to capture the modal height somehow
            // For now, we'll check buffer content to find modal bounds
            let buffer = frame.buffer_mut();
            // Find top border
            let mut top_y = None;
            let mut bottom_y = None;

            for y in 0..buffer.area.height {
                for x in 0..buffer.area.width {
                    let cell = buffer.get(x, y);
                    let symbol = cell.symbol();
                    if (symbol == "┌" || symbol == "╭") && top_y.is_none() {
                        top_y = Some(y);
                    }
                    if symbol == "└" || symbol == "╰" {
                        bottom_y = Some(y);
                    }
                }
            }

            if let (Some(top), Some(bottom)) = (top_y, bottom_y) {
                actual_height1 = bottom - top + 1;
            }
        })
        .unwrap();

    // Expected: min(3 + 4, 30 - 4) = min(7, 26) = 7
    assert_eq!(
        actual_height1, 7,
        "Modal height should be session_count + 4 = 3 + 4 = 7 when terminal is large"
    );

    // Test case 2: Many sessions on small terminal, should be terminal_height - 4
    let mut state2 = create_test_state_with_sessions(20);
    state2.session_modal.open(0);

    let backend2 = TestBackend::new(80, 15);
    let mut terminal2 = Terminal::new(backend2).unwrap();

    let mut actual_height2 = 0;
    terminal2
        .draw(|frame| {
            render_session_modal(frame, &state2);
            let buffer = frame.buffer_mut();
            let mut top_y = None;
            let mut bottom_y = None;

            for y in 0..buffer.area.height {
                for x in 0..buffer.area.width {
                    let cell = buffer.get(x, y);
                    let symbol = cell.symbol();
                    if (symbol == "┌" || symbol == "╭") && top_y.is_none() {
                        top_y = Some(y);
                    }
                    if symbol == "└" || symbol == "╰" {
                        bottom_y = Some(y);
                    }
                }
            }

            if let (Some(top), Some(bottom)) = (top_y, bottom_y) {
                actual_height2 = bottom - top + 1;
            }
        })
        .unwrap();

    // Expected: min(20 + 4, 15 - 4) = min(24, 11) = 11
    assert_eq!(
        actual_height2, 11,
        "Modal height should be terminal_height - 4 = 15 - 4 = 11 when sessions exceed viewport"
    );
}

#[test]
fn render_session_modal_shows_scroll_indicators_when_list_exceeds_viewport() {
    // Create many sessions to exceed a small viewport
    let mut state = create_test_state_with_sessions(20);
    state.session_modal.open(10); // Select middle session

    let backend = TestBackend::new(80, 15); // Small terminal
    let mut terminal = Terminal::new(backend).unwrap();

    terminal
        .draw(|frame| {
            render_session_modal(frame, &state);
        })
        .unwrap();

    let buffer = terminal.backend().buffer();
    let rendered = buffer
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect::<String>();

    // Per contract lines 116-133: Show ▲ in title area when scrolled
    assert!(
        rendered.contains("▲"),
        "Should show ▲ indicator when content extends above viewport"
    );

    // Per contract lines 116-133: Show ▼ in footer area when more content below
    assert!(
        rendered.contains("▼"),
        "Should show ▼ indicator when content extends below viewport"
    );
}

#[test]
fn render_session_modal_no_scroll_indicators_when_all_sessions_visible() {
    // Create few sessions that all fit in viewport
    let mut state = create_test_state_with_sessions(3);
    state.session_modal.open(0);

    let backend = TestBackend::new(80, 30); // Large terminal
    let mut terminal = Terminal::new(backend).unwrap();

    terminal
        .draw(|frame| {
            render_session_modal(frame, &state);
        })
        .unwrap();

    let buffer = terminal.backend().buffer();
    let rendered = buffer
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect::<String>();

    // When all sessions fit, should not show scroll indicators
    assert!(
        !rendered.contains("▲") && !rendered.contains("▼"),
        "Should NOT show scroll indicators when all sessions fit in viewport"
    );
}
