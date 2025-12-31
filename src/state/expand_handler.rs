//! Message expand/collapse keyboard action handler.
//!
//! Pure functions that transform AppState in response to expand/collapse actions.
//! Focus-aware: dispatches actions to the correct ConversationViewState based on current focus.

use crate::model::KeyAction;
use crate::state::{AppState, FocusPane, WrapMode};
use crate::view_state::layout_params::LayoutParams;
use crate::view_state::types::{EntryIndex, LineHeight, ViewportDimensions};

/// Handle a message expand/collapse keyboard action.
///
/// # Arguments
/// * `state` - Current application state to transform
/// * `action` - The expand/collapse action to handle
///
/// Returns a new AppState with the expand/collapse action applied.
pub fn handle_expand_action(mut state: AppState, action: KeyAction) -> AppState {
    // Early return for non-expandable panes
    match state.focus {
        FocusPane::Stats | FocusPane::Search => return state,
        _ => {}
    }

    // Get layout params and viewport for relayout (needed by toggle_expand)
    let params = LayoutParams::new(80, state.global_wrap); // TODO: Use actual viewport width
    let viewport = ViewportDimensions::new(80, 24); // TODO: Use actual viewport dimensions

    // Height calculator stub for now
    let height_calc = |_entry: &crate::model::ConversationEntry,
                       _expanded: bool,
                       _wrap: WrapMode|
     -> LineHeight {
        LineHeight::new(5).unwrap() // Stub height
    };

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
// Tests removed during expand state migration to view-state layer
