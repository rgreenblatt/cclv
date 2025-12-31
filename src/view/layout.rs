//! Split pane layout rendering.
//!
//! Pure layout logic - calculates layout constraints and renders
//! placeholder widgets for main agent, subagent tabs, and status bar.

use crate::state::{AppState, FocusPane};
use crate::view::{message, tabs, MessageStyles};
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

    // Create message styles for consistent coloring across panes
    let styles = MessageStyles::new();

    // Split screen vertically: header + main content area + status bar
    let vertical_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),  // Header bar (1 line)
            Constraint::Min(0),     // Main content area
            Constraint::Length(1),  // Status bar (1 line)
        ])
        .split(frame.area());

    let header_area = vertical_chunks[0];
    let content_area = vertical_chunks[1];
    let status_area = vertical_chunks[2];

    render_header(frame, header_area, state);

    // Split content area horizontally based on subagent presence
    let (main_constraint, subagent_constraint) = calculate_horizontal_constraints(has_subagents);
    let horizontal_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([main_constraint, subagent_constraint])
        .split(content_area);

    // Render panes
    render_main_pane(frame, horizontal_chunks[0], state, &styles);

    if has_subagents {
        render_subagent_pane(frame, horizontal_chunks[1], state, &styles);
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
fn render_main_pane(frame: &mut Frame, area: Rect, state: &AppState, styles: &MessageStyles) {
    message::render_conversation_view(
        frame,
        area,
        state.session().main_agent(),
        &state.main_scroll,
        styles,
        state.focus == FocusPane::Main,
    );
}

/// Render the subagent tabs pane with tab bar and selected conversation.
///
/// Layout: Tab bar (top 3 lines) + conversation content (remainder).
/// Uses state.selected_tab to determine which subagent conversation to display.
fn render_subagent_pane(frame: &mut Frame, area: Rect, state: &AppState, styles: &MessageStyles) {
    // Split area vertically: tab bar + conversation content
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Tab bar (border + title + content)
            Constraint::Min(0),     // Conversation content
        ])
        .split(area);

    let tab_area = chunks[0];
    let content_area = chunks[1];

    // Get ordered subagent IDs and render tab bar
    let agent_ids = state.session().subagent_ids_ordered();
    tabs::render_tab_bar(frame, tab_area, &agent_ids, state.selected_tab);

    // Determine which conversation to display based on selected_tab
    let selected_conversation = if let Some(idx) = state.selected_tab {
        // Get the conversation at the selected index
        agent_ids
            .get(idx)
            .and_then(|agent_id| state.session().subagents().get(agent_id))
    } else {
        // No selection - show first subagent as default
        state.session().subagents().values().next()
    };

    // Render the selected conversation
    if let Some(conversation) = selected_conversation {
        message::render_conversation_view(
            frame,
            content_area,
            conversation,
            &state.subagent_scroll,
            styles,
            state.focus == FocusPane::Subagent,
        );
    }
}

/// Render the status bar with hints and live mode indicator.
fn render_status_bar(frame: &mut Frame, area: Rect, state: &AppState) {
    let live_indicator = if state.live_mode && state.auto_scroll {
        " [LIVE] "
    } else {
        ""
    };

    let status_text = format!("{}q: quit | Tab: switch pane", live_indicator);

    let style = if state.live_mode && state.auto_scroll {
        Style::default().fg(Color::Green)
    } else {
        Style::default().fg(Color::Gray)
    };

    let paragraph = Paragraph::new(Line::from(status_text)).style(style);
    frame.render_widget(paragraph, area);
}

/// Render the header bar showing model name, agent ID, and live indicator.
///
/// Displays:
/// - Model name (from ModelInfo.display_name()) for current conversation
/// - [LIVE] indicator when live_mode && auto_scroll are both true
/// - Agent identifier based on focused pane (Main Agent vs subagent ID)
fn render_header(frame: &mut Frame, area: Rect, state: &AppState) {
    // Determine which conversation to show (main or selected subagent)
    let (agent_label, conversation) = match state.focus {
        FocusPane::Subagent => {
            // Get selected subagent conversation
            let agent_ids = state.session().subagent_ids_ordered();
            let selected_idx = state.selected_tab.unwrap_or(0);

            if let Some(agent_id) = agent_ids.get(selected_idx) {
                if let Some(conv) = state.session().subagents().get(agent_id) {
                    (format!("Subagent {}", agent_id.as_str()), conv)
                } else {
                    ("Main Agent".to_string(), state.session().main_agent())
                }
            } else {
                ("Main Agent".to_string(), state.session().main_agent())
            }
        }
        _ => ("Main Agent".to_string(), state.session().main_agent()),
    };

    // Get model name from conversation
    let model_name = conversation
        .model()
        .map(|m| m.display_name())
        .unwrap_or("Unknown");

    // Show [LIVE] indicator only when both live_mode and auto_scroll are true
    let live_indicator = if state.live_mode && state.auto_scroll {
        " [LIVE]"
    } else {
        ""
    };

    // Format: "Model: Sonnet | Main Agent [LIVE]"
    let header_text = format!("Model: {} | {}{}", model_name, agent_label, live_indicator);

    let style = if state.live_mode && state.auto_scroll {
        Style::default().fg(Color::Green)
    } else {
        Style::default().fg(Color::Cyan)
    };

    let paragraph = Paragraph::new(Line::from(header_text)).style(style);
    frame.render_widget(paragraph, area);
}

// ===== Tests =====

#[cfg(test)]
#[path = "layout_tests.rs"]
mod tests;
