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

    // Calculate which tab was clicked
    // Each tab gets equal width
    let tab_count = agent_ids.len() as u16;
    let tab_width = tab_area.width / tab_count;

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

// ===== Tests =====

#[cfg(test)]
#[path = "mouse_handler_tests.rs"]
mod tests;
