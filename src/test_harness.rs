//! Acceptance Test Harness for TUI testing
//!
//! Provides a high-level API for acceptance testing user stories by wrapping
//! TuiApp<TestBackend> with convenient methods for simulating user interactions.

use crate::config::keybindings::KeyBindings;
use crate::source::{FileSource, StdinSource};
use crate::state::AppState;
use crate::view::{TuiApp, TuiError};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use std::path::PathBuf;

/// Convert a ratatui buffer to a string representation for snapshot testing.
///
/// Captures the visual output character by character, preserving layout.
/// Empty trailing lines are removed to keep snapshots clean.
#[allow(dead_code)]
fn buffer_to_string(buffer: &ratatui::buffer::Buffer) -> String {
    let area = buffer.area();
    let mut lines = Vec::new();

    for y in area.top()..area.bottom() {
        let mut line = String::new();
        for x in area.left()..area.right() {
            let cell = &buffer[(x, y)];
            line.push_str(cell.symbol());
        }
        let trimmed = line.trim_end();
        if !trimmed.is_empty() {
            lines.push(trimmed.to_string());
        }
    }

    lines.join("\n")
}

/// Test harness for acceptance testing
///
/// Wraps TuiApp<TestBackend> to provide a clean API for simulating user
/// interactions in acceptance tests.
pub struct AcceptanceTestHarness {
    app: TuiApp<TestBackend>,
    #[allow(dead_code)] // Stored for potential future use
    width: u16,
    #[allow(dead_code)] // Stored for potential future use
    height: u16,
    running: bool,
}

impl AcceptanceTestHarness {
    /// Load fixture into test app with default terminal size (80x24)
    ///
    /// # Arguments
    /// * `path` - Path to JSONL fixture file
    ///
    /// # Returns
    /// * `Ok(Self)` - Initialized harness with fixture loaded
    /// * `Err(TuiError)` - If fixture cannot be loaded or parsed
    #[allow(dead_code)]
    pub fn from_fixture(path: &str) -> Result<Self, TuiError> {
        Self::from_fixture_with_size(path, 80, 24)
    }

    /// Load fixture with custom terminal size
    ///
    /// # Arguments
    /// * `path` - Path to JSONL fixture file
    /// * `width` - Terminal width in columns
    /// * `height` - Terminal height in rows
    ///
    /// # Returns
    /// * `Ok(Self)` - Initialized harness with fixture loaded
    /// * `Err(TuiError)` - If fixture cannot be loaded or parsed
    pub fn from_fixture_with_size(path: &str, width: u16, height: u16) -> Result<Self, TuiError> {
        // Create test backend and terminal
        let backend = TestBackend::new(width, height);
        let terminal = Terminal::new(backend)?;

        // Load fixture file using FileSource
        let mut file_source = FileSource::new(PathBuf::from(path))?;
        let log_entries = file_source.drain_entries()?;

        // Track entry count for line counter
        let entry_count = log_entries.len();

        // Convert LogEntry to ConversationEntry
        let entries: Vec<crate::model::ConversationEntry> = log_entries
            .into_iter()
            .map(|e| crate::model::ConversationEntry::Valid(Box::new(e)))
            .collect();

        // Create app state and populate with entries
        let mut app_state = AppState::new();
        app_state.add_entries(entries);
        let key_bindings = KeyBindings::default();

        // Create dummy stdin source (won't be used for testing, but required by TuiApp)
        // Use empty buffer for stdin
        let data = b"";
        let stdin_source = StdinSource::from_reader(&data[..]);
        let input_source = crate::source::InputSource::Stdin(stdin_source);

        // Create TuiApp using test constructor
        let app =
            TuiApp::new_for_test(terminal, app_state, input_source, entry_count, key_bindings);

        Ok(Self {
            app,
            width,
            height,
            running: true,
        })
    }

    /// Send a single key event
    ///
    /// # Arguments
    /// * `key` - KeyCode to send (e.g., KeyCode::Char('j'), KeyCode::Down)
    ///
    /// # Returns
    /// * `true` - If app quit as a result of this key
    /// * `false` - If app is still running
    pub fn send_key(&mut self, key: KeyCode) -> bool {
        self.send_key_with_mods(key, KeyModifiers::NONE)
    }

    /// Send key with modifiers (e.g., Ctrl+C)
    ///
    /// # Arguments
    /// * `key` - KeyCode to send
    /// * `mods` - Key modifiers (CONTROL, SHIFT, ALT, etc.)
    ///
    /// # Returns
    /// * `true` - If app quit as a result of this key
    /// * `false` - If app is still running
    pub fn send_key_with_mods(&mut self, key: KeyCode, mods: KeyModifiers) -> bool {
        if !self.running {
            return true; // Already quit
        }

        let key_event = KeyEvent::new(key, mods);
        let quit = self.app.handle_key_test(key_event);

        if quit {
            self.running = false;
        }

        quit
    }

    /// Send a sequence of keys
    ///
    /// Continues sending keys until the sequence is exhausted or app quits.
    ///
    /// # Arguments
    /// * `keys` - Slice of KeyCodes to send in order
    #[allow(dead_code)]
    pub fn send_keys(&mut self, keys: &[KeyCode]) {
        for key in keys {
            if self.send_key(*key) {
                break; // Quit encountered
            }
        }
    }

    /// Type text (sends individual character key events)
    ///
    /// Useful for search input and other text entry scenarios.
    ///
    /// # Arguments
    /// * `text` - Text to type character by character
    #[allow(dead_code)]
    pub fn type_text(&mut self, text: &str) {
        for ch in text.chars() {
            if self.send_key(KeyCode::Char(ch)) {
                break; // Quit encountered
            }
        }
    }

    /// Access app state for assertions
    ///
    /// Provides read-only access to AppState for verifying state transitions.
    ///
    /// # Returns
    /// Reference to the current AppState
    #[allow(dead_code)]
    pub fn state(&self) -> &AppState {
        self.app.app_state()
    }

    /// Check if app is still running (didn't crash/quit)
    ///
    /// # Returns
    /// * `true` - App is running normally
    /// * `false` - App has quit or crashed
    #[allow(dead_code)]
    pub fn is_running(&self) -> bool {
        self.running
    }

    /// Render the current frame to a string
    ///
    /// Renders the app state to the TestBackend and returns the buffer
    /// contents as a string representation.
    ///
    /// # Panics
    /// Panics if rendering fails (should never happen with TestBackend)
    ///
    /// # Returns
    /// The rendered terminal buffer as a string
    #[allow(dead_code)]
    pub fn render_to_string(&mut self) -> String {
        // Render the current frame to the TestBackend
        self.app
            .render_test()
            .expect("Rendering should succeed in test harness");

        // Access the buffer from the TestBackend and convert to string
        let buffer = self.app.terminal().backend().buffer();
        buffer_to_string(buffer)
    }

    /// Assert that the current render matches a snapshot
    ///
    /// Renders the current state and uses insta to verify against
    /// a stored snapshot. Useful for regression testing UI output.
    ///
    /// # Arguments
    /// * `snapshot_name` - Name for the snapshot file
    #[allow(dead_code)]
    pub fn assert_snapshot(&mut self, snapshot_name: &str) {
        let output = self.render_to_string();
        insta::assert_snapshot!(snapshot_name, output);
    }

    /// Send a mouse click event at the specified coordinates
    ///
    /// # Arguments
    /// * `column` - X coordinate (column) of the click
    /// * `row` - Y coordinate (row) of the click
    #[allow(dead_code)]
    pub fn click_at(&mut self, column: u16, row: u16) {
        if !self.running {
            return; // Already quit
        }

        // Render first to ensure layout is calculated
        let _ = self.app.render_test();

        // Create left mouse button down event
        let mouse_event = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column,
            row,
            modifiers: KeyModifiers::NONE,
        };

        self.app.handle_mouse_test(mouse_event);
    }
}
