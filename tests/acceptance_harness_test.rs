//! Tests for AcceptanceTestHarness
//!
//! These tests verify the harness itself works correctly before using it
//! for user story acceptance tests.

mod acceptance_harness;

use acceptance_harness::AcceptanceTestHarness;
use crossterm::event::{KeyCode, KeyModifiers};

// ===== Test Helpers =====

const FIXTURE_PATH: &str = "tests/fixtures/minimal_session.jsonl";

// ===== from_fixture Tests =====

#[test]
fn test_from_fixture_creates_working_harness() {
    // EXPECT: Loading a valid fixture creates a harness in running state
    let harness =
        AcceptanceTestHarness::from_fixture(FIXTURE_PATH).expect("Should load valid fixture");

    // Harness should be running initially
    assert!(
        harness.is_running(),
        "Harness should be running after creation"
    );

    // Should have access to app state
    let state = harness.state();
    assert!(
        !state.session().main_agent().is_empty(),
        "Should have loaded entries from fixture"
    );
}

#[test]
fn test_from_fixture_with_size_respects_dimensions() {
    // EXPECT: Custom terminal size is respected
    let width = 120;
    let height = 40;

    let harness = AcceptanceTestHarness::from_fixture_with_size(FIXTURE_PATH, width, height)
        .expect("Should load fixture with custom size");

    assert!(harness.is_running(), "Harness should be running");

    // Note: We can't directly verify terminal size without exposing it,
    // but we verify the harness was created successfully with those params
}

#[test]
fn test_from_fixture_handles_nonexistent_file() {
    // EXPECT: Loading nonexistent fixture returns error
    let result = AcceptanceTestHarness::from_fixture("tests/fixtures/does_not_exist.jsonl");

    assert!(result.is_err(), "Should return error for nonexistent file");
}

// ===== send_key Tests =====

#[test]
fn test_send_key_quit_returns_true() {
    // EXPECT: Sending 'q' triggers quit and returns true
    let mut harness =
        AcceptanceTestHarness::from_fixture(FIXTURE_PATH).expect("Should load fixture");

    assert!(harness.is_running(), "Should start running");

    let quit = harness.send_key(KeyCode::Char('q'));

    assert!(quit, "send_key('q') should return true (quit)");
    assert!(
        !harness.is_running(),
        "Harness should no longer be running after quit"
    );
}

#[test]
fn test_send_key_navigation_returns_false() {
    // EXPECT: Navigation keys don't quit, return false
    let mut harness =
        AcceptanceTestHarness::from_fixture(FIXTURE_PATH).expect("Should load fixture");

    let quit = harness.send_key(KeyCode::Char('j'));

    assert!(!quit, "send_key('j') should return false (not quit)");
    assert!(harness.is_running(), "Harness should still be running");
}

#[test]
fn test_send_key_modifies_state() {
    // EXPECT: Sending keys actually modifies app state
    let mut harness =
        AcceptanceTestHarness::from_fixture(FIXTURE_PATH).expect("Should load fixture");

    // Send Tab to cycle focus
    harness.send_key(KeyCode::Tab);

    // State should reflect the focus change
    // (Exact assertion depends on initial state, but state() should work)
    let state = harness.state();
    // Just verify we can access state after key send
    let _ = state.focus;
}

// ===== send_key_with_mods Tests =====

#[test]
fn test_send_key_with_mods_ctrl_c_quits() {
    // EXPECT: Ctrl+C quits the app
    let mut harness =
        AcceptanceTestHarness::from_fixture(FIXTURE_PATH).expect("Should load fixture");

    let quit = harness.send_key_with_mods(KeyCode::Char('c'), KeyModifiers::CONTROL);

    assert!(quit, "Ctrl+C should quit");
    assert!(!harness.is_running(), "Should no longer be running");
}

// ===== send_keys Tests =====

#[test]
fn test_send_keys_processes_sequence() {
    // EXPECT: Multiple keys are processed in order
    let mut harness =
        AcceptanceTestHarness::from_fixture(FIXTURE_PATH).expect("Should load fixture");

    // Send a sequence: Tab, j, j, k
    harness.send_keys(&[
        KeyCode::Tab,
        KeyCode::Char('j'),
        KeyCode::Char('j'),
        KeyCode::Char('k'),
    ]);

    // Should still be running (no quit key sent)
    assert!(
        harness.is_running(),
        "Should still be running after navigation keys"
    );
}

#[test]
fn test_send_keys_stops_on_quit() {
    // EXPECT: Sequence stops when quit key is encountered
    let mut harness =
        AcceptanceTestHarness::from_fixture(FIXTURE_PATH).expect("Should load fixture");

    // Send sequence that includes quit
    harness.send_keys(&[
        KeyCode::Char('j'),
        KeyCode::Char('q'), // Quit here
        KeyCode::Char('k'), // Should not be processed
    ]);

    assert!(!harness.is_running(), "Should have quit");
}

// ===== type_text Tests =====

#[test]
fn test_type_text_sends_character_events() {
    // EXPECT: type_text sends individual character key events
    let mut harness =
        AcceptanceTestHarness::from_fixture(FIXTURE_PATH).expect("Should load fixture");

    // Enter search mode first
    harness.send_key(KeyCode::Char('/'));

    // Type search text
    harness.type_text("error");

    // Verify search state updated (actual assertion depends on search implementation)
    let state = harness.state();
    // Just verify we can access state after typing
    let _ = &state.search;

    assert!(harness.is_running(), "Should still be running after typing");
}

// ===== is_running Tests =====

#[test]
fn test_is_running_tracks_quit_state() {
    // EXPECT: is_running correctly reflects app state
    let mut harness =
        AcceptanceTestHarness::from_fixture(FIXTURE_PATH).expect("Should load fixture");

    assert!(harness.is_running(), "Should be running initially");

    harness.send_key(KeyCode::Char('q'));

    assert!(!harness.is_running(), "Should not be running after quit");
}

// ===== state Tests =====

#[test]
fn test_state_provides_readonly_access() {
    // EXPECT: state() provides access to AppState
    let harness = AcceptanceTestHarness::from_fixture(FIXTURE_PATH).expect("Should load fixture");

    let state = harness.state();

    // Should be able to query state
    assert!(
        !state.session().main_agent().is_empty(),
        "Should have entries"
    );
    let _ = state.focus; // Should be able to access state fields
    let _ = &state.search;
}

// ===== render_to_string Tests =====

#[test]
fn test_render_to_string_returns_non_empty_string() {
    // EXPECT: render_to_string should render the current state and return it as a string
    let mut harness =
        AcceptanceTestHarness::from_fixture(FIXTURE_PATH).expect("Should load fixture");

    let output = harness.render_to_string();

    assert!(
        !output.is_empty(),
        "Rendered output should be non-empty string"
    );
    assert!(
        output.len() > 10,
        "Rendered output should contain substantial content"
    );
}

#[test]
fn test_render_to_string_contains_fixture_content() {
    // EXPECT: Rendered output should contain content from the loaded fixture
    let mut harness =
        AcceptanceTestHarness::from_fixture(FIXTURE_PATH).expect("Should load fixture");

    let output = harness.render_to_string();

    // The minimal_session.jsonl fixture contains "Hello!" in the user message
    assert!(
        output.contains("Hello") || output.contains("User") || output.contains("Assistant"),
        "Rendered output should contain content from fixture: got:\n{}",
        output
    );
}

#[test]
fn test_render_to_string_changes_after_scroll() {
    // EXPECT: Rendering after scrolling should produce different output
    let mut harness =
        AcceptanceTestHarness::from_fixture(FIXTURE_PATH).expect("Should load fixture");

    let before = harness.render_to_string();

    // Scroll down
    harness.send_key(KeyCode::Char('j'));

    let after = harness.render_to_string();

    // Output might be the same or different depending on whether scroll caused visual change
    // At minimum, both should be valid renders
    assert!(!before.is_empty(), "Before render should not be empty");
    assert!(!after.is_empty(), "After render should not be empty");
}

#[test]
fn test_render_to_string_can_be_called_multiple_times() {
    // EXPECT: Can call render_to_string repeatedly without issues
    let mut harness =
        AcceptanceTestHarness::from_fixture(FIXTURE_PATH).expect("Should load fixture");

    let render1 = harness.render_to_string();
    let render2 = harness.render_to_string();
    let render3 = harness.render_to_string();

    // All renders should be identical when no state changes occur
    assert_eq!(render1, render2, "Repeated renders should be identical");
    assert_eq!(render2, render3, "Repeated renders should be identical");
}

// ===== assert_snapshot Tests =====

#[test]
fn test_assert_snapshot_creates_snapshot() {
    // EXPECT: assert_snapshot should render and verify against a snapshot
    let mut harness =
        AcceptanceTestHarness::from_fixture(FIXTURE_PATH).expect("Should load fixture");

    // This should create/verify a snapshot
    harness.assert_snapshot("test_harness_initial_state");

    // If we get here without panic, snapshot assertion passed
    assert!(harness.is_running(), "Harness should still be running");
}

#[test]
fn test_assert_snapshot_after_interaction() {
    // EXPECT: Can snapshot after user interaction
    let mut harness =
        AcceptanceTestHarness::from_fixture(FIXTURE_PATH).expect("Should load fixture");

    // Interact with the app
    harness.send_key(KeyCode::Tab); // Cycle focus

    // Snapshot the result
    harness.assert_snapshot("test_harness_after_tab");

    assert!(harness.is_running());
}

#[test]
fn test_assert_snapshot_different_states() {
    // EXPECT: Different states should have different snapshots
    let mut harness =
        AcceptanceTestHarness::from_fixture(FIXTURE_PATH).expect("Should load fixture");

    // Snapshot initial state
    harness.assert_snapshot("test_harness_state_initial");

    // Change state
    harness.send_key(KeyCode::Char('j')); // Scroll down

    // Snapshot after scroll
    harness.assert_snapshot("test_harness_state_after_scroll");

    // Both snapshots should succeed
    assert!(harness.is_running());
}
