//! Layout information for entries

use super::types::{LineHeight, LineOffset};
use crate::model::ConversationEntry;
use crate::state::WrapMode;

/// Layout metadata for a single entry.
///
/// Computed from entry content + viewport width + expand state.
/// Stored alongside the entry in EntryView.
///
/// # Invariants
/// - `height >= 1` (enforced by LineHeight)
/// - `cumulative_y[i] = sum(height[0..i])` (maintained by ConversationViewState)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EntryLayout {
    /// Height of this entry in lines.
    height: LineHeight,
    /// Cumulative Y offset from start of conversation.
    /// Equal to sum of all preceding entry heights.
    cumulative_y: LineOffset,
}

impl EntryLayout {
    /// Create new layout. Called internally during layout computation.
    #[allow(dead_code)] // Used by ConversationViewState during layout computation
    pub(crate) fn new(height: LineHeight, cumulative_y: LineOffset) -> Self {
        Self {
            height,
            cumulative_y,
        }
    }

    /// Height in lines.
    pub fn height(&self) -> LineHeight {
        self.height
    }

    /// Cumulative Y offset (lines from start of conversation).
    pub fn cumulative_y(&self) -> LineOffset {
        self.cumulative_y
    }

    /// Y offset of the line immediately after this entry.
    /// Equal to cumulative_y + height.
    pub fn bottom_y(&self) -> LineOffset {
        LineOffset::new(self.cumulative_y.get() + self.height.get() as usize)
    }
}

#[allow(clippy::derivable_impls)]
impl Default for EntryLayout {
    fn default() -> Self {
        Self {
            height: LineHeight::default(),
            cumulative_y: LineOffset::default(),
        }
    }
}

/// Calculate the rendered height of an entry in terminal lines.
///
/// This is THE canonical implementation. The view layer delegates to this function.
///
/// Computes actual line count accounting for:
/// - Text wrapping at viewport width (when wrap mode is Wrap)
/// - Markdown rendering (headers, lists, code blocks)
/// - Expanded vs collapsed state
/// - Malformed entries (return 5 lines for error display)
///
/// # Arguments
/// - `entry`: The conversation entry to calculate height for
/// - `expanded`: Whether entry is expanded (true) or collapsed (false)
/// - `wrap_mode`: Text wrapping mode (Wrap or NoWrap)
/// - `width`: Viewport width in characters for wrapping calculations
///
/// # Contract (from data-model.md HeightCalculator)
/// - MUST return `LineHeight` with at least 1 for valid entries
/// - MUST return fixed height (5 lines) for malformed entries
/// - MUST be deterministic (same inputs → same output)
/// - SHOULD be fast (called for every entry during layout)
pub fn calculate_height(
    entry: &ConversationEntry,
    expanded: bool,
    wrap_mode: WrapMode,
    width: u16,
) -> LineHeight {
    use crate::model::MessageContent;

    match entry {
        ConversationEntry::Malformed(_) => {
            // Malformed entries render as:
            // - Separator line (1)
            // - Header line "⚠ Parse Error (line N)" (1)
            // - Error message lines (varies, estimate 2)
            // - Separator line (1)
            // Total: ~5 lines
            LineHeight::new(5).unwrap()
        }
        ConversationEntry::Valid(log_entry) => {
            let message = log_entry.message();

            // Count content lines
            let mut content_lines = 0u16;
            match message.content() {
                MessageContent::Text(text) => {
                    content_lines = count_text_lines(text, wrap_mode, width);
                }
                MessageContent::Blocks(blocks) => {
                    for block in blocks {
                        content_lines += count_block_lines(block, wrap_mode, width);
                    }
                }
            }

            // Collapsed entries are truncated if they exceed threshold (default: 10 lines)
            // Rendering constants from ConversationView defaults (message.rs:385-386)
            const COLLAPSE_THRESHOLD: u16 = 10;
            const SUMMARY_LINES: u16 = 3;

            let should_collapse = content_lines > COLLAPSE_THRESHOLD && !expanded;

            let displayed_lines = if should_collapse {
                // Collapsed: summary_lines + collapse indicator
                SUMMARY_LINES + 1
            } else {
                // Show all content
                content_lines
            };

            // Add separator line (always present at end of entry)
            let total_lines = displayed_lines + 1;

            // Return at least LineHeight::ONE
            LineHeight::new(total_lines.max(1)).unwrap()
        }
    }
}

/// Count lines in a text string accounting for newlines and wrapping.
///
/// # Arguments
/// - `text`: The text to count lines for
/// - `wrap`: Wrap mode (Wrap or NoWrap)
/// - `width`: Viewport width for wrapping calculations
///
/// NOTE: Empty text returns 0 to match renderer behavior where `"".lines()`
/// produces zero iterations. The separator line is added separately.
fn count_text_lines(text: &str, wrap: WrapMode, width: u16) -> u16 {
    if text.is_empty() {
        return 0; // Empty text produces 0 content lines (matches "".lines() behavior)
    }

    let lines: Vec<&str> = text.lines().collect();
    let line_count = lines.len().max(1);

    match wrap {
        WrapMode::NoWrap => line_count as u16,
        WrapMode::Wrap => {
            // Adjust width for borders (ConversationView uses `area.width.saturating_sub(2)`)
            // The width parameter is terminal width, but content area is 2 chars narrower
            let content_width = width.saturating_sub(2).max(1) as usize;
            let mut wrapped_lines = 0;
            for line in lines {
                let line_width = line.chars().count();
                if line_width == 0 {
                    wrapped_lines += 1;
                } else {
                    // Calculate how many lines this wraps to
                    wrapped_lines += line_width.div_ceil(content_width).max(1);
                }
            }
            wrapped_lines as u16
        }
    }
}

/// Count lines in a content block.
fn count_block_lines(block: &crate::model::ContentBlock, wrap: WrapMode, width: u16) -> u16 {
    use crate::model::ContentBlock;

    match block {
        ContentBlock::Text { text } => count_text_lines(text, wrap, width),
        ContentBlock::Thinking { thinking } => count_text_lines(thinking, wrap, width),
        ContentBlock::ToolResult { content, .. } => count_text_lines(content, wrap, width),
        ContentBlock::ToolUse(tool_call) => {
            // Tool use renders as: tool name + input (typically 2-3 lines)
            let input_str = tool_call.input().to_string();
            2 + count_text_lines(&input_str, wrap, width)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{
        ConversationEntry, EntryMetadata, EntryType, EntryUuid, LogEntry, MalformedEntry, Message,
        MessageContent, Role, SessionId,
    };
    use chrono::DateTime;

    #[test]
    fn new_creates_layout_with_given_values() {
        let height = LineHeight::new(5).unwrap();
        let cumulative_y = LineOffset::new(10);
        let layout = EntryLayout::new(height, cumulative_y);

        assert_eq!(layout.height(), height);
        assert_eq!(layout.cumulative_y(), cumulative_y);
    }

    #[test]
    fn bottom_y_returns_cumulative_plus_height() {
        let height = LineHeight::new(3).unwrap();
        let cumulative_y = LineOffset::new(7);
        let layout = EntryLayout::new(height, cumulative_y);

        let expected_bottom = LineOffset::new(7 + 3);
        assert_eq!(layout.bottom_y(), expected_bottom);
    }

    #[test]
    fn bottom_y_with_zero_cumulative() {
        let height = LineHeight::new(5).unwrap();
        let cumulative_y = LineOffset::new(0);
        let layout = EntryLayout::new(height, cumulative_y);

        assert_eq!(layout.bottom_y(), LineOffset::new(5));
    }

    #[test]
    fn bottom_y_with_minimum_height() {
        let height = LineHeight::ONE;
        let cumulative_y = LineOffset::new(100);
        let layout = EntryLayout::new(height, cumulative_y);

        assert_eq!(layout.bottom_y(), LineOffset::new(101));
    }

    #[test]
    fn default_returns_default_values() {
        let layout = EntryLayout::default();

        assert_eq!(layout.height(), LineHeight::default());
        assert_eq!(layout.cumulative_y(), LineOffset::default());
    }

    #[test]
    fn default_bottom_y_equals_default_height() {
        let layout = EntryLayout::default();

        // Default LineHeight is ONE, default LineOffset is 0
        // So bottom_y should be 0 + 1 = 1
        assert_eq!(layout.bottom_y(), LineOffset::new(1));
    }

    #[test]
    fn equality_works() {
        let layout1 = EntryLayout::new(LineHeight::new(3).unwrap(), LineOffset::new(5));
        let layout2 = EntryLayout::new(LineHeight::new(3).unwrap(), LineOffset::new(5));
        let layout3 = EntryLayout::new(LineHeight::new(4).unwrap(), LineOffset::new(5));

        assert_eq!(layout1, layout2);
        assert_ne!(layout1, layout3);
    }

    #[test]
    fn clone_produces_equal_layout() {
        let layout1 = EntryLayout::new(LineHeight::new(7).unwrap(), LineOffset::new(20));
        let layout2 = layout1; // Copy semantics, not clone

        assert_eq!(layout1, layout2);
    }

    #[test]
    fn copy_works() {
        let layout1 = EntryLayout::new(LineHeight::new(2).unwrap(), LineOffset::new(8));
        let layout2 = layout1; // Copy, not move

        // Both should be usable
        assert_eq!(layout1.height(), LineHeight::new(2).unwrap());
        assert_eq!(layout2.height(), LineHeight::new(2).unwrap());
    }

    // ===== Height Calculator Tests =====

    // Test helpers
    fn make_uuid(s: &str) -> EntryUuid {
        EntryUuid::new(s).expect("valid uuid")
    }

    fn make_session_id(s: &str) -> SessionId {
        SessionId::new(s).expect("valid session id")
    }

    fn make_timestamp() -> DateTime<chrono::Utc> {
        "2025-12-25T10:30:00Z".parse().expect("valid timestamp")
    }

    fn make_message(text: &str) -> Message {
        Message::new(Role::Assistant, MessageContent::Text(text.to_string()))
    }

    fn make_valid_entry(text: &str) -> ConversationEntry {
        let entry = LogEntry::new(
            make_uuid("test-entry"),
            None,
            make_session_id("test-session"),
            None,
            make_timestamp(),
            EntryType::Assistant,
            make_message(text),
            EntryMetadata::default(),
        );
        ConversationEntry::Valid(Box::new(entry))
    }

    fn make_malformed_entry() -> ConversationEntry {
        ConversationEntry::Malformed(MalformedEntry::new(42, "invalid json", "parse error", None))
    }

    mod calculate_height_tests {
        use super::*;

        #[test]
        fn malformed_entry_returns_nonzero_height() {
            let entry = make_malformed_entry();
            let height = calculate_height(&entry, false, WrapMode::Wrap, 80);
            // Malformed entries now return ~5 lines to account for error display
            assert!(
                !height.is_zero(),
                "Malformed entries should have height > 0"
            );
            assert_eq!(height.get(), 5, "Malformed entries render with 5 lines");
        }

        #[test]
        fn malformed_entry_height_independent_of_expand() {
            let entry = make_malformed_entry();
            let collapsed = calculate_height(&entry, false, WrapMode::Wrap, 80);
            let expanded = calculate_height(&entry, true, WrapMode::Wrap, 80);
            // Malformed entries always render the same height
            assert_eq!(collapsed, expanded);
            assert_eq!(collapsed.get(), 5);
        }

        #[test]
        fn valid_collapsed_entry_returns_fixed_height() {
            let entry = make_valid_entry("Hello, world!");
            let height = calculate_height(&entry, false, WrapMode::Wrap, 80);

            // Collapsed entries show 2-3 lines
            assert!(!height.is_zero());
            assert!(height.get() >= 2);
            assert!(height.get() <= 3);
        }

        #[test]
        fn valid_expanded_entry_returns_at_least_one() {
            let entry = make_valid_entry("Short");
            let height = calculate_height(&entry, true, WrapMode::Wrap, 80);

            assert!(!height.is_zero());
            assert!(height.get() >= 1);
        }

        #[test]
        fn empty_content_expanded_includes_separator() {
            let entry = make_valid_entry("");
            let height = calculate_height(&entry, true, WrapMode::Wrap, 80);

            // Empty content = 0 content lines + 1 separator = 1 line total
            // (matches renderer where "".lines() produces 0 iterations)
            assert_eq!(height, LineHeight::new(1).unwrap());
        }

        #[test]
        fn multiline_content_expanded_returns_multiple_lines() {
            let multiline_text = "Line 1\nLine 2\nLine 3\nLine 4\nLine 5";
            let entry = make_valid_entry(multiline_text);
            let height = calculate_height(&entry, true, WrapMode::Wrap, 80);

            // Should have at least as many lines as newlines + 1
            assert!(height.get() >= 5);
        }

        #[test]
        fn collapsed_height_consistent_regardless_of_content() {
            let short_entry = make_valid_entry("Short");
            let long_entry =
                make_valid_entry("Very long text that spans multiple lines when expanded");

            let short_height = calculate_height(&short_entry, false, WrapMode::Wrap, 80);
            let long_height = calculate_height(&long_entry, false, WrapMode::Wrap, 80);

            // Collapsed height should be the same
            assert_eq!(short_height, long_height);
        }

        #[test]
        fn deterministic_same_inputs_same_output() {
            let entry = make_valid_entry("Test content");

            let height1 = calculate_height(&entry, true, WrapMode::Wrap, 80);
            let height2 = calculate_height(&entry, true, WrapMode::Wrap, 80);

            assert_eq!(height1, height2);
        }

        #[test]
        fn expanded_mode_affects_height() {
            let entry = make_valid_entry("Line 1\nLine 2\nLine 3");

            let collapsed_height = calculate_height(&entry, false, WrapMode::Wrap, 80);
            let expanded_height = calculate_height(&entry, true, WrapMode::Wrap, 80);

            // Expanded should generally be taller than collapsed for multi-line content
            assert!(expanded_height.get() >= collapsed_height.get());
        }
    }
}
