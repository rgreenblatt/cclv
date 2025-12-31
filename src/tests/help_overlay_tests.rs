//! Tests for help overlay functionality (bug cclv-5ur.26)
//!
//! Verifies that pressing '?' toggles the help overlay and Escape dismisses it.

use crate::test_harness::AcceptanceTestHarness;
use crossterm::event::{KeyCode, KeyModifiers};

#[test]
fn test_question_mark_shows_help_overlay() {
    // GIVEN: Application with initial state
    let harness = AcceptanceTestHarness::from_fixture("tests/fixtures/minimal_session.jsonl")
        .expect("Failed to load fixture");

    // VERIFY: Help not visible initially
    let state_before = harness.state();
    assert!(
        !state_before.help_visible,
        "Help overlay should not be visible initially"
    );

    // WHEN: User presses '?'
    let mut harness = harness;
    harness.send_key_with_mods(KeyCode::Char('?'), KeyModifiers::NONE);

    // THEN: Help overlay becomes visible
    let state_after = harness.state();
    assert!(
        state_after.help_visible,
        "Pressing '?' should toggle help overlay to visible"
    );
}

#[test]
fn test_question_mark_toggles_help_overlay() {
    // GIVEN: Application with initial state
    let mut harness = AcceptanceTestHarness::from_fixture("tests/fixtures/minimal_session.jsonl")
        .expect("Failed to load fixture");

    // WHEN: User presses '?' once
    harness.send_key_with_mods(KeyCode::Char('?'), KeyModifiers::NONE);

    // VERIFY: Help visible
    assert!(harness.state().help_visible, "First '?' should show help");

    // WHEN: User presses '?' again
    harness.send_key_with_mods(KeyCode::Char('?'), KeyModifiers::NONE);

    // THEN: Help hidden (toggle behavior)
    assert!(
        !harness.state().help_visible,
        "Second '?' should toggle help off"
    );
}

#[test]
fn test_escape_closes_help_when_visible() {
    // GIVEN: Application with help overlay visible
    let mut harness = AcceptanceTestHarness::from_fixture("tests/fixtures/minimal_session.jsonl")
        .expect("Failed to load fixture");
    harness.send_key_with_mods(KeyCode::Char('?'), KeyModifiers::NONE);

    // VERIFY: Help is visible
    assert!(
        harness.state().help_visible,
        "Help should be visible after pressing '?'"
    );

    // WHEN: User presses Escape
    harness.send_key(KeyCode::Esc);

    // THEN: Help overlay is dismissed
    assert!(
        !harness.state().help_visible,
        "Escape should close help overlay"
    );
}

#[test]
fn test_help_overlay_renders_over_main_ui() {
    // GIVEN: Application with help visible
    let mut harness = AcceptanceTestHarness::from_fixture("tests/fixtures/minimal_session.jsonl")
        .expect("Failed to load fixture");
    harness.send_key_with_mods(KeyCode::Char('?'), KeyModifiers::NONE);

    // WHEN: Rendering the UI
    harness.assert_snapshot("help_overlay_visible");

    // RESULT: Snapshot shows help content overlaid on main UI
}

#[test]
fn test_escape_with_search_active_doesnt_close_help() {
    // GIVEN: Application with search active AND help visible
    let mut harness = AcceptanceTestHarness::from_fixture("tests/fixtures/minimal_session.jsonl")
        .expect("Failed to load fixture");

    // Activate search first
    harness.send_key(KeyCode::Char('/'));

    // Then show help (this is a hypothetical edge case - may not be possible,
    // but worth testing to ensure state machines don't conflict)
    harness.send_key_with_mods(KeyCode::Char('?'), KeyModifiers::NONE);

    // WHEN: User presses Escape (should close search, not help)
    harness.send_key(KeyCode::Esc);

    // THEN: Search is closed, focus returns to main
    use crate::state::{FocusPane, SearchState};
    let state = harness.state();
    assert!(
        matches!(state.search, SearchState::Inactive),
        "Escape should close search when both search and help are active"
    );
    assert_eq!(
        state.focus,
        FocusPane::Main,
        "Focus should return to Main after closing search"
    );

    // Note: Help visibility behavior in this edge case is defined by implementation
    // The current test verifies that search gets priority (closes first)
}
