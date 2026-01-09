// ! Tests for stats filter keyboard wiring (cclv-463.5.5).
//!
//! BLOCKER fixes:
//! 1. Tab key should cycle stats filter when focus is on Stats pane
//! 2. Session change should call on_session_change() to update stats filter

use crate::config::keybindings::KeyBindings;
use crate::model::{AgentId, ConversationEntry, SessionId, StatsFilter};
use crate::parser;
use crate::source::InputSource;
use crate::state::{FocusPane, ViewedSession};
use crate::view::TuiApp;
use crate::view_state::types::SessionIndex;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::backend::TestBackend;
use ratatui::Terminal;

// ===== Test Helpers =====

/// Helper to create a test app with given session count.
fn create_test_app_with_sessions(session_count: usize) -> TuiApp<TestBackend> {
    let backend = TestBackend::new(80, 24);
    let terminal = Terminal::new(backend).expect("Failed to create terminal");
    let app_state = crate::state::AppState::new();

    // Create empty stdin source for testing
    let stdin_data = b"";
    let stdin_source = crate::source::StdinSource::from_reader(&stdin_data[..]);
    let input_source = InputSource::Stdin(stdin_source);

    let line_counter = 0;
    let key_bindings = KeyBindings::default();

    let mut app = TuiApp::new_for_test(
        terminal,
        app_state,
        input_source,
        line_counter,
        key_bindings,
    );

    // Create test sessions with entries
    for i in 0..session_count {
        let session_id =
            SessionId::new(format!("session-{}", i)).expect("Failed to create session ID");
        let json = format!(
            r#"{{"type":"user","message":{{"role":"user","content":"Test message"}},"session_id":"{}","uuid":"test-uuid-{}","timestamp":"2024-01-01T00:00:0{}Z"}}"#,
            session_id.as_str(),
            session_id.as_str(),
            i
        );
        let entry = ConversationEntry::from(parser::parse_entry_graceful(&json, i + 1));
        app.app_state.add_entries(vec![entry]);
    }

    app
}

// ===== ISSUE 1: Tab Key Should Cycle Stats Filter When Focus On Stats =====

#[test]
fn tab_key_cycles_stats_filter_when_stats_pane_focused() {
    // GIVEN: App with session and focus on Stats pane
    let mut app = create_test_app_with_sessions(1);
    app.app_state.focus = FocusPane::Stats;
    app.app_state.stats_filter = StatsFilter::AllSessionsCombined;

    // WHEN: User presses Tab
    let result = app.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));

    // THEN: Stats filter cycles to Session (not tab navigation)
    assert!(!result, "Tab should not quit the app");
    match &app.app_state.stats_filter {
        StatsFilter::Session(id) => {
            let expected_id = SessionId::new("session-0").unwrap();
            assert_eq!(
                id, &expected_id,
                "Filter should cycle to Session(session-0)"
            );
        }
        other => panic!("Expected Session filter, got {:?}", other),
    }
}

#[test]
fn tab_key_cycles_through_all_filter_levels() {
    // GIVEN: App with session that has subagents
    let mut app = create_test_app_with_sessions(1);
    let agent1 = AgentId::new("agent-1").unwrap();
    let agent2 = AgentId::new("agent-2").unwrap();

    // Directly add subagent entries to app_state for testing
    // This ensures proper routing through the log_view
    for (agent, idx) in [(agent1.clone(), 10), (agent2.clone(), 11)] {
        let json = format!(
            r#"{{"type":"user","message":{{"role":"user","content":"Subagent message"}},"session_id":"session-0","agentId":"{}","uuid":"test-uuid-sub-{}","timestamp":"2024-01-01T00:00:{}Z"}}"#,
            agent.as_str(),
            idx,
            idx
        );
        let entry = ConversationEntry::from(parser::parse_entry_graceful(&json, idx + 100));
        app.app_state.add_entries(vec![entry]);
    }

    // Ensure we're viewing the session with subagents
    app.app_state.viewed_session =
        ViewedSession::Pinned(SessionIndex::new(0, 1).expect("Valid session index"));
    app.app_state.focus = FocusPane::Stats;
    app.app_state.stats_filter = StatsFilter::AllSessionsCombined;

    // WHEN: Press Tab repeatedly
    // AllSessionsCombined → Session → MainAgent → Subagent(agent1) → Subagent(agent2) → AllSessionsCombined

    // Cycle 1: AllSessionsCombined → Session
    app.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    assert!(
        matches!(app.app_state.stats_filter, StatsFilter::Session(_)),
        "Should cycle to Session"
    );

    // Cycle 2: Session → MainAgent
    app.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    assert!(
        matches!(app.app_state.stats_filter, StatsFilter::MainAgent(_)),
        "Should cycle to MainAgent"
    );

    // Cycle 3: MainAgent → Subagent(agent1)
    app.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    match &app.app_state.stats_filter {
        StatsFilter::Subagent(id) => {
            assert_eq!(id, &agent1, "Should cycle to first subagent (agent-1)");
        }
        other => panic!("Expected Subagent(agent-1), got {:?}", other),
    }

    // Cycle 4: Subagent(agent1) → Subagent(agent2)
    app.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    match &app.app_state.stats_filter {
        StatsFilter::Subagent(id) => {
            assert_eq!(id, &agent2, "Should cycle to second subagent (agent-2)");
        }
        other => panic!("Expected Subagent(agent-2), got {:?}", other),
    }

    // Cycle 5: Subagent(agent2) → AllSessionsCombined (wrap around)
    app.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    assert_eq!(
        app.app_state.stats_filter,
        StatsFilter::AllSessionsCombined,
        "Should wrap back to AllSessionsCombined"
    );
}

#[test]
fn tab_key_does_tab_navigation_when_not_on_stats_pane() {
    // GIVEN: App with focus on Main pane
    let mut app = create_test_app_with_sessions(1);
    app.app_state.focus = FocusPane::Main;
    let original_filter = app.app_state.stats_filter.clone();

    // WHEN: User presses Tab
    app.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));

    // THEN: Stats filter is unchanged (Tab did pane cycling instead)
    assert_eq!(
        app.app_state.stats_filter, original_filter,
        "Stats filter should not change when focus is not on Stats"
    );
}

// ===== ISSUE 2: Session Change Should Update Stats Filter =====

#[test]
fn session_change_via_modal_updates_session_scoped_filter() {
    // GIVEN: App with 2 sessions, viewing session 0, filter is Session(session-0)
    let mut app = create_test_app_with_sessions(2);
    let session0_id = SessionId::new("session-0").unwrap();
    let session1_id = SessionId::new("session-1").unwrap();

    app.app_state.viewed_session =
        ViewedSession::Pinned(SessionIndex::new(0, 2).expect("Valid session index"));
    app.app_state.stats_filter = StatsFilter::Session(session0_id.clone());

    // Open session modal and select session 1
    app.app_state.session_modal.open(0);
    app.app_state.session_modal.select_next(2); // Move to session 1

    // WHEN: User confirms selection (Enter key)
    let result = crate::state::handle_session_modal_key(
        &mut app.app_state,
        KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
    );

    // THEN: Stats filter updates to Session(session-1)
    assert!(result, "Enter should be consumed by modal");
    match &app.app_state.stats_filter {
        StatsFilter::Session(id) => {
            assert_eq!(
                id, &session1_id,
                "Filter should update to Session(session-1) after session change"
            );
        }
        other => panic!("Expected Session(session-1), got {:?}", other),
    }
}

#[test]
fn session_change_updates_main_agent_filter() {
    // GIVEN: Filter is MainAgent(session-0)
    let mut app = create_test_app_with_sessions(2);
    let session0_id = SessionId::new("session-0").unwrap();
    let session1_id = SessionId::new("session-1").unwrap();

    app.app_state.viewed_session =
        ViewedSession::Pinned(SessionIndex::new(0, 2).expect("Valid session index"));
    app.app_state.stats_filter = StatsFilter::MainAgent(session0_id);

    // Open and change session
    app.app_state.session_modal.open(0);
    app.app_state.session_modal.select_next(2);

    // WHEN: Confirm selection
    crate::state::handle_session_modal_key(
        &mut app.app_state,
        KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
    );

    // THEN: MainAgent filter updates to new session
    match &app.app_state.stats_filter {
        StatsFilter::MainAgent(id) => {
            assert_eq!(
                id, &session1_id,
                "MainAgent filter should update to session-1"
            );
        }
        other => panic!("Expected MainAgent(session-1), got {:?}", other),
    }
}

#[test]
fn session_change_preserves_all_sessions_combined_filter() {
    // GIVEN: Filter is AllSessionsCombined
    let mut app = create_test_app_with_sessions(2);
    app.app_state.stats_filter = StatsFilter::AllSessionsCombined;
    app.app_state.viewed_session =
        ViewedSession::Pinned(SessionIndex::new(0, 2).expect("Valid session index"));

    // Open and change session
    app.app_state.session_modal.open(0);
    app.app_state.session_modal.select_next(2);

    // WHEN: Confirm selection
    crate::state::handle_session_modal_key(
        &mut app.app_state,
        KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
    );

    // THEN: AllSessionsCombined filter is unchanged
    assert_eq!(
        app.app_state.stats_filter,
        StatsFilter::AllSessionsCombined,
        "AllSessionsCombined should not change with session"
    );
}

#[test]
fn session_change_preserves_subagent_filter() {
    // GIVEN: Filter is Subagent(agent-1)
    let mut app = create_test_app_with_sessions(2);
    let agent1 = AgentId::new("agent-1").unwrap();
    app.app_state.stats_filter = StatsFilter::Subagent(agent1.clone());
    app.app_state.viewed_session =
        ViewedSession::Pinned(SessionIndex::new(0, 2).expect("Valid session index"));

    // Open and change session
    app.app_state.session_modal.open(0);
    app.app_state.session_modal.select_next(2);

    // WHEN: Confirm selection
    crate::state::handle_session_modal_key(
        &mut app.app_state,
        KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
    );

    // THEN: Subagent filter is unchanged (identity-based, not session-scoped)
    match &app.app_state.stats_filter {
        StatsFilter::Subagent(id) => {
            assert_eq!(
                id, &agent1,
                "Subagent filter should remain agent-1 (not session-scoped)"
            );
        }
        other => panic!("Expected Subagent(agent-1), got {:?}", other),
    }
}
