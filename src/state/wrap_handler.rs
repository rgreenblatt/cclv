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
/// * `viewport_width` - Viewport width in characters for layout calculations
///
/// Returns a new AppState with the wrap toggle applied (or unchanged if no message focused).
///
/// # Routing Logic
///
/// Routes to the appropriate conversation view based on selected_tab using central routing:
/// - Tab 0 = Main agent conversation
/// - Tab 1+ = Subagent conversations (index - 1 in sorted subagent list)
///
/// This matches the routing in scroll_handler.rs and expand_handler.rs to ensure consistency.
pub fn handle_toggle_wrap(mut state: AppState, _viewport_width: u16) -> AppState {
    // Early return for non-toggleable panes
    match state.focus {
        FocusPane::Stats | FocusPane::Search => return state,
        _ => {}
    }

    // Read global wrap mode and clone search state before borrowing conversation mutably
    let global = state.global_wrap;
    let search_state = state.search.clone();

    // Get mutable reference to the selected conversation using central routing
    let conversation = if let Some(conv) = state.selected_conversation_view_mut() {
        conv
    } else {
        return state; // No conversation selected, nothing to toggle
    };

    if let Some(index) = conversation.focused_message() {
        // Get current override to determine toggle behavior
        let current_override = conversation.get(index).and_then(|e| e.wrap_override());

        // Toggle logic: if override exists, clear it; else set to opposite of global
        let new_override = match current_override {
            Some(_) => None, // Clear override (returns to global)
            None => Some(match global {
                WrapMode::Wrap => WrapMode::NoWrap,
                WrapMode::NoWrap => WrapMode::Wrap,
            }),
        };

        conversation.set_entry_wrap_override(index.get(), new_override, &search_state);
    }

    state
}

// ===== Tests =====

#[cfg(test)]
#[path = "wrap_handler_tests.rs"]
mod tests;
