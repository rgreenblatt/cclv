//! Bug reproduction: Stats pane shows global stats instead of session-specific stats
//!
//! EXPECTED: When viewing a specific tab (Main or subagent), the stats pane should
//!           display token counts and costs for that specific session only.
//!
//! ACTUAL: Stats pane shows the same global/aggregated stats regardless of which
//!         tab is currently selected.
//!
//! Steps to reproduce manually:
//! 1. cargo run -- tests/fixtures/stats_session_mismatch_repro.jsonl
//! 2. Press 's' to toggle stats pane
//! 3. Observe stats show Input: 3,125 (global total)
//! 4. Press Tab to switch to subagent tab
//! 5. Observe stats STILL show Input: 3,125 (should show ~125 for subagent)
//!
//! Reproduction fixture: tests/fixtures/stats_session_mismatch_repro.jsonl
//! - Main agent: 1000 + 2000 = 3000 input tokens
//! - Subagent: 50 + 75 = 125 input tokens
//! - Global total: 3125 input tokens

use crate::test_harness::AcceptanceTestHarness;
use crossterm::event::KeyCode;

const STATS_FIXTURE: &str = "tests/fixtures/stats_session_mismatch_repro.jsonl";

/// Bug reproduction: stats should change when switching tabs
///
/// When user switches from Main tab to subagent tab, the stats panel should
/// update to show stats for the currently focused session, not global totals.
#[test]
#[ignore = "cclv-5ur.68: Stats pane shows global stats instead of session-specific"]
fn bug_stats_should_update_when_switching_tabs() {
    // GIVEN: Viewer loaded with Main (3000 input) and subagent (125 input)
    let mut harness = AcceptanceTestHarness::from_fixture_with_size(STATS_FIXTURE, 100, 30)
        .expect("Should load fixture");

    // Toggle stats pane visible
    harness.send_key(KeyCode::Char('s'));

    // Render with Main tab focused
    let main_tab_output = harness.render_to_string();

    // Snapshot the buggy state (shows global stats: 3,125 input)
    insta::assert_snapshot!("bug_stats_main_tab", main_tab_output);

    // WHEN: User switches to subagent tab
    harness.send_key(KeyCode::Tab);

    // THEN: Stats should show subagent stats (125 input), not global (3,125)
    let subagent_tab_output = harness.render_to_string();

    // Snapshot the buggy state (still shows global stats: 3,125 input)
    insta::assert_snapshot!("bug_stats_subagent_tab", subagent_tab_output);

    // BUG: These two outputs show the SAME stats numbers
    // Expected: Main tab shows ~3,000 input, subagent tab shows ~125 input
    // Actual: Both show 3,125 (global total)

    // This assertion FAILS because the bug exists - stats show global (3,125)
    // instead of subagent-specific (125). When fixed, this will PASS.
    assert!(
        subagent_tab_output.contains("Input:  125")
            || subagent_tab_output.contains("Input: 125"),
        "BUG: Stats pane shows global stats instead of session-specific stats.\n\
         Expected: Subagent tab should show Input: 125 (subagent tokens only)\n\
         Actual: Stats show Input: 3,125 (global total)\n\
         \n\
         The stats pane does not update when switching tabs.\n\
         Actual output:\n{}",
        subagent_tab_output
    );
}
