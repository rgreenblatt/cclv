//! Unified conversation layout rendering.
//!
//! Pure layout logic for the unified tab model (FR-083-088). Single conversation pane
//! with tab bar for switching between main agent and subagent conversations.

use crate::model::{AgentId, PricingConfig};
use crate::state::{AppState, FocusPane, SearchState, WrapMode, agent_ids_with_matches};
use crate::view::{
    MessageStyles, SearchInput,
    constants::{SEARCH_INPUT_HEIGHT, STATS_PANEL_HEIGHT, STATUS_BAR_HEIGHT, TAB_BAR_HEIGHT},
    help::render_help_overlay,
    message,
    stats::StatsPanel,
    tabs,
};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    text::{Line, Span},
    widgets::Paragraph,
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
    // Note: Header line removed per cclv-5ur.61
    let vertical_chunks = if search_visible {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(0),                      // Content
                Constraint::Length(SEARCH_INPUT_HEIGHT), // Search
                Constraint::Length(STATUS_BAR_HEIGHT),   // Status
            ])
            .split(frame_area)
    } else {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(0),                    // Content
                Constraint::Length(STATUS_BAR_HEIGHT), // Status
            ])
            .split(frame_area)
    };

    let content_area = vertical_chunks[0];

    // Calculate conversation area (accounting for stats panel)
    let conversation_area = if state.stats_visible {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0), Constraint::Length(STATS_PANEL_HEIGHT)])
            .split(content_area);
        chunks[0]
    } else {
        content_area
    };

    // FR-083-088: Tab bar is at top of conversation area (no horizontal split)
    // Split conversation area vertically: tab bar + content
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(TAB_BAR_HEIGHT), Constraint::Min(0)])
        .split(conversation_area);

    Some(chunks[0])
}

/// Calculate the conversation pane area for mouse click detection.
///
/// FR-083-088: Unified tab model - returns single full-width conversation area.
/// This calculation must match the layout logic in render_layout().
pub fn calculate_pane_area(frame_area: Rect, state: &AppState) -> Rect {
    // Determine if search input is visible
    let search_visible = matches!(
        state.search,
        SearchState::Typing { .. } | SearchState::Active { .. }
    );

    // Calculate vertical chunks
    // Note: Header line removed per cclv-5ur.61
    let vertical_chunks = if search_visible {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(0),                      // Content
                Constraint::Length(SEARCH_INPUT_HEIGHT), // Search
                Constraint::Length(STATUS_BAR_HEIGHT),   // Status
            ])
            .split(frame_area)
    } else {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(0),                    // Content
                Constraint::Length(STATUS_BAR_HEIGHT), // Status
            ])
            .split(frame_area)
    };

    let content_area = vertical_chunks[0];

    // Calculate conversation area (accounting for stats panel)
    if state.stats_visible {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0), Constraint::Length(STATS_PANEL_HEIGHT)])
            .split(content_area);
        chunks[0]
    } else {
        content_area
    }
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

    // Split screen vertically: main content area + optional search + status bar
    // Note: Header line removed per cclv-5ur.61
    let vertical_chunks = if search_visible {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(0),                      // Main content area
                Constraint::Length(SEARCH_INPUT_HEIGHT), // Search input
                Constraint::Length(STATUS_BAR_HEIGHT),   // Status bar
            ])
            .split(frame.area())
    } else {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(0),                    // Main content area
                Constraint::Length(STATUS_BAR_HEIGHT), // Status bar
            ])
            .split(frame.area())
    };

    let content_area = vertical_chunks[0];
    let (search_area, status_area) = if search_visible {
        (Some(vertical_chunks[1]), vertical_chunks[2])
    } else {
        (None, vertical_chunks[1])
    };

    // Split content area vertically: conversation area + stats panel (if visible)
    let (conversation_area, stats_area) = if state.stats_visible {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(0),                     // Conversation area (flexible)
                Constraint::Length(STATS_PANEL_HEIGHT), // Stats panel
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
        render_help_overlay(frame, state.help_scroll_offset);
    }

    // Render session modal overlay on top of everything else if visible
    // This should be rendered last to appear on top of all other UI elements
    if state.session_modal.is_visible() {
        crate::view::render_session_modal(frame, state);
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
            Constraint::Length(TAB_BAR_HEIGHT), // Tab bar
            Constraint::Min(0),                 // Conversation content
        ])
        .split(area);

    let tab_area = chunks[0];
    let content_area = chunks[1];

    // Build tab list: Main Agent (tab 0) + Subagents (tabs 1..N)
    // FR-011: Get subagents from viewed session, not current session
    // Sort subagent IDs for deterministic tab ordering (HashMap iteration is non-deterministic)
    let session_count = state.log_view().session_count();
    let viewed_session_idx = state.viewed_session.effective_index(session_count);
    let mut subagent_ids: Vec<_> = if let Some(idx) = viewed_session_idx {
        state
            .log_view()
            .get_session(idx.get())
            .map(|s| s.subagent_ids().collect())
            .unwrap_or_default()
    } else {
        Vec::new()
    };
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
///
/// # Bug Fix (cclv-cym)
///
/// Stats panel must use the viewed session (determined by `viewed_session`),
/// not always the last session. This ensures stats match the currently displayed
/// conversation when user switches sessions via the session modal.
fn render_stats_panel(frame: &mut Frame, area: Rect, state: &AppState) {
    // Get the viewed session (not always the last session)
    let session_count = state.log_view().session_count();
    let session_idx = match state.viewed_session.effective_index(session_count) {
        Some(idx) => idx,
        None => {
            // No valid session to display stats for (shouldn't happen normally)
            return;
        }
    };

    let session_view = match state.log_view().get_session(session_idx.get()) {
        Some(session) => session,
        None => {
            // Session doesn't exist (shouldn't happen if effective_index succeeded)
            return;
        }
    };

    // Build session statistics by iterating through entries
    // Uses SessionViewState which contains all entries including pending subagents
    // TODO: This should be cached in SessionViewState once stats are integrated
    let stats = build_session_stats(session_view);

    // Get model ID for pricing calculation
    // TODO: Model ID should be in SessionViewState metadata
    let model_id = session_view.main().model_id();

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
    // cclv-463.4.3: Use is_tailing_enabled to gate LIVE indicator visibility
    let live_indicator = crate::view::LiveIndicator::new(
        state.input_mode,
        state.blink_on,
        state.is_tailing_enabled(),
    );
    spans.push(live_indicator.render());

    // Session indicator (FR-012) - only show if multiple sessions exist
    let session_count = state.log_view().session_count();
    if session_count > 1 {
        if let Some(session_idx) = state.viewed_session.effective_index(session_count) {
            let session_text = format!("│ Session {}/{} │ ", session_idx.display(), session_count);
            spans.push(Span::styled(session_text, super::styles::MUTED_TEXT));
        }
    }

    // Wrap indicator
    let wrap_text = match state.global_wrap {
        WrapMode::Wrap => "Wrap: On | ",
        WrapMode::NoWrap => "Wrap: Off | ",
    };
    spans.push(Span::styled(wrap_text, super::styles::MUTED_TEXT));

    // Calculate available width for hints
    let used_width: u16 = spans.iter().map(|s| s.content.len() as u16).sum();
    let available_width = area.width.saturating_sub(used_width);

    // Keyboard hints
    let search_active = matches!(state.search, SearchState::Active { .. });
    let hints = build_keyboard_hints(state.focus, search_active, available_width);
    spans.push(Span::styled(hints, super::styles::MUTED_TEXT));

    let paragraph = Paragraph::new(Line::from(spans));
    frame.render_widget(paragraph, area);
}

// ===== Tests =====

#[cfg(test)]
#[path = "layout_tests.rs"]
mod tests;
