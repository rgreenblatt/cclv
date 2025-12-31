//! Domain model types (pure).
//!
//! All types in this module are pure data with smart constructors.

pub mod conversation_entry;
pub mod error;
pub mod identifiers;
pub mod key_action;
pub mod log_entry;
pub mod malformed_entry;
pub mod message;
pub mod stats;
pub mod usage;

// Re-export for convenience
pub use conversation_entry::ConversationEntry;
pub use error::{AppError, InputError, ParseError};
pub use identifiers::{
    AgentId, EntryUuid, InvalidAgentId, InvalidSessionId, InvalidToolUseId, InvalidUuid, SessionId,
    ToolUseId,
};
pub use key_action::KeyAction;
pub use log_entry::{EntryMetadata, EntryType, LogEntry, ResultMetadata, SystemMetadata};
pub use malformed_entry::MalformedEntry;
pub use message::{ContentBlock, Message, MessageContent, Role, ToolCall, ToolName};
pub use stats::{ModelPricing, PricingConfig, SessionStats, StatsFilter};
pub use usage::{ModelInfo, TokenUsage};
