//! Hit-test performance benchmarks for O(log n) verification.
//!
//! These benchmarks verify that hit_test performance scales logarithmically
//! with the number of entries, achieving <1ms response time even with 100k entries.
//!
//! Run with: cargo bench --bench hit_test

#![allow(missing_docs)] // criterion macros generate undocumented items

use cclv::model::{
    ConversationEntry, EntryMetadata, EntryType, EntryUuid, LogEntry, Message, MessageContent,
    Role, SessionId,
};
use cclv::state::app_state::WrapMode;
use cclv::view_state::conversation::ConversationViewState;
use cclv::view_state::layout_params::LayoutParams;
use cclv::view_state::types::LineOffset;
use chrono::Utc;
use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};

/// Create a simple valid entry for benchmarking.
fn make_entry(uuid: &str) -> ConversationEntry {
    let log_entry = LogEntry::new(
        EntryUuid::new(uuid.to_string()).expect("valid uuid"),
        None,
        SessionId::new("benchmark-session").expect("valid session id"),
        None,
        Utc::now(),
        EntryType::User,
        Message::new(Role::User, MessageContent::Text("Test message".to_string())),
        EntryMetadata::default(),
    );
    ConversationEntry::Valid(Box::new(log_entry))
}

/// Generate a conversation state with the specified number of entries.
fn generate_conversation_state(num_entries: usize) -> ConversationViewState {
    let entries: Vec<ConversationEntry> = (0..num_entries)
        .map(|i| make_entry(&format!("uuid-{}", i)))
        .collect();

    let mut state = ConversationViewState::new(
        None,
        None,
        entries,
        200_000,
        cclv::model::PricingConfig::default(),
    );

    let params = LayoutParams::new(80, WrapMode::Wrap);
    state.recompute_layout(params);

    state
}

/// Benchmark hit_test with varying numbers of entries to verify O(log n) scaling.
fn benchmark_hit_test_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("hit_test_scaling");

    // Test with different entry counts to verify logarithmic scaling
    for num_entries in [1_000, 10_000, 100_000] {
        let state = generate_conversation_state(num_entries);
        let total_height = state.total_height();

        println!(
            "Generated {} entries, total height: {} lines",
            num_entries, total_height
        );

        group.bench_with_input(
            BenchmarkId::new("hit_test", num_entries),
            &state,
            |b, state| {
                b.iter(|| {
                    // Test various positions in the document
                    let positions = [
                        (0, 0),                               // Start
                        (total_height / 4, 10),               // 25%
                        (total_height / 2, 25),               // 50%
                        (total_height * 3 / 4, 40),           // 75%
                        (total_height.saturating_sub(1), 50), // End
                    ];

                    for &(absolute_y, column) in &positions {
                        let screen_y = (absolute_y % 1000) as u16;
                        let scroll_offset = LineOffset::new(absolute_y - (absolute_y % 1000));
                        let _result = state.hit_test(
                            black_box(screen_y),
                            black_box(column),
                            black_box(scroll_offset),
                        );
                    }
                });
            },
        );
    }

    group.finish();
}

/// Benchmark hit_test at different positions in a 100k entry document.
fn benchmark_hit_test_positions(c: &mut Criterion) {
    let state = generate_conversation_state(100_000);
    let total_height = state.total_height();

    let mut group = c.benchmark_group("hit_test_positions_100k");

    let test_positions = [
        ("start", 0),
        ("quarter", total_height / 4),
        ("middle", total_height / 2),
        ("three_quarters", total_height * 3 / 4),
        ("end", total_height.saturating_sub(1)),
    ];

    for (name, absolute_y) in test_positions {
        group.bench_with_input(
            BenchmarkId::new("position", name),
            &absolute_y,
            |b, &absolute_y| {
                b.iter(|| {
                    let screen_y = (absolute_y % 1000) as u16;
                    let scroll_offset = LineOffset::new(absolute_y - (absolute_y % 1000));
                    state.hit_test(black_box(screen_y), black_box(25), black_box(scroll_offset))
                });
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
    targets = benchmark_hit_test_scaling, benchmark_hit_test_positions
}

criterion_main!(benches);
