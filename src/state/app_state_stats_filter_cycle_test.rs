//! Tests for stats filter cycling with session context (cclv-463.5.5).
//!
//! Tests the cycle_stats_filter and on_session_change methods.

use super::*;
use crate::model::{AgentId, ConversationEntry, SessionId, StatsFilter};
use crate::parser;

// ===== Test Helpers =====

/// Helper to add a test session with entries to the state.
fn add_test_session(state: &mut AppState, session_id: SessionId) {
    // Create a minimal user message entry for the session (main agent - no agentId field)
    let json = format!(
        r#"{{"type":"user","message":{{"role":"user","content":"Test message"}},"session_id":"{}","uuid":"test-uuid-{}","timestamp":"2024-01-01T00:00:00Z"}}"#,
        session_id.as_str(),
        session_id.as_str()
    );
    let entry = ConversationEntry::from(parser::parse_entry_graceful(&json, 1));
    state.add_entries(vec![entry]);
}

/// Helper to add a test session with subagents.
fn add_test_session_with_subagents(
    state: &mut AppState,
    session_id: SessionId,
    subagent_ids: Vec<AgentId>,
) {
    // Add main session entry
    add_test_session(state, session_id.clone());

    // Add entries for each subagent
    for (i, agent_id) in subagent_ids.iter().enumerate() {
        let json = format!(
            r#"{{"type":"user","message":{{"role":"user","content":"Subagent message"}},"session_id":"{}","agentId":"{}","uuid":"test-uuid-sub-{}","timestamp":"2024-01-01T00:00:0{}Z"}}"#,
            session_id.as_str(),
            agent_id.as_str(),
            i,
            i
        );
        let entry = ConversationEntry::from(parser::parse_entry_graceful(&json, i + 2));
        state.add_entries(vec![entry]);
    }
}

// ===== cycle_stats_filter Tests =====

#[test]
fn cycle_from_all_sessions_combined_to_session() {
    // Create app state with test data
    let mut state = AppState::new();

    // Add a session with some data
    let session_id = SessionId::new("test-session-1").expect("valid session id");
    add_test_session(&mut state, session_id.clone());

    // Start with AllSessionsCombined filter
    state.stats_filter = StatsFilter::AllSessionsCombined;

    // Cycle stats filter
    state.cycle_stats_filter();

    // Should transition to Session(current_session)
    assert_eq!(
        state.stats_filter,
        StatsFilter::Session(session_id),
        "Should cycle from AllSessionsCombined to Session"
    );
}

#[test]
fn cycle_from_session_to_main_agent() {
    let mut state = AppState::new();
    let session_id = SessionId::new("test-session-2").expect("valid session id");
    add_test_session(&mut state, session_id.clone());

    // Start with Session filter
    state.stats_filter = StatsFilter::Session(session_id.clone());

    // Cycle stats filter
    state.cycle_stats_filter();

    // Should transition to MainAgent(current_session)
    assert_eq!(
        state.stats_filter,
        StatsFilter::MainAgent(session_id),
        "Should cycle from Session to MainAgent"
    );
}

#[test]
fn cycle_from_main_agent_to_first_subagent() {
    let mut state = AppState::new();
    let session_id = SessionId::new("test-session-3").expect("valid session id");
    let subagent_id = AgentId::new("subagent-alpha").expect("valid agent id");

    add_test_session_with_subagents(&mut state, session_id.clone(), vec![subagent_id.clone()]);

    // Start with MainAgent filter
    state.stats_filter = StatsFilter::MainAgent(session_id);

    // Cycle stats filter
    state.cycle_stats_filter();

    // Should transition to Subagent(first)
    assert_eq!(
        state.stats_filter,
        StatsFilter::Subagent(subagent_id),
        "Should cycle from MainAgent to first subagent"
    );
}

#[test]
fn cycle_from_main_agent_to_all_sessions_when_no_subagents() {
    let mut state = AppState::new();
    let session_id = SessionId::new("test-session-4").expect("valid session id");
    add_test_session(&mut state, session_id.clone());

    // Start with MainAgent filter (no subagents)
    state.stats_filter = StatsFilter::MainAgent(session_id);

    // Cycle stats filter
    state.cycle_stats_filter();

    // Should wrap back to AllSessionsCombined (no subagents)
    assert_eq!(
        state.stats_filter,
        StatsFilter::AllSessionsCombined,
        "Should cycle from MainAgent to AllSessionsCombined when no subagents"
    );
}

#[test]
fn cycle_from_subagent_to_next_subagent() {
    let mut state = AppState::new();
    let session_id = SessionId::new("test-session-5").expect("valid session id");
    let sub1 = AgentId::new("subagent-1").expect("valid agent id");
    let sub2 = AgentId::new("subagent-2").expect("valid agent id");

    add_test_session_with_subagents(
        &mut state,
        session_id.clone(),
        vec![sub1.clone(), sub2.clone()],
    );

    // Start with first subagent
    state.stats_filter = StatsFilter::Subagent(sub1);

    // Cycle stats filter
    state.cycle_stats_filter();

    // Should transition to next subagent
    assert_eq!(
        state.stats_filter,
        StatsFilter::Subagent(sub2),
        "Should cycle from first subagent to second subagent"
    );
}

#[test]
fn cycle_from_last_subagent_to_all_sessions() {
    let mut state = AppState::new();
    let session_id = SessionId::new("test-session-6").expect("valid session id");
    let sub1 = AgentId::new("subagent-1").expect("valid agent id");
    let sub2 = AgentId::new("subagent-2").expect("valid agent id");

    add_test_session_with_subagents(&mut state, session_id.clone(), vec![sub1, sub2.clone()]);

    // Start with last subagent
    state.stats_filter = StatsFilter::Subagent(sub2);

    // Cycle stats filter
    state.cycle_stats_filter();

    // Should wrap back to AllSessionsCombined
    assert_eq!(
        state.stats_filter,
        StatsFilter::AllSessionsCombined,
        "Should cycle from last subagent to AllSessionsCombined"
    );
}

#[test]
fn cycle_from_unknown_subagent_to_all_sessions() {
    let mut state = AppState::new();
    let session_id = SessionId::new("test-session-7").expect("valid session id");
    let sub1 = AgentId::new("subagent-1").expect("valid agent id");
    let unknown_sub = AgentId::new("unknown-subagent").expect("valid agent id");

    add_test_session_with_subagents(&mut state, session_id.clone(), vec![sub1]);

    // Start with unknown subagent (not in current session)
    state.stats_filter = StatsFilter::Subagent(unknown_sub);

    // Cycle stats filter
    state.cycle_stats_filter();

    // Should wrap back to AllSessionsCombined
    assert_eq!(
        state.stats_filter,
        StatsFilter::AllSessionsCombined,
        "Should cycle from unknown subagent to AllSessionsCombined"
    );
}

// ===== on_session_change Tests =====

#[test]
fn on_session_change_preserves_all_sessions_combined() {
    let mut state = AppState::new();
    let old_session = SessionId::new("session-old").expect("valid session id");
    let new_session = SessionId::new("session-new").expect("valid session id");

    add_test_session(&mut state, old_session.clone());
    add_test_session(&mut state, new_session.clone());

    // Set filter to AllSessionsCombined
    state.stats_filter = StatsFilter::AllSessionsCombined;

    // Change session
    state.on_session_change(new_session);

    // Should remain AllSessionsCombined
    assert_eq!(
        state.stats_filter,
        StatsFilter::AllSessionsCombined,
        "AllSessionsCombined filter should not change on session change"
    );
}

#[test]
fn on_session_change_updates_session_filter() {
    let mut state = AppState::new();
    let old_session = SessionId::new("session-old").expect("valid session id");
    let new_session = SessionId::new("session-new").expect("valid session id");

    add_test_session(&mut state, old_session.clone());
    add_test_session(&mut state, new_session.clone());

    // Set filter to Session(old)
    state.stats_filter = StatsFilter::Session(old_session);

    // Change session
    state.on_session_change(new_session.clone());

    // Should update to Session(new)
    assert_eq!(
        state.stats_filter,
        StatsFilter::Session(new_session),
        "Session filter should update to new session ID"
    );
}

#[test]
fn on_session_change_updates_main_agent_filter() {
    let mut state = AppState::new();
    let old_session = SessionId::new("session-old").expect("valid session id");
    let new_session = SessionId::new("session-new").expect("valid session id");

    add_test_session(&mut state, old_session.clone());
    add_test_session(&mut state, new_session.clone());

    // Set filter to MainAgent(old)
    state.stats_filter = StatsFilter::MainAgent(old_session);

    // Change session
    state.on_session_change(new_session.clone());

    // Should update to MainAgent(new)
    assert_eq!(
        state.stats_filter,
        StatsFilter::MainAgent(new_session),
        "MainAgent filter should update to new session ID"
    );
}

#[test]
fn on_session_change_preserves_subagent_filter() {
    let mut state = AppState::new();
    let old_session = SessionId::new("session-old").expect("valid session id");
    let new_session = SessionId::new("session-new").expect("valid session id");
    let subagent = AgentId::new("subagent-123").expect("valid agent id");

    add_test_session_with_subagents(&mut state, old_session.clone(), vec![subagent.clone()]);
    add_test_session_with_subagents(&mut state, new_session.clone(), vec![subagent.clone()]);

    // Set filter to Subagent
    state.stats_filter = StatsFilter::Subagent(subagent.clone());

    // Change session
    state.on_session_change(new_session);

    // Should keep same subagent filter (identity-based, not session-scoped)
    assert_eq!(
        state.stats_filter,
        StatsFilter::Subagent(subagent),
        "Subagent filter should preserve same agent ID"
    );
}
