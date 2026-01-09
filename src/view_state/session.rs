//! View-state for a single session

use super::conversation::ConversationViewState;
use crate::model::{AgentId, ConversationEntry, SessionId};
use crate::state::WrapMode;
use chrono::{DateTime, Utc};
use std::collections::HashMap;

/// View-state for a single session.
///
/// Contains:
/// - Main conversation view-state (always present)
/// - Subagent view-states (created eagerly on first entry, FR-073)
///
/// # Eager Initialization (FR-073)
/// Subagent view-states are created eagerly when the first entry for that
/// subagent arrives. This ensures view-state exists before rendering, avoiding
/// mutable access during immutable render pass.
#[derive(Debug, Clone)]
pub struct SessionViewState {
    /// Session identifier.
    session_id: SessionId,
    /// Main conversation view-state.
    main: ConversationViewState,
    /// Subagent view-states (eagerly initialized on first entry).
    subagents: HashMap<AgentId, ConversationViewState>,
    /// Cumulative line offset from start of log (for multi-session).
    start_line: usize,
    /// Maximum context window size (from config).
    max_context_tokens: usize,
    /// Pricing configuration (from config).
    pricing: crate::model::PricingConfig,
    /// Current viewport width (for propagating to newly created subagents).
    viewport_width: u16,
    /// Global wrap mode (for propagating to newly created subagents).
    global_wrap: WrapMode,
    /// Timestamp of the first entry added to this session (main or subagent).
    start_time: Option<DateTime<Utc>>,
}

impl SessionViewState {
    /// Create new session view-state.
    pub fn new(session_id: SessionId) -> Self {
        Self {
            session_id,
            main: ConversationViewState::empty(),
            subagents: HashMap::new(),
            start_line: 0,
            max_context_tokens: 200_000, // Default
            pricing: crate::model::PricingConfig::default(),
            viewport_width: 0,
            global_wrap: WrapMode::default(),
            start_time: None,
        }
    }

    /// Session identifier.
    pub fn session_id(&self) -> &SessionId {
        &self.session_id
    }

    /// Timestamp of the first entry added to this session.
    ///
    /// Returns None if no entries have been added yet.
    pub fn start_time(&self) -> Option<DateTime<Utc>> {
        self.start_time
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

    /// Get subagent view-state, creating empty state if needed.
    pub fn subagent(&mut self, id: &AgentId) -> &ConversationViewState {
        if !self.subagents.contains_key(id) {
            // Create empty view-state
            let view_state = ConversationViewState::new(
                Some(id.clone()),
                None,
                vec![],
                self.max_context_tokens,
                self.pricing.clone(),
            );
            self.subagents.insert(id.clone(), view_state);
        }
        self.subagents.get(id).unwrap()
    }

    /// Mutable reference to subagent view-state, creating empty state if needed.
    ///
    /// When creating a new subagent, propagates current viewport dimensions
    /// by calling relayout() with stored viewport_width and global_wrap.
    pub fn subagent_mut(&mut self, id: &AgentId) -> &mut ConversationViewState {
        if !self.subagents.contains_key(id) {
            use std::io::Write;
            let _ = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open("/tmp/cclv_debug.log")
                .and_then(|mut f| {
                    writeln!(
                        f,
                        "DEBUG subagent_mut creating: agent_id={:?}, self.viewport_width={}",
                        id, self.viewport_width
                    )
                });
            // Create empty view-state
            let view_state = ConversationViewState::new(
                Some(id.clone()),
                None,
                vec![],
                self.max_context_tokens,
                self.pricing.clone(),
            );
            self.subagents.insert(id.clone(), view_state);

            // Propagate viewport dimensions to newly created subagent
            if self.viewport_width > 0 {
                self.subagents.get_mut(id).unwrap().relayout(
                    self.viewport_width,
                    self.global_wrap,
                    &crate::state::SearchState::Inactive,
                );
            }
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

    /// Get mutable subagent view-state without creating it.
    /// Returns None if subagent hasn't been initialized yet.
    ///
    /// This is the mutable counterpart to `get_subagent()`, with identical semantics:
    /// it does NOT create the subagent if it doesn't exist.
    pub fn get_subagent_mut(&mut self, id: &AgentId) -> Option<&mut ConversationViewState> {
        self.subagents.get_mut(id)
    }

    /// List all known subagent IDs.
    pub fn subagent_ids(&self) -> impl Iterator<Item = &AgentId> {
        self.subagents.keys()
    }

    /// Check if there are any subagents.
    pub fn has_subagents(&self) -> bool {
        !self.subagents.is_empty()
    }

    /// Get system metadata from main conversation.
    pub fn system_metadata(&self) -> Option<&crate::model::SystemMetadata> {
        self.main.system_metadata()
    }

    /// Iterate all subagent conversations.
    ///
    /// Returns iterator over (AgentId, ConversationViewState) pairs for all subagents.
    pub fn initialized_subagents(
        &self,
    ) -> impl Iterator<Item = (&AgentId, &ConversationViewState)> {
        self.subagents.iter()
    }

    /// Get subagent entry count.
    ///
    /// Returns the number of entries for the given agent ID without requiring mutation.
    pub fn get_subagent_entry_count(&self, id: &AgentId) -> usize {
        self.subagents.get(id).map_or(0, |v| v.len())
    }

    /// Add entry to main conversation.
    ///
    /// # Model Extraction
    /// If the entry is an assistant message with a model field, and the main
    /// conversation has no model yet, extracts and stores the model in ConversationViewState.
    ///
    /// # Start Time Tracking (cclv-463.6.3)
    /// If this is the first entry added to the session, captures its timestamp as start_time.
    pub fn add_main_entry(&mut self, entry: ConversationEntry) {
        // Track start time from first entry (cclv-463.6.3)
        if self.start_time.is_none() {
            if let Some(timestamp) = entry.timestamp() {
                self.start_time = Some(timestamp);
            }
        }

        // Extract model from assistant message if present
        if let ConversationEntry::Valid(log_entry) = &entry {
            if let Some(model) = log_entry.message().model() {
                // Clone model before getting mutable reference to avoid borrow checker issues
                let model_clone = model.clone();
                self.main.set_model_if_none(model_clone);
                self.main
                    .append_entries(vec![entry], &crate::state::SearchState::Inactive);
                return;
            }
        }

        // No model to extract, just append
        self.main
            .append_entries(vec![entry], &crate::state::SearchState::Inactive);
    }

    /// Add entry to subagent conversation.
    /// Creates the subagent view-state eagerly via subagent_mut().
    ///
    /// # Model Extraction (cclv-5ur.40.13)
    /// If the entry is an assistant message with a model field, and the subagent
    /// has no model yet, extracts and stores the model in ConversationViewState.
    ///
    /// # Start Time Tracking (cclv-463.6.3)
    /// If this is the first entry added to the session, captures its timestamp as start_time.
    pub fn add_subagent_entry(&mut self, agent_id: AgentId, entry: ConversationEntry) {
        // Track start time from first entry (cclv-463.6.3)
        if self.start_time.is_none() {
            if let Some(timestamp) = entry.timestamp() {
                self.start_time = Some(timestamp);
            }
        }

        // Extract model from assistant message if present (cclv-5ur.40.13)
        if let ConversationEntry::Valid(log_entry) = &entry {
            if let Some(model) = log_entry.message().model() {
                // Clone model before getting mutable reference to avoid borrow checker issues
                let model_clone = model.clone();
                let subagent = self.subagent_mut(&agent_id);
                subagent.set_model_if_none(model_clone);
                subagent.append_entries(vec![entry], &crate::state::SearchState::Inactive);
                return;
            }
        }

        // No model to extract, just append
        self.subagent_mut(&agent_id)
            .append_entries(vec![entry], &crate::state::SearchState::Inactive);
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

    /// Set viewport dimensions and relayout all conversations.
    ///
    /// When viewport dimensions change (e.g., terminal resize), call this
    /// to store the new width and wrap mode. These will be propagated to
    /// any newly created subagents via subagent_mut().
    ///
    /// Also relayouts main and all existing subagent conversations with
    /// the new dimensions.
    pub fn set_viewport(&mut self, width: u16, wrap: WrapMode) {
        self.viewport_width = width;
        self.global_wrap = wrap;

        // Relayout main conversation
        // Use Inactive search state since this is called during initialization/resize
        self.main
            .relayout(width, wrap, &crate::state::SearchState::Inactive);

        // Relayout all existing subagents
        for subagent in self.subagents.values_mut() {
            subagent.relayout(width, wrap, &crate::state::SearchState::Inactive);
        }
    }

    /// Get current viewport width.
    pub fn viewport_width(&self) -> u16 {
        self.viewport_width
    }

    /// Get current global wrap mode.
    pub fn global_wrap(&self) -> WrapMode {
        self.global_wrap
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
    /// - All subagent conversation heights
    pub fn total_height(&self) -> usize {
        let main_h = self.main.total_height();
        let subagent_h: usize = self.subagents.values().map(|s| s.total_height()).sum();
        main_h + subagent_h
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

    // ===== Eager Initialization Tests =====

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
    fn subagent_mut_also_creates_on_first_access() {
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
    fn subagent_ids_returns_all_subagents() {
        let session_id = make_session_id("session-1");
        let mut state = SessionViewState::new(session_id);

        let agent1 = make_agent_id("agent-1");
        let agent2 = make_agent_id("agent-2");
        let agent3 = make_agent_id("agent-3");

        // Initialize agent1 (empty)
        let _ = state.subagent(&agent1);

        // Add entries for agent2 (eager initialization)
        state.add_subagent_entry(agent2.clone(), make_valid_entry("uuid-1", "session-1"));

        // Add entries for agent3 (eager initialization)
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
        // ROOT CAUSE: Lazy initialization used pending_subagent_entries, but
        // get_subagent() (read-only) couldn't trigger initialization.
        //
        // FIX: Eager initialization - add_subagent_entry() now creates view-state
        // immediately via subagent_mut(), ensuring entries are always visible.
        //
        // This test verifies the fix:
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

    // ===== Subagent Model Extraction Tests (cclv-5ur.40.13) =====

    #[test]
    fn add_subagent_entry_extracts_model_from_first_assistant_message() {
        // TEST for cclv-5ur.40.13: Extract model from first assistant message
        //
        // REQUIREMENT: When adding an assistant entry with model field to a subagent
        // that has no model yet, store the model in the ConversationViewState.
        //
        // Real log analysis shows assistant messages in subagent conversations have
        // the model field (e.g., claude-sonnet-4-5-20250929, claude-haiku-4-5-20251001).

        let session_id = make_session_id("session-1");
        let mut state = SessionViewState::new(session_id);
        let agent_id = make_agent_id("agent-1");

        // Create assistant message with model
        let model_info = crate::model::ModelInfo::new("claude-sonnet-4-5-20250929");
        let message = Message::new(
            Role::Assistant,
            MessageContent::Text("Response from subagent".to_string()),
        )
        .with_model(model_info.clone());

        let entry = ConversationEntry::Valid(Box::new(LogEntry::new(
            make_entry_uuid("uuid-1"),
            None,
            make_session_id("session-1"),
            None,
            make_timestamp(),
            EntryType::Assistant,
            message,
            EntryMetadata::default(),
        )));

        // Before adding: subagent doesn't exist yet
        assert!(!state.has_subagent(&agent_id));

        // Add assistant entry with model
        state.add_subagent_entry(agent_id.clone(), entry);

        // After adding: subagent should exist and have the model
        assert!(state.has_subagent(&agent_id));
        let subagent = state
            .get_subagent(&agent_id)
            .expect("subagent should exist");

        assert!(
            subagent.model().is_some(),
            "Subagent should have model extracted from first assistant message"
        );
        assert_eq!(
            subagent.model().unwrap().id(),
            "claude-sonnet-4-5-20250929",
            "Model should match the one from the assistant message"
        );
    }

    #[test]
    fn add_subagent_entry_does_not_overwrite_existing_model() {
        // TEST for cclv-5ur.40.13: Don't overwrite if model already set
        //
        // REQUIREMENT: Only extract model if subagent has no model yet.
        // If model is already set, don't overwrite it.

        let session_id = make_session_id("session-1");
        let mut state = SessionViewState::new(session_id);
        let agent_id = make_agent_id("agent-1");

        // Create first assistant message with model
        let first_model = crate::model::ModelInfo::new("claude-haiku-4-5-20251001");
        let first_message = Message::new(
            Role::Assistant,
            MessageContent::Text("First response".to_string()),
        )
        .with_model(first_model.clone());

        let first_entry = ConversationEntry::Valid(Box::new(LogEntry::new(
            make_entry_uuid("uuid-1"),
            None,
            make_session_id("session-1"),
            None,
            make_timestamp(),
            EntryType::Assistant,
            first_message,
            EntryMetadata::default(),
        )));

        // Add first entry (should set model)
        state.add_subagent_entry(agent_id.clone(), first_entry);

        let subagent = state
            .get_subagent(&agent_id)
            .expect("subagent should exist");
        assert_eq!(
            subagent.model().unwrap().id(),
            "claude-haiku-4-5-20251001",
            "First model should be set"
        );

        // Create second assistant message with DIFFERENT model
        let second_model = crate::model::ModelInfo::new("claude-sonnet-4-5-20250929");
        let second_message = Message::new(
            Role::Assistant,
            MessageContent::Text("Second response".to_string()),
        )
        .with_model(second_model.clone());

        let second_entry = ConversationEntry::Valid(Box::new(LogEntry::new(
            make_entry_uuid("uuid-2"),
            None,
            make_session_id("session-1"),
            None,
            make_timestamp(),
            EntryType::Assistant,
            second_message,
            EntryMetadata::default(),
        )));

        // Add second entry (should NOT overwrite model)
        state.add_subagent_entry(agent_id.clone(), second_entry);

        let subagent = state
            .get_subagent(&agent_id)
            .expect("subagent should exist");
        assert_eq!(
            subagent.model().unwrap().id(),
            "claude-haiku-4-5-20251001",
            "Model should NOT be overwritten - should still be the first one"
        );
    }

    #[test]
    fn add_subagent_entry_ignores_user_messages_for_model_extraction() {
        // TEST for cclv-5ur.40.13: Only extract from assistant messages
        //
        // REQUIREMENT: User messages don't have model field (expected).
        // Only assistant messages should trigger model extraction.

        let session_id = make_session_id("session-1");
        let mut state = SessionViewState::new(session_id);
        let agent_id = make_agent_id("agent-1");

        // Create user message (no model)
        let user_message = Message::new(
            Role::User,
            MessageContent::Text("User question".to_string()),
        );

        let user_entry = ConversationEntry::Valid(Box::new(LogEntry::new(
            make_entry_uuid("uuid-1"),
            None,
            make_session_id("session-1"),
            None,
            make_timestamp(),
            EntryType::User,
            user_message,
            EntryMetadata::default(),
        )));

        // Add user entry
        state.add_subagent_entry(agent_id.clone(), user_entry);

        // Subagent should exist but have no model
        let subagent = state
            .get_subagent(&agent_id)
            .expect("subagent should exist");
        assert!(
            subagent.model().is_none(),
            "Subagent should have no model from user message"
        );
    }

    // ===== Main Agent Model Extraction Tests =====

    #[test]
    fn add_main_entry_extracts_model_from_first_assistant_message() {
        // TEST: Extract model from first assistant message in main conversation
        //
        // BUG FIX: Main agent model is not properly detected. Subagents correctly
        // extract model via set_model_if_none() in add_subagent_entry(), but main
        // agent entries don't go through equivalent model extraction.
        //
        // REQUIREMENT: When adding an assistant entry with model field to main
        // conversation that has no model yet, store the model in the ConversationViewState.

        let session_id = make_session_id("session-1");
        let mut state = SessionViewState::new(session_id);

        // Create assistant message with model
        let model_info = crate::model::ModelInfo::new("claude-opus-4-5-20251101");
        let message = Message::new(
            Role::Assistant,
            MessageContent::Text("Response from main agent".to_string()),
        )
        .with_model(model_info.clone());

        let entry = ConversationEntry::Valid(Box::new(LogEntry::new(
            make_entry_uuid("uuid-1"),
            None,
            make_session_id("session-1"),
            None,
            make_timestamp(),
            EntryType::Assistant,
            message,
            EntryMetadata::default(),
        )));

        // Before adding: main conversation has no model
        assert!(
            state.main().model().is_none(),
            "Main conversation should start with no model"
        );

        // Add assistant entry with model to main conversation
        state.add_main_entry(entry);

        // After adding: main conversation should have the model
        assert!(
            state.main().model().is_some(),
            "Main conversation should have model extracted from first assistant message"
        );
        assert_eq!(
            state.main().model().unwrap().id(),
            "claude-opus-4-5-20251101",
            "Model should match the one from the assistant message"
        );
    }

    #[test]
    fn add_main_entry_does_not_overwrite_existing_model() {
        // TEST: Don't overwrite if model already set in main conversation
        //
        // REQUIREMENT: Only extract model if main conversation has no model yet.
        // If model is already set, don't overwrite it.

        let session_id = make_session_id("session-1");
        let mut state = SessionViewState::new(session_id);

        // Create first assistant message with model
        let first_model = crate::model::ModelInfo::new("claude-opus-4-5-20251101");
        let first_message = Message::new(
            Role::Assistant,
            MessageContent::Text("First response".to_string()),
        )
        .with_model(first_model.clone());

        let first_entry = ConversationEntry::Valid(Box::new(LogEntry::new(
            make_entry_uuid("uuid-1"),
            None,
            make_session_id("session-1"),
            None,
            make_timestamp(),
            EntryType::Assistant,
            first_message,
            EntryMetadata::default(),
        )));

        // Add first entry (should set model)
        state.add_main_entry(first_entry);

        assert_eq!(
            state.main().model().unwrap().id(),
            "claude-opus-4-5-20251101",
            "First model should be set"
        );

        // Create second assistant message with DIFFERENT model
        let second_model = crate::model::ModelInfo::new("claude-sonnet-4-5-20250929");
        let second_message = Message::new(
            Role::Assistant,
            MessageContent::Text("Second response".to_string()),
        )
        .with_model(second_model.clone());

        let second_entry = ConversationEntry::Valid(Box::new(LogEntry::new(
            make_entry_uuid("uuid-2"),
            None,
            make_session_id("session-1"),
            None,
            make_timestamp(),
            EntryType::Assistant,
            second_message,
            EntryMetadata::default(),
        )));

        // Add second entry (should NOT overwrite model)
        state.add_main_entry(second_entry);

        assert_eq!(
            state.main().model().unwrap().id(),
            "claude-opus-4-5-20251101",
            "Model should NOT be overwritten - should still be the first one"
        );
    }

    #[test]
    fn add_main_entry_ignores_user_messages_for_model_extraction() {
        // TEST: Only extract from assistant messages in main conversation
        //
        // REQUIREMENT: User messages don't have model field (expected).
        // Only assistant messages should trigger model extraction.

        let session_id = make_session_id("session-1");
        let mut state = SessionViewState::new(session_id);

        // Create user message (no model)
        let user_message = Message::new(
            Role::User,
            MessageContent::Text("User question".to_string()),
        );

        let user_entry = ConversationEntry::Valid(Box::new(LogEntry::new(
            make_entry_uuid("uuid-1"),
            None,
            make_session_id("session-1"),
            None,
            make_timestamp(),
            EntryType::User,
            user_message,
            EntryMetadata::default(),
        )));

        // Add user entry to main
        state.add_main_entry(user_entry);

        // Main conversation should have no model
        assert!(
            state.main().model().is_none(),
            "Main conversation should have no model from user message"
        );
    }

    // ===== Start Time Tracking Tests (cclv-463.6.3) =====

    #[test]
    fn start_time_is_none_for_new_session() {
        // RED TEST: New session should have no start_time
        let session_id = make_session_id("session-1");
        let state = SessionViewState::new(session_id);

        assert_eq!(
            state.start_time(),
            None,
            "New session should have no start_time"
        );
    }

    #[test]
    fn start_time_captured_from_first_main_entry() {
        // RED TEST: First entry to main conversation sets start_time
        let session_id = make_session_id("session-1");
        let mut state = SessionViewState::new(session_id);

        let timestamp1 = "2025-01-09T10:00:00Z".parse().expect("valid timestamp");
        let entry1 = ConversationEntry::Valid(Box::new(LogEntry::new(
            make_entry_uuid("uuid-1"),
            None,
            make_session_id("session-1"),
            None,
            timestamp1,
            EntryType::User,
            make_message("First entry"),
            EntryMetadata::default(),
        )));

        state.add_main_entry(entry1);

        assert_eq!(
            state.start_time(),
            Some(timestamp1),
            "start_time should be timestamp of first entry"
        );
    }

    #[test]
    fn start_time_not_updated_by_subsequent_main_entries() {
        // RED TEST: Second entry should NOT update start_time
        let session_id = make_session_id("session-1");
        let mut state = SessionViewState::new(session_id);

        let timestamp1 = "2025-01-09T10:00:00Z".parse().expect("valid timestamp");
        let timestamp2 = "2025-01-09T11:00:00Z".parse().expect("valid timestamp");

        let entry1 = ConversationEntry::Valid(Box::new(LogEntry::new(
            make_entry_uuid("uuid-1"),
            None,
            make_session_id("session-1"),
            None,
            timestamp1,
            EntryType::User,
            make_message("First entry"),
            EntryMetadata::default(),
        )));

        let entry2 = ConversationEntry::Valid(Box::new(LogEntry::new(
            make_entry_uuid("uuid-2"),
            None,
            make_session_id("session-1"),
            None,
            timestamp2,
            EntryType::User,
            make_message("Second entry"),
            EntryMetadata::default(),
        )));

        state.add_main_entry(entry1);
        state.add_main_entry(entry2);

        assert_eq!(
            state.start_time(),
            Some(timestamp1),
            "start_time should remain the timestamp of first entry, not second"
        );
    }

    #[test]
    fn start_time_captured_from_first_subagent_entry() {
        // RED TEST: First entry to subagent (when no main entries yet) sets start_time
        let session_id = make_session_id("session-1");
        let mut state = SessionViewState::new(session_id);
        let agent_id = make_agent_id("agent-1");

        let timestamp1 = "2025-01-09T10:00:00Z".parse().expect("valid timestamp");
        let entry1 = ConversationEntry::Valid(Box::new(LogEntry::new(
            make_entry_uuid("uuid-1"),
            None,
            make_session_id("session-1"),
            None,
            timestamp1,
            EntryType::User,
            make_message("First subagent entry"),
            EntryMetadata::default(),
        )));

        state.add_subagent_entry(agent_id, entry1);

        assert_eq!(
            state.start_time(),
            Some(timestamp1),
            "start_time should be timestamp of first subagent entry when no main entries"
        );
    }

    #[test]
    fn start_time_uses_earliest_entry_main_before_subagent() {
        // RED TEST: If main entry comes first, subagent entry doesn't override
        let session_id = make_session_id("session-1");
        let mut state = SessionViewState::new(session_id);
        let agent_id = make_agent_id("agent-1");

        let timestamp_main = "2025-01-09T10:00:00Z".parse().expect("valid timestamp");
        let timestamp_subagent = "2025-01-09T11:00:00Z"
            .parse()
            .expect("valid timestamp");

        let main_entry = ConversationEntry::Valid(Box::new(LogEntry::new(
            make_entry_uuid("uuid-main"),
            None,
            make_session_id("session-1"),
            None,
            timestamp_main,
            EntryType::User,
            make_message("Main entry"),
            EntryMetadata::default(),
        )));

        let subagent_entry = ConversationEntry::Valid(Box::new(LogEntry::new(
            make_entry_uuid("uuid-sub"),
            None,
            make_session_id("session-1"),
            None,
            timestamp_subagent,
            EntryType::User,
            make_message("Subagent entry"),
            EntryMetadata::default(),
        )));

        state.add_main_entry(main_entry);
        state.add_subagent_entry(agent_id, subagent_entry);

        assert_eq!(
            state.start_time(),
            Some(timestamp_main),
            "start_time should remain main entry timestamp (earlier)"
        );
    }

    #[test]
    fn start_time_uses_earliest_entry_subagent_before_main() {
        // RED TEST: If subagent entry comes first, main entry doesn't override
        let session_id = make_session_id("session-1");
        let mut state = SessionViewState::new(session_id);
        let agent_id = make_agent_id("agent-1");

        let timestamp_subagent = "2025-01-09T10:00:00Z"
            .parse()
            .expect("valid timestamp");
        let timestamp_main = "2025-01-09T11:00:00Z".parse().expect("valid timestamp");

        let subagent_entry = ConversationEntry::Valid(Box::new(LogEntry::new(
            make_entry_uuid("uuid-sub"),
            None,
            make_session_id("session-1"),
            None,
            timestamp_subagent,
            EntryType::User,
            make_message("Subagent entry"),
            EntryMetadata::default(),
        )));

        let main_entry = ConversationEntry::Valid(Box::new(LogEntry::new(
            make_entry_uuid("uuid-main"),
            None,
            make_session_id("session-1"),
            None,
            timestamp_main,
            EntryType::User,
            make_message("Main entry"),
            EntryMetadata::default(),
        )));

        state.add_subagent_entry(agent_id, subagent_entry);
        state.add_main_entry(main_entry);

        assert_eq!(
            state.start_time(),
            Some(timestamp_subagent),
            "start_time should remain subagent entry timestamp (earlier)"
        );
    }

    #[test]
    fn start_time_ignores_malformed_entries() {
        // RED TEST: Malformed entries (no timestamp) should not set start_time
        let session_id = make_session_id("session-1");
        let mut state = SessionViewState::new(session_id);

        use crate::model::MalformedEntry;
        let malformed = ConversationEntry::Malformed(MalformedEntry::new(
            1,
            "bad json",
            "parse error",
            Some(make_session_id("session-1")),
        ));

        state.add_main_entry(malformed);

        assert_eq!(
            state.start_time(),
            None,
            "Malformed entries should not set start_time"
        );
    }

    #[test]
    fn start_time_set_by_first_valid_entry_after_malformed() {
        // RED TEST: First valid entry after malformed entries sets start_time
        let session_id = make_session_id("session-1");
        let mut state = SessionViewState::new(session_id);

        use crate::model::MalformedEntry;
        let malformed = ConversationEntry::Malformed(MalformedEntry::new(
            1,
            "bad json",
            "parse error",
            Some(make_session_id("session-1")),
        ));

        let timestamp1 = "2025-01-09T10:00:00Z".parse().expect("valid timestamp");
        let valid_entry = ConversationEntry::Valid(Box::new(LogEntry::new(
            make_entry_uuid("uuid-1"),
            None,
            make_session_id("session-1"),
            None,
            timestamp1,
            EntryType::User,
            make_message("First valid entry"),
            EntryMetadata::default(),
        )));

        state.add_main_entry(malformed);
        state.add_main_entry(valid_entry);

        assert_eq!(
            state.start_time(),
            Some(timestamp1),
            "First valid entry should set start_time, ignoring malformed"
        );
    }
}
