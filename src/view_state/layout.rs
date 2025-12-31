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

/// Height calculator function type.
///
/// Computes rendered height for an entry accounting for:
/// - Malformed entries return LineHeight::ZERO
/// - Collapsed entries return fixed small height (2-3 lines)
/// - Expanded entries compute actual rendered height based on content
pub type HeightCalculator = fn(&ConversationEntry, bool, WrapMode) -> LineHeight;

/// Calculate the rendered height of an entry in terminal lines.
///
/// # Arguments
/// - `entry`: The conversation entry to measure
/// - `expanded`: Whether the entry is currently expanded
/// - `_wrap_mode`: The effective wrap mode for this entry (currently unused)
///
/// # Returns
/// - `LineHeight::ZERO` for malformed entries
/// - At least `LineHeight::ONE` for valid entries
///
/// # Contract Requirements
/// - MUST return `LineHeight::ZERO` for malformed entries
/// - MUST return at least `LineHeight::ONE` for valid entries
/// - MUST be deterministic (same inputs → same output)
/// - SHOULD be fast (called for every entry during layout)
///
/// # Implementation Notes
/// This is a simplified initial implementation that counts newlines.
/// Future enhancements may include:
/// - Text wrapping based on viewport width
/// - Markdown rendering line count
/// - Syntax highlighting effects
pub fn calculate_height(
    entry: &ConversationEntry,
    expanded: bool,
    _wrap_mode: WrapMode,
) -> LineHeight {
    match entry {
        ConversationEntry::Malformed(_) => LineHeight::ZERO,
        ConversationEntry::Valid(log_entry) => {
            if !expanded {
                // Collapsed: fixed height showing summary
                // Type indicator + truncated preview = 2 lines
                LineHeight::new(2).expect("2 is valid line height")
            } else {
                // Expanded: count actual lines in content
                let content = log_entry.message().text();
                let line_count = if content.is_empty() {
                    1
                } else {
                    // Count lines by splitting on newlines
                    // Number of lines = number of newlines + 1
                    content.lines().count().max(1)
                };
                LineHeight::new(line_count as u16).expect("line_count >= 1")
            }
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
        fn malformed_entry_returns_zero_height() {
            let entry = make_malformed_entry();
            let height = calculate_height(&entry, false, WrapMode::Wrap);
            assert_eq!(height, LineHeight::ZERO);
        }

        #[test]
        fn malformed_entry_returns_zero_when_expanded() {
            let entry = make_malformed_entry();
            let height = calculate_height(&entry, true, WrapMode::Wrap);
            assert_eq!(height, LineHeight::ZERO);
        }

        #[test]
        fn valid_collapsed_entry_returns_fixed_height() {
            let entry = make_valid_entry("Hello, world!");
            let height = calculate_height(&entry, false, WrapMode::Wrap);

            // Collapsed entries show 2-3 lines
            assert!(!height.is_zero());
            assert!(height.get() >= 2);
            assert!(height.get() <= 3);
        }

        #[test]
        fn valid_expanded_entry_returns_at_least_one() {
            let entry = make_valid_entry("Short");
            let height = calculate_height(&entry, true, WrapMode::Wrap);

            assert!(!height.is_zero());
            assert!(height.get() >= 1);
        }

        #[test]
        fn empty_content_expanded_returns_at_least_one() {
            let entry = make_valid_entry("");
            let height = calculate_height(&entry, true, WrapMode::Wrap);

            assert_eq!(height, LineHeight::ONE);
        }

        #[test]
        fn multiline_content_expanded_returns_multiple_lines() {
            let multiline_text = "Line 1\nLine 2\nLine 3\nLine 4\nLine 5";
            let entry = make_valid_entry(multiline_text);
            let height = calculate_height(&entry, true, WrapMode::Wrap);

            // Should have at least as many lines as newlines + 1
            assert!(height.get() >= 5);
        }

        #[test]
        fn collapsed_height_consistent_regardless_of_content() {
            let short_entry = make_valid_entry("Short");
            let long_entry =
                make_valid_entry("Very long text that spans multiple lines when expanded");

            let short_height = calculate_height(&short_entry, false, WrapMode::Wrap);
            let long_height = calculate_height(&long_entry, false, WrapMode::Wrap);

            // Collapsed height should be the same
            assert_eq!(short_height, long_height);
        }

        #[test]
        fn deterministic_same_inputs_same_output() {
            let entry = make_valid_entry("Test content");

            let height1 = calculate_height(&entry, true, WrapMode::Wrap);
            let height2 = calculate_height(&entry, true, WrapMode::Wrap);

            assert_eq!(height1, height2);
        }

        #[test]
        fn expanded_mode_affects_height() {
            let entry = make_valid_entry("Line 1\nLine 2\nLine 3");

            let collapsed_height = calculate_height(&entry, false, WrapMode::Wrap);
            let expanded_height = calculate_height(&entry, true, WrapMode::Wrap);

            // Expanded should generally be taller than collapsed for multi-line content
            assert!(expanded_height.get() >= collapsed_height.get());
        }
    }
}
