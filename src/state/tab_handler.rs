//! Tab navigation keyboard action handler.
//!
//! Pure functions that transform AppState in response to tab navigation actions.
//! Only operates when focus is on the Subagent pane.

use crate::model::KeyAction;
use crate::state::AppState;

/// Handle a tab navigation keyboard action.
///
/// # Arguments
/// * `state` - Current application state to transform
/// * `action` - The tab navigation action to handle
///
/// Returns a new AppState with the tab action applied.
pub fn handle_tab_action(mut state: AppState, action: KeyAction) -> AppState {
    match action {
        KeyAction::NextTab => {
            state.next_tab();
        }
        KeyAction::PrevTab => {
            state.prev_tab();
        }
        KeyAction::SelectTab(n) => {
            state.select_tab(n);
        }
        // Non-tab actions are no-ops
        _ => {}
    }

    state
}

// ===== Tests =====

#[cfg(test)]
#[path = "tab_handler_tests.rs"]
mod tests;
