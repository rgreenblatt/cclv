//! Visible range calculation result

use super::types::{EntryIndex, LineOffset};

/// Range of entries visible in the current viewport.
///
/// Computed via binary search on cumulative Y offsets.
/// Indices are into the conversation's entry list.
///
/// # Invariants
/// - `start_index <= end_index`
/// - `end_index <= entries.len()`
/// - All entries in range have some portion visible in viewport
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VisibleRange {
    /// Index of first visible entry (inclusive).
    pub start_index: EntryIndex,
    /// Index of last visible entry (exclusive).
    pub end_index: EntryIndex,
    /// Scroll offset (resolved from ScrollPosition).
    pub scroll_offset: LineOffset,
    /// Viewport height in lines.
    pub viewport_height: u16,
}

impl VisibleRange {
    /// Create new visible range.
    ///
    /// # Panics
    /// In debug builds, panics if start_index > end_index.
    pub fn new(
        _start_index: EntryIndex,
        _end_index: EntryIndex,
        _scroll_offset: LineOffset,
        _viewport_height: u16,
    ) -> Self {
        todo!("VisibleRange::new")
    }

    /// Number of visible entries.
    pub fn len(&self) -> usize {
        todo!("VisibleRange::len")
    }

    /// Check if range is empty.
    pub fn is_empty(&self) -> bool {
        todo!("VisibleRange::is_empty")
    }

    /// Iterate over visible entry indices.
    pub fn indices(&self) -> impl Iterator<Item = EntryIndex> {
        std::iter::empty()
    }

    /// Check if a specific entry index is visible.
    pub fn contains(&self, _index: EntryIndex) -> bool {
        todo!("VisibleRange::contains")
    }
}

impl Default for VisibleRange {
    fn default() -> Self {
        todo!("VisibleRange::default")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod construction {
        use super::*;

        #[test]
        fn new_creates_range_with_given_values() {
            let range = VisibleRange::new(
                EntryIndex::new(5),
                EntryIndex::new(10),
                LineOffset::new(100),
                24,
            );
            assert_eq!(range.start_index, EntryIndex::new(5));
            assert_eq!(range.end_index, EntryIndex::new(10));
            assert_eq!(range.scroll_offset, LineOffset::new(100));
            assert_eq!(range.viewport_height, 24);
        }

        #[test]
        fn new_accepts_equal_indices() {
            let range = VisibleRange::new(
                EntryIndex::new(5),
                EntryIndex::new(5),
                LineOffset::new(0),
                24,
            );
            assert_eq!(range.start_index, EntryIndex::new(5));
            assert_eq!(range.end_index, EntryIndex::new(5));
        }

        #[test]
        #[should_panic]
        #[cfg(debug_assertions)]
        fn new_panics_when_start_greater_than_end() {
            VisibleRange::new(
                EntryIndex::new(10),
                EntryIndex::new(5),
                LineOffset::new(0),
                24,
            );
        }

        #[test]
        fn default_creates_empty_range_at_zero() {
            let range = VisibleRange::default();
            assert_eq!(range.start_index, EntryIndex::default());
            assert_eq!(range.end_index, EntryIndex::default());
            assert_eq!(range.scroll_offset, LineOffset::default());
            assert_eq!(range.viewport_height, 0);
        }
    }

    mod length_and_empty {
        use super::*;

        #[test]
        fn len_returns_difference_between_indices() {
            let range = VisibleRange::new(
                EntryIndex::new(5),
                EntryIndex::new(10),
                LineOffset::new(0),
                24,
            );
            assert_eq!(range.len(), 5);
        }

        #[test]
        fn len_returns_zero_for_equal_indices() {
            let range = VisibleRange::new(
                EntryIndex::new(5),
                EntryIndex::new(5),
                LineOffset::new(0),
                24,
            );
            assert_eq!(range.len(), 0);
        }

        #[test]
        fn is_empty_true_when_start_equals_end() {
            let range = VisibleRange::new(
                EntryIndex::new(5),
                EntryIndex::new(5),
                LineOffset::new(0),
                24,
            );
            assert!(range.is_empty());
        }

        #[test]
        fn is_empty_false_when_start_less_than_end() {
            let range = VisibleRange::new(
                EntryIndex::new(5),
                EntryIndex::new(10),
                LineOffset::new(0),
                24,
            );
            assert!(!range.is_empty());
        }
    }

    mod indices_iterator {
        use super::*;

        #[test]
        fn indices_iterates_from_start_to_end_exclusive() {
            let range = VisibleRange::new(
                EntryIndex::new(5),
                EntryIndex::new(8),
                LineOffset::new(0),
                24,
            );
            let indices: Vec<_> = range.indices().collect();
            assert_eq!(
                indices,
                vec![EntryIndex::new(5), EntryIndex::new(6), EntryIndex::new(7)]
            );
        }

        #[test]
        fn indices_returns_empty_iterator_when_range_empty() {
            let range = VisibleRange::new(
                EntryIndex::new(5),
                EntryIndex::new(5),
                LineOffset::new(0),
                24,
            );
            let indices: Vec<_> = range.indices().collect();
            assert_eq!(indices, vec![]);
        }

        #[test]
        fn indices_works_from_zero() {
            let range = VisibleRange::new(
                EntryIndex::new(0),
                EntryIndex::new(3),
                LineOffset::new(0),
                24,
            );
            let indices: Vec<_> = range.indices().collect();
            assert_eq!(
                indices,
                vec![EntryIndex::new(0), EntryIndex::new(1), EntryIndex::new(2)]
            );
        }

        #[test]
        fn indices_iterator_count_matches_len() {
            let range = VisibleRange::new(
                EntryIndex::new(10),
                EntryIndex::new(20),
                LineOffset::new(0),
                24,
            );
            assert_eq!(range.indices().count(), range.len());
        }
    }

    mod contains {
        use super::*;

        #[test]
        fn contains_true_for_start_index() {
            let range = VisibleRange::new(
                EntryIndex::new(5),
                EntryIndex::new(10),
                LineOffset::new(0),
                24,
            );
            assert!(range.contains(EntryIndex::new(5)));
        }

        #[test]
        fn contains_true_for_middle_index() {
            let range = VisibleRange::new(
                EntryIndex::new(5),
                EntryIndex::new(10),
                LineOffset::new(0),
                24,
            );
            assert!(range.contains(EntryIndex::new(7)));
        }

        #[test]
        fn contains_false_for_end_index() {
            let range = VisibleRange::new(
                EntryIndex::new(5),
                EntryIndex::new(10),
                LineOffset::new(0),
                24,
            );
            assert!(!range.contains(EntryIndex::new(10)));
        }

        #[test]
        fn contains_false_for_index_before_start() {
            let range = VisibleRange::new(
                EntryIndex::new(5),
                EntryIndex::new(10),
                LineOffset::new(0),
                24,
            );
            assert!(!range.contains(EntryIndex::new(4)));
        }

        #[test]
        fn contains_false_for_index_after_end() {
            let range = VisibleRange::new(
                EntryIndex::new(5),
                EntryIndex::new(10),
                LineOffset::new(0),
                24,
            );
            assert!(!range.contains(EntryIndex::new(11)));
        }

        #[test]
        fn contains_false_for_empty_range() {
            let range = VisibleRange::new(
                EntryIndex::new(5),
                EntryIndex::new(5),
                LineOffset::new(0),
                24,
            );
            assert!(!range.contains(EntryIndex::new(5)));
        }
    }
}
