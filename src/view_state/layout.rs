//! Layout information for entries

use super::types::{LineHeight, LineOffset};

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

#[cfg(test)]
mod tests {
    use super::*;

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
}
