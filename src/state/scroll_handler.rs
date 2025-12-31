//! Vertical scrolling keyboard action handler.
//!
//! Pure functions that transform AppState in response to scroll actions.
//! Focus-aware: dispatches actions to the correct scroll state based on current focus.

use crate::model::KeyAction;
use crate::state::{AppState, FocusPane};

/// Handle a scroll keyboard action, dispatching to the appropriate scroll state.
///
/// # Arguments
/// * `state` - Current application state to transform
/// * `action` - The scroll action to handle
/// * `viewport_height` - Height of the visible viewport (for page scrolling)
///
/// Returns a new AppState with the scroll action applied.
pub fn handle_scroll_action(
    mut state: AppState,
    action: KeyAction,
    viewport_height: usize,
) -> AppState {
    // Early return for non-scrollable panes
    match state.focus {
        FocusPane::Stats | FocusPane::Search => return state,
        _ => {}
    }

    // Calculate max scroll offset based on which pane is focused
    // Must do this BEFORE taking mutable borrow of scroll_state
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

    // Apply the scroll action
    match action {
        KeyAction::ScrollUp => {
            scroll_state.scroll_up(1);
        }
        KeyAction::ScrollDown => {
            scroll_state.scroll_down(1, max_entries);
        }
        KeyAction::ScrollLeft => {
            scroll_state.scroll_left(1);
        }
        KeyAction::ScrollRight => {
            scroll_state.scroll_right(1);
        }
        KeyAction::PageUp => {
            scroll_state.scroll_up(viewport_height);
        }
        KeyAction::PageDown => {
            scroll_state.scroll_down(viewport_height, max_entries);
        }
        KeyAction::ScrollToTop => {
            scroll_state.vertical_offset = 0;
        }
        KeyAction::ScrollToBottom => {
            scroll_state.scroll_to_bottom(max_entries);
        }
        // Non-scroll actions are no-ops
        _ => {}
    }

    state
}

// ===== Tests =====

#[cfg(test)]
#[path = "scroll_handler_tests.rs"]
mod tests;
