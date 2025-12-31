//! Line wrap toggle keyboard action handler.
//!
//! Pure functions that transform AppState in response to wrap toggle actions.
//! Focus-aware: dispatches actions to the correct conversation view based on current focus.

use crate::state::{AppState, FocusPane, WrapMode};

/// Handle a line wrap toggle keyboard action for the focused message.
///
/// Toggles the wrap override for the focused entry in the conversation view-state.
/// Toggle semantics: if no override, set opposite of global; if override exists, clear it.
///
/// # Arguments
/// * `state` - Current application state to transform
///
/// Returns a new AppState with the wrap toggle applied (or unchanged if no message focused).
pub fn handle_toggle_wrap(mut state: AppState) -> AppState {
    // Stub height calculator - will be properly wired when rendering is migrated
    let stub_calculator =
        |_entry: &crate::model::ConversationEntry, _expanded: bool, _wrap: WrapMode| {
            crate::view_state::types::LineHeight::new(10).unwrap()
        };

    match state.focus {
        FocusPane::Main => {
            let global = state.global_wrap;
            let params = crate::view_state::layout_params::LayoutParams::new(80, global);

            if let Some(view) = state.main_conversation_view_mut() {
                if let Some(index) = view.focused_message() {
                    // Get current override to determine toggle behavior
                    let current_override = view.get(index).and_then(|e| e.wrap_override());

                    // Toggle logic: if override exists, clear it; else set to opposite of global
                    let new_override = match current_override {
                        Some(_) => None, // Clear override (returns to global)
                        None => Some(match global {
                            WrapMode::Wrap => WrapMode::NoWrap,
                            WrapMode::NoWrap => WrapMode::Wrap,
                        }),
                    };

                    view.set_wrap_override(index, new_override, params, stub_calculator);
                }
            }
        }
        FocusPane::Subagent => {
            if let Some(tab_index) = state.selected_tab {
                let global = state.global_wrap;
                let params = crate::view_state::layout_params::LayoutParams::new(80, global);

                if let Some(view) = state.subagent_conversation_view_mut(tab_index) {
                    if let Some(index) = view.focused_message() {
                        // Get current override to determine toggle behavior
                        let current_override = view.get(index).and_then(|e| e.wrap_override());

                        // Toggle logic: if override exists, clear it; else set to opposite of global
                        let new_override = match current_override {
                            Some(_) => None, // Clear override (returns to global)
                            None => Some(match global {
                                WrapMode::Wrap => WrapMode::NoWrap,
                                WrapMode::NoWrap => WrapMode::Wrap,
                            }),
                        };

                        view.set_wrap_override(index, new_override, params, stub_calculator);
                    }
                }
            }
        }
        _ => {} // No-op for Stats/Search panes
    }

    state
}

// ===== Tests =====

#[cfg(test)]
#[path = "wrap_handler_tests.rs"]
mod tests;
