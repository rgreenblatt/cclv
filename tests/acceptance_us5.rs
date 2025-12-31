//! Acceptance tests for User Story 5: Search Within Conversations
//!
//! Tests the 4 acceptance scenarios from spec.md lines 128-131.
//! Each test verifies actual search behavior across conversations.

mod acceptance_harness;

use acceptance_harness::AcceptanceTestHarness;
use crossterm::event::KeyCode;

// ===== Test Fixtures =====

const MINIMAL_FIXTURE: &str = "tests/fixtures/minimal_session.jsonl";
const SEARCH_WITH_SUBAGENTS_FIXTURE: &str = "tests/fixtures/search_with_subagents.jsonl";

// ===== US5 Scenario 1: Search Highlights =====

#[test]
fn us5_scenario1_search_highlights() {
    // GIVEN: A loaded session
    // WHEN: User searches for a term
    // THEN: All occurrences are highlighted in the visible pane

    // DOING: Load session, activate search, type query, submit
    // EXPECT: SearchState transitions to Active with matches found
    let mut harness = AcceptanceTestHarness::from_fixture(MINIMAL_FIXTURE)
        .expect("Should load session for search highlights test");

    // IF YES: Session loaded
    let initial_state = harness.state();
    assert!(
        matches!(initial_state.search, cclv::state::SearchState::Inactive),
        "Search should start inactive"
    );

    // WHEN: User presses '/' to activate search
    harness.send_key(KeyCode::Char('/'));

    // VERIFY: Search enters typing mode
    let state_after_slash = harness.state();
    assert!(
        matches!(
            state_after_slash.search,
            cclv::state::SearchState::Typing { .. }
        ),
        "Pressing '/' should activate search input (Typing mode)"
    );

    // WHEN: User types a search query "help"
    // This word appears in minimal_session.jsonl: "Can you help?" and "I'd be happy to help."
    harness.send_key(KeyCode::Char('h'));
    harness.send_key(KeyCode::Char('e'));
    harness.send_key(KeyCode::Char('l'));
    harness.send_key(KeyCode::Char('p'));

    // WHEN: User presses Ctrl+S to submit search (Enter is bound to ToggleExpand, not SubmitSearch)
    harness.send_key_with_mods(KeyCode::Char('s'), crossterm::event::KeyModifiers::CONTROL);

    // VERIFY: Search is active with matches
    let state_after_search = harness.state();
    match &state_after_search.search {
        cclv::state::SearchState::Active { query, matches, .. } => {
            assert_eq!(
                query.as_str(),
                "help",
                "Query should be stored as 'help'"
            );
            assert!(
                !matches.is_empty(),
                "Search for 'help' should find matches in minimal_session.jsonl (appears in 'Can you help?' and 'happy to help')"
            );

            // VERIFY: All matches have location information
            for search_match in matches {
                assert!(
                    search_match.agent_id.is_none(),
                    "Minimal session has no subagents, so all matches should be in main agent (agent_id=None)"
                );
                assert!(
                    search_match.length > 0,
                    "Each match should have non-zero length"
                );
            }
        }
        _ => panic!(
            "Search should be active after Enter, got: {:?}",
            state_after_search.search
        ),
    }

    // RESULT: Search activated and found matches with full location info
    // MATCHES: Yes - SearchState::Active contains matches vector
    // THEREFORE: US5 Scenario 1 verified - matches are tracked for highlighting
}

// ===== US5 Scenario 2: Tab Indicators =====

#[test]
fn us5_scenario2_tab_indicators() {
    // GIVEN: Search is active
    // WHEN: Matches exist in subagent conversations
    // THEN: The subagent tabs indicate they contain matches (visual indicator)

    // DOING: Load session with subagents, search for term present in subagents
    // EXPECT: agent_ids_with_matches returns set of subagent IDs containing matches
    let mut harness = AcceptanceTestHarness::from_fixture(SEARCH_WITH_SUBAGENTS_FIXTURE)
        .expect("Should load session for tab indicators test");

    // IF YES: Session loaded with subagents
    let initial_state = harness.state();
    let num_subagents = initial_state.session().subagents().len();
    assert!(
        num_subagents > 0,
        "search_with_subagents.jsonl should contain subagents"
    );

    // WHEN: User activates search and searches for "Implemented"
    // This word appears in subagent messages in search_with_subagents.jsonl
    harness.send_key(KeyCode::Char('/'));
    harness.send_key(KeyCode::Char('I'));
    harness.send_key(KeyCode::Char('m'));
    harness.send_key(KeyCode::Char('p'));
    harness.send_key(KeyCode::Char('l'));
    harness.send_key(KeyCode::Char('e'));
    harness.send_key(KeyCode::Char('m'));
    harness.send_key(KeyCode::Char('e'));
    harness.send_key(KeyCode::Char('n'));
    harness.send_key(KeyCode::Char('t'));
    harness.send_key(KeyCode::Char('e'));
    harness.send_key(KeyCode::Char('d'));

    // WHEN: User submits search with Ctrl+S
    harness.send_key_with_mods(KeyCode::Char('s'), crossterm::event::KeyModifiers::CONTROL);

    // VERIFY: Search is active with matches in subagents
    let state_after_search = harness.state();
    match &state_after_search.search {
        cclv::state::SearchState::Active { matches, .. } => {
            assert!(
                !matches.is_empty(),
                "Search for 'Implemented' should find matches in search_with_subagents.jsonl"
            );

            // VERIFY: Extract agent IDs containing matches for tab indicators
            let agent_ids_with_matches = cclv::state::agent_ids_with_matches(matches);

            assert!(
                !agent_ids_with_matches.is_empty(),
                "At least one subagent should contain matches for 'Implemented'"
            );

            // VERIFY: Each agent ID in the set is a valid subagent
            for agent_id in &agent_ids_with_matches {
                assert!(
                    state_after_search
                        .session()
                        .subagents()
                        .iter()
                        .any(|(id, _)| id == agent_id),
                    "Agent ID {:?} should be a valid subagent in the session",
                    agent_id
                );
            }
        }
        _ => panic!(
            "Search should be active after Enter, got: {:?}",
            state_after_search.search
        ),
    }

    // RESULT: agent_ids_with_matches provides set of subagent IDs for tab indicators
    // MATCHES: Yes - function returns HashSet of AgentId for tabs with matches
    // THEREFORE: US5 Scenario 2 verified - view layer can use this to display indicators
}

// ===== US5 Scenario 3: Navigate to Match =====

#[test]
fn us5_scenario3_navigate_to_match() {
    // GIVEN: Search results exist
    // WHEN: User navigates to a match in a subagent tab
    // THEN: That tab is automatically activated and scrolled to the match

    // DOING: Load session with subagents, search, navigate to subagent match with 'n'
    // EXPECT: selected_tab changes and scroll position updates when match is in different tab
    let mut harness = AcceptanceTestHarness::from_fixture(SEARCH_WITH_SUBAGENTS_FIXTURE)
        .expect("Should load session for match navigation test");

    // IF YES: Session loaded with subagents
    // WHEN: User searches for "Implemented" (present in subagent messages)
    harness.send_key(KeyCode::Char('/'));
    harness.send_key(KeyCode::Char('I'));
    harness.send_key(KeyCode::Char('m'));
    harness.send_key(KeyCode::Char('p'));
    harness.send_key(KeyCode::Char('l'));
    harness.send_key(KeyCode::Char('e'));
    harness.send_key(KeyCode::Char('m'));
    harness.send_key(KeyCode::Char('e'));
    harness.send_key(KeyCode::Char('n'));
    harness.send_key(KeyCode::Char('t'));
    harness.send_key(KeyCode::Char('e'));
    harness.send_key(KeyCode::Char('d'));

    // WHEN: User submits search with Ctrl+S
    harness.send_key_with_mods(KeyCode::Char('s'), crossterm::event::KeyModifiers::CONTROL);

    // VERIFY: Search is active with matches
    let state_after_search = harness.state();
    match &state_after_search.search {
        cclv::state::SearchState::Active {
            matches,
            current_match,
            ..
        } => {
            assert!(
                !matches.is_empty(),
                "Should have matches for 'Implemented'"
            );

            let initial_match_index = *current_match;
            let _initial_selected_tab = state_after_search.selected_tab;
            let _initial_focus = state_after_search.focus;

            // WHEN: User presses 'n' to navigate to next match
            // This should potentially switch tabs if next match is in a different agent
            harness.send_key(KeyCode::Char('n'));

            // VERIFY: Current match advanced
            let state_after_n = harness.state();
            match &state_after_n.search {
                cclv::state::SearchState::Active {
                    current_match: new_match_index,
                    matches: new_matches,
                    ..
                } => {
                    // Verify match navigation occurred (with wraparound)
                    let expected_new_index = (initial_match_index + 1) % new_matches.len();
                    assert_eq!(
                        *new_match_index, expected_new_index,
                        "Pressing 'n' should advance to next match (with wraparound)"
                    );

                    // VERIFY: Tab and focus auto-switch based on match location
                    let current_match = &new_matches[*new_match_index];
                    if let Some(ref agent_id) = current_match.agent_id {
                        // Match is in a subagent - verify focus switched to Subagent pane
                        assert_eq!(
                            state_after_n.focus,
                            cclv::state::FocusPane::Subagent,
                            "Focus should auto-switch to Subagent pane when match is in subagent"
                        );

                        // Verify the correct tab is selected
                        let expected_tab_index = state_after_n
                            .session()
                            .subagents()
                            .iter()
                            .position(|(id, _)| id == agent_id)
                            .expect("Match agent_id should be a valid subagent");

                        assert_eq!(
                            state_after_n.selected_tab,
                            Some(expected_tab_index),
                            "Tab should auto-switch to the subagent containing the match"
                        );
                    } else {
                        // Match is in main agent - verify focus is on Main pane
                        assert_eq!(
                            state_after_n.focus,
                            cclv::state::FocusPane::Main,
                            "Focus should be on Main pane when match is in main agent"
                        );
                    }
                }
                _ => panic!("Search should remain active after 'n'"),
            }

            // VERIFY: Can navigate backward with 'N'
            harness.send_key(KeyCode::Char('N'));

            let state_after_shift_n = harness.state();
            match &state_after_shift_n.search {
                cclv::state::SearchState::Active {
                    current_match: final_match_index,
                    ..
                } => {
                    assert_eq!(
                        *final_match_index, initial_match_index,
                        "Pressing 'N' should move back to initial match"
                    );
                }
                _ => panic!("Search should remain active after 'N'"),
            }
        }
        _ => panic!(
            "Search should be active after Enter, got: {:?}",
            state_after_search.search
        ),
    }

    // RESULT: n/N navigate through matches; tab and focus auto-switch to match location
    // MATCHES: Yes - current_match changes, focus/tab switch to correct location
    // THEREFORE: US5 Scenario 3 verified - tab auto-activation works correctly
}

// ===== US5 Scenario 4: Clear Search =====

#[test]
fn us5_scenario4_clear_search() {
    // GIVEN: Search is active
    // WHEN: User clears the search
    // THEN: All highlighting is removed

    // DOING: Load session, execute search, press Esc to clear
    // EXPECT: SearchState transitions back to Inactive
    let mut harness = AcceptanceTestHarness::from_fixture(MINIMAL_FIXTURE)
        .expect("Should load session for clear search test");

    // IF YES: Session loaded
    // WHEN: User activates search and types query
    harness.send_key(KeyCode::Char('/'));
    harness.send_key(KeyCode::Char('h'));
    harness.send_key(KeyCode::Char('e'));
    harness.send_key(KeyCode::Char('l'));
    harness.send_key(KeyCode::Char('p'));

    // WHEN: User submits search with Ctrl+S
    harness.send_key_with_mods(KeyCode::Char('s'), crossterm::event::KeyModifiers::CONTROL);

    // VERIFY: Search is active
    let state_with_search = harness.state();
    assert!(
        matches!(
            state_with_search.search,
            cclv::state::SearchState::Active { .. }
        ),
        "Search should be active before clearing"
    );

    // WHEN: User presses Esc to clear search
    harness.send_key(KeyCode::Esc);

    // VERIFY: Search returns to inactive
    let state_after_clear = harness.state();
    assert!(
        matches!(
            state_after_clear.search,
            cclv::state::SearchState::Inactive
        ),
        "Pressing Esc should clear search and return to Inactive state"
    );

    // VERIFY: Can also cancel search from Typing mode
    // WHEN: User activates search again
    harness.send_key(KeyCode::Char('/'));

    let state_typing = harness.state();
    assert!(
        matches!(
            state_typing.search,
            cclv::state::SearchState::Typing { .. }
        ),
        "Search should be in Typing mode"
    );

    // WHEN: User presses Esc to cancel
    harness.send_key(KeyCode::Esc);

    // VERIFY: Search cancelled back to inactive
    let state_after_cancel = harness.state();
    assert!(
        matches!(
            state_after_cancel.search,
            cclv::state::SearchState::Inactive
        ),
        "Pressing Esc in Typing mode should cancel search"
    );

    // RESULT: Esc clears search from both Active and Typing states
    // MATCHES: Yes - SearchState transitions to Inactive
    // THEREFORE: US5 Scenario 4 verified - search can be cleared to remove highlights
}
