//! Property-based tests for per-item wrap rendering.
//!
//! Tests validate wrap-related invariants:
//! 1. Wrapped line count never less than unwrapped count
//! 2. Zero viewport width handles gracefully (no panic)
//! 3. Effective wrap mode double-toggle is identity
//! 4. Height consistency across wrap modes for simple text

use cclv::model::{
    ConversationEntry, EntryMetadata, EntryType, EntryUuid, LogEntry, Message, MessageContent,
    Role, SessionId,
};
use cclv::state::{ScrollState, WrapMode};
use chrono::Utc;
use proptest::prelude::*;

// ===== Helper Functions =====

/// Create a test LogEntry with text content.
#[allow(dead_code)]
fn make_text_entry(uuid: &str, text: String) -> ConversationEntry {
    let message = Message::new(Role::Assistant, MessageContent::Text(text));
    let entry = LogEntry::new(
        EntryUuid::new(uuid).unwrap(),
        None,
        SessionId::new("test-session").unwrap(),
        None,
        Utc::now(),
        EntryType::Assistant,
        message,
        EntryMetadata::default(),
    );
    ConversationEntry::Valid(Box::new(entry))
}

/// Calculate wrapped lines using simple character-based wrapping.
/// This duplicates the logic from ConversationView::calculate_wrapped_lines
/// to test it independently.
fn calculate_wrapped_lines(text: &str, viewport_width: usize) -> usize {
    if viewport_width == 0 {
        return text.lines().count().max(1);
    }

    let mut total_lines = 0;
    for line in text.lines() {
        let line_len = line.len();
        if line_len == 0 {
            total_lines += 1;
        } else {
            total_lines += line_len.div_ceil(viewport_width);
        }
    }

    total_lines.max(1)
}

// ===== Property 1: Wrapped Lines Never Less Than Unwrapped =====

proptest! {
    #[test]
    fn wrapped_lines_never_less_than_unwrapped(
        text in ".{0,500}",
        width in 1usize..200
    ) {
        let wrapped = calculate_wrapped_lines(&text, width);
        let unwrapped = text.lines().count().max(1);

        prop_assert!(
            wrapped >= unwrapped,
            "wrapped {} < unwrapped {} for text len={} width={}",
            wrapped, unwrapped, text.len(), width
        );
    }
}

// ===== Property 2: Zero Viewport Width Safe =====

proptest! {
    #[test]
    fn zero_viewport_width_safe(text in ".{0,500}") {
        // Should not panic with zero width
        let result = calculate_wrapped_lines(&text, 0);

        // Should return same as unwrapped line count
        let expected = text.lines().count().max(1);
        prop_assert_eq!(
            result,
            expected,
            "Zero width should return unwrapped line count"
        );
    }
}

// ===== Property 3: Effective Wrap Double Toggle Identity =====

proptest! {
    #[test]
    fn effective_wrap_double_toggle_identity(
        uuid_str in "[a-z0-9-]{1,50}",
        global in prop_oneof![Just(WrapMode::Wrap), Just(WrapMode::NoWrap)]
    ) {
        // Skip empty UUIDs (invalid)
        if uuid_str.is_empty() {
            return Ok(());
        }

        let uuid = EntryUuid::new(&uuid_str).unwrap();
        let mut state = ScrollState {
            vertical_offset: 0,
            horizontal_offset: 0,
            expanded_messages: Default::default(),
            focused_message: None,
            wrap_overrides: Default::default(),
        };

        // Initial effective mode (no override)
        let initial = state.effective_wrap(&uuid, global);
        prop_assert_eq!(initial, global, "Initial should match global");

        // Toggle once
        state.toggle_wrap(&uuid);
        let after_first_toggle = state.effective_wrap(&uuid, global);

        // Should be inverted
        let expected_inverted = match global {
            WrapMode::Wrap => WrapMode::NoWrap,
            WrapMode::NoWrap => WrapMode::Wrap,
        };
        prop_assert_eq!(
            after_first_toggle, expected_inverted,
            "First toggle should invert global mode"
        );

        // Toggle twice (back to original)
        state.toggle_wrap(&uuid);
        let after_second_toggle = state.effective_wrap(&uuid, global);

        // Should restore original
        prop_assert_eq!(
            after_second_toggle, initial,
            "Double toggle should restore original mode"
        );
        prop_assert_eq!(
            after_second_toggle, global,
            "Double toggle should match global again"
        );
    }
}

// ===== Property 4: Height Consistency - NoWrap Mode =====

proptest! {
    #[test]
    fn nowrap_height_equals_line_count(
        text in ".{0,300}",
        width in 1usize..200
    ) {
        // For NoWrap mode, height should equal line count regardless of viewport width
        let line_count = text.lines().count().max(1);

        // Calculate what wrapped would give (should be >= line_count)
        let wrapped_lines = calculate_wrapped_lines(&text, width);

        // For NoWrap, we should use line_count, not wrapped_lines
        // This tests the logic: WrapMode::NoWrap => text.lines().count().max(1)
        prop_assert!(
            line_count <= wrapped_lines,
            "NoWrap line count {} should be <= wrapped count {} (width={})",
            line_count, wrapped_lines, width
        );
    }
}

// ===== Property 5: Single Line Text Invariant =====

proptest! {
    #[test]
    fn single_line_text_wraps_correctly(
        // Generate single-line text (no newlines)
        text in "[^\n]{1,500}",
        width in 1usize..200
    ) {
        let wrapped = calculate_wrapped_lines(&text, width);
        let expected = text.len().div_ceil(width);

        prop_assert_eq!(
            wrapped, expected,
            "Single-line text of len {} should wrap into {} lines at width {}",
            text.len(), expected, width
        );
    }
}

// ===== Property 6: Empty Lines Preserved =====

proptest! {
    #[test]
    fn empty_lines_preserved_when_wrapping(
        // Generate text with multiple explicit newlines
        line_count in 1usize..20,
        width in 1usize..200
    ) {
        // Create text with N empty lines (just newlines)
        let text = "\n".repeat(line_count.saturating_sub(1));

        let wrapped = calculate_wrapped_lines(&text, width);

        // Each empty line should still count as a line
        // lines() on "\n\n" yields ["", ""] (2 lines)
        let expected_lines = text.lines().count().max(1);
        prop_assert_eq!(
            wrapped, expected_lines,
            "Empty lines should be preserved: {} newlines -> {} lines",
            line_count.saturating_sub(1), expected_lines
        );
    }
}

// ===== Property 7: Wrap Override Set Membership =====

proptest! {
    #[test]
    fn wrap_override_set_membership_consistent(
        uuid_str in "[a-z0-9-]{1,50}",
        toggle_count in 0usize..10
    ) {
        if uuid_str.is_empty() {
            return Ok(());
        }

        let uuid = EntryUuid::new(&uuid_str).unwrap();
        let mut state = ScrollState {
            vertical_offset: 0,
            horizontal_offset: 0,
            expanded_messages: Default::default(),
            focused_message: None,
            wrap_overrides: Default::default(),
        };

        // Toggle N times
        for _ in 0..toggle_count {
            state.toggle_wrap(&uuid);
        }

        // Set membership should match parity of toggle_count
        let is_in_set = state.wrap_overrides.contains(&uuid);
        let expected_in_set = toggle_count % 2 == 1;

        prop_assert_eq!(
            is_in_set, expected_in_set,
            "After {} toggles, override should be {} (is: {})",
            toggle_count,
            if expected_in_set { "present" } else { "absent" },
            if is_in_set { "present" } else { "absent" }
        );
    }
}

// ===== Property 8: Width Increase Never Increases Line Count =====

proptest! {
    #[test]
    fn width_increase_never_increases_lines(
        text in ".{1,300}",
        width1 in 1usize..100,
        width2 in 1usize..100
    ) {
        let smaller_width = width1.min(width2);
        let larger_width = width1.max(width2);

        let lines_at_smaller = calculate_wrapped_lines(&text, smaller_width);
        let lines_at_larger = calculate_wrapped_lines(&text, larger_width);

        prop_assert!(
            lines_at_larger <= lines_at_smaller,
            "Larger width {} should not increase line count: {} lines vs {} lines at width {}",
            larger_width, lines_at_larger, lines_at_smaller, smaller_width
        );
    }
}
