//! Split pane layout rendering.
//!
//! Pure layout logic - calculates layout constraints and renders
//! placeholder widgets for main agent, subagent tabs, and status bar.

use crate::model::{AgentId, PricingConfig};
use crate::state::{agent_ids_with_matches, AppState, FocusPane, SearchState, WrapMode};
use crate::view::{message, stats::StatsPanel, tabs, MessageStyles, SearchInput};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};
use std::collections::HashSet;

/// Calculate the tab area for mouse click detection.
///
/// Returns None if there are no subagents or if the layout doesn't show tabs.
/// This calculation must match the layout logic in render_layout().
pub fn calculate_tab_area(frame_area: Rect, state: &AppState) -> Option<Rect> {
    let has_subagents = state.session_view().has_subagents();
    if !has_subagents {
        return None;
    }

    // Determine if search input is visible (same logic as render_layout)
    let search_visible = matches!(
        state.search,
        SearchState::Typing { .. } | SearchState::Active { .. }
    );

    // Calculate vertical chunks (same as render_layout)
    let vertical_chunks = if search_visible {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // Header
                Constraint::Min(0),    // Content
                Constraint::Length(3), // Search
                Constraint::Length(1), // Status
            ])
            .split(frame_area)
    } else {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // Header
                Constraint::Min(0),    // Content
                Constraint::Length(1), // Status
            ])
            .split(frame_area)
    };

    let content_area = vertical_chunks[1];

    // Calculate conversation area (accounting for stats panel)
    let conversation_area = if state.stats_visible {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0), Constraint::Length(10)])
            .split(content_area);
        chunks[0]
    } else {
        content_area
    };

    // Calculate horizontal split (main + subagent)
    let horizontal_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(conversation_area);

    let subagent_area = horizontal_chunks[1];

    // Tab area is the top 3 lines of subagent pane
    let subagent_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)])
        .split(subagent_area);

    Some(subagent_chunks[0])
}

/// Calculate the main and subagent pane areas for mouse click detection.
///
/// Returns (main_area, subagent_area) where subagent_area is None if no subagents exist.
/// This calculation must match the layout logic in render_layout().
pub fn calculate_pane_areas(frame_area: Rect, state: &AppState) -> (Rect, Option<Rect>) {
    let has_subagents = state.session_view().has_subagents();

    // Determine if search input is visible
    let search_visible = matches!(
        state.search,
        SearchState::Typing { .. } | SearchState::Active { .. }
    );

    // Calculate vertical chunks
    let vertical_chunks = if search_visible {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // Header
                Constraint::Min(0),    // Content
                Constraint::Length(3), // Search
                Constraint::Length(1), // Status
            ])
            .split(frame_area)
    } else {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // Header
                Constraint::Min(0),    // Content
                Constraint::Length(1), // Status
            ])
            .split(frame_area)
    };

    let content_area = vertical_chunks[1];

    // Calculate conversation area (accounting for stats panel)
    let conversation_area = if state.stats_visible {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0), Constraint::Length(10)])
            .split(content_area);
        chunks[0]
    } else {
        content_area
    };

    // Calculate horizontal split (main + subagent)
    if has_subagents {
        let horizontal_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
            .split(conversation_area);

        (horizontal_chunks[0], Some(horizontal_chunks[1]))
    } else {
        (conversation_area, None)
    }
}

/// Render the split pane layout with main agent (left), subagent tabs (right),
/// and status bar (bottom).
///
/// When session has no subagents, right pane is hidden and left pane takes full width.
pub fn render_layout(frame: &mut Frame, state: &AppState) {
    let has_subagents = state.session_view().has_subagents();

    // Create message styles for consistent coloring across panes
    let styles = MessageStyles::new();

    // Determine if search input should be shown
    let search_visible = matches!(
        state.search,
        SearchState::Typing { .. } | SearchState::Active { .. }
    );

    // Split screen vertically: header + main content area + optional search + status bar
    let vertical_chunks = if search_visible {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // Header bar (1 line)
                Constraint::Min(0),    // Main content area
                Constraint::Length(3), // Search input (3 lines for border + text)
                Constraint::Length(1), // Status bar (1 line)
            ])
            .split(frame.area())
    } else {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // Header bar (1 line)
                Constraint::Min(0),    // Main content area
                Constraint::Length(1), // Status bar (1 line)
            ])
            .split(frame.area())
    };

    let header_area = vertical_chunks[0];
    let content_area = vertical_chunks[1];
    let (search_area, status_area) = if search_visible {
        (Some(vertical_chunks[2]), vertical_chunks[3])
    } else {
        (None, vertical_chunks[2])
    };

    render_header(frame, header_area, state);

    // Split content area vertically: conversation area + stats panel (if visible)
    let (conversation_area, stats_area) = if state.stats_visible {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(0),     // Conversation area (flexible)
                Constraint::Length(10), // Stats panel (fixed ~10 lines)
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

    // Render search input if visible
    if let Some(search_area_rect) = search_area {
        let search_widget = SearchInput::new(&state.search);
        frame.render_widget(search_widget, search_area_rect);
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
    // Get view-state for main conversation (fallback to empty if no session yet)
    let empty_view_state = crate::view_state::conversation::ConversationViewState::empty();
    let view_state = state
        .log_view()
        .current_session()
        .map(|s| s.main())
        .unwrap_or(&empty_view_state);

    message::render_conversation_view_with_search(
        frame,
        area,
        view_state,
        &state.main_scroll,
        &state.search,
        styles,
        state.focus == FocusPane::Main,
        state.global_wrap,
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
    let agent_ids: Vec<_> = state.session_view().subagent_ids().collect();

    // Extract agent IDs with matches from search state
    let tabs_with_matches: HashSet<AgentId> = match &state.search {
        SearchState::Active { matches, .. } => agent_ids_with_matches(matches),
        _ => HashSet::new(), // No search active, no matches
    };

    tabs::render_tab_bar(
        frame,
        tab_area,
        &agent_ids,
        state.selected_tab,
        &tabs_with_matches,
    );

    // Determine which conversation to display based on selected_tab
    let selected_conversation_view = if let Some(idx) = state.selected_tab {
        // Get the conversation at the selected index (read-only access)
        agent_ids
            .get(idx)
            .and_then(|&agent_id| state.session_view().get_subagent(agent_id))
    } else {
        // No selection - show first subagent as default (read-only access)
        agent_ids
            .first()
            .and_then(|&agent_id| state.session_view().get_subagent(agent_id))
    };

    // Render the selected conversation using its view-state
    if let Some(conversation_view) = selected_conversation_view {
        message::render_conversation_view_with_search(
            frame,
            content_area,
            conversation_view,
            &state.subagent_scroll,
            &state.search,
            styles,
            state.focus == FocusPane::Subagent,
            state.global_wrap,
        );
    }
}

/// Render the stats panel with session statistics.
///
/// The panel displays token usage, estimated cost, tool usage, and subagent count.
/// Border is highlighted when FocusPane::Stats is active.
fn render_stats_panel(frame: &mut Frame, area: Rect, state: &AppState) {
    // Build session statistics by iterating through entries
    // Uses SessionViewState which contains all entries including pending subagents
    // TODO: This should be cached in SessionViewState once stats are integrated
    let session_view = state
        .log_view()
        .get_session(0)
        .expect("Session 0 must exist");
    let stats = build_session_stats(session_view);

    // Get model ID for pricing calculation
    // TODO: Model ID should be in SessionViewState metadata
    let model_id = state.session_view().main().model_id();

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

/// Build SessionStats by iterating through all entries in SessionViewState.
/// This is temporary - stats should eventually be maintained in SessionViewState during ingestion.
fn build_session_stats(
    session_view: &crate::view_state::session::SessionViewState,
) -> crate::model::SessionStats {
    use crate::model::SessionStats;

    let mut stats = SessionStats::default();

    // Process main agent entries
    for entry_view in session_view.main().iter() {
        if let Some(log_entry) = entry_view.entry().as_valid() {
            stats.record_entry(log_entry);
        }
    }

    // Process initialized subagent entries
    for (_agent_id, conversation_view) in session_view.initialized_subagents() {
        for entry_view in conversation_view.iter() {
            if let Some(log_entry) = entry_view.entry().as_valid() {
                stats.record_entry(log_entry);
            }
        }
    }

    // Process pending subagent entries (not yet lazily initialized)
    for (_agent_id, entries) in session_view.pending_subagents() {
        for entry in entries {
            if let Some(log_entry) = entry.as_valid() {
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
    // Common shortcuts always displayed
    let common = "q: Quit | ?: Help";

    // Context-specific shortcuts based on focus pane
    let context_hints = match focus {
        FocusPane::Main => "/: Search | s: Stats | Tab: Cycle panes",
        FocusPane::Subagent => "[ ]: Tabs | 1-9: Select tab | Tab: Cycle panes",
        FocusPane::Stats => "!: Global | @: Main | #: Current | Tab: Cycle panes",
        FocusPane::Search if search_active => "n: Next | N: Prev | Esc: Exit",
        FocusPane::Search => "Enter: Submit | Esc: Cancel",
    };

    // Combine common and context hints
    let full_hints = format!("{} | {}", common, context_hints);

    // Truncate if terminal is too narrow
    if terminal_width < 60 {
        // Very narrow - show only critical shortcuts
        format!(
            "q: Quit | ?: Help | {}",
            match focus {
                FocusPane::Main => "/: Search",
                FocusPane::Subagent => "[ ]: Tabs",
                FocusPane::Stats => "!/@/#: Filter",
                FocusPane::Search if search_active => "n: Next",
                FocusPane::Search => "Enter",
            }
        )
    } else if (full_hints.len() as u16) > terminal_width {
        // Moderate width - abbreviate but keep most info
        let abbreviated = match focus {
            FocusPane::Main => "q: Quit | /: Search | s: Stats | ?: Help",
            FocusPane::Subagent => "q: Quit | [ ]: Tabs | 1-9: Select | ?: Help",
            FocusPane::Stats => "q: Quit | !/@/#: Filters | ?: Help",
            FocusPane::Search if search_active => "n/N: Navigate | Esc: Exit",
            FocusPane::Search => "Enter: Submit | Esc: Cancel",
        };
        abbreviated.to_string()
    } else {
        // Wide enough - show full hints
        full_hints
    }
}

/// Render the status bar with hints and live mode indicator.
fn render_status_bar(frame: &mut Frame, area: Rect, state: &AppState) {
    let mut spans = Vec::new();

    // LIVE indicator using LiveIndicator widget (FR-042b)
    let live_indicator = crate::view::LiveIndicator::new(state.input_mode, state.blink_on);
    spans.push(live_indicator.render());

    // Wrap indicator
    let wrap_text = match state.global_wrap {
        WrapMode::Wrap => "Wrap: On | ",
        WrapMode::NoWrap => "Wrap: Off | ",
    };
    spans.push(Span::styled(wrap_text, Style::default().fg(Color::Gray)));

    // Calculate available width for hints
    let used_width: u16 = spans.iter().map(|s| s.content.len() as u16).sum();
    let available_width = area.width.saturating_sub(used_width);

    // Keyboard hints
    let search_active = matches!(state.search, SearchState::Active { .. });
    let hints = build_keyboard_hints(state.focus, search_active, available_width);
    spans.push(Span::styled(hints, Style::default().fg(Color::Gray)));

    let paragraph = Paragraph::new(Line::from(spans));
    frame.render_widget(paragraph, area);
}

/// Render the header bar showing model name, agent ID, and session metadata.
///
/// Displays:
/// - Model name (from ModelInfo.display_name()) for current conversation
/// - Session metadata (cwd, tools count, agents count, skills count) from system:init
/// - [LIVE] indicator when live_mode && auto_scroll are both true
/// - Agent identifier based on focused pane (Main Agent vs subagent ID)
fn render_header(frame: &mut Frame, area: Rect, state: &AppState) {
    // Determine which conversation to show (main or selected subagent)
    let (agent_label, conversation_view) = match state.focus {
        FocusPane::Subagent => {
            // Get selected subagent conversation (read-only access)
            let agent_ids: Vec<_> = state.session_view().subagent_ids().collect();
            let selected_idx = state.selected_tab.unwrap_or(0);

            if let Some(&agent_id) = agent_ids.get(selected_idx) {
                // Try to get initialized subagent, but show subagent label even if pending
                let conv = state
                    .session_view()
                    .get_subagent(agent_id)
                    .unwrap_or_else(|| state.session_view().main());
                (format!("Subagent {}", agent_id.as_str()), conv)
            } else {
                ("Main Agent".to_string(), state.session_view().main())
            }
        }
        _ => ("Main Agent".to_string(), state.session_view().main()),
    };

    // Get model name from conversation view-state
    let model_name = conversation_view.model_name().unwrap_or("Unknown");

    // Show [LIVE] indicator only when both live_mode and auto_scroll are true
    let live_indicator = if state.live_mode && state.auto_scroll {
        " [LIVE]"
    } else {
        ""
    };

    // Get session metadata from SessionViewState
    let metadata_text = if let Some(sys_meta) = state.session_view().system_metadata() {
        let cwd_display = sys_meta
            .cwd
            .as_ref()
            .and_then(|p| p.to_str())
            .unwrap_or("?");
        let tools_count = sys_meta.tools.len();
        let agents_count = sys_meta.agents.len();
        let skills_count = sys_meta.skills.len();

        format!(
            " | {} | {} tools, {} agents, {} skills",
            cwd_display, tools_count, agents_count, skills_count
        )
    } else {
        String::new()
    };

    // Format: "Model: Sonnet | Main Agent [LIVE] | /path | 45 tools, 3 agents, 20 skills"
    let header_text = format!(
        "Model: {} | {}{}{}",
        model_name, agent_label, live_indicator, metadata_text
    );

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
