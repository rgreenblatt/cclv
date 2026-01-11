//! Search performance benchmarks for SC-003 validation.
//!
//! SC-003 requirement: Search results appear within 1 second for log files up to 50MB
//!
//! Run with: cargo bench

#![allow(missing_docs)] // criterion macros generate undocumented items

use cclv::model::{
    AgentId, ContentBlock, ConversationEntry, EntryMetadata, EntryType, EntryUuid, LogEntry,
    Message, MessageContent, Role, SessionId,
};
use cclv::state::search::{SearchQuery, execute_search};
use cclv::view_state::session::SessionViewState;
use chrono::Utc;
use criterion::{Criterion, black_box, criterion_group, criterion_main};

/// Generate a large session with ~50MB of text content.
///
/// Strategy:
/// - Create entries with text blocks of varying sizes
/// - Target ~50MB total by generating enough entries with substantial text
/// - Include both main agent and subagent entries
/// - Mix different content types (text, thinking, tool results)
fn generate_large_session() -> SessionViewState {
    let session_id = SessionId::new("benchmark-session").expect("valid session id");
    let mut session_view = SessionViewState::new(session_id.clone());

    // Estimate: Each entry with ~10KB text = ~5,000 entries for 50MB
    // To be safe and hit 50MB, generate 6,000 entries
    const NUM_ENTRIES: usize = 6_000;
    const TEXT_SIZE_PER_ENTRY: usize = 10_000; // ~10KB per entry

    // Create a large text block template
    let text_template = "Lorem ipsum dolor sit amet, consectetur adipiscing elit. "
        .repeat(TEXT_SIZE_PER_ENTRY / 60);

    for i in 0..NUM_ENTRIES {
        let uuid = EntryUuid::new(format!("entry-{:06}", i)).expect("valid uuid");
        let timestamp = Utc::now();

        // Vary content types for realism
        let content = if i % 3 == 0 {
            // Text block
            MessageContent::Blocks(vec![ContentBlock::Text {
                text: format!("{} Entry {}", text_template, i),
            }])
        } else if i % 3 == 1 {
            // Thinking block
            MessageContent::Blocks(vec![ContentBlock::Thinking {
                thinking: format!("Thinking about {}. {}", i, text_template),
            }])
        } else {
            // Text message
            MessageContent::Text(format!("Message {}. {}", i, text_template))
        };

        // Alternate between user and assistant
        let role = if i % 2 == 0 {
            Role::User
        } else {
            Role::Assistant
        };

        let entry_type = if role == Role::User {
            EntryType::User
        } else {
            EntryType::Assistant
        };

        let message = Message::new(role, content);

        // Create some subagent entries (every 10th entry)
        let agent_id = if i % 10 == 0 {
            Some(AgentId::new(format!("agent-{}", i / 10)).expect("valid agent id"))
        } else {
            None
        };

        let entry = LogEntry::new(
            uuid,
            None,
            session_id.clone(),
            agent_id.clone(),
            timestamp,
            entry_type,
            message,
            EntryMetadata::default(),
        );

        let conv_entry = ConversationEntry::Valid(Box::new(entry));

        // Add to appropriate conversation
        if let Some(aid) = agent_id {
            session_view.add_subagent_entry(aid, conv_entry);
        } else {
            session_view.add_main_entry(conv_entry);
        }
    }

    session_view
}

/// Benchmark search performance on large session data.
fn benchmark_search(c: &mut Criterion) {
    // Generate session once (expensive, don't time this)
    let session = generate_large_session();

    // Verify we have substantial data
    let main_entries = session.main().len();
    let subagent_count = session.subagents().len();
    println!(
        "Benchmark session: {} main entries, {} subagents",
        main_entries, subagent_count
    );

    // Estimate size (rough)
    let estimated_size_mb = (main_entries * 10_000) / (1024 * 1024);
    println!("Estimated session size: ~{}MB", estimated_size_mb);

    c.bench_function("search_50mb_common_term", |b| {
        b.iter(|| {
            // Search for a common term that will match many entries
            let query = SearchQuery::new("Lorem").expect("valid query");
            let matches = execute_search(black_box(&session), black_box(&query));
            black_box(matches)
        })
    });

    c.bench_function("search_50mb_rare_term", |b| {
        b.iter(|| {
            // Search for a rare term that matches few entries
            let query = SearchQuery::new("Entry 1000").expect("valid query");
            let matches = execute_search(black_box(&session), black_box(&query));
            black_box(matches)
        })
    });

    c.bench_function("search_50mb_no_match", |b| {
        b.iter(|| {
            // Search for a term that never matches
            let query = SearchQuery::new("XYZNONEXISTENT").expect("valid query");
            let matches = execute_search(black_box(&session), black_box(&query));
            black_box(matches)
        })
    });
}

criterion_group! {
    name = benches;
    config = Criterion::default()
        // SC-003: Search must complete in <1s for 50MB files
        // Set measurement time to give accurate results
        .measurement_time(std::time::Duration::from_secs(10));
    targets = benchmark_search
}

criterion_main!(benches);
