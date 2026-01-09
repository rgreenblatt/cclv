//! Session summary metadata for display in session list modal.

use crate::model::SessionId;
use crate::view_state::types::SessionIndex;
use chrono::{DateTime, Utc};

/// Summary metadata for a session, used in the session list modal.
///
/// This is a read-only snapshot of session state for display purposes.
/// Computed from SessionViewState on demand.
///
/// # FR-009: Display session metadata including:
/// - Session number (index + 1)
/// - Start timestamp
/// - Message count
#[derive(Debug, Clone)]
pub struct SessionSummary {
    /// Validated index of this session.
    index: SessionIndex,

    /// Session identifier (UUID).
    session_id: SessionId,

    /// Total message count in main conversation.
    message_count: usize,

    /// Timestamp of first entry in session (if available).
    start_time: Option<DateTime<Utc>>,

    /// Number of subagents spawned in this session.
    subagent_count: usize,
}

impl SessionSummary {
    /// Create a new session summary.
    ///
    /// # Arguments
    /// - `index`: Validated session index
    /// - `session_id`: Session UUID
    /// - `message_count`: Number of messages in main conversation
    /// - `start_time`: Timestamp of first entry
    /// - `subagent_count`: Number of subagents
    pub fn new(
        index: SessionIndex,
        session_id: SessionId,
        message_count: usize,
        start_time: Option<DateTime<Utc>>,
        subagent_count: usize,
    ) -> Self {
        Self {
            index,
            session_id,
            message_count,
            start_time,
            subagent_count,
        }
    }

    /// Session index.
    pub fn index(&self) -> SessionIndex {
        self.index
    }

    /// Session ID.
    pub fn session_id(&self) -> &SessionId {
        &self.session_id
    }

    /// Message count in main conversation.
    pub fn message_count(&self) -> usize {
        self.message_count
    }

    /// Start time of session.
    pub fn start_time(&self) -> Option<DateTime<Utc>> {
        self.start_time
    }

    /// Number of subagents.
    pub fn subagent_count(&self) -> usize {
        self.subagent_count
    }

    /// Format for display in session list.
    ///
    /// Returns: "Session N: X messages, Y subagents (HH:MM)"
    pub fn display_line(&self) -> String {
        let time_str = self
            .start_time
            .map(|t| t.format(" (%H:%M)").to_string())
            .unwrap_or_default();

        format!(
            "Session {}: {} messages, {} subagents{}",
            self.index.display(),
            self.message_count,
            self.subagent_count,
            time_str
        )
    }

    /// Create a new session summary from SessionViewState.
    ///
    /// Extracts:
    /// - `session_id` from session.session_id()
    /// - `message_count` from session.main().len()
    /// - `start_time` from session.start_time()
    /// - `subagent_count` from session.subagents().len()
    ///
    /// # Arguments
    /// - `index`: Validated session index
    /// - `session`: Reference to SessionViewState to extract data from
    pub fn from_session(
        index: SessionIndex,
        session: &crate::view_state::session::SessionViewState,
    ) -> Self {
        Self {
            index,
            session_id: session.session_id().clone(),
            message_count: session.main().len(),
            start_time: session.start_time(),
            subagent_count: session.subagents().len(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_session_id() -> SessionId {
        SessionId::new("550e8400-e29b-41d4-a716-446655440000").unwrap()
    }

    fn make_test_index() -> SessionIndex {
        SessionIndex::new(0, 3).unwrap()
    }

    fn make_test_time() -> DateTime<Utc> {
        chrono::NaiveDate::from_ymd_opt(2024, 1, 15)
            .unwrap()
            .and_hms_opt(14, 30, 45)
            .unwrap()
            .and_utc()
    }

    #[test]
    fn new_creates_session_summary() {
        let index = make_test_index();
        let session_id = make_test_session_id();
        let start_time = Some(make_test_time());

        let summary = SessionSummary::new(index, session_id.clone(), 10, start_time, 3);

        assert_eq!(summary.index(), index);
        assert_eq!(summary.session_id(), &session_id);
        assert_eq!(summary.message_count(), 10);
        assert_eq!(summary.start_time(), start_time);
        assert_eq!(summary.subagent_count(), 3);
    }

    #[test]
    fn new_handles_none_start_time() {
        let index = make_test_index();
        let session_id = make_test_session_id();

        let summary = SessionSummary::new(index, session_id, 5, None, 1);

        assert_eq!(summary.start_time(), None);
    }

    #[test]
    fn index_returns_session_index() {
        let index = SessionIndex::new(2, 5).unwrap();
        let session_id = make_test_session_id();

        let summary = SessionSummary::new(index, session_id, 7, None, 0);

        assert_eq!(summary.index(), index);
    }

    #[test]
    fn session_id_returns_reference() {
        let index = make_test_index();
        let session_id = make_test_session_id();

        let summary = SessionSummary::new(index, session_id.clone(), 1, None, 0);

        assert_eq!(summary.session_id(), &session_id);
    }

    #[test]
    fn message_count_returns_count() {
        let index = make_test_index();
        let session_id = make_test_session_id();

        let summary = SessionSummary::new(index, session_id, 42, None, 0);

        assert_eq!(summary.message_count(), 42);
    }

    #[test]
    fn start_time_returns_time() {
        let index = make_test_index();
        let session_id = make_test_session_id();
        let start_time = Some(make_test_time());

        let summary = SessionSummary::new(index, session_id, 1, start_time, 0);

        assert_eq!(summary.start_time(), start_time);
    }

    #[test]
    fn subagent_count_returns_count() {
        let index = make_test_index();
        let session_id = make_test_session_id();

        let summary = SessionSummary::new(index, session_id, 1, None, 7);

        assert_eq!(summary.subagent_count(), 7);
    }

    #[test]
    fn display_line_with_time() {
        let index = SessionIndex::new(0, 3).unwrap(); // Display as "Session 1"
        let session_id = make_test_session_id();
        let start_time = Some(make_test_time()); // 14:30

        let summary = SessionSummary::new(index, session_id, 10, start_time, 3);

        let result = summary.display_line();
        assert_eq!(result, "Session 1: 10 messages, 3 subagents (14:30)");
    }

    #[test]
    fn display_line_without_time() {
        let index = SessionIndex::new(1, 3).unwrap(); // Display as "Session 2"
        let session_id = make_test_session_id();

        let summary = SessionSummary::new(index, session_id, 5, None, 2);

        let result = summary.display_line();
        assert_eq!(result, "Session 2: 5 messages, 2 subagents");
    }

    #[test]
    fn display_line_zero_subagents() {
        let index = SessionIndex::new(2, 3).unwrap(); // Display as "Session 3"
        let session_id = make_test_session_id();

        let summary = SessionSummary::new(index, session_id, 1, None, 0);

        let result = summary.display_line();
        assert_eq!(result, "Session 3: 1 messages, 0 subagents");
    }

    #[test]
    fn display_line_large_numbers() {
        let index = SessionIndex::new(99, 100).unwrap(); // Display as "Session 100"
        let session_id = make_test_session_id();
        let start_time = Some(make_test_time());

        let summary = SessionSummary::new(index, session_id, 999, start_time, 42);

        let result = summary.display_line();
        assert_eq!(result, "Session 100: 999 messages, 42 subagents (14:30)");
    }

    // ===== from_session Factory Tests (cclv-463.6.1) =====

    #[test]
    fn from_session_extracts_session_id() {
        use crate::view_state::session::SessionViewState;

        let session_id = make_test_session_id();
        let session = SessionViewState::new(session_id.clone());
        let index = make_test_index();

        let summary = SessionSummary::from_session(index, &session);

        assert_eq!(summary.session_id(), &session_id);
    }

    #[test]
    fn from_session_extracts_message_count() {
        use crate::model::{
            EntryMetadata, EntryType, EntryUuid, LogEntry, Message, MessageContent, Role,
        };
        use crate::view_state::session::SessionViewState;

        let session_id = make_test_session_id();
        let mut session = SessionViewState::new(session_id.clone());
        let index = make_test_index();

        // Add 3 messages to main conversation
        for i in 0..3 {
            let entry = crate::model::ConversationEntry::Valid(Box::new(LogEntry::new(
                EntryUuid::new(&format!("uuid-{}", i)).unwrap(),
                None,
                session_id.clone(),
                None,
                make_test_time(),
                EntryType::User,
                Message::new(Role::User, MessageContent::Text(format!("Message {}", i))),
                EntryMetadata::default(),
            )));
            session.add_main_entry(entry);
        }

        let summary = SessionSummary::from_session(index, &session);

        assert_eq!(summary.message_count(), 3);
    }

    #[test]
    fn from_session_extracts_start_time() {
        use crate::view_state::session::SessionViewState;

        let session_id = make_test_session_id();
        let mut session = SessionViewState::new(session_id.clone());
        let index = make_test_index();

        // Add entry with timestamp to trigger start_time capture
        use crate::model::{
            EntryMetadata, EntryType, EntryUuid, LogEntry, Message, MessageContent, Role,
        };
        let timestamp = make_test_time();
        let entry = crate::model::ConversationEntry::Valid(Box::new(LogEntry::new(
            EntryUuid::new("uuid-1").unwrap(),
            None,
            session_id.clone(),
            None,
            timestamp,
            EntryType::User,
            Message::new(
                Role::User,
                MessageContent::Text("First message".to_string()),
            ),
            EntryMetadata::default(),
        )));
        session.add_main_entry(entry);

        let summary = SessionSummary::from_session(index, &session);

        assert_eq!(summary.start_time(), Some(timestamp));
    }

    #[test]
    fn from_session_extracts_subagent_count() {
        use crate::model::AgentId;
        use crate::view_state::session::SessionViewState;

        let session_id = make_test_session_id();
        let mut session = SessionViewState::new(session_id.clone());
        let index = make_test_index();

        // Add 2 subagents
        use crate::model::{
            EntryMetadata, EntryType, EntryUuid, LogEntry, Message, MessageContent, Role,
        };
        let agent1 = AgentId::new("agent-1").unwrap();
        let agent2 = AgentId::new("agent-2").unwrap();

        let entry1 = crate::model::ConversationEntry::Valid(Box::new(LogEntry::new(
            EntryUuid::new("uuid-1").unwrap(),
            None,
            session_id.clone(),
            None,
            make_test_time(),
            EntryType::User,
            Message::new(
                Role::User,
                MessageContent::Text("Subagent 1 message".to_string()),
            ),
            EntryMetadata::default(),
        )));
        session.add_subagent_entry(agent1, entry1);

        let entry2 = crate::model::ConversationEntry::Valid(Box::new(LogEntry::new(
            EntryUuid::new("uuid-2").unwrap(),
            None,
            session_id.clone(),
            None,
            make_test_time(),
            EntryType::User,
            Message::new(
                Role::User,
                MessageContent::Text("Subagent 2 message".to_string()),
            ),
            EntryMetadata::default(),
        )));
        session.add_subagent_entry(agent2, entry2);

        let summary = SessionSummary::from_session(index, &session);

        assert_eq!(summary.subagent_count(), 2);
    }

    #[test]
    fn from_session_with_empty_session() {
        use crate::view_state::session::SessionViewState;

        let session_id = make_test_session_id();
        let session = SessionViewState::new(session_id.clone());
        let index = make_test_index();

        let summary = SessionSummary::from_session(index, &session);

        assert_eq!(summary.message_count(), 0);
        assert_eq!(summary.start_time(), None);
        assert_eq!(summary.subagent_count(), 0);
    }

    #[test]
    fn from_session_preserves_index() {
        use crate::view_state::session::SessionViewState;

        let session_id = make_test_session_id();
        let session = SessionViewState::new(session_id);
        let index = SessionIndex::new(5, 10).unwrap();

        let summary = SessionSummary::from_session(index, &session);

        assert_eq!(summary.index(), index);
    }
}
