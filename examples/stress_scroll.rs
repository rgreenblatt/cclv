//! Stress test for scroll profiling.
//!
//! Extracts the hot loop from benches/scroll_benchmark.rs for flamegraph profiling.
//!
//! Run with:
//!   cargo run --example stress_scroll --release --features bench-internals -- [iterations]
//!
//! Profile with cargo-flamegraph:
//!   cargo flamegraph --example stress_scroll --features bench-internals -- 1000

use cclv::config::keybindings::KeyBindings;
use cclv::source::{FileSource, InputSource, StdinSource};
use cclv::state::app_state::WrapMode;
use cclv::state::AppState;
use cclv::view::TuiApp;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use std::path::PathBuf;

/// Load the large fixture file and create a baseline app state.
fn load_fixture() -> (AppState, usize) {
    let fixture_path = PathBuf::from("tests/fixtures/cc-session-log.jsonl");

    let mut file_source = FileSource::new(fixture_path).expect("Failed to load fixture file");
    let log_entries = file_source
        .drain_entries()
        .expect("Failed to parse fixture entries");

    let entry_count = log_entries.len();

    let entries: Vec<cclv::model::ConversationEntry> = log_entries
        .into_iter()
        .map(|e| cclv::model::ConversationEntry::Valid(Box::new(e)))
        .collect();

    let mut app_state = AppState::new();
    app_state.add_entries(entries);

    (app_state, entry_count)
}

/// Create a dummy stdin InputSource for testing.
fn create_dummy_input_source() -> InputSource {
    let data = b"";
    let stdin_source = StdinSource::from_reader(&data[..]);
    InputSource::Stdin(stdin_source)
}

/// Scroll the app to the middle of the conversation using PageDown.
fn scroll_to_middle(app: &mut TuiApp<TestBackend>, total_lines: usize) {
    let viewport_height = 60;
    let actions = (total_lines / 2) / viewport_height;
    for _ in 0..actions {
        app.handle_key_bench(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE));
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let iterations: usize = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(1000);

    eprintln!("Loading fixture...");
    let (baseline_state, entry_count) = load_fixture();

    let total_lines = baseline_state
        .log_view()
        .current_session()
        .map(|s| s.main().total_height())
        .unwrap_or(0);

    eprintln!(
        "Loaded: {} entries, {} total lines",
        entry_count, total_lines
    );

    // Setup - matches benchmark configuration
    let mut state = baseline_state.clone();
    state.global_wrap = WrapMode::Wrap;

    let backend = TestBackend::new(200, 60);
    let terminal = Terminal::new(backend).unwrap();
    let input_source = create_dummy_input_source();
    let key_bindings = KeyBindings::default();
    let mut app = TuiApp::new_for_bench(terminal, state, input_source, entry_count, key_bindings);

    // Scroll to middle (setup, not measured)
    scroll_to_middle(&mut app, total_lines);
    app.render_bench().unwrap();

    eprintln!("Running {} scroll iterations...", iterations);

    // Hot loop - matches benchmark exactly:
    // single line scroll (j) + re-render
    for i in 0..iterations {
        app.handle_key_bench(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE));
        app.render_bench().unwrap();

        if (i + 1) % 100 == 0 {
            eprintln!("  {} / {}", i + 1, iterations);
        }
    }

    eprintln!("Done.");
}
