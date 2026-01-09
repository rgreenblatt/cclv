//! Per-session scroll state storage (FR-010).
//!
//! Implements "preserve on return" semantics:
//! - Key absent = session never visited → first visit shows top (offset 0)
//! - Key present = session previously visited → return restores stored offset

use std::collections::HashMap;
use crate::model::SessionId;

/// Per-session scroll state storage (FR-010).
///
/// # Cardinality
/// - States: 0 to S entries (S = session count)
/// - Each entry: SessionId → usize offset
/// - Precision: 1.0 (all states valid)
///
/// # Invariant
/// Offsets are only stored for sessions that have been visited and scrolled.
/// A session with offset 0 that was visited will have an entry; an unvisited
/// session will have no entry (distinguishing "visited at top" from "never visited").
pub type SessionScrollStates = HashMap<SessionId, ScrollState>;

/// Scroll state for a single session.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ScrollState {
    /// Vertical scroll offset (line number at top of viewport).
    pub offset: usize,
}

impl ScrollState {
    /// Create a new scroll state with the given offset.
    pub fn new(offset: usize) -> Self {
        Self { offset }
    }
}

/// Extension trait for managing session scroll states.
pub trait SessionScrollExt {
    /// Get scroll offset for a session.
    /// Returns 0 for unvisited sessions (first-visit behavior).
    fn scroll_offset_for(&self, session_id: &SessionId) -> usize;

    /// Check if a session has been visited.
    fn is_session_visited(&self, session_id: &SessionId) -> bool;

    /// Save scroll state when leaving a session.
    fn save_scroll_state(&mut self, session_id: SessionId, offset: usize);
}

impl SessionScrollExt for SessionScrollStates {
    fn scroll_offset_for(&self, session_id: &SessionId) -> usize {
        self.get(session_id).map(|s| s.offset).unwrap_or(0)
    }

    fn is_session_visited(&self, session_id: &SessionId) -> bool {
        self.contains_key(session_id)
    }

    fn save_scroll_state(&mut self, session_id: SessionId, offset: usize) {
        self.insert(session_id, ScrollState::new(offset));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Helper to create a test SessionId
    fn test_session_id(s: &str) -> SessionId {
        SessionId::new(s).expect("Valid session ID")
    }

    #[test]
    fn first_visit_to_session_returns_offset_zero() {
        let states = SessionScrollStates::new();
        let session = test_session_id("session-1");

        assert_eq!(states.scroll_offset_for(&session), 0);
    }

    #[test]
    fn after_saving_offset_returns_stored_offset() {
        let mut states = SessionScrollStates::new();
        let session = test_session_id("session-1");

        states.save_scroll_state(session.clone(), 42);

        assert_eq!(states.scroll_offset_for(&session), 42);
    }

    #[test]
    fn is_session_visited_false_for_unvisited_session() {
        let states = SessionScrollStates::new();
        let session = test_session_id("session-1");

        assert!(!states.is_session_visited(&session));
    }

    #[test]
    fn is_session_visited_true_after_saving_state() {
        let mut states = SessionScrollStates::new();
        let session = test_session_id("session-1");

        states.save_scroll_state(session.clone(), 0);

        assert!(states.is_session_visited(&session));
    }

    #[test]
    fn session_visited_at_offset_zero_has_entry() {
        let mut states = SessionScrollStates::new();
        let session = test_session_id("session-1");

        states.save_scroll_state(session.clone(), 0);

        // Key distinction: visited at offset 0 has an entry
        assert!(states.is_session_visited(&session));
        assert_eq!(states.scroll_offset_for(&session), 0);
    }

    #[test]
    fn multiple_sessions_tracked_independently() {
        let mut states = SessionScrollStates::new();
        let session1 = test_session_id("session-1");
        let session2 = test_session_id("session-2");

        states.save_scroll_state(session1.clone(), 10);
        states.save_scroll_state(session2.clone(), 20);

        assert_eq!(states.scroll_offset_for(&session1), 10);
        assert_eq!(states.scroll_offset_for(&session2), 20);
    }

    #[test]
    fn updating_session_offset_replaces_previous_value() {
        let mut states = SessionScrollStates::new();
        let session = test_session_id("session-1");

        states.save_scroll_state(session.clone(), 10);
        states.save_scroll_state(session.clone(), 30);

        assert_eq!(states.scroll_offset_for(&session), 30);
    }
}
