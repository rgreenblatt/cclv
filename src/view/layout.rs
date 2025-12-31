//! Split pane layout rendering.
//!
//! Pure layout logic - calculates layout constraints and renders
//! placeholder widgets for main agent, subagent tabs, and status bar.

use crate::state::{AppState, FocusPane};
use crate::view::message;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::Line,
    widgets::Paragraph,
    Frame,
};

/// Render the split pane layout with main agent (left), subagent tabs (right),
/// and status bar (bottom).
///
/// When session has no subagents, right pane is hidden and left pane takes full width.
pub fn render_layout(frame: &mut Frame, state: &AppState) {
    let has_subagents = !state.session().subagents().is_empty();

    // Split screen vertically: main content area + status bar
    let vertical_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(0),     // Main content area
            Constraint::Length(1),  // Status bar (1 line)
        ])
        .split(frame.area());

    let content_area = vertical_chunks[0];
    let status_area = vertical_chunks[1];

    // Split content area horizontally based on subagent presence
    let (main_constraint, subagent_constraint) = calculate_horizontal_constraints(has_subagents);
    let horizontal_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([main_constraint, subagent_constraint])
        .split(content_area);

    // Render panes
    render_main_pane(frame, horizontal_chunks[0], state);

    if has_subagents {
        render_subagent_pane(frame, horizontal_chunks[1], state);
    }

    render_status_bar(frame, status_area, state);
}

/// Calculate the horizontal split constraints based on subagent presence.
///
/// Returns (main_pane_width, subagent_pane_width):
/// - With subagents: (60%, 40%)
/// - Without subagents: (100%, 0%)
fn calculate_horizontal_constraints(has_subagents: bool) -> (Constraint, Constraint) {
    if has_subagents {
        (Constraint::Percentage(60), Constraint::Percentage(40))
    } else {
        (Constraint::Percentage(100), Constraint::Min(0))
    }
}

/// Render the main agent pane using shared ConversationView widget.
fn render_main_pane(frame: &mut Frame, area: Rect, state: &AppState) {
    message::render_conversation_view(
        frame,
        area,
        state.session().main_agent(),
        &state.main_scroll,
        state.focus == FocusPane::Main,
    );
}

/// Render the subagent tabs pane using shared ConversationView widget.
///
/// For now, shows the first subagent as a placeholder. Full tab selection
/// will be implemented in a later bead.
fn render_subagent_pane(frame: &mut Frame, area: Rect, state: &AppState) {
    // Get first subagent as placeholder (tab selection not yet implemented)
    if let Some((_agent_id, conversation)) = state.session().subagents().iter().next() {
        message::render_conversation_view(
            frame,
            area,
            conversation,
            &state.subagent_scroll,
            state.focus == FocusPane::Subagent,
        );
    }
}

/// Render the status bar with hints and live mode indicator.
fn render_status_bar(frame: &mut Frame, area: Rect, state: &AppState) {
    let live_indicator = if state.live_mode {
        " [LIVE] "
    } else {
        ""
    };

    let status_text = format!("{}q: quit | Tab: switch pane", live_indicator);

    let style = if state.live_mode {
        Style::default().fg(Color::Green)
    } else {
        Style::default().fg(Color::Gray)
    };

    let paragraph = Paragraph::new(Line::from(status_text)).style(style);
    frame.render_widget(paragraph, area);
}

// ===== Tests =====

#[cfg(test)]
#[path = "layout_tests.rs"]
mod tests;
