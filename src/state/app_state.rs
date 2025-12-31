//! Application state and transitions.
//!
//! AppState is the root state type containing all UI state.
//! All state transitions are pure functions following Elm architecture.

use crate::model::{EntryUuid, Session, StatsFilter};
use crate::state::{LogPaneState, SearchState};
use std::collections::HashSet;

// ===== AppState =====

/// Application state. Pure data, no side effects.
///
/// This is the root state type containing all UI state following the Elm Architecture.
/// All state transitions are pure functions that return new state values.
///
/// # State Machine
///
/// The UI operates as a state machine with these primary states:
///
/// - **Focus**: Which pane has keyboard focus (Main, Subagent, Stats, Search)
/// - **Live Mode**: Following a log file in real-time vs viewing static content
/// - **Auto-Scroll**: Automatically scrolling to new content vs manual navigation
/// - **Search**: Inactive, typing query, or displaying active results
///
/// # State Transitions
///
/// Valid state transitions (see methods for details):
///
/// - Focus: Main ⇄ Subagent ⇄ Stats (via `cycle_focus`, `focus_*` methods)
/// - Search: Inactive → Typing → Active → Inactive (via SearchState transitions)
/// - Auto-scroll: On → Off (when user scrolls up in live mode, FR-036)
/// - Auto-scroll: Off → On (when user returns to bottom, FR-038)
///
/// # Functional Requirements
///
/// - **FR-001**: Main agent conversation displayed in dedicated pane
/// - **FR-003**: Subagent conversations in tabbed pane
/// - **FR-035**: Auto-scroll enabled by default in live mode
/// - **FR-036**: Auto-scroll pauses when user scrolls away from bottom
/// - **FR-039**: Toggleable line-wrapping with configurable default
#[derive(Debug, Clone)]
pub struct AppState {
    /// The session data containing all parsed log entries and statistics.
    /// This is the domain model; all other fields are UI state.
    session: Session,

    /// Which pane currently has keyboard focus.
    /// Determines which pane receives keyboard input and displays focus indicator.
    /// See `FocusPane` for valid states and transitions.
    pub focus: FocusPane,

    /// Scroll state for the main agent conversation pane.
    /// Tracks vertical/horizontal offset, expanded messages, and per-item wrap overrides.
    pub main_scroll: ScrollState,

    /// Scroll state for the currently selected subagent tab.
    /// Each subagent maintains independent scroll state.
    pub subagent_scroll: ScrollState,

    /// Currently selected subagent tab index (0-based).
    /// `None` means no tab is selected (e.g., no subagents exist).
    /// Valid range: `0..session.subagents().len()`.
    pub selected_tab: Option<usize>,

    /// Current search state (inactive, typing, or active with results).
    /// See `SearchState` for the search state machine.
    pub search: SearchState,

    /// Filter for statistics display (Global, MainAgent, or specific Subagent).
    /// Controls which agent's statistics are shown in the stats pane.
    pub stats_filter: StatsFilter,

    /// Whether the statistics pane is currently visible.
    /// Toggled by user action (FR-019).
    pub stats_visible: bool,

    /// Whether the help overlay is currently visible.
    /// Toggled by user action to show keyboard shortcuts.
    pub help_visible: bool,

    /// Whether the application is following a live log file.
    /// `true` means new entries are being tailed in real-time (FR-007).
    /// `false` means viewing a static/completed log file (FR-008).
    pub live_mode: bool,

    /// Whether auto-scroll is active (only meaningful when `live_mode` is true).
    /// `true` means automatically scroll to new content as it arrives (FR-035).
    /// `false` means user has scrolled away from bottom; pause auto-scroll (FR-036).
    pub auto_scroll: bool,

    /// Global line-wrapping mode for all panes.
    /// Individual messages can override via `ScrollState::wrap_overrides` (FR-048).
    /// Default is `Wrap` when config is unset (FR-039).
    pub global_wrap: WrapMode,

    /// Toggleable internal logging pane state.
    /// Maintains a ring buffer of log entries with unread tracking.
    pub log_pane: LogPaneState,
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
            log_pane: LogPaneState::new(1000),
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
///
/// Determines which pane receives keyboard input and displays the focus indicator.
///
/// # State Transitions
///
/// - Main ⇄ Subagent ⇄ Stats (via Tab or explicit focus commands, FR-025)
/// - Any → Search (when user activates search with `/` or Ctrl+F, FR-004)
/// - Search → (previous pane) (when search is cancelled or submitted)
///
/// # Keyboard Navigation (FR-025)
///
/// - Tab: cycle through Main → Subagent → Stats → Main
/// - Explicit focus keys: focus specific panes directly
/// - Search activation: temporarily moves to Search pane
///
/// Note: Search pane is skipped in the normal focus cycle and is only entered
/// when the user explicitly activates search mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusPane {
    /// Main agent conversation pane has focus.
    /// User can scroll, expand/collapse messages, and search within main agent.
    Main,

    /// Subagent conversation pane has focus.
    /// User can switch tabs, scroll, and interact with selected subagent's messages.
    Subagent,

    /// Statistics panel has focus.
    /// User can view token usage, costs, and tool counts (FR-015 to FR-020).
    Stats,

    /// Search input has focus.
    /// User is typing a search query. Entered via `/` or Ctrl+F (FR-004).
    Search,
}

// ===== WrapMode =====

/// Global line-wrapping mode for prose text.
///
/// Controls whether long lines of prose text wrap to fit the viewport width
/// or extend horizontally requiring horizontal scrolling.
///
/// # Behavior (FR-039, FR-050, FR-053)
///
/// - **Prose text**: Follows this global setting (toggleable with `W` key)
/// - **Code blocks**: NEVER wrap, always use horizontal scroll (FR-053)
/// - **Per-item override**: Individual messages can override via `ScrollState::wrap_overrides` (FR-048)
///
/// # Default (FR-039)
///
/// Defaults to `Wrap` when not configured. This ensures readable prose by default
/// while preserving code formatting.
///
/// # State Transitions
///
/// - Wrap ⇄ NoWrap (via `AppState::toggle_global_wrap`, bound to `W` key by default)
///
/// Note: Wrapping state is global but individual messages can override it per-item.
/// Per-item overrides are ephemeral and not persisted across sessions (FR-049).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WrapMode {
    /// Wrap long lines to fit viewport width.
    /// Prose flows naturally; wrapped lines show continuation indicator (↩) (FR-052).
    #[default]
    Wrap,

    /// Do not wrap lines; use horizontal scrolling instead.
    /// User must scroll left/right to view content beyond viewport (FR-040).
    NoWrap,
}

// ===== ScrollState =====

/// Scroll state for a conversation pane.
///
/// Tracks the viewport position, message expansion state, and per-message wrap overrides
/// for a single conversation pane (either main agent or a subagent).
///
/// # Invariants
///
/// - `vertical_offset ≤ max_entries` (enforced by `scroll_down`)
/// - `horizontal_offset ≥ 0` (enforced by `scroll_left` saturation)
/// - `expanded_messages` contains only valid `EntryUuid`s from the conversation
/// - `focused_message < conversation.entries().len()` when `Some`
///
/// # State Independence (FR-034)
///
/// Each conversation pane maintains independent scroll state. Scrolling in the main
/// pane does not affect subagent panes, and vice versa. This allows users to navigate
/// different conversations at their own pace.
///
/// # Expansion State (FR-031 to FR-033)
///
/// Messages longer than the collapse threshold (default 10 lines) are collapsed by
/// default, showing only the first 3 lines plus "(+N more lines)" indicator.
/// Users can expand/collapse individual messages via `toggle_expand`.
///
/// # Wrap Overrides (FR-048, FR-049)
///
/// Individual messages can override the global wrap mode. This is ephemeral state
/// (not persisted across sessions). Per-item toggle inverts the global setting:
/// - Global=Wrap, Override present → NoWrap for this message
/// - Global=NoWrap, Override present → Wrap for this message
#[derive(Debug, Clone, Default)]
pub struct ScrollState {
    /// Vertical scroll offset (number of lines scrolled down from top).
    /// 0 means viewing from the first line. Clamped to conversation length.
    pub vertical_offset: usize,

    /// Horizontal scroll offset (number of characters scrolled right from left edge).
    /// Only relevant when line wrapping is disabled (FR-040).
    /// 0 means viewing from the leftmost column.
    pub horizontal_offset: usize,

    /// Set of message UUIDs that are currently expanded.
    /// Messages NOT in this set are displayed in collapsed form (if they exceed threshold).
    /// Toggled via `toggle_expand` (FR-032, FR-033).
    pub expanded_messages: HashSet<EntryUuid>,

    /// Index of the currently focused message within this pane's entry list.
    /// `None` means no specific message has focus (pane-level focus only).
    /// Used for keyboard navigation within the conversation.
    pub focused_message: Option<usize>,

    /// Set of message UUIDs with per-item wrap mode override.
    /// Presence in this set inverts the global wrap setting for that message.
    /// Toggled via `toggle_wrap` with `w` key (FR-048, FR-050).
    /// EPHEMERAL: Not persisted across sessions (FR-049).
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
