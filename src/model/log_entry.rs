//! Log entry types representing parsed JSONL entries.
//!
//! LogEntry is the core parsed log entry from the JSONL file.
//! All fields are validated at construction time.

use crate::model::{AgentId, EntryUuid, Message, SessionId};
use chrono::{DateTime, Utc};
use std::path::PathBuf;

// ===== EntryType =====

/// Type of log entry - exactly one variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryType {
    User,
    Assistant,
    Summary,
}

// ===== EntryMetadata =====

/// Additional metadata from the log entry.
#[derive(Debug, Clone, Default)]
pub struct EntryMetadata {
    pub cwd: Option<PathBuf>,
    pub git_branch: Option<String>,
    pub version: Option<String>,
    pub is_sidechain: bool,
}

// ===== LogEntry =====

/// A parsed log entry from the JSONL file.
/// Invariant: All fields validated at construction time.
#[derive(Debug, Clone)]
pub struct LogEntry {
    uuid: EntryUuid,
    parent_uuid: Option<EntryUuid>,
    session_id: SessionId,
    agent_id: Option<AgentId>,
    timestamp: DateTime<Utc>,
    entry_type: EntryType,
    message: Message,
    metadata: EntryMetadata,
}

impl LogEntry {
    /// Create a new log entry.
    ///
    /// This constructor mirrors all fields and is intended for use by the parser.
    /// For creating entries from JSONL, use `LogEntry::parse()` instead.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        uuid: EntryUuid,
        parent_uuid: Option<EntryUuid>,
        session_id: SessionId,
        agent_id: Option<AgentId>,
        timestamp: DateTime<Utc>,
        entry_type: EntryType,
        message: Message,
        metadata: EntryMetadata,
    ) -> Self {
        Self {
            uuid,
            parent_uuid,
            session_id,
            agent_id,
            timestamp,
            entry_type,
            message,
            metadata,
        }
    }

    // ===== Accessors (read-only) =====

    pub fn uuid(&self) -> &EntryUuid {
        &self.uuid
    }

    pub fn parent_uuid(&self) -> Option<&EntryUuid> {
        self.parent_uuid.as_ref()
    }

    pub fn session_id(&self) -> &SessionId {
        &self.session_id
    }

    pub fn agent_id(&self) -> Option<&AgentId> {
        self.agent_id.as_ref()
    }

    pub fn timestamp(&self) -> DateTime<Utc> {
        self.timestamp
    }

    pub fn entry_type(&self) -> EntryType {
        self.entry_type
    }

    pub fn message(&self) -> &Message {
        &self.message
    }

    pub fn metadata(&self) -> &EntryMetadata {
        &self.metadata
    }

    /// Returns true if this entry is from a subagent.
    pub fn is_subagent(&self) -> bool {
        self.agent_id.is_some()
    }
}

// ===== Tests =====

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{MessageContent, Role};

    // ===== Test Helpers =====

    fn make_uuid(s: &str) -> EntryUuid {
        EntryUuid::new(s).expect("valid uuid")
    }

    fn make_session_id(s: &str) -> SessionId {
        SessionId::new(s).expect("valid session id")
    }

    fn make_agent_id(s: &str) -> AgentId {
        AgentId::new(s).expect("valid agent id")
    }

    fn make_timestamp() -> DateTime<Utc> {
        "2025-12-25T10:30:00Z".parse().expect("valid timestamp")
    }

    fn make_message() -> Message {
        Message::new(Role::Assistant, MessageContent::Text("Test".to_string()))
    }

    // ===== EntryType Tests =====

    #[test]
    fn entry_type_variants_are_distinct() {
        assert_ne!(EntryType::User, EntryType::Assistant);
        assert_ne!(EntryType::User, EntryType::Summary);
        assert_ne!(EntryType::Assistant, EntryType::Summary);
    }

    #[test]
    fn entry_type_user_equals_user() {
        assert_eq!(EntryType::User, EntryType::User);
    }

    #[test]
    fn entry_type_assistant_equals_assistant() {
        assert_eq!(EntryType::Assistant, EntryType::Assistant);
    }

    #[test]
    fn entry_type_summary_equals_summary() {
        assert_eq!(EntryType::Summary, EntryType::Summary);
    }

    #[test]
    fn entry_type_can_be_cloned() {
        let e = EntryType::User;
        let cloned = e;
        assert_eq!(e, cloned);
    }

    // ===== EntryMetadata Tests =====

    #[test]
    fn entry_metadata_default_has_none_fields() {
        let meta = EntryMetadata::default();
        assert!(meta.cwd.is_none());
        assert!(meta.git_branch.is_none());
        assert!(meta.version.is_none());
    }

    #[test]
    fn entry_metadata_default_is_not_sidechain() {
        let meta = EntryMetadata::default();
        assert!(!meta.is_sidechain);
    }

    #[test]
    fn entry_metadata_can_set_cwd() {
        let meta = EntryMetadata {
            cwd: Some(PathBuf::from("/home/user")),
            ..Default::default()
        };
        assert_eq!(meta.cwd, Some(PathBuf::from("/home/user")));
    }

    #[test]
    fn entry_metadata_can_set_git_branch() {
        let meta = EntryMetadata {
            git_branch: Some("main".to_string()),
            ..Default::default()
        };
        assert_eq!(meta.git_branch, Some("main".to_string()));
    }

    #[test]
    fn entry_metadata_can_set_version() {
        let meta = EntryMetadata {
            version: Some("1.0.0".to_string()),
            ..Default::default()
        };
        assert_eq!(meta.version, Some("1.0.0".to_string()));
    }

    #[test]
    fn entry_metadata_can_set_is_sidechain() {
        let meta = EntryMetadata {
            is_sidechain: true,
            ..Default::default()
        };
        assert!(meta.is_sidechain);
    }

    // ===== LogEntry Constructor Tests =====

    #[test]
    fn log_entry_new_creates_main_agent_entry() {
        let uuid = make_uuid("entry-1");
        let session_id = make_session_id("session-1");
        let timestamp = make_timestamp();
        let message = make_message();
        let metadata = EntryMetadata::default();

        let entry = LogEntry::new(
            uuid.clone(),
            None,
            session_id.clone(),
            None,
            timestamp,
            EntryType::Assistant,
            message,
            metadata,
        );

        assert_eq!(entry.uuid(), &uuid);
        assert_eq!(entry.session_id(), &session_id);
        assert!(entry.agent_id().is_none());
        assert_eq!(entry.timestamp(), timestamp);
        assert_eq!(entry.entry_type(), EntryType::Assistant);
    }

    #[test]
    fn log_entry_new_creates_subagent_entry() {
        let uuid = make_uuid("entry-2");
        let session_id = make_session_id("session-1");
        let agent_id = make_agent_id("agent-abc");
        let timestamp = make_timestamp();
        let message = make_message();
        let metadata = EntryMetadata::default();

        let entry = LogEntry::new(
            uuid.clone(),
            None,
            session_id.clone(),
            Some(agent_id.clone()),
            timestamp,
            EntryType::User,
            message,
            metadata,
        );

        assert_eq!(entry.uuid(), &uuid);
        assert_eq!(entry.agent_id(), Some(&agent_id));
        assert_eq!(entry.entry_type(), EntryType::User);
    }

    #[test]
    fn log_entry_new_creates_entry_with_parent() {
        let uuid = make_uuid("entry-3");
        let parent_uuid = make_uuid("entry-2");
        let session_id = make_session_id("session-1");
        let timestamp = make_timestamp();
        let message = make_message();
        let metadata = EntryMetadata::default();

        let entry = LogEntry::new(
            uuid,
            Some(parent_uuid.clone()),
            session_id,
            None,
            timestamp,
            EntryType::Summary,
            message,
            metadata,
        );

        assert_eq!(entry.parent_uuid(), Some(&parent_uuid));
        assert_eq!(entry.entry_type(), EntryType::Summary);
    }

    // ===== LogEntry Accessor Tests =====

    #[test]
    fn log_entry_uuid_returns_correct_value() {
        let uuid = make_uuid("entry-uuid-123");
        let entry = LogEntry::new(
            uuid.clone(),
            None,
            make_session_id("s1"),
            None,
            make_timestamp(),
            EntryType::User,
            make_message(),
            EntryMetadata::default(),
        );

        assert_eq!(entry.uuid().as_str(), "entry-uuid-123");
    }

    #[test]
    fn log_entry_parent_uuid_returns_none_when_not_set() {
        let entry = LogEntry::new(
            make_uuid("e1"),
            None,
            make_session_id("s1"),
            None,
            make_timestamp(),
            EntryType::User,
            make_message(),
            EntryMetadata::default(),
        );

        assert!(entry.parent_uuid().is_none());
    }

    #[test]
    fn log_entry_parent_uuid_returns_some_when_set() {
        let parent = make_uuid("parent-123");
        let entry = LogEntry::new(
            make_uuid("e1"),
            Some(parent.clone()),
            make_session_id("s1"),
            None,
            make_timestamp(),
            EntryType::User,
            make_message(),
            EntryMetadata::default(),
        );

        assert_eq!(entry.parent_uuid(), Some(&parent));
    }

    #[test]
    fn log_entry_session_id_returns_correct_value() {
        let session_id = make_session_id("session-xyz");
        let entry = LogEntry::new(
            make_uuid("e1"),
            None,
            session_id.clone(),
            None,
            make_timestamp(),
            EntryType::Assistant,
            make_message(),
            EntryMetadata::default(),
        );

        assert_eq!(entry.session_id().as_str(), "session-xyz");
    }

    #[test]
    fn log_entry_agent_id_returns_none_for_main_agent() {
        let entry = LogEntry::new(
            make_uuid("e1"),
            None,
            make_session_id("s1"),
            None,
            make_timestamp(),
            EntryType::Assistant,
            make_message(),
            EntryMetadata::default(),
        );

        assert!(entry.agent_id().is_none());
    }

    #[test]
    fn log_entry_agent_id_returns_some_for_subagent() {
        let agent_id = make_agent_id("subagent-456");
        let entry = LogEntry::new(
            make_uuid("e1"),
            None,
            make_session_id("s1"),
            Some(agent_id.clone()),
            make_timestamp(),
            EntryType::User,
            make_message(),
            EntryMetadata::default(),
        );

        assert_eq!(entry.agent_id(), Some(&agent_id));
    }

    #[test]
    fn log_entry_timestamp_returns_correct_value() {
        let timestamp = make_timestamp();
        let entry = LogEntry::new(
            make_uuid("e1"),
            None,
            make_session_id("s1"),
            None,
            timestamp,
            EntryType::Summary,
            make_message(),
            EntryMetadata::default(),
        );

        assert_eq!(entry.timestamp(), timestamp);
    }

    #[test]
    fn log_entry_entry_type_returns_correct_value() {
        let entry = LogEntry::new(
            make_uuid("e1"),
            None,
            make_session_id("s1"),
            None,
            make_timestamp(),
            EntryType::Summary,
            make_message(),
            EntryMetadata::default(),
        );

        assert_eq!(entry.entry_type(), EntryType::Summary);
    }

    #[test]
    fn log_entry_message_returns_reference() {
        let message = make_message();
        let entry = LogEntry::new(
            make_uuid("e1"),
            None,
            make_session_id("s1"),
            None,
            make_timestamp(),
            EntryType::User,
            message.clone(),
            EntryMetadata::default(),
        );

        assert_eq!(entry.message().role(), Role::Assistant);
    }

    #[test]
    fn log_entry_metadata_returns_reference() {
        let metadata = EntryMetadata {
            cwd: Some(PathBuf::from("/test")),
            git_branch: Some("dev".to_string()),
            version: Some("2.0.0".to_string()),
            is_sidechain: true,
        };

        let entry = LogEntry::new(
            make_uuid("e1"),
            None,
            make_session_id("s1"),
            None,
            make_timestamp(),
            EntryType::Assistant,
            make_message(),
            metadata.clone(),
        );

        let meta = entry.metadata();
        assert_eq!(meta.cwd, Some(PathBuf::from("/test")));
        assert_eq!(meta.git_branch, Some("dev".to_string()));
        assert_eq!(meta.version, Some("2.0.0".to_string()));
        assert!(meta.is_sidechain);
    }

    // ===== LogEntry::is_subagent Tests =====

    #[test]
    fn log_entry_is_subagent_returns_false_for_main_agent() {
        let entry = LogEntry::new(
            make_uuid("e1"),
            None,
            make_session_id("s1"),
            None,
            make_timestamp(),
            EntryType::Assistant,
            make_message(),
            EntryMetadata::default(),
        );

        assert!(!entry.is_subagent());
    }

    #[test]
    fn log_entry_is_subagent_returns_true_for_subagent() {
        let entry = LogEntry::new(
            make_uuid("e1"),
            None,
            make_session_id("s1"),
            Some(make_agent_id("agent-123")),
            make_timestamp(),
            EntryType::User,
            make_message(),
            EntryMetadata::default(),
        );

        assert!(entry.is_subagent());
    }
}
