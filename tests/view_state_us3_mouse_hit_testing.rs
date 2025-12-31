//! Acceptance tests for User Story 3: Click Entries with Mouse
//!
//! Tests the 4 acceptance scenarios from spec.md US3:
//! 1. Click on specific entry selects that entry (not adjacent)
//! 2. Click on expand/collapse indicator toggles state
//! 3. Scrolled to middle, hit-test maps Y correctly
//! 4. Variable height entries, click near boundaries targets correct entry
//!
//! These are acceptance tests that verify end-to-end behavior.

use cclv::model::{
    ConversationEntry, EntryMetadata, EntryType, EntryUuid, LogEntry, Message, MessageContent,
    Role, SessionId,
};
use cclv::state::WrapMode;
use cclv::view_state::{
    conversation::ConversationViewState,
    hit_test::HitTestResult,
    layout_params::LayoutParams,
    types::{EntryIndex, LineHeight, LineOffset, ViewportDimensions},
};
use chrono::Utc;

// ===== Test Helpers =====

/// Create a valid conversation entry for testing
fn make_entry(uuid: &str) -> ConversationEntry {
    let uuid = EntryUuid::new(uuid).unwrap();
    let session = SessionId::new("test-session").unwrap();
    let message = Message::new(Role::User, MessageContent::Text(format!("Test message for {}", uuid)));
    let entry = LogEntry::new(
        uuid,
        None,
        session,
        None,
        Utc::now(),
        EntryType::User,
        message,
        EntryMetadata::default(),
    );
    ConversationEntry::Valid(Box::new(entry))
}

/// Height calculator that returns a fixed height
fn fixed_height(_entry: &ConversationEntry, _expanded: bool, _wrap: WrapMode) -> LineHeight {
    LineHeight::new(10).unwrap()
}

/// Height calculator with variable heights based on entry index
fn variable_height(entry: &ConversationEntry, _expanded: bool, _wrap: WrapMode) -> LineHeight {
    // Extract index from UUID pattern "entry-N"
    if let ConversationEntry::Valid(log_entry) = entry {
        let uuid_str = log_entry.uuid().as_str();
        if let Some(idx_str) = uuid_str.strip_prefix("entry-") {
            if let Ok(idx) = idx_str.parse::<u16>() {
                // Vary heights: 5, 10, 15, 20, 5, 10, 15, 20, ...
                let height = 5 + ((idx % 4) * 5);
                return LineHeight::new(height).unwrap();
            }
        }
    }
    LineHeight::new(10).unwrap()
}

// ===== US3 Scenario 1: Click Specific Entry (Not Adjacent) =====

#[test]
fn us3_scenario1_click_specific_entry_not_adjacent() {
    // GIVEN: Multiple entries
    let entries = vec![
        make_entry("entry-0"),
        make_entry("entry-1"),
        make_entry("entry-2"),
        make_entry("entry-3"),
        make_entry("entry-4"),
    ];

    let mut state = ConversationViewState::new(None, None, entries);
    let params = LayoutParams::new(80, WrapMode::Wrap);
    state.recompute_layout(params, fixed_height);

    // Layout: Each entry is 10 lines
    // Entry 0: lines 0..10
    // Entry 1: lines 10..20
    // Entry 2: lines 20..30
    // Entry 3: lines 30..40
    // Entry 4: lines 40..50

    // WHEN: Click on entry 2 (middle of entry at line 25)
    let result = state.hit_test(25, 10, LineOffset::new(0));

    // THEN: Entry 2 is selected (not entry 1 or 3)
    assert_eq!(
        result,
        HitTestResult::Hit {
            entry_index: EntryIndex::new(2),
            line_in_entry: 5,
            column: 10
        },
        "Click at line 25 should hit entry 2, not adjacent entries"
    );

    // Verify adjacent entries are NOT hit
    // Click at line 19 should hit entry 1
    let result_above = state.hit_test(19, 10, LineOffset::new(0));
    assert_eq!(
        result_above.entry_index(),
        Some(EntryIndex::new(1)),
        "Click at line 19 should hit entry 1"
    );

    // Click at line 30 should hit entry 3
    let result_below = state.hit_test(30, 10, LineOffset::new(0));
    assert_eq!(
        result_below.entry_index(),
        Some(EntryIndex::new(3)),
        "Click at line 30 should hit entry 3"
    );
}

// ===== US3 Scenario 2: Click Expand/Collapse Indicator =====

#[test]
fn us3_scenario2_click_expand_collapse_indicator() {
    // GIVEN: Entry that is collapsible
    let entries = vec![
        make_entry("entry-0"),
        make_entry("entry-1"),
        make_entry("entry-2"),
    ];

    let mut state = ConversationViewState::new(None, None, entries);
    let params = LayoutParams::new(80, WrapMode::Wrap);
    state.recompute_layout(params, fixed_height);

    // WHEN: Click on expand/collapse indicator (assumed to be at column 0-2)
    // First, we need to hit-test to identify which entry
    let result = state.hit_test(15, 1, LineOffset::new(0)); // Line 15, col 1 -> entry 1

    // THEN: hit_test returns correct entry for toggle action
    assert_eq!(
        result.entry_index(),
        Some(EntryIndex::new(1)),
        "Click on expand indicator should identify entry 1"
    );

    // Verify the toggle mechanism works (using ConversationViewState.toggle_expand)
    let initial_expanded = state
        .get(EntryIndex::new(1))
        .unwrap()
        .is_expanded();

    // Toggle expand state
    let viewport = ViewportDimensions::new(80, 24);
    let new_state = state.toggle_expand(EntryIndex::new(1), params, viewport, fixed_height);
    assert_eq!(
        new_state,
        Some(!initial_expanded),
        "toggle_expand should change expanded state"
    );

    // Verify entry state changed
    let after_toggle = state
        .get(EntryIndex::new(1))
        .unwrap()
        .is_expanded();
    assert_eq!(
        after_toggle, !initial_expanded,
        "Entry expanded state should toggle"
    );
}

// ===== US3 Scenario 3: Scrolled to Middle, Hit-Test Maps Y Correctly =====

#[test]
fn us3_scenario3_scrolled_middle_hit_test_correct() {
    // GIVEN: Conversation with many entries, scrolled to middle
    let entries: Vec<_> = (0..20).map(|i| make_entry(&format!("entry-{}", i))).collect();

    let mut state = ConversationViewState::new(None, None, entries);
    let params = LayoutParams::new(80, WrapMode::Wrap);
    state.recompute_layout(params, fixed_height);

    // Total height: 20 entries * 10 lines = 200 lines
    // Entry 10: lines 100..110
    // Entry 11: lines 110..120

    // WHEN: Scrolled to middle (scroll_offset = 100), click at screen_y = 15
    // Absolute Y = 100 + 15 = 115 (which is in entry 11)
    let result = state.hit_test(15, 20, LineOffset::new(100));

    // THEN: Hit-test correctly maps to entry 11, line 5 within entry
    assert_eq!(
        result,
        HitTestResult::Hit {
            entry_index: EntryIndex::new(11),
            line_in_entry: 5,
            column: 20
        },
        "With scroll_offset=100, screen_y=15 should map to entry 11"
    );

    // Verify at different scroll positions
    // Scroll to bottom (offset = 180), click at screen_y = 10
    // Absolute Y = 180 + 10 = 190 (entry 19, line 0)
    let result_bottom = state.hit_test(10, 30, LineOffset::new(180));
    assert_eq!(
        result_bottom,
        HitTestResult::Hit {
            entry_index: EntryIndex::new(19),
            line_in_entry: 0,
            column: 30
        },
        "With scroll_offset=180, screen_y=10 should map to entry 19"
    );
}

// ===== US3 Scenario 4: Variable Height Entries, Click Near Boundaries =====

#[test]
fn us3_scenario4_variable_heights_boundary_clicks() {
    // GIVEN: Variable height entries
    let entries: Vec<_> = (0..8).map(|i| make_entry(&format!("entry-{}", i))).collect();

    let mut state = ConversationViewState::new(None, None, entries);
    let params = LayoutParams::new(80, WrapMode::Wrap);
    state.recompute_layout(params, variable_height);

    // Heights: 5, 10, 15, 20, 5, 10, 15, 20
    // Entry 0: lines 0..5   (height 5)
    // Entry 1: lines 5..15  (height 10)
    // Entry 2: lines 15..30 (height 15)
    // Entry 3: lines 30..50 (height 20)
    // Entry 4: lines 50..55 (height 5)
    // Entry 5: lines 55..65 (height 10)
    // Entry 6: lines 65..80 (height 15)
    // Entry 7: lines 80..100 (height 20)

    // WHEN: Click at last line of entry 0 (line 4)
    let result_end_0 = state.hit_test(4, 10, LineOffset::new(0));
    // THEN: Should hit entry 0
    assert_eq!(
        result_end_0.entry_index(),
        Some(EntryIndex::new(0)),
        "Last line of entry 0 should hit entry 0"
    );

    // WHEN: Click at first line of entry 1 (line 5)
    let result_start_1 = state.hit_test(5, 10, LineOffset::new(0));
    // THEN: Should hit entry 1
    assert_eq!(
        result_start_1.entry_index(),
        Some(EntryIndex::new(1)),
        "First line of entry 1 should hit entry 1"
    );

    // WHEN: Click at last line of entry 2 (line 29)
    let result_end_2 = state.hit_test(29, 10, LineOffset::new(0));
    // THEN: Should hit entry 2
    assert_eq!(
        result_end_2.entry_index(),
        Some(EntryIndex::new(2)),
        "Last line of entry 2 should hit entry 2"
    );

    // WHEN: Click at first line of entry 3 (line 30)
    let result_start_3 = state.hit_test(30, 10, LineOffset::new(0));
    // THEN: Should hit entry 3
    assert_eq!(
        result_start_3.entry_index(),
        Some(EntryIndex::new(3)),
        "First line of entry 3 should hit entry 3"
    );

    // WHEN: Click between entries 4 and 5 (at line 55)
    let result_boundary_4_5 = state.hit_test(55, 10, LineOffset::new(0));
    // THEN: Should hit entry 5 (first line)
    assert_eq!(
        result_boundary_4_5,
        HitTestResult::Hit {
            entry_index: EntryIndex::new(5),
            line_in_entry: 0,
            column: 10
        },
        "Boundary between entries 4 and 5 should hit entry 5"
    );
}

// ===== Performance Verification: O(log n) =====

#[test]
fn us3_performance_hit_test_o_log_n() {
    use std::time::Instant;

    // GIVEN: Large number of entries (100k would be ideal, but 10k for CI speed)
    let entries: Vec<_> = (0..10_000)
        .map(|i| make_entry(&format!("entry-{}", i)))
        .collect();

    let mut state = ConversationViewState::new(None, None, entries);
    let params = LayoutParams::new(80, WrapMode::Wrap);
    state.recompute_layout(params, fixed_height);

    // Total height: 10,000 entries * 10 lines = 100,000 lines

    // WHEN: Perform hit_test at various positions
    let test_positions = vec![
        (0, 0, 0),           // Start
        (50_000, 25, 10),    // Middle
        (99_999, 50, 20),    // End
        (25_000, 10, 5),     // Quarter
        (75_000, 30, 15),    // Three quarters
    ];

    let mut total_duration = std::time::Duration::ZERO;
    let iterations = 100;

    for &(absolute_y, column, _expected_entry) in &test_positions {
        for _ in 0..iterations {
            let screen_y = (absolute_y % 1000) as u16;
            let scroll_offset = LineOffset::new(absolute_y - (absolute_y % 1000));

            let start = Instant::now();
            let _result = state.hit_test(screen_y, column, scroll_offset);
            total_duration += start.elapsed();
        }
    }

    let avg_duration = total_duration / (test_positions.len() as u32 * iterations);

    // THEN: Average duration should be < 1ms (O(log n) performance)
    assert!(
        avg_duration.as_micros() < 1000,
        "hit_test should complete in <1ms for 10k entries (O(log n)), got {:?}",
        avg_duration
    );
}
