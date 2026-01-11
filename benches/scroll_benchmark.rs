//! Scroll performance benchmarks for SC-002 validation.
//!
//! These benchmarks verify that scroll operations complete within acceptable
//! time bounds for large log files (181MB, 31k lines).
//!
//! Run with: cargo bench --bench scroll_benchmark

#![allow(missing_docs)] // criterion macros generate undocumented items

use cclv::config::keybindings::KeyBindings;
use cclv::source::{FileSource, InputSource, StdinSource};
use cclv::state::AppState;
use cclv::state::app_state::WrapMode;
use cclv::view::TuiApp;
use criterion::{BatchSize, BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use std::path::PathBuf;

/// Load the large fixture file and create a baseline app state.
///
/// This is expensive, so we do it once and clone for each benchmark iteration.
fn load_fixture() -> (AppState, usize) {
    let fixture_path = PathBuf::from("tests/fixtures/cc-session-log.jsonl");

    // Load fixture file using FileSource
    let mut file_source = FileSource::new(fixture_path).expect("Failed to load fixture file");
    let log_entries = file_source
        .drain_entries()
        .expect("Failed to parse fixture entries");

    // Track entry count for line counter
    let entry_count = log_entries.len();

    // Convert LogEntry to ConversationEntry
    let entries: Vec<cclv::model::ConversationEntry> = log_entries
        .into_iter()
        .map(|e| cclv::model::ConversationEntry::Valid(Box::new(e)))
        .collect();

    // Create app state and populate with entries
    let mut app_state = AppState::new();
    app_state.add_entries(entries);

    (app_state, entry_count)
}

/// Create a dummy stdin InputSource for testing.
///
/// Required by TuiApp but not actually used during benchmarks.
fn create_dummy_input_source() -> InputSource {
    let data = b"";
    let stdin_source = StdinSource::from_reader(&data[..]);
    InputSource::Stdin(stdin_source)
}

/// Scroll the app to the middle of the conversation using PageDown.
///
/// This is part of setup, not measured.
fn scroll_to_middle(app: &mut TuiApp<TestBackend>, total_lines: usize) {
    let viewport_height = 60;
    let actions = (total_lines / 2) / viewport_height;
    for _ in 0..actions {
        app.handle_key_bench(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE));
    }
}

/// Benchmark vertical line scrolling (single line down).
///
/// Tests two cases: WrapMode::Wrap and WrapMode::NoWrap, both from middle position.
fn benchmark_line_scroll_down(c: &mut Criterion) {
    let (baseline_state, entry_count) = load_fixture();

    // Get total lines from state for position calculation
    let total_lines = baseline_state
        .log_view()
        .current_session()
        .map(|s| s.main().total_height())
        .unwrap_or(0);

    println!(
        "Loaded fixture: {} entries, {} total lines",
        entry_count, total_lines
    );

    let mut group = c.benchmark_group("scroll_line_down");

    // Test both wrap modes from middle position
    for wrap_mode in [WrapMode::Wrap, WrapMode::NoWrap] {
        let bench_name = match wrap_mode {
            WrapMode::Wrap => "wrap",
            WrapMode::NoWrap => "nowrap",
        };

        group.bench_with_input(
            BenchmarkId::from_parameter(bench_name),
            &wrap_mode,
            |b, &wrap| {
                b.iter_batched(
                    || {
                        // SETUP (outside timing): clone state, create app, pre-render, scroll to middle
                        let mut state = baseline_state.clone();
                        state.global_wrap = wrap;

                        let backend = TestBackend::new(200, 60);
                        let terminal = Terminal::new(backend).unwrap();
                        let input_source = create_dummy_input_source();
                        let key_bindings = KeyBindings::default();
                        let mut app = TuiApp::new_for_bench(
                            terminal,
                            state,
                            input_source,
                            entry_count,
                            key_bindings,
                        );

                        // Scroll to middle position (not measured)
                        scroll_to_middle(&mut app, total_lines);

                        // PRE-RENDER (critical: establishes baseline)
                        app.render_bench().unwrap();
                        app
                    },
                    |mut app| {
                        // MEASUREMENT: single line scroll + re-render
                        app.handle_key_bench(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE));
                        app.render_bench().unwrap();
                        black_box(app.terminal_bench().backend().buffer().clone())
                    },
                    BatchSize::SmallInput,
                );
            },
        );
    }

    group.finish();
}

criterion_group! {
    name = benches;
    config = Criterion::default()
        // Set measurement time for accurate results
        .measurement_time(std::time::Duration::from_secs(10));
    targets = benchmark_line_scroll_down
}

criterion_main!(benches);
