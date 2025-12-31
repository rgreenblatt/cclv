//! Acceptance tests for User Story 3: Review Usage Statistics
//!
//! Tests the 4 acceptance scenarios from spec.md lines 88-93.
//! Each test verifies actual runtime behavior for stats panel functionality.

use crate::test_harness::AcceptanceTestHarness;
use crossterm::event::KeyCode;

// ===== Test Helpers =====

/// Build SessionStats from SessionViewState by iterating all entries.
/// Matches the approach used in view/layout.rs until stats are integrated into Session.
fn build_session_stats(
    session_view: &crate::view_state::session::SessionViewState,
) -> crate::model::SessionStats {
    use crate::model::{ConversationEntry, SessionStats};

    let mut stats = SessionStats::default();

    // Process main agent entries
    for entry_view in session_view.main().entries() {
        if let ConversationEntry::Valid(log_entry) = entry_view.entry() {
            stats.record_entry(&**log_entry);
        }
    }

    // Process subagent entries
    for conversation_view in session_view.subagents().values() {
        for entry_view in conversation_view.entries() {
            if let ConversationEntry::Valid(log_entry) = entry_view.entry() {
                stats.record_entry(&**log_entry);
            }
        }
    }

    stats
}

// ===== Test Fixtures =====

const MINIMAL_FIXTURE: &str = "tests/fixtures/minimal_session.jsonl";
const TOOL_CALLS_FIXTURE: &str = "tests/fixtures/tool_calls.jsonl";
const WITH_SUBAGENTS_FIXTURE: &str = "tests/fixtures/with_subagents.jsonl";

// ===== US3 Scenario 1: View Stats Panel =====

#[test]
fn us3_scenario1_view_stats_panel() {
    // US3-SC1
    // GIVEN: A loaded session
    // WHEN: User opens the stats panel
    // THEN: They see total input tokens, output tokens, and estimated cost

    // DOING: Load session with token usage data
    // EXPECT: Session loads successfully with stats available
    let mut harness = AcceptanceTestHarness::from_fixture(MINIMAL_FIXTURE)
        .expect("Should load session with stats data");

    // IF YES: Session loaded
    assert!(
        harness.is_running(),
        "Harness should be running after loading fixture"
    );

    let initial_state = harness.state();
    assert!(
        !initial_state.stats_visible,
        "Stats panel should be hidden initially"
    );

    // WHEN: User presses 's' to toggle stats panel
    harness.send_key(KeyCode::Char('s'));

    // VERIFY: Stats panel is now visible
    let state_after_toggle = harness.state();
    assert!(
        state_after_toggle.stats_visible,
        "Stats panel should be visible after pressing 's'"
    );

    // VERIFY: Rendered output shows stats panel
    let output = harness.render_to_string();

    // Should show "Statistics" title
    assert!(
        output.contains("Statistics"),
        "Should display 'Statistics' panel title"
    );

    // Should show token counts (Input/Output)
    let has_input_label = output.contains("Input");
    let has_output_label = output.contains("Output");
    assert!(
        has_input_label && has_output_label,
        "Should display 'Input' and 'Output' token labels"
    );

    // Should show some numeric token values
    // The minimal_session.jsonl has token usage data
    let has_token_numbers = output
        .lines()
        .any(|line| line.contains("Input") && line.chars().any(|c| c.is_ascii_digit()));
    assert!(
        has_token_numbers,
        "Should display actual token count numbers"
    );

    // Should show cost estimate (looks for $ or "Cost")
    let has_cost = output.contains("Cost") || output.contains('$');
    assert!(has_cost, "Should display estimated cost");

    // RESULT: Stats panel visible with token counts and cost
    // MATCHES: Yes - stats panel displays usage metrics
    // THEREFORE: US3 Scenario 1 verified (when ToggleStats is implemented)
}

// ===== US3 Scenario 2: Filter Main Agent =====

#[test]
fn us3_scenario2_filter_main_agent() {
    // US3-SC2
    // GIVEN: Stats are displayed
    // WHEN: User filters by "Main Agent"
    // THEN: Only main agent statistics are shown (excluding subagent activity)

    // DOING: Load session, open stats, apply main agent filter
    // EXPECT: Stats show main agent data only
    let mut harness = AcceptanceTestHarness::from_fixture(TOOL_CALLS_FIXTURE)
        .expect("Should load session with tool usage");

    // IF YES: Session loaded
    // Open stats panel (assuming this will work once ToggleStats is implemented)
    // For now, we'll manually set stats_visible via state inspection
    // Note: This test verifies the filter mechanism works, not the toggle

    let initial_state = harness.state();
    let initial_filter = &initial_state.stats_filter;

    // Verify initial filter is Global
    assert_eq!(
        *initial_filter,
        crate::model::StatsFilter::Global,
        "Default stats filter should be Global"
    );

    // WHEN: User presses 'm' to filter to Main Agent
    harness.send_key(KeyCode::Char('m'));

    // VERIFY: Filter changed to MainAgent
    let state_after_filter = harness.state();
    assert_eq!(
        state_after_filter.stats_filter,
        crate::model::StatsFilter::MainAgent,
        "Stats filter should change to MainAgent after pressing 'm'"
    );

    // VERIFY: Stats calculations use the filter correctly
    // The SessionStats.filtered_usage() method should return main agent usage only
    let session = state_after_filter.session_view();
    let stats = build_session_stats(session);
    let main_usage = stats.filtered_usage(&crate::model::StatsFilter::MainAgent);
    let global_usage = stats.filtered_usage(&crate::model::StatsFilter::Global);

    // Main agent usage should be <= global usage (subset)
    assert!(
        main_usage.input_tokens <= global_usage.input_tokens,
        "Main agent input tokens should be subset of global"
    );
    assert!(
        main_usage.output_tokens <= global_usage.output_tokens,
        "Main agent output tokens should be subset of global"
    );

    // RESULT: Main agent filter applied successfully
    // MATCHES: Yes - filter changes stats view to main agent only
    // THEREFORE: US3 Scenario 2 verified
}

// ===== US3 Scenario 3: Tool Breakdown =====

#[test]
fn us3_scenario3_tool_breakdown() {
    // US3-SC3
    // GIVEN: Stats are displayed
    // WHEN: User views tool breakdown
    // THEN: They see each tool name with count of invocations (e.g., "Read: 15, Write: 8, Bash: 12")

    // DOING: Load session with tool calls, open stats panel
    // EXPECT: Tool breakdown visible in stats panel
    let mut harness = AcceptanceTestHarness::from_fixture(TOOL_CALLS_FIXTURE)
        .expect("Should load session with tool usage");

    // IF YES: Session loaded with tool calls
    let state = harness.state();
    let stats = build_session_stats(state.session_view());

    // Verify fixture has tool calls recorded
    let tool_counts = stats.filtered_tool_counts(&crate::model::StatsFilter::Global);
    assert!(
        !tool_counts.is_empty(),
        "Fixture should contain tool usage data"
    );

    // WHEN: User opens stats panel
    harness.send_key(KeyCode::Char('s')); // Toggle stats

    // VERIFY: Stats panel is visible
    let state_after = harness.state();
    assert!(
        state_after.stats_visible,
        "Stats panel should be visible after toggle"
    );

    // VERIFY: Rendered output shows tool breakdown
    let output = harness.render_to_string();

    // Should show "Tools:" section header
    assert!(
        output.contains("Tools") || output.contains("Tool"),
        "Should display 'Tools' section header"
    );

    // Should show at least some tool names with counts
    // The tool_calls.jsonl has Read, Write, Bash, Edit tools
    let has_tool_with_count = output.lines().any(|line| {
        (line.contains("Read") || line.contains("Write") || line.contains("Bash"))
            && line.chars().any(|c| c.is_ascii_digit())
    });

    assert!(
        has_tool_with_count,
        "Should display tool names with invocation counts (e.g., 'Read: 15')"
    );

    // RESULT: Tool breakdown visible in stats panel
    // MATCHES: Yes - tool names and counts displayed
    // THEREFORE: US3 Scenario 3 verified (when rendering is complete)
}

// ===== US3 Scenario 4: Filter Subagent =====

#[test]
fn us3_scenario4_filter_subagent() {
    // US3-SC4
    // GIVEN: A session with multiple subagents
    // WHEN: User filters by a specific subagent ID
    // THEN: Only that subagent's statistics are shown

    // DOING: Load session with subagents, apply subagent filter
    // EXPECT: Stats show specific subagent's data only
    let mut harness = AcceptanceTestHarness::from_fixture(WITH_SUBAGENTS_FIXTURE)
        .expect("Should load session with subagents");

    // IF YES: Session loaded with subagents
    let initial_state = harness.state();
    let subagent_count = initial_state.session_view().subagent_ids().count();

    assert!(
        subagent_count > 0,
        "Fixture should contain multiple subagents"
    );

    // WHEN: User selects a subagent tab first (required for filter context)
    harness.send_key(KeyCode::Tab); // Switch to subagent pane

    let state_after_tab = harness.state();
    assert_eq!(
        state_after_tab.focus,
        crate::state::FocusPane::Subagent,
        "Focus should switch to subagent pane"
    );

    // Verify a subagent tab is selected
    assert!(
        state_after_tab.selected_tab_index().is_some(),
        "A subagent tab should be selected"
    );

    // WHEN: User presses 'S' (Shift+s) to filter to current subagent
    harness.send_key(KeyCode::Char('S')); // FilterSubagent action

    // VERIFY: Filter changed to specific Subagent
    let state_after_filter = harness.state();
    match &state_after_filter.stats_filter {
        crate::model::StatsFilter::Subagent(agent_id) => {
            // Verify the agent_id corresponds to the selected tab
            // Unified tab model (FR-086): tab 0 = main, tab 1+ = subagents
            let subagent_ids: Vec<_> = state_after_filter.session_view().subagent_ids().collect();
            let tab_index = state_after_filter
                .selected_tab_index()
                .expect("Tab should be selected");

            // Convert from global tab index to subagent position
            // tab 1 -> subagent[0], tab 2 -> subagent[1], etc.
            let subagent_position = tab_index
                .checked_sub(1)
                .expect("Tab index should be >= 1 for subagents");
            let expected_agent_id = subagent_ids
                .get(subagent_position)
                .expect("Tab index should be valid");

            assert_eq!(
                agent_id, *expected_agent_id,
                "Filter should target the currently selected subagent tab"
            );
        }
        other => panic!("Stats filter should be Subagent variant, got: {:?}", other),
    }

    // VERIFY: Filtered stats show only this subagent's data
    let stats = build_session_stats(state_after_filter.session_view());
    let subagent_usage = stats.filtered_usage(&state_after_filter.stats_filter);
    let global_usage = stats.filtered_usage(&crate::model::StatsFilter::Global);

    // Subagent usage should be <= global usage (subset)
    assert!(
        subagent_usage.input_tokens <= global_usage.input_tokens,
        "Subagent input tokens should be subset of global"
    );
    assert!(
        subagent_usage.output_tokens <= global_usage.output_tokens,
        "Subagent output tokens should be subset of global"
    );

    // VERIFY: Rendered stats panel shows subagent-specific data
    // Open stats panel to verify rendering
    harness.send_key(KeyCode::Char('s')); // Toggle stats

    let output = harness.render_to_string();
    assert!(
        output.contains("Statistics (Subagent)"),
        "Stats panel title should indicate subagent filter"
    );

    // RESULT: Subagent filter applied, stats show specific subagent's data
    // MATCHES: Yes - filter isolates specific subagent statistics
    // THEREFORE: US3 Scenario 4 verified (when subagent support is implemented)
}
