//! Match navigation handler.
//!
//! Pure functions for navigating between search matches.
//! Handles next/prev navigation with wrap-around and focus/tab switching.

use crate::model::AgentId;
use crate::state::{AppState, FocusPane, SearchState};

// ===== Public API =====

/// Navigate to the next search match.
///
/// Behavior:
/// - If not in Active search state, does nothing
/// - Increments current_match by 1
/// - Wraps from last match to first (0)
/// - Switches focus to Main or Subagent pane based on match location
/// - Selects correct subagent tab if match is in a subagent
pub fn next_match(mut state: AppState) -> AppState {
    // Only operate when in Active search state
    if let SearchState::Active {
        query,
        matches,
        current_match,
    } = &state.search
    {
        // Cannot navigate if no matches
        if matches.is_empty() {
            return state;
        }

        // Calculate next match index with wrap-around
        let next_index = if *current_match + 1 >= matches.len() {
            0 // Wrap to first
        } else {
            current_match + 1
        };

        // Clone data we need before mutating state
        let target_agent_id = matches[next_index].agent_id.clone();
        let query = query.clone();
        let matches = matches.clone();

        // Update search state with new current_match
        state.search = SearchState::Active {
            query,
            matches,
            current_match: next_index,
        };

        // Switch focus/tab to match location
        switch_to_match_location(state, &target_agent_id)
    } else {
        // Not in Active state - do nothing
        state
    }
}

/// Navigate to the previous search match.
///
/// Behavior:
/// - If not in Active search state, does nothing
/// - Decrements current_match by 1
/// - Wraps from first match (0) to last
/// - Switches focus to Main or Subagent pane based on match location
/// - Selects correct subagent tab if match is in a subagent
pub fn prev_match(mut state: AppState) -> AppState {
    // Only operate when in Active search state
    if let SearchState::Active {
        query,
        matches,
        current_match,
    } = &state.search
    {
        // Cannot navigate if no matches
        if matches.is_empty() {
            return state;
        }

        // Calculate previous match index with wrap-around
        let prev_index = if *current_match == 0 {
            matches.len() - 1 // Wrap to last
        } else {
            current_match - 1
        };

        // Clone data we need before mutating state
        let target_agent_id = matches[prev_index].agent_id.clone();
        let query = query.clone();
        let matches = matches.clone();

        // Update search state with new current_match
        state.search = SearchState::Active {
            query,
            matches,
            current_match: prev_index,
        };

        // Switch focus/tab to match location
        switch_to_match_location(state, &target_agent_id)
    } else {
        // Not in Active state - do nothing
        state
    }
}

// ===== Helper Functions =====

/// Find the tab index for a given agent_id.
/// Returns None if agent_id is not found in subagents.
fn find_tab_for_agent(state: &AppState, agent_id: &AgentId) -> Option<usize> {
    state.tab_index_for_agent(agent_id)
}

/// Switch focus and tab to the correct location for a search match.
/// If agent_id is None, switches to Main pane.
/// If agent_id is Some, switches to Subagent pane and selects the correct tab.
fn switch_to_match_location(mut state: AppState, agent_id: &Option<AgentId>) -> AppState {
    match agent_id {
        None => {
            // Match is in main agent - switch to Main pane
            state.focus = FocusPane::Main;
        }
        Some(aid) => {
            // Match is in subagent - switch to Subagent pane and select tab
            state.focus = FocusPane::Subagent;

            // Find and select the tab for this agent
            if let Some(tab_index) = find_tab_for_agent(&state, aid) {
                state.selected_tab = Some(tab_index);
            }
        }
    }
    state
}

// ===== Tests =====

#[cfg(test)]
#[path = "match_navigation_handler_tests.rs"]
mod tests;
