//! Tests for HeightIndex integration in ConversationViewState
//!
//! These tests verify that the HeightIndex is properly integrated and maintained
//! by ConversationViewState mutation methods.

use super::*;
use crate::model::{
    EntryMetadata, EntryType, EntryUuid, LogEntry, Message, MessageContent, Role, SessionId,
};

// ===== Test Helpers =====

fn make_session_id(s: &str) -> SessionId {
    SessionId::new(s).expect("valid session id")
}

fn make_entry_uuid(s: &str) -> EntryUuid {
    EntryUuid::new(s).expect("valid uuid")
}

fn make_timestamp() -> chrono::DateTime<chrono::Utc> {
    "2025-12-25T10:00:00Z".parse().expect("valid timestamp")
}

fn make_message(text: &str) -> Message {
    Message::new(Role::User, MessageContent::Text(text.to_string()))
}

fn make_valid_entry(uuid: &str) -> ConversationEntry {
    let log_entry = LogEntry::new(
        make_entry_uuid(uuid),
        None,
        make_session_id("session-1"),
        None,
        make_timestamp(),
        EntryType::User,
        make_message("Test message"),
        EntryMetadata::default(),
    );
    ConversationEntry::Valid(Box::new(log_entry))
}

// ===== HeightIndex Integration Tests =====

#[test]
fn new_state_has_empty_height_index() {
    let state = ConversationViewState::new(None, None, vec![]);

    // HeightIndex should be initialized but empty
    assert_eq!(state.height_index.len(), 0);
    assert_eq!(state.height_index.total(), 0);
}

#[test]
fn relayout_populates_height_index() {
    let entries = vec![
        make_valid_entry("uuid-1"),
        make_valid_entry("uuid-2"),
        make_valid_entry("uuid-3"),
    ];
    let mut state = ConversationViewState::new(None, None, entries);

    // Before relayout: index should be empty
    assert_eq!(state.height_index.len(), 0);

    // Relayout with width=80, wrap=Wrap
    state.relayout(80, WrapMode::Wrap);

    // After relayout: index should match entries
    assert_eq!(state.height_index.len(), 3);
    assert_eq!(state.total_height(), state.height_index.total());

    // Verify invariant: height_index[i] == entries[i].height()
    for i in 0..3 {
        let expected_height = state.entries[i].height().get() as usize;
        let actual_height = if i == 0 {
            state.height_index.prefix_sum(0)
        } else {
            state.height_index.prefix_sum(i) - state.height_index.prefix_sum(i - 1)
        };
        assert_eq!(
            actual_height, expected_height,
            "HeightIndex[{}] should match entry height",
            i
        );
    }
}

#[test]
fn toggle_entry_expanded_updates_height_index() {
    let entries = vec![
        make_valid_entry("uuid-1"),
        make_valid_entry("uuid-2"),
        make_valid_entry("uuid-3"),
    ];
    let mut state = ConversationViewState::new(None, None, entries);

    // Initial relayout
    state.relayout(80, WrapMode::Wrap);

    let height_before = state.entries[1].height().get() as usize;
    let total_before = state.total_height();

    // Toggle expand on entry 1
    state.toggle_entry_expanded(1);

    let height_after = state.entries[1].height().get() as usize;
    let total_after = state.total_height();

    // NOTE: For simple test messages, height may not change on toggle
    // (entry is already fully visible). The important thing is that
    // HeightIndex stays in sync regardless.

    // Total height should be consistent with individual heights
    let expected_total = total_before - height_before + height_after;
    assert_eq!(total_after, expected_total, "Total height should be updated");

    // Verify HeightIndex is in sync
    let index_height = if 1 == 0 {
        state.height_index.prefix_sum(1)
    } else {
        state.height_index.prefix_sum(1) - state.height_index.prefix_sum(0)
    };
    assert_eq!(
        index_height, height_after,
        "HeightIndex should reflect new height"
    );

    // Also verify total from HeightIndex matches total_height
    assert_eq!(state.total_height(), state.height_index.total());
}

#[test]
fn set_entry_wrap_override_updates_height_index() {
    let entries = vec![
        make_valid_entry("uuid-1"),
        make_valid_entry("uuid-2"),
    ];
    let mut state = ConversationViewState::new(None, None, entries);

    // Initial relayout with Wrap mode
    state.relayout(80, WrapMode::Wrap);

    let _height_before = state.entries[0].height().get() as usize;

    // Set wrap override to NoWrap
    state.set_entry_wrap_override(0, Some(WrapMode::NoWrap));

    let height_after = state.entries[0].height().get() as usize;

    // Height might change (depending on content)
    // Verify HeightIndex is in sync regardless
    let index_height = state.height_index.prefix_sum(0);
    assert_eq!(
        index_height, height_after,
        "HeightIndex should reflect height after wrap override"
    );
}

#[test]
fn append_entries_updates_height_index() {
    let mut state = ConversationViewState::new(None, None, vec![make_valid_entry("uuid-1")]);

    // Initial relayout
    state.relayout(80, WrapMode::Wrap);
    assert_eq!(state.height_index.len(), 1);

    let total_before = state.total_height();

    // Append new entries
    let new_entries = vec![make_valid_entry("uuid-2"), make_valid_entry("uuid-3")];
    state.append_entries(new_entries);

    // HeightIndex should be updated
    assert_eq!(state.height_index.len(), 3);

    // Total should include new entries
    let total_after = state.total_height();
    assert!(
        total_after > total_before,
        "Total height should increase after append"
    );

    // Verify new entries are in index
    for i in 0..3 {
        let expected_height = state.entries[i].height().get() as usize;
        let actual_height = if i == 0 {
            state.height_index.prefix_sum(0)
        } else {
            state.height_index.prefix_sum(i) - state.height_index.prefix_sum(i - 1)
        };
        assert_eq!(
            actual_height, expected_height,
            "HeightIndex[{}] should match entry height after append",
            i
        );
    }
}

#[test]
fn visible_range_uses_height_index() {
    let entries: Vec<_> = (0..10)
        .map(|i| make_valid_entry(&format!("uuid-{}", i)))
        .collect();
    let mut state = ConversationViewState::new(None, None, entries);

    state.relayout(80, WrapMode::Wrap);

    let viewport = ViewportDimensions::new(80, 24);
    let range = state.visible_range(viewport);

    // Should return a valid range
    assert!(range.start_index.get() < state.entries.len());

    // The range should be computed using HeightIndex (O(log n))
    // We can't directly verify the algorithm, but we can verify correctness:
    // All entries in range should overlap with viewport
    let scroll = state.scroll().resolve(
        state.total_height(),
        viewport.height as usize,
        |idx| state.entry_cumulative_y(idx)
    );
    let viewport_top = scroll.get();
    let viewport_bottom = viewport_top + viewport.height as usize;

    for i in range.start_index.get()..range.end_index.get() {
        let entry = &state.entries[i];
        let entry_top = entry.layout().cumulative_y().get();
        let entry_bottom = entry.layout().bottom_y().get();

        // Entry should overlap viewport
        assert!(
            entry_bottom > viewport_top && entry_top < viewport_bottom,
            "Entry {} in range should overlap viewport",
            i
        );
    }
}

#[test]
fn total_height_equals_height_index_total() {
    let entries = vec![
        make_valid_entry("uuid-1"),
        make_valid_entry("uuid-2"),
        make_valid_entry("uuid-3"),
    ];
    let mut state = ConversationViewState::new(None, None, entries);

    state.relayout(80, WrapMode::Wrap);

    // Core invariant: total_height() == height_index.total()
    assert_eq!(state.total_height(), state.height_index.total());

    // After mutation, invariant should still hold
    state.toggle_entry_expanded(1);
    assert_eq!(state.total_height(), state.height_index.total());

    state.set_entry_wrap_override(0, Some(WrapMode::NoWrap));
    assert_eq!(state.total_height(), state.height_index.total());
}

#[test]
fn height_index_invariant_maintained_across_operations() {
    let entries: Vec<_> = (0..5)
        .map(|i| make_valid_entry(&format!("uuid-{}", i)))
        .collect();
    let mut state = ConversationViewState::new(None, None, entries);

    state.relayout(80, WrapMode::Wrap);
    verify_height_index_invariant(&state);

    // Toggle expand
    state.toggle_entry_expanded(2);
    verify_height_index_invariant(&state);

    // Set wrap override
    state.set_entry_wrap_override(1, Some(WrapMode::NoWrap));
    verify_height_index_invariant(&state);

    // Append
    state.append_entries(vec![make_valid_entry("uuid-new")]);
    verify_height_index_invariant(&state);

    // Relayout
    state.relayout(120, WrapMode::NoWrap);
    verify_height_index_invariant(&state);
}

/// Helper to verify the core invariant: height_index[i] == entries[i].height()
fn verify_height_index_invariant(state: &ConversationViewState) {
    assert_eq!(state.height_index.len(), state.entries.len());

    for i in 0..state.entries.len() {
        let expected_height = state.entries[i].height().get() as usize;
        let actual_height = if i == 0 {
            state.height_index.prefix_sum(0)
        } else {
            state.height_index.prefix_sum(i) - state.height_index.prefix_sum(i - 1)
        };
        assert_eq!(
            actual_height, expected_height,
            "Invariant broken at index {}: height_index[{}] = {}, but entry height = {}",
            i, i, actual_height, expected_height
        );
    }
}
