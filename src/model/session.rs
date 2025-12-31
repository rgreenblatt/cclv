//! Session and conversation types.
//!
//! Session is the aggregate root containing main agent and subagents.
//! All types use smart constructors - raw constructors never exported.

use crate::model::{AgentId, LogEntry, ModelInfo, SessionId};
use chrono::{DateTime, Utc};
use std::collections::HashMap;

// ===== Session =====

/// A complete session with main agent and subagents.
/// Invariant: At least one agent (the main agent) always exists.
#[derive(Debug, Clone)]
pub struct Session {
    session_id: SessionId,
    main_agent: AgentConversation,
    subagents: HashMap<AgentId, AgentConversation>,
    // TODO: stats field will be added when SessionStats exists
}

impl Session {
    /// Create empty session with ID.
    pub fn new(_session_id: SessionId) -> Self {
        todo!("Session::new")
    }

    /// Add an entry to the appropriate agent conversation.
    /// Routes to main_agent if entry.agent_id() is None,
    /// otherwise routes to the corresponding subagent (creating if needed).
    pub fn add_entry(&mut self, _entry: LogEntry) {
        todo!("Session::add_entry")
    }

    /// Get subagent IDs in order of first appearance (by timestamp).
    pub fn subagent_ids_ordered(&self) -> Vec<&AgentId> {
        todo!("Session::subagent_ids_ordered")
    }

    // ===== Accessors (read-only) =====

    pub fn session_id(&self) -> &SessionId {
        &self.session_id
    }

    pub fn main_agent(&self) -> &AgentConversation {
        &self.main_agent
    }

    pub fn subagents(&self) -> &HashMap<AgentId, AgentConversation> {
        &self.subagents
    }
}

// ===== AgentConversation =====

/// A single agent's conversation (main or subagent).
#[derive(Debug, Clone)]
pub struct AgentConversation {
    agent_id: Option<AgentId>,
    entries: Vec<LogEntry>,
    model: Option<ModelInfo>,
}

impl AgentConversation {
    /// Create a new empty conversation.
    /// agent_id is None for main agent, Some(id) for subagents.
    pub fn new(_agent_id: Option<AgentId>) -> Self {
        todo!("AgentConversation::new")
    }

    /// Add an entry to this conversation.
    /// Updates model if entry.message().model() is Some.
    pub fn add_entry(&mut self, _entry: LogEntry) {
        todo!("AgentConversation::add_entry")
    }

    // ===== Accessors (read-only) =====

    pub fn agent_id(&self) -> Option<&AgentId> {
        self.agent_id.as_ref()
    }

    pub fn entries(&self) -> &[LogEntry] {
        &self.entries
    }

    pub fn model(&self) -> Option<&ModelInfo> {
        self.model.as_ref()
    }

    pub fn first_timestamp(&self) -> Option<DateTime<Utc>> {
        self.entries.first().map(|e| e.timestamp())
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

// ===== Tests =====

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{EntryType, EntryUuid, Message, MessageContent, Role};

    // ===== Test Helpers =====

    fn make_session_id(s: &str) -> SessionId {
        SessionId::new(s).expect("valid session id")
    }

    fn make_agent_id(s: &str) -> AgentId {
        AgentId::new(s).expect("valid agent id")
    }

    fn make_entry_uuid(s: &str) -> EntryUuid {
        EntryUuid::new(s).expect("valid uuid")
    }

    fn make_timestamp() -> DateTime<Utc> {
        "2025-12-25T10:00:00Z".parse().expect("valid timestamp")
    }

    fn make_timestamp_offset(minutes: i64) -> DateTime<Utc> {
        let base: DateTime<Utc> = "2025-12-25T10:00:00Z".parse().expect("valid timestamp");
        base + chrono::Duration::minutes(minutes)
    }

    fn make_message() -> Message {
        Message::new(Role::Assistant, MessageContent::Text("Test".to_string()))
    }

    fn make_main_agent_entry() -> LogEntry {
        use crate::model::EntryMetadata;
        LogEntry::new(
            make_entry_uuid("entry-1"),
            None,
            make_session_id("session-1"),
            None,
            make_timestamp(),
            EntryType::Assistant,
            make_message(),
            EntryMetadata::default(),
        )
    }

    fn make_subagent_entry(agent_id: &str, uuid: &str, offset_minutes: i64) -> LogEntry {
        use crate::model::EntryMetadata;
        LogEntry::new(
            make_entry_uuid(uuid),
            None,
            make_session_id("session-1"),
            Some(make_agent_id(agent_id)),
            make_timestamp_offset(offset_minutes),
            EntryType::User,
            make_message(),
            EntryMetadata::default(),
        )
    }

    // ===== AgentConversation::new Tests =====

    #[test]
    fn agent_conversation_new_creates_empty_main_agent() {
        let conv = AgentConversation::new(None);

        assert!(conv.agent_id().is_none());
        assert!(conv.entries().is_empty());
        assert!(conv.model().is_none());
    }

    #[test]
    fn agent_conversation_new_creates_empty_subagent() {
        let agent_id = make_agent_id("agent-123");
        let conv = AgentConversation::new(Some(agent_id.clone()));

        assert_eq!(conv.agent_id(), Some(&agent_id));
        assert!(conv.entries().is_empty());
        assert!(conv.model().is_none());
    }

    // ===== AgentConversation::add_entry Tests =====

    #[test]
    fn agent_conversation_add_entry_appends_to_entries() {
        let mut conv = AgentConversation::new(None);
        let entry = make_main_agent_entry();

        conv.add_entry(entry);

        assert_eq!(conv.entries().len(), 1);
        assert_eq!(conv.entries()[0].uuid().as_str(), "entry-1");
    }

    #[test]
    fn agent_conversation_add_entry_multiple_entries() {
        let mut conv = AgentConversation::new(None);

        use crate::model::EntryMetadata;
        let entry1 = LogEntry::new(
            make_entry_uuid("entry-1"),
            None,
            make_session_id("session-1"),
            None,
            make_timestamp(),
            EntryType::User,
            make_message(),
            EntryMetadata::default(),
        );
        let entry2 = LogEntry::new(
            make_entry_uuid("entry-2"),
            None,
            make_session_id("session-1"),
            None,
            make_timestamp_offset(5),
            EntryType::Assistant,
            make_message(),
            EntryMetadata::default(),
        );

        conv.add_entry(entry1);
        conv.add_entry(entry2);

        assert_eq!(conv.entries().len(), 2);
        assert_eq!(conv.entries()[0].uuid().as_str(), "entry-1");
        assert_eq!(conv.entries()[1].uuid().as_str(), "entry-2");
    }

    // ===== AgentConversation Accessor Tests =====

    #[test]
    fn agent_conversation_len_returns_entry_count() {
        let mut conv = AgentConversation::new(None);
        assert_eq!(conv.len(), 0);

        conv.add_entry(make_main_agent_entry());
        assert_eq!(conv.len(), 1);

        conv.add_entry(make_main_agent_entry());
        assert_eq!(conv.len(), 2);
    }

    #[test]
    fn agent_conversation_is_empty_returns_true_when_no_entries() {
        let conv = AgentConversation::new(None);
        assert!(conv.is_empty());
    }

    #[test]
    fn agent_conversation_is_empty_returns_false_with_entries() {
        let mut conv = AgentConversation::new(None);
        conv.add_entry(make_main_agent_entry());
        assert!(!conv.is_empty());
    }

    #[test]
    fn agent_conversation_first_timestamp_returns_none_when_empty() {
        let conv = AgentConversation::new(None);
        assert!(conv.first_timestamp().is_none());
    }

    #[test]
    fn agent_conversation_first_timestamp_returns_first_entry_time() {
        let mut conv = AgentConversation::new(None);

        use crate::model::EntryMetadata;
        let t1 = make_timestamp_offset(10);
        let t2 = make_timestamp_offset(20);

        let entry1 = LogEntry::new(
            make_entry_uuid("e1"),
            None,
            make_session_id("s1"),
            None,
            t1,
            EntryType::User,
            make_message(),
            EntryMetadata::default(),
        );
        let entry2 = LogEntry::new(
            make_entry_uuid("e2"),
            None,
            make_session_id("s1"),
            None,
            t2,
            EntryType::Assistant,
            make_message(),
            EntryMetadata::default(),
        );

        conv.add_entry(entry1);
        conv.add_entry(entry2);

        assert_eq!(conv.first_timestamp(), Some(t1));
    }

    // ===== Session::new Tests =====

    #[test]
    fn session_new_creates_session_with_id() {
        let session_id = make_session_id("session-123");
        let session = Session::new(session_id.clone());

        assert_eq!(session.session_id().as_str(), "session-123");
    }

    #[test]
    fn session_new_creates_empty_main_agent() {
        let session = Session::new(make_session_id("session-1"));

        assert!(session.main_agent().is_empty());
        assert!(session.main_agent().agent_id().is_none());
    }

    #[test]
    fn session_new_creates_empty_subagents_map() {
        let session = Session::new(make_session_id("session-1"));

        assert_eq!(session.subagents().len(), 0);
    }

    // ===== Session::add_entry Tests =====

    #[test]
    fn session_add_entry_routes_to_main_agent_when_agent_id_is_none() {
        let mut session = Session::new(make_session_id("session-1"));
        let entry = make_main_agent_entry();

        session.add_entry(entry);

        assert_eq!(session.main_agent().len(), 1);
        assert_eq!(session.subagents().len(), 0);
    }

    #[test]
    fn session_add_entry_routes_to_subagent_when_agent_id_is_some() {
        let mut session = Session::new(make_session_id("session-1"));
        let entry = make_subagent_entry("agent-abc", "entry-1", 0);

        session.add_entry(entry);

        assert_eq!(session.main_agent().len(), 0);
        assert_eq!(session.subagents().len(), 1);

        let agent_id = make_agent_id("agent-abc");
        assert!(session.subagents().contains_key(&agent_id));
        assert_eq!(session.subagents()[&agent_id].len(), 1);
    }

    #[test]
    fn session_add_entry_creates_subagent_if_not_exists() {
        let mut session = Session::new(make_session_id("session-1"));

        let entry1 = make_subagent_entry("agent-new", "entry-1", 0);
        session.add_entry(entry1);

        let agent_id = make_agent_id("agent-new");
        assert!(session.subagents().contains_key(&agent_id));
    }

    #[test]
    fn session_add_entry_appends_to_existing_subagent() {
        let mut session = Session::new(make_session_id("session-1"));

        let entry1 = make_subagent_entry("agent-abc", "entry-1", 0);
        let entry2 = make_subagent_entry("agent-abc", "entry-2", 5);

        session.add_entry(entry1);
        session.add_entry(entry2);

        let agent_id = make_agent_id("agent-abc");
        assert_eq!(session.subagents().len(), 1);
        assert_eq!(session.subagents()[&agent_id].len(), 2);
    }

    #[test]
    fn session_add_entry_handles_multiple_subagents() {
        let mut session = Session::new(make_session_id("session-1"));

        session.add_entry(make_subagent_entry("agent-1", "e1", 0));
        session.add_entry(make_subagent_entry("agent-2", "e2", 1));
        session.add_entry(make_subagent_entry("agent-3", "e3", 2));

        assert_eq!(session.subagents().len(), 3);
    }

    // ===== Session::subagent_ids_ordered Tests =====

    #[test]
    fn session_subagent_ids_ordered_returns_empty_when_no_subagents() {
        let session = Session::new(make_session_id("session-1"));
        let ids = session.subagent_ids_ordered();

        assert_eq!(ids.len(), 0);
    }

    #[test]
    fn session_subagent_ids_ordered_returns_single_id() {
        let mut session = Session::new(make_session_id("session-1"));
        session.add_entry(make_subagent_entry("agent-1", "e1", 0));

        let ids = session.subagent_ids_ordered();

        assert_eq!(ids.len(), 1);
        assert_eq!(ids[0].as_str(), "agent-1");
    }

    #[test]
    fn session_subagent_ids_ordered_sorts_by_first_timestamp() {
        let mut session = Session::new(make_session_id("session-1"));

        // Add in non-chronological order
        session.add_entry(make_subagent_entry("agent-second", "e2", 20));
        session.add_entry(make_subagent_entry("agent-first", "e1", 10));
        session.add_entry(make_subagent_entry("agent-third", "e3", 30));

        let ids = session.subagent_ids_ordered();

        assert_eq!(ids.len(), 3);
        assert_eq!(ids[0].as_str(), "agent-first");
        assert_eq!(ids[1].as_str(), "agent-second");
        assert_eq!(ids[2].as_str(), "agent-third");
    }

    #[test]
    fn session_subagent_ids_ordered_uses_first_entry_only() {
        let mut session = Session::new(make_session_id("session-1"));

        // agent-1's first entry at t=10, second at t=100
        session.add_entry(make_subagent_entry("agent-1", "e1", 10));
        session.add_entry(make_subagent_entry("agent-1", "e1b", 100));

        // agent-2's first entry at t=5
        session.add_entry(make_subagent_entry("agent-2", "e2", 5));

        let ids = session.subagent_ids_ordered();

        // Should be ordered by first entry only
        assert_eq!(ids.len(), 2);
        assert_eq!(ids[0].as_str(), "agent-2");  // t=5
        assert_eq!(ids[1].as_str(), "agent-1");  // t=10
    }
}
