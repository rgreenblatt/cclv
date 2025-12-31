//! Crash Regression Tests
//!
//! These tests are designed to catch crash bugs in the TUI application,
//! particularly the known scroll crash bug (cclv-31l.3). These tests may
//! initially FAIL until the underlying bugs are fixed - that's expected
//! and correct behavior for regression tests.
//!
//! Each test verifies that the app remains running after potentially
//! crash-inducing operations.

use crate::test_harness::AcceptanceTestHarness;
use crossterm::event::KeyCode;

// ===== Test Fixtures =====

const MINIMAL_SESSION: &str = "tests/fixtures/minimal_session.jsonl";
const WITH_SUBAGENTS: &str = "tests/fixtures/with_subagents.jsonl";
const LARGE_FIXTURE: &str = "tests/fixtures/cc-session-log.jsonl";

// ===== Crash Regression Tests =====

/// Test: Scrolling down many times should not crash
///
/// This test exposes the known scroll crash bug (cclv-31l.3) where
/// scrolling down 4-5 times in a real session log causes a crash.
///
/// EXPECT: App remains running after 20 scroll-down operations
/// ACTUAL: May fail due to known bug - test will pass once bug is fixed
#[test]
fn crash_scroll_down_many_times() {
    // DOING: Load fixture and scroll down 20 times
    // EXPECT: App should remain running throughout

    let mut harness = AcceptanceTestHarness::from_fixture(LARGE_FIXTURE)
        .expect("Should load large fixture to test scroll crash bug");

    // Verify initial state
    assert!(
        harness.is_running(),
        "Harness should start in running state"
    );

    // Scroll down many times (using 'j' key, vim-style navigation)
    for i in 0..20 {
        harness.send_key(KeyCode::Char('j'));

        // CRITICAL: Verify app is still running after each scroll
        // This catches the crash at the exact iteration it occurs
        assert!(
            harness.is_running(),
            "App crashed on scroll iteration {} - this is the known bug cclv-31l.3",
            i + 1
        );
    }

    // VERIFY: App should still be running after all scrolls
    assert!(
        harness.is_running(),
        "App should remain running after 20 scroll-down operations"
    );
}

/// Test: Scrolling up past the top should not crash
///
/// Tests bounds checking when attempting to scroll above the first entry.
///
/// EXPECT: App handles scroll-up at top boundary gracefully
#[test]
fn crash_scroll_up_past_top() {
    // DOING: Load fixture and attempt to scroll up from top
    // EXPECT: App remains running, scroll position stays at 0

    let mut harness = AcceptanceTestHarness::from_fixture(MINIMAL_SESSION)
        .expect("Should load minimal session fixture");

    assert!(harness.is_running(), "Should start running");

    // Ensure we're at the top (send PageUp first, then spam 'k')
    harness.send_key(KeyCode::PageUp);
    assert!(harness.is_running(), "Should survive PageUp from top");

    // Try to scroll up past the top multiple times
    for i in 0..10 {
        harness.send_key(KeyCode::Char('k'));

        assert!(
            harness.is_running(),
            "App crashed on scroll-up-past-top iteration {} - bounds check failed",
            i + 1
        );
    }

    // VERIFY: App should still be running
    assert!(
        harness.is_running(),
        "App should remain running after attempting to scroll past top"
    );
}

/// Test: Rapid tab switching should not crash
///
/// Tests stability when rapidly switching between subagent tabs.
///
/// EXPECT: App handles rapid tab navigation without crashing
#[test]
fn crash_rapid_tab_switching() {
    // DOING: Load fixture with subagents and rapidly switch tabs
    // EXPECT: App remains stable during rapid navigation

    let mut harness = AcceptanceTestHarness::from_fixture(WITH_SUBAGENTS)
        .expect("Should load fixture with subagents");

    assert!(harness.is_running(), "Should start running");

    // Rapidly switch between tabs using arrow keys
    // This tests for race conditions, index out of bounds, etc.
    for i in 0..50 {
        // Alternate between next and previous tab
        let key = if i % 2 == 0 {
            KeyCode::Right // Next tab
        } else {
            KeyCode::Left // Previous tab
        };

        harness.send_key(key);

        assert!(
            harness.is_running(),
            "App crashed during rapid tab switching at iteration {} - possible race condition or index error",
            i + 1
        );
    }

    // VERIFY: App should still be running
    assert!(
        harness.is_running(),
        "App should remain running after 50 rapid tab switches"
    );
}

/// Test: Search with empty results should not crash
///
/// Tests that searching for non-existent terms handles zero matches gracefully.
///
/// EXPECT: App handles search with no matches without crashing
#[test]
fn crash_search_empty_results() {
    // DOING: Search for term that doesn't exist and navigate matches
    // EXPECT: App handles zero matches without panic

    let mut harness = AcceptanceTestHarness::from_fixture(MINIMAL_SESSION)
        .expect("Should load minimal session fixture");

    assert!(harness.is_running(), "Should start running");

    // Open search
    harness.send_key(KeyCode::Char('/'));
    assert!(harness.is_running(), "Should survive opening search");

    // Type a search term that definitely won't match
    harness.type_text("xyznonexistent123impossible");
    assert!(harness.is_running(), "Should survive typing search query");

    // Submit search
    harness.send_key(KeyCode::Enter);
    assert!(
        harness.is_running(),
        "Should survive submitting search with no matches"
    );

    // Try to navigate to next match (should handle gracefully when count = 0)
    for i in 0..5 {
        harness.send_key(KeyCode::Char('n'));

        assert!(
            harness.is_running(),
            "App crashed trying to navigate to match {} when zero matches exist - division by zero or index error?",
            i + 1
        );
    }

    // Try to navigate to previous match
    for i in 0..5 {
        harness.send_key(KeyCode::Char('N'));

        assert!(
            harness.is_running(),
            "App crashed trying to navigate to previous match {} when zero matches exist",
            i + 1
        );
    }

    // VERIFY: App should still be running
    assert!(
        harness.is_running(),
        "App should remain running after search with zero results"
    );
}

/// Test: Large fixture navigation should not crash
///
/// Tests robustness with a large real-world session log (180MB).
/// This is the most comprehensive crash test - if the app can handle
/// this fixture with extensive navigation, it's quite robust.
///
/// EXPECT: App loads large fixture and survives extensive navigation
#[test]
fn crash_large_fixture_navigation() {
    // DOING: Load large fixture and perform extensive navigation
    // EXPECT: App remains stable with large dataset

    let mut harness = AcceptanceTestHarness::from_fixture(LARGE_FIXTURE)
        .expect("Should load large 180MB fixture");

    assert!(
        harness.is_running(),
        "Should start running with large fixture"
    );

    // Scroll down significantly
    for i in 0..30 {
        harness.send_key(KeyCode::Char('j'));

        assert!(
            harness.is_running(),
            "App crashed on scroll iteration {} in large fixture - likely the scroll bug",
            i + 1
        );
    }

    // Page down several times
    for i in 0..10 {
        harness.send_key(KeyCode::PageDown);

        assert!(
            harness.is_running(),
            "App crashed on PageDown iteration {} in large fixture",
            i + 1
        );
    }

    // Scroll back up
    for i in 0..20 {
        harness.send_key(KeyCode::Char('k'));

        assert!(
            harness.is_running(),
            "App crashed on scroll-up iteration {} in large fixture",
            i + 1
        );
    }

    // Try tab switching (if subagents exist in fixture)
    for i in 0..10 {
        harness.send_key(KeyCode::Right);

        assert!(
            harness.is_running(),
            "App crashed during tab navigation in large fixture at iteration {}",
            i + 1
        );
    }

    // Toggle stats panel
    harness.send_key(KeyCode::Char('s'));
    assert!(
        harness.is_running(),
        "Should survive toggling stats in large fixture"
    );

    // Close stats panel
    harness.send_key(KeyCode::Char('s'));
    assert!(
        harness.is_running(),
        "Should survive closing stats in large fixture"
    );

    // VERIFY: App should still be running after all operations
    assert!(
        harness.is_running(),
        "App should remain running after extensive navigation in 180MB fixture"
    );
}

/// Test: Buffer bounds crash regression (cclv-31l.13)
///
/// Specific test for the buffer index out of bounds crash:
/// "index outside of buffer: the area is Rect { x: 0, y: 0, width: 181, height: 46 }
///  but index is (1, 46)"
///
/// The issue is that view rendering writes to y=46 which is outside buffer height (0-45).
/// This is an off-by-one error in scroll/cursor position calculation that fails to clamp
/// to viewport bounds.
///
/// EXPECT: App survives scrolling and rendering without buffer bounds panic
#[test]
fn crash_buffer_bounds_regression() {
    // DOING: Load large fixture with specific terminal size matching crash report
    // EXPECT: Reproduce buffer bounds crash scenario

    let mut harness = AcceptanceTestHarness::from_fixture_with_size(LARGE_FIXTURE, 181, 46)
        .expect("Should load large fixture with crash dimensions");

    assert!(
        harness.is_running(),
        "Should start running with large fixture"
    );

    // The crash occurs when rendering writes beyond viewport bounds
    // This happens during scrolling when y coordinates aren't properly clamped

    // Scroll down and render to trigger the crash scenario
    for i in 0..10 {
        harness.send_key(KeyCode::Char('j'));

        // Explicitly render to catch buffer bounds panics
        // The panic occurs during rendering, not during key handling
        let render_result =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| harness.render_to_string()));

        assert!(
            render_result.is_ok(),
            "App crashed during rendering on scroll iteration {} - buffer bounds check failed (y=46 in height=46 buffer)",
            i + 1
        );

        assert!(
            harness.is_running(),
            "App should still be running after scroll iteration {}",
            i + 1
        );
    }

    // Scroll down more aggressively
    for i in 0..20 {
        harness.send_key(KeyCode::Char('j'));

        let render_result =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| harness.render_to_string()));

        assert!(
            render_result.is_ok(),
            "App crashed during rendering on scroll iteration {} - buffer bounds not clamped properly",
            i + 10
        );
    }

    // Try PageDown which advances scroll position significantly
    for i in 0..5 {
        harness.send_key(KeyCode::PageDown);

        let render_result =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| harness.render_to_string()));

        assert!(
            render_result.is_ok(),
            "App crashed during rendering on PageDown iteration {} - large scroll advance triggers bounds violation",
            i + 1
        );
    }

    // Final render check
    let final_render =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| harness.render_to_string()));

    assert!(
        final_render.is_ok(),
        "Final render should succeed - all y coordinates properly clamped to viewport"
    );

    // VERIFY: App should still be running
    assert!(
        harness.is_running(),
        "App should remain running - no buffer bounds violations"
    );
}
