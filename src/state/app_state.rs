//! Application state and transitions.
//!
//! AppState is the root state type containing all UI state.
//! All state transitions are pure functions following Elm architecture.

use crate::model::{AgentId, StatsFilter};
use crate::state::SearchState;
use crate::view_state::log::LogViewState;

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

    /// Currently selected tab index in unified tab model (FR-086).
    /// - `Some(0)` = main agent tab
    /// - `Some(1)` = first subagent (alphabetically sorted)
    /// - `Some(n)` = (n-1)th subagent
    /// - `None` = no tab selected (initial state before first render)
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
            selected_tab: Some(0), // FR-083: Default to main agent tab (tab 0)
            search: SearchState::Inactive,
            stats_filter: StatsFilter::Global,
            stats_visible: false,
            help_visible: false,
            live_mode: false,
            auto_scroll: true,
            global_wrap: WrapMode::default(),
            input_mode: InputMode::default(),
            blink_on: true, // Start with indicator visible
            max_context_tokens: 200_000,
            pricing: crate::model::PricingConfig::default(),
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
                crate::model::ConversationEntry::Valid(log_entry) => log_entry.agent_id().cloned(),
                crate::model::ConversationEntry::Malformed(_) => None,
            };

            // Write to log_view (source of truth)
            self.log_view.add_entry(entry, agent_id);
        }
    }

    /// Get immutable reference to current session view-state.
    ///
    /// Returns the last/active session in the log.
    /// Panics if no sessions exist (shouldn't happen in normal operation).
    pub fn session_view(&self) -> &crate::view_state::session::SessionViewState {
        self.log_view
            .current_session()
            .expect("No session view-state - this is a bug")
    }

    /// Get mutable reference to current session view-state.
    pub fn session_view_mut(&mut self) -> &mut crate::view_state::session::SessionViewState {
        self.log_view
            .current_session_mut()
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
    /// Assumes single session (current limitation).
    /// Returns None if no sessions exist (shouldn't happen in normal operation).
    pub fn main_conversation_view(
        &self,
    ) -> Option<&crate::view_state::conversation::ConversationViewState> {
        self.log_view.current_session().map(|s| s.main())
    }

    /// Get mutable main conversation view-state.
    pub fn main_conversation_view_mut(
        &mut self,
    ) -> Option<&mut crate::view_state::conversation::ConversationViewState> {
        self.log_view.current_session_mut().map(|s| s.main_mut())
    }

    /// Get subagent conversation view-state by tab index.
    ///
    /// Returns None if tab_index is out of range or session doesn't exist.
    pub fn subagent_conversation_view(
        &mut self,
        tab_index: usize,
    ) -> Option<&crate::view_state::conversation::ConversationViewState> {
        let session = self.log_view.get_session_mut(0)?;
        let mut agent_ids: Vec<_> = session.subagent_ids().cloned().collect();
        agent_ids.sort_by(|a, b| a.as_str().cmp(b.as_str()));
        let agent_id = agent_ids.get(tab_index)?;
        Some(session.subagent(agent_id))
    }

    /// Get mutable subagent conversation view-state by tab index.
    ///
    /// Returns None if tab_index is out of range or session doesn't exist.
    pub fn subagent_conversation_view_mut(
        &mut self,
        tab_index: usize,
    ) -> Option<&mut crate::view_state::conversation::ConversationViewState> {
        let session = self.log_view.get_session_mut(0)?;
        let mut agent_ids: Vec<_> = session.subagent_ids().cloned().collect();
        agent_ids.sort_by(|a, b| a.as_str().cmp(b.as_str()));
        let agent_id = agent_ids.get(tab_index).cloned()?;
        Some(session.subagent_mut(&agent_id))
    }

    /// Get immutable reference to currently selected conversation view-state.
    ///
    /// Routes to the appropriate conversation based on selected_tab:
    /// - Tab 0: Main agent conversation
    /// - Tab 1+: Subagent at sorted index (tab - 1)
    ///
    /// Returns None if no session exists or selected tab is out of range.
    ///
    /// # Routing Logic
    ///
    /// This encapsulates the tab→conversation routing logic used throughout the application.
    /// The routing matches the rendering logic to ensure input handlers and display stay synchronized.
    pub fn selected_conversation_view(
        &self,
    ) -> Option<&crate::view_state::conversation::ConversationViewState> {
        let tab_index = self.selected_tab.unwrap_or(0);

        if tab_index == 0 {
            // Tab 0: Main agent conversation
            self.log_view.current_session().map(|s| s.main())
        } else {
            // Tab 1+: Subagent at sorted index (tab - 1)
            let session = self.log_view.current_session()?;
            let mut agent_ids: Vec<_> = session.subagent_ids().cloned().collect();
            agent_ids.sort_by(|a, b| a.as_str().cmp(b.as_str()));
            let agent_id = agent_ids.get(tab_index - 1)?;
            session.subagents().get(agent_id)
        }
    }

    /// Get mutable reference to currently selected conversation view-state.
    ///
    /// Routes to the appropriate conversation based on selected_tab:
    /// - Tab 0: Main agent conversation
    /// - Tab 1+: Subagent at sorted index (tab - 1)
    ///
    /// Returns None if no session exists or selected tab is out of range.
    ///
    /// # Routing Logic
    ///
    /// This encapsulates the tab→conversation routing logic used throughout the application.
    /// The routing matches the rendering logic to ensure input handlers and display stay synchronized.
    pub fn selected_conversation_view_mut(
        &mut self,
    ) -> Option<&mut crate::view_state::conversation::ConversationViewState> {
        let tab_index = self.selected_tab.unwrap_or(0);

        if tab_index == 0 {
            // Tab 0: Main agent conversation
            self.log_view.current_session_mut().map(|s| s.main_mut())
        } else {
            // Tab 1+: Subagent at sorted index (tab - 1)
            let session = self.log_view.current_session_mut()?;
            let mut agent_ids: Vec<_> = session.subagent_ids().cloned().collect();
            agent_ids.sort_by(|a, b| a.as_str().cmp(b.as_str()));
            let agent_id = agent_ids.get(tab_index - 1).cloned()?;
            Some(session.subagent_mut(&agent_id))
        }
    }

    /// Get AgentId of currently selected tab.
    ///
    /// Returns:
    /// - None if tab 0 is selected (main agent has no AgentId)
    /// - Some(AgentId) if tab 1+ is selected (subagent)
    /// - None if selected_tab is invalid or no session exists
    ///
    /// # Routing Logic
    ///
    /// This encapsulates the tab→agent routing logic. Main agent (tab 0) has no AgentId,
    /// while subagents (tab 1+) map to AgentId at sorted index (tab - 1).
    pub fn selected_agent_id(&self) -> Option<AgentId> {
        let tab_index = self.selected_tab.unwrap_or(0);

        if tab_index == 0 {
            // Tab 0: Main agent has no AgentId
            None
        } else {
            // Tab 1+: Get AgentId at sorted index (tab - 1)
            let session = self.log_view.current_session()?;
            let mut agent_ids: Vec<_> = session.subagent_ids().cloned().collect();
            agent_ids.sort_by(|a, b| a.as_str().cmp(b.as_str()));
            agent_ids.get(tab_index - 1).cloned()
        }
    }

    /// Find the tab index for a given agent_id.
    ///
    /// # Routing Logic
    ///
    /// Maps AgentId to its tab index using unified tab model (FR-086):
    /// - First subagent (alphabetically) -> tab 1
    /// - Second subagent -> tab 2
    /// - etc.
    ///
    /// Tab 0 is reserved for the main agent and is not included in this mapping.
    ///
    /// Returns None if the agent_id is not found in the current session's subagents.
    pub fn tab_index_for_agent(&self, agent_id: &AgentId) -> Option<usize> {
        let session = self.log_view.current_session()?;
        let mut agent_ids: Vec<_> = session.subagent_ids().cloned().collect();
        agent_ids.sort_by(|a, b| a.as_str().cmp(b.as_str()));
        agent_ids
            .iter()
            .enumerate()
            .find(|(_, aid)| **aid == *agent_id)
            .map(|(idx, _)| idx.saturating_add(1))
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
    ///
    /// Auto-selects appropriate tab when switching panes:
    /// - Switching to Subagent pane: selects tab 1 (first subagent) if subagents exist
    /// - Switching to Main pane: selects tab 0 (main agent)
    pub fn cycle_focus(&mut self) {
        self.focus = match self.focus {
            FocusPane::Main => FocusPane::Subagent,
            FocusPane::Subagent => FocusPane::Stats,
            FocusPane::Stats => FocusPane::Main,
            FocusPane::Search => FocusPane::Main,
        };

        // Auto-select appropriate tab for new pane (unified tab model FR-086)
        match self.focus {
            FocusPane::Subagent => {
                // Switching to Subagent pane: select first subagent tab (tab 1)
                if let Some(session) = self.log_view.current_session() {
                    if session.subagent_ids().next().is_some() {
                        self.selected_tab = Some(1); // First subagent is at tab 1
                    }
                }
            }
            FocusPane::Main => {
                // Switching to Main pane: select main agent tab (tab 0)
                self.selected_tab = Some(0);
            }
            _ => {
                // Stats and Search panes don't change tab selection
            }
        }
    }

    /// Set focus to Main pane.
    pub fn focus_main(&mut self) {
        self.focus = FocusPane::Main;
    }

    /// Set focus to Subagent pane.
    ///
    /// Auto-selects first subagent tab if no tab is currently selected
    /// and subagents exist.
    pub fn focus_subagent(&mut self) {
        self.focus = FocusPane::Subagent;

        // Auto-select first subagent tab if none selected
        if self.selected_tab.is_none() {
            if let Some(session) = self.log_view.current_session() {
                if session.subagent_ids().next().is_some() {
                    self.selected_tab = Some(0);
                }
            }
        }
    }

    /// Set focus to Stats pane.
    pub fn focus_stats(&mut self) {
        self.focus = FocusPane::Stats;
    }

    /// Move to next tab (unified tab model, FR-086).
    /// Works for all conversations (main agent + subagents).
    /// Wraps from last tab to first tab (tab 0 = main agent).
    /// No-op when Search modal is active.
    pub fn next_tab(&mut self) {
        // No-op when Search modal is active (FR-086)
        if self.focus == FocusPane::Search {
            return;
        }

        // Calculate total tabs: 1 (main) + num_subagents
        let num_subagents = self
            .log_view
            .current_session()
            .map(|s| s.subagent_ids().count())
            .unwrap_or(0);
        let total_tabs = 1 + num_subagents;

        // No-op if only one tab (main only, wrapping is meaningless)
        if total_tabs <= 1 {
            return;
        }

        self.selected_tab = match self.selected_tab {
            None => Some(0), // Initialize to first tab (main)
            Some(current) => {
                if current + 1 >= total_tabs {
                    Some(0) // Wrap to first (main agent)
                } else {
                    Some(current + 1) // Move to next
                }
            }
        };
    }

    /// Move to previous tab (unified tab model, FR-086).
    /// Works for all conversations (main agent + subagents).
    /// Wraps from first tab (main) to last tab.
    /// No-op when Search modal is active.
    pub fn prev_tab(&mut self) {
        // No-op when Search modal is active (FR-086)
        if self.focus == FocusPane::Search {
            return;
        }

        // Calculate total tabs: 1 (main) + num_subagents
        let num_subagents = self
            .log_view
            .current_session()
            .map(|s| s.subagent_ids().count())
            .unwrap_or(0);
        let total_tabs = 1 + num_subagents;

        // No-op if only one tab (main only, wrapping is meaningless)
        if total_tabs <= 1 {
            return;
        }

        self.selected_tab = match self.selected_tab {
            None => Some(0),                    // Initialize to first tab (main)
            Some(0) => Some(total_tabs - 1),    // Wrap from main to last tab
            Some(current) => Some(current - 1), // Move to previous
        };
    }

    /// Select a specific tab by 1-indexed number (unified tab model, FR-086).
    /// Works for all conversations: tab 1 = main (index 0), tab 2+ = subagents.
    /// Clamps to last tab if number is too high.
    /// Ignores if number is 0.
    /// No-op when Search modal is active.
    pub fn select_tab(&mut self, tab_number: usize) {
        // No-op when Search modal is active (FR-086)
        if self.focus == FocusPane::Search {
            return;
        }

        // Calculate total tabs: 1 (main) + num_subagents
        let num_subagents = self
            .log_view
            .current_session()
            .map(|s| s.subagent_ids().count())
            .unwrap_or(0);
        let total_tabs = 1 + num_subagents;

        // Ignore 0 (invalid 1-indexed input)
        if tab_number == 0 {
            return;
        }

        // Convert from 1-indexed to 0-indexed, clamping to last tab
        let zero_indexed = tab_number - 1;
        self.selected_tab = Some(zero_indexed.min(total_tabs - 1));
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

#[cfg(test)]
#[path = "app_state_unified_tabs_test.rs"]
mod unified_tabs_test;

#[cfg(test)]
#[path = "app_state_routing_test.rs"]
mod routing_test;
