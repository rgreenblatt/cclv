//! Property-based invariant tests for the view-state layer.
//!
//! Tests validate the 14 core invariants specified in specs/002-view-state-layer/data-model.md:
//!
//! 1. LineHeight is always >= 1 for valid entries (ZERO for malformed)
//! 2. Cumulative Y is monotonically increasing
//! 3. Cumulative Y is sum of preceding heights
//! 4. Total height equals sum of all heights
//! 5. Scroll position resolution is bounded
//! 6. Visible range is within bounds
//! 7. Hit test index is valid when hit
//! 8. Lazy subagent initialization
//! 9. Session boundaries preserve order
//! 10. RenderCache key equality (TODO: implement when cache module is ready)
//! 11. Effective wrap mode semantics
//! 12. Focused message is valid index
//! 13. Toggle expand is idempotent pair
//! 14. Relayout from preserves cumulative_y invariant

#![allow(dead_code)] // Allow unused helper strategies

use cclv::model::{
    AgentId, ConversationEntry, EntryMetadata, EntryType, EntryUuid, LogEntry, MalformedEntry,
    Message, MessageContent, Role, SessionId,
};
use cclv::state::WrapMode;
use cclv::view_state::{
    conversation::ConversationViewState,
    entry_view::EntryView,
    hit_test::HitTestResult,
    layout_params::LayoutParams,
    log::LogViewState,
    scroll::ScrollPosition,
    session::SessionViewState,
    types::{EntryIndex, LineHeight, LineOffset, ViewportDimensions},
};
use chrono::Utc;
use proptest::prelude::*;

// ===== Arbitrary Strategies =====

/// Strategy for generating valid LineHeight values (1..=1000).
fn arb_line_height() -> impl Strategy<Value = LineHeight> {
    (1u16..=1000).prop_map(|h| LineHeight::new(h).unwrap())
}

/// Strategy for generating EntryIndex values (0..=100).
fn arb_entry_index() -> impl Strategy<Value = EntryIndex> {
    (0usize..=100).prop_map(EntryIndex::new)
}

/// Strategy for generating LineOffset values (0..=10000).
fn arb_line_offset() -> impl Strategy<Value = LineOffset> {
    (0usize..=10000).prop_map(LineOffset::new)
}

/// Strategy for generating WrapMode.
fn arb_wrap_mode() -> impl Strategy<Value = WrapMode> {
    prop_oneof![Just(WrapMode::Wrap), Just(WrapMode::NoWrap)]
}

/// Strategy for generating ViewportDimensions (1..=200 width/height).
fn arb_viewport() -> impl Strategy<Value = ViewportDimensions> {
    (1u16..=200, 1u16..=200).prop_map(|(w, h)| ViewportDimensions::new(w, h))
}

/// Strategy for generating a simple test ConversationEntry.
fn arb_conversation_entry() -> impl Strategy<Value = ConversationEntry> {
    // Generate valid entries only for most tests
    ("[a-z0-9-]{1,50}", "[a-zA-Z0-9 ,.!?]{1,200}").prop_map(|(uuid_str, text)| {
        let uuid = EntryUuid::new(uuid_str).unwrap();
        let session = SessionId::new("test-session").unwrap();
        let message = Message::new(Role::User, MessageContent::Text(text));
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
    })
}

/// Strategy for generating a list of ConversationEntry values.
fn arb_entry_list(max_len: usize) -> impl Strategy<Value = Vec<ConversationEntry>> {
    prop::collection::vec(arb_conversation_entry(), 0..=max_len)
}

/// Strategy for generating a malformed entry.
fn arb_malformed_entry() -> impl Strategy<Value = ConversationEntry> {
    (
        1usize..=1000,
        "[a-zA-Z0-9 ]{1,100}",
        "[a-zA-Z0-9 :,]{1,100}",
        prop::bool::ANY,
    )
        .prop_map(|(line_num, raw_line, error_msg, has_session)| {
            let session_id = if has_session {
                Some(SessionId::new("test-session").unwrap())
            } else {
                None
            };
            ConversationEntry::Malformed(MalformedEntry::new(
                line_num, raw_line, error_msg, session_id,
            ))
        })
}

/// Strategy for generating ScrollPosition variants.
fn arb_scroll_position() -> impl Strategy<Value = ScrollPosition> {
    prop_oneof![
        Just(ScrollPosition::Top),
        Just(ScrollPosition::Bottom),
        arb_line_offset().prop_map(ScrollPosition::AtLine),
        (arb_entry_index(), 0usize..=100).prop_map(|(idx, line)| ScrollPosition::AtEntry {
            entry_index: idx,
            line_in_entry: line
        }),
        (0.0f64..=1.0).prop_map(ScrollPosition::Fraction),
    ]
}

// ===== Helper: Simple Height Calculator =====

/// Stub height calculator for testing.
/// Returns a constant height of 3 lines for all valid entries, 0 for malformed.
fn simple_height_calculator(
    entry: &ConversationEntry,
    _expanded: bool,
    _wrap: WrapMode,
    _width: u16,
) -> LineHeight {
    match entry {
        ConversationEntry::Valid(_) => LineHeight::new(3).unwrap(),
        ConversationEntry::Malformed(_) => LineHeight::ZERO,
    }
}

// ===== Invariant 1: LineHeight is always >= 1 for valid entries =====

proptest! {
    #[test]
    fn line_height_valid_is_always_gte_1(h in 1u16..=65535) {
        let height = LineHeight::new(h).unwrap();
        prop_assert!(height.get() >= 1, "Valid LineHeight must be >= 1");
    }
}

#[test]
fn line_height_zero_is_rejected_by_smart_constructor() {
    let result = LineHeight::new(0);
    assert!(result.is_err(), "LineHeight::new(0) should fail");
}

#[test]
fn line_height_zero_sentinel_exists() {
    assert_eq!(LineHeight::ZERO.get(), 0, "ZERO sentinel should be 0");
    assert!(LineHeight::ZERO.is_zero(), "ZERO.is_zero() should be true");
}

proptest! {
    #[test]
    fn malformed_entries_always_get_zero_height(entry in arb_malformed_entry()) {
        let height = simple_height_calculator(&entry, false, WrapMode::Wrap, 80);
        prop_assert!(
            height.is_zero(),
            "Malformed entry height must be ZERO, got {}",
            height.get()
        );
    }
}

// ===== Invariant 2: Cumulative Y is monotonically increasing =====

proptest! {
    #[test]
    fn cumulative_y_monotonically_increasing(entries in arb_entry_list(50)) {
        if entries.is_empty() {
            return Ok(());
        }

        let mut state = ConversationViewState::new(None, None, entries);
        let params = LayoutParams::new(80, WrapMode::Wrap);
        state.relayout_from(EntryIndex::new(0), params);

        // Check monotonicity: forall i < j: entries[i].cumulative_y <= entries[j].cumulative_y
        for i in 0..state.len() {
            for j in i + 1..state.len() {
                let y_i = state.entry_cumulative_y(EntryIndex::new(i)).unwrap();
                let y_j = state.entry_cumulative_y(EntryIndex::new(j)).unwrap();

                prop_assert!(
                    y_i <= y_j,
                    "Cumulative Y not monotonic: entries[{}]={} > entries[{}]={}",
                    i, y_i.get(),
                    j, y_j.get()
                );
            }
        }
    }
}

// ===== Invariant 3: Cumulative Y is sum of preceding heights =====

proptest! {
    #[test]
    fn cumulative_y_equals_sum_of_preceding_heights(entries in arb_entry_list(50)) {
        if entries.is_empty() {
            return Ok(());
        }

        let mut state = ConversationViewState::new(None, None, entries);
        let params = LayoutParams::new(80, WrapMode::Wrap);
        state.relayout_from(EntryIndex::new(0), params);

        // Check: forall i: entries[i].cumulative_y == sum(entries[0..i].height)
        let mut cumulative = 0usize;
        for i in 0..state.len() {
            let entry = state.get(EntryIndex::new(i)).unwrap();
            let entry_y = state.entry_cumulative_y(EntryIndex::new(i)).unwrap();
            prop_assert_eq!(
                entry_y.get(),
                cumulative,
                "Entry {} cumulative_y {} != sum of preceding heights {}",
                i, entry_y.get(), cumulative
            );
            cumulative += entry.height().get() as usize;
        }
    }
}

// ===== Invariant 4: Total height equals sum of all heights =====

proptest! {
    #[test]
    fn total_height_equals_sum_of_all_heights(entries in arb_entry_list(50)) {
        let mut state = ConversationViewState::new(None, None, entries);
        let params = LayoutParams::new(80, WrapMode::Wrap);
        state.relayout_from(EntryIndex::new(0), params);

        let expected_total: usize = (0..state.len())
            .map(|i| {
                state.get(EntryIndex::new(i))
                    .unwrap()
                    .height()
                    .get() as usize
            })
            .sum();

        prop_assert_eq!(
            state.total_height(),
            expected_total,
            "Total height {} != sum of all heights {}",
            state.total_height(),
            expected_total
        );
    }
}

// ===== Invariant 5: Scroll position resolution is bounded =====

proptest! {
    #[test]
    fn scroll_position_resolution_bounded(
        entries in arb_entry_list(50),
        scroll in arb_scroll_position(),
        viewport in arb_viewport()
    ) {
        let mut state = ConversationViewState::new(None, None, entries);
        let params = LayoutParams::new(viewport.width, WrapMode::Wrap);
        state.relayout_from(EntryIndex::new(0), params);

        let total_height = state.total_height();
        let viewport_height = viewport.height as usize;

        let resolved = scroll.resolve(
            total_height,
            viewport_height,
            |idx| state.entry_cumulative_y(idx)
        );

        let max_offset = total_height.saturating_sub(viewport_height);

        prop_assert!(
            resolved.get() <= max_offset,
            "Scroll position resolved to {} but max offset is {}",
            resolved.get(),
            max_offset
        );
    }
}

// ===== Invariant 6: Visible range is within bounds =====

proptest! {
    #[test]
    fn visible_range_within_bounds(
        entries in arb_entry_list(50),
        viewport in arb_viewport()
    ) {
        let mut state = ConversationViewState::new(None, None, entries.clone());
        let params = LayoutParams::new(viewport.width, WrapMode::Wrap);
        state.relayout_from(EntryIndex::new(0), params);

        let visible = state.visible_range(viewport);

        prop_assert!(
            visible.start_index <= visible.end_index,
            "Visible range start {} > end {}",
            visible.start_index.get(),
            visible.end_index.get()
        );

        prop_assert!(
            visible.end_index.get() <= entries.len(),
            "Visible range end {} > entry count {}",
            visible.end_index.get(),
            entries.len()
        );
    }
}

// ===== Invariant 7: Hit test index is valid when hit =====

proptest! {
    #[test]
    fn hit_test_index_valid_when_hit(
        entries in arb_entry_list(50),
        screen_y in 0u16..=100,
        screen_x in 0u16..=200,
        scroll_offset in arb_line_offset()
    ) {
        if entries.is_empty() {
            return Ok(());
        }

        let mut state = ConversationViewState::new(None, None, entries.clone());
        let params = LayoutParams::new(80, WrapMode::Wrap);
        state.relayout_from(EntryIndex::new(0), params);

        let result = state.hit_test(screen_y, screen_x, scroll_offset);

        if let HitTestResult::Hit { entry_index, .. } = result {
            prop_assert!(
                entry_index.get() < entries.len(),
                "Hit test returned index {} but only {} entries exist",
                entry_index.get(),
                entries.len()
            );
        }
    }
}

// ===== Invariant 8: Lazy subagent initialization =====

proptest! {
    #[test]
    fn lazy_subagent_initialization(agent_name in "[a-z]{1,20}") {
        let session_id = SessionId::new("test-session").unwrap();
        let mut session = SessionViewState::new(session_id);

        let agent_id = AgentId::new(agent_name).unwrap();

        // Before accessing, subagent should not exist
        prop_assert!(
            !session.has_subagent(&agent_id),
            "Subagent should not exist before first access"
        );

        // Access subagent
        let _view = session.subagent(&agent_id);

        // After accessing, subagent should exist
        prop_assert!(
            session.has_subagent(&agent_id),
            "Subagent should exist after first access"
        );
    }
}

// ===== Invariant 9: Session boundaries preserve order =====

proptest! {
    #[test]
    fn session_boundaries_preserve_order(session_count in 1usize..=20) {
        let mut log = LogViewState::new();

        // Create sessions with sequential session IDs
        for i in 0..session_count {
            let session_id = SessionId::new(format!("session-{}", i)).unwrap();
            let entry_uuid = EntryUuid::new(format!("entry-{}", i)).unwrap();
            let message = Message::new(Role::User, MessageContent::Text("test".to_string()));
            let entry = LogEntry::new(
                entry_uuid,
                None,
                session_id,
                None,
                Utc::now(),
                EntryType::User,
                message,
                EntryMetadata::default(),
            );
            log.add_entry(ConversationEntry::Valid(Box::new(entry)), None);
        }

        // Verify session start_line monotonicity: forall i < j: sessions[i].start_line <= sessions[j].start_line
        for i in 0..log.session_count() {
            for j in i + 1..log.session_count() {
                let session_i = log.get_session(i).unwrap();
                let session_j = log.get_session(j).unwrap();

                prop_assert!(
                    session_i.start_line() <= session_j.start_line(),
                    "Session {} start_line {} > session {} start_line {}",
                    i, session_i.start_line(),
                    j, session_j.start_line()
                );
            }
        }
    }
}

// ===== Invariant 10: RenderCache key equality =====
// TODO: Implement when cache module is ready (cclv-5ur.2.11 or later)

// proptest! {
//     #[test]
//     fn render_cache_key_equality(
//         uuid_str in "[a-z0-9-]{1,50}",
//         width1 in 1u16..=200,
//         width2 in 1u16..=200,
//         expanded1 in prop::bool::ANY,
//         expanded2 in prop::bool::ANY,
//         wrap1 in arb_wrap_mode(),
//         wrap2 in arb_wrap_mode()
//     ) {
//         let uuid = EntryUuid::new(uuid_str).unwrap();
//
//         let key1 = RenderCacheKey::new(uuid.clone(), width1, expanded1, wrap1);
//         let key2 = RenderCacheKey::new(uuid.clone(), width2, expanded2, wrap2);
//
//         let should_be_equal = width1 == width2 && expanded1 == expanded2 && wrap1 == wrap2;
//
//         prop_assert_eq!(
//             key1 == key2,
//             should_be_equal,
//             "RenderCacheKey equality mismatch: key1={:?} key2={:?}",
//             key1, key2
//         );
//     }
// }

// ===== Invariant 11: Effective wrap mode semantics =====

proptest! {
    #[test]
    fn effective_wrap_mode_semantics(
        global in arb_wrap_mode(),
        has_override in prop::bool::ANY,
        override_mode in arb_wrap_mode()
    ) {
        let entry_uuid = EntryUuid::new("test-entry").unwrap();
        let message = Message::new(Role::User, MessageContent::Text("test".to_string()));
        let log_entry = LogEntry::new(
            entry_uuid,
            None,
            SessionId::new("test-session").unwrap(),
            None,
            Utc::now(),
            EntryType::User,
            message,
            EntryMetadata::default(),
        );

        let entry_view = EntryView::new(
            ConversationEntry::Valid(Box::new(log_entry)),
            EntryIndex::new(0)
        );

        // Note: set_wrap_override is pub(crate), so we can't call it from integration tests
        // This integration test can only verify the fallback behavior (no override set)
        // The full override behavior is tested in unit tests in entry_view.rs
        let _ = has_override; // Suppress unused variable warning
        let _ = override_mode;

        let effective = entry_view.effective_wrap(global);

        // Without override set, effective_wrap should return global
        prop_assert_eq!(
            effective,
            global,
            "Effective wrap mode should match global when no override is set: got {:?}, expected {:?}",
            effective, global
        );
    }
}

// ===== Invariant 12: Focused message is valid index =====

proptest! {
    #[test]
    fn focused_message_is_valid_index(
        entries in arb_entry_list(50),
        focus_index in 0usize..=100
    ) {
        if entries.is_empty() {
            return Ok(());
        }

        let mut state = ConversationViewState::new(None, None, entries.clone());

        // Set focus to an arbitrary index
        state.set_focused_message(Some(EntryIndex::new(focus_index)));

        if let Some(focused_idx) = state.focused_message() {
            prop_assert!(
                focused_idx.get() < entries.len(),
                "Focused message index {} >= entry count {}",
                focused_idx.get(),
                entries.len()
            );
        }
    }
}

// ===== Invariant 13: Toggle expand is idempotent pair =====

proptest! {
    #[test]
    fn toggle_expand_is_idempotent_pair(entries in arb_entry_list(5)) {
        if entries.is_empty() {
            return Ok(());
        }

        let mut state = ConversationViewState::new(None, None, entries);
        let params = LayoutParams::new(80, WrapMode::Wrap);
        let viewport = ViewportDimensions::new(80, 24);

        state.relayout_from(EntryIndex::new(0), params);

        // Get initial expanded state
        let original = state.get(EntryIndex::new(0)).unwrap().is_expanded();

        // Toggle twice using ConversationViewState API
        state
            .toggle_expand(EntryIndex::new(0), params, viewport)
            .expect("Should be able to toggle expand");
        state
            .toggle_expand(EntryIndex::new(0), params, viewport)
            .expect("Should be able to toggle expand");

        // Should be back to original state
        let final_state = state.get(EntryIndex::new(0)).unwrap().is_expanded();

        prop_assert_eq!(
            final_state,
            original,
            "Double toggle did not restore original state: original={}, after double toggle={}",
            original,
            final_state
        );
    }
}

// ===== Invariant 14: Relayout from preserves cumulative_y invariant =====

proptest! {
    #[test]
    fn relayout_from_preserves_cumulative_y_invariant(
        entries in arb_entry_list(50),
        relayout_from_idx in 0usize..=49
    ) {
        if entries.is_empty() {
            return Ok(());
        }

        let mut state = ConversationViewState::new(None, None, entries);
        let params = LayoutParams::new(80, WrapMode::Wrap);
        state.relayout_from(EntryIndex::new(0), params);

        // Pick a valid relayout index
        let from_index = EntryIndex::new(relayout_from_idx.min(state.len().saturating_sub(1)));

        // Relayout from this index
        state.relayout_from(from_index, params);

        // Verify invariant: forall j >= from_index:
        // entries[j].cumulative_y == entries[j-1].bottom_y() (or 0 if j==0)
        for j in from_index.get()..state.len() {
            let y_j = state.entry_cumulative_y(EntryIndex::new(j)).unwrap();

            let expected_cumulative_y = if j == 0 {
                0
            } else {
                let entry_prev = state.get(EntryIndex::new(j - 1)).unwrap();
                let y_prev = state.entry_cumulative_y(EntryIndex::new(j - 1)).unwrap();
                // bottom_y = cumulative_y + height
                y_prev.get() + entry_prev.height().get() as usize
            };

            prop_assert_eq!(
                y_j.get(),
                expected_cumulative_y,
                "Entry {} cumulative_y {} != expected {} after relayout_from({})",
                j,
                y_j.get(),
                expected_cumulative_y,
                from_index.get()
            );
        }
    }
}
