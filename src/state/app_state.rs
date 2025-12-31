//! Application state and transitions.
//!
//! AppState is the root state type containing all UI state.
//! All state transitions are pure functions following Elm architecture.

use crate::model::{EntryUuid, Session, StatsFilter};
use crate::state::SearchState;
use std::collections::HashSet;

// ===== AppState =====

/// Application state. Pure data, no side effects.
/// This is the root state type containing all UI state.
#[derive(Debug, Clone)]
pub struct AppState {
    pub session: Session,
    pub focus: FocusPane,
    pub main_scroll: ScrollState,
    pub subagent_scroll: ScrollState,
    pub selected_tab: Option<usize>,
    pub search: SearchState,
    pub stats_filter: StatsFilter,
    pub stats_visible: bool,
    pub live_mode: bool,
    pub auto_scroll: bool,
}

impl AppState {
    /// Create new AppState with default UI state.
    pub fn new(_session: Session) -> Self {
        todo!("AppState::new")
    }
}

// ===== FocusPane =====

/// Which pane has focus. Sum type - exactly one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusPane {
    Main,
    Subagent,
    Stats,
    Search,
}

// ===== ScrollState =====

/// Scroll state for a pane.
/// Tracks vertical/horizontal offsets and which messages are expanded.
#[derive(Debug, Clone, Default)]
pub struct ScrollState {
    pub vertical_offset: usize,
    pub horizontal_offset: usize,
    pub expanded_messages: HashSet<EntryUuid>,
}

impl ScrollState {
    /// Scroll up by amount, saturating at 0.
    pub fn scroll_up(&mut self, _amount: usize) {
        todo!("ScrollState::scroll_up")
    }

    /// Scroll down by amount, clamped to max.
    pub fn scroll_down(&mut self, _amount: usize, _max: usize) {
        todo!("ScrollState::scroll_down")
    }

    /// Scroll left by amount, saturating at 0.
    pub fn scroll_left(&mut self, _amount: usize) {
        todo!("ScrollState::scroll_left")
    }

    /// Scroll right by amount.
    pub fn scroll_right(&mut self, _amount: usize) {
        todo!("ScrollState::scroll_right")
    }

    /// Toggle expand/collapse for a message.
    pub fn toggle_expand(&mut self, _uuid: &EntryUuid) {
        todo!("ScrollState::toggle_expand")
    }

    /// Check if a message is expanded.
    pub fn is_expanded(&self, _uuid: &EntryUuid) -> bool {
        todo!("ScrollState::is_expanded")
    }
}

// ===== Tests =====

#[cfg(test)]
#[path = "app_state_tests.rs"]
mod tests;
