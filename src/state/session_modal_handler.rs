//! Keyboard handler for session modal.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::state::{AppState, ViewedSession};

/// Handle keyboard input when session modal is visible.
///
/// Returns `true` if the key was consumed by the modal, `false` otherwise.
///
/// # Key Bindings
/// - Up/k: Select previous session (clamps at 0)
/// - Down/j: Select next session (clamps at last)
/// - Home/g: Jump to first session
/// - End/G: Jump to last session
/// - Enter: Confirm selection (sets viewed_session, closes modal)
/// - Esc: Cancel (closes modal without changing viewed_session)
/// - S (lowercase or uppercase): Toggle close (closes modal without changing viewed_session)
/// - 1-9: Quick select session N (if valid)
///
/// # Behavior
/// - Navigation clamps to bounds (does NOT wrap)
/// - Enter on last session sets ViewedSession::Latest (enables live tailing)
/// - Enter on non-last session sets ViewedSession::Pinned(idx)
/// - Returns false if modal not visible
pub fn handle_session_modal_key(state: &mut AppState, key: KeyEvent) -> bool {
    // Early return if modal not visible
    if !state.session_modal.is_visible() {
        return false;
    }

    let session_count = state.log_view().session_count();

    match key.code {
        // Close without changing viewed_session
        KeyCode::Esc => {
            state.session_modal.close();
            true
        }

        // S key (either case) closes without changing viewed_session
        KeyCode::Char('s') | KeyCode::Char('S') => {
            state.session_modal.close();
            true
        }

        // Navigate up
        KeyCode::Up | KeyCode::Char('k') => {
            state.session_modal.select_prev();
            true
        }

        // Navigate down
        KeyCode::Down | KeyCode::Char('j') => {
            state.session_modal.select_next(session_count);
            true
        }

        // Jump to first
        KeyCode::Home | KeyCode::Char('g') => {
            state.session_modal.select_first();
            true
        }

        // Jump to last (End key or Shift+G)
        KeyCode::End => {
            state.session_modal.select_last(session_count);
            true
        }
        KeyCode::Char('G') if key.modifiers.contains(KeyModifiers::SHIFT) => {
            state.session_modal.select_last(session_count);
            true
        }

        // Enter: Confirm selection
        KeyCode::Enter => {
            // Get validated session index
            if let Some(idx) = state.session_modal.selected_session_index(session_count) {
                // If selecting last session, switch to Latest mode (enables live tailing)
                // Otherwise, pin to specific session
                if idx.is_last(session_count) {
                    state.viewed_session = ViewedSession::Latest;
                } else {
                    state.viewed_session = ViewedSession::Pinned(idx);
                }

                // Update stats filter to reflect new session (cclv-463.5.5, AC-STATS-007)
                if let Some(session) = state.log_view().get_session(idx.get()) {
                    state.on_session_change(session.session_id().clone());
                }
            }
            // Close modal even if selection was invalid
            state.session_modal.close();
            true
        }

        // Quick select: 1-9 jumps to session N-1 (0-indexed)
        KeyCode::Char(c @ '1'..='9') => {
            let target_index = (c as usize) - ('1' as usize);
            // Only change selection if target is valid
            if target_index < session_count {
                // Manually set selection by navigating to it
                state.session_modal.select_first();
                for _ in 0..target_index {
                    state.session_modal.select_next(session_count);
                }
            }
            true
        }

        // Unhandled keys
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::ViewedSession;
    use crossterm::event::{KeyCode, KeyModifiers};

    /// Helper to create a test AppState with a given number of sessions.
    fn create_test_state(session_count: usize) -> AppState {
        let mut state = AppState::new();

        // Create dummy sessions
        for i in 0..session_count {
            let session_id = crate::model::SessionId::new(format!("session-{}", i))
                .expect("Failed to create test session ID");
            state.log_view_mut().create_empty_session(session_id);
        }

        // Open the modal at the first session
        state.session_modal.open(0);

        state
    }

    /// Helper to create a KeyEvent
    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::empty())
    }

    mod when_modal_not_visible {
        use super::*;

        #[test]
        fn returns_false_for_any_key() {
            let mut state = create_test_state(3);
            state.session_modal.close();

            let result = handle_session_modal_key(&mut state, key(KeyCode::Up));
            assert!(!result, "Should return false when modal not visible");
        }

        #[test]
        fn does_not_change_state() {
            let mut state = create_test_state(3);
            state.session_modal.close();

            let original_viewed = state.viewed_session;

            handle_session_modal_key(&mut state, key(KeyCode::Enter));

            assert_eq!(
                state.viewed_session, original_viewed,
                "Should not change viewed_session when modal closed"
            );
        }
    }

    mod navigation_up {
        use super::*;

        #[test]
        fn moves_selection_up_from_middle() {
            let mut state = create_test_state(5);
            state.session_modal.open(2); // Start at index 2

            let result = handle_session_modal_key(&mut state, key(KeyCode::Up));

            assert!(result, "Should return true");
            assert_eq!(state.session_modal.selected_index(), 1);
        }

        #[test]
        fn clamps_at_zero_does_not_wrap() {
            let mut state = create_test_state(5);
            state.session_modal.open(0); // Start at first

            let result = handle_session_modal_key(&mut state, key(KeyCode::Up));

            assert!(result, "Should return true");
            assert_eq!(state.session_modal.selected_index(), 0, "Should clamp at 0");
        }

        #[test]
        fn k_key_also_moves_up() {
            let mut state = create_test_state(5);
            state.session_modal.open(2);

            let result = handle_session_modal_key(&mut state, key(KeyCode::Char('k')));

            assert!(result, "Should return true");
            assert_eq!(state.session_modal.selected_index(), 1);
        }
    }

    mod navigation_down {
        use super::*;

        #[test]
        fn moves_selection_down_from_middle() {
            let mut state = create_test_state(5);
            state.session_modal.open(2);

            let result = handle_session_modal_key(&mut state, key(KeyCode::Down));

            assert!(result, "Should return true");
            assert_eq!(state.session_modal.selected_index(), 3);
        }

        #[test]
        fn clamps_at_last_does_not_wrap() {
            let mut state = create_test_state(5);
            state.session_modal.open(4); // Last session (index 4)

            let result = handle_session_modal_key(&mut state, key(KeyCode::Down));

            assert!(result, "Should return true");
            assert_eq!(
                state.session_modal.selected_index(),
                4,
                "Should clamp at last"
            );
        }

        #[test]
        fn j_key_also_moves_down() {
            let mut state = create_test_state(5);
            state.session_modal.open(2);

            let result = handle_session_modal_key(&mut state, key(KeyCode::Char('j')));

            assert!(result, "Should return true");
            assert_eq!(state.session_modal.selected_index(), 3);
        }
    }

    mod navigation_home_end {
        use super::*;

        #[test]
        fn home_jumps_to_first_session() {
            let mut state = create_test_state(5);
            state.session_modal.open(3);

            let result = handle_session_modal_key(&mut state, key(KeyCode::Home));

            assert!(result, "Should return true");
            assert_eq!(state.session_modal.selected_index(), 0);
        }

        #[test]
        fn g_key_jumps_to_first_session() {
            let mut state = create_test_state(5);
            state.session_modal.open(3);

            let result = handle_session_modal_key(&mut state, key(KeyCode::Char('g')));

            assert!(result, "Should return true");
            assert_eq!(state.session_modal.selected_index(), 0);
        }

        #[test]
        fn end_jumps_to_last_session() {
            let mut state = create_test_state(5);
            state.session_modal.open(1);

            let result = handle_session_modal_key(&mut state, key(KeyCode::End));

            assert!(result, "Should return true");
            assert_eq!(state.session_modal.selected_index(), 4);
        }

        #[test]
        fn uppercase_g_jumps_to_last_session() {
            let mut state = create_test_state(5);
            state.session_modal.open(1);

            let result = handle_session_modal_key(
                &mut state,
                KeyEvent::new(KeyCode::Char('G'), KeyModifiers::SHIFT),
            );

            assert!(result, "Should return true");
            assert_eq!(state.session_modal.selected_index(), 4);
        }
    }

    mod enter_key_selection {
        use super::*;

        #[test]
        fn enter_on_first_session_pins_to_first() {
            let mut state = create_test_state(3);
            state.session_modal.open(0);

            let result = handle_session_modal_key(&mut state, key(KeyCode::Enter));

            assert!(result, "Should return true");
            match state.viewed_session {
                ViewedSession::Pinned(idx) => {
                    assert_eq!(idx.get(), 0, "Should pin to first session")
                }
                ViewedSession::Latest => panic!("Should be Pinned, not Latest"),
            }
            assert!(!state.session_modal.is_visible(), "Modal should be closed");
        }

        #[test]
        fn enter_on_middle_session_pins_to_that_session() {
            let mut state = create_test_state(5);
            state.session_modal.open(2);

            let result = handle_session_modal_key(&mut state, key(KeyCode::Enter));

            assert!(result, "Should return true");
            match state.viewed_session {
                ViewedSession::Pinned(idx) => {
                    assert_eq!(idx.get(), 2, "Should pin to selected session")
                }
                ViewedSession::Latest => panic!("Should be Pinned, not Latest"),
            }
            assert!(!state.session_modal.is_visible(), "Modal should be closed");
        }

        #[test]
        fn enter_on_last_session_sets_latest_mode() {
            let mut state = create_test_state(5);
            state.session_modal.open(4); // Last session

            let result = handle_session_modal_key(&mut state, key(KeyCode::Enter));

            assert!(result, "Should return true");
            assert_eq!(
                state.viewed_session,
                ViewedSession::Latest,
                "Should set Latest mode when selecting last session"
            );
            assert!(!state.session_modal.is_visible(), "Modal should be closed");
        }

        #[test]
        fn enter_with_invalid_selection_closes_modal() {
            let mut state = create_test_state(3);
            // Manually set selected_index to invalid value (shouldn't happen, but defensive)
            state.session_modal.open(0);
            state.session_modal.select_last(10); // Forces index to 9, which is > session_count

            let result = handle_session_modal_key(&mut state, key(KeyCode::Enter));

            assert!(result, "Should return true");
            assert!(
                !state.session_modal.is_visible(),
                "Modal should be closed even with invalid selection"
            );
        }
    }

    mod escape_and_close {
        use super::*;

        #[test]
        fn esc_closes_modal_without_changing_viewed_session() {
            let mut state = create_test_state(5);
            state.viewed_session = ViewedSession::Latest;
            state.session_modal.open(2);

            let result = handle_session_modal_key(&mut state, key(KeyCode::Esc));

            assert!(result, "Should return true");
            assert_eq!(
                state.viewed_session,
                ViewedSession::Latest,
                "Should not change viewed_session"
            );
            assert!(!state.session_modal.is_visible(), "Modal should be closed");
        }

        #[test]
        fn lowercase_s_closes_modal_without_changing_viewed_session() {
            let mut state = create_test_state(5);
            state.viewed_session = ViewedSession::Latest;
            state.session_modal.open(2);

            let result = handle_session_modal_key(&mut state, key(KeyCode::Char('s')));

            assert!(result, "Should return true");
            assert_eq!(
                state.viewed_session,
                ViewedSession::Latest,
                "Should not change viewed_session"
            );
            assert!(!state.session_modal.is_visible(), "Modal should be closed");
        }

        #[test]
        fn uppercase_s_closes_modal_without_changing_viewed_session() {
            let mut state = create_test_state(5);
            state.viewed_session = ViewedSession::Latest;
            state.session_modal.open(2);

            let result = handle_session_modal_key(
                &mut state,
                KeyEvent::new(KeyCode::Char('S'), KeyModifiers::SHIFT),
            );

            assert!(result, "Should return true");
            assert_eq!(
                state.viewed_session,
                ViewedSession::Latest,
                "Should not change viewed_session"
            );
            assert!(!state.session_modal.is_visible(), "Modal should be closed");
        }
    }

    mod quick_select {
        use super::*;

        #[test]
        fn key_1_selects_first_session() {
            let mut state = create_test_state(5);
            state.session_modal.open(3);

            let result = handle_session_modal_key(&mut state, key(KeyCode::Char('1')));

            assert!(result, "Should return true");
            assert_eq!(
                state.session_modal.selected_index(),
                0,
                "Key 1 should select session at index 0"
            );
        }

        #[test]
        fn key_5_selects_fifth_session() {
            let mut state = create_test_state(10);
            state.session_modal.open(0);

            let result = handle_session_modal_key(&mut state, key(KeyCode::Char('5')));

            assert!(result, "Should return true");
            assert_eq!(
                state.session_modal.selected_index(),
                4,
                "Key 5 should select session at index 4"
            );
        }

        #[test]
        fn key_9_selects_ninth_session() {
            let mut state = create_test_state(10);
            state.session_modal.open(0);

            let result = handle_session_modal_key(&mut state, key(KeyCode::Char('9')));

            assert!(result, "Should return true");
            assert_eq!(
                state.session_modal.selected_index(),
                8,
                "Key 9 should select session at index 8"
            );
        }

        #[test]
        fn out_of_range_quick_select_does_nothing() {
            let mut state = create_test_state(3); // Only 3 sessions
            state.session_modal.open(1);

            let result = handle_session_modal_key(&mut state, key(KeyCode::Char('5')));

            assert!(result, "Should return true (key consumed)");
            assert_eq!(
                state.session_modal.selected_index(),
                1,
                "Selection should not change for out-of-range quick select"
            );
        }

        #[test]
        fn key_0_is_not_quick_select() {
            let mut state = create_test_state(5);
            state.session_modal.open(2);

            let result = handle_session_modal_key(&mut state, key(KeyCode::Char('0')));

            assert!(!result, "Should return false (key not consumed)");
            assert_eq!(
                state.session_modal.selected_index(),
                2,
                "Selection should not change for key 0"
            );
        }
    }

    mod unhandled_keys {
        use super::*;

        #[test]
        fn random_letter_returns_false() {
            let mut state = create_test_state(5);
            state.session_modal.open(2);

            let result = handle_session_modal_key(&mut state, key(KeyCode::Char('x')));

            assert!(!result, "Should return false for unhandled key");
            assert_eq!(
                state.session_modal.selected_index(),
                2,
                "Selection should not change"
            );
        }

        #[test]
        fn space_key_returns_false() {
            let mut state = create_test_state(5);
            state.session_modal.open(2);

            let result = handle_session_modal_key(&mut state, key(KeyCode::Char(' ')));

            assert!(!result, "Should return false for space key");
        }

        #[test]
        fn tab_key_returns_false() {
            let mut state = create_test_state(5);
            state.session_modal.open(2);

            let result = handle_session_modal_key(&mut state, key(KeyCode::Tab));

            assert!(!result, "Should return false for tab key");
        }
    }
}
