//! Split pane layout rendering.
//!
//! Pure layout logic - calculates layout constraints and renders
//! placeholder widgets for main agent, subagent tabs, and status bar.

use crate::model::{AgentId, PricingConfig};
use crate::state::{agent_ids_with_matches, AppState, FocusPane, SearchState, WrapMode};
use crate::view::{
    help::render_help_overlay, message, stats::StatsPanel, tabs, MessageStyles, SearchInput,
};
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
/// FR-083-088: Tab bar always visible at top of conversation area (3 lines).
/// Returns tab area (top 3 lines of conversation pane).
/// This calculation must match the layout logic in render_layout().
pub fn calculate_tab_area(frame_area: Rect, state: &AppState) -> Option<Rect> {
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

    // FR-083-088: Tab bar is at top of conversation area (no horizontal split)
    // Split conversation area vertically: tab bar (3 lines) + content
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)])
        .split(conversation_area);

    Some(chunks[0])
}

/// Calculate the conversation pane area for mouse click detection.
///
/// FR-083-088: Unified tab model - returns single full-width conversation area.
/// Returns (conversation_area, None) - second value always None (no horizontal split).
/// This calculation must match the layout logic in render_layout().
pub fn calculate_pane_areas(frame_area: Rect, state: &AppState) -> (Rect, Option<Rect>) {
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

    // FR-083: No horizontal split - single full-width conversation area
    (conversation_area, None)
}

/// Render the unified tab layout with tab bar, conversation area, and status bar.
///
/// FR-083-088: Unified tab model - no horizontal split.
/// Tab bar shows all conversations: Main Agent (tab 0) + Subagents (tabs 1..N).
/// selected_tab determines which conversation is displayed below tab bar.
pub fn render_layout(frame: &mut Frame, state: &AppState) {
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

    // Render unified conversation pane (tab bar + selected conversation)
    // FR-083-088: Single pane with tab bar at top, no horizontal split
    render_conversation_pane(frame, conversation_area, state, &styles);

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

    // Render help overlay on top of everything else if visible
    if state.help_visible {
        render_help_overlay(frame);
    }
}

/// Render unified conversation pane with tab bar and selected conversation.
///
/// FR-083-088: Unified tab model - single pane, no horizontal split.
/// Layout: Tab bar (top 3 lines) + selected conversation content (remainder).
///
/// Tab 0 = Main Agent, Tabs 1..N = Subagents (in spawn order).
/// Tab bar always visible, even when only main agent exists.
fn render_conversation_pane(
    frame: &mut Frame,
    area: Rect,
    state: &AppState,
    styles: &MessageStyles,
) {
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

    // Build tab list: Main Agent (tab 0) + Subagents (tabs 1..N)
    // Sort subagent IDs for deterministic tab ordering (HashMap iteration is non-deterministic)
    let mut subagent_ids: Vec<_> = state.session_view().subagent_ids().collect();
    subagent_ids.sort_by(|a, b| a.as_str().cmp(b.as_str()));

    // FR-086: Build ConversationTab list with Main Agent at position 0
    let mut conversation_tabs = vec![tabs::ConversationTab::Main];
    conversation_tabs.extend(
        subagent_ids
            .iter()
            .map(|id| tabs::ConversationTab::Subagent(id)),
    );

    // Extract agent IDs with matches from search state
    let tabs_with_matches: HashSet<AgentId> = match &state.search {
        SearchState::Active { matches, .. } => agent_ids_with_matches(matches),
        _ => HashSet::new(), // No search active, no matches
    };

    // Render tab bar with Main Agent at position 0 (cclv-5ur.53)
    tabs::render_tab_bar(
        frame,
        tab_area,
        &conversation_tabs,
        state.selected_tab_index(),
        &tabs_with_matches,
    );

    // Determine which conversation to display using central routing
    let selected_tab_index = state.selected_tab_index().unwrap_or(0);
    let is_main_tab = selected_tab_index == 0;

    if let Some(view_state) = state.selected_conversation_view() {
        let conversation_widget = message::ConversationView::new(
            view_state,
            styles,
            if is_main_tab {
                state.focus == FocusPane::Main
            } else {
                state.focus == FocusPane::Subagent
            },
        )
        .is_subagent_view(!is_main_tab)
        .global_wrap(state.global_wrap)
        .max_context_tokens(state.max_context_tokens)
        .pricing(state.pricing.clone());
        frame.render_widget(conversation_widget, content_area);
    }
    // If no conversation selected, render nothing (empty content area)
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

    // Process subagent entries (all eagerly initialized)
    for (_agent_id, conversation_view) in session_view.initialized_subagents() {
        for entry_view in conversation_view.iter() {
            if let Some(log_entry) = entry_view.entry().as_valid() {
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
/// - Agent identifier based on selected tab (Main Agent vs subagent ID)
fn render_header(frame: &mut Frame, area: Rect, state: &AppState) {
    // Determine which conversation to show based on selected_tab (cclv-5ur.53)
    // Tab 0 = Main Agent, Tabs 1+ = Subagents
    let selected_tab_index = state.selected_tab_index().unwrap_or(0);

    let (agent_label, conversation_view) = if selected_tab_index == 0 {
        // Tab 0: Main Agent
        ("Main Agent".to_string(), state.session_view().main())
    } else {
        // Tabs 1+: Subagents (index - 1 in subagent list)
        let subagent_index = selected_tab_index - 1;
        let mut agent_ids: Vec<_> = state.session_view().subagent_ids().collect();
        agent_ids.sort_by(|a, b| a.as_str().cmp(b.as_str()));

        if let Some(&agent_id) = agent_ids.get(subagent_index) {
            // Try to get initialized subagent, but show subagent label even if pending
            let conv = state
                .session_view()
                .get_subagent(agent_id)
                .unwrap_or_else(|| state.session_view().main());
            (format!("Subagent {}", agent_id.as_str()), conv)
        } else {
            // subagent_index out of bounds, fallback to main
            ("Main Agent".to_string(), state.session_view().main())
        }
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
