//! State for the session list modal.

use crate::view_state::types::SessionIndex;

/// State for the session list modal.
///
/// # Cardinality
/// - When closed: 1 state (visible = false)
/// - When open: session_count states (one per valid selection)
/// - Total: 1 + session_count states (all valid)
/// - Precision: 1.0
#[derive(Debug, Clone, Default)]
pub struct SessionModalState {
    /// Whether the modal is visible.
    visible: bool,

    /// Currently selected row in the modal (0-indexed).
    /// Only meaningful when `visible` is true.
    selected_index: usize,

    /// Scroll offset for long session lists.
    scroll_offset: usize,
}

impl SessionModalState {
    /// Create new modal state (closed).
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if modal is visible.
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Open the modal, pre-selecting the given session.
    pub fn open(&mut self, current_session_index: usize) {
        self.visible = true;
        self.selected_index = current_session_index;
        self.scroll_offset = 0;
    }

    /// Close the modal.
    pub fn close(&mut self) {
        self.visible = false;
    }

    /// Toggle modal visibility.
    pub fn toggle(&mut self, current_session_index: usize) {
        if self.visible {
            self.close();
        } else {
            self.open(current_session_index);
        }
    }

    /// Currently selected index.
    pub fn selected_index(&self) -> usize {
        self.selected_index
    }

    /// Move selection up, clamping at 0.
    pub fn select_prev(&mut self) {
        self.selected_index = self.selected_index.saturating_sub(1);
    }

    /// Move selection down, clamping at max.
    pub fn select_next(&mut self, session_count: usize) {
        if session_count > 0 {
            self.selected_index = (self.selected_index + 1).min(session_count - 1);
        }
    }

    /// Jump to first session.
    pub fn select_first(&mut self) {
        self.selected_index = 0;
    }

    /// Jump to last session.
    pub fn select_last(&mut self, session_count: usize) {
        if session_count > 0 {
            self.selected_index = session_count - 1;
        }
    }

    /// Get selected session index, validated against session count.
    pub fn selected_session_index(&self, session_count: usize) -> Option<SessionIndex> {
        SessionIndex::new(self.selected_index, session_count)
    }

    /// Scroll offset for rendering.
    pub fn scroll_offset(&self) -> usize {
        self.scroll_offset
    }

    /// Update scroll offset to keep selection visible.
    pub fn adjust_scroll(&mut self, visible_rows: usize) {
        if self.selected_index < self.scroll_offset {
            self.scroll_offset = self.selected_index;
        } else if self.selected_index >= self.scroll_offset + visible_rows {
            self.scroll_offset = self.selected_index - visible_rows + 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toggle_changes_visibility() {
        let mut modal = SessionModalState::new();
        assert!(!modal.is_visible());

        modal.toggle(0);
        assert!(modal.is_visible());

        modal.toggle(0);
        assert!(!modal.is_visible());
    }

    #[test]
    fn select_next_clamps_to_session_count_minus_one() {
        let mut modal = SessionModalState::new();
        modal.open(0);

        // Move to session 1
        modal.select_next(3);
        assert_eq!(modal.selected_index(), 1);

        // Move to session 2
        modal.select_next(3);
        assert_eq!(modal.selected_index(), 2);

        // Try to move beyond last session (should clamp)
        modal.select_next(3);
        assert_eq!(modal.selected_index(), 2);
    }

    #[test]
    fn selected_session_index_returns_validated_session_index() {
        let mut modal = SessionModalState::new();
        modal.open(1);

        // Valid index should return Some
        let result = modal.selected_session_index(3);
        assert!(result.is_some());
        assert_eq!(result.unwrap().get(), 1);

        // Manually set to out of bounds (simulating edge case)
        modal.select_last(10);
        let result = modal.selected_session_index(3);
        // Should return None because 9 >= 3
        assert!(result.is_none());
    }

    #[test]
    fn select_prev_clamps_at_zero() {
        let mut modal = SessionModalState::new();
        modal.open(2);

        // Should be at index 2
        assert_eq!(modal.selected_index(), 2);

        // Move to 1
        modal.select_prev();
        assert_eq!(modal.selected_index(), 1);

        // Move to 0
        modal.select_prev();
        assert_eq!(modal.selected_index(), 0);

        // Try to move below 0 (should clamp)
        modal.select_prev();
        assert_eq!(modal.selected_index(), 0);
    }

    #[test]
    fn adjust_scroll_keeps_selection_visible() {
        let mut modal = SessionModalState::new();
        modal.open(0);

        // Select index 5 with visible_rows = 3
        // This simulates moving down beyond visible area
        for _ in 0..5 {
            modal.select_next(10);
        }

        modal.adjust_scroll(3);

        // scroll_offset should be adjusted so that index 5 is visible
        // Expected: scroll_offset = 5 - 3 + 1 = 3
        assert_eq!(modal.scroll_offset(), 3);

        // Now move up to index 2
        modal.select_first();
        modal.select_next(10);
        modal.select_next(10);

        modal.adjust_scroll(3);

        // scroll_offset should adjust down to show index 2
        // Expected: scroll_offset = 2 (since 2 < 3)
        assert_eq!(modal.scroll_offset(), 2);
    }

    #[test]
    fn new_modal_starts_closed() {
        let modal = SessionModalState::new();
        assert!(!modal.is_visible());
    }

    #[test]
    fn open_sets_initial_selection() {
        let mut modal = SessionModalState::new();
        modal.open(5);

        assert!(modal.is_visible());
        assert_eq!(modal.selected_index(), 5);
        assert_eq!(modal.scroll_offset(), 0);
    }

    #[test]
    fn close_hides_modal() {
        let mut modal = SessionModalState::new();
        modal.open(0);
        assert!(modal.is_visible());

        modal.close();
        assert!(!modal.is_visible());
    }

    #[test]
    fn select_first_goes_to_zero() {
        let mut modal = SessionModalState::new();
        modal.open(5);

        modal.select_first();
        assert_eq!(modal.selected_index(), 0);
    }

    #[test]
    fn select_last_goes_to_session_count_minus_one() {
        let mut modal = SessionModalState::new();
        modal.open(0);

        modal.select_last(10);
        assert_eq!(modal.selected_index(), 9);
    }

    #[test]
    fn select_last_with_zero_sessions_stays_at_zero() {
        let mut modal = SessionModalState::new();
        modal.open(0);

        modal.select_last(0);
        assert_eq!(modal.selected_index(), 0);
    }

    #[test]
    fn select_next_with_zero_sessions_does_nothing() {
        let mut modal = SessionModalState::new();
        modal.open(0);

        let initial = modal.selected_index();
        modal.select_next(0);
        assert_eq!(modal.selected_index(), initial);
    }
}
