//! Message expand/collapse keyboard action handler.
//!
//! Pure functions that transform AppState in response to expand/collapse actions.
//! Focus-aware: dispatches actions to the correct ConversationViewState based on current focus.

use crate::model::KeyAction;
use crate::state::{AppState, FocusPane};
use crate::view_state::layout_params::LayoutParams;
use crate::view_state::types::{EntryIndex, ViewportDimensions};

/// Handle a message expand/collapse keyboard action.
///
/// # Arguments
/// * `state` - Current application state to transform
/// * `action` - The expand/collapse action to handle
/// * `viewport_width` - Viewport width in characters for layout calculations
///
/// Returns a new AppState with the expand/collapse action applied.
pub fn handle_expand_action(
    mut state: AppState,
    action: KeyAction,
    viewport_width: u16,
) -> AppState {
    // Early return for non-expandable panes
    match state.focus {
        FocusPane::Stats | FocusPane::Search => return state,
        _ => {}
    }

    // Get layout params and viewport for relayout (needed by toggle_expand)
    let params = LayoutParams::new(viewport_width, state.global_wrap);
    let viewport = ViewportDimensions::new(viewport_width, 24); // Height not used for expand

    // Use the real height calculator from view layer
    let height_calc = crate::view::calculate_entry_height;

    // Apply the action based on focus
    match state.focus {
        FocusPane::Main => {
            if let Some(session_view) = state.log_view_mut().current_session_mut() {
                let conv_view = session_view.main_mut();

                match action {
                    KeyAction::ToggleExpand => {
                        // Toggle the focused message via ConversationViewState
                        if let Some(focused_idx) = conv_view.focused_message() {
                            conv_view.toggle_expand(focused_idx, params, viewport, height_calc);
                        }
                    }
                    KeyAction::ExpandMessage => {
                        // Expand all messages in main pane
                        let count = conv_view.len();
                        for i in 0..count {
                            let idx = EntryIndex::new(i);
                            if let Some(entry) = conv_view.get(idx) {
                                if !entry.is_expanded() {
                                    conv_view.toggle_expand(idx, params, viewport, height_calc);
                                }
                            }
                        }
                    }
                    KeyAction::CollapseMessage => {
                        // Collapse all messages in main pane
                        let count = conv_view.len();
                        for i in 0..count {
                            let idx = EntryIndex::new(i);
                            if let Some(entry) = conv_view.get(idx) {
                                if entry.is_expanded() {
                                    conv_view.toggle_expand(idx, params, viewport, height_calc);
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        FocusPane::Subagent => {
            // TODO: Implement subagent expand/collapse using ConversationViewState
            // This requires identifying which subagent tab is selected and getting its ConversationViewState
        }
        _ => {}
    }

    state
}

// ===== Tests =====

#[cfg(test)]
#[path = "expand_handler_tests.rs"]
mod tests;
