//! Core view-state newtypes

/// Height of an entry in lines. Always >= 1 for valid entries.
/// LineHeight::ZERO is a sentinel for malformed entries that don't render.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct LineHeight(u16);

/// Error returned when attempting to create a LineHeight of zero via the smart constructor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("LineHeight must be >= 1 for valid entries (got {0})")]
pub struct InvalidLineHeight(pub u16);

impl LineHeight {
    /// Sentinel value for malformed entries that don't render.
    pub const ZERO: Self = Self(0);

    /// Minimum valid line height for renderable entries.
    pub const ONE: Self = Self(1);

    /// Smart constructor that validates line height is >= 1.
    pub fn new(height: u16) -> Result<Self, InvalidLineHeight> {
        if height == 0 {
            Err(InvalidLineHeight(height))
        } else {
            Ok(Self(height))
        }
    }

    /// Get the raw u16 value.
    pub fn get(&self) -> u16 {
        self.0
    }

    /// Check if this is the ZERO sentinel value.
    pub fn is_zero(&self) -> bool {
        self.0 == 0
    }
}

impl Default for LineHeight {
    fn default() -> Self {
        Self::ONE
    }
}

/// Absolute line offset from start of conversation. 0-indexed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct LineOffset(usize);

impl LineOffset {
    /// Create a new LineOffset from a raw value.
    pub fn new(offset: usize) -> Self {
        Self(offset)
    }

    /// Get the raw usize value.
    pub fn get(&self) -> usize {
        self.0
    }

    /// Add an amount to this offset, saturating at usize::MAX.
    pub fn saturating_add(&self, amount: usize) -> Self {
        Self(self.0.saturating_add(amount))
    }

    /// Subtract an amount from this offset, saturating at 0.
    pub fn saturating_sub(&self, amount: usize) -> Self {
        Self(self.0.saturating_sub(amount))
    }
}

/// Entry index within conversation. 0-indexed internally, 1-based for display.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct EntryIndex(usize);

impl EntryIndex {
    /// Create a new EntryIndex from a raw 0-based value.
    pub fn new(index: usize) -> Self {
        Self(index)
    }

    /// Get the raw 0-based index value.
    pub fn get(&self) -> usize {
        self.0
    }

    /// Get the 1-based index for display purposes.
    pub fn display(&self) -> usize {
        self.0 + 1
    }

    /// Get the next entry index.
    pub fn next(&self) -> Self {
        Self(self.0 + 1)
    }

    /// Get the previous entry index, saturating at 0.
    pub fn prev(&self) -> Self {
        Self(self.0.saturating_sub(1))
    }
}

impl From<usize> for EntryIndex {
    fn from(index: usize) -> Self {
        Self(index)
    }
}

/// Viewport dimensions in terminal cells.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ViewportDimensions {
    /// Width in terminal columns.
    pub width: u16,
    /// Height in terminal rows.
    pub height: u16,
}

impl ViewportDimensions {
    /// Create new viewport dimensions.
    pub fn new(width: u16, height: u16) -> Self {
        Self { width, height }
    }
}

/// Validated index into LogViewState.sessions.
///
/// # Invariants
/// - Always < session_count at construction time
/// - 0-indexed: 0 is the first session
///
/// # Smart Constructor
/// Use `SessionIndex::new(index, session_count)` which returns `Option<Self>`.
/// Never export the raw constructor.
///
/// # Cardinality
/// - Valid states: [0, session_count)
/// - Total states: [0, usize::MAX)
/// - Precision: session_count / usize::MAX ≈ 1.0 for typical session counts
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SessionIndex(usize);

impl SessionIndex {
    /// Create a validated session index.
    ///
    /// Returns `None` if index >= session_count.
    ///
    /// # Examples
    /// ```
    /// # use cclv::view_state::types::SessionIndex;
    /// let idx = SessionIndex::new(0, 3); // Some(SessionIndex(0))
    /// let idx = SessionIndex::new(3, 3); // None (out of bounds)
    /// ```
    pub fn new(_index: usize, _session_count: usize) -> Option<Self> {
        todo!("SessionIndex::new")
    }

    /// Get the raw index value.
    pub fn get(&self) -> usize {
        todo!("SessionIndex::get")
    }

    /// Display index (1-based, for user-facing display).
    pub fn display(&self) -> usize {
        todo!("SessionIndex::display")
    }

    /// Check if this is the last session.
    ///
    /// Used to determine if live tailing should be enabled.
    pub fn is_last(&self, _session_count: usize) -> bool {
        todo!("SessionIndex::is_last")
    }

    /// Check if this is the first session.
    pub fn is_first(&self) -> bool {
        todo!("SessionIndex::is_first")
    }

    /// Next session index, if valid.
    pub fn next(&self, _session_count: usize) -> Option<Self> {
        todo!("SessionIndex::next")
    }

    /// Previous session index, if valid.
    pub fn prev(&self) -> Option<Self> {
        todo!("SessionIndex::prev")
    }
}

impl std::fmt::Display for SessionIndex {
    fn fmt(&self, _f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        todo!("SessionIndex::Display")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod line_height {
        use super::*;

        #[test]
        fn new_accepts_one() {
            let result = LineHeight::new(1);
            assert!(result.is_ok());
            assert_eq!(result.unwrap(), LineHeight::ONE);
        }

        #[test]
        fn new_accepts_greater_than_one() {
            let result = LineHeight::new(42);
            assert!(result.is_ok());
            assert_eq!(result.unwrap().get(), 42);
        }

        #[test]
        fn new_rejects_zero() {
            let result = LineHeight::new(0);
            assert!(result.is_err());
            assert_eq!(result.unwrap_err(), InvalidLineHeight(0));
        }

        #[test]
        fn zero_sentinel_is_zero() {
            assert!(LineHeight::ZERO.is_zero());
        }

        #[test]
        fn one_is_not_zero() {
            assert!(!LineHeight::ONE.is_zero());
        }

        #[test]
        fn valid_height_is_not_zero() {
            let height = LineHeight::new(5).unwrap();
            assert!(!height.is_zero());
        }

        #[test]
        fn default_is_one() {
            assert_eq!(LineHeight::default(), LineHeight::ONE);
        }

        #[test]
        fn get_returns_raw_value() {
            let height = LineHeight::new(10).unwrap();
            assert_eq!(height.get(), 10);
        }

        #[test]
        fn ordering_works() {
            let h1 = LineHeight::new(1).unwrap();
            let h2 = LineHeight::new(2).unwrap();
            assert!(h1 < h2);
            assert!(LineHeight::ZERO < h1);
        }
    }

    mod line_offset {
        use super::*;

        #[test]
        fn new_creates_offset() {
            let offset = LineOffset::new(42);
            assert_eq!(offset.get(), 42);
        }

        #[test]
        fn default_is_zero() {
            let offset = LineOffset::default();
            assert_eq!(offset.get(), 0);
        }

        #[test]
        fn saturating_add_normal_case() {
            let offset = LineOffset::new(10);
            let result = offset.saturating_add(5);
            assert_eq!(result.get(), 15);
        }

        #[test]
        fn saturating_add_at_max() {
            let offset = LineOffset::new(usize::MAX);
            let result = offset.saturating_add(100);
            assert_eq!(result.get(), usize::MAX);
        }

        #[test]
        fn saturating_add_near_max() {
            let offset = LineOffset::new(usize::MAX - 1);
            let result = offset.saturating_add(5);
            assert_eq!(result.get(), usize::MAX);
        }

        #[test]
        fn saturating_sub_normal_case() {
            let offset = LineOffset::new(10);
            let result = offset.saturating_sub(5);
            assert_eq!(result.get(), 5);
        }

        #[test]
        fn saturating_sub_at_zero() {
            let offset = LineOffset::new(0);
            let result = offset.saturating_sub(100);
            assert_eq!(result.get(), 0);
        }

        #[test]
        fn saturating_sub_near_zero() {
            let offset = LineOffset::new(2);
            let result = offset.saturating_sub(5);
            assert_eq!(result.get(), 0);
        }

        #[test]
        fn ordering_works() {
            let o1 = LineOffset::new(5);
            let o2 = LineOffset::new(10);
            assert!(o1 < o2);
        }
    }

    mod entry_index {
        use super::*;

        #[test]
        fn new_creates_index() {
            let index = EntryIndex::new(42);
            assert_eq!(index.get(), 42);
        }

        #[test]
        fn default_is_zero() {
            let index = EntryIndex::default();
            assert_eq!(index.get(), 0);
        }

        #[test]
        fn display_returns_one_based() {
            let index = EntryIndex::new(0);
            assert_eq!(index.display(), 1);
        }

        #[test]
        fn display_for_later_entries() {
            let index = EntryIndex::new(5);
            assert_eq!(index.display(), 6);
        }

        #[test]
        fn next_increments() {
            let index = EntryIndex::new(5);
            assert_eq!(index.next().get(), 6);
        }

        #[test]
        fn next_from_zero() {
            let index = EntryIndex::new(0);
            assert_eq!(index.next().get(), 1);
        }

        #[test]
        fn prev_decrements() {
            let index = EntryIndex::new(5);
            assert_eq!(index.prev().get(), 4);
        }

        #[test]
        fn prev_saturates_at_zero() {
            let index = EntryIndex::new(0);
            assert_eq!(index.prev().get(), 0);
        }

        #[test]
        fn prev_from_one() {
            let index = EntryIndex::new(1);
            assert_eq!(index.prev().get(), 0);
        }

        #[test]
        fn from_usize_conversion() {
            let index: EntryIndex = 42.into();
            assert_eq!(index.get(), 42);
        }

        #[test]
        fn ordering_works() {
            let i1 = EntryIndex::new(5);
            let i2 = EntryIndex::new(10);
            assert!(i1 < i2);
        }

        #[test]
        fn hash_works() {
            use std::collections::HashSet;
            let mut set = HashSet::new();
            set.insert(EntryIndex::new(1));
            set.insert(EntryIndex::new(2));
            set.insert(EntryIndex::new(1)); // Duplicate
            assert_eq!(set.len(), 2);
        }
    }

    mod viewport_dimensions {
        use super::*;

        #[test]
        fn new_creates_dimensions() {
            let dims = ViewportDimensions::new(80, 24);
            assert_eq!(dims.width, 80);
            assert_eq!(dims.height, 24);
        }

        #[test]
        fn equality_works() {
            let dims1 = ViewportDimensions::new(80, 24);
            let dims2 = ViewportDimensions::new(80, 24);
            let dims3 = ViewportDimensions::new(100, 30);
            assert_eq!(dims1, dims2);
            assert_ne!(dims1, dims3);
        }

        #[test]
        fn debug_formatting() {
            let dims = ViewportDimensions::new(80, 24);
            let debug_str = format!("{:?}", dims);
            assert!(debug_str.contains("80"));
            assert!(debug_str.contains("24"));
        }
    }

    mod session_index {
        use super::*;

        #[test]
        fn new_accepts_valid_index() {
            let result = SessionIndex::new(0, 3);
            assert!(result.is_some());
        }

        #[test]
        fn new_accepts_middle_index() {
            let result = SessionIndex::new(1, 3);
            assert!(result.is_some());
        }

        #[test]
        fn new_accepts_last_valid_index() {
            let result = SessionIndex::new(2, 3);
            assert!(result.is_some());
        }

        #[test]
        fn new_rejects_out_of_bounds() {
            let result = SessionIndex::new(3, 3);
            assert!(result.is_none());
        }

        #[test]
        fn new_rejects_far_out_of_bounds() {
            let result = SessionIndex::new(100, 3);
            assert!(result.is_none());
        }

        #[test]
        fn get_returns_raw_index() {
            let index = SessionIndex::new(0, 3).unwrap();
            assert_eq!(index.get(), 0);
        }

        #[test]
        fn get_returns_raw_middle_index() {
            let index = SessionIndex::new(5, 10).unwrap();
            assert_eq!(index.get(), 5);
        }

        #[test]
        fn display_returns_one_based_for_first() {
            let index = SessionIndex::new(0, 3).unwrap();
            assert_eq!(index.display(), 1);
        }

        #[test]
        fn display_returns_one_based_for_middle() {
            let index = SessionIndex::new(1, 3).unwrap();
            assert_eq!(index.display(), 2);
        }

        #[test]
        fn display_returns_one_based_for_last() {
            let index = SessionIndex::new(2, 3).unwrap();
            assert_eq!(index.display(), 3);
        }

        #[test]
        fn is_last_true_for_last_session() {
            let index = SessionIndex::new(2, 3).unwrap();
            assert!(index.is_last(3));
        }

        #[test]
        fn is_last_false_for_first_session() {
            let index = SessionIndex::new(0, 3).unwrap();
            assert!(!index.is_last(3));
        }

        #[test]
        fn is_last_false_for_middle_session() {
            let index = SessionIndex::new(1, 3).unwrap();
            assert!(!index.is_last(3));
        }

        #[test]
        fn is_first_true_for_first_session() {
            let index = SessionIndex::new(0, 3).unwrap();
            assert!(index.is_first());
        }

        #[test]
        fn is_first_false_for_middle_session() {
            let index = SessionIndex::new(1, 3).unwrap();
            assert!(!index.is_first());
        }

        #[test]
        fn is_first_false_for_last_session() {
            let index = SessionIndex::new(2, 3).unwrap();
            assert!(!index.is_first());
        }

        #[test]
        fn next_returns_some_when_valid() {
            let index = SessionIndex::new(1, 3).unwrap();
            let next = index.next(3);
            assert!(next.is_some());
            assert_eq!(next.unwrap().get(), 2);
        }

        #[test]
        fn next_from_first() {
            let index = SessionIndex::new(0, 3).unwrap();
            let next = index.next(3);
            assert!(next.is_some());
            assert_eq!(next.unwrap().get(), 1);
        }

        #[test]
        fn next_returns_none_at_last() {
            let index = SessionIndex::new(2, 3).unwrap();
            let next = index.next(3);
            assert!(next.is_none());
        }

        #[test]
        fn prev_returns_some_when_valid() {
            let index = SessionIndex::new(1, 3).unwrap();
            let prev = index.prev();
            assert!(prev.is_some());
            assert_eq!(prev.unwrap().get(), 0);
        }

        #[test]
        fn prev_from_last() {
            let index = SessionIndex::new(2, 3).unwrap();
            let prev = index.prev();
            assert!(prev.is_some());
            assert_eq!(prev.unwrap().get(), 1);
        }

        #[test]
        fn prev_returns_none_at_first() {
            let index = SessionIndex::new(0, 3).unwrap();
            let prev = index.prev();
            assert!(prev.is_none());
        }

        #[test]
        fn display_trait_shows_one_based() {
            let index = SessionIndex::new(0, 3).unwrap();
            let display_str = format!("{}", index);
            assert_eq!(display_str, "1");
        }

        #[test]
        fn display_trait_for_middle() {
            let index = SessionIndex::new(5, 10).unwrap();
            let display_str = format!("{}", index);
            assert_eq!(display_str, "6");
        }

        #[test]
        fn ordering_works() {
            let i1 = SessionIndex::new(0, 3).unwrap();
            let i2 = SessionIndex::new(1, 3).unwrap();
            let i3 = SessionIndex::new(2, 3).unwrap();
            assert!(i1 < i2);
            assert!(i2 < i3);
            assert!(i1 < i3);
        }

        #[test]
        fn equality_works() {
            let i1 = SessionIndex::new(1, 3).unwrap();
            let i2 = SessionIndex::new(1, 5).unwrap(); // Different session_count, same index
            let i3 = SessionIndex::new(2, 3).unwrap();
            assert_eq!(i1, i2); // Equality based on index only
            assert_ne!(i1, i3);
        }

        #[test]
        fn hash_works() {
            use std::collections::HashSet;
            let mut set = HashSet::new();
            set.insert(SessionIndex::new(0, 3).unwrap());
            set.insert(SessionIndex::new(1, 3).unwrap());
            set.insert(SessionIndex::new(0, 3).unwrap()); // Duplicate
            assert_eq!(set.len(), 2);
        }

        #[test]
        fn debug_formatting() {
            let index = SessionIndex::new(5, 10).unwrap();
            let debug_str = format!("{:?}", index);
            assert!(debug_str.contains("SessionIndex"));
        }
    }
}
