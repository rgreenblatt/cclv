//! UI state machine (pure).
//!
//! All state transitions are pure functions testable without TUI.

pub mod app_state;
pub mod search;

// Re-export for convenience
pub use app_state::{AppState, FocusPane, ScrollState};
pub use search::{SearchMatch, SearchQuery, SearchState};
