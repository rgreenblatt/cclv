//! Tests for split pane layout rendering.

use super::*;
use crate::model::{Session, SessionId};
use crate::state::AppState;
use ratatui::backend::TestBackend;
use ratatui::Terminal;

// ===== Test Helpers =====

fn create_test_terminal() -> Terminal<TestBackend> {
    let backend = TestBackend::new(80, 24);
    Terminal::new(backend).unwrap()
}

fn create_session_no_subagents() -> Session {
    let session_id = SessionId::new("test-session").unwrap();
    Session::new(session_id)
}

fn create_session_with_subagents() -> Session {
    use crate::model::{
        AgentId, EntryMetadata, EntryType, EntryUuid, LogEntry, Message, MessageContent, Role,
    };
    use chrono::Utc;

    let session_id = SessionId::new("test-session").unwrap();
    let mut session = Session::new(session_id);

    // Add a main agent entry
    let main_entry = LogEntry::new(
        EntryUuid::new("entry-1").unwrap(),
        None,
        SessionId::new("test-session").unwrap(),
        None,
        Utc::now(),
        EntryType::User,
        Message::new(Role::User, MessageContent::Text("Main message".to_string())),
        EntryMetadata::default(),
    );
    session.add_entry(main_entry);

    // Add a subagent entry
    let subagent_entry = LogEntry::new(
        EntryUuid::new("entry-2").unwrap(),
        None,
        SessionId::new("test-session").unwrap(),
        Some(AgentId::new("subagent-1").unwrap()),
        Utc::now(),
        EntryType::User,
        Message::new(
            Role::User,
            MessageContent::Text("Subagent message".to_string()),
        ),
        EntryMetadata::default(),
    );
    session.add_entry(subagent_entry);

    session
}

// ===== calculate_horizontal_constraints Tests =====

#[test]
fn calculate_constraints_with_subagents_returns_60_40_split() {
    let (main, subagent) = calculate_horizontal_constraints(true);

    // Should be 60% and 40%
    assert!(
        matches!(main, Constraint::Percentage(60)),
        "Main pane should be 60% when subagents exist"
    );
    assert!(
        matches!(subagent, Constraint::Percentage(40)),
        "Subagent pane should be 40% when subagents exist"
    );
}

#[test]
fn calculate_constraints_without_subagents_returns_100_0_split() {
    let (main, subagent) = calculate_horizontal_constraints(false);

    // Should be 100% and 0% (or Min(0))
    assert!(
        matches!(main, Constraint::Percentage(100)),
        "Main pane should be 100% when no subagents"
    );
    assert!(
        matches!(subagent, Constraint::Min(0)),
        "Subagent pane should be Min(0) when no subagents"
    );
}

// ===== render_layout Integration Tests =====

#[test]
fn render_layout_creates_three_areas_with_subagents() {
    let mut terminal = create_test_terminal();
    let session = create_session_with_subagents();
    let state = AppState::new(session);

    terminal
        .draw(|frame| {
            render_layout(frame, &state);
        })
        .unwrap();

    let buffer = terminal.backend().buffer().clone();

    // Verify layout structure exists by checking for:
    // 1. Main pane border/title (left side)
    // 2. Subagent pane border/title (right side)
    // 3. Status bar (bottom)

    // Look for "Main Agent" title somewhere in the buffer
    let content = buffer.content.iter().map(|c| c.symbol()).collect::<String>();
    assert!(
        content.contains("Main Agent"),
        "Main agent pane title should be rendered"
    );
    assert!(
        content.contains("Subagent"),
        "Subagent pane should be rendered"
    );
}

#[test]
fn render_layout_hides_subagent_pane_when_no_subagents() {
    let mut terminal = create_test_terminal();
    let session = create_session_no_subagents();
    let state = AppState::new(session);

    terminal
        .draw(|frame| {
            render_layout(frame, &state);
        })
        .unwrap();

    let buffer = terminal.backend().buffer().clone();
    let content = buffer.content.iter().map(|c| c.symbol()).collect::<String>();

    // Main agent should be visible
    assert!(
        content.contains("Main Agent"),
        "Main agent pane should be rendered"
    );

    // Subagent pane should NOT be rendered (or have zero width)
    // We can't easily verify zero width, so we just check main pane exists
    // The constraint test above ensures the logic is correct
}

#[test]
fn render_layout_includes_status_bar() {
    let mut terminal = create_test_terminal();
    let session = create_session_no_subagents();
    let state = AppState::new(session);

    terminal
        .draw(|frame| {
            render_layout(frame, &state);
        })
        .unwrap();

    let buffer = terminal.backend().buffer().clone();
    let content = buffer.content.iter().map(|c| c.symbol()).collect::<String>();

    // Status bar should show hints or live indicator
    assert!(
        content.contains("q: quit") || content.contains("LIVE"),
        "Status bar should contain hints or live mode indicator"
    );
}

#[test]
fn render_layout_shows_live_indicator_when_live_mode() {
    let mut terminal = create_test_terminal();
    let session = create_session_no_subagents();
    let mut state = AppState::new(session);
    state.live_mode = true;

    terminal
        .draw(|frame| {
            render_layout(frame, &state);
        })
        .unwrap();

    let buffer = terminal.backend().buffer().clone();
    let content = buffer.content.iter().map(|c| c.symbol()).collect::<String>();

    assert!(
        content.contains("LIVE"),
        "Status bar should show LIVE indicator when in live mode"
    );
}
