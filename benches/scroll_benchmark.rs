//! Scroll performance benchmarks for SC-002 validation.
//!
//! These benchmarks verify that scroll operations complete within acceptable
//! time bounds for large log files (181MB, 31k lines).
//!
//! Run with: cargo bench --bench scroll_benchmark

#![allow(missing_docs)] // criterion macros generate undocumented items

use cclv::config::keybindings::KeyBindings;
use cclv::source::{FileSource, InputSource, StdinSource};
use cclv::state::app_state::WrapMode;
use cclv::state::AppState;
use cclv::view::TuiApp;
use criterion::{
    black_box, criterion_group, criterion_main, BenchmarkId, Criterion, BatchSize,
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use std::path::PathBuf;

/// Scroll position in the document.
#[derive(Debug, Clone, Copy)]
enum ScrollPosition {
    Start,       // 0%
    Quarter,     // 25%
    Middle,      // 50%
    ThreeQuarters, // 75%
    End,         // Near bottom
}

impl ScrollPosition {
    /// Get the position name for benchmark IDs.
    fn name(&self) -> &'static str {
        match self {
            ScrollPosition::Start => "start",
            ScrollPosition::Quarter => "quarter",
            ScrollPosition::Middle => "middle",
            ScrollPosition::ThreeQuarters => "three_quarters",
            ScrollPosition::End => "end",
        }
    }

    /// Calculate the number of scroll actions needed to reach this position.
    ///
    /// Assumes viewport height of 60 lines.
    fn actions_from_start(&self, total_lines: usize) -> usize {
        let viewport_height = 60;
        match self {
            ScrollPosition::Start => 0,
            ScrollPosition::Quarter => (total_lines / 4) / viewport_height,
            ScrollPosition::Middle => (total_lines / 2) / viewport_height,
            ScrollPosition::ThreeQuarters => (total_lines * 3 / 4) / viewport_height,
            ScrollPosition::End => total_lines.saturating_sub(viewport_height) / viewport_height,
        }
    }
}

/// Load the large fixture file and create a baseline app state.
///
/// This is expensive, so we do it once and clone for each benchmark iteration.
fn load_fixture() -> (AppState, usize) {
    let fixture_path = PathBuf::from("tests/fixtures/cc-session-log.jsonl");

    // Load fixture file using FileSource
    let mut file_source = FileSource::new(fixture_path)
        .expect("Failed to load fixture file");
    let log_entries = file_source.drain_entries()
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

/// Scroll the app to the specified position using PageDown.
///
/// This is part of setup, not measured.
fn scroll_to_position(app: &mut TuiApp<TestBackend>, position: ScrollPosition, total_lines: usize) {
    let actions = position.actions_from_start(total_lines);
    for _ in 0..actions {
        app.handle_key_bench(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE));
    }
}

/// Benchmark vertical line scrolling (single line down).
fn benchmark_line_scroll_down(c: &mut Criterion) {
    let (baseline_state, entry_count) = load_fixture();

    // Get total lines from state for position calculation
    let total_lines = baseline_state.log_view().current_session()
        .map(|s| s.main().total_height())
        .unwrap_or(0);

    println!("Loaded fixture: {} entries, {} total lines", entry_count, total_lines);

    let mut group = c.benchmark_group("line_scroll_down");

    for position in [
        ScrollPosition::Start,
        ScrollPosition::Quarter,
        ScrollPosition::Middle,
        ScrollPosition::ThreeQuarters,
        ScrollPosition::End,
    ] {
        // Test both wrap modes
        for wrap_mode in [WrapMode::Wrap, WrapMode::NoWrap] {
            let mode_name = match wrap_mode {
                WrapMode::Wrap => "wrap",
                WrapMode::NoWrap => "nowrap",
            };
            let bench_name = format!("{}_{}", position.name(), mode_name);

            group.bench_with_input(
                BenchmarkId::new("position", bench_name),
                &(position, wrap_mode),
                |b, &(pos, wrap)| {
                    b.iter_batched(
                        || {
                            // SETUP (outside timing): clone state, create app, pre-render, scroll to position
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

                            // Scroll to starting position (not measured)
                            scroll_to_position(&mut app, pos, total_lines);

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
    }

    group.finish();
}

/// Benchmark vertical line scrolling (single line up).
fn benchmark_line_scroll_up(c: &mut Criterion) {
    let (baseline_state, entry_count) = load_fixture();

    let total_lines = baseline_state.log_view().current_session()
        .map(|s| s.main().total_height())
        .unwrap_or(0);

    let mut group = c.benchmark_group("line_scroll_up");

    for position in [
        ScrollPosition::Quarter,
        ScrollPosition::Middle,
        ScrollPosition::ThreeQuarters,
        ScrollPosition::End,
    ] {
        for wrap_mode in [WrapMode::Wrap, WrapMode::NoWrap] {
            let mode_name = match wrap_mode {
                WrapMode::Wrap => "wrap",
                WrapMode::NoWrap => "nowrap",
            };
            let bench_name = format!("{}_{}", position.name(), mode_name);

            group.bench_with_input(
                BenchmarkId::new("position", bench_name),
                &(position, wrap_mode),
                |b, &(pos, wrap)| {
                    b.iter_batched(
                        || {
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

                            scroll_to_position(&mut app, pos, total_lines);
                            app.render_bench().unwrap();
                            app
                        },
                        |mut app| {
                            app.handle_key_bench(KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE));
                            app.render_bench().unwrap();
                            black_box(app.terminal_bench().backend().buffer().clone())
                        },
                        BatchSize::SmallInput,
                    );
                },
            );
        }
    }

    group.finish();
}

/// Benchmark page scrolling (page down).
fn benchmark_page_scroll_down(c: &mut Criterion) {
    let (baseline_state, entry_count) = load_fixture();

    let total_lines = baseline_state.log_view().current_session()
        .map(|s| s.main().total_height())
        .unwrap_or(0);

    let mut group = c.benchmark_group("page_scroll_down");

    for position in [
        ScrollPosition::Start,
        ScrollPosition::Quarter,
        ScrollPosition::Middle,
        ScrollPosition::ThreeQuarters,
    ] {
        for wrap_mode in [WrapMode::Wrap, WrapMode::NoWrap] {
            let mode_name = match wrap_mode {
                WrapMode::Wrap => "wrap",
                WrapMode::NoWrap => "nowrap",
            };
            let bench_name = format!("{}_{}", position.name(), mode_name);

            group.bench_with_input(
                BenchmarkId::new("position", bench_name),
                &(position, wrap_mode),
                |b, &(pos, wrap)| {
                    b.iter_batched(
                        || {
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

                            scroll_to_position(&mut app, pos, total_lines);
                            app.render_bench().unwrap();
                            app
                        },
                        |mut app| {
                            app.handle_key_bench(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE));
                            app.render_bench().unwrap();
                            black_box(app.terminal_bench().backend().buffer().clone())
                        },
                        BatchSize::SmallInput,
                    );
                },
            );
        }
    }

    group.finish();
}

/// Benchmark page scrolling (page up).
fn benchmark_page_scroll_up(c: &mut Criterion) {
    let (baseline_state, entry_count) = load_fixture();

    let total_lines = baseline_state.log_view().current_session()
        .map(|s| s.main().total_height())
        .unwrap_or(0);

    let mut group = c.benchmark_group("page_scroll_up");

    for position in [
        ScrollPosition::Quarter,
        ScrollPosition::Middle,
        ScrollPosition::ThreeQuarters,
        ScrollPosition::End,
    ] {
        for wrap_mode in [WrapMode::Wrap, WrapMode::NoWrap] {
            let mode_name = match wrap_mode {
                WrapMode::Wrap => "wrap",
                WrapMode::NoWrap => "nowrap",
            };
            let bench_name = format!("{}_{}", position.name(), mode_name);

            group.bench_with_input(
                BenchmarkId::new("position", bench_name),
                &(position, wrap_mode),
                |b, &(pos, wrap)| {
                    b.iter_batched(
                        || {
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

                            scroll_to_position(&mut app, pos, total_lines);
                            app.render_bench().unwrap();
                            app
                        },
                        |mut app| {
                            app.handle_key_bench(KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE));
                            app.render_bench().unwrap();
                            black_box(app.terminal_bench().backend().buffer().clone())
                        },
                        BatchSize::SmallInput,
                    );
                },
            );
        }
    }

    group.finish();
}

/// Benchmark horizontal scrolling (right).
fn benchmark_horizontal_scroll_right(c: &mut Criterion) {
    let (baseline_state, entry_count) = load_fixture();

    let total_lines = baseline_state.log_view().current_session()
        .map(|s| s.main().total_height())
        .unwrap_or(0);

    let mut group = c.benchmark_group("horizontal_scroll_right");

    for position in [
        ScrollPosition::Start,
        ScrollPosition::Quarter,
        ScrollPosition::Middle,
        ScrollPosition::ThreeQuarters,
        ScrollPosition::End,
    ] {
        group.bench_with_input(
            BenchmarkId::new("position", position.name()),
            &position,
            |b, &pos| {
                b.iter_batched(
                    || {
                        let mut state = baseline_state.clone();
                        state.global_wrap = WrapMode::NoWrap; // Horizontal scroll only in NoWrap mode

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

                        scroll_to_position(&mut app, pos, total_lines);
                        app.render_bench().unwrap();
                        app
                    },
                    |mut app| {
                        app.handle_key_bench(KeyEvent::new(KeyCode::Char('l'), KeyModifiers::NONE));
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

/// Benchmark horizontal scrolling (left).
fn benchmark_horizontal_scroll_left(c: &mut Criterion) {
    let (baseline_state, entry_count) = load_fixture();

    let total_lines = baseline_state.log_view().current_session()
        .map(|s| s.main().total_height())
        .unwrap_or(0);

    let mut group = c.benchmark_group("horizontal_scroll_left");

    for position in [
        ScrollPosition::Start,
        ScrollPosition::Quarter,
        ScrollPosition::Middle,
        ScrollPosition::ThreeQuarters,
        ScrollPosition::End,
    ] {
        group.bench_with_input(
            BenchmarkId::new("position", position.name()),
            &position,
            |b, &pos| {
                b.iter_batched(
                    || {
                        let mut state = baseline_state.clone();
                        state.global_wrap = WrapMode::NoWrap;

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

                        scroll_to_position(&mut app, pos, total_lines);

                        // Scroll right a few times first (so left has something to do)
                        for _ in 0..10 {
                            app.handle_key_bench(KeyEvent::new(KeyCode::Char('l'), KeyModifiers::NONE));
                        }

                        app.render_bench().unwrap();
                        app
                    },
                    |mut app| {
                        app.handle_key_bench(KeyEvent::new(KeyCode::Char('h'), KeyModifiers::NONE));
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
    targets =
        benchmark_line_scroll_down,
        benchmark_line_scroll_up,
        benchmark_page_scroll_down,
        benchmark_page_scroll_up,
        benchmark_horizontal_scroll_right,
        benchmark_horizontal_scroll_left
}

criterion_main!(benches);
