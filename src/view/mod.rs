//! TUI rendering and terminal management (impure shell)

mod layout;
mod message;
mod stats;
mod styles;
pub mod tabs;

pub use message::ConversationView;
pub use stats::StatsPanel;
pub use styles::MessageStyles;

use crate::integration;
use crate::model::{AppError, SessionId};
use crate::source::InputSource;
use crate::state::AppState;
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

        Ok(Self {
            terminal,
            app_state,
            input_source,
            line_counter,
        })
    }

    /// Run the main event loop
    ///
    /// Returns when user quits (q or Ctrl+C)
    /// Target: 60fps (16ms frame budget)
    pub fn run(&mut self) -> Result<(), TuiError> {
        const FRAME_DURATION: Duration = Duration::from_millis(16); // ~60fps

        loop {
            // Poll for new log entries
            self.poll_input()?;

            // Render frame
            self.draw()?;

            // Poll for keyboard events (non-blocking with timeout)
            if event::poll(FRAME_DURATION)? {
                if let Event::Key(key) = event::read()? {
                    if self.handle_key(key) {
                        break;
                    }
                }
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

            // Update line counter BEFORE adding entries
            self.line_counter += entries.len();

            // Add entries to session via AppState
            self.app_state.add_entries(entries.clone());

            // FR-035: Auto-scroll to bottom when live_mode && auto_scroll
            if self.app_state.live_mode && self.app_state.auto_scroll && !entries.is_empty() {
                let entry_count = self.app_state.session().main_agent().len();
                self.app_state
                    .main_scroll
                    .scroll_to_bottom(entry_count.saturating_sub(1));
            }
        }

        Ok(())
    }

    /// Handle a single keyboard event
    ///
    /// Returns true if app should quit
    fn handle_key(&mut self, key: KeyEvent) -> bool {
        // FR-038: Toggle auto-scroll on 'a' key
        if key.code == KeyCode::Char('a') {
            self.app_state.auto_scroll = !self.app_state.auto_scroll;
            // If enabling, scroll to bottom immediately
            if self.app_state.auto_scroll {
                let entry_count = self.app_state.session().main_agent().len();
                self.app_state
                    .main_scroll
                    .scroll_to_bottom(entry_count.saturating_sub(1));
            }
            return false;
        }

        // FR-020: Stats filtering by agent
        // '!' - Filter stats to Global (all agents)
        if key.code == KeyCode::Char('!') {
            self.app_state.stats_filter = crate::model::StatsFilter::Global;
            return false;
        }

        // '@' - Filter stats to Main Agent only
        if key.code == KeyCode::Char('@') {
            self.app_state.stats_filter = crate::model::StatsFilter::MainAgent;
            return false;
        }

        // '#' - Filter stats to current Subagent (if tab selected)
        if key.code == KeyCode::Char('#') {
            if let Some(tab_index) = self.app_state.selected_tab {
                let subagent_ids = self.app_state.session().subagent_ids_ordered();
                if let Some(&agent_id) = subagent_ids.get(tab_index) {
                    self.app_state.stats_filter =
                        crate::model::StatsFilter::Subagent(agent_id.clone());
                }
            }
            return false;
        }

        // Quit on 'q' or Ctrl+C
        matches!(key.code, KeyCode::Char('q'))
            || (key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL))
    }

    /// Render the current frame
    fn draw(&mut self) -> Result<(), TuiError> {
        self.terminal.draw(|frame| {
            layout::render_layout(frame, &self.app_state);
        })?;
        Ok(())
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

        TuiApp {
            terminal,
            app_state,
            input_source,
            line_counter: 0,
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

        // Press '!' to set Global filter
        let key = KeyEvent::new(KeyCode::Char('!'), KeyModifiers::NONE);
        let should_quit = app.handle_key(key);

        assert!(!should_quit, "'!' should not trigger quit");
        assert_eq!(
            app.app_state.stats_filter,
            crate::model::StatsFilter::Global,
            "'!' should set stats filter to Global"
        );
    }

    #[test]
    fn handle_key_at_sets_main_agent_filter() {
        let mut app = create_test_app();

        // Set to Global initially
        app.app_state.stats_filter = crate::model::StatsFilter::Global;

        // Press '@' to set MainAgent filter
        let key = KeyEvent::new(KeyCode::Char('@'), KeyModifiers::NONE);
        let should_quit = app.handle_key(key);

        assert!(!should_quit, "'@' should not trigger quit");
        assert_eq!(
            app.app_state.stats_filter,
            crate::model::StatsFilter::MainAgent,
            "'@' should set stats filter to MainAgent"
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

        // Press '#' to set Subagent filter for the selected tab
        let key = KeyEvent::new(KeyCode::Char('#'), KeyModifiers::NONE);
        let should_quit = app.handle_key(key);

        assert!(!should_quit, "'#' should not trigger quit");
        assert_eq!(
            app.app_state.stats_filter,
            crate::model::StatsFilter::Subagent(agent_id),
            "'#' should set stats filter to Subagent with selected tab's agent ID"
        );
    }

    #[test]
    fn handle_key_hash_does_nothing_when_no_tab_selected() {
        let mut app = create_test_app();

        // No tab selected
        app.app_state.selected_tab = None;

        // Set to Global initially
        app.app_state.stats_filter = crate::model::StatsFilter::Global;

        // Press '#' when no tab is selected
        let key = KeyEvent::new(KeyCode::Char('#'), KeyModifiers::NONE);
        let should_quit = app.handle_key(key);

        assert!(!should_quit, "'#' should not trigger quit");
        assert_eq!(
            app.app_state.stats_filter,
            crate::model::StatsFilter::Global,
            "'#' should not change filter when no tab is selected"
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
}
