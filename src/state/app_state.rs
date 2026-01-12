//! Application state and transitions.
//!
//! AppState is the root state type containing all UI state.
//! All state transitions are pure functions following Elm architecture.

use crate::model::{AgentId, SessionId, StatsFilter};
use crate::state::SearchState;
use crate::view_state::log::LogViewState;

// ===== ConversationSelection =====

/// Which conversation is currently selected in the unified tab model.
///
/// This type replaces `Option<usize>` to provide type-safe, stable selection
/// that survives subagent additions/removals.
///
/// # Cardinality (cclv-5ur.53)
/// - Option<usize>: 2^64 + 1 states (many invalid)
/// - ConversationSelection: 1 + N states (all valid)
///
/// # Stability
/// AgentId-based selection is stable across subagent list changes,
/// while index-based selection breaks when subagents are added/removed.
///
/// # Design Principles
/// - I: Type-Driven (sum type makes intent clear)
/// - VII: Cardinality (minimize invalid states)
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ConversationSelection {
    /// Main agent conversation selected.
    #[default]
    Main,

    /// Subagent conversation selected by identity.
    /// Identity-based selection is stable across subagent additions/removals.
    Subagent(AgentId),
}

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
    /// View-state layer for rendering log entries.
    /// Source of truth for entry layout and display.
    /// Migration complete: session field removed, view-state is primary.
    log_view: LogViewState,

    /// Which pane currently has keyboard focus.
    /// Determines which pane receives keyboard input and displays focus indicator.
    /// See `FocusPane` for valid states and transitions.
    pub focus: FocusPane,

    /// Currently selected conversation in unified tab model (FR-086, cclv-5ur.53).
    /// Stable identity-based selection using AgentId.
    pub selected_conversation: ConversationSelection,

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

    /// Scroll offset for the help overlay (cclv-5ur.76).
    /// 0 means showing from the top, higher values scroll down.
    /// Only meaningful when help_visible is true.
    pub help_scroll_offset: u16,

    /// Whether the application is following a live log file.
    /// `true` means new entries are being tailed in real-time (FR-007).
    /// `false` means viewing a static/completed log file (FR-008).
    pub live_mode: bool,

    /// Whether auto-scroll is active (only meaningful when `live_mode` is true).
    /// `true` means automatically scroll to new content as it arrives (FR-035).
    /// `false` means user has scrolled away from bottom; pause auto-scroll (FR-036).
    pub auto_scroll: bool,

    /// Global line-wrapping mode for all panes.
    /// Individual messages can override via `EntryView.wrap_override` (FR-048).
    /// Default is `Wrap` when config is unset (FR-039).
    pub global_wrap: WrapMode,

    /// Input mode for LIVE indicator display (FR-042b).
    /// Indicates whether reading from static file, actively streaming, or EOF.
    pub input_mode: InputMode,

    /// Blink state for LIVE indicator animation (FR-028).
    /// Toggles on 500ms timer events when `input_mode` is Streaming.
    /// `true` means indicator is visible (green), `false` means hidden.
    pub blink_on: bool,

    /// Maximum context window size in tokens (cclv-5ur.32).
    /// Used for token divider percentage calculation.
    /// Default: 200,000 tokens (Claude Opus 4.5 context window).
    pub max_context_tokens: u64,

    /// Pricing configuration for cost estimation (cclv-5ur.32).
    /// Used by token divider to show estimated costs.
    pub pricing: crate::model::PricingConfig,

    /// Session list modal state.
    pub session_modal: crate::state::SessionModalState,

    /// Which session is currently being viewed.
    pub viewed_session: crate::state::ViewedSession,

    /// Per-session scroll positions (FR-010).
    /// Tracks scroll offset for each visited session.
    /// Key absence = unvisited (first visit shows top).
    /// Key presence = visited (return restores offset).
    pub session_scroll_states: crate::state::SessionScrollStates,
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

impl AppState {
    /// Create new AppState with default UI state.
    pub fn new() -> Self {
        Self {
            log_view: LogViewState::new(),
            focus: FocusPane::Main,
            selected_conversation: ConversationSelection::Main, // FR-083: Default to main agent
            search: SearchState::Inactive,
            stats_filter: StatsFilter::AllSessionsCombined, // TODO: Should be session-aware
            stats_visible: false,
            help_visible: false,
            help_scroll_offset: 0,
            live_mode: false,
            auto_scroll: true,
            global_wrap: WrapMode::default(),
            input_mode: InputMode::default(),
            blink_on: true, // Start with indicator visible
            max_context_tokens: 200_000,
            pricing: crate::model::PricingConfig::default(),
            session_modal: crate::state::SessionModalState::new(),
            viewed_session: crate::state::ViewedSession::default(), // ViewedSession::Latest
            session_scroll_states: crate::state::SessionScrollStates::new(),
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

    /// Compute tab index from conversation selection for rendering (cclv-5ur.53).
    ///
    /// Maps identity-based selection to positional index:
    /// - Main -> 0
    /// - Subagent(agent_id) -> position in sorted subagent list + 1
    ///
    /// Returns None if the selected subagent doesn't exist in viewed session.
    ///
    /// # Viewed Session (cclv-463.3.6)
    ///
    /// Uses `viewed_session` to determine which session to display.
    /// This ensures correct tab index computation when viewing historical sessions.
    pub fn selected_tab_index(&self) -> Option<usize> {
        match &self.selected_conversation {
            ConversationSelection::Main => Some(0),
            ConversationSelection::Subagent(agent_id) => {
                // Use viewed_session to determine which session to display
                let session_count = self.log_view.session_count();
                let session_idx = self.viewed_session.effective_index(session_count)?;
                let session = self.log_view.get_session(session_idx.get())?;

                // Find position in sorted subagent list
                let mut sorted_ids: Vec<_> = session.subagents().keys().collect();
                sorted_ids.sort();

                sorted_ids
                    .iter()
                    .position(|id| *id == agent_id)
                    .map(|pos| pos + 1) // +1 because main is tab 0
            }
        }
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
                crate::model::ConversationEntry::Valid(log_entry) => log_entry.agent_id().cloned(),
                crate::model::ConversationEntry::Malformed(_) => None,
            };

            // Write to log_view (source of truth)
            self.log_view.add_entry(entry, agent_id);
        }

        // Synchronize stats filter with current session after adding entries
        self.sync_stats_filter();
    }

    /// Get immutable reference to current session view-state.
    ///
    /// Uses `viewed_session` to determine which session to display.
    /// Panics if no sessions exist (shouldn't happen in normal operation).
    pub fn session_view(&self) -> &crate::view_state::session::SessionViewState {
        let session_count = self.log_view.session_count();
        let session_idx = self
            .viewed_session
            .effective_index(session_count)
            .expect("No session view-state - this is a bug");
        self.log_view
            .get_session(session_idx.get())
            .expect("No session view-state - this is a bug")
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
    /// Uses `viewed_session` to determine which session to display.
    /// Returns None if no sessions exist or viewed session is invalid.
    pub fn main_conversation_view(
        &self,
    ) -> Option<&crate::view_state::conversation::ConversationViewState> {
        let session_count = self.log_view.session_count();
        let session_idx = self.viewed_session.effective_index(session_count)?;
        self.log_view
            .get_session(session_idx.get())
            .map(|s| s.main())
    }

    /// Get mutable main conversation view-state.
    ///
    /// Uses `viewed_session` to determine which session to display.
    pub fn main_conversation_view_mut(
        &mut self,
    ) -> Option<&mut crate::view_state::conversation::ConversationViewState> {
        let session_count = self.log_view.session_count();
        let session_idx = self.viewed_session.effective_index(session_count)?;
        self.log_view
            .get_session_mut(session_idx.get())
            .map(|s| s.main_mut())
    }

    /// Get subagent conversation view-state by tab index.
    ///
    /// Uses `viewed_session` to determine which session to display.
    /// Returns None if tab_index is out of range or session doesn't exist.
    pub fn subagent_conversation_view(
        &mut self,
        tab_index: usize,
    ) -> Option<&crate::view_state::conversation::ConversationViewState> {
        let session_count = self.log_view.session_count();
        let session_idx = self.viewed_session.effective_index(session_count)?;
        let session = self.log_view.get_session_mut(session_idx.get())?;
        let mut agent_ids: Vec<_> = session.subagent_ids().cloned().collect();
        agent_ids.sort_by(|a, b| a.as_str().cmp(b.as_str()));
        let agent_id = agent_ids.get(tab_index)?;
        Some(session.subagent(agent_id))
    }

    /// Get mutable subagent conversation view-state by tab index.
    ///
    /// Uses `viewed_session` to determine which session to display.
    /// Returns None if tab_index is out of range or session doesn't exist.
    pub fn subagent_conversation_view_mut(
        &mut self,
        tab_index: usize,
    ) -> Option<&mut crate::view_state::conversation::ConversationViewState> {
        let session_count = self.log_view.session_count();
        let session_idx = self.viewed_session.effective_index(session_count)?;
        let session = self.log_view.get_session_mut(session_idx.get())?;
        let mut agent_ids: Vec<_> = session.subagent_ids().cloned().collect();
        agent_ids.sort_by(|a, b| a.as_str().cmp(b.as_str()));
        let agent_id = agent_ids.get(tab_index).cloned()?;
        Some(session.subagent_mut(&agent_id))
    }

    /// Get immutable reference to currently selected conversation view-state.
    ///
    /// Routes to the appropriate conversation based on selected_conversation:
    /// - Main: Main agent conversation
    /// - Subagent(agent_id): Specific subagent by identity
    ///
    /// Returns None if no session exists or selected subagent doesn't exist.
    ///
    /// # Routing Logic
    ///
    /// This encapsulates the conversation routing logic used throughout the application.
    /// The routing matches the rendering logic to ensure input handlers and display stay synchronized.
    ///
    /// # Viewed Session (cclv-463.3.6)
    ///
    /// Uses `viewed_session` to determine which session to display.
    /// This ensures that when user selects a historical session from the session modal,
    /// the rendering shows that session, not always the latest session.
    pub fn selected_conversation_view(
        &self,
    ) -> Option<&crate::view_state::conversation::ConversationViewState> {
        // Use viewed_session to determine which session to display
        let session_count = self.log_view.session_count();
        let session_idx = self.viewed_session.effective_index(session_count)?;
        let session = self.log_view.get_session(session_idx.get())?;

        match &self.selected_conversation {
            ConversationSelection::Main => Some(session.main()),
            ConversationSelection::Subagent(agent_id) => {
                // Use get_subagent to avoid creating subagent if it doesn't exist
                session.get_subagent(agent_id)
            }
        }
    }

    /// Get mutable reference to currently selected conversation view-state.
    ///
    /// Routes to the appropriate conversation based on selected_conversation:
    /// - Main: Main agent conversation
    /// - Subagent(agent_id): Specific subagent by identity
    ///
    /// Returns None if no session exists or selected subagent doesn't exist.
    ///
    /// # Routing Logic
    ///
    /// This encapsulates the conversation routing logic used throughout the application.
    /// The routing matches the rendering logic to ensure input handlers and display stay synchronized.
    ///
    /// # Semantics
    ///
    /// This method does NOT create subagents that don't exist. Both the immutable
    /// and mutable accessors have identical semantics: they return None when a
    /// selected subagent doesn't exist. This prevents creating ghost subagents
    /// that have no actual conversation data.
    ///
    /// # Viewed Session (cclv-463.3.6)
    ///
    /// Uses `viewed_session` to determine which session to display.
    /// This ensures that when user selects a historical session from the session modal,
    /// the rendering shows that session, not always the latest session.
    pub fn selected_conversation_view_mut(
        &mut self,
    ) -> Option<&mut crate::view_state::conversation::ConversationViewState> {
        // Use viewed_session to determine which session to display
        let session_count = self.log_view.session_count();
        let session_idx = self.viewed_session.effective_index(session_count)?;
        let session = self.log_view.get_session_mut(session_idx.get())?;

        match &self.selected_conversation {
            ConversationSelection::Main => Some(session.main_mut()),
            ConversationSelection::Subagent(agent_id) => {
                // Use get_subagent_mut to avoid creating subagent if it doesn't exist
                session.get_subagent_mut(agent_id)
            }
        }
    }

    /// Get AgentId of currently selected conversation.
    ///
    /// Returns:
    /// - None if Main is selected (main agent has no AgentId)
    /// - Some(AgentId) if Subagent is selected
    ///
    /// # Routing Logic
    ///
    /// This directly extracts the AgentId from the ConversationSelection.
    /// Main agent has no AgentId, while subagents have explicit identity.
    pub fn selected_agent_id(&self) -> Option<AgentId> {
        match &self.selected_conversation {
            ConversationSelection::Main => None,
            ConversationSelection::Subagent(agent_id) => Some(agent_id.clone()),
        }
    }

    /// Check if new messages indicator should be shown.
    /// Returns true when live_mode is active but auto_scroll is paused.
    /// This signals to the UI that new content has arrived below the current view.
    pub fn has_new_messages_indicator(&self) -> bool {
        self.live_mode && !self.auto_scroll
    }

    /// Synchronize stats_filter with selected_conversation.
    ///
    /// Ensures that the statistics panel displays stats for the currently
    /// focused conversation. Called after any operation that changes
    /// selected_conversation (tab switching, session changes).
    ///
    /// Maps ConversationSelection to StatsFilter:
    /// - Main -> MainAgent(current_session)
    /// - Subagent(id) -> Subagent(id)
    fn sync_stats_filter(&mut self) {
        self.stats_filter = match &self.selected_conversation {
            ConversationSelection::Main => {
                // Get the currently viewed session's ID
                let session_count = self.log_view.session_count();
                if let Some(session_idx) = self.viewed_session.effective_index(session_count) {
                    if let Some(session) = self.log_view.get_session(session_idx.get()) {
                        StatsFilter::MainAgent(session.session_id().clone())
                    } else {
                        // Fallback if session doesn't exist (shouldn't happen in normal operation)
                        StatsFilter::AllSessionsCombined
                    }
                } else {
                    // Fallback if no valid session index
                    StatsFilter::AllSessionsCombined
                }
            }
            ConversationSelection::Subagent(id) => StatsFilter::Subagent(id.clone()),
        };
    }

    /// Move to next tab (unified tab model, FR-086, cclv-5ur.53).
    /// Works for all conversations (main agent + subagents).
    /// Wraps from last to first (main).
    /// No-op when Search modal is active.
    pub fn next_tab(&mut self) {
        // No-op when Search modal is active
        if !matches!(self.search, SearchState::Inactive) {
            return;
        }

        // Get viewed session's sorted subagent list
        let session_count = self.log_view.session_count();
        let Some(session_idx) = self.viewed_session.effective_index(session_count) else {
            return;
        };
        let Some(session) = self.log_view.get_session(session_idx.get()) else {
            return;
        };

        let mut sorted_ids: Vec<_> = session.subagents().keys().cloned().collect();
        sorted_ids.sort();

        match &self.selected_conversation {
            ConversationSelection::Main => {
                // Main -> first subagent (or wrap to main if no subagents)
                if let Some(first_id) = sorted_ids.first() {
                    self.selected_conversation = ConversationSelection::Subagent(first_id.clone());
                }
            }
            ConversationSelection::Subagent(current_id) => {
                // Find current position
                if let Some(pos) = sorted_ids.iter().position(|id| id == current_id) {
                    if pos + 1 < sorted_ids.len() {
                        // Move to next subagent
                        self.selected_conversation =
                            ConversationSelection::Subagent(sorted_ids[pos + 1].clone());
                    } else {
                        // Last subagent -> wrap to main
                        self.selected_conversation = ConversationSelection::Main;
                    }
                } else {
                    // Current subagent no longer exists, go to main
                    self.selected_conversation = ConversationSelection::Main;
                }
            }
        }

        // Sync stats filter to match new conversation selection
        self.sync_stats_filter();
    }

    /// Move to previous tab (unified tab model, FR-086, cclv-5ur.53).
    /// Works for all conversations (main agent + subagents).
    /// Wraps from first (main) to last.
    /// No-op when Search modal is active.
    pub fn prev_tab(&mut self) {
        // No-op when Search modal is active
        if !matches!(self.search, SearchState::Inactive) {
            return;
        }

        // Get viewed session's sorted subagent list
        let session_count = self.log_view.session_count();
        let Some(session_idx) = self.viewed_session.effective_index(session_count) else {
            return;
        };
        let Some(session) = self.log_view.get_session(session_idx.get()) else {
            return;
        };

        let mut sorted_ids: Vec<_> = session.subagents().keys().cloned().collect();
        sorted_ids.sort();

        match &self.selected_conversation {
            ConversationSelection::Main => {
                // Main -> last subagent (or stay at main if no subagents)
                if let Some(last_id) = sorted_ids.last() {
                    self.selected_conversation = ConversationSelection::Subagent(last_id.clone());
                }
            }
            ConversationSelection::Subagent(current_id) => {
                // Find current position
                if let Some(pos) = sorted_ids.iter().position(|id| id == current_id) {
                    if pos > 0 {
                        // Move to previous subagent
                        self.selected_conversation =
                            ConversationSelection::Subagent(sorted_ids[pos - 1].clone());
                    } else {
                        // First subagent -> wrap to main
                        self.selected_conversation = ConversationSelection::Main;
                    }
                } else {
                    // Current subagent no longer exists, go to main
                    self.selected_conversation = ConversationSelection::Main;
                }
            }
        }

        // Sync stats filter to match new conversation selection
        self.sync_stats_filter();
    }

    /// Select a specific tab by 1-indexed number (unified tab model, FR-086, cclv-5ur.53).
    /// Works for all conversations: tab 1 = main (index 0), tab 2+ = subagents.
    /// Clamps to last tab if number is too high.
    /// Ignores if number is 0.
    /// No-op when Search modal is active.
    pub fn select_tab(&mut self, tab_number: usize) {
        // Ignore 0 (invalid 1-indexed input)
        if tab_number == 0 {
            return;
        }

        // No-op when Search modal is active
        if !matches!(self.search, SearchState::Inactive) {
            return;
        }

        // Convert to 0-indexed
        let index = tab_number - 1;

        if index == 0 {
            // Tab 1 = Main
            self.selected_conversation = ConversationSelection::Main;
        } else {
            // Tab 2+ = Subagent
            // Get viewed session's sorted subagent list
            let session_count = self.log_view.session_count();
            if let Some(session_idx) = self.viewed_session.effective_index(session_count) {
                if let Some(session) = self.log_view.get_session(session_idx.get()) {
                    let mut sorted_ids: Vec<_> = session.subagents().keys().cloned().collect();
                    sorted_ids.sort();

                    // Clamp to last subagent if index too high
                    let subagent_index = (index - 1).min(sorted_ids.len().saturating_sub(1));

                    if let Some(agent_id) = sorted_ids.get(subagent_index) {
                        self.selected_conversation =
                            ConversationSelection::Subagent(agent_id.clone());
                    }
                }
            }
        }

        // Sync stats filter to match new conversation selection
        self.sync_stats_filter();
    }

    /// Toggle global wrap mode (FR-050: W key)
    pub fn toggle_global_wrap(&mut self) {
        self.global_wrap = match self.global_wrap {
            WrapMode::Wrap => WrapMode::NoWrap,
            WrapMode::NoWrap => WrapMode::Wrap,
        };
    }

    /// Check if live tailing should be active (cclv-463.4.1).
    ///
    /// Live tailing is enabled when BOTH conditions are met:
    /// - `auto_scroll` is true (user has not scrolled away from bottom)
    /// - Currently viewing the last (most recent) session
    ///
    /// # Functional Requirements
    ///
    /// - **FR-006**: System MUST disable live tailing when viewing any session
    ///   other than the last (most recent) session
    /// - **FR-007**: System MUST re-enable live tailing when user navigates
    ///   back to last session
    ///
    /// # Returns
    ///
    /// `true` if live tailing should be active, `false` otherwise.
    ///
    /// # Examples
    ///
    /// ```
    /// # use cclv::state::AppState;
    /// let mut state = AppState::new();
    ///
    /// // Default: auto_scroll=true, viewing latest session
    /// assert!(state.is_tailing_enabled());
    ///
    /// // Disable auto_scroll
    /// state.auto_scroll = false;
    /// assert!(!state.is_tailing_enabled());
    /// ```
    pub fn is_tailing_enabled(&self) -> bool {
        self.auto_scroll && self.viewed_session.is_last(self.log_view.session_count())
    }

    /// Cycle through stats filter levels with session context (cclv-463.5.5).
    ///
    /// Implements the cycle order from contracts/stats-filter.md:
    /// AllSessionsCombined → Session(current) → MainAgent(current) → Subagent(first) → ... → Subagent(last) → AllSessionsCombined
    ///
    /// Uses the currently viewed session for session-scoped filters.
    pub fn cycle_stats_filter(&mut self) {
        // Get the currently viewed session ID for session-scoped filters
        let session_count = self.log_view.session_count();
        let current_session_idx = self.viewed_session.effective_index(session_count);

        let current_session =
            current_session_idx.and_then(|idx| self.log_view.get_session(idx.get()));

        // Get sorted list of subagent IDs from current session
        let subagent_ids: Vec<AgentId> = current_session
            .map(|s| {
                let mut ids: Vec<_> = s.subagent_ids().cloned().collect();
                ids.sort();
                ids
            })
            .unwrap_or_default();

        // Get current session ID (needed for session-scoped filters)
        let session_id = current_session.map(|s| s.session_id().clone());

        // Cycle according to the specification
        self.stats_filter = match &self.stats_filter {
            StatsFilter::AllSessionsCombined => {
                // AllSessionsCombined → Session(current)
                if let Some(id) = session_id {
                    StatsFilter::Session(id)
                } else {
                    // No sessions exist, stay at AllSessionsCombined
                    StatsFilter::AllSessionsCombined
                }
            }
            StatsFilter::Session(_) => {
                // Session → MainAgent(current)
                if let Some(id) = session_id {
                    StatsFilter::MainAgent(id)
                } else {
                    // No sessions exist, wrap to AllSessionsCombined
                    StatsFilter::AllSessionsCombined
                }
            }
            StatsFilter::MainAgent(_) => {
                // MainAgent → Subagent(first) or AllSessionsCombined if no subagents
                if let Some(first) = subagent_ids.first() {
                    StatsFilter::Subagent(first.clone())
                } else {
                    StatsFilter::AllSessionsCombined
                }
            }
            StatsFilter::Subagent(agent_id) => {
                // Subagent → next Subagent or AllSessionsCombined
                let idx = subagent_ids.iter().position(|id| id == agent_id);
                match idx {
                    Some(i) if i + 1 < subagent_ids.len() => {
                        StatsFilter::Subagent(subagent_ids[i + 1].clone())
                    }
                    _ => StatsFilter::AllSessionsCombined,
                }
            }
        };
    }

    /// Update stats filter when session changes (cclv-463.5.5).
    ///
    /// Called when user selects a different session from the session modal.
    /// Updates session-scoped filters to use the new session ID.
    ///
    /// # Behavior
    ///
    /// - AllSessionsCombined: unchanged
    /// - Session(old) → Session(new)
    /// - MainAgent(old) → MainAgent(new)
    /// - Subagent(id) → unchanged (identity-based, not session-scoped)
    pub fn on_session_change(&mut self, new_session_id: SessionId) {
        // Update session-scoped filters to use new session
        self.stats_filter = match &self.stats_filter {
            StatsFilter::AllSessionsCombined => StatsFilter::AllSessionsCombined,
            StatsFilter::Session(_) => StatsFilter::Session(new_session_id),
            StatsFilter::MainAgent(_) => StatsFilter::MainAgent(new_session_id),
            StatsFilter::Subagent(id) => StatsFilter::Subagent(id.clone()),
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
/// - **Per-item override**: Individual messages can override via `EntryView.wrap_override` (FR-048)
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum WrapMode {
    /// Wrap long lines to fit viewport width.
    /// Prose flows naturally; wrapped lines show continuation indicator (↩) (FR-052).
    #[default]
    Wrap,

    /// Do not wrap lines; use horizontal scrolling instead.
    /// User must scroll left/right to view content beyond viewport (FR-040).
    NoWrap,
}

/// Wrap context captures both the wrap mode and whether it's from an explicit per-entry override.
///
/// This type allows render logic to distinguish between:
/// - Global wrap mode (default behavior for content type)
/// - Explicit per-entry override (user explicitly chose to wrap/nowrap this entry)
///
/// Use case: ToolUse and ToolResult blocks default to NoWrap (structured data)
/// UNLESS the user explicitly overrides to Wrap for that specific entry.
///
/// # Design (cclv-5ur.22)
/// ToolUse and ToolResult contain structured data (JSON, file contents).
/// Default should be NoWrap to preserve structure visibility.
/// If user explicitly wants wrapping for a specific entry, per-entry override takes precedence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WrapContext {
    /// The effective wrap mode (either from override or global fallback).
    pub mode: WrapMode,
    /// True if this mode came from an explicit per-entry override.
    /// False if this is the global default.
    pub is_explicit_override: bool,
}

impl WrapContext {
    /// Create context from explicit per-entry override.
    pub fn from_override(mode: WrapMode) -> Self {
        Self {
            mode,
            is_explicit_override: true,
        }
    }

    /// Create context from global fallback (no per-entry override).
    pub fn from_global(mode: WrapMode) -> Self {
        Self {
            mode,
            is_explicit_override: false,
        }
    }
}

// ===== Tests =====
// Tests removed during expand state migration to view-state layer

// TEMPORARILY DISABLED during cclv-5ur.53 refactor
// #[cfg(test)]
// #[path = "app_state_unified_tabs_test.rs"]
// mod unified_tabs_test;

#[cfg(test)]
#[path = "app_state_routing_test.rs"]
mod routing_test;

#[cfg(test)]
#[path = "app_state_conversation_selection_test.rs"]
mod conversation_selection_test;

#[cfg(test)]
#[path = "app_state_session_navigation_wiring_test.rs"]
mod session_navigation_wiring_test;

#[cfg(test)]
#[path = "app_state_stats_filter_cycle_test.rs"]
mod app_state_stats_filter_cycle_test;
