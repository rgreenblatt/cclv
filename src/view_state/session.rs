//! View-state for a single session

use super::conversation::ConversationViewState;
use crate::model::{AgentId, ConversationEntry, SessionId};
use std::collections::HashMap;

/// View-state for a single session.
///
/// Contains:
/// - Main conversation view-state (always present)
/// - Subagent view-states (lazily created on first view, FR-073)
/// - Pending subagent entries (before view-state creation)
///
/// # Lazy Initialization (FR-073)
/// Subagent view-states are created lazily when first accessed.
/// Until accessed, entries are stored in `pending_subagent_entries`.
#[derive(Debug, Clone)]
pub struct SessionViewState {
    /// Session identifier.
    session_id: SessionId,
    /// Main conversation view-state.
    main: ConversationViewState,
    /// Subagent view-states (lazily initialized).
    subagents: HashMap<AgentId, ConversationViewState>,
    /// Pending subagent entries (before lazy init).
    pending_subagent_entries: HashMap<AgentId, Vec<ConversationEntry>>,
    /// Cumulative line offset from start of log (for multi-session).
    start_line: usize,
    /// Maximum context window size (from config).
    max_context_tokens: usize,
    /// Pricing configuration (from config).
    pricing: crate::model::PricingConfig,
}

impl SessionViewState {
    /// Create new session view-state.
    pub fn new(session_id: SessionId) -> Self {
        Self {
            session_id,
            main: ConversationViewState::empty(),
            subagents: HashMap::new(),
            pending_subagent_entries: HashMap::new(),
            start_line: 0,
            max_context_tokens: 200_000, // Default
            pricing: crate::model::PricingConfig::default(),
        }
    }

    /// Session identifier.
    pub fn session_id(&self) -> &SessionId {
        &self.session_id
    }

    /// Reference to main conversation view-state.
    pub fn main(&self) -> &ConversationViewState {
        &self.main
    }

    /// Mutable reference to main conversation.
    pub fn main_mut(&mut self) -> &mut ConversationViewState {
        &mut self.main
    }

    /// Reference to subagents map.
    pub fn subagents(&self) -> &HashMap<AgentId, ConversationViewState> {
        &self.subagents
    }

    /// Get subagent view-state, creating lazily if needed.
    pub fn subagent(&mut self, id: &AgentId) -> &ConversationViewState {
        if !self.subagents.contains_key(id) {
            // Create from pending entries
            let entries = self.pending_subagent_entries.remove(id).unwrap_or_default();
            let view_state = ConversationViewState::new(
                Some(id.clone()),
                None,
                entries,
                self.max_context_tokens,
                self.pricing.clone(),
            );
            self.subagents.insert(id.clone(), view_state);
        }
        self.subagents.get(id).unwrap()
    }

    /// Mutable reference to subagent view-state.
    pub fn subagent_mut(&mut self, id: &AgentId) -> &mut ConversationViewState {
        if !self.subagents.contains_key(id) {
            let entries = self.pending_subagent_entries.remove(id).unwrap_or_default();
            let view_state = ConversationViewState::new(
                Some(id.clone()),
                None,
                entries,
                self.max_context_tokens,
                self.pricing.clone(),
            );
            self.subagents.insert(id.clone(), view_state);
        }
        self.subagents.get_mut(id).unwrap()
    }

    /// Check if subagent view-state exists (has been accessed).
    pub fn has_subagent(&self, id: &AgentId) -> bool {
        self.subagents.contains_key(id)
    }

    /// Get subagent view-state without creating it.
    /// Returns None if subagent hasn't been initialized yet.
    pub fn get_subagent(&self, id: &AgentId) -> Option<&ConversationViewState> {
        self.subagents.get(id)
    }

    /// List all known subagent IDs (initialized or pending).
    pub fn subagent_ids(&self) -> impl Iterator<Item = &AgentId> {
        self.subagents
            .keys()
            .chain(self.pending_subagent_entries.keys())
    }

    /// Check if there are any subagents (initialized or pending).
    pub fn has_subagents(&self) -> bool {
        !self.subagents.is_empty() || !self.pending_subagent_entries.is_empty()
    }

    /// Get system metadata from main conversation.
    pub fn system_metadata(&self) -> Option<&crate::model::SystemMetadata> {
        self.main.system_metadata()
    }

    /// Iterate all initialized subagent conversations.
    ///
    /// Returns iterator over (AgentId, ConversationViewState) pairs for
    /// subagents that have been lazily initialized.
    pub fn initialized_subagents(
        &self,
    ) -> impl Iterator<Item = (&AgentId, &ConversationViewState)> {
        self.subagents.iter()
    }

    /// Iterate all pending subagent entries.
    ///
    /// Returns iterator over (AgentId, Vec<ConversationEntry>) pairs for
    /// subagents that have entries but haven't been lazily initialized yet.
    pub fn pending_subagents(&self) -> impl Iterator<Item = (&AgentId, &Vec<ConversationEntry>)> {
        self.pending_subagent_entries.iter()
    }

    /// Get subagent entry count (from either initialized or pending state).
    ///
    /// Returns the number of entries for the given agent ID without requiring mutation.
    /// Checks initialized subagents first, then pending entries.
    pub fn get_subagent_entry_count(&self, id: &AgentId) -> usize {
        // Check initialized subagents first
        if let Some(view_state) = self.subagents.get(id) {
            return view_state.len();
        }
        // Check pending entries
        self.pending_subagent_entries.get(id).map_or(0, |v| v.len())
    }

    /// Add entry to main conversation.
    pub fn add_main_entry(&mut self, entry: ConversationEntry) {
        self.main.append_entries(vec![entry]);
    }

    /// Add entry to subagent conversation.
    /// Creates the subagent view-state eagerly via subagent_mut().
    pub fn add_subagent_entry(&mut self, agent_id: AgentId, entry: ConversationEntry) {
        self.subagent_mut(&agent_id).append_entries(vec![entry]);
    }

    /// Start line offset (for multi-session positioning).
    pub fn start_line(&self) -> usize {
        self.start_line
    }

    /// Set start line offset.
    #[allow(dead_code)] // Used by LogViewState in same module
    pub(crate) fn set_start_line(&mut self, offset: usize) {
        self.start_line = offset;
    }

    /// Height of main conversation only.
    pub fn main_height(&self) -> usize {
        self.main.total_height()
    }

    /// Total height of all conversations in this session.
    /// In continuous scroll display mode, this is the height contribution
    /// of this entire session to the log view.
    ///
    /// Includes:
    /// - Main conversation height
    /// - All initialized subagent conversation heights
    /// - Pending subagent entries (estimated at 1 line each until initialized)
    pub fn total_height(&self) -> usize {
        let main_h = self.main.total_height();
        let subagent_h: usize = self.subagents.values().map(|s| s.total_height()).sum();
        let pending_h: usize = self
            .pending_subagent_entries
            .values()
            .map(|v| v.len())
            .sum();
        main_h + subagent_h + pending_h
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{
        EntryMetadata, EntryType, EntryUuid, LogEntry, Message, MessageContent, Role,
    };

    // ===== Test Helpers =====

    fn make_session_id(s: &str) -> SessionId {
        SessionId::new(s).expect("valid session id")
    }

    fn make_agent_id(s: &str) -> AgentId {
        AgentId::new(s).expect("valid agent id")
    }

    fn make_entry_uuid(s: &str) -> EntryUuid {
        EntryUuid::new(s).expect("valid uuid")
    }

    fn make_timestamp() -> chrono::DateTime<chrono::Utc> {
        "2025-12-25T10:00:00Z".parse().expect("valid timestamp")
    }

    fn make_message(text: &str) -> Message {
        Message::new(Role::User, MessageContent::Text(text.to_string()))
    }

    fn make_valid_entry(uuid: &str, session_id: &str) -> ConversationEntry {
        let log_entry = LogEntry::new(
            make_entry_uuid(uuid),
            None,
            make_session_id(session_id),
            None,
            make_timestamp(),
            EntryType::User,
            make_message(uuid), // Use UUID as message text for easy identification
            EntryMetadata::default(),
        );
        ConversationEntry::Valid(Box::new(log_entry))
    }

    // ===== SessionViewState::new Tests =====

    #[test]
    fn new_creates_session_with_empty_main_conversation() {
        let session_id = make_session_id("session-1");
        let state = SessionViewState::new(session_id.clone());

        assert_eq!(state.session_id(), &session_id);
        assert!(state.main().is_empty(), "Main conversation should be empty");
    }

    // ===== session_id Tests =====

    #[test]
    fn session_id_returns_correct_value() {
        let session_id = make_session_id("test-session");
        let state = SessionViewState::new(session_id.clone());

        assert_eq!(state.session_id(), &session_id);
    }

    // ===== main/main_mut Tests =====

    #[test]
    fn main_returns_main_conversation() {
        let session_id = make_session_id("session-1");
        let state = SessionViewState::new(session_id);

        let main = state.main();
        assert!(main.is_empty());
    }

    #[test]
    fn main_mut_allows_mutation() {
        let session_id = make_session_id("session-1");
        let mut state = SessionViewState::new(session_id);

        state
            .main_mut()
            .append(vec![make_valid_entry("uuid-1", "session-1")]);
        assert_eq!(state.main().len(), 1);
    }

    // ===== Lazy Initialization Tests =====

    #[test]
    fn subagent_creates_view_state_on_first_access() {
        let session_id = make_session_id("session-1");
        let mut state = SessionViewState::new(session_id);
        let agent_id = make_agent_id("agent-1");

        // Before access: should not have subagent
        assert!(
            !state.has_subagent(&agent_id),
            "Should not have subagent before access"
        );

        // First access: creates view-state
        let subagent = state.subagent(&agent_id);
        assert!(
            subagent.is_empty(),
            "Newly created subagent should be empty"
        );

        // After access: should have subagent
        assert!(
            state.has_subagent(&agent_id),
            "Should have subagent after access"
        );
    }

    #[test]
    fn add_subagent_entry_after_access_goes_directly_to_subagent() {
        let session_id = make_session_id("session-1");
        let mut state = SessionViewState::new(session_id);
        let agent_id = make_agent_id("agent-1");

        // First access creates empty subagent
        let _ = state.subagent(&agent_id);
        assert_eq!(state.subagent(&agent_id).len(), 0);

        // Add entry after initialization
        state.add_subagent_entry(agent_id.clone(), make_valid_entry("uuid-1", "session-1"));

        // Should go directly to subagent
        assert_eq!(state.subagent(&agent_id).len(), 1);
    }

    #[test]
    fn subagent_mut_also_creates_lazily() {
        let session_id = make_session_id("session-1");
        let mut state = SessionViewState::new(session_id);
        let agent_id = make_agent_id("agent-1");

        // Use mutable access for first time
        state
            .subagent_mut(&agent_id)
            .append(vec![make_valid_entry("uuid-1", "session-1")]);

        assert!(state.has_subagent(&agent_id));
        assert_eq!(state.subagent(&agent_id).len(), 1);
    }

    // ===== subagent_ids Tests =====

    #[test]
    fn subagent_ids_returns_both_initialized_and_pending() {
        let session_id = make_session_id("session-1");
        let mut state = SessionViewState::new(session_id);

        let agent1 = make_agent_id("agent-1");
        let agent2 = make_agent_id("agent-2");
        let agent3 = make_agent_id("agent-3");

        // Initialize agent1
        let _ = state.subagent(&agent1);

        // Add pending for agent2
        state.add_subagent_entry(agent2.clone(), make_valid_entry("uuid-1", "session-1"));

        // Add pending for agent3
        state.add_subagent_entry(agent3.clone(), make_valid_entry("uuid-2", "session-1"));

        let mut ids: Vec<_> = state.subagent_ids().cloned().collect();
        ids.sort_by(|a, b| a.as_str().cmp(b.as_str()));

        assert_eq!(ids.len(), 3);
        assert!(ids.contains(&agent1));
        assert!(ids.contains(&agent2));
        assert!(ids.contains(&agent3));
    }

    #[test]
    fn subagent_ids_empty_when_no_subagents() {
        let session_id = make_session_id("session-1");
        let state = SessionViewState::new(session_id);

        let ids: Vec<_> = state.subagent_ids().collect();
        assert!(ids.is_empty());
    }

    // ===== has_subagent Tests =====

    #[test]
    fn has_subagent_returns_false_for_never_accessed() {
        let session_id = make_session_id("session-1");
        let state = SessionViewState::new(session_id);
        let agent_id = make_agent_id("agent-1");

        assert!(!state.has_subagent(&agent_id));
    }

    #[test]
    fn has_subagent_returns_true_after_add_entry() {
        let session_id = make_session_id("session-1");
        let mut state = SessionViewState::new(session_id);
        let agent_id = make_agent_id("agent-1");

        // Eager initialization: adding entry immediately creates subagent
        state.add_subagent_entry(agent_id.clone(), make_valid_entry("uuid-1", "session-1"));

        assert!(state.has_subagent(&agent_id));
    }

    #[test]
    fn has_subagent_returns_true_after_access() {
        let session_id = make_session_id("session-1");
        let mut state = SessionViewState::new(session_id);
        let agent_id = make_agent_id("agent-1");

        let _ = state.subagent(&agent_id);

        assert!(state.has_subagent(&agent_id));
    }

    // ===== start_line/set_start_line Tests =====

    #[test]
    fn start_line_defaults_to_zero() {
        let session_id = make_session_id("session-1");
        let state = SessionViewState::new(session_id);

        assert_eq!(state.start_line(), 0);
    }

    #[test]
    fn set_start_line_updates_offset() {
        let session_id = make_session_id("session-1");
        let mut state = SessionViewState::new(session_id);

        state.set_start_line(100);
        assert_eq!(state.start_line(), 100);
    }

    // ===== add_main_entry Tests =====

    #[test]
    fn add_main_entry_appends_to_main_conversation() {
        let session_id = make_session_id("session-1");
        let mut state = SessionViewState::new(session_id);

        state.add_main_entry(make_valid_entry("uuid-1", "session-1"));
        state.add_main_entry(make_valid_entry("uuid-2", "session-1"));

        assert_eq!(state.main().len(), 2);
    }

    // ===== main_height Tests =====

    #[test]
    fn main_height_returns_main_conversation_total_height() {
        let session_id = make_session_id("session-1");
        let state = SessionViewState::new(session_id);

        // Note: ConversationViewState returns 0 for total_height until layout computed
        // This test verifies delegation, not the layout logic
        let height = state.main_height();
        assert_eq!(height, state.main().total_height());
    }

    // ===== total_height Tests =====

    #[test]
    fn total_height_includes_main_and_subagents() {
        let session_id = make_session_id("session-1");
        let mut state = SessionViewState::new(session_id);
        let agent_id = make_agent_id("agent-1");

        // Add to main
        state.add_main_entry(make_valid_entry("uuid-1", "session-1"));

        // Initialize subagent and add entry
        state
            .subagent_mut(&agent_id)
            .append(vec![make_valid_entry("uuid-2", "session-1")]);

        // Total height = main + initialized subagents + pending estimate
        // Note: Without layout computation, heights are 0
        // This test verifies the calculation logic, not the actual values
        let total = state.total_height();
        let expected = state.main().total_height() + state.subagent(&agent_id).total_height();
        assert_eq!(total, expected);
    }

    #[test]
    fn total_height_includes_initialized_subagents() {
        let session_id = make_session_id("session-1");
        let mut state = SessionViewState::new(session_id);
        let agent_id = make_agent_id("agent-1");

        // With eager initialization, adding entries immediately creates subagent
        state.add_subagent_entry(agent_id.clone(), make_valid_entry("uuid-1", "session-1"));
        state.add_subagent_entry(agent_id.clone(), make_valid_entry("uuid-2", "session-1"));
        state.add_subagent_entry(agent_id.clone(), make_valid_entry("uuid-3", "session-1"));

        // Total height should include subagent height
        // Note: Without layout computation, heights are 0
        let total = state.total_height();
        let expected = state.main().total_height() + state.subagent(&agent_id).total_height();
        assert_eq!(total, expected);
    }

    #[test]
    fn total_height_zero_for_empty_session() {
        let session_id = make_session_id("session-1");
        let state = SessionViewState::new(session_id);

        assert_eq!(state.total_height(), 0);
    }

    // ===== Bug Regression Tests (cclv-5ur.30) =====

    #[test]
    fn get_subagent_returns_entries_added_before_initialization() {
        // REGRESSION TEST for cclv-5ur.30: Subagent conversations mixing with main
        //
        // BUG: Entries with agentId field were appearing in main conversation
        // instead of routing to separate subagent tabs.
        //
        // ROOT CAUSE: Rendering uses get_subagent() which returns None for
        // pending (non-initialized) subagents, so entries were invisible.
        //
        // This test reproduces the rendering scenario:
        // 1. Entries arrive and route to pending_subagent_entries
        // 2. View layer tries to render using get_subagent() (read-only)
        // 3. View layer expects to see the entries
        let session_id = make_session_id("session-1");
        let mut state = SessionViewState::new(session_id);
        let agent_id = make_agent_id("agent-1");

        // Simulate entries arriving from log parsing
        state.add_subagent_entry(agent_id.clone(), make_valid_entry("uuid-1", "session-1"));
        state.add_subagent_entry(agent_id.clone(), make_valid_entry("uuid-2", "session-1"));

        // Simulate view layer rendering (read-only access, like layout.rs:302-313)
        // EXPECTATION: Should see the 2 entries that were added
        let subagent_view = state.get_subagent(&agent_id);

        // ASSERTION: Entries should be visible for rendering
        assert!(
            subagent_view.is_some(),
            "get_subagent() should return Some for subagent with pending entries"
        );
        assert_eq!(
            subagent_view.unwrap().len(),
            2,
            "Subagent conversation should contain 2 entries added before initialization"
        );
    }
}
