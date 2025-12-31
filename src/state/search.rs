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

// ===== Match Extraction =====

/// Extract the set of agent IDs that contain search matches.
///
/// Returns a HashSet of AgentIds for all subagents that have matches.
/// Main agent matches are ignored (agent_id = None).
/// If no matches exist, returns an empty set.
pub fn agent_ids_with_matches(matches: &[SearchMatch]) -> std::collections::HashSet<AgentId> {
    matches
        .iter()
        .filter_map(|m| m.agent_id.clone())
        .collect()
}

// ===== Search Execution =====

/// Execute a search across all conversations in a session.
///
/// Searches all text content in main agent and all subagents.
/// Performs case-insensitive substring matching.
/// Returns all matches with full location information.
pub fn execute_search(session: &Session, query: &SearchQuery) -> Vec<SearchMatch> {
    let mut matches = Vec::new();
    let query_lower = query.as_str().to_lowercase();

    // Search main agent
    search_conversation(session.main_agent(), None, &query_lower, &mut matches);

    // Search all subagents
    for (agent_id, conversation) in session.subagents() {
        search_conversation(conversation, Some(agent_id.clone()), &query_lower, &mut matches);
    }

    matches
}

/// Search a single conversation (main or subagent) for matches.
fn search_conversation(
    conversation: &crate::model::AgentConversation,
    agent_id: Option<AgentId>,
    query_lower: &str,
    matches: &mut Vec<SearchMatch>,
) {
    use crate::model::{ContentBlock, MessageContent};

    for entry in conversation.entries() {
        // Only search valid entries
        if let Some(log_entry) = entry.as_valid() {
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
        start = char_offset + 1; // Move by 1 to find overlapping matches
    }
}

// ===== Tests =====

#[cfg(test)]
#[path = "search_tests.rs"]
mod tests;
