//! Domain model types (pure).
//!
//! All types in this module are pure data with smart constructors.

pub mod identifiers;

// Re-export for convenience
pub use identifiers::{
    AgentId, EntryUuid, InvalidAgentId, InvalidSessionId, InvalidToolUseId, InvalidUuid,
    SessionId, ToolUseId,
};
