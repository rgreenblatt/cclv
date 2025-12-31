//! Search input handling (pure state transitions).
//!
//! Handles text input for the SearchState::Typing variant.
//! All functions are pure - no side effects, testable without TUI.

use crate::state::SearchState;

/// Handle character input when in Typing state.
/// Inserts the character at cursor position and advances cursor.
///
/// Returns updated SearchState. No-op if not in Typing state.
pub fn handle_char_input(state: SearchState, ch: char) -> SearchState {
    match state {
        SearchState::Typing { mut query, cursor } => {
            query.insert(cursor, ch);
            SearchState::Typing {
                query,
                cursor: cursor + 1,
            }
        }
        // No-op for other states
        other => other,
    }
}

/// Handle backspace when in Typing state.
/// Deletes character before cursor if cursor > 0.
///
/// Returns updated SearchState. No-op if not in Typing state.
pub fn handle_backspace(state: SearchState) -> SearchState {
    match state {
        SearchState::Typing { mut query, cursor } => {
            if cursor > 0 {
                query.remove(cursor - 1);
                SearchState::Typing {
                    query,
                    cursor: cursor - 1,
                }
            } else {
                // cursor == 0, can't delete
                SearchState::Typing { query, cursor }
            }
        }
        // No-op for other states
        other => other,
    }
}

/// Move cursor left by one position.
/// Saturates at 0 (does not wrap).
///
/// Returns updated SearchState. No-op if not in Typing state.
pub fn handle_cursor_left(state: SearchState) -> SearchState {
    match state {
        SearchState::Typing { query, cursor } => SearchState::Typing {
            query,
            cursor: cursor.saturating_sub(1),
        },
        // No-op for other states
        other => other,
    }
}

/// Move cursor right by one position.
/// Saturates at query length (does not wrap).
///
/// Returns updated SearchState. No-op if not in Typing state.
pub fn handle_cursor_right(state: SearchState) -> SearchState {
    match state {
        SearchState::Typing { query, cursor } => {
            let max_cursor = query.len();
            SearchState::Typing {
                query,
                cursor: (cursor + 1).min(max_cursor),
            }
        }
        // No-op for other states
        other => other,
    }
}

/// Activate search input mode.
/// Transitions from Inactive to Typing with empty query and cursor at 0.
///
/// No-op if already in Typing or Active state.
pub fn activate_search_input(state: SearchState) -> SearchState {
    match state {
        SearchState::Inactive => SearchState::Typing {
            query: String::new(),
            cursor: 0,
        },
        // No-op if already in Typing or Active
        other => other,
    }
}

/// Cancel search input.
/// Transitions from Typing or Active to Inactive.
///
/// No-op if already Inactive.
pub fn cancel_search(state: SearchState) -> SearchState {
    match state {
        SearchState::Typing { .. } | SearchState::Active { .. } => SearchState::Inactive,
        SearchState::Inactive => SearchState::Inactive,
    }
}

/// Submit search query.
/// Transitions from Typing to Active if query is non-empty.
/// If query is empty, transitions to Inactive instead.
///
/// Returns updated SearchState. No-op if not in Typing state.
/// Note: Actual search execution happens elsewhere - this just changes state.
pub fn submit_search(state: SearchState) -> SearchState {
    match state {
        SearchState::Typing { query, .. } => {
            // Try to create SearchQuery (validates non-empty)
            match crate::state::SearchQuery::new(query) {
                Some(search_query) => SearchState::Active {
                    query: search_query,
                    matches: vec![],
                    current_match: 0,
                },
                None => SearchState::Inactive, // Empty/whitespace query
            }
        }
        // No-op for other states
        other => other,
    }
}

// ===== Tests =====

#[cfg(test)]
#[path = "search_input_handler_tests.rs"]
mod tests;
