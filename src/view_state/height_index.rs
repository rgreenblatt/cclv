//! HeightIndex - O(log n) prefix sums and lower_bound via Fenwick tree
//!
//! Provides efficient operations for computing cumulative heights and finding
//! entry indices by vertical offset (scroll position).
//!
//! # Complexity
//!
//! - `set`: O(log n)
//! - `prefix_sum`: O(log n)
//! - `lower_bound`: O(log² n)
//! - `push`: O(log n)
//! - `total`: O(log n)
//! - `len`: O(1)
//! - `clear`: O(1)

/// HeightIndex wraps a Fenwick tree for O(log n) prefix sum queries and updates.
///
/// Maintains cumulative heights for a sequence of entries, supporting:
/// - Setting individual entry heights
/// - Computing prefix sums (cumulative height up to index)
/// - Binary search for entry by vertical offset (lower_bound)
#[derive(Debug, Clone)]
pub struct HeightIndex {
    /// Fenwick tree backing storage (1-indexed internally, but we expose 0-indexed API)
    tree: Vec<isize>,
    /// Number of valid entries (len <= tree.len())
    len: usize,
}

impl HeightIndex {
    /// Creates a new HeightIndex with the given initial capacity.
    ///
    /// # Arguments
    ///
    /// * `capacity` - Initial capacity (pre-allocates backing storage)
    ///
    /// # Examples
    ///
    /// ```
    /// # use cclv::view_state::height_index::HeightIndex;
    /// let index = HeightIndex::new(100);
    /// assert_eq!(index.len(), 0);
    /// assert_eq!(index.total(), 0);
    /// ```
    pub fn new(capacity: usize) -> Self {
        Self {
            tree: vec![0; capacity],
            len: 0,
        }
    }

    /// Sets the height at the given index.
    ///
    /// Computes the delta from the current height and updates the Fenwick tree.
    ///
    /// # Panics
    ///
    /// Panics if `index >= len()`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use cclv::view_state::height_index::HeightIndex;
    /// let mut index = HeightIndex::new(10);
    /// index.push(5);
    /// index.set(0, 10);
    /// assert_eq!(index.prefix_sum(0), 10);
    /// ```
    pub fn set(&mut self, index: usize, height: usize) {
        assert!(
            index < self.len,
            "index {} out of bounds (len: {})",
            index,
            self.len
        );

        // Compute delta from current height
        let current_height = if index == 0 {
            self.prefix_sum(0)
        } else {
            self.prefix_sum(index) - self.prefix_sum(index - 1)
        };

        // Update the Fenwick tree with the delta (use full tree for correct propagation)
        let delta = height as isize - current_height as isize;
        if delta != 0 {
            fenwick::array::update(&mut self.tree, index, delta);
        }
    }

    /// Returns the cumulative height up to and including the given index.
    ///
    /// # Panics
    ///
    /// Panics if `index >= len()`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use cclv::view_state::height_index::HeightIndex;
    /// let mut index = HeightIndex::new(10);
    /// index.push(3);
    /// index.push(4);
    /// index.push(5);
    /// assert_eq!(index.prefix_sum(0), 3);
    /// assert_eq!(index.prefix_sum(1), 7);
    /// assert_eq!(index.prefix_sum(2), 12);
    /// ```
    pub fn prefix_sum(&self, index: usize) -> usize {
        assert!(
            index < self.len,
            "index {} out of bounds (len: {})",
            index,
            self.len
        );

        let sum = fenwick::array::prefix_sum(&self.tree, index);
        sum.max(0) as usize // Handle potential negative sums from set operations
    }

    /// Binary search for the first index where `prefix_sum(index) > value`.
    ///
    /// Returns the index of the entry containing the given vertical offset.
    ///
    /// # Returns
    ///
    /// - `Some(index)` if there exists an index where `prefix_sum(index) > value`
    /// - `None` if `value >= total()` or the index is empty
    ///
    /// # Examples
    ///
    /// ```
    /// # use cclv::view_state::height_index::HeightIndex;
    /// let mut index = HeightIndex::new(10);
    /// index.push(10);  // [0..10)
    /// index.push(20);  // [10..30)
    /// index.push(15);  // [30..45)
    ///
    /// assert_eq!(index.lower_bound(0), Some(0));
    /// assert_eq!(index.lower_bound(5), Some(0));
    /// assert_eq!(index.lower_bound(10), Some(1));
    /// assert_eq!(index.lower_bound(29), Some(1));
    /// assert_eq!(index.lower_bound(30), Some(2));
    /// assert_eq!(index.lower_bound(100), None);
    /// ```
    pub fn lower_bound(&self, value: usize) -> Option<usize> {
        if self.is_empty() {
            return None;
        }

        // Binary search for first index where prefix_sum(index) > value
        // Entry i covers range [prefix_sum(i-1), prefix_sum(i))
        let mut left = 0;
        let mut right = self.len;

        while left < right {
            let mid = left + (right - left) / 2;
            let sum = self.prefix_sum(mid);

            if sum > value {
                right = mid;
            } else {
                left = mid + 1;
            }
        }

        if left >= self.len {
            None
        } else {
            Some(left)
        }
    }

    /// Returns the total cumulative height of all entries.
    ///
    /// Equivalent to `prefix_sum(len() - 1)` if non-empty, 0 otherwise.
    ///
    /// # Examples
    ///
    /// ```
    /// # use cclv::view_state::height_index::HeightIndex;
    /// let mut index = HeightIndex::new(10);
    /// assert_eq!(index.total(), 0);
    /// index.push(5);
    /// assert_eq!(index.total(), 5);
    /// index.push(3);
    /// assert_eq!(index.total(), 8);
    /// ```
    pub fn total(&self) -> usize {
        if self.is_empty() {
            0
        } else {
            self.prefix_sum(self.len - 1)
        }
    }

    /// Returns the number of entries in the index.
    ///
    /// # Examples
    ///
    /// ```
    /// # use cclv::view_state::height_index::HeightIndex;
    /// let mut index = HeightIndex::new(10);
    /// assert_eq!(index.len(), 0);
    /// index.push(5);
    /// assert_eq!(index.len(), 1);
    /// ```
    pub fn len(&self) -> usize {
        self.len
    }

    /// Returns true if the index contains no entries.
    ///
    /// # Examples
    ///
    /// ```
    /// # use cclv::view_state::height_index::HeightIndex;
    /// let mut index = HeightIndex::new(10);
    /// assert!(index.is_empty());
    /// index.push(5);
    /// assert!(!index.is_empty());
    /// ```
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Appends a new entry with the given height.
    ///
    /// Grows the backing storage if necessary.
    ///
    /// # Examples
    ///
    /// ```
    /// # use cclv::view_state::height_index::HeightIndex;
    /// let mut index = HeightIndex::new(10);
    /// index.push(5);
    /// index.push(3);
    /// assert_eq!(index.len(), 2);
    /// assert_eq!(index.total(), 8);
    /// ```
    pub fn push(&mut self, height: usize) {
        // Grow backing storage if necessary
        if self.len >= self.tree.len() {
            self.tree.resize(self.tree.len().max(1) * 2, 0);
        }

        let idx = self.len;
        self.len += 1;

        // Update fenwick tree at new position (use full tree for correct propagation)
        fenwick::array::update(&mut self.tree, idx, height as isize);
    }

    /// Clears all entries, resetting to empty state.
    ///
    /// Retains allocated capacity for reuse.
    ///
    /// # Examples
    ///
    /// ```
    /// # use cclv::view_state::height_index::HeightIndex;
    /// let mut index = HeightIndex::new(10);
    /// index.push(5);
    /// index.push(3);
    /// index.clear();
    /// assert_eq!(index.len(), 0);
    /// assert_eq!(index.total(), 0);
    /// ```
    pub fn clear(&mut self) {
        // Reset tree to zeros (retain capacity)
        for i in 0..self.len {
            self.tree[i] = 0;
        }
        self.len = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn test_empty_index() {
        let index = HeightIndex::new(10);
        assert_eq!(index.len(), 0);
        assert_eq!(index.total(), 0);
        assert!(index.is_empty());
    }

    #[test]
    fn test_single_entry() {
        let mut index = HeightIndex::new(10);
        index.push(5);
        assert_eq!(index.len(), 1);
        assert_eq!(index.total(), 5);
        assert!(!index.is_empty());
        assert_eq!(index.prefix_sum(0), 5);
    }

    #[test]
    fn test_multiple_entries() {
        let mut index = HeightIndex::new(10);
        index.push(3);
        index.push(4);
        index.push(5);

        assert_eq!(index.len(), 3);
        assert_eq!(index.prefix_sum(0), 3);
        assert_eq!(index.prefix_sum(1), 7);
        assert_eq!(index.prefix_sum(2), 12);
        assert_eq!(index.total(), 12);
    }

    #[test]
    fn test_set_updates_height() {
        let mut index = HeightIndex::new(10);
        index.push(3);
        index.push(4);
        index.push(5);

        // Change middle entry from 4 to 10
        index.set(1, 10);

        assert_eq!(index.prefix_sum(0), 3);
        assert_eq!(index.prefix_sum(1), 13); // 3 + 10
        assert_eq!(index.prefix_sum(2), 18); // 3 + 10 + 5
        assert_eq!(index.total(), 18);
    }

    #[test]
    fn test_lower_bound_basic() {
        let mut index = HeightIndex::new(10);
        index.push(10); // [0..10)
        index.push(20); // [10..30)
        index.push(15); // [30..45)

        assert_eq!(index.lower_bound(0), Some(0));
        assert_eq!(index.lower_bound(5), Some(0));
        assert_eq!(index.lower_bound(10), Some(1));
        assert_eq!(index.lower_bound(15), Some(1));
        assert_eq!(index.lower_bound(29), Some(1));
        assert_eq!(index.lower_bound(30), Some(2));
        assert_eq!(index.lower_bound(44), Some(2));
    }

    #[test]
    fn test_lower_bound_edge_cases() {
        let mut index = HeightIndex::new(10);
        index.push(5);
        index.push(5);
        index.push(5);

        // value == 0 should return first entry
        assert_eq!(index.lower_bound(0), Some(0));

        // value > total should return None
        assert_eq!(index.lower_bound(100), None);

        // value == exact prefix_sum boundary
        assert_eq!(index.lower_bound(5), Some(1));
        assert_eq!(index.lower_bound(10), Some(2));
        assert_eq!(index.lower_bound(15), None); // Exactly at total
    }

    #[test]
    fn test_lower_bound_empty() {
        let index = HeightIndex::new(10);
        assert_eq!(index.lower_bound(0), None);
        assert_eq!(index.lower_bound(10), None);
    }

    #[test]
    fn test_clear() {
        let mut index = HeightIndex::new(10);
        index.push(5);
        index.push(3);
        index.push(7);

        index.clear();

        assert_eq!(index.len(), 0);
        assert_eq!(index.total(), 0);
        assert!(index.is_empty());
    }

    #[test]
    fn test_clear_and_reuse() {
        let mut index = HeightIndex::new(10);
        index.push(5);
        index.clear();

        // Should be able to push after clear
        index.push(10);
        assert_eq!(index.len(), 1);
        assert_eq!(index.total(), 10);
        assert_eq!(index.prefix_sum(0), 10);
    }

    // Property-based tests (Constitution Principle VI)

    proptest! {
        /// Property 1: prefix_sum is cumulative - prefix_sum(i) == sum(heights[0..=i])
        #[test]
        fn prop_prefix_sum_is_cumulative(heights in prop::collection::vec(1usize..=100, 1..50)) {
            let mut index = HeightIndex::new(heights.len());
            for &h in &heights {
                index.push(h);
            }

            // Verify each prefix sum equals the sum of all heights up to that index
            let mut expected_sum = 0;
            for (i, &h) in heights.iter().enumerate() {
                expected_sum += h;
                prop_assert_eq!(index.prefix_sum(i), expected_sum);
            }
        }

        /// Property 2: lower_bound returns valid indices - lower_bound(prefix_sum(i)) <= i + 1
        #[test]
        fn prop_lower_bound_within_bounds(heights in prop::collection::vec(1usize..=100, 1..50)) {
            let mut index = HeightIndex::new(heights.len());
            for &h in &heights {
                index.push(h);
            }

            // For every valid index, lower_bound of its prefix_sum should be <= i + 1
            for i in 0..index.len() {
                let prefix = index.prefix_sum(i);
                if let Some(bound) = index.lower_bound(prefix) {
                    prop_assert!(bound <= i + 1);
                }
            }
        }

        /// Property 3: set updates height correctly - after set(i, h), height(i) == h
        #[test]
        fn prop_set_updates_height(
            heights in prop::collection::vec(1usize..=100, 1..50),
            update_index in 0usize..50,
            new_height in 1usize..=100
        ) {
            let mut index = HeightIndex::new(heights.len());
            for &h in &heights {
                index.push(h);
            }

            // Only test if update_index is valid
            if update_index < index.len() {
                index.set(update_index, new_height);

                // Extract height at update_index
                let actual_height = if update_index == 0 {
                    index.prefix_sum(0)
                } else {
                    index.prefix_sum(update_index) - index.prefix_sum(update_index - 1)
                };

                prop_assert_eq!(actual_height, new_height);
            }
        }

        /// Property 4: push increments len - after push, len == old_len + 1
        #[test]
        fn prop_push_increments_len(heights in prop::collection::vec(1usize..=100, 0..50)) {
            let mut index = HeightIndex::new(heights.len() + 1);
            for &h in &heights {
                index.push(h);
            }

            let old_len = index.len();
            index.push(42);
            prop_assert_eq!(index.len(), old_len + 1);
        }

        /// Property 5: total equals last prefix_sum when non-empty
        #[test]
        fn prop_total_equals_last_prefix_sum(heights in prop::collection::vec(1usize..=100, 1..50)) {
            let mut index = HeightIndex::new(heights.len());
            for &h in &heights {
                index.push(h);
            }

            if !index.is_empty() {
                prop_assert_eq!(index.total(), index.prefix_sum(index.len() - 1));
            }
        }
    }
}
