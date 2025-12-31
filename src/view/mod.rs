//! TUI rendering and terminal management (impure shell)

mod help;
mod layout;
mod log_pane;
mod message;
mod search_input;
mod stats;
mod styles;
pub mod tabs;

pub use help::render_help_overlay;
pub use log_pane::LogPaneView;
pub use message::ConversationView;
pub use search_input::SearchInput;
pub use stats::StatsPanel;
pub use styles::{ColorConfig, MessageStyles};

use crate::config::keybindings::KeyBindings;
use crate::integration;
use crate::model::{AppError, KeyAction, SessionId};
use crate::source::InputSource;
use crate::state::{
    expand_handler, handle_toggle_wrap, next_match, prev_match, scroll_handler,
    search_input_handler, AppState, FocusPane,
};
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
    /// Receiver for log entries from tracing subscriber
    log_receiver: std::sync::mpsc::Receiver<crate::state::log_pane::LogPaneEntry>,
}

impl TuiApp<CrosstermBackend<Stdout>> {
    /// Create and initialize a new TUI application
    ///
    /// Sets up terminal in raw mode with alternate screen
    pub fn new(
        mut input_source: InputSource,
        session_id: SessionId,
        log_receiver: std::sync::mpsc::Receiver<crate::state::log_pane::LogPaneEntry>,
    ) -> Result<Self, TuiError> {
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
            log_receiver,
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

            // Poll for log pane entries from tracing subscriber
            self.poll_log_entries();

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

    /// Poll log receiver and push entries to log pane state
    ///
    /// Non-blocking poll using try_recv(). All available entries are consumed.
    fn poll_log_entries(&mut self) {
        while let Ok(entry) = self.log_receiver.try_recv() {
            self.app_state.log_pane.push(entry);
        }
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
                            ch,
                        );
                        return false;
                    }
                    KeyCode::Backspace => {
                        self.app_state.search =
                            search_input_handler::handle_backspace(self.app_state.search.clone());
                        return false;
                    }
                    KeyCode::Left => {
                        self.app_state.search =
                            search_input_handler::handle_cursor_left(self.app_state.search.clone());
                        return false;
                    }
                    KeyCode::Right => {
                        self.app_state.search = search_input_handler::handle_cursor_right(
                            self.app_state.search.clone(),
                        );
                        return false;
                    }
                    KeyCode::Enter => {
                        // Submit search on Enter when typing
                        self.app_state.search =
                            search_input_handler::submit_search(self.app_state.search.clone());
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
            | KeyAction::ScrollLeft
            | KeyAction::ScrollRight
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

            // Tab navigation - delegate to app_state methods
            KeyAction::NextTab => {
                self.app_state.next_tab();
            }
            KeyAction::PrevTab => {
                self.app_state.prev_tab();
            }
            KeyAction::SelectTab(n) => {
                self.app_state.select_tab(n);
            }

            // Message expand/collapse - delegate to pure expand handler
            KeyAction::ToggleExpand | KeyAction::ExpandMessage | KeyAction::CollapseMessage => {
                let new_state =
                    expand_handler::handle_expand_action(self.app_state.clone(), action);
                self.app_state = new_state;
            }

            // Search actions - delegate to pure search input handler
            KeyAction::StartSearch => {
                self.app_state.search =
                    search_input_handler::activate_search_input(self.app_state.search.clone());
                self.app_state.focus = FocusPane::Search;
            }
            KeyAction::SubmitSearch => {
                self.app_state.search =
                    search_input_handler::submit_search(self.app_state.search.clone());
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
                self.app_state.search =
                    search_input_handler::cancel_search(self.app_state.search.clone());
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

            // Line wrapping - per-item toggle (w key)
            KeyAction::ToggleWrap => {
                self.app_state = handle_toggle_wrap(self.app_state.clone());
            }

            // Line wrapping - global toggle (W key)
            KeyAction::ToggleGlobalWrap => {
                self.app_state.toggle_global_wrap();
            }

            // Log pane toggle (L key)
            KeyAction::ToggleLogPane => {
                self.app_state.log_pane.toggle_visible();
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

/// CLI arguments for TUI initialization
///
/// This struct represents the subset of command-line arguments that affect
/// the TUI's initial state. It lives in the view module because it configures
/// the rendering layer (impure shell), not the domain logic.
///
/// # Design Notes
///
/// Per the Pure Core / Impure Shell architecture (see constitution.md):
/// - Domain state lives in `AppState` (pure)
/// - CLI parsing happens in main.rs (impure)
/// - This struct bridges the gap, carrying configuration from CLI → TUI
///
/// # Example
///
/// ```rust,no_run
/// use cclv::view::CliArgs;
///
/// let args = CliArgs::new(
///     true,   // Show stats panel on startup
///     true,   // Enable live-follow mode
/// );
/// ```
pub struct CliArgs {
    /// Whether to show the statistics panel on startup
    ///
    /// Maps to `--stats` CLI flag. When true, the stats panel
    /// is visible immediately; when false, user can toggle with 's' key.
    pub stats: bool,

    /// Whether to enable live-follow mode (tail -f behavior)
    ///
    /// Maps to `--follow` CLI flag. When true:
    /// - Auto-scroll is enabled by default (FR-035)
    /// - New entries trigger scroll to bottom (FR-036)
    /// - Input source continues polling for new data
    ///
    /// When false, the log is treated as static/completed.
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
    // Initialize logging with log pane integration
    let (log_tx, log_rx) = std::sync::mpsc::channel();
    crate::logging::init_with_log_pane(log_tx).map_err(|e| TuiError::Io(io::Error::other(e)))?;

    // Extract or create session ID
    // For now, use a default session ID. In the future, this could be
    // extracted from the first log entry or passed via args.
    let session_id = SessionId::new("default-session").map_err(|_| {
        TuiError::Io(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Invalid session ID",
        ))
    })?;

    let mut app = TuiApp::new(input_source, session_id, log_rx)?;

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

        // Create a dummy log receiver for tests
        let (_log_tx, log_rx) = std::sync::mpsc::channel();

        TuiApp {
            terminal,
            app_state,
            input_source,
            line_counter: 0,
            key_bindings,
            last_render: std::time::Instant::now(),
            pending_entries: Vec::new(),
            log_receiver: log_rx,
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

    // ===== US4: Tab Navigation Dispatch Tests =====

    #[test]
    fn handle_key_right_bracket_calls_next_tab() {
        use crate::model::{
            AgentId, ConversationEntry, EntryMetadata, EntryType, EntryUuid, LogEntry, Message,
            MessageContent, Role, SessionId, TokenUsage,
        };
        use chrono::Utc;

        let mut app = create_test_app();

        // Add two subagent entries to create tabs
        let agent_id_1 = AgentId::new("agent-1").unwrap();
        let agent_id_2 = AgentId::new("agent-2").unwrap();

        for (idx, agent_id) in [&agent_id_1, &agent_id_2].iter().enumerate() {
            let message =
                Message::new(Role::Assistant, MessageContent::Text(format!("msg{}", idx)))
                    .with_usage(TokenUsage::default());
            let entry = LogEntry::new(
                EntryUuid::new(&format!("uuid-{}", idx)).unwrap(),
                None,
                SessionId::new("test-session").unwrap(),
                Some((*agent_id).clone()),
                Utc::now(),
                EntryType::Assistant,
                message,
                EntryMetadata::default(),
            );
            app.app_state
                .add_entries(vec![ConversationEntry::Valid(Box::new(entry))]);
        }

        // Focus on Subagent pane and select first tab
        app.app_state.focus = FocusPane::Subagent;
        app.app_state.selected_tab = Some(0);

        // Press ']' to go to next tab
        let key = KeyEvent::new(KeyCode::Char(']'), KeyModifiers::NONE);
        let should_quit = app.handle_key(key);

        assert!(!should_quit, "']' should not trigger quit");
        assert_eq!(
            app.app_state.selected_tab,
            Some(1),
            "']' should call next_tab() and move to tab 1"
        );
    }

    #[test]
    fn handle_key_left_bracket_calls_prev_tab() {
        use crate::model::{
            AgentId, ConversationEntry, EntryMetadata, EntryType, EntryUuid, LogEntry, Message,
            MessageContent, Role, SessionId, TokenUsage,
        };
        use chrono::Utc;

        let mut app = create_test_app();

        // Add two subagent entries
        let agent_id_1 = AgentId::new("agent-1").unwrap();
        let agent_id_2 = AgentId::new("agent-2").unwrap();

        for (idx, agent_id) in [&agent_id_1, &agent_id_2].iter().enumerate() {
            let message =
                Message::new(Role::Assistant, MessageContent::Text(format!("msg{}", idx)))
                    .with_usage(TokenUsage::default());
            let entry = LogEntry::new(
                EntryUuid::new(&format!("uuid-{}", idx)).unwrap(),
                None,
                SessionId::new("test-session").unwrap(),
                Some((*agent_id).clone()),
                Utc::now(),
                EntryType::Assistant,
                message,
                EntryMetadata::default(),
            );
            app.app_state
                .add_entries(vec![ConversationEntry::Valid(Box::new(entry))]);
        }

        // Focus on Subagent pane and select second tab
        app.app_state.focus = FocusPane::Subagent;
        app.app_state.selected_tab = Some(1);

        // Press '[' to go to previous tab
        let key = KeyEvent::new(KeyCode::Char('['), KeyModifiers::NONE);
        let should_quit = app.handle_key(key);

        assert!(!should_quit, "'[' should not trigger quit");
        assert_eq!(
            app.app_state.selected_tab,
            Some(0),
            "'[' should call prev_tab() and move to tab 0"
        );
    }

    #[test]
    fn handle_key_number_calls_select_tab() {
        use crate::model::{
            AgentId, ConversationEntry, EntryMetadata, EntryType, EntryUuid, LogEntry, Message,
            MessageContent, Role, SessionId, TokenUsage,
        };
        use chrono::Utc;

        let mut app = create_test_app();

        // Add 5 subagent entries
        for idx in 0..5 {
            let agent_id = AgentId::new(&format!("agent-{}", idx)).unwrap();
            let message =
                Message::new(Role::Assistant, MessageContent::Text(format!("msg{}", idx)))
                    .with_usage(TokenUsage::default());
            let entry = LogEntry::new(
                EntryUuid::new(&format!("uuid-{}", idx)).unwrap(),
                None,
                SessionId::new("test-session").unwrap(),
                Some(agent_id),
                Utc::now(),
                EntryType::Assistant,
                message,
                EntryMetadata::default(),
            );
            app.app_state
                .add_entries(vec![ConversationEntry::Valid(Box::new(entry))]);
        }

        // Focus on Subagent pane
        app.app_state.focus = FocusPane::Subagent;
        app.app_state.selected_tab = Some(0);

        // Press '5' to select 5th tab (0-indexed: tab 4)
        let key = KeyEvent::new(KeyCode::Char('5'), KeyModifiers::NONE);
        let should_quit = app.handle_key(key);

        assert!(!should_quit, "'5' should not trigger quit");
        assert_eq!(
            app.app_state.selected_tab,
            Some(4),
            "'5' should call select_tab(5) and move to 0-indexed tab 4"
        );
    }

    // ===== US4: Message Expand/Collapse Dispatch Tests =====

    #[test]
    fn handle_key_enter_toggles_expand() {
        let mut app = create_test_app();

        // Add an entry to the main pane
        let entry = create_test_entry("test message");
        app.app_state.add_entries(vec![entry]);

        // Focus on Main pane and set focused message
        app.app_state.focus = FocusPane::Main;
        app.app_state.main_scroll.set_focused_message(Some(0));

        // Get the UUID of the first message
        let uuid = app
            .app_state
            .session()
            .main_agent()
            .entries()
            .first()
            .and_then(|e| e.as_valid())
            .map(|log| log.uuid().clone())
            .unwrap();

        // Initially not expanded
        assert!(
            !app.app_state.main_scroll.is_expanded(&uuid),
            "Message should start collapsed"
        );

        // Press Enter to toggle expand
        let key = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
        let should_quit = app.handle_key(key);

        assert!(!should_quit, "Enter should not trigger quit");
        assert!(
            app.app_state.main_scroll.is_expanded(&uuid),
            "Enter should call expand_handler and toggle message to expanded"
        );
    }

    #[test]
    fn handle_key_e_expands_all_messages() {
        use crate::model::{
            ConversationEntry, EntryMetadata, EntryType, EntryUuid, LogEntry, Message,
            MessageContent, Role, SessionId,
        };
        use chrono::Utc;

        let mut app = create_test_app();

        // Add multiple entries with UNIQUE UUIDs
        for i in 0..3 {
            let message = Message::new(Role::User, MessageContent::Text(format!("msg{}", i)));
            let entry = LogEntry::new(
                EntryUuid::new(&format!("uuid-{}", i)).unwrap(),
                None,
                SessionId::new("test-session").unwrap(),
                None,
                Utc::now(),
                EntryType::User,
                message,
                EntryMetadata::default(),
            );
            app.app_state
                .add_entries(vec![ConversationEntry::Valid(Box::new(entry))]);
        }

        // Focus on Main pane
        app.app_state.focus = FocusPane::Main;

        // Initially all collapsed
        assert_eq!(
            app.app_state.main_scroll.expanded_messages.len(),
            0,
            "No messages should be expanded initially"
        );

        // Press 'e' to expand all
        let key = KeyEvent::new(KeyCode::Char('e'), KeyModifiers::NONE);
        let should_quit = app.handle_key(key);

        assert!(!should_quit, "'e' should not trigger quit");
        assert_eq!(
            app.app_state.main_scroll.expanded_messages.len(),
            3,
            "'e' should call expand_handler and expand all 3 messages"
        );
    }

    #[test]
    fn handle_key_c_collapses_all_messages() {
        use crate::model::{
            ConversationEntry, EntryMetadata, EntryType, EntryUuid, LogEntry, Message,
            MessageContent, Role, SessionId,
        };
        use chrono::Utc;

        let mut app = create_test_app();

        // Add entries with UNIQUE UUIDs
        for i in 0..3 {
            let message = Message::new(Role::User, MessageContent::Text(format!("msg{}", i)));
            let entry = LogEntry::new(
                EntryUuid::new(&format!("uuid-collapse-{}", i)).unwrap(),
                None,
                SessionId::new("test-session").unwrap(),
                None,
                Utc::now(),
                EntryType::User,
                message,
                EntryMetadata::default(),
            );
            app.app_state
                .add_entries(vec![ConversationEntry::Valid(Box::new(entry))]);
        }

        // Focus on Main and expand all
        app.app_state.focus = FocusPane::Main;
        let uuids: Vec<_> = app
            .app_state
            .session()
            .main_agent()
            .entries()
            .iter()
            .filter_map(|e| e.as_valid().map(|log| log.uuid().clone()))
            .collect();
        app.app_state.main_scroll.expand_all(uuids.into_iter());

        assert_eq!(app.app_state.main_scroll.expanded_messages.len(), 3);

        // Press 'c' to collapse all
        let key = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE);
        let should_quit = app.handle_key(key);

        assert!(!should_quit, "'c' should not trigger quit");
        assert_eq!(
            app.app_state.main_scroll.expanded_messages.len(),
            0,
            "'c' should call expand_handler and collapse all messages"
        );
    }

    // ===== US4: Horizontal Scrolling Tests =====

    #[test]
    fn handle_key_ctrl_f_starts_search() {
        let mut app = create_test_app();

        // Start with focus on Main
        app.app_state.focus = FocusPane::Main;

        // Verify search is inactive
        assert!(matches!(
            app.app_state.search,
            crate::state::SearchState::Inactive
        ));

        // Press Ctrl+F to start search
        let key = KeyEvent::new(KeyCode::Char('f'), KeyModifiers::CONTROL);
        let should_quit = app.handle_key(key);

        assert!(!should_quit, "Ctrl+F should not trigger quit");
        assert_eq!(
            app.app_state.focus,
            FocusPane::Search,
            "Ctrl+F should focus on Search pane"
        );
        assert!(
            matches!(
                app.app_state.search,
                crate::state::SearchState::Typing { .. }
            ),
            "Ctrl+F should activate search input"
        );
    }

    #[test]
    fn handle_key_h_scrolls_left() {
        let mut app = create_test_app();

        // Focus on Main pane and set horizontal offset
        app.app_state.focus = FocusPane::Main;
        app.app_state.main_scroll.horizontal_offset = 10;

        // Press 'h' to scroll left
        let key = KeyEvent::new(KeyCode::Char('h'), KeyModifiers::NONE);
        let should_quit = app.handle_key(key);

        assert!(!should_quit, "'h' should not trigger quit");
        assert_eq!(
            app.app_state.main_scroll.horizontal_offset, 9,
            "'h' should call scroll_handler with ScrollLeft action"
        );
    }

    #[test]
    fn handle_key_l_scrolls_right() {
        let mut app = create_test_app();

        // Focus on Main pane
        app.app_state.focus = FocusPane::Main;
        app.app_state.main_scroll.horizontal_offset = 0;

        // Press 'l' to scroll right
        let key = KeyEvent::new(KeyCode::Char('l'), KeyModifiers::NONE);
        let should_quit = app.handle_key(key);

        assert!(!should_quit, "'l' should not trigger quit");
        assert_eq!(
            app.app_state.main_scroll.horizontal_offset, 1,
            "'l' should call scroll_handler with ScrollRight action"
        );
    }

    // ===== Line Wrapping Tests (LW-007) =====

    #[test]
    fn handle_key_w_toggles_wrap_for_focused_message() {
        use crate::model::{
            ConversationEntry, EntryMetadata, EntryType, EntryUuid, LogEntry, Message,
            MessageContent, Role, SessionId,
        };
        use crate::state::WrapMode;
        use chrono::Utc;

        let mut app = create_test_app();

        // Add an entry to main pane
        let message = Message::new(Role::User, MessageContent::Text("test message".to_string()));
        let uuid = EntryUuid::new("test-uuid-wrap").unwrap();
        let entry = LogEntry::new(
            uuid.clone(),
            None,
            SessionId::new("test-session").unwrap(),
            None,
            Utc::now(),
            EntryType::User,
            message,
            EntryMetadata::default(),
        );
        app.app_state
            .add_entries(vec![ConversationEntry::Valid(Box::new(entry))]);

        // Focus on Main pane and set focused message
        app.app_state.focus = FocusPane::Main;
        app.app_state.main_scroll.set_focused_message(Some(0));

        // Global wrap is Wrap by default
        assert_eq!(app.app_state.global_wrap, WrapMode::Wrap);

        // Initially no overrides
        assert!(
            !app.app_state.main_scroll.wrap_overrides.contains(&uuid),
            "UUID should not be in wrap_overrides initially"
        );

        // Press 'w' to toggle wrap for focused message
        let key = KeyEvent::new(KeyCode::Char('w'), KeyModifiers::NONE);
        let should_quit = app.handle_key(key);

        assert!(!should_quit, "'w' should not trigger quit");
        assert!(
            app.app_state.main_scroll.wrap_overrides.contains(&uuid),
            "'w' should add focused message UUID to wrap_overrides"
        );

        // Press 'w' again to toggle back
        let should_quit = app.handle_key(key);

        assert!(!should_quit, "'w' should not trigger quit");
        assert!(
            !app.app_state.main_scroll.wrap_overrides.contains(&uuid),
            "'w' should remove UUID from wrap_overrides on second toggle"
        );
    }

    #[test]
    fn handle_key_w_does_nothing_when_no_focused_message() {
        let mut app = create_test_app();

        // Focus on Main pane but no focused message
        app.app_state.focus = FocusPane::Main;
        app.app_state.main_scroll.set_focused_message(None);

        // Initially no overrides
        let initial_overrides_count = app.app_state.main_scroll.wrap_overrides.len();

        // Press 'w'
        let key = KeyEvent::new(KeyCode::Char('w'), KeyModifiers::NONE);
        let should_quit = app.handle_key(key);

        assert!(!should_quit, "'w' should not trigger quit");
        assert_eq!(
            app.app_state.main_scroll.wrap_overrides.len(),
            initial_overrides_count,
            "'w' should not change wrap_overrides when no message is focused"
        );
    }

    #[test]
    fn handle_key_w_operates_on_correct_pane() {
        use crate::model::{
            AgentId, ConversationEntry, EntryMetadata, EntryType, EntryUuid, LogEntry, Message,
            MessageContent, Role, SessionId, TokenUsage,
        };
        use chrono::Utc;

        let mut app = create_test_app();

        // Add entry to main pane
        let main_uuid = EntryUuid::new("main-uuid").unwrap();
        let main_message = Message::new(Role::User, MessageContent::Text("main".to_string()));
        let main_entry = LogEntry::new(
            main_uuid.clone(),
            None,
            SessionId::new("test-session").unwrap(),
            None,
            Utc::now(),
            EntryType::User,
            main_message,
            EntryMetadata::default(),
        );
        app.app_state
            .add_entries(vec![ConversationEntry::Valid(Box::new(main_entry))]);

        // Add entry to subagent pane
        let agent_id = AgentId::new("test-agent").unwrap();
        let sub_uuid = EntryUuid::new("sub-uuid").unwrap();
        let sub_message = Message::new(Role::Assistant, MessageContent::Text("sub".to_string()))
            .with_usage(TokenUsage::default());
        let sub_entry = LogEntry::new(
            sub_uuid.clone(),
            None,
            SessionId::new("test-session").unwrap(),
            Some(agent_id),
            Utc::now(),
            EntryType::Assistant,
            sub_message,
            EntryMetadata::default(),
        );
        app.app_state
            .add_entries(vec![ConversationEntry::Valid(Box::new(sub_entry))]);

        // Focus on Subagent pane
        app.app_state.focus = FocusPane::Subagent;
        app.app_state.selected_tab = Some(0);
        app.app_state.subagent_scroll.set_focused_message(Some(0));

        // Press 'w' - should toggle subagent scroll state, not main
        let key = KeyEvent::new(KeyCode::Char('w'), KeyModifiers::NONE);
        app.handle_key(key);

        assert!(
            app.app_state
                .subagent_scroll
                .wrap_overrides
                .contains(&sub_uuid),
            "'w' should toggle wrap in subagent_scroll when Subagent pane is focused"
        );
        assert!(
            !app.app_state
                .main_scroll
                .wrap_overrides
                .contains(&main_uuid),
            "'w' should not affect main_scroll when Subagent pane is focused"
        );
    }

    #[test]
    fn handle_key_shift_w_toggles_global_wrap() {
        use crate::state::WrapMode;

        let mut app = create_test_app();

        // Default global wrap is Wrap
        assert_eq!(
            app.app_state.global_wrap,
            WrapMode::Wrap,
            "Initial global_wrap should be Wrap"
        );

        // Press Shift+W to toggle global wrap
        let key = KeyEvent::new(KeyCode::Char('W'), KeyModifiers::SHIFT);
        let should_quit = app.handle_key(key);

        assert!(!should_quit, "'W' should not trigger quit");
        assert_eq!(
            app.app_state.global_wrap,
            WrapMode::NoWrap,
            "'W' should toggle global_wrap to NoWrap"
        );

        // Press Shift+W again to toggle back
        let should_quit = app.handle_key(key);

        assert!(!should_quit, "'W' should not trigger quit");
        assert_eq!(
            app.app_state.global_wrap,
            WrapMode::Wrap,
            "'W' should toggle global_wrap back to Wrap"
        );
    }

    #[test]
    fn handle_key_shift_w_is_independent_of_focus() {
        use crate::state::WrapMode;

        let mut app = create_test_app();

        // Test with focus on Main
        app.app_state.focus = FocusPane::Main;
        app.app_state.global_wrap = WrapMode::Wrap;

        let key = KeyEvent::new(KeyCode::Char('W'), KeyModifiers::SHIFT);
        app.handle_key(key);

        assert_eq!(
            app.app_state.global_wrap,
            WrapMode::NoWrap,
            "'W' should work when focused on Main"
        );

        // Test with focus on Stats
        app.app_state.focus = FocusPane::Stats;
        app.handle_key(key);

        assert_eq!(
            app.app_state.global_wrap,
            WrapMode::Wrap,
            "'W' should work when focused on Stats"
        );

        // Test with focus on Search
        app.app_state.focus = FocusPane::Search;
        app.handle_key(key);

        assert_eq!(
            app.app_state.global_wrap,
            WrapMode::NoWrap,
            "'W' should work when focused on Search"
        );
    }

    // ===== Log Pane Integration Tests =====

    #[test]
    fn poll_log_entries_consumes_all_available_entries() {
        let mut app = create_test_app();

        // Send some log entries
        let (tx, rx) = std::sync::mpsc::channel();
        let entry1 = crate::state::log_pane::LogPaneEntry {
            timestamp: chrono::Utc::now(),
            level: tracing::Level::INFO,
            message: "test message 1".to_string(),
        };
        let entry2 = crate::state::log_pane::LogPaneEntry {
            timestamp: chrono::Utc::now(),
            level: tracing::Level::WARN,
            message: "test message 2".to_string(),
        };

        tx.send(entry1.clone()).unwrap();
        tx.send(entry2.clone()).unwrap();

        // Replace app's receiver with our test receiver
        app.log_receiver = rx;

        // Initially log pane should be empty
        assert_eq!(app.app_state.log_pane.entries().len(), 0);

        // Poll log entries
        app.poll_log_entries();

        // Verify entries were consumed and pushed to log pane
        assert_eq!(
            app.app_state.log_pane.entries().len(),
            2,
            "Should have consumed both log entries"
        );
        assert_eq!(
            app.app_state.log_pane.entries()[0].message, "test message 1"
        );
        assert_eq!(
            app.app_state.log_pane.entries()[1].message, "test message 2"
        );
    }

    #[test]
    fn poll_log_entries_handles_empty_receiver() {
        let mut app = create_test_app();

        // Receiver is empty
        assert_eq!(app.app_state.log_pane.entries().len(), 0);

        // Poll should not panic or error
        app.poll_log_entries();

        // Log pane should still be empty
        assert_eq!(app.app_state.log_pane.entries().len(), 0);
    }

    #[test]
    fn poll_log_entries_is_non_blocking() {
        let mut app = create_test_app();

        // Empty receiver - should return immediately, not block
        let start = std::time::Instant::now();
        app.poll_log_entries();
        let elapsed = start.elapsed();

        // Should complete almost instantly (< 10ms)
        assert!(
            elapsed < std::time::Duration::from_millis(10),
            "poll_log_entries should be non-blocking, took {:?}",
            elapsed
        );
    }
}
