//! Line wrap toggle keyboard action handler.
//!
//! Pure functions that transform AppState in response to wrap toggle actions.
//! Focus-aware: dispatches actions to the correct scroll state based on current focus.

use crate::state::{AppState, FocusPane};

/// Handle a line wrap toggle keyboard action for the focused message.
///
/// Looks up the currently focused message's UUID and toggles its wrap override
/// in the appropriate scroll state (main or subagent).
///
/// # Arguments
/// * `state` - Current application state to transform
///
/// Returns a new AppState with the wrap toggle applied (or unchanged if no message focused).
pub fn handle_toggle_wrap(mut state: AppState) -> AppState {
    // First, get the UUID of the focused entry (reading phase)
    let focused_uuid = match state.focus {
        FocusPane::Main => {
            // Get focused message index from main scroll state
            if let Some(focused_index) = state.main_scroll.focused_message() {
                state
                    .session()
                    .main_agent()
                    .entries()
                    .get(focused_index)
                    .and_then(|e| e.as_valid())
                    .map(|log| log.uuid().clone())
            } else {
                None
            }
        }
        FocusPane::Subagent => {
            // Get focused message index from subagent scroll state
            if let Some(focused_index) = state.subagent_scroll.focused_message() {
                // Get the currently selected subagent's entries
                if let Some(tab_index) = state.selected_tab {
                    let subagent_ids = state.session().subagent_ids_ordered();
                    if let Some(&agent_id) = subagent_ids.get(tab_index) {
                        state.session().subagents().get(agent_id).and_then(|conv| {
                            conv.entries()
                                .get(focused_index)
                                .and_then(|e| e.as_valid())
                                .map(|log| log.uuid().clone())
                        })
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            }
        }
        _ => None, // No-op for Stats/Search panes
    };

    // Then, toggle wrap for the UUID (mutation phase)
    if let Some(uuid) = focused_uuid {
        let scroll_state = match state.focus {
            FocusPane::Main => &mut state.main_scroll,
            FocusPane::Subagent => &mut state.subagent_scroll,
            _ => return state, // Unreachable due to focused_uuid pattern, but defensive
        };
        scroll_state.toggle_wrap(&uuid);
    }

    state
}

// ===== Tests =====

#[cfg(test)]
#[path = "wrap_handler_tests.rs"]
mod tests;
