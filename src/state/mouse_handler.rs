//! Mouse event handler.
//!
//! Pure functions that transform AppState in response to mouse events.

use crate::model::AgentId;
use crate::state::AppState;

/// Result of detecting which tab was clicked.
///
/// The tab bar needs to expose its layout (tab positions) so we can
/// map click coordinates to tab indices.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TabClickResult {
    /// Click was on tab at index
    TabClicked(usize),
    /// Click was outside any tab
    NoTab,
}

/// Result of detecting which entry was clicked.
///
/// Maps click coordinates to conversation entry indices for expand/collapse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntryClickResult {
    /// Click was on a main pane entry at index
    MainPaneEntry(usize),
    /// Click was on a subagent pane entry at index
    SubagentPaneEntry(usize),
    /// Click was outside any entry
    NoEntry,
}

/// Detect which tab (if any) was clicked based on mouse position.
///
/// # Arguments
/// * `click_x` - Mouse click column position (0-based)
/// * `click_y` - Mouse click row position (0-based)
/// * `tab_area` - The rectangular area containing the tab bar
/// * `agent_ids` - Ordered list of agent IDs (determines tab count and labels)
///
/// # Returns
/// * `TabClickResult::TabClicked(index)` - Click was on tab at index
/// * `TabClickResult::NoTab` - Click was outside any tab
///
/// # Behavior
/// - Returns NoTab if click is outside tab_area bounds
/// - Calculates tab widths based on agent_id lengths and available space
/// - Returns the index of the clicked tab if within bounds
pub fn detect_tab_click(
    click_x: u16,
    click_y: u16,
    tab_area: ratatui::layout::Rect,
    agent_ids: &[&AgentId],
) -> TabClickResult {
    // Check if click is within tab area bounds
    if click_x < tab_area.x
        || click_x >= tab_area.x + tab_area.width
        || click_y < tab_area.y
        || click_y >= tab_area.y + tab_area.height
    {
        return TabClickResult::NoTab;
    }

    // No tabs = no click
    if agent_ids.is_empty() {
        return TabClickResult::NoTab;
    }

    // Guard: zero width tab area cannot have clickable tabs
    if tab_area.width == 0 {
        return TabClickResult::NoTab;
    }

    // Calculate which tab was clicked
    // Each tab gets equal width
    let tab_count = agent_ids.len() as u16;
    let tab_width = tab_area.width / tab_count;

    // Guard: if tabs are too narrow to render (width rounds to zero), no click
    if tab_width == 0 {
        return TabClickResult::NoTab;
    }

    // Relative position within tab area
    let relative_x = click_x - tab_area.x;

    // Which tab index (0-based)
    let tab_index = (relative_x / tab_width) as usize;

    // Bounds check
    if tab_index >= agent_ids.len() {
        TabClickResult::NoTab
    } else {
        TabClickResult::TabClicked(tab_index)
    }
}

/// Detect which entry (if any) was clicked based on mouse position.
///
/// # Arguments
/// * `click_x` - Mouse click column position (0-based)
/// * `click_y` - Mouse click row position (0-based)
/// * `conversation_area` - The rectangular area for the unified conversation pane (FR-083)
/// * `state` - Current application state (for entry layout calculation)
///
/// # Returns
/// * `EntryClickResult::MainPaneEntry(index)` - Click on main pane entry at index
/// * `EntryClickResult::SubagentPaneEntry(index)` - Click on subagent pane entry at index
/// * `EntryClickResult::NoEntry` - Click outside any entry
///
/// # Behavior
/// - Uses central routing (selected_conversation_view) to get the correct conversation
/// - Accounts for scroll offset and entry heights via hit_test
/// - Inner area has 1px border on each side
/// - Returns MainPaneEntry when main tab selected, SubagentPaneEntry when subagent tab selected
pub fn detect_entry_click(
    click_x: u16,
    click_y: u16,
    conversation_area: ratatui::layout::Rect,
    state: &AppState,
) -> EntryClickResult {
    // Check if click is within conversation area bounds
    if click_x < conversation_area.x
        || click_x >= conversation_area.x + conversation_area.width
        || click_y < conversation_area.y
        || click_y >= conversation_area.y + conversation_area.height
    {
        return EntryClickResult::NoEntry;
    }

    // Check if click is within inner area (accounting for border)
    let inner_x = conversation_area.x + 1;
    let inner_y = conversation_area.y + 1;
    let inner_width = conversation_area.width.saturating_sub(2);
    let inner_height = conversation_area.height.saturating_sub(2);

    if click_x < inner_x
        || click_x >= inner_x + inner_width
        || click_y < inner_y
        || click_y >= inner_y + inner_height
    {
        return EntryClickResult::NoEntry;
    }

    // Use central routing to get the selected conversation view
    let conv_view = match state.selected_conversation_view() {
        Some(view) => view,
        None => return EntryClickResult::NoEntry,
    };

    use crate::view_state::hit_test::HitTestResult;

    // Get scroll offset
    let scroll_offset =
        conv_view
            .scroll()
            .resolve(conv_view.total_height(), inner_height as usize, |idx| {
                conv_view.entry_cumulative_y(idx)
            });

    // Calculate viewport-relative position
    let viewport_y = click_y.saturating_sub(inner_y);
    let viewport_x = click_x.saturating_sub(inner_x);

    // Hit-test using ConversationViewState
    match conv_view.hit_test(viewport_y, viewport_x, scroll_offset) {
        HitTestResult::Hit { entry_index, .. } => {
            // Determine which result variant based on selected tab
            // Main tab (0) -> MainPaneEntry, Subagent tabs (1+) -> SubagentPaneEntry
            if state.selected_agent_id().is_some() {
                EntryClickResult::SubagentPaneEntry(entry_index.get())
            } else {
                EntryClickResult::MainPaneEntry(entry_index.get())
            }
        }
        HitTestResult::Miss => EntryClickResult::NoEntry,
    }
}

/// Handle an entry click event and toggle expand/collapse.
///
/// # Arguments
/// * `state` - Current application state
/// * `entry_click` - Result from detect_entry_click indicating which entry was clicked
/// * `_viewport_width` - Unused (kept for API compatibility)
///
/// # Returns
/// Updated AppState with entry expansion toggled if an entry was clicked.
///
/// # Behavior
/// - If entry was clicked, toggles expansion state via ConversationViewState
/// - Main pane entries toggle via main ConversationViewState
/// - Subagent pane entries toggle via selected subagent's ConversationViewState
/// - If click was outside entries, state is unchanged
/// - Uses HeightIndex-aware toggle_entry_expanded for O(log n) updates
pub fn handle_entry_click(
    mut state: AppState,
    entry_click: EntryClickResult,
    _viewport_width: u16,
) -> AppState {
    match entry_click {
        EntryClickResult::MainPaneEntry(index) => {
            // Toggle expand via ConversationViewState
            if let Some(session_view) = state.log_view_mut().current_session_mut() {
                let conv_view = session_view.main_mut();
                conv_view.toggle_entry_expanded(index);
            }
            state
        }
        EntryClickResult::SubagentPaneEntry(index) => {
            // Toggle expand via selected subagent's ConversationViewState
            // Use central routing to get agent ID and conversation view
            if let Some(agent_id) = state.selected_agent_id() {
                if let Some(session_view) = state.log_view_mut().current_session_mut() {
                    let conv_view = session_view.subagent_mut(&agent_id);
                    conv_view.toggle_entry_expanded(index);
                }
            }
            state
        }
        EntryClickResult::NoEntry => state,
    }
}

/// Handle a mouse click event and update AppState accordingly.
///
/// # Arguments
/// * `state` - Current application state
/// * `click_x` - Mouse click column position
/// * `click_y` - Mouse click row position
/// * `tab_area` - The rectangular area containing the tab bar
///
/// # Returns
/// Updated AppState with tab selection changed if a tab was clicked.
///
/// # Behavior
/// - If click is on a tab, switches to that tab (updates selected_tab)
/// - If click is outside tabs, state is unchanged
/// - Uses agent_ids from state.session() to determine tab layout
pub fn handle_mouse_click(
    mut state: AppState,
    click_x: u16,
    click_y: u16,
    tab_area: ratatui::layout::Rect,
) -> AppState {
    // Get agent IDs from the session view-state
    let mut agent_ids: Vec<_> = state.session_view().subagent_ids().collect();
    agent_ids.sort_by(|a, b| a.as_str().cmp(b.as_str()));

    // Detect which tab was clicked
    let click_result = detect_tab_click(click_x, click_y, tab_area, &agent_ids);

    // Update state if a tab was clicked (cclv-5ur.53: convert index to ConversationSelection)
    match click_result {
        TabClickResult::TabClicked(index) => {
            // Tab 0 = Main, Tab 1+ = Subagent by sorted position
            if index == 0 {
                state.selected_conversation = crate::state::ConversationSelection::Main;
            } else if let Some(agent_id) = agent_ids.get(index - 1) {
                state.selected_conversation = crate::state::ConversationSelection::Subagent((*agent_id).clone());
            }
            state
        }
        TabClickResult::NoTab => state,
    }
}

/// Handle a mouse scroll event and update AppState accordingly.
///
/// # Arguments
/// * `state` - Current application state
/// * `is_scroll_up` - true for scroll up, false for scroll down
/// * `viewport` - Viewport dimensions (width and height) for scroll calculations
///
/// # Returns
/// Updated AppState with scroll position changed based on focused pane.
///
/// # Behavior
/// - Determines which pane to scroll based on current focus
/// - Scrolls Main pane when focus is FocusPane::Main
/// - Scrolls Subagent pane when focus is FocusPane::Subagent
/// - No scroll when focus is FocusPane::Stats or FocusPane::Search
/// - Delegates to scroll_handler for actual scroll logic
pub fn handle_mouse_scroll(
    state: AppState,
    is_scroll_up: bool,
    viewport: crate::view_state::types::ViewportDimensions,
) -> AppState {
    use crate::model::KeyAction;

    // Delegate to scroll_handler with appropriate action
    let action = if is_scroll_up {
        KeyAction::ScrollUp
    } else {
        KeyAction::ScrollDown
    };

    crate::state::scroll_handler::handle_scroll_action(state, action, viewport)
}

// ===== Tests =====

#[cfg(test)]
#[path = "mouse_handler_tests.rs"]
mod tests;
