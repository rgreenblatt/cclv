//! Domain model types (pure).
//!
//! All types in this module are pure data with smart constructors.

pub mod identifiers;
pub mod log_entry;
pub mod message;
pub mod session;
pub mod usage;

// Re-export for convenience
pub use identifiers::{
    AgentId, EntryUuid, InvalidAgentId, InvalidSessionId, InvalidToolUseId, InvalidUuid,
    SessionId, ToolUseId,
};
pub use log_entry::{EntryMetadata, EntryType, LogEntry};
pub use message::{ContentBlock, Message, MessageContent, Role, ToolCall, ToolName};
pub use session::{AgentConversation, Session};
pub use usage::{ModelInfo, TokenUsage};
