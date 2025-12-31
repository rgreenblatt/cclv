//! Split pane layout rendering.
//!
//! Pure layout logic - calculates layout constraints and renders
//! placeholder widgets for main agent, subagent tabs, and status bar.

use crate::model::PricingConfig;
use crate::state::{AppState, FocusPane};
use crate::view::{message, stats::StatsPanel, tabs, MessageStyles};
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
            Constraint::Length(1), // Header bar (1 line)
            Constraint::Min(0),    // Main content area
            Constraint::Length(1), // Status bar (1 line)
        ])
        .split(frame.area());

    let header_area = vertical_chunks[0];
    let content_area = vertical_chunks[1];
    let status_area = vertical_chunks[2];

    render_header(frame, header_area, state);

    // Split content area vertically: conversation area + stats panel (if visible)
    let (conversation_area, stats_area) = if state.stats_visible {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(0),      // Conversation area (flexible)
                Constraint::Length(10),  // Stats panel (fixed ~10 lines)
            ])
            .split(content_area);
        (chunks[0], Some(chunks[1]))
    } else {
        (content_area, None)
    };

    // Split conversation area horizontally based on subagent presence
    let (main_constraint, subagent_constraint) = calculate_horizontal_constraints(has_subagents);
    let horizontal_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([main_constraint, subagent_constraint])
        .split(conversation_area);

    // Render panes
    render_main_pane(frame, horizontal_chunks[0], state, &styles);

    if has_subagents {
        render_subagent_pane(frame, horizontal_chunks[1], state, &styles);
    }

    // Render stats panel if visible
    if let Some(stats_area_rect) = stats_area {
        render_stats_panel(frame, stats_area_rect, state);
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
            Constraint::Min(0),    // Conversation content
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

/// Render the stats panel with session statistics.
///
/// The panel displays token usage, estimated cost, tool usage, and subagent count.
/// Border is highlighted when FocusPane::Stats is active.
fn render_stats_panel(frame: &mut Frame, area: Rect, state: &AppState) {
    // Build session statistics by iterating through entries
    // TODO: This should be cached in Session once stats are integrated
    let stats = build_session_stats(state.session());

    // Get model ID for pricing calculation
    let model_id = state
        .session()
        .main_agent()
        .model()
        .map(|m| m.id());

    // Use default pricing configuration
    let pricing = PricingConfig::default();

    // Create stats panel widget - it handles focus styling internally
    let panel = StatsPanel::new(
        &stats,
        &state.stats_filter,
        &pricing,
        model_id,
        state.focus == FocusPane::Stats,
    );

    frame.render_widget(panel, area);
}

/// Build SessionStats by iterating through all session entries.
/// This is temporary until stats tracking is integrated into Session.
fn build_session_stats(session: &crate::model::Session) -> crate::model::SessionStats {
    use crate::model::{ConversationEntry, SessionStats};

    let mut stats = SessionStats::default();

    // Process main agent entries
    for entry in session.main_agent().entries() {
        if let ConversationEntry::Valid(log_entry) = entry {
            stats.record_entry(log_entry);
        }
    }

    // Process subagent entries
    for conversation in session.subagents().values() {
        for entry in conversation.entries() {
            if let ConversationEntry::Valid(log_entry) = entry {
                stats.record_entry(log_entry);
            }
        }
    }

    stats
}

/// Build context-sensitive keyboard hints based on current focus pane.
///
/// Returns a formatted string with keyboard shortcuts appropriate for the
/// current pane. Truncates or adapts hints based on terminal width.
///
/// # Arguments
/// * `focus` - Current focused pane
/// * `search_active` - Whether search mode is currently active
/// * `terminal_width` - Available width for rendering hints
fn build_keyboard_hints(focus: FocusPane, search_active: bool, terminal_width: u16) -> String {
    todo!("build_keyboard_hints")
}

/// Render the status bar with hints and live mode indicator.
fn render_status_bar(frame: &mut Frame, area: Rect, state: &AppState) {
    let live_indicator = if state.live_mode && state.auto_scroll {
        " [LIVE] "
    } else {
        ""
    };

    let status_text = format!(
        "{}q: quit | Tab: cycle panes | 1/2/3: focus Main/Subagent/Stats",
        live_indicator
    );

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
