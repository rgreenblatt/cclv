//! Tests for central routing methods (cclv-5ur.49)
//!
//! Tests that AppState routing methods correctly map selected_tab to conversations.

use crate::model::{
    AgentId, ConversationEntry, EntryMetadata, EntryType, EntryUuid, LogEntry, Message,
    MessageContent, Role, SessionId,
};
use crate::state::{AppState, ConversationSelection};
use chrono::Utc;

// ===== Test Helpers =====

/// Create a test conversation entry for the main agent.
fn create_main_entry(uuid_str: &str, text: &str) -> ConversationEntry {
    let uuid = EntryUuid::new(uuid_str).unwrap();
    let session = SessionId::new("test-session").unwrap();
    let message = Message::new(Role::User, MessageContent::Text(text.to_string()));
    let entry = LogEntry::new(
        uuid,
        None, // parent_uuid
        session,
        None, // agent_id (main agent has none)
        Utc::now(),
        EntryType::User,
        message,
        EntryMetadata::default(),
    );
    ConversationEntry::Valid(Box::new(entry))
}

/// Create a test conversation entry for a subagent.
fn create_subagent_entry(uuid_str: &str, agent_id_str: &str, text: &str) -> ConversationEntry {
    let uuid = EntryUuid::new(uuid_str).unwrap();
    let session = SessionId::new("test-session").unwrap();
    let agent_id = AgentId::new(agent_id_str).unwrap();
    let message = Message::new(Role::Assistant, MessageContent::Text(text.to_string()));
    let entry = LogEntry::new(
        uuid,
        None, // parent_uuid
        session,
        Some(agent_id), // agent_id (this is a subagent entry)
        Utc::now(),
        EntryType::Result,
        message,
        EntryMetadata::default(),
    );
    ConversationEntry::Valid(Box::new(entry))
}

// ===== Tests for selected_conversation_view() =====

#[test]
fn selected_conversation_view_tab0_routes_to_main() {
    // GIVEN: AppState with main agent entries and selected_tab = 0
    let mut state = AppState::new();
    state.add_entries(vec![
        create_main_entry("main-1", "Main agent message 1"),
        create_main_entry("main-2", "Main agent message 2"),
    ]);
    state.selected_conversation = ConversationSelection::Main;

    // WHEN: Getting selected conversation view
    let view = state.selected_conversation_view();

    // THEN: Returns main conversation
    assert!(view.is_some(), "Should return main conversation for tab 0");
    let view = view.unwrap();
    assert_eq!(view.len(), 2, "Main conversation should have 2 entries");
}

#[test]
fn selected_conversation_view_tab1_routes_to_first_subagent() {
    // GIVEN: AppState with main + 2 subagents, selected_tab = 1
    let mut state = AppState::new();
    state.add_entries(vec![
        create_main_entry("main-1", "Main agent message"),
        create_subagent_entry("sub-b-1", "subagent-bravo", "Bravo message"),
        create_subagent_entry("sub-a-1", "subagent-alpha", "Alpha message"),
    ]);
    state.selected_conversation =
        ConversationSelection::Subagent(AgentId::new("subagent-alpha").unwrap());

    // WHEN: Getting selected conversation view
    let view = state.selected_conversation_view();

    // THEN: Returns first subagent by sorted AgentId (alpha comes before bravo)
    assert!(
        view.is_some(),
        "Should return subagent conversation for tab 1"
    );
    let view = view.unwrap();
    assert_eq!(view.len(), 1, "First subagent (alpha) should have 1 entry");
}

#[test]
fn selected_conversation_view_multiple_subagents_correct_routing() {
    // GIVEN: AppState with main + 3 subagents
    let mut state = AppState::new();
    state.add_entries(vec![
        create_main_entry("main-1", "Main agent message"),
        create_subagent_entry("sub-c-1", "subagent-charlie", "Charlie 1"),
        create_subagent_entry("sub-c-2", "subagent-charlie", "Charlie 2"),
        create_subagent_entry("sub-a-1", "subagent-alpha", "Alpha 1"),
        create_subagent_entry("sub-b-1", "subagent-bravo", "Bravo 1"),
        create_subagent_entry("sub-b-2", "subagent-bravo", "Bravo 2"),
        create_subagent_entry("sub-b-3", "subagent-bravo", "Bravo 3"),
    ]);

    // THEN: Tab 0 -> main (1 entry)
    state.selected_conversation = ConversationSelection::Main;
    let view = state.selected_conversation_view().unwrap();
    assert_eq!(view.len(), 1, "Tab 0 should route to main");

    // THEN: Tab 1 -> alpha (1 entry, first by sort)
    state.selected_conversation =
        ConversationSelection::Subagent(AgentId::new("subagent-alpha").unwrap());
    let view = state.selected_conversation_view().unwrap();
    assert_eq!(view.len(), 1, "Tab 1 should route to alpha");

    // THEN: Tab 2 -> bravo (3 entries, second by sort)
    state.selected_conversation =
        ConversationSelection::Subagent(AgentId::new("subagent-bravo").unwrap());
    let view = state.selected_conversation_view().unwrap();
    assert_eq!(view.len(), 3, "Tab 2 should route to bravo");

    // THEN: Tab 3 -> charlie (2 entries, third by sort)
    state.selected_conversation =
        ConversationSelection::Subagent(AgentId::new("subagent-charlie").unwrap());
    let view = state.selected_conversation_view().unwrap();
    assert_eq!(view.len(), 2, "Tab 3 should route to charlie");
}

#[test]
fn selected_conversation_view_returns_none_when_no_session() {
    // GIVEN: Empty AppState (no sessions)
    let state = AppState::new();

    // WHEN: Getting selected conversation view
    let view = state.selected_conversation_view();

    // THEN: Returns None gracefully
    assert!(view.is_none(), "Should return None when no session exists");
}

#[test]
fn selected_conversation_view_returns_none_when_tab_out_of_range() {
    // GIVEN: AppState with main + 1 subagent, selected_tab = 5 (out of range)
    let mut state = AppState::new();
    state.add_entries(vec![
        create_main_entry("main-1", "Main agent message"),
        create_subagent_entry("sub-a-1", "subagent-alpha", "Alpha message"),
    ]);
    state.selected_conversation =
        ConversationSelection::Subagent(AgentId::new("subagent-nonexistent").unwrap()); // Nonexistent subagent

    // WHEN: Getting selected conversation view
    let view = state.selected_conversation_view();

    // THEN: Returns None gracefully
    assert!(
        view.is_none(),
        "Should return None when tab index is out of range"
    );
}

// ===== Tests for selected_conversation_view_mut() =====

#[test]
fn selected_conversation_view_mut_tab0_routes_to_main() {
    // GIVEN: AppState with main agent entries and selected_tab = 0
    let mut state = AppState::new();
    state.add_entries(vec![
        create_main_entry("main-1", "Main agent message 1"),
        create_main_entry("main-2", "Main agent message 2"),
    ]);
    state.selected_conversation = ConversationSelection::Main;

    // WHEN: Getting mutable selected conversation view
    let view = state.selected_conversation_view_mut();

    // THEN: Returns mutable main conversation
    assert!(
        view.is_some(),
        "Should return mutable main conversation for tab 0"
    );
    let view = view.unwrap();
    assert_eq!(view.len(), 2, "Main conversation should have 2 entries");
}

#[test]
fn selected_conversation_view_mut_tab1_routes_to_first_subagent() {
    // GIVEN: AppState with main + 2 subagents, selected_tab = 1
    let mut state = AppState::new();
    state.add_entries(vec![
        create_main_entry("main-1", "Main agent message"),
        create_subagent_entry("sub-b-1", "subagent-bravo", "Bravo message"),
        create_subagent_entry("sub-a-1", "subagent-alpha", "Alpha message"),
    ]);
    state.selected_conversation =
        ConversationSelection::Subagent(AgentId::new("subagent-alpha").unwrap());

    // WHEN: Getting mutable selected conversation view
    let view = state.selected_conversation_view_mut();

    // THEN: Returns mutable first subagent (alpha)
    assert!(
        view.is_some(),
        "Should return mutable subagent conversation for tab 1"
    );
    let view = view.unwrap();
    assert_eq!(view.len(), 1, "First subagent (alpha) should have 1 entry");
}

// ===== Tests for selected_agent_id() =====

#[test]
fn selected_agent_id_tab0_returns_none() {
    // GIVEN: AppState with main agent, selected_tab = 0
    let mut state = AppState::new();
    state.add_entries(vec![create_main_entry("main-1", "Main agent message")]);
    state.selected_conversation = ConversationSelection::Main;

    // WHEN: Getting selected agent ID
    let agent_id = state.selected_agent_id();

    // THEN: Returns None (main agent has no AgentId)
    assert!(
        agent_id.is_none(),
        "Tab 0 (main agent) should return None for agent_id"
    );
}

#[test]
fn selected_agent_id_tab1_returns_first_subagent_id() {
    // GIVEN: AppState with main + 2 subagents, selected_tab = 1
    let mut state = AppState::new();
    state.add_entries(vec![
        create_main_entry("main-1", "Main agent message"),
        create_subagent_entry("sub-b-1", "subagent-bravo", "Bravo message"),
        create_subagent_entry("sub-a-1", "subagent-alpha", "Alpha message"),
    ]);
    state.selected_conversation =
        ConversationSelection::Subagent(AgentId::new("subagent-alpha").unwrap());

    // WHEN: Getting selected agent ID
    let agent_id = state.selected_agent_id();

    // THEN: Returns first subagent ID by sort (alpha)
    assert!(agent_id.is_some(), "Tab 1 should return Some(AgentId)");
    let agent_id = agent_id.unwrap();
    assert_eq!(
        agent_id.as_str(),
        "subagent-alpha",
        "First sorted subagent should be alpha"
    );
}

#[test]
fn selected_agent_id_multiple_subagents_correct_routing() {
    // GIVEN: AppState with main + 3 subagents
    let mut state = AppState::new();
    state.add_entries(vec![
        create_main_entry("main-1", "Main agent message"),
        create_subagent_entry("sub-c-1", "subagent-charlie", "Charlie 1"),
        create_subagent_entry("sub-a-1", "subagent-alpha", "Alpha 1"),
        create_subagent_entry("sub-b-1", "subagent-bravo", "Bravo 1"),
    ]);

    // THEN: Tab 0 -> None (main)
    state.selected_conversation = ConversationSelection::Main;
    assert!(
        state.selected_agent_id().is_none(),
        "Tab 0 should return None"
    );

    // THEN: Tab 1 -> alpha
    state.selected_conversation =
        ConversationSelection::Subagent(AgentId::new("subagent-alpha").unwrap());
    assert_eq!(
        state.selected_agent_id().unwrap().as_str(),
        "subagent-alpha",
        "Tab 1 should return alpha"
    );

    // THEN: Tab 2 -> bravo
    state.selected_conversation =
        ConversationSelection::Subagent(AgentId::new("subagent-bravo").unwrap());
    assert_eq!(
        state.selected_agent_id().unwrap().as_str(),
        "subagent-bravo",
        "Tab 2 should return bravo"
    );

    // THEN: Tab 3 -> charlie
    state.selected_conversation =
        ConversationSelection::Subagent(AgentId::new("subagent-charlie").unwrap());
    assert_eq!(
        state.selected_agent_id().unwrap().as_str(),
        "subagent-charlie",
        "Tab 3 should return charlie"
    );
}

#[test]
fn selected_agent_id_returns_none_when_no_session() {
    // GIVEN: Empty AppState (no sessions)
    let state = AppState::new();

    // WHEN: Getting selected agent ID
    let agent_id = state.selected_agent_id();

    // THEN: Returns None gracefully
    assert!(
        agent_id.is_none(),
        "Should return None when no session exists"
    );
}

#[test]
fn selected_agent_id_returns_agent_from_conversation_selection() {
    // GIVEN: AppState with ConversationSelection::Subagent(nonexistent agent)
    let mut state = AppState::new();
    state.add_entries(vec![
        create_main_entry("main-1", "Main agent message"),
        create_subagent_entry("sub-a-1", "subagent-alpha", "Alpha message"),
    ]);
    state.selected_conversation =
        ConversationSelection::Subagent(AgentId::new("subagent-nonexistent").unwrap());

    // WHEN: Getting selected agent ID
    let agent_id = state.selected_agent_id();

    // THEN: Returns the AgentId from ConversationSelection (doesn't validate existence)
    assert_eq!(
        agent_id,
        Some(AgentId::new("subagent-nonexistent").unwrap()),
        "selected_agent_id() returns AgentId from enum without validation"
    );
}

#[test]
fn selected_tab_index_returns_none_when_agent_not_in_session() {
    // GIVEN: AppState with ConversationSelection::Subagent(nonexistent agent)
    let mut state = AppState::new();
    state.add_entries(vec![
        create_main_entry("main-1", "Main agent message"),
        create_subagent_entry("sub-a-1", "subagent-alpha", "Alpha message"),
    ]);
    state.selected_conversation =
        ConversationSelection::Subagent(AgentId::new("subagent-nonexistent").unwrap());

    // WHEN: Getting selected tab index
    let tab_index = state.selected_tab_index();

    // THEN: Returns None when agent doesn't exist in current session
    assert!(
        tab_index.is_none(),
        "selected_tab_index() returns None when agent not found in session"
    );
}
