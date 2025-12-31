//! Session and conversation types.
//!
//! Session is the aggregate root containing main agent and subagents.
//! All types use smart constructors - raw constructors never exported.

use crate::model::{AgentId, ConversationEntry, LogEntry, MalformedEntry, ModelInfo, SessionId};
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
    pub fn new(session_id: SessionId) -> Self {
        Self {
            session_id,
            main_agent: AgentConversation::new(None),
            subagents: HashMap::new(),
        }
    }

    /// Add an entry to the appropriate agent conversation.
    /// Routes to main_agent if entry.agent_id() is None,
    /// otherwise routes to the corresponding subagent (creating if needed).
    /// If a subagent doesn't exist, creates an incomplete placeholder conversation.
    pub fn add_entry(&mut self, entry: LogEntry) {
        if let Some(agent_id) = entry.agent_id() {
            // Route to subagent (create incomplete if doesn't exist)
            self.subagents
                .entry(agent_id.clone())
                .or_insert_with(|| AgentConversation::new_incomplete(agent_id.clone()))
                .add_entry(entry);
        } else {
            // Route to main agent
            self.main_agent.add_entry(entry);
        }
    }

    /// Add a conversation entry (valid or malformed) to the appropriate agent.
    ///
    /// Routes based on session_id() in the entry:
    /// - If session_id matches and has agent_id -> route to subagent
    /// - Otherwise -> route to main agent
    ///
    /// If a subagent doesn't exist, creates an incomplete placeholder conversation.
    pub fn add_conversation_entry(&mut self, conv_entry: ConversationEntry) {
        // Extract agent_id from the entry (if it's valid)
        let agent_id = match &conv_entry {
            ConversationEntry::Valid(log_entry) => log_entry.agent_id().cloned(),
            ConversationEntry::Malformed(_) => None,
        };

        if let Some(agent_id) = agent_id {
            // Route to subagent (create incomplete if doesn't exist)
            self.subagents
                .entry(agent_id.clone())
                .or_insert_with(|| AgentConversation::new_incomplete(agent_id.clone()))
                .add_conversation_entry(conv_entry);
        } else {
            // Route to main agent
            self.main_agent.add_conversation_entry(conv_entry);
        }
    }

    /// Get subagent IDs in order of first appearance (by timestamp).
    pub fn subagent_ids_ordered(&self) -> Vec<&AgentId> {
        let mut agents: Vec<_> = self.subagents.iter().collect();
        agents.sort_by_key(|(_, conv)| conv.first_timestamp());
        agents.into_iter().map(|(id, _)| id).collect()
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
    entries: Vec<ConversationEntry>,
    model: Option<ModelInfo>,
    is_incomplete: bool,
}

impl AgentConversation {
    /// Create a new empty conversation.
    /// agent_id is None for main agent, Some(id) for subagents.
    pub fn new(agent_id: Option<AgentId>) -> Self {
        Self {
            agent_id,
            entries: Vec::new(),
            model: None,
            is_incomplete: false,
        }
    }

    /// Create an incomplete conversation for a subagent with missing spawn event.
    /// Used when entries reference an unknown agent_id.
    pub fn new_incomplete(agent_id: AgentId) -> Self {
        Self {
            agent_id: Some(agent_id),
            entries: Vec::new(),
            model: None,
            is_incomplete: true,
        }
    }

    /// Check if this conversation is missing its spawn event.
    /// Returns true for placeholder conversations created on-the-fly.
    pub fn is_incomplete(&self) -> bool {
        self.is_incomplete
    }

    /// Add a valid log entry to this conversation.
    /// Updates model if entry.message().model() is Some.
    /// Wraps the LogEntry in ConversationEntry::Valid.
    pub fn add_entry(&mut self, entry: LogEntry) {
        // Update model if this is an assistant message with model metadata
        if matches!(entry.message().role(), super::Role::Assistant) {
            if let Some(model) = entry.message().model() {
                self.model = Some(model.clone());
            }
        }

        // Wrap in ConversationEntry and append
        self.entries.push(ConversationEntry::Valid(Box::new(entry)));
    }

    /// Add a malformed entry to this conversation.
    /// Malformed entries don't update the model.
    pub fn add_malformed(&mut self, malformed: MalformedEntry) {
        self.entries.push(ConversationEntry::Malformed(malformed));
    }

    /// Add a conversation entry directly (valid or malformed).
    /// Updates model if it's a Valid entry with a model.
    pub fn add_conversation_entry(&mut self, conv_entry: ConversationEntry) {
        // Update model if this is a valid assistant message with model metadata
        if let ConversationEntry::Valid(ref log_entry) = conv_entry {
            if matches!(log_entry.message().role(), super::Role::Assistant) {
                if let Some(model) = log_entry.message().model() {
                    self.model = Some(model.clone());
                }
            }
        }

        // Append the entry
        self.entries.push(conv_entry);
    }

    // ===== Accessors (read-only) =====

    pub fn agent_id(&self) -> Option<&AgentId> {
        self.agent_id.as_ref()
    }

    pub fn entries(&self) -> &[ConversationEntry] {
        &self.entries
    }

    pub fn model(&self) -> Option<&ModelInfo> {
        self.model.as_ref()
    }

    pub fn first_timestamp(&self) -> Option<DateTime<Utc>> {
        self.entries.first().and_then(|e| e.timestamp())
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
    use crate::model::{EntryMetadata, EntryType, EntryUuid, Message, MessageContent, Role};

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
        assert_eq!(
            conv.entries()[0].as_valid().unwrap().uuid().as_str(),
            "entry-1"
        );
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
        assert_eq!(
            conv.entries()[0].as_valid().unwrap().uuid().as_str(),
            "entry-1"
        );
        assert_eq!(
            conv.entries()[1].as_valid().unwrap().uuid().as_str(),
            "entry-2"
        );
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
        assert_eq!(ids[0].as_str(), "agent-2"); // t=5
        assert_eq!(ids[1].as_str(), "agent-1"); // t=10
    }

    // ===== AgentConversation::new_incomplete Tests =====

    #[test]
    fn agent_conversation_new_incomplete_creates_incomplete_conversation() {
        let agent_id = make_agent_id("agent-missing");
        let conv = AgentConversation::new_incomplete(agent_id.clone());

        assert_eq!(conv.agent_id(), Some(&agent_id));
        assert!(conv.is_incomplete(), "Should be marked as incomplete");
        assert!(conv.entries().is_empty());
        assert!(conv.model().is_none());
    }

    #[test]
    fn agent_conversation_new_incomplete_has_agent_id() {
        let agent_id = make_agent_id("agent-123");
        let conv = AgentConversation::new_incomplete(agent_id.clone());

        assert_eq!(
            conv.agent_id(),
            Some(&agent_id),
            "Incomplete conversations must have agent_id"
        );
    }

    // ===== AgentConversation::is_incomplete Tests =====

    #[test]
    fn agent_conversation_new_creates_complete_conversation() {
        let conv = AgentConversation::new(Some(make_agent_id("agent-1")));
        assert!(
            !conv.is_incomplete(),
            "new() should create complete conversation"
        );
    }

    #[test]
    fn agent_conversation_main_agent_is_always_complete() {
        let conv = AgentConversation::new(None);
        assert!(
            !conv.is_incomplete(),
            "Main agent is always complete (no spawn event needed)"
        );
    }

    // ===== Session::add_entry with incomplete conversations =====

    #[test]
    fn session_add_entry_creates_incomplete_subagent_when_not_exists() {
        let mut session = Session::new(make_session_id("session-1"));
        let entry = make_subagent_entry("unknown-agent", "entry-1", 0);

        session.add_entry(entry);

        let agent_id = make_agent_id("unknown-agent");
        let subagent = &session.subagents()[&agent_id];

        assert!(
            subagent.is_incomplete(),
            "Subagent created on-the-fly should be marked incomplete"
        );
        assert_eq!(
            subagent.len(),
            1,
            "Entry should be added despite incomplete"
        );
    }

    #[test]
    fn session_add_entry_preserves_incomplete_flag_on_existing_subagent() {
        let mut session = Session::new(make_session_id("session-1"));

        // First entry creates incomplete conversation
        session.add_entry(make_subagent_entry("agent-abc", "entry-1", 0));

        // Second entry to same agent
        session.add_entry(make_subagent_entry("agent-abc", "entry-2", 5));

        let agent_id = make_agent_id("agent-abc");
        let subagent = &session.subagents()[&agent_id];

        assert!(
            subagent.is_incomplete(),
            "Incomplete flag should persist across multiple entries"
        );
        assert_eq!(subagent.len(), 2);
    }

    #[test]
    fn session_add_conversation_entry_creates_incomplete_when_not_exists() {
        let mut session = Session::new(make_session_id("session-1"));
        let entry = make_subagent_entry("unknown-agent", "entry-1", 0);
        let conv_entry = ConversationEntry::Valid(Box::new(entry));

        session.add_conversation_entry(conv_entry);

        let agent_id = make_agent_id("unknown-agent");
        let subagent = &session.subagents()[&agent_id];

        assert!(
            subagent.is_incomplete(),
            "add_conversation_entry should also create incomplete placeholders"
        );
    }

    // ===== AgentConversation::add_malformed Tests =====

    #[test]
    fn agent_conversation_add_malformed_appends_malformed_entry() {
        let mut conv = AgentConversation::new(None);
        let malformed = MalformedEntry::new(
            42,
            "bad json",
            "Parse error: unexpected token",
            Some(make_session_id("session-1")),
        );

        conv.add_malformed(malformed);

        assert_eq!(conv.entries().len(), 1);
        assert!(conv.entries()[0].is_malformed());
        assert_eq!(conv.entries()[0].as_malformed().unwrap().line_number(), 42);
    }

    #[test]
    fn agent_conversation_add_malformed_does_not_update_model() {
        let mut conv = AgentConversation::new(None);
        let malformed = MalformedEntry::new(10, "bad", "error", None);

        conv.add_malformed(malformed);

        assert!(
            conv.model().is_none(),
            "Malformed entry should not set model"
        );
    }

    // ===== AgentConversation::add_conversation_entry Tests =====

    #[test]
    fn agent_conversation_add_conversation_entry_handles_valid() {
        let mut conv = AgentConversation::new(None);
        let entry = make_main_agent_entry();
        let conv_entry = ConversationEntry::Valid(Box::new(entry));

        conv.add_conversation_entry(conv_entry);

        assert_eq!(conv.entries().len(), 1);
        assert!(conv.entries()[0].is_valid());
    }

    #[test]
    fn agent_conversation_add_conversation_entry_handles_malformed() {
        let mut conv = AgentConversation::new(None);
        let malformed = MalformedEntry::new(10, "bad", "error", None);
        let conv_entry = ConversationEntry::Malformed(malformed);

        conv.add_conversation_entry(conv_entry);

        assert_eq!(conv.entries().len(), 1);
        assert!(conv.entries()[0].is_malformed());
    }

    #[test]
    fn agent_conversation_maintains_insertion_order_with_mixed_entries() {
        let mut conv = AgentConversation::new(None);

        // Add valid entry
        let entry1 = LogEntry::new(
            make_entry_uuid("entry-1"),
            None,
            make_session_id("s1"),
            None,
            make_timestamp(),
            EntryType::User,
            make_message(),
            EntryMetadata::default(),
        );
        conv.add_entry(entry1);

        // Add malformed entry
        let malformed = MalformedEntry::new(42, "bad", "error", None);
        conv.add_malformed(malformed);

        // Add another valid entry
        let entry2 = LogEntry::new(
            make_entry_uuid("entry-2"),
            None,
            make_session_id("s1"),
            None,
            make_timestamp_offset(5),
            EntryType::Assistant,
            make_message(),
            EntryMetadata::default(),
        );
        conv.add_entry(entry2);

        assert_eq!(conv.entries().len(), 3);
        assert!(conv.entries()[0].is_valid());
        assert!(conv.entries()[1].is_malformed());
        assert!(conv.entries()[2].is_valid());
    }
}
