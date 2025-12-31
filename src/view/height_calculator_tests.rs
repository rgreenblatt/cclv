//! Tests for entry height calculation.
//!
//! Verifies that calculate_entry_height correctly computes rendered heights
//! accounting for text wrapping, markdown rendering, and expanded state.

use super::calculate_entry_height;
use crate::model::{
    ConversationEntry, EntryMetadata, EntryType, EntryUuid, LogEntry, MalformedEntry, Message,
    MessageContent, Role, SessionId,
};
use crate::state::WrapMode;
use chrono::Utc;

// ===== Test Helpers =====

fn make_text_entry(text: &str) -> ConversationEntry {
    let message = Message::new(Role::User, MessageContent::Text(text.to_string()));
    let entry = LogEntry::new(
        EntryUuid::new("test-uuid").unwrap(),
        None,
        SessionId::new("test-session").unwrap(),
        None,
        Utc::now(),
        EntryType::User,
        message,
        EntryMetadata::default(),
    );
    ConversationEntry::Valid(Box::new(entry))
}

fn make_malformed_entry() -> ConversationEntry {
    ConversationEntry::Malformed(MalformedEntry::new(
        1,
        "bad json",
        "Parse error",
        None,
    ))
}

// ===== Malformed Entry Tests =====

#[test]
fn malformed_entry_returns_nonzero_height() {
    let entry = make_malformed_entry();
    let height = calculate_entry_height(&entry, false, WrapMode::Wrap);
    assert!(
        height.get() > 0,
        "Malformed entries must return non-zero height for rendering"
    );
}

#[test]
fn malformed_entry_height_same_regardless_of_expanded() {
    let entry = make_malformed_entry();
    let collapsed = calculate_entry_height(&entry, false, WrapMode::Wrap);
    let expanded = calculate_entry_height(&entry, true, WrapMode::Wrap);
    assert_eq!(
        collapsed, expanded,
        "Malformed entry height should be same whether expanded or not"
    );
}

// ===== Valid Entry Minimum Height Tests =====

#[test]
fn valid_entry_returns_at_least_one_line() {
    let entry = make_text_entry("");
    let height = calculate_entry_height(&entry, true, WrapMode::Wrap);
    assert!(
        height.get() >= 1,
        "Valid entries must return at least LineHeight::ONE, got {}",
        height.get()
    );
}

#[test]
fn empty_text_collapsed_returns_different_than_multiline_expanded() {
    // Stub returns constant 5 for all entries - this will FAIL
    let empty = make_text_entry("");
    let multiline = make_text_entry("Line 1\nLine 2\nLine 3\nLine 4\nLine 5\nLine 6\nLine 7\nLine 8\nLine 9\nLine 10");

    let empty_collapsed = calculate_entry_height(&empty, false, WrapMode::Wrap);
    let multiline_expanded = calculate_entry_height(&multiline, true, WrapMode::Wrap);

    // Stub returns 5 for both, so this assertion will FAIL proving stub is wrong
    assert_ne!(
        empty_collapsed, multiline_expanded,
        "Empty collapsed ({}) should differ from 10-line expanded ({})",
        empty_collapsed.get(),
        multiline_expanded.get()
    );
}

#[test]
fn single_line_text_returns_appropriate_height() {
    let entry = make_text_entry("Hello");
    let height = calculate_entry_height(&entry, true, WrapMode::Wrap);
    // Should be at least 1 line, likely 2+ for role header + content
    assert!(
        height.get() >= 1,
        "Single line text should return at least 1 line"
    );
}

// ===== Wrapping Tests =====

#[test]
fn long_line_wrapped_returns_more_lines_than_no_wrap() {
    // Create text that would wrap at typical terminal width
    let long_text = "a".repeat(200); // 200 chars, will wrap multiple times at 80 cols
    let entry = make_text_entry(&long_text);

    let wrapped = calculate_entry_height(&entry, true, WrapMode::Wrap);
    let no_wrap = calculate_entry_height(&entry, true, WrapMode::NoWrap);

    // With wrapping at 80 cols, 200 chars should wrap to at least 3 lines (200/80 = 2.5)
    // Stub returns constant 5 for both, so this will FAIL on stub (not greater)
    assert!(
        wrapped > no_wrap,
        "Wrapped mode should use MORE lines than NoWrap for long text (wrapped={}, no_wrap={})",
        wrapped.get(),
        no_wrap.get()
    );
}

// ===== Expanded State Tests =====

#[test]
fn collapsed_entry_returns_smaller_height_than_expanded() {
    // Use a multi-line message that would show more when expanded
    let multiline = "Line 1\n\nLine 2\n\nLine 3\n\nLine 4\n\nLine 5";
    let entry = make_text_entry(multiline);

    let collapsed = calculate_entry_height(&entry, false, WrapMode::Wrap);
    let expanded = calculate_entry_height(&entry, true, WrapMode::Wrap);

    // Collapsed should show summary only (typically 1-3 lines)
    // Expanded should show full content
    assert!(
        collapsed <= expanded,
        "Collapsed height ({}) should be <= expanded height ({})",
        collapsed.get(),
        expanded.get()
    );
}

#[test]
fn collapsed_entry_returns_summary_height() {
    let entry = make_text_entry("Some text");
    let collapsed = calculate_entry_height(&entry, false, WrapMode::Wrap);

    // Collapsed should return a small summary height (1-3 lines typically)
    assert!(
        collapsed.get() <= 5,
        "Collapsed summary should be small (got {} lines)",
        collapsed.get()
    );
}

// ===== Multiline Content Tests =====

#[test]
fn multiline_text_returns_height_accounting_for_lines() {
    let text = "Line 1\nLine 2\nLine 3\nLine 4\nLine 5";
    let entry = make_text_entry(text);

    let height = calculate_entry_height(&entry, true, WrapMode::Wrap);

    // Should account for all lines plus role header
    // Minimum 5 lines of content (may be more with headers/formatting)
    assert!(
        height.get() >= 5,
        "Multiline text should account for line count (got {})",
        height.get()
    );
}

// ===== Determinism Tests =====

#[test]
fn height_calculation_is_deterministic() {
    let entry = make_text_entry("Deterministic test");

    let height1 = calculate_entry_height(&entry, true, WrapMode::Wrap);
    let height2 = calculate_entry_height(&entry, true, WrapMode::Wrap);
    let height3 = calculate_entry_height(&entry, true, WrapMode::Wrap);

    assert_eq!(
        height1, height2,
        "Same inputs should produce same height (call 1 vs 2)"
    );
    assert_eq!(
        height2, height3,
        "Same inputs should produce same height (call 2 vs 3)"
    );
}

#[test]
fn different_wrap_modes_may_produce_different_heights() {
    let long_text = "a".repeat(150);
    let entry = make_text_entry(&long_text);

    let wrap = calculate_entry_height(&entry, true, WrapMode::Wrap);
    let _no_wrap = calculate_entry_height(&entry, true, WrapMode::NoWrap);

    // Determinism: same mode always gives same result
    let wrap2 = calculate_entry_height(&entry, true, WrapMode::Wrap);
    assert_eq!(wrap, wrap2, "Wrap mode should be deterministic");

    // Note: wrap >= no_wrap is expected for long lines
    // (wrapping creates more vertical lines)
}
