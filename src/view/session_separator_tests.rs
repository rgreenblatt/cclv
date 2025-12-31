//! Tests for session separator rendering (FR-074)

use crate::model::{
    ConversationEntry, EntryMetadata, EntryType, EntryUuid, LogEntry, Message, MessageContent,
    Role, SessionId,
};
use crate::state::app_state::WrapMode;
use crate::view::message::ConversationView;
use crate::view::MessageStyles;
use crate::view_state::conversation::ConversationViewState;
use crate::view_state::layout_params::LayoutParams;
use ratatui::{backend::TestBackend, buffer::Buffer, layout::Rect, Terminal};

// ===== Test Helpers =====

fn make_session_id(s: &str) -> SessionId {
    SessionId::new(s).expect("valid session id")
}

fn make_entry_uuid(s: &str) -> EntryUuid {
    EntryUuid::new(s).expect("valid uuid")
}

fn make_timestamp() -> chrono::DateTime<chrono::Utc> {
    "2025-12-25T10:00:00Z".parse().expect("valid timestamp")
}

fn make_message(text: &str) -> Message {
    Message::new(Role::User, MessageContent::Text(text.to_string()))
}

fn make_valid_entry(uuid: &str, session_id: &str, text: &str) -> ConversationEntry {
    let log_entry = LogEntry::new(
        make_entry_uuid(uuid),
        None,
        make_session_id(session_id),
        None,
        make_timestamp(),
        EntryType::User,
        make_message(text),
        EntryMetadata::default(),
    );
    ConversationEntry::Valid(Box::new(log_entry))
}

// ===== Session Separator Tests =====

#[test]
fn renders_separator_when_session_changes() {
    // Create entries from two different sessions
    let entries = vec![
        make_valid_entry("uuid-1", "session-1", "Message in session 1"),
        make_valid_entry("uuid-2", "session-1", "Another message in session 1"),
        make_valid_entry("uuid-3", "session-2", "First message in session 2"),
        make_valid_entry("uuid-4", "session-2", "Second message in session 2"),
    ];

    let mut view_state = ConversationViewState::new(
        None,
        None,
        entries,
        200_000,
        crate::model::PricingConfig::default(),
    );

    // Compute layout
    let params = LayoutParams::new(80, WrapMode::Wrap);
    view_state.recompute_layout(params);

    // Render the conversation
    let styles = MessageStyles::default();
    let widget = ConversationView::new(&view_state, &styles, false);

    let mut terminal = Terminal::new(TestBackend::new(80, 20)).unwrap();
    terminal
        .draw(|f| {
            let area = Rect::new(0, 0, 80, 20);
            f.render_widget(widget, area);
        })
        .unwrap();

    let buffer = terminal.backend().buffer();

    // Find separator line in rendered output
    // Separator should appear before "First message in session 2"
    let separator_found = buffer_contains_session_separator(buffer, "session-2");

    assert!(
        separator_found,
        "Should render session separator before session-2 starts"
    );
}

#[test]
fn no_separator_for_first_session() {
    // Create entries from single session
    let entries = vec![
        make_valid_entry("uuid-1", "session-1", "First message"),
        make_valid_entry("uuid-2", "session-1", "Second message"),
    ];

    let mut view_state = ConversationViewState::new(
        None,
        None,
        entries,
        200_000,
        crate::model::PricingConfig::default(),
    );

    // Compute layout
    let params = LayoutParams::new(80, WrapMode::Wrap);
    view_state.recompute_layout(params);

    // Render the conversation
    let styles = MessageStyles::default();
    let widget = ConversationView::new(&view_state, &styles, false);

    let mut terminal = Terminal::new(TestBackend::new(80, 20)).unwrap();
    terminal
        .draw(|f| {
            let area = Rect::new(0, 0, 80, 20);
            f.render_widget(widget, area);
        })
        .unwrap();

    let buffer = terminal.backend().buffer();

    // Should NOT find any session separator
    let separator_found = buffer_contains_text(buffer, "Session:");

    assert!(
        !separator_found,
        "Should NOT render separator for first session"
    );
}

#[test]
fn separator_contains_session_id() {
    // Create entries from two sessions
    let entries = vec![
        make_valid_entry("uuid-1", "session-alpha", "Message 1"),
        make_valid_entry("uuid-2", "session-beta", "Message 2"),
    ];

    let mut view_state = ConversationViewState::new(
        None,
        None,
        entries,
        200_000,
        crate::model::PricingConfig::default(),
    );

    // Compute layout
    let params = LayoutParams::new(80, WrapMode::Wrap);
    view_state.recompute_layout(params);

    // Render
    let styles = MessageStyles::default();
    let widget = ConversationView::new(&view_state, &styles, false);

    let mut terminal = Terminal::new(TestBackend::new(80, 20)).unwrap();
    terminal
        .draw(|f| {
            let area = Rect::new(0, 0, 80, 20);
            f.render_widget(widget, area);
        })
        .unwrap();

    let buffer = terminal.backend().buffer();

    // Separator should contain "session-beta"
    let separator_has_session_id = buffer_contains_text(buffer, "session-beta");

    assert!(
        separator_has_session_id,
        "Separator should display session ID"
    );
}

#[test]
fn multiple_session_changes_render_multiple_separators() {
    // Create entries across three sessions
    let entries = vec![
        make_valid_entry("uuid-1", "session-1", "Session 1"),
        make_valid_entry("uuid-2", "session-2", "Session 2"),
        make_valid_entry("uuid-3", "session-3", "Session 3"),
    ];

    let mut view_state = ConversationViewState::new(
        None,
        None,
        entries,
        200_000,
        crate::model::PricingConfig::default(),
    );

    // Compute layout
    let params = LayoutParams::new(80, WrapMode::Wrap);
    view_state.recompute_layout(params);

    // Render
    let styles = MessageStyles::default();
    let widget = ConversationView::new(&view_state, &styles, false);

    let mut terminal = Terminal::new(TestBackend::new(80, 20)).unwrap();
    terminal
        .draw(|f| {
            let area = Rect::new(0, 0, 80, 20);
            f.render_widget(widget, area);
        })
        .unwrap();

    let buffer = terminal.backend().buffer();

    // Should find separators for session-2 and session-3
    let sep2_found = buffer_contains_session_separator(buffer, "session-2");
    let sep3_found = buffer_contains_session_separator(buffer, "session-3");

    assert!(sep2_found, "Should render separator for session-2");
    assert!(sep3_found, "Should render separator for session-3");
}

#[test]
fn separator_styled_with_dim_gray() {
    // Create entries from two sessions
    let entries = vec![
        make_valid_entry("uuid-1", "session-1", "Message 1"),
        make_valid_entry("uuid-2", "session-2", "Message 2"),
    ];

    let mut view_state = ConversationViewState::new(
        None,
        None,
        entries,
        200_000,
        crate::model::PricingConfig::default(),
    );

    // Compute layout
    let params = LayoutParams::new(80, WrapMode::Wrap);
    view_state.recompute_layout(params);

    // Render
    let styles = MessageStyles::default();
    let widget = ConversationView::new(&view_state, &styles, false);

    let mut terminal = Terminal::new(TestBackend::new(80, 20)).unwrap();
    terminal
        .draw(|f| {
            let area = Rect::new(0, 0, 80, 20);
            f.render_widget(widget, area);
        })
        .unwrap();

    let buffer = terminal.backend().buffer();

    // Find the separator line and check its styling
    let separator_is_dimmed = buffer_separator_is_dimmed(buffer, "session-2");

    assert!(separator_is_dimmed, "Separator should use dim/gray styling");
}

// ===== Buffer Inspection Helpers =====

/// Check if buffer contains a session separator for given session ID.
fn buffer_contains_session_separator(buffer: &Buffer, session_id: &str) -> bool {
    // Separator format: "─────────── Session: <session_id> ───────────"
    let separator_text = format!("Session: {}", session_id);
    buffer_contains_text(buffer, &separator_text)
}

/// Check if buffer contains given text anywhere.
fn buffer_contains_text(buffer: &Buffer, text: &str) -> bool {
    for y in 0..buffer.area.height {
        let mut line_content = String::new();
        for x in 0..buffer.area.width {
            let cell = buffer.get(x, y);
            line_content.push_str(cell.symbol());
        }
        if line_content.contains(text) {
            return true;
        }
    }
    false
}

/// Check if session separator has dim/gray styling.
fn buffer_separator_is_dimmed(buffer: &Buffer, session_id: &str) -> bool {
    use ratatui::style::{Color, Modifier};

    let separator_text = format!("Session: {}", session_id);

    // Find line containing separator
    for y in 0..buffer.area.height {
        let mut line_content = String::new();
        for x in 0..buffer.area.width {
            let cell = buffer.get(x, y);
            line_content.push_str(cell.symbol());
        }

        if line_content.contains(&separator_text) {
            // Check if any cell in this line has dim/gray styling
            for x in 0..buffer.area.width {
                let cell = buffer.get(x, y);
                let style = cell.style();

                // Accept either DarkGray foreground or DIM modifier
                let is_dark_gray =
                    style.fg == Some(Color::DarkGray) || style.fg == Some(Color::Gray);
                let is_dimmed = style.add_modifier.contains(Modifier::DIM);

                if is_dark_gray || is_dimmed {
                    return true;
                }
            }
        }
    }

    false
}
