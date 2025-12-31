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
    pub help_visible: bool,
    pub live_mode: bool,
    pub auto_scroll: bool,
    pub global_wrap: WrapMode, // FR-039: toggleable line-wrapping
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
            help_visible: false,
            live_mode: false,
            auto_scroll: true,
            global_wrap: WrapMode::default(),
        }
    }

    /// Add multiple conversation entries (valid or malformed) to the session.
    ///
    /// This is the proper way for the shell layer to add entries
    /// without directly mutating the core session state.
    pub fn add_entries(&mut self, entries: Vec<crate::model::ConversationEntry>) {
        for entry in entries {
            self.session.add_conversation_entry(entry);
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

    /// Cycle focus between Main, Subagent, and Stats panes.
    /// Skip Search pane in the cycle.
    /// Order: Main -> Subagent -> Stats -> Main
    pub fn cycle_focus(&mut self) {
        self.focus = match self.focus {
            FocusPane::Main => FocusPane::Subagent,
            FocusPane::Subagent => FocusPane::Stats,
            FocusPane::Stats => FocusPane::Main,
            FocusPane::Search => FocusPane::Main,
        };
    }

    /// Set focus to Main pane.
    pub fn focus_main(&mut self) {
        self.focus = FocusPane::Main;
    }

    /// Set focus to Subagent pane.
    pub fn focus_subagent(&mut self) {
        self.focus = FocusPane::Subagent;
    }

    /// Set focus to Stats pane.
    pub fn focus_stats(&mut self) {
        self.focus = FocusPane::Stats;
    }

    /// Move to next subagent tab.
    /// Only works when focus is on Subagent pane.
    /// Wraps from last to first tab.
    pub fn next_tab(&mut self) {
        // Only operate when focus is on Subagent pane
        if self.focus != FocusPane::Subagent {
            return;
        }

        let num_subagents = self.session.subagents().len();

        // No-op if no subagents exist
        if num_subagents == 0 {
            return;
        }

        self.selected_tab = match self.selected_tab {
            None => Some(0), // Initialize to first tab
            Some(current) => {
                if current + 1 >= num_subagents {
                    Some(0) // Wrap to first
                } else {
                    Some(current + 1) // Move to next
                }
            }
        };
    }

    /// Move to previous subagent tab.
    /// Only works when focus is on Subagent pane.
    /// Wraps from first to last tab.
    pub fn prev_tab(&mut self) {
        // Only operate when focus is on Subagent pane
        if self.focus != FocusPane::Subagent {
            return;
        }

        let num_subagents = self.session.subagents().len();

        // No-op if no subagents exist
        if num_subagents == 0 {
            return;
        }

        self.selected_tab = match self.selected_tab {
            None => Some(0),                    // Initialize to first tab
            Some(0) => Some(num_subagents - 1), // Wrap to last
            Some(current) => Some(current - 1), // Move to previous
        };
    }

    /// Select a specific subagent tab by 1-indexed number.
    /// Only works when focus is on Subagent pane.
    /// Clamps to last tab if number is too high.
    /// Ignores if number is 0.
    pub fn select_tab(&mut self, tab_number: usize) {
        // Only operate when focus is on Subagent pane
        if self.focus != FocusPane::Subagent {
            return;
        }

        let num_subagents = self.session.subagents().len();

        // No-op if no subagents exist
        if num_subagents == 0 {
            return;
        }

        // Ignore 0 (invalid 1-indexed input)
        if tab_number == 0 {
            return;
        }

        // Convert from 1-indexed to 0-indexed, clamping to last tab
        let zero_indexed = tab_number - 1;
        self.selected_tab = Some(zero_indexed.min(num_subagents - 1));
    }

    /// Toggle global wrap mode (FR-050: W key)
    pub fn toggle_global_wrap(&mut self) {
        self.global_wrap = match self.global_wrap {
            WrapMode::Wrap => WrapMode::NoWrap,
            WrapMode::NoWrap => WrapMode::Wrap,
        };
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

// ===== WrapMode =====

/// Global line-wrapping mode.
/// Default: Wrap (FR-039: wrap enabled when config unset)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WrapMode {
    #[default]
    Wrap,
    NoWrap,
}

// ===== ScrollState =====

/// Scroll state for a pane.
/// Tracks vertical/horizontal offsets, which messages are expanded, and focus.
#[derive(Debug, Clone, Default)]
pub struct ScrollState {
    pub vertical_offset: usize,
    pub horizontal_offset: usize,
    pub expanded_messages: HashSet<EntryUuid>,
    pub focused_message: Option<usize>,
    /// Messages with wrap override (FR-048: per-item toggle overrides global)
    /// FR-049: ephemeral, not persisted
    pub wrap_overrides: HashSet<EntryUuid>,
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

    /// Expand all messages by adding all UUIDs to expanded_messages.
    pub fn expand_all(&mut self, uuids: impl Iterator<Item = EntryUuid>) {
        for uuid in uuids {
            self.expanded_messages.insert(uuid);
        }
    }

    /// Collapse all messages by clearing the expanded_messages set.
    pub fn collapse_all(&mut self) {
        self.expanded_messages.clear();
    }

    /// Set the focused message index.
    pub fn set_focused_message(&mut self, index: Option<usize>) {
        self.focused_message = index;
    }

    /// Get the focused message index.
    pub fn focused_message(&self) -> Option<usize> {
        self.focused_message
    }

    /// Toggle wrap override for a specific message (FR-050: w key)
    pub fn toggle_wrap(&mut self, uuid: &EntryUuid) {
        if self.wrap_overrides.contains(uuid) {
            self.wrap_overrides.remove(uuid);
        } else {
            self.wrap_overrides.insert(uuid.clone());
        }
    }

    /// Get effective wrap mode for a message (FR-048)
    /// Per-item override inverts the global setting
    pub fn effective_wrap(&self, uuid: &EntryUuid, global: WrapMode) -> WrapMode {
        if self.wrap_overrides.contains(uuid) {
            match global {
                WrapMode::Wrap => WrapMode::NoWrap,
                WrapMode::NoWrap => WrapMode::Wrap,
            }
        } else {
            global
        }
    }
}

// ===== Tests =====

#[cfg(test)]
#[path = "app_state_tests.rs"]
mod tests;
