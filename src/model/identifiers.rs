//! Core identifier newtypes with smart constructors.
//!
//! All identifiers validate non-empty strings at construction time.
//! Raw constructors are never exported - use smart constructors only.

use std::fmt;

/// Unique identifier for a log entry within a session.
/// NEVER export the constructor.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EntryUuid(String);

impl EntryUuid {
    /// Smart constructor: validates non-empty UUID.
    ///
    /// # Errors
    /// Returns `InvalidUuid::Empty` if the input is empty.
    ///
    /// # Examples
    /// ```
    /// # use cclv::model::identifiers::EntryUuid;
    /// let uuid = EntryUuid::new("550e8400-e29b-41d4-a716-446655440000")?;
    /// assert_eq!(uuid.as_str(), "550e8400-e29b-41d4-a716-446655440000");
    /// # Ok::<(), cclv::model::identifiers::InvalidUuid>(())
    /// ```
    pub fn new(raw: impl Into<String>) -> Result<Self, InvalidUuid> {
        let s = raw.into();
        if s.is_empty() {
            return Err(InvalidUuid::Empty);
        }
        Ok(Self(s))
    }

    /// Returns the underlying string slice.
    ///
    /// This is the only way to access the wrapped value after construction,
    /// ensuring the UUID has been validated via the smart constructor.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for EntryUuid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Session identifier grouping related log entries.
///
/// A session represents a single Claude Code interaction, containing
/// a main agent and zero or more subagents. All log entries with the
/// same `SessionId` belong to the same conversation.
///
/// # Invariants
/// - Never empty (enforced by smart constructor)
/// - Must be constructed via `new()` or `unknown()`
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SessionId(String);

impl SessionId {
    /// Smart constructor: validates non-empty session ID.
    ///
    /// # Errors
    /// Returns `InvalidSessionId::Empty` if the input is empty.
    ///
    /// # Examples
    /// ```
    /// # use cclv::model::identifiers::SessionId;
    /// let id = SessionId::new("session-12345")?;
    /// assert_eq!(id.as_str(), "session-12345");
    /// # Ok::<(), cclv::model::identifiers::InvalidSessionId>(())
    /// ```
    pub fn new(raw: impl Into<String>) -> Result<Self, InvalidSessionId> {
        let s = raw.into();
        if s.is_empty() {
            return Err(InvalidSessionId::Empty);
        }
        Ok(Self(s))
    }

    /// Returns a pre-validated fallback `SessionId` for unknown sessions.
    ///
    /// This is panic-free and used as a last resort when parsing encounters
    /// a log entry with no valid session ID. The returned value is guaranteed
    /// to be valid (non-empty).
    pub fn unknown() -> Self {
        // SAFETY: "unknown-session" is a valid non-empty string constant
        Self(String::from("unknown-session"))
    }

    /// Returns the underlying string slice.
    ///
    /// This is the only way to access the wrapped value after construction,
    /// ensuring the session ID has been validated via a smart constructor.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SessionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Subagent identifier identifying a task delegated to a specialized agent.
///
/// Subagents are spawned by the main agent to handle specific tasks like
/// file analysis, test execution, or code review. Each subagent gets a
/// unique short ID (e.g., "a7b2877") that appears in the log entries.
///
/// # Invariants
/// - Never empty (enforced by smart constructor)
/// - Uniquely identifies a subagent within a session
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AgentId(String);

impl AgentId {
    /// Smart constructor: validates non-empty agent ID.
    ///
    /// # Errors
    /// Returns `InvalidAgentId::Empty` if the input is empty.
    ///
    /// # Examples
    /// ```
    /// # use cclv::model::identifiers::AgentId;
    /// let id = AgentId::new("a7b2877")?;
    /// assert_eq!(id.as_str(), "a7b2877");
    /// # Ok::<(), cclv::model::identifiers::InvalidAgentId>(())
    /// ```
    pub fn new(raw: impl Into<String>) -> Result<Self, InvalidAgentId> {
        let s = raw.into();
        if s.is_empty() {
            return Err(InvalidAgentId::Empty);
        }
        Ok(Self(s))
    }

    /// Returns the underlying string slice.
    ///
    /// This is the only way to access the wrapped value after construction,
    /// ensuring the agent ID has been validated via the smart constructor.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for AgentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Tool invocation identifier for linking `tool_use` to `tool_result` blocks.
///
/// When Claude invokes a tool (e.g., Read, Write, Bash), it generates a
/// `tool_use` content block with a unique ID. The corresponding result
/// appears in a `tool_result` block with the same ID, allowing the
/// viewer to correlate requests with responses.
///
/// # Invariants
/// - Never empty (enforced by smart constructor)
/// - Uniquely identifies a tool invocation within a message
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ToolUseId(String);

impl ToolUseId {
    /// Smart constructor: validates non-empty tool use ID.
    ///
    /// # Errors
    /// Returns `InvalidToolUseId::Empty` if the input is empty.
    ///
    /// # Examples
    /// ```
    /// # use cclv::model::identifiers::ToolUseId;
    /// let id = ToolUseId::new("toolu_01A2B3C4D5E6F")?;
    /// assert_eq!(id.as_str(), "toolu_01A2B3C4D5E6F");
    /// # Ok::<(), cclv::model::identifiers::InvalidToolUseId>(())
    /// ```
    pub fn new(raw: impl Into<String>) -> Result<Self, InvalidToolUseId> {
        let s = raw.into();
        if s.is_empty() {
            return Err(InvalidToolUseId::Empty);
        }
        Ok(Self(s))
    }

    /// Returns the underlying string slice.
    ///
    /// This is the only way to access the wrapped value after construction,
    /// ensuring the tool use ID has been validated via the smart constructor.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ToolUseId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

// ===== Error Types =====

/// Error returned when constructing an `EntryUuid` from invalid input.
#[derive(Debug, Clone, thiserror::Error)]
pub enum InvalidUuid {
    /// The UUID string was empty.
    #[error("UUID cannot be empty")]
    Empty,
}

/// Error returned when constructing a `SessionId` from invalid input.
#[derive(Debug, Clone, thiserror::Error)]
pub enum InvalidSessionId {
    /// The session ID string was empty.
    #[error("Session ID cannot be empty")]
    Empty,
}

/// Error returned when constructing an `AgentId` from invalid input.
#[derive(Debug, Clone, thiserror::Error)]
pub enum InvalidAgentId {
    /// The agent ID string was empty.
    #[error("Agent ID cannot be empty")]
    Empty,
}

/// Error returned when constructing a `ToolUseId` from invalid input.
#[derive(Debug, Clone, thiserror::Error)]
pub enum InvalidToolUseId {
    /// The tool use ID string was empty.
    #[error("Tool Use ID cannot be empty")]
    Empty,
}

// ===== Tests =====

#[cfg(test)]
mod tests {
    use super::*;

    // ===== EntryUuid Tests =====

    #[test]
    fn entry_uuid_accepts_valid_string() {
        let uuid = EntryUuid::new("550e8400-e29b-41d4-a716-446655440000");
        assert!(uuid.is_ok(), "Valid UUID should be accepted");
    }

    #[test]
    fn entry_uuid_accepts_simple_alphanumeric() {
        let uuid = EntryUuid::new("abc123");
        assert!(uuid.is_ok(), "Simple alphanumeric should be accepted");
    }

    #[test]
    fn entry_uuid_rejects_empty_string() {
        let uuid = EntryUuid::new("");
        assert!(
            matches!(uuid, Err(InvalidUuid::Empty)),
            "Empty string should return InvalidUuid::Empty"
        );
    }

    #[test]
    fn entry_uuid_as_str_returns_original() {
        let original = "550e8400-e29b-41d4-a716-446655440000";
        let uuid = EntryUuid::new(original).expect("Valid UUID");
        assert_eq!(
            uuid.as_str(),
            original,
            "as_str() should return original value"
        );
    }

    #[test]
    fn entry_uuid_display_returns_inner_string() {
        let original = "550e8400-e29b-41d4-a716-446655440000";
        let uuid = EntryUuid::new(original).expect("Valid UUID");
        assert_eq!(
            uuid.to_string(),
            original,
            "Display should output inner string"
        );
    }

    #[test]
    fn entry_uuid_accepts_string_type() {
        let owned = String::from("abc123");
        let uuid = EntryUuid::new(owned);
        assert!(uuid.is_ok(), "Should accept owned String");
    }

    // ===== SessionId Tests =====

    #[test]
    fn session_id_accepts_valid_string() {
        let id = SessionId::new("session-12345");
        assert!(id.is_ok(), "Valid session ID should be accepted");
    }

    #[test]
    fn session_id_rejects_empty_string() {
        let id = SessionId::new("");
        assert!(
            matches!(id, Err(InvalidSessionId::Empty)),
            "Empty string should return InvalidSessionId::Empty"
        );
    }

    #[test]
    fn session_id_as_str_returns_original() {
        let original = "session-12345";
        let id = SessionId::new(original).expect("Valid session ID");
        assert_eq!(
            id.as_str(),
            original,
            "as_str() should return original value"
        );
    }

    #[test]
    fn session_id_display_returns_inner_string() {
        let original = "session-12345";
        let id = SessionId::new(original).expect("Valid session ID");
        assert_eq!(
            id.to_string(),
            original,
            "Display should output inner string"
        );
    }

    #[test]
    fn session_id_accepts_string_type() {
        let owned = String::from("session-abc");
        let id = SessionId::new(owned);
        assert!(id.is_ok(), "Should accept owned String");
    }

    // ===== AgentId Tests =====

    #[test]
    fn agent_id_accepts_valid_string() {
        let id = AgentId::new("a7b2877");
        assert!(id.is_ok(), "Valid agent ID should be accepted");
    }

    #[test]
    fn agent_id_rejects_empty_string() {
        let id = AgentId::new("");
        assert!(
            matches!(id, Err(InvalidAgentId::Empty)),
            "Empty string should return InvalidAgentId::Empty"
        );
    }

    #[test]
    fn agent_id_as_str_returns_original() {
        let original = "a7b2877";
        let id = AgentId::new(original).expect("Valid agent ID");
        assert_eq!(
            id.as_str(),
            original,
            "as_str() should return original value"
        );
    }

    #[test]
    fn agent_id_display_returns_inner_string() {
        let original = "a7b2877";
        let id = AgentId::new(original).expect("Valid agent ID");
        assert_eq!(
            id.to_string(),
            original,
            "Display should output inner string"
        );
    }

    #[test]
    fn agent_id_accepts_string_type() {
        let owned = String::from("agent-xyz");
        let id = AgentId::new(owned);
        assert!(id.is_ok(), "Should accept owned String");
    }

    // ===== ToolUseId Tests =====

    #[test]
    fn tool_use_id_accepts_valid_string() {
        let id = ToolUseId::new("tool-123");
        assert!(id.is_ok(), "Valid tool use ID should be accepted");
    }

    #[test]
    fn tool_use_id_rejects_empty_string() {
        let id = ToolUseId::new("");
        assert!(
            matches!(id, Err(InvalidToolUseId::Empty)),
            "Empty string should return InvalidToolUseId::Empty"
        );
    }

    #[test]
    fn tool_use_id_as_str_returns_original() {
        let original = "tool-123";
        let id = ToolUseId::new(original).expect("Valid tool use ID");
        assert_eq!(
            id.as_str(),
            original,
            "as_str() should return original value"
        );
    }

    #[test]
    fn tool_use_id_display_returns_inner_string() {
        let original = "tool-123";
        let id = ToolUseId::new(original).expect("Valid tool use ID");
        assert_eq!(
            id.to_string(),
            original,
            "Display should output inner string"
        );
    }

    #[test]
    fn tool_use_id_accepts_string_type() {
        let owned = String::from("tool-xyz");
        let id = ToolUseId::new(owned);
        assert!(id.is_ok(), "Should accept owned String");
    }

    // ===== Error Message Tests =====

    #[test]
    fn invalid_uuid_error_message() {
        let err = InvalidUuid::Empty;
        assert_eq!(err.to_string(), "UUID cannot be empty");
    }

    #[test]
    fn invalid_session_id_error_message() {
        let err = InvalidSessionId::Empty;
        assert_eq!(err.to_string(), "Session ID cannot be empty");
    }

    #[test]
    fn invalid_agent_id_error_message() {
        let err = InvalidAgentId::Empty;
        assert_eq!(err.to_string(), "Agent ID cannot be empty");
    }

    #[test]
    fn invalid_tool_use_id_error_message() {
        let err = InvalidToolUseId::Empty;
        assert_eq!(err.to_string(), "Tool Use ID cannot be empty");
    }

    // ===== Clone and Equality Tests =====

    #[test]
    fn entry_uuid_clone_equals_original() {
        let uuid = EntryUuid::new("test-uuid").expect("Valid UUID");
        let cloned = uuid.clone();
        assert_eq!(uuid, cloned, "Cloned UUID should equal original");
    }

    #[test]
    fn session_id_clone_equals_original() {
        let id = SessionId::new("test-session").expect("Valid session ID");
        let cloned = id.clone();
        assert_eq!(id, cloned, "Cloned SessionId should equal original");
    }

    #[test]
    fn agent_id_clone_equals_original() {
        let id = AgentId::new("test-agent").expect("Valid agent ID");
        let cloned = id.clone();
        assert_eq!(id, cloned, "Cloned AgentId should equal original");
    }

    #[test]
    fn tool_use_id_clone_equals_original() {
        let id = ToolUseId::new("test-tool").expect("Valid tool use ID");
        let cloned = id.clone();
        assert_eq!(id, cloned, "Cloned ToolUseId should equal original");
    }
}
