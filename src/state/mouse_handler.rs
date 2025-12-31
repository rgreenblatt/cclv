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
/// * `main_pane_area` - The rectangular area for the main conversation pane
/// * `subagent_pane_area` - Optional rectangular area for the subagent pane
/// * `state` - Current application state (for entry layout calculation)
///
/// # Returns
/// * `EntryClickResult::MainPaneEntry(index)` - Click on main pane entry at index
/// * `EntryClickResult::SubagentPaneEntry(index)` - Click on subagent pane entry at index
/// * `EntryClickResult::NoEntry` - Click outside any entry
///
/// # Behavior
/// - Determines which pane was clicked
/// - Calculates entry layouts to map Y position to entry index
/// - Accounts for scroll offset and entry heights
/// - Inner area has 1px border on each side
pub fn detect_entry_click(
    click_x: u16,
    click_y: u16,
    main_pane_area: ratatui::layout::Rect,
    subagent_pane_area: Option<ratatui::layout::Rect>,
    state: &AppState,
) -> EntryClickResult {
    // Check if click is in subagent pane
    if let Some(subagent_area) = subagent_pane_area {
        if click_x >= subagent_area.x
            && click_x < subagent_area.x + subagent_area.width
            && click_y >= subagent_area.y
            && click_y < subagent_area.y + subagent_area.height
        {
            // Click is in subagent pane - check if it's within inner area (accounting for border)
            let inner_x = subagent_area.x + 1;
            let inner_y = subagent_area.y + 1;
            let inner_width = subagent_area.width.saturating_sub(2);
            let inner_height = subagent_area.height.saturating_sub(2);

            if click_x >= inner_x
                && click_x < inner_x + inner_width
                && click_y >= inner_y
                && click_y < inner_y + inner_height
            {
                // Check if there are any entries
                if let Some(tab_index) = state.selected_tab {
                    let agent_ids = state.session().subagent_ids_ordered();
                    if let Some(agent_id) = agent_ids.get(tab_index) {
                        if let Some(conversation) = state.session().subagents().get(agent_id) {
                            let entries = conversation.entries();
                            if !entries.is_empty() {
                                // Calculate which entry was clicked based on Y position
                                let relative_y = click_y.saturating_sub(inner_y);

                                // Simple fixed-height approach: each entry gets ~4 lines
                                const ENTRY_HEIGHT: u16 = 4;
                                let entry_index = (relative_y / ENTRY_HEIGHT) as usize;

                                // Clamp to valid range
                                let clamped_index = entry_index.min(entries.len() - 1);
                                return EntryClickResult::SubagentPaneEntry(clamped_index);
                            }
                        }
                    }
                }
            }
            return EntryClickResult::NoEntry;
        }
    }

    // Check if click is in main pane
    if click_x >= main_pane_area.x
        && click_x < main_pane_area.x + main_pane_area.width
        && click_y >= main_pane_area.y
        && click_y < main_pane_area.y + main_pane_area.height
    {
        // Click is in main pane - check if it's within inner area (accounting for border)
        let inner_x = main_pane_area.x + 1;
        let inner_y = main_pane_area.y + 1;
        let inner_width = main_pane_area.width.saturating_sub(2);
        let inner_height = main_pane_area.height.saturating_sub(2);

        if click_x >= inner_x
            && click_x < inner_x + inner_width
            && click_y >= inner_y
            && click_y < inner_y + inner_height
        {
            // Check if there are any entries
            let entries = state.session().main_agent().entries();
            if !entries.is_empty() {
                // Calculate which entry was clicked based on Y position
                let relative_y = click_y.saturating_sub(inner_y);

                // Simple fixed-height approach: each entry gets ~4 lines
                const ENTRY_HEIGHT: u16 = 4;
                let entry_index = (relative_y / ENTRY_HEIGHT) as usize;

                // Clamp to valid range
                let clamped_index = entry_index.min(entries.len() - 1);
                return EntryClickResult::MainPaneEntry(clamped_index);
            }
        }
        return EntryClickResult::NoEntry;
    }

    // Click is outside both panes
    EntryClickResult::NoEntry
}

/// Handle an entry click event and toggle expand/collapse.
///
/// # Arguments
/// * `state` - Current application state
/// * `entry_click` - Result from detect_entry_click indicating which entry was clicked
///
/// # Returns
/// Updated AppState with entry expansion toggled if an entry was clicked.
///
/// # Behavior
/// - If entry was clicked, looks up its EntryUuid and toggles expansion state
/// - Main pane entries toggle in main_scroll.expanded_messages
/// - Subagent pane entries toggle in subagent_scroll.expanded_messages
/// - If click was outside entries, state is unchanged
pub fn handle_entry_click(mut state: AppState, entry_click: EntryClickResult) -> AppState {
    match entry_click {
        EntryClickResult::MainPaneEntry(index) => {
            // Get the UUID of the clicked entry (clone to avoid borrow conflict)
            let uuid_opt = state
                .session()
                .main_agent()
                .entries()
                .get(index)
                .and_then(|entry| entry.uuid())
                .cloned();

            if let Some(uuid) = uuid_opt {
                state.main_scroll.toggle_expand(&uuid);
            }
            state
        }
        EntryClickResult::SubagentPaneEntry(index) => {
            // Get the current selected tab's conversation and clone UUID
            let uuid_opt = if let Some(tab_index) = state.selected_tab {
                let agent_ids = state.session().subagent_ids_ordered();
                agent_ids
                    .get(tab_index)
                    .and_then(|agent_id| state.session().subagents().get(agent_id))
                    .and_then(|conversation| conversation.entries().get(index))
                    .and_then(|entry| entry.uuid())
                    .cloned()
            } else {
                None
            };

            if let Some(uuid) = uuid_opt {
                state.subagent_scroll.toggle_expand(&uuid);
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
    // Get agent IDs from the session
    let agent_ids = state.session().subagent_ids_ordered();

    // Detect which tab was clicked
    let click_result = detect_tab_click(click_x, click_y, tab_area, &agent_ids);

    // Update state if a tab was clicked
    match click_result {
        TabClickResult::TabClicked(index) => {
            state.selected_tab = Some(index);
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
/// * `viewport_height` - Height of the visible viewport (for scroll calculations)
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
    mut state: AppState,
    is_scroll_up: bool,
    _viewport_height: usize,
) -> AppState {
    use crate::state::FocusPane;

    // Early return for non-scrollable panes
    match state.focus {
        FocusPane::Stats | FocusPane::Search => return state,
        _ => {}
    }

    // Calculate max scroll offset based on which pane is focused
    let max_entries = match state.focus {
        FocusPane::Main => state.session().main_agent().len().saturating_sub(1),
        FocusPane::Subagent => {
            // Get the currently selected subagent's entry count
            if let Some(tab_index) = state.selected_tab {
                let subagent_ids = state.session().subagent_ids_ordered();
                if let Some(&agent_id) = subagent_ids.get(tab_index) {
                    if let Some(conv) = state.session().subagents().get(agent_id) {
                        conv.len().saturating_sub(1)
                    } else {
                        0
                    }
                } else {
                    0
                }
            } else {
                0
            }
        }
        _ => 0,
    };

    // Get mutable reference to the appropriate scroll state
    let scroll_state = match state.focus {
        FocusPane::Main => &mut state.main_scroll,
        FocusPane::Subagent => &mut state.subagent_scroll,
        _ => return state, // Already handled above
    };

    // Apply scroll action based on direction
    if is_scroll_up {
        scroll_state.scroll_up(1);
    } else {
        scroll_state.scroll_down(1, max_entries);
    }

    state
}

// ===== Tests =====

#[cfg(test)]
#[path = "mouse_handler_tests.rs"]
mod tests;
