//! Search state machine for full-text search across conversations.
//!
//! # Overview
//!
//! This module implements a type-driven search state machine that enforces exactly one search
//! state at a time through sum types. The search functionality supports:
//!
//! - Case-insensitive full-text search across all conversations (main agent and subagents)
//! - Searching in text content, thinking blocks, and tool results (FR-011a)
//! - Explicit exclusion of tool use blocks (FR-011b) - structured metadata is not searchable
//! - Match highlighting and navigation (FR-012, FR-013)
//! - Indication of which tabs contain matches (FR-014)
//!
//! # State Machine
//!
//! `SearchState` is a sum type with three mutually exclusive states:
//!
//! ```text
//! ┌──────────┐
//! │ Inactive │ ◄─────────────────────────┐
//! └────┬─────┘                           │
//!      │ "/" or Ctrl+F (FR-011)          │ Esc
//!      │                                 │
//!      ▼                                 │
//! ┌──────────┐                      ┌────┴─────┐
//! │  Typing  │─────────────────────►│  Active  │
//! │  query   │  Enter (FR-012)      │ w/results│
//! └──────────┘                      └──────────┘
//!      ▲                                 │
//!      │                                 │
//!      └─────────────────────────────────┘
//!           n/N navigation (FR-013)
//! ```
//!
//! ## State Transitions
//!
//! - **Inactive → Typing**: User presses `/` or `Ctrl+F` to activate search input
//! - **Typing → Active**: User presses `Enter` to execute search with non-empty query
//! - **Typing → Inactive**: User presses `Esc` to cancel search
//! - **Active → Inactive**: User presses `Esc` to clear search and remove highlights
//! - **Active → Active**: User presses `n` (next) or `N` (previous) to navigate between matches
//!
//! # Type Design
//!
//! ## Smart Constructors
//!
//! - `SearchQuery::new()` enforces non-empty query invariant through smart constructor
//! - Returns `None` if query is empty or whitespace-only
//! - Private inner `String` prevents construction of invalid queries
//!
//! ## Invalid States Unrepresentable
//!
//! The sum type design makes these states impossible:
//! - Active search with empty query (prevented by `SearchQuery` smart constructor)
//! - Typing with no query buffer (always has `query: String`)
//! - Multiple states active simultaneously (enum enforces exactly one variant)
//!
//! # Match Navigation
//!
//! Matches are indexed sequentially across all conversations:
//! - `current_match` is a 0-based index into the `matches` vector
//! - Next (`n`) increments with wraparound: `(current + 1) % matches.len()`
//! - Previous (`N`) decrements with wraparound: `(current + matches.len() - 1) % matches.len()`
//! - Each match contains full location: agent, entry, block, character offset, and length
//!
//! # Search Semantics
//!
//! ## What is Searched (FR-011a)
//!
//! - Text content blocks (`ContentBlock::Text`)
//! - Thinking blocks (`ContentBlock::Thinking`)
//! - Tool result output (`ContentBlock::ToolResult`)
//!
//! ## What is NOT Searched (FR-011b)
//!
//! - Tool use blocks (`ContentBlock::ToolUse`) - these contain tool names and JSON parameters,
//!   which are structured metadata rather than conversation content
//!
//! ## Match Finding
//!
//! - Case-insensitive substring matching (both query and text lowercased)
//! - Overlapping matches are found (searching "aaa" in "aaaa" finds 2 matches)
//! - UTF-8 safe character boundary advancement
//!
//! # Examples
//!
//! ```rust
//! use cclv::state::search::{SearchState, SearchQuery};
//!
//! // Start with inactive search
//! let state = SearchState::Inactive;
//!
//! // User presses "/" - transition to typing
//! let state = SearchState::Typing {
//!     query: String::new(),
//!     cursor: 0,
//! };
//!
//! // User types "error" and presses Enter
//! let query = SearchQuery::new("error").unwrap();
//! // Execute search (see execute_search function)
//! // let matches = execute_search(&session, &query);
//! // let state = SearchState::Active { query, matches, current_match: 0 };
//!
//! // User presses "n" to go to next match
//! // current_match = (current_match + 1) % matches.len()
//! ```

use crate::model::{AgentId, EntryUuid};

// ===== SearchState =====

/// Search state machine.
///
/// Sum type enforces exactly one state at a time. See module documentation for state transitions.
#[derive(Debug, Clone)]
pub enum SearchState {
    /// No active search.
    Inactive,
    /// User is typing query.
    Typing {
        /// Current query string being typed.
        query: String,
        /// Cursor position within query for text editing.
        cursor: usize,
    },
    /// Search complete with results.
    Active {
        /// Validated non-empty search query.
        query: SearchQuery,
        /// All matches found across all conversations.
        matches: Vec<SearchMatch>,
        /// Index of currently focused match (0-based).
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

    /// Returns the query string as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

// ===== SearchMatch =====

/// A search match location.
///
/// Contains full location information for a single match, enabling navigation
/// and highlighting. Matches are ordered by appearance in the session.
#[derive(Debug, Clone)]
pub struct SearchMatch {
    /// Agent containing this match. None = main agent, Some(id) = subagent.
    pub agent_id: Option<AgentId>,
    /// Log entry UUID containing this match.
    pub entry_uuid: EntryUuid,
    /// Index of content block within the entry's message (0-based).
    pub block_index: usize,
    /// Character offset within the block where match starts (0-based, UTF-8 safe).
    pub char_offset: usize,
    /// Length of the matched text in characters.
    pub length: usize,
}

// ===== Match Extraction =====

/// Extract the set of agent IDs that contain search matches.
///
/// Returns a HashSet of AgentIds for all subagents that have matches.
/// Main agent matches are ignored (agent_id = None).
/// If no matches exist, returns an empty set.
pub fn agent_ids_with_matches(matches: &[SearchMatch]) -> std::collections::HashSet<AgentId> {
    matches.iter().filter_map(|m| m.agent_id.clone()).collect()
}

// ===== Search Execution =====

/// Execute a search across all conversations in a session view-state.
///
/// Searches all text content in main agent and all subagents (initialized + pending).
/// Performs case-insensitive substring matching.
/// Returns all matches with full location information.
pub fn execute_search(
    session_view: &crate::view_state::session::SessionViewState,
    query: &SearchQuery,
) -> Vec<SearchMatch> {
    let mut matches = Vec::new();
    let query_lower = query.as_str().to_lowercase();

    // Search main agent entries
    for entry_view in session_view.main().iter() {
        if let Some(log_entry) = entry_view.entry().as_valid() {
            search_entry(log_entry, None, &query_lower, &mut matches);
        }
    }

    // Search initialized subagent entries
    for (agent_id, conversation_view) in session_view.initialized_subagents() {
        for entry_view in conversation_view.iter() {
            if let Some(log_entry) = entry_view.entry().as_valid() {
                search_entry(
                    log_entry,
                    Some(agent_id.clone()),
                    &query_lower,
                    &mut matches,
                );
            }
        }
    }

    // Search pending subagent entries (not yet lazily initialized)
    for (agent_id, entries) in session_view.pending_subagents() {
        for entry in entries {
            if let Some(log_entry) = entry.as_valid() {
                search_entry(
                    log_entry,
                    Some(agent_id.clone()),
                    &query_lower,
                    &mut matches,
                );
            }
        }
    }

    matches
}

/// Search a single log entry for matches.
fn search_entry(
    log_entry: &crate::model::LogEntry,
    agent_id: Option<AgentId>,
    query_lower: &str,
    matches: &mut Vec<SearchMatch>,
) {
    use crate::model::{ContentBlock, MessageContent};

    let message = log_entry.message();
    let entry_uuid = log_entry.uuid().clone();

    match message.content() {
        MessageContent::Text(text) => {
            // Search in simple text content
            find_matches_in_text(
                text,
                &entry_uuid,
                agent_id.clone(),
                0, // block_index for Text is always 0
                query_lower,
                matches,
            );
        }
        MessageContent::Blocks(blocks) => {
            // Search in each block
            for (block_index, block) in blocks.iter().enumerate() {
                let text = match block {
                    ContentBlock::Text { text } => Some(text.as_str()),
                    ContentBlock::Thinking { thinking } => Some(thinking.as_str()),
                    ContentBlock::ToolResult { content, .. } => Some(content.as_str()),
                    ContentBlock::ToolUse(_) => None, // Don't search tool use blocks
                };

                if let Some(text) = text {
                    find_matches_in_text(
                        text,
                        &entry_uuid,
                        agent_id.clone(),
                        block_index,
                        query_lower,
                        matches,
                    );
                }
            }
        }
    }
}

/// Find all matches of query in text and add to matches vector.
fn find_matches_in_text(
    text: &str,
    entry_uuid: &EntryUuid,
    agent_id: Option<AgentId>,
    block_index: usize,
    query_lower: &str,
    matches: &mut Vec<SearchMatch>,
) {
    let text_lower = text.to_lowercase();
    let query_len = query_lower.len();

    // Find all overlapping matches
    let mut start = 0;
    while let Some(pos) = text_lower[start..].find(query_lower) {
        let char_offset = start + pos;
        matches.push(SearchMatch {
            agent_id: agent_id.clone(),
            entry_uuid: entry_uuid.clone(),
            block_index,
            char_offset,
            length: query_len,
        });
        // Advance to next character boundary for UTF-8 safety
        start = text_lower[char_offset..]
            .char_indices()
            .nth(1)
            .map(|(idx, _)| char_offset + idx)
            .unwrap_or(text_lower.len());
    }
}

// ===== Tests =====

#[cfg(test)]
#[path = "search_tests.rs"]
mod tests;
