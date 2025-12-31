//! Top-level view-state for entire log

use super::session::SessionViewState;
use crate::model::{AgentId, ConversationEntry, SessionId};

/// Top-level view-state for an entire log file.
///
/// Contains ordered sessions, supports:
/// - Multi-session logs (FR-070)
/// - Session boundary detection (FR-078)
/// - Active session determination (FR-080)
///
/// # Display Mode Independence (FR-076, FR-077)
/// LogViewState stores sessions in order. Display mode (continuous,
/// one-at-a-time, collapsible) is determined by view layer.
#[derive(Debug, Clone)]
pub struct LogViewState {
    /// Ordered sessions.
    sessions: Vec<SessionViewState>,
    /// Current session ID (for streaming detection).
    current_session_id: Option<SessionId>,
}

impl LogViewState {
    /// Create empty log view-state.
    pub fn new() -> Self {
        Self {
            sessions: Vec::new(),
            current_session_id: None,
        }
    }

    /// Number of sessions.
    pub fn session_count(&self) -> usize {
        self.sessions.len()
    }

    /// Check if empty.
    pub fn is_empty(&self) -> bool {
        self.sessions.is_empty()
    }

    /// Get session by index.
    pub fn get_session(&self, index: usize) -> Option<&SessionViewState> {
        self.sessions.get(index)
    }

    /// Get mutable session by index.
    pub fn get_session_mut(&mut self, index: usize) -> Option<&mut SessionViewState> {
        self.sessions.get_mut(index)
    }

    /// Iterate over sessions.
    pub fn sessions(&self) -> impl Iterator<Item = &SessionViewState> {
        self.sessions.iter()
    }

    /// Find active session containing scroll position (FR-080).
    /// Uses session start_line to determine which session is visible.
    ///
    /// Returns the LAST session whose start_line is <= scroll_line.
    /// This matches the specification which uses rfind.
    pub fn active_session(&self, scroll_line: usize) -> Option<&SessionViewState> {
        self.sessions
            .iter()
            .rfind(|s| s.start_line() <= scroll_line)
    }

    /// Active session index.
    pub fn active_session_index(&self, scroll_line: usize) -> Option<usize> {
        self.sessions
            .iter()
            .rposition(|s| s.start_line() <= scroll_line)
    }

    /// Add entry, routing to correct session/conversation.
    /// Creates new session if session_id changes (FR-078).
    pub fn add_entry(&mut self, entry: ConversationEntry, agent_id: Option<AgentId>) {
        let session_id = entry.session_id().cloned();

        // Detect session boundary
        if session_id != self.current_session_id {
            if let Some(new_id) = session_id.clone() {
                // Calculate start line for new session.
                // In continuous scroll mode, sessions are concatenated, so start_line
                // must account for all content from all previous sessions.
                let start_line = self.sessions.iter().map(|s| s.total_height()).sum();
                let mut new_session = SessionViewState::new(new_id);
                new_session.set_start_line(start_line);
                self.sessions.push(new_session);
                self.current_session_id = session_id;
            }
        }

        // Add to current session
        if let Some(session) = self.sessions.last_mut() {
            match agent_id {
                None => session.add_main_entry(entry),
                Some(id) => session.add_subagent_entry(id, entry),
            }
        }
    }

    /// Get current session (last one).
    pub fn current_session(&self) -> Option<&SessionViewState> {
        self.sessions.last()
    }

    /// Get mutable current session.
    pub fn current_session_mut(&mut self) -> Option<&mut SessionViewState> {
        self.sessions.last_mut()
    }

    /// Create an empty session (used when model session has no entries).
    /// This ensures session_view() doesn't panic in tests/edge cases.
    pub fn create_empty_session(&mut self, session_id: SessionId) {
        let start_line = self.sessions.iter().map(|s| s.total_height()).sum();
        let mut new_session = SessionViewState::new(session_id.clone());
        new_session.set_start_line(start_line);
        self.sessions.push(new_session);
        self.current_session_id = Some(session_id);
    }
}

impl Default for LogViewState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{
        EntryMetadata, EntryType, EntryUuid, LogEntry, Message, MessageContent, Role,
    };

    // ===== Test Helpers =====

    fn make_session_id(s: &str) -> SessionId {
        SessionId::new(s).expect("valid session id")
    }

    fn make_entry_uuid(s: &str) -> EntryUuid {
        EntryUuid::new(s).expect("valid uuid")
    }

    fn make_timestamp() -> chrono::DateTime<chrono::Utc> {
        "2025-12-25T10:00:00Z".parse().expect("valid timestamp")
    }

    fn make_entry(session: &str, uuid: &str, role: Role) -> ConversationEntry {
        let entry_type = match role {
            Role::User => EntryType::User,
            Role::Assistant => EntryType::Assistant,
        };
        let message = Message::new(role, MessageContent::Text(uuid.to_string()));
        let log_entry = LogEntry::new(
            make_entry_uuid(uuid),
            None,
            make_session_id(session),
            None,
            make_timestamp(),
            entry_type,
            message,
            EntryMetadata::default(),
        );
        ConversationEntry::Valid(Box::new(log_entry))
    }

    // ===== Basic Operations =====

    #[test]
    fn new_creates_empty_log_view_state() {
        let log = LogViewState::new();
        assert_eq!(log.session_count(), 0);
        assert!(log.is_empty());
        assert!(log.current_session().is_none());
    }

    #[test]
    fn default_creates_empty_log_view_state() {
        let log = LogViewState::default();
        assert_eq!(log.session_count(), 0);
        assert!(log.is_empty());
    }

    // ===== Session Boundary Detection (FR-078) =====

    #[test]
    fn add_entry_creates_first_session() {
        let mut log = LogViewState::new();
        let entry = make_entry("session-1", "uuid-1", Role::User);

        log.add_entry(entry, None);

        assert_eq!(log.session_count(), 1);
        assert!(!log.is_empty());
        let session = log.get_session(0).expect("session should exist");
        assert_eq!(session.session_id(), &make_session_id("session-1"));
    }

    #[test]
    fn add_entry_same_session_does_not_create_new_session() {
        let mut log = LogViewState::new();
        let entry1 = make_entry("session-1", "uuid-1", Role::User);
        let entry2 = make_entry("session-1", "uuid-2", Role::Assistant);

        log.add_entry(entry1, None);
        log.add_entry(entry2, None);

        assert_eq!(log.session_count(), 1);
        let session = log.get_session(0).expect("session should exist");
        // Session should have 2 entries
        assert_eq!(session.main().len(), 2);
    }

    #[test]
    fn add_entry_different_session_creates_new_session() {
        let mut log = LogViewState::new();
        let entry1 = make_entry("session-1", "uuid-1", Role::User);
        let entry2 = make_entry("session-2", "uuid-2", Role::User);

        log.add_entry(entry1, None);
        log.add_entry(entry2, None);

        assert_eq!(log.session_count(), 2);
        let session1 = log.get_session(0).expect("session 1 should exist");
        let session2 = log.get_session(1).expect("session 2 should exist");
        assert_eq!(session1.session_id(), &make_session_id("session-1"));
        assert_eq!(session2.session_id(), &make_session_id("session-2"));
    }

    #[test]
    fn add_entry_routes_to_main_when_agent_id_none() {
        let mut log = LogViewState::new();
        let entry = make_entry("session-1", "uuid-1", Role::User);

        log.add_entry(entry, None);

        let session = log.get_session(0).expect("session should exist");
        assert_eq!(session.main().len(), 1);
    }

    #[test]
    fn add_entry_routes_to_subagent_when_agent_id_some() {
        let mut log = LogViewState::new();
        let entry = make_entry("session-1", "uuid-1", Role::User);
        let agent_id = AgentId::new("subagent-1").expect("valid agent id");

        log.add_entry(entry, Some(agent_id.clone()));

        let session = log.get_session(0).expect("session should exist");
        // Entry should be in pending subagent entries, not main
        assert_eq!(session.main().len(), 0);
        // Subagent ID should be known
        assert!(session.subagent_ids().any(|id| id == &agent_id));
    }

    // ===== Start Line Calculation =====

    #[test]
    fn first_session_has_start_line_zero() {
        let mut log = LogViewState::new();
        let entry = make_entry("session-1", "uuid-1", Role::User);

        log.add_entry(entry, None);

        let session = log.get_session(0).expect("session should exist");
        assert_eq!(session.start_line(), 0);
    }

    #[test]
    fn second_session_start_line_equals_first_session_total_height() {
        let mut log = LogViewState::new();

        // Add entries to first session
        log.add_entry(make_entry("session-1", "uuid-1", Role::User), None);
        log.add_entry(make_entry("session-1", "uuid-2", Role::Assistant), None);

        let first_height = log.get_session(0).unwrap().total_height();

        // Add entry to trigger second session
        log.add_entry(make_entry("session-2", "uuid-3", Role::User), None);

        let session2 = log.get_session(1).expect("session 2 should exist");
        assert_eq!(session2.start_line(), first_height);
    }

    #[test]
    fn third_session_start_line_equals_sum_of_previous_heights() {
        let mut log = LogViewState::new();

        // Session 1
        log.add_entry(make_entry("session-1", "uuid-1", Role::User), None);
        log.add_entry(make_entry("session-1", "uuid-2", Role::Assistant), None);

        // Session 2
        log.add_entry(make_entry("session-2", "uuid-3", Role::User), None);

        let height_1_plus_2 =
            log.get_session(0).unwrap().total_height() + log.get_session(1).unwrap().total_height();

        // Session 3
        log.add_entry(make_entry("session-3", "uuid-4", Role::User), None);

        let session3 = log.get_session(2).expect("session 3 should exist");
        assert_eq!(session3.start_line(), height_1_plus_2);
    }

    // ===== Active Session (FR-080) =====

    #[test]
    fn active_session_returns_none_when_empty() {
        let log = LogViewState::new();
        assert!(log.active_session(0).is_none());
        assert!(log.active_session_index(0).is_none());
    }

    #[test]
    fn active_session_returns_first_session_for_scroll_line_zero() {
        let mut log = LogViewState::new();
        log.add_entry(make_entry("session-1", "uuid-1", Role::User), None);

        let session = log.active_session(0).expect("should have active session");
        assert_eq!(session.session_id(), &make_session_id("session-1"));
        assert_eq!(log.active_session_index(0), Some(0));
    }

    #[test]
    fn active_session_returns_correct_session_within_bounds() {
        let mut log = LogViewState::new();

        // Session 1: 2 entries
        log.add_entry(make_entry("session-1", "uuid-1", Role::User), None);
        log.add_entry(make_entry("session-1", "uuid-2", Role::Assistant), None);

        let height_1 = log.get_session(0).unwrap().total_height();

        // Session 2: 1 entry
        log.add_entry(make_entry("session-2", "uuid-3", Role::User), None);

        // NOTE: Without layout computation, all sessions have height=0 and start_line=0.
        // The rfind algorithm returns the LAST session with start_line <= scroll_line.
        // So when all sessions are at start_line=0, scroll_line=0 returns the last session.
        //
        // This test verifies the spec behavior (rfind), which may seem counter-intuitive
        // without layout, but is correct for the normal case with computed layouts.

        // Scroll line 0: with all sessions at start_line=0, rfind returns the LAST one
        let session = log.active_session(0).expect("should have active session");
        assert_eq!(session.session_id(), &make_session_id("session-2"));
        assert_eq!(log.active_session_index(0), Some(1));

        // Scroll line at height_1 (which is 0): same as above
        let session = log
            .active_session(height_1)
            .expect("should have active session");
        assert_eq!(session.session_id(), &make_session_id("session-2"));
        assert_eq!(log.active_session_index(height_1), Some(1));

        // Scroll line beyond all sessions: returns last session
        let session = log
            .active_session(height_1 + 1)
            .expect("should have active session");
        assert_eq!(session.session_id(), &make_session_id("session-2"));
        assert_eq!(log.active_session_index(height_1 + 1), Some(1));
    }

    #[test]
    fn active_session_returns_last_session_for_scroll_beyond_end() {
        let mut log = LogViewState::new();
        log.add_entry(make_entry("session-1", "uuid-1", Role::User), None);
        log.add_entry(make_entry("session-2", "uuid-2", Role::User), None);

        let huge_scroll = 999999;
        let session = log
            .active_session(huge_scroll)
            .expect("should have active session");
        assert_eq!(session.session_id(), &make_session_id("session-2"));
        assert_eq!(log.active_session_index(huge_scroll), Some(1));
    }

    // ===== Session Access =====

    #[test]
    fn get_session_returns_none_for_out_of_bounds() {
        let mut log = LogViewState::new();
        log.add_entry(make_entry("session-1", "uuid-1", Role::User), None);

        assert!(log.get_session(1).is_none());
        assert!(log.get_session(999).is_none());
    }

    #[test]
    fn current_session_returns_last_session() {
        let mut log = LogViewState::new();
        log.add_entry(make_entry("session-1", "uuid-1", Role::User), None);
        log.add_entry(make_entry("session-2", "uuid-2", Role::User), None);

        let session = log.current_session().expect("should have current session");
        assert_eq!(session.session_id(), &make_session_id("session-2"));
    }

    #[test]
    fn sessions_iterator_yields_all_sessions() {
        let mut log = LogViewState::new();
        log.add_entry(make_entry("session-1", "uuid-1", Role::User), None);
        log.add_entry(make_entry("session-2", "uuid-2", Role::User), None);
        log.add_entry(make_entry("session-3", "uuid-3", Role::User), None);

        let session_ids: Vec<_> = log.sessions().map(|s| s.session_id().clone()).collect();

        assert_eq!(session_ids.len(), 3);
        assert_eq!(session_ids[0], make_session_id("session-1"));
        assert_eq!(session_ids[1], make_session_id("session-2"));
        assert_eq!(session_ids[2], make_session_id("session-3"));
    }
}
