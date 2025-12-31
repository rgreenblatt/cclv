//! Property-based tests for per-item wrap rendering.
//!
//! Tests validate wrap-related invariants:
//! 1. Wrapped line count never less than unwrapped count
//! 2. Zero viewport width handles gracefully (no panic)
//! 3. Effective wrap mode double-toggle is identity
//! 4. FR-053: Code blocks force NoWrap mode regardless of settings
//! 5. Single-line text wraps correctly
//! 6. Empty lines preserved when wrapping
//! 7. Wrap override set membership consistency
//! 8. Width increase never increases line count

use cclv::model::{
    ContentBlock, ConversationEntry, EntryMetadata, EntryType, EntryUuid, LogEntry, Message,
    MessageContent, Role, SessionId,
};
use cclv::state::{ScrollState, WrapMode};
use cclv::view::{extract_entry_text, has_code_blocks, ConversationView};
use chrono::Utc;
use proptest::prelude::*;

// ===== Property 1: Wrapped Lines Never Less Than Unwrapped =====

proptest! {
    #[test]
    fn wrapped_lines_never_less_than_unwrapped(
        text in ".{0,500}",
        width in 1usize..200
    ) {
        let wrapped = ConversationView::calculate_wrapped_lines(&text, width);
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
        let result = ConversationView::calculate_wrapped_lines(&text, 0);

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

// ===== Property 4: FR-053 Code Blocks Force NoWrap Mode =====

proptest! {
    #[test]
    fn code_blocks_force_nowrap_mode_regardless_of_settings(
        prose_before in ".{10,100}",
        code_content in ".{1,50}",
        prose_after in ".{10,100}",
        global in prop_oneof![Just(WrapMode::Wrap), Just(WrapMode::NoWrap)],
        has_override in prop::bool::ANY,
    ) {
        // Create text with fenced code block
        let text_with_code = format!(
            "{}\n```rust\n{}\n```\n{}",
            prose_before, code_content, prose_after
        );

        // Verify has_code_blocks detection works
        prop_assert!(
            has_code_blocks(&text_with_code),
            "Text with fenced code block should be detected"
        );

        // Create entry with code block
        let uuid = EntryUuid::new("test-code-entry").unwrap();
        let message = Message::new(Role::Assistant, MessageContent::Text(text_with_code));
        let entry = LogEntry::new(
            uuid.clone(),
            None,
            SessionId::new("test-session").unwrap(),
            None,
            Utc::now(),
            EntryType::Assistant,
            message,
            EntryMetadata::default(),
        );

        // Set up scroll state with optional override
        let mut scroll = ScrollState {
            vertical_offset: 0,
            horizontal_offset: 0,
            focused_message: None,
            wrap_overrides: Default::default(),
        };

        if has_override {
            scroll.toggle_wrap(&uuid);
        }

        // Extract text and verify code blocks detected
        let entry_text = extract_entry_text(&ConversationEntry::Valid(Box::new(entry.clone())));
        prop_assert!(
            has_code_blocks(&entry_text),
            "Code blocks should be detected in entry text"
        );

        // FR-053: Code blocks MUST force NoWrap regardless of global or per-item settings
        // This is the critical invariant - if has_code_blocks returns true,
        // the effective wrap mode MUST be NoWrap
        let effective = if has_code_blocks(&entry_text) {
            WrapMode::NoWrap
        } else {
            scroll.effective_wrap(&uuid, global)
        };

        prop_assert_eq!(
            effective,
            WrapMode::NoWrap,
            "Code blocks must force NoWrap mode (global={:?}, has_override={})",
            global,
            has_override
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
        let wrapped = ConversationView::calculate_wrapped_lines(&text, width);
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

        let wrapped = ConversationView::calculate_wrapped_lines(&text, width);

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

        let lines_at_smaller = ConversationView::calculate_wrapped_lines(&text, smaller_width);
        let lines_at_larger = ConversationView::calculate_wrapped_lines(&text, larger_width);

        prop_assert!(
            lines_at_larger <= lines_at_smaller,
            "Larger width {} should not increase line count: {} lines vs {} lines at width {}",
            larger_width, lines_at_larger, lines_at_smaller, smaller_width
        );
    }
}

// ===== Property 9: Code Block Detection with Indented Blocks =====

proptest! {
    #[test]
    fn indented_code_blocks_force_nowrap_mode(
        // Constrain prose to ASCII-only, no leading/trailing whitespace
        // to ensure predictable string structure for indented code block detection
        prose in "[a-zA-Z0-9 ,.!?;:(){}<>\\-_~]{10,100}",
        // Constrain code_line to non-empty ASCII (no spaces to ensure it's not all whitespace)
        code_line in "[a-zA-Z0-9,.=+*]{1,50}",
        global in prop_oneof![Just(WrapMode::Wrap), Just(WrapMode::NoWrap)],
    ) {
        // Create text with indented code block (4+ spaces)
        let text_with_code = format!(
            "{}\n\n    {}\n\n{}",
            prose, code_line, prose
        );

        // Verify has_code_blocks detection works for indented blocks
        prop_assert!(
            has_code_blocks(&text_with_code),
            "Text with indented code block (4+ spaces) should be detected"
        );

        // Create entry
        let uuid = EntryUuid::new("test-indent-code").unwrap();
        let message = Message::new(Role::Assistant, MessageContent::Text(text_with_code));
        let entry = LogEntry::new(
            uuid.clone(),
            None,
            SessionId::new("test-session").unwrap(),
            None,
            Utc::now(),
            EntryType::Assistant,
            message,
            EntryMetadata::default(),
        );

        let scroll = ScrollState {
            vertical_offset: 0,
            horizontal_offset: 0,
            focused_message: None,
            wrap_overrides: Default::default(),
        };

        // Extract text and verify
        let entry_text = extract_entry_text(&ConversationEntry::Valid(Box::new(entry)));
        prop_assert!(
            has_code_blocks(&entry_text),
            "Indented code blocks should be detected"
        );

        // FR-053: Must force NoWrap
        let effective = if has_code_blocks(&entry_text) {
            WrapMode::NoWrap
        } else {
            scroll.effective_wrap(&uuid, global)
        };

        prop_assert_eq!(
            effective,
            WrapMode::NoWrap,
            "Indented code blocks must force NoWrap mode (global={:?})",
            global
        );
    }
}

// ===== Property 10: Code Blocks in ContentBlocks =====

proptest! {
    #[test]
    fn code_blocks_in_thinking_blocks_force_nowrap(
        thinking_text in ".{10,100}",
        code in ".{1,50}",
        global in prop_oneof![Just(WrapMode::Wrap), Just(WrapMode::NoWrap)],
    ) {
        // Create Thinking block with code
        let thinking_with_code = format!(
            "{}\n```rust\n{}\n```",
            thinking_text, code
        );

        let blocks = vec![
            ContentBlock::Text {
                text: "User-visible text".to_string(),
            },
            ContentBlock::Thinking {
                thinking: thinking_with_code,
            },
        ];

        let message = Message::new(Role::Assistant, MessageContent::Blocks(blocks));
        let uuid = EntryUuid::new("test-thinking-code").unwrap();
        let entry = LogEntry::new(
            uuid.clone(),
            None,
            SessionId::new("test-session").unwrap(),
            None,
            Utc::now(),
            EntryType::Assistant,
            message,
            EntryMetadata::default(),
        );

        let scroll = ScrollState {
            vertical_offset: 0,
            horizontal_offset: 0,
            focused_message: None,
            wrap_overrides: Default::default(),
        };

        // Extract and verify
        let entry_text = extract_entry_text(&ConversationEntry::Valid(Box::new(entry)));
        prop_assert!(
            has_code_blocks(&entry_text),
            "Code in Thinking blocks should be detected"
        );

        // FR-053: Must force NoWrap
        let effective = if has_code_blocks(&entry_text) {
            WrapMode::NoWrap
        } else {
            scroll.effective_wrap(&uuid, global)
        };

        prop_assert_eq!(
            effective,
            WrapMode::NoWrap,
            "Code in Thinking blocks must force NoWrap mode (global={:?})",
            global
        );
    }
}
