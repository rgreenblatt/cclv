//! Acceptance tests for User Story 2: Expand/Collapse Entries
//!
//! Tests the 4 acceptance scenarios from spec.md lines 48-53:
//! 1. Collapsed entry expands and remains visible
//! 2. Expanded entry collapses with smooth shift
//! 3. Toggle response under 16ms
//! 4. Entries above viewport toggle without affecting visible entries

use cclv::model::{
    ConversationEntry, EntryMetadata, EntryType, EntryUuid, LogEntry, Message, MessageContent,
    PricingConfig, Role, SessionId,
};
use cclv::state::WrapMode;
use cclv::view_state::{
    conversation::ConversationViewState,
    layout_params::LayoutParams,
    scroll::ScrollPosition,
    types::{EntryIndex, ViewportDimensions},
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

/// Create a test entry with long text that will be collapsible.
/// The text must exceed 10 lines to trigger collapse behavior.
fn create_long_entry(uuid_str: &str) -> ConversationEntry {
    // Create text with 15 lines (exceeds COLLAPSE_THRESHOLD of 10)
    let long_text = (0..15)
        .map(|i| {
            format!(
                "This is line {} of a long message that will be collapsible",
                i
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    create_test_entry(uuid_str, &long_text)
}

// ===== US2 Scenario 1: Expand Collapsed Entry, Remains Visible =====

#[test]
fn us2_scenario1_expand_collapsed_entry_remains_visible() {
    // GIVEN: A collapsed entry visible in viewport
    // WHEN: User presses Enter/Space to expand
    // THEN: Entry expands and remains visible

    // Create conversation with 5 entries (using long text so they're collapsible)
    let entries = vec![
        create_long_entry("entry-0"),
        create_long_entry("entry-1"),
        create_long_entry("entry-2"), // target entry
        create_long_entry("entry-3"),
        create_long_entry("entry-4"),
    ];

    let mut view_state =
        ConversationViewState::new(None, None, entries, 200_000, PricingConfig::default());
    let params = LayoutParams::new(80, WrapMode::Wrap);
    let viewport = ViewportDimensions::new(80, 24);

    // Initial layout - all collapsed
    view_state.relayout_from(EntryIndex::new(0), params);

    // Calculate expected height from actual entries (all collapsed)
    let expected_total: usize = view_state
        .entries()
        .iter()
        .map(|entry_view| entry_view.height().get() as usize)
        .sum();
    assert_eq!(
        view_state.total_height(),
        expected_total,
        "Initial total height should match sum of entry heights"
    );

    // Scroll to show entry 2
    let entry_2_y = view_state
        .entry_cumulative_y(EntryIndex::new(2))
        .unwrap()
        .get();
    view_state.set_scroll(ScrollPosition::at_line(entry_2_y));

    // Verify entry 2 is NOT expanded
    let entry_2 = view_state.get(EntryIndex::new(2)).unwrap();
    assert!(!entry_2.is_expanded(), "Entry 2 should start collapsed");

    // Calculate expected cumulative_y for entry 2 (sum of heights of entries 0 and 1)
    let entry_2_y_expected: usize = view_state
        .entries()
        .iter()
        .take(2)
        .map(|e| e.height().get() as usize)
        .sum();
    assert_eq!(
        view_state
            .entry_cumulative_y(EntryIndex::new(2))
            .unwrap()
            .get(),
        entry_2_y_expected,
        "Entry 2 cumulative_y should be sum of previous entry heights"
    );

    // Capture state before toggle
    let entry_2_height_before = entry_2.height().get();
    let total_height_before = view_state.total_height();
    let entry_2_y_before = view_state
        .entry_cumulative_y(EntryIndex::new(2))
        .unwrap()
        .get();
    let entry_3_y_before = view_state
        .entry_cumulative_y(EntryIndex::new(3))
        .unwrap()
        .get();

    // WHEN: Toggle expand on entry 2
    let result = view_state.toggle_expand(EntryIndex::new(2), params, viewport);

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

    // THEN: Entry 2 height changed (should be taller when expanded)
    let entry_2_height_after = entry_2_after.height().get();
    assert!(
        entry_2_height_after > entry_2_height_before,
        "Expanded entry should be taller than collapsed (was {}, now {})",
        entry_2_height_before,
        entry_2_height_after
    );

    // THEN: Total height increased by the height delta
    let height_delta = entry_2_height_after - entry_2_height_before;
    let expected_total_after = total_height_before + height_delta as usize;
    assert_eq!(
        view_state.total_height(),
        expected_total_after,
        "Total height should increase by height delta"
    );

    // THEN: Entry 2 remains at same cumulative_y (scroll stability)
    assert_eq!(
        view_state
            .entry_cumulative_y(EntryIndex::new(2))
            .unwrap()
            .get(),
        entry_2_y_before,
        "Entry 2 cumulative_y should remain stable"
    );

    // THEN: Following entries shifted down by height delta
    assert_eq!(
        view_state
            .entry_cumulative_y(EntryIndex::new(3))
            .unwrap()
            .get(),
        entry_3_y_before + height_delta as usize,
        "Entry 3 should shift down by height delta"
    );
}

// ===== US2 Scenario 2: Collapse Expanded Entry, Smooth Shift =====

#[test]
fn us2_scenario2_collapse_expanded_entry_smooth_shift() {
    // GIVEN: An expanded entry visible
    // WHEN: User collapses it
    // THEN: Following entries shift up smoothly without viewport jump

    let entries = vec![
        create_long_entry("entry-0"),
        create_long_entry("entry-1"), // will be expanded then collapsed
        create_long_entry("entry-2"),
        create_long_entry("entry-3"),
    ];

    let mut view_state =
        ConversationViewState::new(None, None, entries, 200_000, PricingConfig::default());
    let params = LayoutParams::new(80, WrapMode::Wrap);
    let viewport = ViewportDimensions::new(80, 24);

    // Initial layout
    view_state.relayout_from(EntryIndex::new(0), params);

    // Expand entry 1 first
    view_state
        .toggle_expand(EntryIndex::new(1), params, viewport)
        .expect("Should be able to toggle expand");

    // Verify entry 1 is expanded
    let entry_1_before = view_state.get(EntryIndex::new(1)).unwrap();
    assert!(entry_1_before.is_expanded(), "Entry 1 should be expanded");

    // Calculate expected total height after expansion
    let expected_total: usize = view_state
        .entries()
        .iter()
        .map(|e| e.height().get() as usize)
        .sum();
    assert_eq!(view_state.total_height(), expected_total);

    // Entry 2 should be at cumulative_y = sum of heights of entries 0 and 1
    let entry_2_y_expected: usize = view_state
        .entries()
        .iter()
        .take(2)
        .map(|e| e.height().get() as usize)
        .sum();
    assert_eq!(
        view_state
            .entry_cumulative_y(EntryIndex::new(2))
            .unwrap()
            .get(),
        entry_2_y_expected,
        "Entry 2 cumulative_y should be sum of previous entry heights"
    );

    // Capture heights before collapse
    let entry_1_height_before = entry_1_before.height().get();
    let total_height_before = view_state.total_height();
    let entry_2_y_before = view_state
        .entry_cumulative_y(EntryIndex::new(2))
        .unwrap()
        .get();

    // WHEN: Collapse entry 1
    let result = view_state.toggle_expand(EntryIndex::new(1), params, viewport);

    // THEN: Toggle succeeded, now collapsed
    assert_eq!(
        result,
        Some(false),
        "Toggle should return false (collapsed state)"
    );

    // THEN: Entry 1 is collapsed
    let entry_1_after = view_state.get(EntryIndex::new(1)).unwrap();
    assert!(!entry_1_after.is_expanded(), "Entry 1 should be collapsed");

    // THEN: Entry 1 height decreased
    let entry_1_height_after = entry_1_after.height().get();
    assert!(
        entry_1_height_after < entry_1_height_before,
        "Collapsed entry should be shorter than expanded (was {}, now {})",
        entry_1_height_before,
        entry_1_height_after
    );

    // THEN: Total height reduced by height delta
    let height_delta = entry_1_height_before - entry_1_height_after;
    let expected_total_after = total_height_before - height_delta as usize;
    assert_eq!(
        view_state.total_height(),
        expected_total_after,
        "Total height should decrease by height delta"
    );

    // THEN: Entry 2 shifted up smoothly
    let entry_2_y_after = view_state
        .entry_cumulative_y(EntryIndex::new(2))
        .unwrap()
        .get();
    assert_eq!(
        entry_2_y_after,
        entry_2_y_before - height_delta as usize,
        "Entry 2 should shift up by height delta"
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

    let mut view_state =
        ConversationViewState::new(None, None, entries, 200_000, PricingConfig::default());
    let params = LayoutParams::new(80, WrapMode::Wrap);
    let viewport = ViewportDimensions::new(80, 24);

    // Initial layout
    view_state.relayout_from(EntryIndex::new(0), params);

    // Measure toggle time
    let start = Instant::now();

    view_state
        .toggle_expand(EntryIndex::new(10), params, viewport)
        .expect("Should be able to toggle expand");

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

    let mut view_state =
        ConversationViewState::new(None, None, entries, 200_000, PricingConfig::default());
    let params = LayoutParams::new(80, WrapMode::Wrap);
    let viewport = ViewportDimensions::new(80, 24);

    // Initial layout - all collapsed (3 lines each)
    view_state.relayout_from(EntryIndex::new(0), params);

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

    // Record entry 6 position and entry 2 height before toggle
    let entry_6_y_before = view_state
        .entry_cumulative_y(EntryIndex::new(6))
        .unwrap()
        .get();
    let entry_2_height_before = view_state.get(EntryIndex::new(2)).unwrap().height().get();

    // WHEN: Toggle entry 2 (above viewport)
    view_state
        .toggle_expand(EntryIndex::new(2), params, viewport)
        .expect("Should be able to toggle expand");

    // THEN: Entry 2 is now expanded
    let entry_2 = view_state.get(EntryIndex::new(2)).unwrap();
    assert!(entry_2.is_expanded(), "Entry 2 should be expanded");

    // THEN: Entry 6 shifted down by height delta
    let entry_2_height_after = entry_2.height().get();
    let height_delta = entry_2_height_after - entry_2_height_before;
    assert_eq!(
        view_state
            .entry_cumulative_y(EntryIndex::new(6))
            .unwrap()
            .get(),
        entry_6_y_before + height_delta as usize,
        "Entry 6 should shift down by height delta"
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

    let mut view_state =
        ConversationViewState::new(None, None, entries, 200_000, PricingConfig::default());
    let params = LayoutParams::new(80, WrapMode::Wrap);
    let viewport = ViewportDimensions::new(80, 24);

    view_state.relayout_from(EntryIndex::new(0), params);

    // Try to toggle entry 999 (doesn't exist)
    let result = view_state.toggle_expand(EntryIndex::new(999), params, viewport);

    assert_eq!(
        result, None,
        "Toggle on non-existent entry should return None"
    );
}

// ===== Edge Case: Multiple Toggles Preserve Idempotence =====

#[test]
fn multiple_toggles_preserve_idempotence() {
    let entries = vec![create_test_entry("entry-0", "Test")];

    let mut view_state =
        ConversationViewState::new(None, None, entries, 200_000, PricingConfig::default());
    let params = LayoutParams::new(80, WrapMode::Wrap);
    let viewport = ViewportDimensions::new(80, 24);

    view_state.relayout_from(EntryIndex::new(0), params);

    // Initial state: collapsed
    let initial = view_state.get(EntryIndex::new(0)).unwrap().is_expanded();
    assert!(!initial, "Should start collapsed");

    // Toggle 4 times (even number)
    for _ in 0..4 {
        view_state
            .toggle_expand(EntryIndex::new(0), params, viewport)
            .expect("Should be able to toggle expand");
    }

    // Should be back to initial state
    let final_state = view_state.get(EntryIndex::new(0)).unwrap().is_expanded();
    assert_eq!(
        final_state, initial,
        "Even number of toggles should restore initial state"
    );
}
