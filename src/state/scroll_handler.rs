//! Vertical scrolling keyboard action handler.
//!
//! Pure functions that transform AppState in response to scroll actions.
//! Focus-aware: dispatches actions to the correct ConversationViewState based on current focus.
//!
//! # Migration Note
//! This handler now uses ConversationViewState.set_scroll() with ScrollPosition variants
//! instead of directly modifying ScrollState.vertical_offset (which has been removed).
//! Scroll position is now line-based (total_height in lines) rather than entry-based.

use crate::model::KeyAction;
use crate::state::{AppState, FocusPane};
use crate::view_state::scroll::ScrollPosition;

/// Handle a scroll keyboard action, dispatching to the appropriate conversation view.
///
/// # Arguments
/// * `state` - Current application state to transform
/// * `action` - The scroll action to handle
/// * `viewport_height` - Height of the visible viewport (for page scrolling)
///
/// Returns a new AppState with the scroll action applied via ScrollPosition.
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

    // Get mutable reference to the appropriate conversation view-state
    let conversation = match state.focus {
        FocusPane::Main => {
            if let Some(session) = state.log_view_mut().current_session_mut() {
                session.main_mut()
            } else {
                return state; // No session, nothing to scroll
            }
        }
        FocusPane::Subagent => {
            // Get the currently selected subagent's conversation
            if let Some(tab_index) = state.selected_tab {
                // Get agent ID and clone to avoid borrow conflicts
                let agent_id = state.session().subagent_ids_ordered()
                    .get(tab_index)
                    .map(|id| (*id).clone());

                if let Some(agent_id) = agent_id {
                    if let Some(session) = state.log_view_mut().current_session_mut() {
                        session.subagent_mut(&agent_id)
                    } else {
                        return state;
                    }
                } else {
                    return state;
                }
            } else {
                return state; // No tab selected
            }
        }
        _ => return state, // Already handled above
    };

    // Get current scroll position
    let current_scroll = conversation.scroll().clone();

    // Calculate new scroll position based on action
    let new_scroll = match action {
        KeyAction::ScrollUp => {
            // Scroll up by 1 line
            match current_scroll {
                ScrollPosition::AtLine(offset) => {
                    ScrollPosition::AtLine(offset.saturating_sub(1))
                }
                ScrollPosition::Top => ScrollPosition::Top, // Already at top
                _ => {
                    // Resolve current position to line offset, then scroll up
                    let total_height = conversation.total_height();
                    let offset = current_scroll.resolve(
                        total_height,
                        viewport_height,
                        |idx| conversation.entry_cumulative_y(idx),
                    );
                    ScrollPosition::AtLine(offset.saturating_sub(1))
                }
            }
        }
        KeyAction::ScrollDown => {
            // Scroll down by 1 line
            match current_scroll {
                ScrollPosition::AtLine(offset) => {
                    ScrollPosition::AtLine(offset.saturating_add(1))
                }
                ScrollPosition::Bottom => ScrollPosition::Bottom, // Already at bottom
                _ => {
                    // Resolve current position to line offset, then scroll down
                    let total_height = conversation.total_height();
                    let offset = current_scroll.resolve(
                        total_height,
                        viewport_height,
                        |idx| conversation.entry_cumulative_y(idx),
                    );
                    ScrollPosition::AtLine(offset.saturating_add(1))
                }
            }
        }
        KeyAction::PageUp => {
            // Scroll up by viewport_height
            match current_scroll {
                ScrollPosition::AtLine(offset) => {
                    ScrollPosition::AtLine(offset.saturating_sub(viewport_height))
                }
                ScrollPosition::Top => ScrollPosition::Top, // Already at top
                _ => {
                    // Resolve current position to line offset, then page up
                    let total_height = conversation.total_height();
                    let offset = current_scroll.resolve(
                        total_height,
                        viewport_height,
                        |idx| conversation.entry_cumulative_y(idx),
                    );
                    ScrollPosition::AtLine(offset.saturating_sub(viewport_height))
                }
            }
        }
        KeyAction::PageDown => {
            // Scroll down by viewport_height
            match current_scroll {
                ScrollPosition::AtLine(offset) => {
                    ScrollPosition::AtLine(offset.saturating_add(viewport_height))
                }
                ScrollPosition::Bottom => ScrollPosition::Bottom, // Already at bottom
                _ => {
                    // Resolve current position to line offset, then page down
                    let total_height = conversation.total_height();
                    let offset = current_scroll.resolve(
                        total_height,
                        viewport_height,
                        |idx| conversation.entry_cumulative_y(idx),
                    );
                    ScrollPosition::AtLine(offset.saturating_add(viewport_height))
                }
            }
        }
        KeyAction::ScrollToTop => {
            // Jump to top
            ScrollPosition::Top
        }
        KeyAction::ScrollToBottom => {
            // Jump to bottom
            ScrollPosition::Bottom
        }
        KeyAction::ScrollLeft => {
            // Horizontal scrolling still uses ScrollState
            state.main_scroll.scroll_left(1);
            return state;
        }
        KeyAction::ScrollRight => {
            // Horizontal scrolling still uses ScrollState
            state.main_scroll.scroll_right(1);
            return state;
        }
        // Non-scroll actions are no-ops
        _ => return state,
    };

    // Apply the new scroll position
    conversation.set_scroll(new_scroll);

    state
}

// ===== Tests =====

#[cfg(test)]
#[path = "scroll_handler_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "scroll_handler_migration_tests.rs"]
mod migration_tests;
