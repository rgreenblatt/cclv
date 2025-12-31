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
    PricingConfig, Role, SessionId,
};
use cclv::state::WrapMode;
use cclv::view_state::{
    conversation::ConversationViewState,
    layout_params::LayoutParams,
    types::{EntryIndex, LineOffset, ViewportDimensions},
};
use chrono::Utc;

// ===== Test Helpers =====

/// Create a valid conversation entry for testing
fn make_entry(uuid: &str) -> ConversationEntry {
    let uuid = EntryUuid::new(uuid).unwrap();
    let session = SessionId::new("test-session").unwrap();
    let message = Message::new(
        Role::User,
        MessageContent::Text(format!("Test message for {}", uuid)),
    );
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

    let mut state =
        ConversationViewState::new(None, None, entries, 200_000, PricingConfig::default());
    let params = LayoutParams::new(80, WrapMode::Wrap);
    state.relayout_from(EntryIndex::new(0), params);

    // Get actual heights after relayout
    let h0 = state.get(EntryIndex::new(0)).unwrap().height().get() as usize;
    let h1 = state.get(EntryIndex::new(1)).unwrap().height().get() as usize;
    let h2 = state.get(EntryIndex::new(2)).unwrap().height().get() as usize;

    // Layout: (using actual heights)
    // Entry 0: lines 0..h0
    // Entry 1: lines h0..(h0+h1)
    // Entry 2: lines (h0+h1)..(h0+h1+h2)
    // Entry 3: lines (h0+h1+h2)..
    // Entry 4: lines ..

    // WHEN: Click in middle of entry 2
    let entry2_start = h0 + h1;
    let click_y = entry2_start + (h2 / 2);
    let result = state.hit_test(click_y as u16, 10, LineOffset::new(0));

    // THEN: Entry 2 is selected (not entry 1 or 3)
    assert_eq!(
        result.entry_index(),
        Some(EntryIndex::new(2)),
        "Click in middle of entry 2 should hit entry 2, not adjacent entries"
    );

    // Verify adjacent entries are NOT hit
    // Click in middle of entry 1
    let entry1_start = h0;
    let click_y_entry1 = entry1_start + (h1 / 2);
    let result_above = state.hit_test(click_y_entry1 as u16, 10, LineOffset::new(0));
    assert_eq!(
        result_above.entry_index(),
        Some(EntryIndex::new(1)),
        "Click in middle of entry 1 should hit entry 1"
    );

    // Click in middle of entry 3
    let h3 = state.get(EntryIndex::new(3)).unwrap().height().get() as usize;
    let entry3_start = h0 + h1 + h2;
    let click_y_entry3 = entry3_start + (h3 / 2);
    let result_below = state.hit_test(click_y_entry3 as u16, 10, LineOffset::new(0));
    assert_eq!(
        result_below.entry_index(),
        Some(EntryIndex::new(3)),
        "Click in middle of entry 3 should hit entry 3"
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

    let mut state =
        ConversationViewState::new(None, None, entries, 200_000, PricingConfig::default());
    let params = LayoutParams::new(80, WrapMode::Wrap);
    state.relayout_from(EntryIndex::new(0), params);

    // Get actual heights
    let h0 = state.get(EntryIndex::new(0)).unwrap().height().get() as usize;
    let h1 = state.get(EntryIndex::new(1)).unwrap().height().get() as usize;

    // WHEN: Click on expand/collapse indicator (assumed to be at column 0-2)
    // Click in middle of entry 1
    let entry1_start = h0;
    let click_y = entry1_start + (h1 / 2);
    let result = state.hit_test(click_y as u16, 1, LineOffset::new(0));

    // THEN: hit_test returns correct entry for toggle action
    assert_eq!(
        result.entry_index(),
        Some(EntryIndex::new(1)),
        "Click on expand indicator should identify entry 1"
    );

    // Verify the toggle mechanism works (using ConversationViewState.toggle_expand)
    let initial_expanded = state.get(EntryIndex::new(1)).unwrap().is_expanded();

    // Toggle expand state
    let viewport = ViewportDimensions::new(80, 24);
    let new_state = state.toggle_expand(EntryIndex::new(1), params, viewport);
    assert_eq!(
        new_state,
        Some(!initial_expanded),
        "toggle_expand should change expanded state"
    );

    // Verify entry state changed
    let after_toggle = state.get(EntryIndex::new(1)).unwrap().is_expanded();
    assert_eq!(
        after_toggle, !initial_expanded,
        "Entry expanded state should toggle"
    );
}

// ===== US3 Scenario 3: Scrolled to Middle, Hit-Test Maps Y Correctly =====

#[test]
fn us3_scenario3_scrolled_middle_hit_test_correct() {
    // GIVEN: Conversation with many entries, scrolled to middle
    let entries: Vec<_> = (0..20)
        .map(|i| make_entry(&format!("entry-{}", i)))
        .collect();

    let mut state =
        ConversationViewState::new(None, None, entries, 200_000, PricingConfig::default());
    let params = LayoutParams::new(80, WrapMode::Wrap);
    state.relayout_from(EntryIndex::new(0), params);

    // Calculate cumulative heights to find entry positions
    let mut cumulative_heights = vec![0usize];
    for i in 0..20 {
        let h = state.get(EntryIndex::new(i)).unwrap().height().get() as usize;
        cumulative_heights.push(cumulative_heights[i] + h);
    }

    // WHEN: Scrolled to middle of entry 11, click at screen_y = 5
    // Find entry 11 position
    let entry11_start = cumulative_heights[11];
    let h11 = state.get(EntryIndex::new(11)).unwrap().height().get() as usize;
    let scroll_offset = entry11_start; // Scroll so entry 11 is at top
    let screen_y = h11 / 2; // Click in middle of entry 11
    let result = state.hit_test(screen_y as u16, 20, LineOffset::new(scroll_offset));

    // THEN: Hit-test correctly maps to entry 11
    assert_eq!(
        result.entry_index(),
        Some(EntryIndex::new(11)),
        "With entry 11 at top, clicking in middle should hit entry 11"
    );

    // Verify at different scroll positions
    // Scroll to entry 19, click at screen_y = 0 (first line of entry 19)
    let entry19_start = cumulative_heights[19];
    let result_bottom = state.hit_test(0, 30, LineOffset::new(entry19_start));
    assert_eq!(
        result_bottom.entry_index(),
        Some(EntryIndex::new(19)),
        "With entry 19 at top, clicking at screen_y=0 should hit entry 19"
    );
}

// ===== US3 Scenario 4: Variable Height Entries, Click Near Boundaries =====

#[test]
fn us3_scenario4_variable_heights_boundary_clicks() {
    // GIVEN: Variable height entries
    let entries: Vec<_> = (0..8)
        .map(|i| make_entry(&format!("entry-{}", i)))
        .collect();

    let mut state =
        ConversationViewState::new(None, None, entries, 200_000, PricingConfig::default());
    let params = LayoutParams::new(80, WrapMode::Wrap);
    state.relayout_from(EntryIndex::new(0), params);

    // Calculate cumulative heights for boundary testing
    let mut cumulative_heights = vec![0usize];
    for i in 0..8 {
        let h = state.get(EntryIndex::new(i)).unwrap().height().get() as usize;
        cumulative_heights.push(cumulative_heights[i] + h);
    }

    // Test boundary clicks for all entries
    for i in 0..7 {
        let entry_start = cumulative_heights[i];
        let entry_end = cumulative_heights[i + 1];
        let h = entry_end - entry_start;

        // WHEN: Click at last line of entry i
        if h > 0 {
            let last_line = entry_start + h - 1;
            let result_end = state.hit_test(last_line as u16, 10, LineOffset::new(0));
            // THEN: Should hit entry i
            assert_eq!(
                result_end.entry_index(),
                Some(EntryIndex::new(i)),
                "Last line of entry {} (line {}) should hit entry {}",
                i,
                last_line,
                i
            );
        }

        // WHEN: Click at first line of entry i+1
        let first_line = entry_end;
        let result_start = state.hit_test(first_line as u16, 10, LineOffset::new(0));
        // THEN: Should hit entry i+1
        assert_eq!(
            result_start.entry_index(),
            Some(EntryIndex::new(i + 1)),
            "First line of entry {} (line {}) should hit entry {}",
            i + 1,
            first_line,
            i + 1
        );
    }
}

// ===== Performance Verification: O(log n) =====

#[test]
fn us3_performance_hit_test_o_log_n() {
    use std::time::Instant;

    // GIVEN: Large number of entries (100k would be ideal, but 10k for CI speed)
    let entries: Vec<_> = (0..10_000)
        .map(|i| make_entry(&format!("entry-{}", i)))
        .collect();

    let mut state =
        ConversationViewState::new(None, None, entries, 200_000, PricingConfig::default());
    let params = LayoutParams::new(80, WrapMode::Wrap);
    state.relayout_from(EntryIndex::new(0), params);

    // WHEN: Perform hit_test at various positions
    let test_positions = vec![
        (0, 0, 0),        // Start
        (50_000, 25, 10), // Middle
        (99_999, 50, 20), // End
        (25_000, 10, 5),  // Quarter
        (75_000, 30, 15), // Three quarters
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
