//! Mouse hit-testing results

use super::types::EntryIndex;

/// Result of hit-testing a screen coordinate.
///
/// Determines what entry (if any) was clicked and where.
/// Uses `EntryIndex` as the canonical reference for entries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HitTestResult {
    /// Click was outside any entry bounds.
    Miss,

    /// Click hit an entry.
    Hit {
        /// Index of the hit entry (canonical reference).
        entry_index: EntryIndex,
        /// Line within the entry that was hit (0-indexed).
        line_in_entry: usize,
        /// Column within the line (0-indexed).
        column: u16,
    },
}

impl HitTestResult {
    /// Create a miss result.
    pub fn miss() -> Self {
        todo!("HitTestResult::miss")
    }

    /// Create a hit result.
    pub fn hit(_entry_index: EntryIndex, _line_in_entry: usize, _column: u16) -> Self {
        todo!("HitTestResult::hit")
    }

    /// Check if this was a hit.
    pub fn is_hit(&self) -> bool {
        todo!("HitTestResult::is_hit")
    }

    /// Get entry index if hit.
    pub fn entry_index(&self) -> Option<EntryIndex> {
        todo!("HitTestResult::entry_index")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod miss_constructor {
        use super::*;

        #[test]
        fn creates_miss_variant() {
            let result = HitTestResult::miss();
            assert_eq!(result, HitTestResult::Miss);
        }
    }

    mod hit_constructor {
        use super::*;

        #[test]
        fn creates_hit_variant_with_correct_fields() {
            let entry_index = EntryIndex::new(5);
            let line_in_entry = 3;
            let column = 42;

            let result = HitTestResult::hit(entry_index, line_in_entry, column);

            match result {
                HitTestResult::Hit {
                    entry_index: idx,
                    line_in_entry: line,
                    column: col,
                } => {
                    assert_eq!(idx, entry_index);
                    assert_eq!(line, 3);
                    assert_eq!(col, 42);
                }
                HitTestResult::Miss => panic!("Expected Hit, got Miss"),
            }
        }

        #[test]
        fn works_with_zero_index() {
            let result = HitTestResult::hit(EntryIndex::new(0), 0, 0);
            assert!(matches!(result, HitTestResult::Hit { .. }));
        }

        #[test]
        fn works_with_large_values() {
            let result = HitTestResult::hit(EntryIndex::new(9999), 1000, 500);
            match result {
                HitTestResult::Hit {
                    entry_index,
                    line_in_entry,
                    column,
                } => {
                    assert_eq!(entry_index.get(), 9999);
                    assert_eq!(line_in_entry, 1000);
                    assert_eq!(column, 500);
                }
                HitTestResult::Miss => panic!("Expected Hit, got Miss"),
            }
        }
    }

    mod is_hit {
        use super::*;

        #[test]
        fn returns_true_for_hit_variant() {
            let result = HitTestResult::hit(EntryIndex::new(0), 0, 0);
            assert!(result.is_hit());
        }

        #[test]
        fn returns_false_for_miss_variant() {
            let result = HitTestResult::miss();
            assert!(!result.is_hit());
        }
    }

    mod entry_index_accessor {
        use super::*;

        #[test]
        fn returns_some_for_hit_variant() {
            let entry_index = EntryIndex::new(10);
            let result = HitTestResult::hit(entry_index, 5, 20);
            assert_eq!(result.entry_index(), Some(entry_index));
        }

        #[test]
        fn returns_none_for_miss_variant() {
            let result = HitTestResult::miss();
            assert_eq!(result.entry_index(), None);
        }

        #[test]
        fn returns_correct_index_for_zero() {
            let result = HitTestResult::hit(EntryIndex::new(0), 0, 0);
            assert_eq!(result.entry_index(), Some(EntryIndex::new(0)));
        }
    }

    mod derived_traits {
        use super::*;

        #[test]
        fn clone_creates_equal_copy() {
            let original = HitTestResult::hit(EntryIndex::new(5), 3, 42);
            let cloned = original.clone();
            assert_eq!(original, cloned);
        }

        #[test]
        fn clone_miss_creates_equal_copy() {
            let original = HitTestResult::miss();
            let cloned = original.clone();
            assert_eq!(original, cloned);
        }

        #[test]
        fn debug_formatting_works_for_miss() {
            let result = HitTestResult::miss();
            let debug_str = format!("{:?}", result);
            assert!(debug_str.contains("Miss"));
        }

        #[test]
        fn debug_formatting_works_for_hit() {
            let result = HitTestResult::hit(EntryIndex::new(5), 3, 42);
            let debug_str = format!("{:?}", result);
            assert!(debug_str.contains("Hit"));
            assert!(debug_str.contains("entry_index"));
        }

        #[test]
        fn equality_works_for_identical_hits() {
            let result1 = HitTestResult::hit(EntryIndex::new(5), 3, 42);
            let result2 = HitTestResult::hit(EntryIndex::new(5), 3, 42);
            assert_eq!(result1, result2);
        }

        #[test]
        fn equality_works_for_different_hits() {
            let result1 = HitTestResult::hit(EntryIndex::new(5), 3, 42);
            let result2 = HitTestResult::hit(EntryIndex::new(6), 3, 42);
            assert_ne!(result1, result2);
        }

        #[test]
        fn equality_works_for_misses() {
            let result1 = HitTestResult::miss();
            let result2 = HitTestResult::miss();
            assert_eq!(result1, result2);
        }

        #[test]
        fn hit_not_equal_to_miss() {
            let hit = HitTestResult::hit(EntryIndex::new(0), 0, 0);
            let miss = HitTestResult::miss();
            assert_ne!(hit, miss);
        }
    }
}
