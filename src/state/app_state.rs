//! Application state and transitions.
//!
//! AppState is the root state type containing all UI state.
//! All state transitions are pure functions following Elm architecture.

use crate::model::{EntryUuid, Session, StatsFilter};
use crate::state::SearchState;
use crate::view_state::log::LogViewState;
use std::collections::HashSet;

// ===== InputMode =====

/// Input mode indicator for the LIVE status indicator.
///
/// Tracks whether the application is reading from a static file,
/// actively streaming from stdin, or has reached EOF on stdin.
///
/// # Functional Requirements
///
/// - **FR-042b**: LIVE indicator displays gray when Static or Eof,
///   blinking green when Streaming
///
/// # State Transitions
///
/// - Static (file input, default)
/// - Streaming (stdin active)
/// - Eof (stdin reached end)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InputMode {
    /// File loaded once, no live updates expected.
    /// This is the default variant (FR-042b).
    #[default]
    Static,

    /// Actively streaming from stdin.
    Streaming,

    /// Stdin has reached EOF, no more data expected.
    Eof,
}

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

    /// View-state layer for rendering log entries.
    /// This becomes the primary source of truth for entry layout and display.
    /// Dual-write pattern during migration (cclv-5ur.6.x tasks).
    log_view: LogViewState,

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

    /// Input mode for LIVE indicator display (FR-042b).
    /// Indicates whether reading from static file, actively streaming, or EOF.
    pub input_mode: InputMode,

    /// Blink state for LIVE indicator animation (FR-028).
    /// Toggles on 500ms timer events when `input_mode` is Streaming.
    /// `true` means indicator is visible (green), `false` means hidden.
    pub blink_on: bool,
}

impl AppState {
    /// Create new AppState with default UI state.
    pub fn new(session: Session) -> Self {
        Self {
            session,
            log_view: LogViewState::new(),
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
            input_mode: InputMode::default(),
            blink_on: true, // Start with indicator visible
        }
    }

    /// Toggle the LIVE indicator blink state (FR-028).
    ///
    /// Should be called on timer events (every 500ms) when `input_mode` is Streaming.
    /// When not Streaming, blink state doesn't affect rendering.
    ///
    /// # Returns
    /// The new blink state after toggle.
    pub fn toggle_blink(&mut self) -> bool {
        self.blink_on = !self.blink_on;
        self.blink_on
    }

    /// Add multiple conversation entries (valid or malformed) to the session.
    ///
    /// This is the proper way for the shell layer to add entries
    /// without directly mutating the core session state.
    ///
    /// # Dual-Write Migration (cclv-5ur.6.1)
    /// During migration, this writes to BOTH:
    /// - `session` (existing, for compatibility)
    /// - `log_view` (new, becomes source of truth)
    ///
    /// Subsequent tasks will migrate call sites from session to log_view.
    pub fn add_entries(&mut self, entries: Vec<crate::model::ConversationEntry>) {
        for entry in entries {
            // Dual-write: populate both session (old) and log_view (new)

            // Extract agent_id for routing to log_view
            let agent_id = match &entry {
                crate::model::ConversationEntry::Valid(log_entry) => {
                    log_entry.agent_id().cloned()
                }
                crate::model::ConversationEntry::Malformed(_) => None,
            };

            // Write to log_view (new path - source of truth)
            self.log_view.add_entry(entry.clone(), agent_id);

            // Write to session (old path - compatibility during migration)
            self.session.add_conversation_entry(entry);
        }
    }

    /// Get immutable reference to the session.
    pub fn session(&self) -> &Session {
        &self.session
    }

    /// Get immutable reference to log_view (view-state layer).
    pub fn log_view(&self) -> &LogViewState {
        &self.log_view
    }

    /// Get mutable reference to log_view (view-state layer).
    pub fn log_view_mut(&mut self) -> &mut LogViewState {
        &mut self.log_view
    }

    /// Get main conversation view-state (for rendering).
    ///
    /// Assumes single session (current limitation).
    /// Returns None if no sessions exist (shouldn't happen in normal operation).
    pub fn main_conversation_view(&self) -> Option<&crate::view_state::conversation::ConversationViewState> {
        self.log_view.get_session(0).map(|s| s.main())
    }

    /// Get mutable main conversation view-state.
    pub fn main_conversation_view_mut(&mut self) -> Option<&mut crate::view_state::conversation::ConversationViewState> {
        self.log_view.get_session_mut(0).map(|s| s.main_mut())
    }

    /// Get subagent conversation view-state by tab index.
    ///
    /// Returns None if tab_index is out of range or session doesn't exist.
    pub fn subagent_conversation_view(&mut self, tab_index: usize) -> Option<&crate::view_state::conversation::ConversationViewState> {
        let session = self.log_view.get_session_mut(0)?;
        let agent_ids: Vec<_> = self.session.subagents().keys().cloned().collect();
        let agent_id = agent_ids.get(tab_index)?;
        Some(session.subagent(agent_id))
    }

    /// Populate log_view from existing session entries (test helper).
    ///
    /// This is needed for tests that build Session first, then create AppState.
    /// In production, entries arrive via add_entries() which does dual-write.
    /// In tests, Session is already populated, so we need to sync log_view.
    ///
    /// NOTE: This only writes to log_view, NOT to session (no duplication).
    ///
    /// # Warning
    /// This is a test-only helper. Do not use in production code.
    #[doc(hidden)]
    pub fn populate_log_view_from_session(&mut self) {
        // Main agent entries
        let main_entries: Vec<_> = self.session.main_agent().entries().to_vec();
        for entry in main_entries {
            self.log_view.add_entry(entry, None);
        }

        // Subagent entries
        for (agent_id, conversation) in self.session.subagents() {
            let entries: Vec<_> = conversation.entries().to_vec();
            for entry in entries {
                self.log_view.add_entry(entry, Some(agent_id.clone()));
            }
        }
    }

    /// Check if new messages indicator should be shown.
    /// Returns true when live_mode is active but auto_scroll is paused.
    /// This signals to the UI that new content has arrived below the current view.
    pub fn has_new_messages_indicator(&self) -> bool {
        self.live_mode && !self.auto_scroll
    }

    /// Cycle focus between Main, Subagent, and Stats.
    /// Skip Search in the cycle.
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
/// Tracks message expansion state and per-message wrap overrides
/// for a single conversation pane (either main agent or a subagent).
///
/// NOTE: Scroll position is now managed by ConversationViewState via ScrollPosition.
/// The vertical_offset field is maintained via dual-write for view layer compatibility.
///
/// # Invariants
///
/// - `vertical_offset ≥ 0` (managed by scroll_handler via ScrollPosition.resolve())
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
/// Expansion state is managed per-entry via ConversationViewState (EntryView.expanded field).
///
/// # Wrap Overrides (FR-048, FR-049)
///
/// Individual messages can override the global wrap mode. This is ephemeral state
/// (not persisted across sessions). Per-item toggle inverts the global setting:
/// - Global=Wrap, Override present → NoWrap for this message
/// - Global=NoWrap, Override present → Wrap for this message
#[derive(Debug, Clone, Default)]
pub struct ScrollState {
    /// Vertical scroll offset in lines (absolute line offset from top of content).
    ///
    /// **COMPATIBILITY**: This field is written by scroll_handler via dual-write pattern
    /// and read by view/message.rs during rendering. The semantic scroll position is
    /// managed by ConversationViewState.scroll() using ScrollPosition enum.
    ///
    /// **TODO(cclv-5ur.6.9)**: Remove when view/message.rs migrated to use
    /// ConversationViewState.visible_range() instead of reading this field directly.
    pub vertical_offset: usize,

    /// Horizontal scroll offset (number of characters scrolled right from left edge).
    /// Only relevant when line wrapping is disabled (FR-040).
    /// 0 means viewing from the leftmost column.
    pub horizontal_offset: usize,


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
    // ===== Active methods (horizontal scrolling) =====

    /// Scroll left by amount, saturating at 0.
    pub fn scroll_left(&mut self, amount: usize) {
        self.horizontal_offset = self.horizontal_offset.saturating_sub(amount);
    }

    /// Scroll right by amount.
    pub fn scroll_right(&mut self, amount: usize) {
        self.horizontal_offset = self.horizontal_offset.saturating_add(amount);
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
// Tests removed during expand state migration to view-state layer