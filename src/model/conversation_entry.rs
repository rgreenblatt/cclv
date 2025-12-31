//! Conversation entry type - unified storage for valid and malformed entries.
//!
//! This module provides the ConversationEntry enum which allows
//! AgentConversation to store both successfully parsed LogEntry instances
//! and malformed entries in a single ordered sequence.

use crate::model::{EntryUuid, LogEntry, MalformedEntry, SessionId};
use crate::parser::ParseResult;
use chrono::{DateTime, Utc};

/// A single entry in an agent conversation.
///
/// Can be either a valid parsed entry or a malformed entry that
/// failed parsing but should still be displayed inline with context.
#[derive(Debug, Clone)]
pub enum ConversationEntry {
    /// Successfully parsed log entry
    Valid(Box<LogEntry>),
    /// Malformed entry that failed parsing
    Malformed(MalformedEntry),
}

impl ConversationEntry {
    /// Get the session ID if available.
    ///
    /// Returns Some for both Valid entries (always has session_id)
    /// and Malformed entries (may have extracted session_id).
    pub fn session_id(&self) -> Option<&SessionId> {
        match self {
            ConversationEntry::Valid(entry) => Some(entry.session_id()),
            ConversationEntry::Malformed(malformed) => malformed.session_id(),
        }
    }

    /// Get the timestamp if available.
    ///
    /// Returns Some for Valid entries (always has timestamp),
    /// None for Malformed entries (no timestamp available).
    pub fn timestamp(&self) -> Option<DateTime<Utc>> {
        match self {
            ConversationEntry::Valid(entry) => Some(entry.timestamp()),
            ConversationEntry::Malformed(_) => None,
        }
    }

    /// Get the entry UUID if available.
    ///
    /// Returns Some for Valid entries (always has uuid),
    /// None for Malformed entries (no UUID - failed to parse).
    pub fn uuid(&self) -> Option<&EntryUuid> {
        match self {
            ConversationEntry::Valid(entry) => Some(entry.uuid()),
            ConversationEntry::Malformed(_) => None,
        }
    }

    /// Check if this is a valid entry.
    pub fn is_valid(&self) -> bool {
        matches!(self, ConversationEntry::Valid(_))
    }

    /// Check if this is a malformed entry.
    pub fn is_malformed(&self) -> bool {
        matches!(self, ConversationEntry::Malformed(_))
    }

    /// Get the valid entry if this is a Valid variant.
    pub fn as_valid(&self) -> Option<&LogEntry> {
        match self {
            ConversationEntry::Valid(entry) => Some(entry),
            ConversationEntry::Malformed(_) => None,
        }
    }

    /// Get the malformed entry if this is a Malformed variant.
    pub fn as_malformed(&self) -> Option<&MalformedEntry> {
        match self {
            ConversationEntry::Valid(_) => None,
            ConversationEntry::Malformed(malformed) => Some(malformed),
        }
    }

    /// Get the input token count for this entry.
    ///
    /// Returns the total input token count from the entry's message usage metadata.
    /// Input tokens are what count toward context window limits.
    /// Returns 0 for malformed entries or entries without usage data.
    pub fn token_count(&self) -> usize {
        match self {
            ConversationEntry::Valid(entry) => entry
                .message()
                .usage()
                .map(|usage| usage.total_input() as usize)
                .unwrap_or(0),
            ConversationEntry::Malformed(_) => 0,
        }
    }
}

impl From<ParseResult> for ConversationEntry {
    fn from(result: ParseResult) -> Self {
        match result {
            ParseResult::Valid(entry) => ConversationEntry::Valid(entry),
            ParseResult::Malformed(malformed) => ConversationEntry::Malformed(malformed),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{EntryMetadata, EntryType, EntryUuid, Message, MessageContent, Role};

    // ===== Test Helpers =====

    fn make_session_id(s: &str) -> SessionId {
        SessionId::new(s).expect("valid session id")
    }

    fn make_entry_uuid(s: &str) -> EntryUuid {
        EntryUuid::new(s).expect("valid uuid")
    }

    fn make_timestamp() -> DateTime<Utc> {
        "2025-12-25T10:00:00Z".parse().expect("valid timestamp")
    }

    fn make_message() -> Message {
        Message::new(Role::User, MessageContent::Text("Test".to_string()))
    }

    fn make_valid_log_entry() -> LogEntry {
        LogEntry::new(
            make_entry_uuid("uuid-1"),
            None,
            make_session_id("session-1"),
            None,
            make_timestamp(),
            EntryType::User,
            make_message(),
            EntryMetadata::default(),
        )
    }

    fn make_malformed_entry() -> MalformedEntry {
        MalformedEntry::new(
            42,
            "bad json",
            "Parse error: unexpected token",
            Some(make_session_id("session-1")),
        )
    }

    // ===== ConversationEntry::session_id Tests =====

    #[test]
    fn conversation_entry_session_id_returns_some_for_valid() {
        let entry = LogEntry::new(
            make_entry_uuid("uuid-1"),
            None,
            make_session_id("session-123"),
            None,
            make_timestamp(),
            EntryType::User,
            make_message(),
            EntryMetadata::default(),
        );
        let conv_entry = ConversationEntry::Valid(Box::new(entry));

        let session_id = conv_entry.session_id();

        assert!(session_id.is_some(), "Valid entry should have session_id");
        assert_eq!(session_id.unwrap().as_str(), "session-123");
    }

    #[test]
    fn conversation_entry_session_id_returns_some_for_malformed_with_session() {
        let malformed =
            MalformedEntry::new(10, "bad", "error", Some(make_session_id("session-456")));
        let conv_entry = ConversationEntry::Malformed(malformed);

        let session_id = conv_entry.session_id();

        assert!(
            session_id.is_some(),
            "Malformed entry with session_id should return it"
        );
        assert_eq!(session_id.unwrap().as_str(), "session-456");
    }

    #[test]
    fn conversation_entry_session_id_returns_none_for_malformed_without_session() {
        let malformed = MalformedEntry::new(10, "bad", "error", None);
        let conv_entry = ConversationEntry::Malformed(malformed);

        let session_id = conv_entry.session_id();

        assert!(
            session_id.is_none(),
            "Malformed entry without session_id should return None"
        );
    }

    // ===== ConversationEntry::timestamp Tests =====

    #[test]
    fn conversation_entry_timestamp_returns_some_for_valid() {
        let entry = make_valid_log_entry();
        let expected_timestamp = entry.timestamp();
        let conv_entry = ConversationEntry::Valid(Box::new(entry));

        let timestamp = conv_entry.timestamp();

        assert!(timestamp.is_some(), "Valid entry should have timestamp");
        assert_eq!(timestamp.unwrap(), expected_timestamp);
    }

    #[test]
    fn conversation_entry_timestamp_returns_none_for_malformed() {
        let malformed = make_malformed_entry();
        let conv_entry = ConversationEntry::Malformed(malformed);

        let timestamp = conv_entry.timestamp();

        assert!(
            timestamp.is_none(),
            "Malformed entry should not have timestamp"
        );
    }

    // ===== ConversationEntry::is_valid / is_malformed Tests =====

    #[test]
    fn conversation_entry_is_valid_returns_true_for_valid() {
        let entry = make_valid_log_entry();
        let conv_entry = ConversationEntry::Valid(Box::new(entry));

        assert!(conv_entry.is_valid());
        assert!(!conv_entry.is_malformed());
    }

    #[test]
    fn conversation_entry_is_malformed_returns_true_for_malformed() {
        let malformed = make_malformed_entry();
        let conv_entry = ConversationEntry::Malformed(malformed);

        assert!(!conv_entry.is_valid());
        assert!(conv_entry.is_malformed());
    }

    // ===== ConversationEntry::as_valid / as_malformed Tests =====

    #[test]
    fn conversation_entry_as_valid_returns_some_for_valid() {
        let entry = make_valid_log_entry();
        let uuid = entry.uuid().clone();
        let conv_entry = ConversationEntry::Valid(Box::new(entry));

        let valid = conv_entry.as_valid();

        assert!(valid.is_some());
        assert_eq!(valid.unwrap().uuid(), &uuid);
    }

    #[test]
    fn conversation_entry_as_valid_returns_none_for_malformed() {
        let malformed = make_malformed_entry();
        let conv_entry = ConversationEntry::Malformed(malformed);

        let valid = conv_entry.as_valid();

        assert!(valid.is_none());
    }

    #[test]
    fn conversation_entry_as_malformed_returns_some_for_malformed() {
        let malformed = make_malformed_entry();
        let conv_entry = ConversationEntry::Malformed(malformed);

        let malformed_ref = conv_entry.as_malformed();

        assert!(malformed_ref.is_some());
        assert_eq!(malformed_ref.unwrap().line_number(), 42);
    }

    #[test]
    fn conversation_entry_as_malformed_returns_none_for_valid() {
        let entry = make_valid_log_entry();
        let conv_entry = ConversationEntry::Valid(Box::new(entry));

        let malformed = conv_entry.as_malformed();

        assert!(malformed.is_none());
    }
}
