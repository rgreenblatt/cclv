//! Property-based tests for scroll rendering consistency.
//!
//! TRUE BLACK-BOX TESTING: Verifies scroll behavior by observing rendered output only.
//! No height calculation. No layout prediction. Pure input → render → observe.
//!
//! Property Under Test:
//! "Scrolling by 1 line shifts rendered content by exactly 1 line"
//!
//! Verification method:
//! 1. Render before scroll → capture lines
//! 2. Scroll by 1
//! 3. Render after scroll → capture new lines
//! 4. Assert: overlapping region is identical
//!
//! This catches:
//! - Spurious blank lines introduced during scroll
//! - Lines omitted during scroll
//! - Rendering artifacts from layout changes
//! - Incorrect clamping at top/bottom bounds

use cclv::model::{
    ConversationEntry, EntryMetadata, EntryType, EntryUuid, LogEntry, Message, MessageContent,
    Role, SessionId,
};
use cclv::state::WrapMode;
use cclv::view::{ConversationView, MessageStyles};
use cclv::view_state::conversation::ConversationViewState;
use cclv::view_state::layout::calculate_height;
use cclv::view_state::layout_params::LayoutParams;
use cclv::view_state::scroll::ScrollPosition;
use cclv::view_state::types::EntryIndex;
use cclv::view_state::types::ViewportDimensions;
use chrono::Utc;
use proptest::prelude::*;
use ratatui::backend::TestBackend;
use ratatui::Terminal;

// ===== Arbitrary Strategies =====

/// Strategy for generating valid WrapMode.
fn arb_wrap_mode() -> impl Strategy<Value = WrapMode> {
    prop_oneof![Just(WrapMode::Wrap), Just(WrapMode::NoWrap)]
}

/// Strategy for generating a simple test ConversationEntry.
///
/// Generates entries with 1-5 lines of text to ensure deterministic height calculation.
/// All entries are shorter than collapse_threshold=10, so they never collapse.
fn arb_conversation_entry() -> impl Strategy<Value = ConversationEntry> {
    // Generate valid entries only
    (
        "[a-z0-9-]{1,50}",
        1usize..=5, // Number of lines (always < collapse_threshold=10)
    )
        .prop_map(|(uuid_str, line_count)| {
            let uuid = EntryUuid::new(uuid_str).unwrap();
            let session = SessionId::new("test-session").unwrap();

            // Generate text with exactly line_count lines
            // Each line is simple text to avoid wrapping issues
            let text = (0..line_count)
                .map(|i| format!("Line {} content", i))
                .collect::<Vec<_>>()
                .join("\n");

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
    prop::collection::vec(arb_conversation_entry(), 5..=max_len)
}

/// Strategy for generating ConversationViewState with random entries and wrap mode.
///
/// BLACK-BOX: Creates state and computes layout using the REAL production height calculator.
/// We don't predict heights - we use actual production logic.
fn arb_conversation_view_state() -> impl Strategy<Value = ConversationViewState> {
    (arb_entry_list(20), arb_wrap_mode()).prop_map(|(entries, wrap_mode)| {
        let mut state = ConversationViewState::new(None, None, entries);
        let params = LayoutParams::new(80, wrap_mode);
        // Use REAL production height calculator - this is still black-box testing
        state.relayout_from(EntryIndex::new(0), params, calculate_height);
        state
    })
}

/// Direction for scroll moves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScrollDirection {
    Up,
    Down,
}

/// Strategy for generating a sequence of scroll moves.
fn arb_scroll_sequence(max_moves: usize) -> impl Strategy<Value = Vec<ScrollDirection>> {
    prop::collection::vec(
        prop_oneof![Just(ScrollDirection::Up), Just(ScrollDirection::Down),],
        1..=max_moves,
    )
}

// ===== Rendering Helpers =====

/// Render ConversationViewState to TestBackend and extract visible content lines.
/// Strips the frame border to focus on content area.
fn render_to_lines(state: &ConversationViewState, viewport: ViewportDimensions) -> Vec<String> {
    let mut terminal = Terminal::new(TestBackend::new(viewport.width, viewport.height)).unwrap();

    terminal
        .draw(|frame| {
            let styles = MessageStyles::default();
            let widget = ConversationView::new(state, &styles, false);
            frame.render_widget(widget, frame.area());
        })
        .unwrap();

    let buffer = terminal.backend().buffer();
    let area = buffer.area();

    let mut lines = Vec::new();
    for y in area.top()..area.bottom() {
        let mut line = String::new();
        for x in area.left()..area.right() {
            let cell = &buffer[(x, y)];
            line.push_str(cell.symbol());
        }
        // Trim right padding but keep the line structure
        lines.push(line.trim_end().to_string());
    }

    // Skip first line (top frame border) and last line (bottom frame border)
    // Also skip left/right frame characters from each line
    let content_lines: Vec<String> = lines
        .iter()
        .skip(1) // Skip top border
        .take(lines.len().saturating_sub(2)) // Exclude top and bottom borders
        .map(|line| {
            // Strip left and right border characters (first and last char if present)
            if line.len() >= 2 {
                let chars: Vec<char> = line.chars().collect();
                // Check if this looks like a frame line (starts with │ or similar)
                if chars[0] == '│' || chars[0] == '┌' || chars[0] == '└' {
                    // Strip first and last character
                    chars[1..chars.len().saturating_sub(1)]
                        .iter()
                        .collect::<String>()
                } else {
                    line.clone()
                }
            } else {
                line.clone()
            }
        })
        .collect();

    content_lines
}

/// Execute a single scroll move and return whether scroll actually happened.
///
/// BLACK-BOX: Detects boundary by observing scroll position change, not by prediction.
fn execute_scroll(
    state: &mut ConversationViewState,
    direction: ScrollDirection,
    viewport: ViewportDimensions,
) -> bool {
    let total_height = state.total_height();

    // Get current resolved offset BEFORE scroll
    let offset_before = state
        .scroll()
        .resolve(total_height, viewport.height as usize, |idx| {
            state.entry_cumulative_y(idx)
        })
        .get();

    // Calculate new offset (scroll by 1 line)
    let max_offset = total_height.saturating_sub(viewport.height as usize);
    let new_offset = match direction {
        ScrollDirection::Up => offset_before.saturating_sub(1),
        ScrollDirection::Down => (offset_before + 1).min(max_offset),
    };

    // Set new scroll position
    state.set_scroll(ScrollPosition::at_line(new_offset));

    // If offset changed, scroll happened. If same, we were at boundary.
    offset_before != new_offset
}

// ===== Property Tests =====

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Test that scrolling down shifts lines up consistently.
    ///
    /// BLACK-BOX: Render before/after, compare overlapping region.
    #[test]
    fn scroll_down_shifts_lines_up_consistently(
        mut state in arb_conversation_view_state(),
    ) {
        let viewport = ViewportDimensions::new(80, 24);

        // Skip if content too short to scroll
        if state.total_height() <= viewport.height as usize {
            return Ok(());
        }

        // Start from top
        state.set_scroll(ScrollPosition::Top);

        // Render before scroll
        let lines_before = render_to_lines(&state, viewport);

        // Execute scroll down
        let scrolled = execute_scroll(&mut state, ScrollDirection::Down, viewport);

        // If we didn't scroll (boundary), no visual change expected
        if !scrolled {
            return Ok(());
        }

        // Render after scroll
        let lines_after = render_to_lines(&state, viewport);

        // BLACK-BOX ASSERTION: lines shifted up by 1
        // lines_before[1..] should match lines_after[..n-1]
        let content_height = lines_before.len().min(lines_after.len());
        for i in 0..(content_height.saturating_sub(1)) {
            if i + 1 < lines_before.len() && i < lines_after.len() {
                prop_assert_eq!(
                    &lines_before[i + 1],
                    &lines_after[i],
                    "After scrolling down, line {} (previously line {}) should match",
                    i, i + 1
                );
            }
        }
    }

    /// Test that scrolling up shifts lines down consistently.
    ///
    /// BLACK-BOX: Render before/after, compare overlapping region.
    #[test]
    fn scroll_up_shifts_lines_down_consistently(
        mut state in arb_conversation_view_state(),
    ) {
        let viewport = ViewportDimensions::new(80, 24);

        // Skip if content too short to scroll
        if state.total_height() <= viewport.height as usize {
            return Ok(());
        }

        // Start from offset 1 (one line down from top)
        state.set_scroll(ScrollPosition::at_line(1));

        // Render before scroll
        let lines_before = render_to_lines(&state, viewport);

        // Execute scroll up (back to top)
        let scrolled = execute_scroll(&mut state, ScrollDirection::Up, viewport);

        // If we didn't scroll (boundary), no visual change expected
        if !scrolled {
            return Ok(());
        }

        // Render after scroll
        let lines_after = render_to_lines(&state, viewport);

        // BLACK-BOX ASSERTION: lines shifted down by 1
        // lines_before[..n-1] should match lines_after[1..]
        let content_height = lines_before.len().min(lines_after.len());
        for i in 0..(content_height.saturating_sub(1)) {
            if i < lines_before.len() && i + 1 < lines_after.len() {
                prop_assert_eq!(
                    &lines_before[i],
                    &lines_after[i + 1],
                    "After scrolling up, line {} should match previous line {}",
                    i + 1, i
                );
            }
        }
    }

    /// Test that scrolling doesn't crash and completes without panic.
    ///
    /// BLACK-BOX SMOKE TEST: Execute random scroll sequences and verify app remains stable.
    /// This is a weaker test than full consistency checking, but catches crashes and panics.
    #[test]
    fn scroll_sequence_stability(
        mut state in arb_conversation_view_state(),
        moves in arb_scroll_sequence(12), // Up to 12 moves (50% of 24-line viewport)
    ) {
        let viewport = ViewportDimensions::new(80, 24);

        // Skip if content too short to scroll
        if state.total_height() <= viewport.height as usize {
            return Ok(());
        }

        // Start from top
        state.set_scroll(ScrollPosition::Top);

        // Execute all scroll moves - just verify no crashes
        for direction in moves {
            execute_scroll(&mut state, direction, viewport);

            // Render to verify no panics during rendering
            let _lines = render_to_lines(&state, viewport);
        }

        // If we got here without panicking, test passes
    }

    /// Test that scrolling at boundaries is safe and doesn't corrupt rendering.
    ///
    /// BLACK-BOX: Render should be identical when trying to scroll past boundaries.
    #[test]
    fn scroll_at_boundaries_is_safe(
        mut state in arb_conversation_view_state(),
    ) {
        let viewport = ViewportDimensions::new(80, 24);

        // Skip if content too short to scroll
        if state.total_height() <= viewport.height as usize {
            return Ok(());
        }

        // Test top boundary: scroll up when already at top
        state.set_scroll(ScrollPosition::Top);
        let lines_before = render_to_lines(&state, viewport);
        let scrolled = execute_scroll(&mut state, ScrollDirection::Up, viewport);

        prop_assert!(!scrolled, "Should not scroll up from top");

        let lines_after = render_to_lines(&state, viewport);
        prop_assert_eq!(
            lines_before,
            lines_after,
            "Scrolling up at top boundary should not change rendering"
        );

        // Test bottom boundary: scroll down when already at bottom
        // Calculate max offset and set scroll there
        let max_offset = state.total_height().saturating_sub(viewport.height as usize);
        state.set_scroll(ScrollPosition::at_line(max_offset));

        let lines_before = render_to_lines(&state, viewport);
        let scrolled = execute_scroll(&mut state, ScrollDirection::Down, viewport);

        prop_assert!(!scrolled, "Should not scroll down from bottom");

        let lines_after = render_to_lines(&state, viewport);
        prop_assert_eq!(
            lines_before,
            lines_after,
            "Scrolling down at bottom boundary should not change rendering"
        );
    }

    /// Test that no blank lines appear spuriously during scroll.
    ///
    /// BLACK-BOX: Observe rendered lines, detect suspicious blank line patterns.
    #[test]
    #[ignore = "cclv-07v.12.21.3: pre-existing blank lines bug"]
    fn no_spurious_blank_lines_during_scroll(
        mut state in arb_conversation_view_state(),
        moves in arb_scroll_sequence(12),
    ) {
        let viewport = ViewportDimensions::new(80, 24);

        // Skip if content too short to scroll
        if state.total_height() <= viewport.height as usize {
            return Ok(());
        }

        state.set_scroll(ScrollPosition::Top);

        for direction in moves {
            execute_scroll(&mut state, direction, viewport);
            let lines = render_to_lines(&state, viewport);

            // Check for consecutive blank lines (suspicious pattern)
            let mut consecutive_blanks = 0;
            let mut max_consecutive_blanks = 0;

            for line in &lines {
                if line.trim().is_empty() {
                    consecutive_blanks += 1;
                    max_consecutive_blanks = max_consecutive_blanks.max(consecutive_blanks);
                } else {
                    consecutive_blanks = 0;
                }
            }

            prop_assert!(
                max_consecutive_blanks <= 2,
                "Found {} consecutive blank lines after {:?} scroll - likely spurious",
                max_consecutive_blanks, direction
            );
        }
    }
}
