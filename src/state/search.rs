//! Search state machine.
//!
//! SearchState is a sum type representing the three possible search states:
//! - Inactive: No search active
//! - Typing: User is entering a query
//! - Active: Search complete with results

use crate::model::{AgentId, EntryUuid, Session};

// ===== SearchState =====

/// Search state machine.
/// Sum type enforces exactly one state at a time.
#[derive(Debug, Clone)]
pub enum SearchState {
    /// No active search.
    Inactive,
    /// User is typing query.
    Typing { query: String, cursor: usize },
    /// Search complete with results.
    Active {
        query: SearchQuery,
        matches: Vec<SearchMatch>,
        current_match: usize,
    },
}

// ===== SearchQuery =====

/// Validated search query. Never empty.
/// Smart constructor enforces non-empty invariant.
#[derive(Debug, Clone)]
pub struct SearchQuery(String);

impl SearchQuery {
    /// Smart constructor: validates query is non-empty.
    /// Returns None if query is empty or whitespace-only.
    pub fn new(raw: impl Into<String>) -> Option<Self> {
        let s = raw.into();
        if s.trim().is_empty() {
            None
        } else {
            Some(Self(s))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

// ===== SearchMatch =====

/// A search match location.
#[derive(Debug, Clone)]
pub struct SearchMatch {
    pub agent_id: Option<AgentId>,
    pub entry_uuid: EntryUuid,
    pub block_index: usize,
    pub char_offset: usize,
    pub length: usize,
}

// ===== Search Execution =====

/// Execute a search across all conversations in a session.
///
/// Searches all text content in main agent and all subagents.
/// Performs case-insensitive substring matching.
/// Returns all matches with full location information.
pub fn execute_search(session: &Session, query: &SearchQuery) -> Vec<SearchMatch> {
    todo!("execute_search")
}

// ===== Tests =====

#[cfg(test)]
#[path = "search_tests.rs"]
mod tests;
