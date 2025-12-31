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
    session: Session,
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
    pub fn new(session: Session) -> Self {
        Self {
            session,
            focus: FocusPane::Main,
            main_scroll: ScrollState::default(),
            subagent_scroll: ScrollState::default(),
            selected_tab: None,
            search: SearchState::Inactive,
            stats_filter: StatsFilter::Global,
            stats_visible: false,
            live_mode: false,
            auto_scroll: true,
        }
    }

    /// Add multiple log entries to the session.
    ///
    /// This is the proper way for the shell layer to add entries
    /// without directly mutating the core session state.
    pub fn add_entries(&mut self, entries: Vec<crate::model::LogEntry>) {
        for entry in entries {
            self.session.add_entry(entry);
        }
    }

    /// Get immutable reference to the session.
    pub fn session(&self) -> &Session {
        &self.session
    }

    /// Check if new messages indicator should be shown.
    /// Returns true when live_mode is active but auto_scroll is paused.
    /// This signals to the UI that new content has arrived below the current view.
    pub fn has_new_messages_indicator(&self) -> bool {
        self.live_mode && !self.auto_scroll
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
    pub fn scroll_up(&mut self, amount: usize) {
        self.vertical_offset = self.vertical_offset.saturating_sub(amount);
    }

    /// Scroll down by amount, clamped to max.
    pub fn scroll_down(&mut self, amount: usize, max: usize) {
        self.vertical_offset = (self.vertical_offset + amount).min(max);
    }

    /// Scroll left by amount, saturating at 0.
    pub fn scroll_left(&mut self, amount: usize) {
        self.horizontal_offset = self.horizontal_offset.saturating_sub(amount);
    }

    /// Scroll right by amount.
    pub fn scroll_right(&mut self, amount: usize) {
        self.horizontal_offset = self.horizontal_offset.saturating_add(amount);
    }

    /// Toggle expand/collapse for a message.
    pub fn toggle_expand(&mut self, uuid: &EntryUuid) {
        if self.expanded_messages.contains(uuid) {
            self.expanded_messages.remove(uuid);
        } else {
            self.expanded_messages.insert(uuid.clone());
        }
    }

    /// Check if a message is expanded.
    pub fn is_expanded(&self, uuid: &EntryUuid) -> bool {
        self.expanded_messages.contains(uuid)
    }

    /// Check if currently at bottom of scroll range.
    /// Returns true when vertical_offset equals max_entries.
    pub fn at_bottom(&self, max_entries: usize) -> bool {
        self.vertical_offset == max_entries
    }

    /// Scroll to the bottom of the content.
    /// Sets vertical_offset to max_entries.
    pub fn scroll_to_bottom(&mut self, max_entries: usize) {
        self.vertical_offset = max_entries;
    }
}

// ===== Tests =====

#[cfg(test)]
#[path = "app_state_tests.rs"]
mod tests;
