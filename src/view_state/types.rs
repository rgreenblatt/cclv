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
}
