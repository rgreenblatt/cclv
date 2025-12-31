//! Semantic scroll position

use super::types::{EntryIndex, LineOffset};

/// Semantic scroll position within a conversation.
///
/// A sum type that preserves scroll intent across layout changes:
/// - `Top`: Always shows from line 0
/// - `Bottom`: Always shows last lines in viewport
/// - `AtLine`: Specific absolute line offset
/// - `AtEntry`: Keep specific entry visible (survives relayout)
/// - `Fraction`: Proportional position (for scrollbar)
///
/// # Resolution
/// All variants resolve to `LineOffset` via `resolve()` method.
/// The resolution uses current layout state for `AtEntry` and `Bottom`.
///
/// # Clamping Behavior
/// When a scroll position would resolve beyond document bounds,
/// it is clamped to the valid range `[0, max(0, total_height - viewport_height)]`.
/// This ensures no blank viewports regardless of the requested position.
#[derive(Debug, Clone, PartialEq)]
pub enum ScrollPosition {
    /// View from the very top (line 0).
    Top,

    /// View from the very bottom.
    /// Resolves to: total_height - viewport_height (clamped to 0).
    Bottom,

    /// Specific line offset from top.
    /// Clamped to valid range on resolution.
    AtLine(LineOffset),

    /// Keep specific entry at top of viewport.
    /// Survives relayout: resolves using entry's cumulative_y.
    /// If entry_index is beyond document end, clamps to last entry.
    AtEntry {
        /// Index of entry in the conversation.
        entry_index: EntryIndex,
        /// Line offset within the entry (0 = top of entry).
        line_in_entry: usize,
    },

    /// Fractional position (0.0 = top, 1.0 = bottom).
    /// Used by scrollbar for proportional navigation.
    /// Clamped to [0.0, 1.0] on resolution.
    Fraction(f64),
}

impl Default for ScrollPosition {
    fn default() -> Self {
        Self::Top
    }
}

impl ScrollPosition {
    /// Resolve to absolute line offset.
    ///
    /// # Arguments
    /// - `total_height`: Total height of content in lines
    /// - `viewport_height`: Height of viewport in lines
    /// - `entry_lookup`: Function to get entry's cumulative_y by index
    ///
    /// # Returns
    /// Absolute line offset from top, clamped to valid range.
    /// Never returns an offset that would cause a blank viewport.
    pub fn resolve<F>(&self, _total_height: usize, _viewport_height: usize, _entry_lookup: F) -> LineOffset
    where
        F: Fn(EntryIndex) -> Option<LineOffset>,
    {
        todo!("ScrollPosition::resolve")
    }

    /// Create AtEntry position for given entry index.
    pub fn at_entry(entry_index: EntryIndex) -> Self {
        Self::AtEntry {
            entry_index,
            line_in_entry: 0,
        }
    }

    /// Create AtLine position.
    pub fn at_line(offset: usize) -> Self {
        Self::AtLine(LineOffset::new(offset))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_top() {
        assert_eq!(ScrollPosition::default(), ScrollPosition::Top);
    }

    #[test]
    fn at_entry_constructor_sets_line_in_entry_to_zero() {
        let pos = ScrollPosition::at_entry(EntryIndex::new(5));
        assert_eq!(
            pos,
            ScrollPosition::AtEntry {
                entry_index: EntryIndex::new(5),
                line_in_entry: 0
            }
        );
    }

    #[test]
    fn at_line_constructor_wraps_offset() {
        let pos = ScrollPosition::at_line(42);
        assert_eq!(pos, ScrollPosition::AtLine(LineOffset::new(42)));
    }

    mod resolve {
        use super::*;

        // Helper: no-op entry lookup that always returns None
        fn no_entries(_idx: EntryIndex) -> Option<LineOffset> {
            None
        }

        // Helper: mock entry lookup
        fn mock_lookup(idx: EntryIndex) -> Option<LineOffset> {
            match idx.get() {
                0 => Some(LineOffset::new(0)),
                1 => Some(LineOffset::new(10)),
                2 => Some(LineOffset::new(25)),
                _ => None,
            }
        }

        #[test]
        fn top_resolves_to_zero() {
            let pos = ScrollPosition::Top;
            let result = pos.resolve(100, 20, no_entries);
            assert_eq!(result.get(), 0);
        }

        #[test]
        fn bottom_resolves_to_max_offset() {
            let pos = ScrollPosition::Bottom;
            let result = pos.resolve(100, 20, no_entries);
            assert_eq!(result.get(), 80); // 100 - 20
        }

        #[test]
        fn bottom_clamps_to_zero_when_viewport_exceeds_content() {
            let pos = ScrollPosition::Bottom;
            let result = pos.resolve(10, 50, no_entries);
            assert_eq!(result.get(), 0); // saturating_sub: 10 - 50 = 0
        }

        #[test]
        fn at_line_returns_exact_offset_when_in_range() {
            let pos = ScrollPosition::AtLine(LineOffset::new(30));
            let result = pos.resolve(100, 20, no_entries);
            assert_eq!(result.get(), 30);
        }

        #[test]
        fn at_line_clamps_to_max_offset_when_beyond_range() {
            let pos = ScrollPosition::AtLine(LineOffset::new(95));
            let result = pos.resolve(100, 20, no_entries);
            assert_eq!(result.get(), 80); // clamped to max_offset (100 - 20)
        }

        #[test]
        fn at_entry_uses_lookup_to_find_cumulative_y() {
            let pos = ScrollPosition::AtEntry {
                entry_index: EntryIndex::new(1),
                line_in_entry: 0,
            };
            let result = pos.resolve(100, 20, mock_lookup);
            assert_eq!(result.get(), 10); // entry 1 is at cumulative_y = 10
        }

        #[test]
        fn at_entry_adds_line_in_entry_offset() {
            let pos = ScrollPosition::AtEntry {
                entry_index: EntryIndex::new(2),
                line_in_entry: 5,
            };
            let result = pos.resolve(100, 20, mock_lookup);
            assert_eq!(result.get(), 30); // entry 2 at y=25, plus 5
        }

        #[test]
        fn at_entry_returns_zero_when_entry_not_found() {
            let pos = ScrollPosition::AtEntry {
                entry_index: EntryIndex::new(999),
                line_in_entry: 0,
            };
            let result = pos.resolve(100, 20, mock_lookup);
            assert_eq!(result.get(), 0); // lookup returns None → fallback to 0
        }

        #[test]
        fn at_entry_clamps_computed_offset_to_max() {
            let pos = ScrollPosition::AtEntry {
                entry_index: EntryIndex::new(2),
                line_in_entry: 70, // 25 + 70 = 95, which exceeds max_offset
            };
            let result = pos.resolve(100, 20, mock_lookup);
            assert_eq!(result.get(), 80); // clamped to max_offset
        }

        #[test]
        fn fraction_zero_resolves_to_top() {
            let pos = ScrollPosition::Fraction(0.0);
            let result = pos.resolve(100, 20, no_entries);
            assert_eq!(result.get(), 0);
        }

        #[test]
        fn fraction_one_resolves_to_bottom() {
            let pos = ScrollPosition::Fraction(1.0);
            let result = pos.resolve(100, 20, no_entries);
            assert_eq!(result.get(), 80); // max_offset
        }

        #[test]
        fn fraction_half_resolves_to_midpoint() {
            let pos = ScrollPosition::Fraction(0.5);
            let result = pos.resolve(100, 20, no_entries);
            assert_eq!(result.get(), 40); // 0.5 * 80 = 40
        }

        #[test]
        fn fraction_clamps_negative_to_zero() {
            let pos = ScrollPosition::Fraction(-0.5);
            let result = pos.resolve(100, 20, no_entries);
            assert_eq!(result.get(), 0); // clamped to [0.0, 1.0]
        }

        #[test]
        fn fraction_clamps_above_one() {
            let pos = ScrollPosition::Fraction(1.5);
            let result = pos.resolve(100, 20, no_entries);
            assert_eq!(result.get(), 80); // clamped to 1.0 → max_offset
        }

        #[test]
        fn empty_document_all_positions_resolve_to_zero() {
            let positions = vec![
                ScrollPosition::Top,
                ScrollPosition::Bottom,
                ScrollPosition::AtLine(LineOffset::new(100)),
                ScrollPosition::at_entry(EntryIndex::new(5)),
                ScrollPosition::Fraction(0.5),
            ];

            for pos in positions {
                let result = pos.resolve(0, 10, no_entries);
                assert_eq!(result.get(), 0, "position {:?} should resolve to 0", pos);
            }
        }

        #[test]
        fn viewport_larger_than_content_clamps_all_to_zero() {
            let positions = vec![
                ScrollPosition::Top,
                ScrollPosition::Bottom,
                ScrollPosition::AtLine(LineOffset::new(5)),
                ScrollPosition::Fraction(0.5),
            ];

            for pos in positions {
                let result = pos.resolve(10, 50, no_entries);
                assert_eq!(
                    result.get(),
                    0,
                    "position {:?} should clamp to 0 when viewport > content",
                    pos
                );
            }
        }
    }
}
