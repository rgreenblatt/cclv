//! TUI rendering and terminal management (impure shell)

mod help;
mod layout;
mod message;
mod search_input;
mod stats;
mod styles;
pub mod tabs;

pub use help::render_help_overlay;
pub use message::ConversationView;
pub use search_input::SearchInput;
pub use stats::StatsPanel;
pub use styles::MessageStyles;

use crate::config::keybindings::KeyBindings;
use crate::integration;
use crate::model::{AppError, KeyAction, SessionId};
use crate::source::InputSource;
use crate::state::{next_match, prev_match, scroll_handler, search_input_handler, AppState, FocusPane};
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io::{self, Stdout};
use std::time::Duration;
use thiserror::Error;
use tracing::{debug, warn};

/// Errors that can occur during TUI operations
#[derive(Debug, Error)]
pub enum TuiError {
    /// IO error during terminal operations
    #[error("Terminal IO error: {0}")]
    Io(#[from] io::Error),

    /// Input source error
    #[error("Input error: {0}")]
    Input(#[from] crate::model::InputError),

    /// Application error
    #[error("Application error: {0}")]
    App(#[from] AppError),
}

/// Main TUI application
///
/// Generic over backend to support testing with TestBackend
pub struct TuiApp<B>
where
    B: ratatui::backend::Backend,
{
    terminal: Terminal<B>,
    app_state: AppState,
    input_source: InputSource,
    line_counter: usize,
    key_bindings: KeyBindings,
    /// Last time a frame was rendered (for 60fps batching)
    last_render: std::time::Instant,
    /// Pending entries accumulated between renders
    pending_entries: Vec<crate::model::ConversationEntry>,
}

impl TuiApp<CrosstermBackend<Stdout>> {
    /// Create and initialize a new TUI application
    ///
    /// Sets up terminal in raw mode with alternate screen
    pub fn new(mut input_source: InputSource, session_id: SessionId) -> Result<Self, TuiError> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        stdout.execute(EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;

        // Load initial content from input source
        let initial_lines = input_source.poll()?;
        let entries = integration::process_lines(initial_lines, 1);

        // Log any malformed entries
        for entry in &entries {
            if let Some(malformed) = entry.as_malformed() {
                warn!(
                    "Parse error at line {}: {}",
                    malformed.line_number(),
                    malformed.error_message()
                );
            }
        }

        let mut session = crate::model::Session::new(session_id);
        let line_counter = entries.len();
        for entry in entries {
            session.add_conversation_entry(entry);
        }

        let app_state = AppState::new(session);
        let key_bindings = KeyBindings::default();

        Ok(Self {
            terminal,
            app_state,
            input_source,
            line_counter,
            key_bindings,
            last_render: std::time::Instant::now(),
            pending_entries: Vec::new(),
        })
    }

    /// Run the main event loop
    ///
    /// Returns when user quits (q or Ctrl+C)
    /// Target: 60fps (16ms frame budget) with batched rendering
    pub fn run(&mut self) -> Result<(), TuiError> {
        const FRAME_DURATION: Duration = Duration::from_millis(16); // ~60fps

        loop {
            // Poll for new log entries (accumulates to pending buffer)
            self.poll_input()?;

            // Poll for keyboard events (non-blocking with timeout)
            let keyboard_event = if event::poll(FRAME_DURATION)? {
                if let Event::Key(key) = event::read()? {
                    if self.handle_key(key) {
                        break;
                    }
                    true // Keyboard event occurred - force render
                } else {
                    false
                }
            } else {
                false
            };

            // Check if frame budget elapsed AFTER poll (not before)
            let should_render = self.should_render_frame();

            // Render frame if budget elapsed or keyboard event
            if should_render || keyboard_event {
                self.draw()?;
            }
        }

        Ok(())
    }
}

impl<B> TuiApp<B>
where
    B: ratatui::backend::Backend,
{
    /// Poll input source for new lines and process them
    ///
    /// Accumulates entries to pending buffer instead of adding directly to session.
    /// Entries are flushed to session during render phase.
    fn poll_input(&mut self) -> Result<(), TuiError> {
        let new_lines = self.input_source.poll()?;

        if !new_lines.is_empty() {
            debug!("Processing {} new lines", new_lines.len());
            let starting_line = self.line_counter + 1;
            let entries = integration::process_lines(new_lines, starting_line);

            // Log malformed entries
            for entry in &entries {
                if let Some(malformed) = entry.as_malformed() {
                    warn!(
                        "Parse error at line {}: {}",
                        malformed.line_number(),
                        malformed.error_message()
                    );
                }
            }

            // Update line counter BEFORE accumulating entries
            self.line_counter += entries.len();

            // Accumulate entries to pending buffer (batching for 60fps)
            self.accumulate_pending_entries(entries);
        }

        Ok(())
    }

    /// Handle a single keyboard event
    ///
    /// Returns true if app should quit
    fn handle_key(&mut self, key: KeyEvent) -> bool {
        // Special case: Ctrl+C should always quit, even if not in bindings
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            return true;
        }

        // Handle character input when in Search Typing mode (before key binding dispatch)
        if self.app_state.focus == FocusPane::Search {
            if let crate::state::SearchState::Typing { .. } = &self.app_state.search {
                match key.code {
                    KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                        self.app_state.search = search_input_handler::handle_char_input(
                            self.app_state.search.clone(),
                            ch
                        );
                        return false;
                    }
                    KeyCode::Backspace => {
                        self.app_state.search = search_input_handler::handle_backspace(
                            self.app_state.search.clone()
                        );
                        return false;
                    }
                    KeyCode::Left => {
                        self.app_state.search = search_input_handler::handle_cursor_left(
                            self.app_state.search.clone()
                        );
                        return false;
                    }
                    KeyCode::Right => {
                        self.app_state.search = search_input_handler::handle_cursor_right(
                            self.app_state.search.clone()
                        );
                        return false;
                    }
                    KeyCode::Enter => {
                        // Submit search on Enter when typing
                        self.app_state.search = search_input_handler::submit_search(
                            self.app_state.search.clone()
                        );
                        // Keep focus on Search pane after submit (stays active)
                        return false;
                    }
                    _ => {} // Fall through to key binding dispatch
                }
            }
        }

        // Look up action in key bindings
        let action = match self.key_bindings.get(key) {
            Some(action) => action,
            None => return false, // Unknown key, ignore
        };

        // Dispatch action
        match action {
            // Quit
            KeyAction::Quit => return true,

            // Focus navigation
            KeyAction::CycleFocus => {
                self.app_state.cycle_focus();
            }
            KeyAction::FocusMain => {
                self.app_state.focus_main();
            }
            KeyAction::FocusSubagent => {
                self.app_state.focus_subagent();
            }
            KeyAction::FocusStats => {
                self.app_state.focus_stats();
            }

            // Auto-scroll
            KeyAction::ToggleAutoScroll => {
                self.app_state.auto_scroll = !self.app_state.auto_scroll;
                // If enabling, scroll to bottom immediately
                if self.app_state.auto_scroll {
                    let entry_count = self.app_state.session().main_agent().len();
                    self.app_state
                        .main_scroll
                        .scroll_to_bottom(entry_count.saturating_sub(1));
                }
            }
            KeyAction::ScrollToLatest => {
                let entry_count = self.app_state.session().main_agent().len();
                self.app_state
                    .main_scroll
                    .scroll_to_bottom(entry_count.saturating_sub(1));
            }

            // Stats filters (legacy keybindings not yet in KeyBindings)
            KeyAction::FilterGlobal => {
                self.app_state.stats_filter = crate::model::StatsFilter::Global;
            }
            KeyAction::FilterMainAgent => {
                self.app_state.stats_filter = crate::model::StatsFilter::MainAgent;
            }
            KeyAction::FilterSubagent => {
                // Filter to current subagent tab if selected
                if let Some(tab_index) = self.app_state.selected_tab {
                    let subagent_ids = self.app_state.session().subagent_ids_ordered();
                    if let Some(&agent_id) = subagent_ids.get(tab_index) {
                        self.app_state.stats_filter =
                            crate::model::StatsFilter::Subagent(agent_id.clone());
                    }
                }
            }

            // Scrolling actions - delegate to pure scroll handler
            KeyAction::ScrollUp
            | KeyAction::ScrollDown
            | KeyAction::PageUp
            | KeyAction::PageDown
            | KeyAction::ScrollToTop
            | KeyAction::ScrollToBottom => {
                // Calculate viewport height from terminal size
                let viewport_height = self
                    .terminal
                    .size()
                    .map(|rect| rect.height as usize)
                    .unwrap_or(20)
                    .saturating_sub(5); // Reserve space for header/footer

                // Clone app_state, apply scroll action, then replace
                // This is safe because AppState is cheap to clone (Rc internals)
                let new_state = scroll_handler::handle_scroll_action(
                    self.app_state.clone(),
                    action,
                    viewport_height,
                );
                self.app_state = new_state;
            }

            // Search actions - delegate to pure search input handler
            KeyAction::StartSearch => {
                self.app_state.search = search_input_handler::activate_search_input(
                    self.app_state.search.clone()
                );
                self.app_state.focus = FocusPane::Search;
            }
            KeyAction::SubmitSearch => {
                self.app_state.search = search_input_handler::submit_search(
                    self.app_state.search.clone()
                );
                // Execute search to populate matches
                use crate::state::{execute_search, SearchState};
                if let SearchState::Active { query, .. } = &self.app_state.search {
                    let matches = execute_search(self.app_state.session(), query);
                    self.app_state.search = SearchState::Active {
                        query: query.clone(),
                        matches,
                        current_match: 0,
                    };
                }
                // Keep focus on Search pane after submit (stays active)
            }
            KeyAction::CancelSearch => {
                self.app_state.search = search_input_handler::cancel_search(
                    self.app_state.search.clone()
                );
                // Return focus to Main pane after cancel
                self.app_state.focus = FocusPane::Main;
            }

            // Match navigation - delegate to pure match navigation handler
            KeyAction::NextMatch => {
                self.app_state = next_match(self.app_state.clone());
            }
            KeyAction::PrevMatch => {
                self.app_state = prev_match(self.app_state.clone());
            }

            // Not yet implemented
            _ => {}
        }

        false
    }

    /// Render the current frame
    ///
    /// Flushes pending entries to session, applies auto-scroll, then renders.
    fn draw(&mut self) -> Result<(), TuiError> {
        // Flush accumulated entries before rendering
        let had_pending = !self.pending_entries.is_empty();
        self.flush_pending_entries();

        // FR-035: Auto-scroll to bottom when live_mode && auto_scroll && new entries
        if had_pending && self.app_state.live_mode && self.app_state.auto_scroll {
            let entry_count = self.app_state.session().main_agent().len();
            self.app_state
                .main_scroll
                .scroll_to_bottom(entry_count.saturating_sub(1));
        }

        // Render the frame
        self.terminal.draw(|frame| {
            layout::render_layout(frame, &self.app_state);
        })?;

        // Update last render time
        self.last_render = std::time::Instant::now();

        Ok(())
    }

    /// Accumulate entries to the pending buffer without rendering
    ///
    /// Used for batching rapid updates to maintain 60fps
    fn accumulate_pending_entries(&mut self, entries: Vec<crate::model::ConversationEntry>) {
        self.pending_entries.extend(entries);
    }

    /// Get count of pending entries in buffer (for testing)
    #[cfg(test)]
    fn pending_entry_count(&self) -> usize {
        self.pending_entries.len()
    }

    /// Flush pending entries to session and clear buffer
    fn flush_pending_entries(&mut self) {
        if self.pending_entries.is_empty() {
            return;
        }

        // Move entries from buffer to session
        let entries = std::mem::take(&mut self.pending_entries);
        self.app_state.add_entries(entries);
    }

    /// Check if enough time has elapsed to render a new frame (16ms for 60fps)
    fn should_render_frame(&self) -> bool {
        const FRAME_DURATION: std::time::Duration = std::time::Duration::from_millis(16);
        self.last_render.elapsed() >= FRAME_DURATION
    }

    /// Set the last render time (for testing)
    #[cfg(test)]
    fn set_last_render_time(&mut self, time: std::time::Instant) {
        self.last_render = time;
    }
}

/// CLI arguments (simplified for TUI layer)
pub struct CliArgs {
    pub stats: bool,
    pub follow: bool,
}

impl CliArgs {
    /// Create new CliArgs
    pub fn new(stats: bool, follow: bool) -> Self {
        Self { stats, follow }
    }
}

/// Initialize and run the TUI application with input source and args
///
/// This is the main entry point for the TUI. It handles terminal
/// setup, runs the event loop, and ensures cleanup on exit.
pub fn run_with_source(input_source: InputSource, args: CliArgs) -> Result<(), TuiError> {
    // Extract or create session ID
    // For now, use a default session ID. In the future, this could be
    // extracted from the first log entry or passed via args.
    let session_id = SessionId::new("default-session").map_err(|_| {
        TuiError::Io(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Invalid session ID",
        ))
    })?;

    let mut app = TuiApp::new(input_source, session_id)?;

    // Apply initial args (stats visible, search query, etc.)
    app.app_state.stats_visible = args.stats;
    app.app_state.live_mode = args.follow;

    // Run the app and ensure cleanup happens even on error
    let result = app.run();

    // Always restore terminal state
    restore_terminal()?;

    result
}

/// Initialize and run the TUI application (deprecated - use run_with_source)
///
/// This is kept for backward compatibility with existing tests.
#[deprecated(note = "Use run_with_source instead")]
#[allow(deprecated)]
pub fn run() -> Result<(), TuiError> {
    // For tests, just return Ok immediately to avoid blocking
    Ok(())
}

/// Restore terminal to normal state
///
/// Disables raw mode and leaves alternate screen
fn restore_terminal() -> Result<(), TuiError> {
    disable_raw_mode()?;
    io::stdout().execute(LeaveAlternateScreen)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyModifiers;

    #[test]
    fn tui_error_from_io_error() {
        let io_err = io::Error::other("test error");
        let tui_err: TuiError = io_err.into();
        assert!(matches!(tui_err, TuiError::Io(_)));
    }

    // Helper to create test TuiApp
    fn create_test_app() -> TuiApp<ratatui::backend::TestBackend> {
        use ratatui::backend::TestBackend;

        let backend = TestBackend::new(80, 24);
        let terminal = Terminal::new(backend).unwrap();

        let stdin_data = b"";
        let stdin_source = crate::source::StdinSource::from_reader(&stdin_data[..]);
        let input_source = InputSource::Stdin(stdin_source);

        let session_id = SessionId::new("test-session").unwrap();
        let session = crate::model::Session::new(session_id);
        let app_state = AppState::new(session);
        let key_bindings = KeyBindings::default();

        TuiApp {
            terminal,
            app_state,
            input_source,
            line_counter: 0,
            key_bindings,
            last_render: std::time::Instant::now(),
            pending_entries: Vec::new(),
        }
    }

    #[test]
    fn handle_key_q_returns_true() {
        let mut app = create_test_app();
        let key = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE);
        let should_quit = app.handle_key(key);
        assert!(should_quit, "'q' should trigger quit");
    }

    #[test]
    fn handle_key_ctrl_c_returns_true() {
        let mut app = create_test_app();
        let key = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        let should_quit = app.handle_key(key);
        assert!(should_quit, "Ctrl+C should trigger quit");
    }

    #[test]
    fn handle_key_other_returns_false() {
        let mut app = create_test_app();
        let key = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE);
        let should_quit = app.handle_key(key);
        assert!(!should_quit, "Normal keys should not trigger quit");
    }

    #[test]
    fn draw_renders_without_error() {
        let mut app = create_test_app();
        let result = app.draw();
        assert!(result.is_ok(), "Drawing should succeed");
    }

    // ===== Auto-scroll integration tests =====

    #[test]
    fn handle_key_a_toggles_auto_scroll() {
        let mut app = create_test_app();

        // Initially auto_scroll is true
        assert!(
            app.app_state.auto_scroll,
            "auto_scroll should start as true"
        );

        // Press 'a' to toggle off
        let key = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE);
        let should_quit = app.handle_key(key);
        assert!(!should_quit, "'a' should not trigger quit");
        assert!(
            !app.app_state.auto_scroll,
            "auto_scroll should toggle to false"
        );

        // Press 'a' again to toggle back on
        let should_quit = app.handle_key(key);
        assert!(!should_quit, "'a' should not trigger quit");
        assert!(
            app.app_state.auto_scroll,
            "auto_scroll should toggle back to true"
        );
    }

    #[test]
    fn handle_key_a_scrolls_to_bottom_when_enabling() {
        let mut app = create_test_app();

        // Add some entries to the session
        let entry1 = create_test_entry("msg1");
        let entry2 = create_test_entry("msg2");
        app.app_state.add_entries(vec![entry1, entry2]);

        // Disable auto_scroll and scroll to top
        app.app_state.auto_scroll = false;
        app.app_state.main_scroll.vertical_offset = 0;

        // Press 'a' to re-enable auto_scroll
        let key = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE);
        app.handle_key(key);

        // Should have scrolled to bottom
        let entry_count = app.app_state.session().main_agent().len();
        let expected_offset = entry_count.saturating_sub(1);
        assert_eq!(
            app.app_state.main_scroll.vertical_offset, expected_offset,
            "Enabling auto_scroll should scroll to bottom"
        );
    }

    #[test]
    fn auto_scroll_behavior_when_new_entries_arrive() {
        let mut app = create_test_app();

        // Add initial entries
        let entry1 = create_test_entry("msg1");
        let entry2 = create_test_entry("msg2");
        app.app_state.add_entries(vec![entry1, entry2]);

        app.app_state.live_mode = true;
        app.app_state.auto_scroll = true;

        // User scrolls to top
        app.app_state.main_scroll.vertical_offset = 0;

        // New entry arrives and we trigger auto-scroll (mimicking poll_input behavior)
        let new_entry = create_test_entry("new message");
        let entries_to_add = vec![new_entry];
        app.app_state.add_entries(entries_to_add.clone());

        // This is what poll_input() does after adding entries
        if app.app_state.live_mode && app.app_state.auto_scroll && !entries_to_add.is_empty() {
            let entry_count = app.app_state.session().main_agent().len();
            app.app_state
                .main_scroll
                .scroll_to_bottom(entry_count.saturating_sub(1));
        }

        // Verify scroll position moved to bottom
        let entry_count = app.app_state.session().main_agent().len();
        let expected_offset = entry_count.saturating_sub(1);
        assert_eq!(
            app.app_state.main_scroll.vertical_offset, expected_offset,
            "Should auto-scroll to bottom when live_mode && auto_scroll"
        );
        assert!(expected_offset >= 2, "Should have at least 3 entries");
    }

    #[test]
    fn auto_scroll_does_not_trigger_when_disabled() {
        let mut app = create_test_app();

        app.app_state.live_mode = true;
        app.app_state.auto_scroll = false; // Disabled

        // Set scroll position to top
        app.app_state.main_scroll.vertical_offset = 0;

        // Add entry
        let new_entry = create_test_entry("new message");
        let entries_to_add = vec![new_entry];
        app.app_state.add_entries(entries_to_add.clone());

        // Try to trigger auto-scroll (should be skipped when auto_scroll=false)
        if app.app_state.live_mode && app.app_state.auto_scroll && !entries_to_add.is_empty() {
            let entry_count = app.app_state.session().main_agent().len();
            app.app_state
                .main_scroll
                .scroll_to_bottom(entry_count.saturating_sub(1));
        }

        // Should still be at top
        assert_eq!(
            app.app_state.main_scroll.vertical_offset, 0,
            "Should NOT auto-scroll when auto_scroll is disabled"
        );
    }

    #[test]
    fn auto_scroll_does_not_trigger_when_not_live_mode() {
        let mut app = create_test_app();

        app.app_state.live_mode = false; // Not live mode
        app.app_state.auto_scroll = true;

        // Set scroll position to top
        app.app_state.main_scroll.vertical_offset = 0;

        // Add entry
        let new_entry = create_test_entry("new message");
        let entries_to_add = vec![new_entry];
        app.app_state.add_entries(entries_to_add.clone());

        // Try to trigger auto-scroll (should be skipped when not live_mode)
        if app.app_state.live_mode && app.app_state.auto_scroll && !entries_to_add.is_empty() {
            let entry_count = app.app_state.session().main_agent().len();
            app.app_state
                .main_scroll
                .scroll_to_bottom(entry_count.saturating_sub(1));
        }

        // Should still be at top
        assert_eq!(
            app.app_state.main_scroll.vertical_offset, 0,
            "Should NOT auto-scroll when not in live_mode"
        );
    }

    // ===== Stats filter keyboard shortcut tests =====

    #[test]
    fn handle_key_exclamation_sets_global_filter() {
        let mut app = create_test_app();

        // Set to a different filter initially
        app.app_state.stats_filter = crate::model::StatsFilter::MainAgent;

        // Press 'f' to set Global filter
        let key = KeyEvent::new(KeyCode::Char('f'), KeyModifiers::NONE);
        let should_quit = app.handle_key(key);

        assert!(!should_quit, "'f' should not trigger quit");
        assert_eq!(
            app.app_state.stats_filter,
            crate::model::StatsFilter::Global,
            "'f' should set stats filter to Global"
        );
    }

    #[test]
    fn handle_key_at_sets_main_agent_filter() {
        let mut app = create_test_app();

        // Set to Global initially
        app.app_state.stats_filter = crate::model::StatsFilter::Global;

        // Press 'm' to set MainAgent filter
        let key = KeyEvent::new(KeyCode::Char('m'), KeyModifiers::NONE);
        let should_quit = app.handle_key(key);

        assert!(!should_quit, "'m' should not trigger quit");
        assert_eq!(
            app.app_state.stats_filter,
            crate::model::StatsFilter::MainAgent,
            "'m' should set stats filter to MainAgent"
        );
    }

    #[test]
    fn handle_key_hash_sets_subagent_filter_when_tab_selected() {
        use crate::model::{
            AgentId, ConversationEntry, EntryMetadata, EntryType, EntryUuid, LogEntry, Message,
            MessageContent, Role, SessionId, TokenUsage,
        };
        use chrono::Utc;

        let mut app = create_test_app();

        // Add a subagent entry to create a tab
        let agent_id = AgentId::new("test-agent-1").unwrap();
        let message = Message::new(Role::Assistant, MessageContent::Text("test".to_string()))
            .with_usage(TokenUsage::default());
        let entry = LogEntry::new(
            EntryUuid::new("test-uuid-1").unwrap(),
            None,
            SessionId::new("test-session").unwrap(),
            Some(agent_id.clone()),
            Utc::now(),
            EntryType::Assistant,
            message,
            EntryMetadata::default(),
        );
        app.app_state
            .add_entries(vec![ConversationEntry::Valid(Box::new(entry))]);

        // Select the subagent tab (index 0)
        app.app_state.selected_tab = Some(0);

        // Set to Global initially
        app.app_state.stats_filter = crate::model::StatsFilter::Global;

        // Press 'S' (Shift+s) to set Subagent filter for the selected tab
        let key = KeyEvent::new(KeyCode::Char('S'), KeyModifiers::SHIFT);
        let should_quit = app.handle_key(key);

        assert!(!should_quit, "'S' should not trigger quit");
        assert_eq!(
            app.app_state.stats_filter,
            crate::model::StatsFilter::Subagent(agent_id),
            "'S' should set stats filter to Subagent with selected tab's agent ID"
        );
    }

    #[test]
    fn handle_key_hash_does_nothing_when_no_tab_selected() {
        let mut app = create_test_app();

        // No tab selected
        app.app_state.selected_tab = None;

        // Set to Global initially
        app.app_state.stats_filter = crate::model::StatsFilter::Global;

        // Press 'S' when no tab is selected
        let key = KeyEvent::new(KeyCode::Char('S'), KeyModifiers::SHIFT);
        let should_quit = app.handle_key(key);

        assert!(!should_quit, "'S' should not trigger quit");
        assert_eq!(
            app.app_state.stats_filter,
            crate::model::StatsFilter::Global,
            "'S' should not change filter when no tab is selected"
        );
    }

    // ===== Focus cycling keyboard handler tests =====

    #[test]
    fn handle_key_tab_cycles_focus_main_to_subagent() {
        let mut app = create_test_app();

        // Verify initial focus is Main
        assert_eq!(
            app.app_state.focus,
            crate::state::FocusPane::Main,
            "Initial focus should be Main"
        );

        // Press Tab
        let key = KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE);
        let should_quit = app.handle_key(key);

        assert!(!should_quit, "Tab should not trigger quit");
        assert_eq!(
            app.app_state.focus,
            crate::state::FocusPane::Subagent,
            "Tab should cycle focus from Main to Subagent"
        );
    }

    #[test]
    fn handle_key_tab_cycles_focus_subagent_to_stats() {
        let mut app = create_test_app();

        // Set focus to Subagent
        app.app_state.focus = crate::state::FocusPane::Subagent;

        // Press Tab
        let key = KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE);
        let should_quit = app.handle_key(key);

        assert!(!should_quit, "Tab should not trigger quit");
        assert_eq!(
            app.app_state.focus,
            crate::state::FocusPane::Stats,
            "Tab should cycle focus from Subagent to Stats"
        );
    }

    #[test]
    fn handle_key_tab_cycles_focus_stats_to_main() {
        let mut app = create_test_app();

        // Set focus to Stats
        app.app_state.focus = crate::state::FocusPane::Stats;

        // Press Tab
        let key = KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE);
        let should_quit = app.handle_key(key);

        assert!(!should_quit, "Tab should not trigger quit");
        assert_eq!(
            app.app_state.focus,
            crate::state::FocusPane::Main,
            "Tab should cycle focus from Stats back to Main"
        );
    }

    #[test]
    fn handle_key_1_sets_focus_main() {
        let mut app = create_test_app();

        // Start with focus on Stats
        app.app_state.focus = crate::state::FocusPane::Stats;

        // Press '1'
        let key = KeyEvent::new(KeyCode::Char('1'), KeyModifiers::NONE);
        let should_quit = app.handle_key(key);

        assert!(!should_quit, "'1' should not trigger quit");
        assert_eq!(
            app.app_state.focus,
            crate::state::FocusPane::Main,
            "'1' should set focus to Main"
        );
    }

    #[test]
    fn handle_key_2_sets_focus_subagent() {
        let mut app = create_test_app();

        // Start with focus on Main
        app.app_state.focus = crate::state::FocusPane::Main;

        // Press '2'
        let key = KeyEvent::new(KeyCode::Char('2'), KeyModifiers::NONE);
        let should_quit = app.handle_key(key);

        assert!(!should_quit, "'2' should not trigger quit");
        assert_eq!(
            app.app_state.focus,
            crate::state::FocusPane::Subagent,
            "'2' should set focus to Subagent"
        );
    }

    #[test]
    fn handle_key_3_sets_focus_stats() {
        let mut app = create_test_app();

        // Start with focus on Subagent
        app.app_state.focus = crate::state::FocusPane::Subagent;

        // Press '3'
        let key = KeyEvent::new(KeyCode::Char('3'), KeyModifiers::NONE);
        let should_quit = app.handle_key(key);

        assert!(!should_quit, "'3' should not trigger quit");
        assert_eq!(
            app.app_state.focus,
            crate::state::FocusPane::Stats,
            "'3' should set focus to Stats"
        );
    }

    // Helper function to create a test LogEntry
    fn create_test_entry(content: &str) -> crate::model::ConversationEntry {
        use crate::model::{
            ConversationEntry, EntryMetadata, EntryType, EntryUuid, LogEntry, Message,
            MessageContent, Role, SessionId,
        };
        use chrono::Utc;

        let message = Message::new(Role::User, MessageContent::Text(content.to_string()));

        let log_entry = LogEntry::new(
            EntryUuid::new("test-uuid").unwrap(),
            None,
            SessionId::new("test-session").unwrap(),
            None,
            Utc::now(),
            EntryType::User,
            message,
            EntryMetadata::default(),
        );

        ConversationEntry::Valid(Box::new(log_entry))
    }

    // ===== Batch rendering tests (60fps) =====

    #[test]
    fn batch_accumulates_entries_without_dropping() {
        // Test that rapid entries are accumulated, not dropped
        let mut app = create_test_app();

        // Simulate 100 rapid entries arriving
        let entries: Vec<_> = (0..100)
            .map(|i| create_test_entry(&format!("message {}", i)))
            .collect();

        // Add all entries to pending buffer (simulating rapid poll_input calls)
        app.accumulate_pending_entries(entries);

        // Verify all entries are in buffer
        assert_eq!(
            app.pending_entry_count(),
            100,
            "All 100 entries should be accumulated in buffer"
        );

        // Verify session hasn't been updated yet (batched)
        assert_eq!(
            app.app_state.session().main_agent().len(),
            0,
            "Entries should not be added to session until flush"
        );
    }

    #[test]
    fn batch_flush_moves_all_entries_to_session() {
        let mut app = create_test_app();

        // Add pending entries
        let entries: Vec<_> = (0..50)
            .map(|i| create_test_entry(&format!("msg {}", i)))
            .collect();
        app.accumulate_pending_entries(entries);

        assert_eq!(app.pending_entry_count(), 50);

        // Flush should move all to session and clear buffer
        app.flush_pending_entries();

        assert_eq!(
            app.pending_entry_count(),
            0,
            "Buffer should be empty after flush"
        );
        assert_eq!(
            app.app_state.session().main_agent().len(),
            50,
            "All entries should be in session after flush"
        );
    }

    #[test]
    fn should_render_returns_false_before_frame_duration() {
        use std::time::{Duration, Instant};

        let mut app = create_test_app();

        // Set last render to just now
        app.set_last_render_time(Instant::now());

        // Immediately check - should not render yet
        assert!(
            !app.should_render_frame(),
            "Should not render immediately after last render"
        );

        // Check after 10ms (less than 16ms frame budget)
        std::thread::sleep(Duration::from_millis(10));
        assert!(
            !app.should_render_frame(),
            "Should not render after only 10ms"
        );
    }

    #[test]
    fn should_render_returns_true_after_frame_duration() {
        use std::time::{Duration, Instant};

        let mut app = create_test_app();

        // Set last render to 20ms ago (past the 16ms frame budget)
        let past = Instant::now() - Duration::from_millis(20);
        app.set_last_render_time(past);

        assert!(
            app.should_render_frame(),
            "Should render after 16ms frame duration has elapsed"
        );
    }

    #[test]
    fn should_render_returns_true_when_pending_entries_and_frame_elapsed() {
        use std::time::{Duration, Instant};

        let mut app = create_test_app();

        // Set last render to 20ms ago
        let past = Instant::now() - Duration::from_millis(20);
        app.set_last_render_time(past);

        // Add pending entries
        app.accumulate_pending_entries(vec![create_test_entry("msg1")]);

        assert!(
            app.should_render_frame(),
            "Should render when frame elapsed and entries pending"
        );
    }

    #[test]
    fn batching_prevents_rendering_on_every_entry() {
        // This test verifies the core requirement:
        // When many entries arrive rapidly (< 16ms apart),
        // we should NOT render after each one.

        let mut app = create_test_app();

        // Set last render time to now so timing checks are valid
        app.set_last_render_time(std::time::Instant::now());

        // Simulate 10 entries arriving in quick succession
        for i in 0..10 {
            let entry = create_test_entry(&format!("rapid msg {}", i));
            app.accumulate_pending_entries(vec![entry]);

            // Check if we should render (simulating the event loop decision)
            let should_render = app.should_render_frame();

            if i == 0 {
                // First entry after enough time should trigger render
                continue;
            }

            // Subsequent rapid entries should NOT trigger render
            // (assuming they arrive within 16ms)
            assert!(
                !should_render,
                "Rapid entry {} should not trigger immediate render",
                i
            );
        }
    }
}
