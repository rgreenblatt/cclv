//! Acceptance tests for User Story 2: Expand/Collapse Entries
//!
//! Tests the 4 acceptance scenarios from spec.md lines 48-53:
//! 1. Collapsed entry expands and remains visible
//! 2. Expanded entry collapses with smooth shift
//! 3. Toggle response under 16ms
//! 4. Entries above viewport toggle without affecting visible entries

use cclv::model::{
    ConversationEntry, EntryMetadata, EntryType, EntryUuid, LogEntry, Message, MessageContent,
    Role, SessionId,
};
use cclv::state::WrapMode;
use cclv::view_state::{
    conversation::ConversationViewState,
    layout_params::LayoutParams,
    scroll::ScrollPosition,
    types::{EntryIndex, LineHeight, ViewportDimensions},
};
use chrono::Utc;
use std::time::Instant;

// ===== Test Helpers =====

/// Create a test conversation entry with the given UUID and text.
fn create_test_entry(uuid_str: &str, text: &str) -> ConversationEntry {
    let uuid = EntryUuid::new(uuid_str).unwrap();
    let session = SessionId::new("test-session").unwrap();
    let message = Message::new(Role::User, MessageContent::Text(text.to_string()));
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

/// Height calculator: collapsed entries = 3 lines, expanded = 50 lines.
/// Simulates a long message that's much taller when expanded.
fn variable_height_calculator(
    entry: &ConversationEntry,
    expanded: bool,
    _wrap: WrapMode,
    _width: u16,
) -> LineHeight {
    match entry {
        ConversationEntry::Valid(_) => {
            if expanded {
                LineHeight::new(50).unwrap() // Expanded: tall
            } else {
                LineHeight::new(3).unwrap() // Collapsed: summary
            }
        }
        ConversationEntry::Malformed(_) => LineHeight::ZERO,
    }
}

// ===== US2 Scenario 1: Expand Collapsed Entry, Remains Visible =====

#[test]
fn us2_scenario1_expand_collapsed_entry_remains_visible() {
    // GIVEN: A collapsed entry visible in viewport
    // WHEN: User presses Enter/Space to expand
    // THEN: Entry expands and remains visible

    // Create conversation with 5 entries
    let entries = vec![
        create_test_entry("entry-0", "First message"),
        create_test_entry("entry-1", "Second message"),
        create_test_entry("entry-2", "Third message (target)"),
        create_test_entry("entry-3", "Fourth message"),
        create_test_entry("entry-4", "Fifth message"),
    ];

    let mut view_state = ConversationViewState::new(None, None, entries);
    let params = LayoutParams::new(80, WrapMode::Wrap);
    let viewport = ViewportDimensions::new(80, 24);

    // Initial layout - all collapsed
    view_state.relayout_from(EntryIndex::new(0), params, variable_height_calculator);

    // Each entry is 3 lines when collapsed
    // Total height = 5 * 3 = 15 lines
    assert_eq!(
        view_state.total_height(),
        15,
        "Initial total height should be 15 lines (5 entries * 3 lines)"
    );

    // Scroll to show entry 2 (it's at line offset 6)
    view_state.set_scroll(ScrollPosition::at_line(6));

    // Verify entry 2 is NOT expanded
    let entry_2 = view_state.get(EntryIndex::new(2)).unwrap();
    assert!(!entry_2.is_expanded(), "Entry 2 should start collapsed");
    assert_eq!(
        view_state.entry_cumulative_y(EntryIndex::new(2)).unwrap().get(),
        6,
        "Entry 2 should be at line offset 6"
    );

    // WHEN: Toggle expand on entry 2
    let result = view_state.toggle_expand(
        EntryIndex::new(2),
        params,
        viewport,
        variable_height_calculator,
    );

    // THEN: Toggle succeeded
    assert_eq!(
        result,
        Some(true),
        "Toggle should succeed and return new state (expanded)"
    );

    // THEN: Entry 2 is now expanded
    let entry_2_after = view_state.get(EntryIndex::new(2)).unwrap();
    assert!(
        entry_2_after.is_expanded(),
        "Entry 2 should be expanded after toggle"
    );

    // THEN: Entry 2 height changed from 3 to 50
    assert_eq!(
        entry_2_after.height().get(),
        50,
        "Expanded entry should be 50 lines tall"
    );

    // THEN: Total height increased by 47 lines (50 - 3)
    // New total: 3 + 3 + 50 + 3 + 3 = 62
    assert_eq!(
        view_state.total_height(),
        62,
        "Total height should increase to 62 lines"
    );

    // THEN: Entry 2 remains at same cumulative_y (scroll stability)
    assert_eq!(
        view_state.entry_cumulative_y(EntryIndex::new(2)).unwrap().get(),
        6,
        "Entry 2 cumulative_y should remain stable"
    );

    // THEN: Following entries shifted down by 47 lines
    assert_eq!(
        view_state.entry_cumulative_y(EntryIndex::new(3)).unwrap().get(),
        56, // Was at 9, now at 9 + 47 = 56
        "Entry 3 should shift down after entry 2 expands"
    );
}

// ===== US2 Scenario 2: Collapse Expanded Entry, Smooth Shift =====

#[test]
fn us2_scenario2_collapse_expanded_entry_smooth_shift() {
    // GIVEN: An expanded entry visible
    // WHEN: User collapses it
    // THEN: Following entries shift up smoothly without viewport jump

    let entries = vec![
        create_test_entry("entry-0", "First"),
        create_test_entry("entry-1", "Second (will be expanded)"),
        create_test_entry("entry-2", "Third"),
        create_test_entry("entry-3", "Fourth"),
    ];

    let mut view_state = ConversationViewState::new(None, None, entries);
    let params = LayoutParams::new(80, WrapMode::Wrap);
    let viewport = ViewportDimensions::new(80, 24);

    // Initial layout
    view_state.relayout_from(EntryIndex::new(0), params, variable_height_calculator);

    // Expand entry 1 first
    view_state.toggle_expand(
        EntryIndex::new(1),
        params,
        viewport,
        variable_height_calculator,
    );

    // Verify entry 1 is expanded
    let entry_1_before = view_state.get(EntryIndex::new(1)).unwrap();
    assert!(entry_1_before.is_expanded(), "Entry 1 should be expanded");
    assert_eq!(entry_1_before.height().get(), 50);

    // Total height: 3 + 50 + 3 + 3 = 59
    assert_eq!(view_state.total_height(), 59);

    // Entry 2 should be at line 53 (3 + 50)
    assert_eq!(
        view_state.entry_cumulative_y(EntryIndex::new(2)).unwrap().get(),
        53,
        "Entry 2 should be at line 53 before collapse"
    );

    // WHEN: Collapse entry 1
    let result = view_state.toggle_expand(
        EntryIndex::new(1),
        params,
        viewport,
        variable_height_calculator,
    );

    // THEN: Toggle succeeded, now collapsed
    assert_eq!(
        result,
        Some(false),
        "Toggle should return false (collapsed state)"
    );

    // THEN: Entry 1 is collapsed
    let entry_1_after = view_state.get(EntryIndex::new(1)).unwrap();
    assert!(!entry_1_after.is_expanded(), "Entry 1 should be collapsed");
    assert_eq!(
        entry_1_after.height().get(),
        3,
        "Collapsed entry should be 3 lines"
    );

    // THEN: Total height reduced by 47 lines
    assert_eq!(
        view_state.total_height(),
        12, // 3 + 3 + 3 + 3
        "Total height should be 12 lines after collapse"
    );

    // THEN: Entry 2 shifted up smoothly
    assert_eq!(
        view_state.entry_cumulative_y(EntryIndex::new(2)).unwrap().get(),
        6, // 3 + 3
        "Entry 2 should shift up to line 6"
    );
}

// ===== US2 Scenario 3: Toggle Response Under 16ms =====

#[test]
fn us2_scenario3_toggle_response_under_16ms() {
    // GIVEN: User toggles entry expand/collapse
    // WHEN: UI updates
    // THEN: Response is under 16ms (60fps target)

    // Create a moderately sized conversation (20 entries)
    let entries: Vec<_> = (0..20)
        .map(|i| create_test_entry(&format!("entry-{}", i), &format!("Message {}", i)))
        .collect();

    let mut view_state = ConversationViewState::new(None, None, entries);
    let params = LayoutParams::new(80, WrapMode::Wrap);
    let viewport = ViewportDimensions::new(80, 24);

    // Initial layout
    view_state.relayout_from(EntryIndex::new(0), params, variable_height_calculator);

    // Measure toggle time
    let start = Instant::now();

    view_state.toggle_expand(
        EntryIndex::new(10),
        params,
        viewport,
        variable_height_calculator,
    );

    let elapsed = start.elapsed();

    // THEN: Toggle completes in under 16ms
    assert!(
        elapsed.as_millis() < 16,
        "Toggle should complete in under 16ms, took {}ms",
        elapsed.as_millis()
    );

    // Verify the toggle actually worked
    let entry_10 = view_state.get(EntryIndex::new(10)).unwrap();
    assert!(
        entry_10.is_expanded(),
        "Entry should be expanded after toggle"
    );
}

// ===== US2 Scenario 4: Entries Above Viewport Toggle, Visible Entries Stable =====

#[test]
fn us2_scenario4_entries_above_viewport_toggle_visible_stable() {
    // GIVEN: Entries above current viewport are toggled
    // WHEN: Layout updates
    // THEN: Current visible entries remain stable (scroll adjusts)

    // Create conversation with 10 entries
    let entries: Vec<_> = (0..10)
        .map(|i| create_test_entry(&format!("entry-{}", i), &format!("Message {}", i)))
        .collect();

    let mut view_state = ConversationViewState::new(None, None, entries);
    let params = LayoutParams::new(80, WrapMode::Wrap);
    let viewport = ViewportDimensions::new(80, 24);

    // Initial layout - all collapsed (3 lines each)
    view_state.relayout_from(EntryIndex::new(0), params, variable_height_calculator);

    // Scroll to show entry 6 at top of viewport
    // Entry 6 is at line 18 (6 * 3)
    // Viewport will show lines 18-42, but total height is 30
    // So viewport shows lines 6-30 (clamped)
    // This means entries 2-9 are visible, with entry 6 being our anchor
    view_state.set_scroll(ScrollPosition::AtEntry {
        entry_index: EntryIndex::new(6),
        line_in_entry: 0,
    });

    // Capture visible range before toggle
    let visible_before = view_state.visible_range(viewport);
    // Entry 6 should be visible (though not necessarily at start due to clamping)
    assert!(
        visible_before.contains(EntryIndex::new(6)),
        "Entry 6 should be in viewport before toggle"
    );

    // Record entry 6 position before toggle
    let entry_6_y_before = view_state.entry_cumulative_y(EntryIndex::new(6)).unwrap().get();

    // WHEN: Toggle entry 2 (above viewport)
    view_state.toggle_expand(
        EntryIndex::new(2),
        params,
        viewport,
        variable_height_calculator,
    );

    // THEN: Entry 2 is now expanded
    let entry_2 = view_state.get(EntryIndex::new(2)).unwrap();
    assert!(entry_2.is_expanded(), "Entry 2 should be expanded");

    // THEN: Entry 6 shifted down by 47 lines (50 - 3)
    assert_eq!(
        view_state.entry_cumulative_y(EntryIndex::new(6)).unwrap().get(),
        entry_6_y_before + 47,
        "Entry 6 should shift down by 47 lines"
    );

    // THEN: Scroll position adjusted to keep entry 6 at same position in viewport
    // The scroll anchor mechanism should have preserved the viewport
    let scroll_after = view_state.scroll();

    // The scroll should still be anchored to entry 6
    match scroll_after {
        ScrollPosition::AtEntry {
            entry_index,
            line_in_entry,
        } => {
            assert_eq!(
                entry_index.get(),
                6,
                "Scroll should still be anchored to entry 6"
            );
            assert_eq!(*line_in_entry, 0, "Scroll should be at top of entry 6");
        }
        _ => panic!("Scroll should remain as AtEntry after toggle above viewport"),
    }

    // THEN: Entry 6 still visible in viewport
    let visible_after = view_state.visible_range(viewport);
    assert!(
        visible_after.contains(EntryIndex::new(6)),
        "Entry 6 should still be visible (scroll stability)"
    );
}

// ===== Edge Case: Toggle on Non-Existent Entry =====

#[test]
fn toggle_nonexistent_entry_returns_none() {
    let entries = vec![create_test_entry("entry-0", "Only message")];

    let mut view_state = ConversationViewState::new(None, None, entries);
    let params = LayoutParams::new(80, WrapMode::Wrap);
    let viewport = ViewportDimensions::new(80, 24);

    view_state.relayout_from(EntryIndex::new(0), params, variable_height_calculator);

    // Try to toggle entry 999 (doesn't exist)
    let result = view_state.toggle_expand(
        EntryIndex::new(999),
        params,
        viewport,
        variable_height_calculator,
    );

    assert_eq!(
        result, None,
        "Toggle on non-existent entry should return None"
    );
}

// ===== Edge Case: Multiple Toggles Preserve Idempotence =====

#[test]
fn multiple_toggles_preserve_idempotence() {
    let entries = vec![create_test_entry("entry-0", "Test")];

    let mut view_state = ConversationViewState::new(None, None, entries);
    let params = LayoutParams::new(80, WrapMode::Wrap);
    let viewport = ViewportDimensions::new(80, 24);

    view_state.relayout_from(EntryIndex::new(0), params, variable_height_calculator);

    // Initial state: collapsed
    let initial = view_state.get(EntryIndex::new(0)).unwrap().is_expanded();
    assert!(!initial, "Should start collapsed");

    // Toggle 4 times (even number)
    for _ in 0..4 {
        view_state.toggle_expand(
            EntryIndex::new(0),
            params,
            viewport,
            variable_height_calculator,
        );
    }

    // Should be back to initial state
    let final_state = view_state.get(EntryIndex::new(0)).unwrap().is_expanded();
    assert_eq!(
        final_state, initial,
        "Even number of toggles should restore initial state"
    );
}
