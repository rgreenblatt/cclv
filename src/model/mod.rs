//! Domain model types (pure).
//!
//! All types in this module are pure data with smart constructors.

pub mod error;
pub mod identifiers;
pub mod log_entry;
pub mod malformed_entry;
pub mod message;
pub mod session;
pub mod stats;
pub mod usage;

// Re-export for convenience
pub use error::{AppError, InputError, ParseError};
pub use identifiers::{
    AgentId, EntryUuid, InvalidAgentId, InvalidSessionId, InvalidToolUseId, InvalidUuid,
    SessionId, ToolUseId,
};
pub use log_entry::{EntryMetadata, EntryType, LogEntry};
pub use malformed_entry::MalformedEntry;
pub use message::{ContentBlock, Message, MessageContent, Role, ToolCall, ToolName};
pub use session::{AgentConversation, Session};
pub use stats::{ModelPricing, PricingConfig, SessionStats, StatsFilter};
pub use usage::{ModelInfo, TokenUsage};
