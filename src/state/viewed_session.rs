//! Which session is currently being viewed.

use crate::view_state::types::SessionIndex;

/// Which session is currently being viewed.
///
/// # States
/// - `Latest`: Follow the most recent session (enables live tailing)
/// - `Pinned(index)`: View a specific historical session (disables live tailing)
///
/// # Cardinality
/// - Latest: 1 state
/// - Pinned: N states (where N = session count)
/// - Total: N + 1 states (all valid)
/// - Precision: 1.0
///
/// # Invariant
/// `Pinned(idx)` always holds a valid SessionIndex (validated at construction).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ViewedSession {
    /// Follow the latest (last) session. Enables live tailing.
    #[default]
    Latest,

    /// Pinned to a specific session. Disables live tailing.
    Pinned(SessionIndex),
}

impl ViewedSession {
    /// Create a pinned view to specific session.
    ///
    /// Returns `None` if index is invalid for current session count.
    pub fn pinned(index: usize, session_count: usize) -> Option<Self> {
        SessionIndex::new(index, session_count).map(Self::Pinned)
    }

    /// Check if viewing the last session.
    ///
    /// Used to determine if live tailing should be enabled.
    pub fn is_last(&self, session_count: usize) -> bool {
        match self {
            ViewedSession::Latest => true,
            ViewedSession::Pinned(idx) => idx.is_last(session_count),
        }
    }

    /// Get the effective session index.
    ///
    /// For `Latest`, returns the last session index.
    /// For `Pinned`, returns the pinned index.
    pub fn effective_index(&self, session_count: usize) -> Option<SessionIndex> {
        match self {
            ViewedSession::Latest => {
                if session_count > 0 {
                    SessionIndex::new(session_count - 1, session_count)
                } else {
                    None
                }
            }
            ViewedSession::Pinned(idx) => Some(*idx),
        }
    }

    /// Move to next session (toward latest).
    ///
    /// If at last session, switches to `Latest` mode.
    pub fn next(&self, session_count: usize) -> Self {
        match self {
            ViewedSession::Latest => ViewedSession::Latest,
            ViewedSession::Pinned(idx) => {
                if idx.is_last(session_count) {
                    ViewedSession::Latest
                } else {
                    idx.next(session_count)
                        .map(ViewedSession::Pinned)
                        .unwrap_or(ViewedSession::Latest)
                }
            }
        }
    }

    /// Move to previous session (toward first).
    ///
    /// If at first session, stays at first.
    pub fn prev(&self, session_count: usize) -> Self {
        match self {
            ViewedSession::Latest => {
                if session_count > 1 {
                    SessionIndex::new(session_count - 2, session_count)
                        .map(ViewedSession::Pinned)
                        .unwrap_or(ViewedSession::Latest)
                } else {
                    ViewedSession::Latest
                }
            }
            ViewedSession::Pinned(idx) => idx
                .prev()
                .map(ViewedSession::Pinned)
                .unwrap_or(ViewedSession::Pinned(*idx)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod properties {
        use super::*;
        use proptest::prelude::*;

        proptest! {
            /// Invariant 2: ViewedSession::effective_index always returns valid index
            /// For any ViewedSession (Latest or Pinned), when session_count > 0:
            /// - effective_index returns Some(idx)
            /// - idx.get() < session_count
            #[test]
            fn effective_index_always_valid_when_sessions_exist(
                session_count in 1usize..100,
                offset in 0usize..100,
            ) {
                let index = offset % session_count;

                // Test Latest variant
                if let Some(idx) = ViewedSession::Latest.effective_index(session_count) {
                    prop_assert!(idx.get() < session_count);
                    prop_assert_eq!(idx.get(), session_count - 1);
                }

                // Test Pinned variant
                if let Some(session) = ViewedSession::pinned(index, session_count) {
                    if let Some(idx) = session.effective_index(session_count) {
                        prop_assert!(idx.get() < session_count);
                        prop_assert_eq!(idx.get(), index);
                    }
                }
            }

            /// Invariant 2 (edge case): effective_index returns None for zero sessions
            #[test]
            fn effective_index_none_for_zero_sessions(_dummy in 0usize..10) {
                let result = ViewedSession::Latest.effective_index(0);
                prop_assert!(result.is_none());
            }

            /// Invariant 3: ViewedSession::Latest.is_last() always returns true
            /// For any session_count > 0, Latest.is_last(n) == true
            #[test]
            fn latest_is_always_last(session_count in 1usize..100) {
                prop_assert!(ViewedSession::Latest.is_last(session_count));
            }

            /// Invariant 3 (continued): Pinned to last index is_last() returns true
            /// For Pinned(last_idx), is_last(n) == true where last_idx is n-1
            #[test]
            fn pinned_to_last_is_last(session_count in 1usize..100) {
                let last_index = session_count - 1;
                if let Some(session) = ViewedSession::pinned(last_index, session_count) {
                    prop_assert!(session.is_last(session_count));
                }
            }

            /// Invariant 3 (continued): Pinned to non-last index is_last() returns false
            #[test]
            fn pinned_to_non_last_not_last(session_count in 2usize..100) {
                let index = session_count - 2;
                if let Some(session) = ViewedSession::pinned(index, session_count) {
                    prop_assert!(!session.is_last(session_count));
                }
            }

            /// Invariant: next() preserves validity
            /// After any number of next() calls, effective_index is valid
            #[test]
            fn next_preserves_validity(
                session_count in 1usize..20,
                num_nexts in 0usize..50,
            ) {
                let mut current = ViewedSession::pinned(0, session_count)
                    .unwrap_or(ViewedSession::Latest);

                for _ in 0..num_nexts {
                    current = current.next(session_count);
                }

                if let Some(idx) = current.effective_index(session_count) {
                    prop_assert!(idx.get() < session_count);
                }
            }

            /// Invariant: prev() preserves validity
            /// After any number of prev() calls, effective_index is valid
            #[test]
            fn prev_preserves_validity(
                session_count in 1usize..20,
                num_prevs in 0usize..50,
            ) {
                let mut current = ViewedSession::Latest;

                for _ in 0..num_prevs {
                    current = current.prev(session_count);
                }

                if let Some(idx) = current.effective_index(session_count) {
                    prop_assert!(idx.get() < session_count);
                }
            }

            /// Invariant: next from Latest stays Latest
            #[test]
            fn next_from_latest_stays_latest(session_count in 1usize..100) {
                let result = ViewedSession::Latest.next(session_count);
                prop_assert_eq!(result, ViewedSession::Latest);
            }

            /// Invariant: next to last switches to Latest
            #[test]
            fn next_from_last_switches_to_latest(session_count in 1usize..100) {
                let last_index = session_count - 1;
                if let Some(session) = ViewedSession::pinned(last_index, session_count) {
                    let result = session.next(session_count);
                    prop_assert_eq!(result, ViewedSession::Latest);
                }
            }

            /// Invariant: prev from first stays at first
            #[test]
            fn prev_from_first_stays_at_first(session_count in 1usize..100) {
                if let Some(session) = ViewedSession::pinned(0, session_count) {
                    let result = session.prev(session_count);
                    if let ViewedSession::Pinned(idx) = result {
                        prop_assert_eq!(idx.get(), 0);
                    } else {
                        // Should never be Latest after prev() from Pinned(0)
                        prop_assert!(false, "Expected Pinned(0), got Latest");
                    }
                }
            }
        }
    }

    mod default {
        use super::*;

        #[test]
        fn default_is_latest() {
            assert_eq!(ViewedSession::default(), ViewedSession::Latest);
        }
    }

    mod pinned {
        use super::*;

        #[test]
        fn pinned_accepts_valid_index() {
            let result = ViewedSession::pinned(0, 3);
            assert!(result.is_some());
        }

        #[test]
        fn pinned_accepts_middle_index() {
            let result = ViewedSession::pinned(1, 3);
            assert!(result.is_some());
        }

        #[test]
        fn pinned_accepts_last_index() {
            let result = ViewedSession::pinned(2, 3);
            assert!(result.is_some());
        }

        #[test]
        fn pinned_rejects_out_of_bounds() {
            let result = ViewedSession::pinned(3, 3);
            assert!(result.is_none());
        }

        #[test]
        fn pinned_rejects_far_out_of_bounds() {
            let result = ViewedSession::pinned(100, 3);
            assert!(result.is_none());
        }

        #[test]
        fn pinned_creates_pinned_variant() {
            let result = ViewedSession::pinned(1, 3).unwrap();
            match result {
                ViewedSession::Pinned(idx) => assert_eq!(idx.get(), 1),
                ViewedSession::Latest => panic!("Expected Pinned variant"),
            }
        }
    }

    mod is_last {
        use super::*;

        #[test]
        fn latest_is_always_last() {
            assert!(ViewedSession::Latest.is_last(1));
            assert!(ViewedSession::Latest.is_last(3));
            assert!(ViewedSession::Latest.is_last(100));
        }

        #[test]
        fn pinned_to_last_session_is_last() {
            let session = ViewedSession::pinned(2, 3).unwrap();
            assert!(session.is_last(3));
        }

        #[test]
        fn pinned_to_first_session_not_last() {
            let session = ViewedSession::pinned(0, 3).unwrap();
            assert!(!session.is_last(3));
        }

        #[test]
        fn pinned_to_middle_session_not_last() {
            let session = ViewedSession::pinned(1, 3).unwrap();
            assert!(!session.is_last(3));
        }
    }

    mod effective_index {
        use super::*;

        #[test]
        fn latest_returns_last_session_index() {
            let result = ViewedSession::Latest.effective_index(3);
            assert!(result.is_some());
            assert_eq!(result.unwrap().get(), 2);
        }

        #[test]
        fn latest_with_single_session() {
            let result = ViewedSession::Latest.effective_index(1);
            assert!(result.is_some());
            assert_eq!(result.unwrap().get(), 0);
        }

        #[test]
        fn latest_with_no_sessions_returns_none() {
            let result = ViewedSession::Latest.effective_index(0);
            assert!(result.is_none());
        }

        #[test]
        fn pinned_returns_pinned_index() {
            let session = ViewedSession::pinned(1, 3).unwrap();
            let result = session.effective_index(3);
            assert!(result.is_some());
            assert_eq!(result.unwrap().get(), 1);
        }

        #[test]
        fn pinned_to_first_returns_first_index() {
            let session = ViewedSession::pinned(0, 3).unwrap();
            let result = session.effective_index(3);
            assert!(result.is_some());
            assert_eq!(result.unwrap().get(), 0);
        }

        #[test]
        fn pinned_to_last_returns_last_index() {
            let session = ViewedSession::pinned(2, 3).unwrap();
            let result = session.effective_index(3);
            assert!(result.is_some());
            assert_eq!(result.unwrap().get(), 2);
        }
    }

    mod next {
        use super::*;

        #[test]
        fn latest_next_stays_latest() {
            let result = ViewedSession::Latest.next(3);
            assert_eq!(result, ViewedSession::Latest);
        }

        #[test]
        fn pinned_first_moves_to_second() {
            let session = ViewedSession::pinned(0, 3).unwrap();
            let result = session.next(3);
            match result {
                ViewedSession::Pinned(idx) => assert_eq!(idx.get(), 1),
                ViewedSession::Latest => panic!("Expected Pinned variant"),
            }
        }

        #[test]
        fn pinned_middle_moves_forward() {
            let session = ViewedSession::pinned(1, 3).unwrap();
            let result = session.next(3);
            match result {
                ViewedSession::Pinned(idx) => assert_eq!(idx.get(), 2),
                ViewedSession::Latest => panic!("Expected Pinned variant"),
            }
        }

        #[test]
        fn pinned_last_switches_to_latest() {
            let session = ViewedSession::pinned(2, 3).unwrap();
            let result = session.next(3);
            assert_eq!(result, ViewedSession::Latest);
        }
    }

    mod prev {
        use super::*;

        #[test]
        fn latest_with_multiple_sessions_pins_to_second_last() {
            let result = ViewedSession::Latest.prev(3);
            match result {
                ViewedSession::Pinned(idx) => assert_eq!(idx.get(), 1),
                ViewedSession::Latest => panic!("Expected Pinned variant"),
            }
        }

        #[test]
        fn latest_with_single_session_stays_latest() {
            let result = ViewedSession::Latest.prev(1);
            assert_eq!(result, ViewedSession::Latest);
        }

        #[test]
        fn latest_with_no_sessions_stays_latest() {
            let result = ViewedSession::Latest.prev(0);
            assert_eq!(result, ViewedSession::Latest);
        }

        #[test]
        fn pinned_second_moves_to_first() {
            let session = ViewedSession::pinned(1, 3).unwrap();
            let result = session.prev(3);
            match result {
                ViewedSession::Pinned(idx) => assert_eq!(idx.get(), 0),
                ViewedSession::Latest => panic!("Expected Pinned variant"),
            }
        }

        #[test]
        fn pinned_last_moves_to_second_last() {
            let session = ViewedSession::pinned(2, 3).unwrap();
            let result = session.prev(3);
            match result {
                ViewedSession::Pinned(idx) => assert_eq!(idx.get(), 1),
                ViewedSession::Latest => panic!("Expected Pinned variant"),
            }
        }

        #[test]
        fn pinned_first_stays_at_first() {
            let session = ViewedSession::pinned(0, 3).unwrap();
            let result = session.prev(3);
            match result {
                ViewedSession::Pinned(idx) => assert_eq!(idx.get(), 0),
                ViewedSession::Latest => panic!("Expected Pinned variant"),
            }
        }
    }
}
