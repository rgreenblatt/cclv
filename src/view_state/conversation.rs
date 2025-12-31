//! View-state for a single conversation

#![allow(dead_code)] // Will be used by tests and other modules

use super::{
    entry_view::EntryView,
    hit_test::HitTestResult,
    layout_params::LayoutParams,
    scroll::ScrollPosition,
    types::{EntryIndex, LineHeight, LineOffset, ViewportDimensions},
    visible_range::VisibleRange,
};
use crate::model::ConversationEntry;
use crate::state::app_state::WrapMode;

/// View-state for a single conversation.
///
/// Contains:
/// - Owned entries with computed layouts
/// - Current scroll position
/// - Cached total height
/// - Layout validity tracking
///
/// # Layout Computation (FR-020 to FR-024)
/// Layout is computed lazily on first render or explicitly.
/// Heights depend on viewport width, expand state, and wrap mode.
/// Cumulative Y offsets are maintained as running sum.
///
/// # Visible Range (FR-030, FR-031)
/// `visible_range()` uses binary search on cumulative_y for O(log n) lookup.
///
/// # Hit Testing (FR-040 to FR-043)
/// `hit_test()` uses binary search on cumulative_y for O(log n) lookup.
#[derive(Debug, Clone)]
pub struct ConversationViewState {
    /// Agent ID (None for main agent, Some(id) for subagents).
    agent_id: Option<crate::model::AgentId>,
    /// Model information (for display in title bar).
    model: Option<crate::model::ModelInfo>,
    /// Entries with computed layouts and per-entry view state.
    entries: Vec<EntryView>,
    /// Current scroll position.
    scroll: ScrollPosition,
    /// Cached total height in lines (sum of all entry heights).
    total_height: usize,
    /// Index of currently focused entry (for keyboard navigation).
    /// `None` means no specific entry is focused.
    focused_message: Option<EntryIndex>,
    /// Global parameters used for last layout computation.
    last_layout_params: Option<LayoutParams>,
    /// Horizontal scroll offset (number of characters scrolled right from left edge).
    /// Only relevant when line wrapping is disabled (FR-040).
    /// 0 means viewing from the leftmost column.
    horizontal_offset: u16,
}

impl ConversationViewState {
    /// Create new conversation view-state from entries.
    /// Layout is not computed until `recompute_layout` is called.
    pub fn new(
        agent_id: Option<crate::model::AgentId>,
        model: Option<crate::model::ModelInfo>,
        entries: Vec<ConversationEntry>,
    ) -> Self {
        let entry_views: Vec<EntryView> = entries
            .into_iter()
            .enumerate()
            .map(|(idx, entry)| EntryView::new(entry, EntryIndex::new(idx)))
            .collect();
        Self {
            agent_id,
            model,
            entries: entry_views,
            scroll: ScrollPosition::Top,
            total_height: 0,
            focused_message: None,
            last_layout_params: None,
            horizontal_offset: 0,
        }
    }

    /// Create empty conversation view-state for main agent.
    pub fn empty() -> Self {
        Self::new(None, None, Vec::new())
    }

    // === Focus Management ===

    /// Get focused entry index.
    pub fn focused_message(&self) -> Option<EntryIndex> {
        self.focused_message
    }

    /// Set focused entry index.
    /// Clamps to valid range if index >= len.
    pub fn set_focused_message(&mut self, index: Option<EntryIndex>) {
        self.focused_message = index.map(|i| {
            let max_idx = self.entries.len().saturating_sub(1);
            EntryIndex::new(i.get().min(max_idx))
        });
    }

    /// Get focused entry view, if any.
    pub fn focused_entry(&self) -> Option<&EntryView> {
        self.focused_message.and_then(|i| self.entries.get(i.get()))
    }

    /// Get mutable focused entry view, if any.
    pub fn focused_entry_mut(&mut self) -> Option<&mut EntryView> {
        self.focused_message
            .and_then(|i| self.entries.get_mut(i.get()))
    }

    /// Number of entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Get entry at index.
    pub fn get(&self, index: EntryIndex) -> Option<&EntryView> {
        self.entries.get(index.get())
    }

    /// Get slice of all entries.
    /// Used by mouse_handler to calculate entry clicks.
    pub fn entries(&self) -> &[EntryView] {
        &self.entries
    }

    /// Iterate over all entries.
    pub fn iter(&self) -> impl Iterator<Item = &EntryView> {
        self.entries.iter()
    }

    /// Get agent ID (None for main agent, Some(id) for subagents).
    pub fn agent_id(&self) -> Option<&crate::model::AgentId> {
        self.agent_id.as_ref()
    }

    /// Get model information.
    pub fn model(&self) -> Option<&crate::model::ModelInfo> {
        self.model.as_ref()
    }

    /// Get model ID from first system:init entry.
    #[deprecated(note = "Use model() instead - this is for backward compatibility")]
    pub fn model_id(&self) -> Option<&str> {
        self.system_metadata().and_then(|m| m.model.as_deref())
    }

    /// Get model display name from first system:init entry.
    #[deprecated(note = "Use model() instead - this is for backward compatibility")]
    pub fn model_name(&self) -> Option<&str> {
        self.model_id()
    }

    /// Get system metadata from first system:init entry.
    pub fn system_metadata(&self) -> Option<&crate::model::SystemMetadata> {
        self.entries
            .iter()
            .filter_map(|e| match e.entry() {
                crate::model::ConversationEntry::Valid(log_entry) => {
                    log_entry.as_ref().system_metadata()
                }
                _ => None,
            })
            .next()
    }

    /// Current scroll position.
    pub fn scroll(&self) -> &ScrollPosition {
        &self.scroll
    }

    /// Set scroll position.
    pub fn set_scroll(&mut self, position: ScrollPosition) {
        self.scroll = position;
    }

    // === Horizontal Scrolling ===

    /// Get horizontal scroll offset.
    pub fn horizontal_offset(&self) -> u16 {
        self.horizontal_offset
    }

    /// Set horizontal scroll offset.
    pub fn set_horizontal_offset(&mut self, offset: u16) {
        self.horizontal_offset = offset;
    }

    /// Scroll left by amount, saturating at 0.
    pub fn scroll_left(&mut self, amount: u16) {
        self.horizontal_offset = self.horizontal_offset.saturating_sub(amount);
    }

    /// Scroll right by amount.
    pub fn scroll_right(&mut self, amount: u16) {
        self.horizontal_offset = self.horizontal_offset.saturating_add(amount);
    }

    /// Total height in lines.
    pub fn total_height(&self) -> usize {
        self.total_height
    }

    /// Check if global layout params changed (width, global_wrap).
    /// Note: Per-entry state changes (expand, wrap_override) require
    /// targeted relayout via `relayout_entry` or `relayout_from`.
    pub fn needs_relayout(&self, params: &LayoutParams) -> bool {
        self.last_layout_params.as_ref() != Some(params)
    }

    /// Append new entries (streaming mode).
    /// New entries have default layout; call `recompute_layout` to update.
    pub fn append(&mut self, entries: Vec<ConversationEntry>) {
        let start_idx = self.entries.len();
        for (offset, entry) in entries.into_iter().enumerate() {
            self.entries
                .push(EntryView::new(entry, EntryIndex::new(start_idx + offset)));
        }
        // Invalidate layout
        self.last_layout_params = None;
    }

    /// Recompute layout for all entries.
    ///
    /// # Arguments
    /// - `params`: Current global layout parameters
    /// - `height_calculator`: Function to compute height for an entry
    ///   Receives: entry, expanded state, effective wrap mode
    pub fn recompute_layout<F>(&mut self, params: LayoutParams, height_calculator: F)
    where
        F: Fn(&ConversationEntry, bool, WrapMode) -> LineHeight,
    {
        use super::layout::EntryLayout;

        let mut cumulative_y = 0usize;

        for entry_view in &mut self.entries {
            let expanded = entry_view.is_expanded();
            let wrap = entry_view.effective_wrap(params.global_wrap);
            let height = height_calculator(entry_view.entry(), expanded, wrap);
            let layout = EntryLayout::new(height, LineOffset::new(cumulative_y));
            entry_view.set_layout(layout);
            cumulative_y += height.get() as usize;
        }

        self.total_height = cumulative_y;
        self.last_layout_params = Some(params);
    }

    /// Relayout from a specific entry index onward.
    /// Used after toggling expand/wrap on a single entry.
    /// More efficient than full relayout for single-entry changes.
    ///
    /// # Arguments
    /// - `from_index`: Index of first entry to relayout
    /// - `params`: Current global layout parameters
    /// - `height_calculator`: Function to compute height
    pub fn relayout_from<F>(
        &mut self,
        from_index: EntryIndex,
        params: LayoutParams,
        height_calculator: F,
    ) where
        F: Fn(&ConversationEntry, bool, WrapMode) -> LineHeight,
    {
        use super::layout::EntryLayout;

        let idx = from_index.get();
        if idx >= self.entries.len() {
            return;
        }

        // Get cumulative_y from previous entry (or 0 if from_index is 0)
        let mut cumulative_y = if idx == 0 {
            0
        } else {
            self.entries[idx - 1].layout().bottom_y().get()
        };

        for entry_view in &mut self.entries[idx..] {
            let expanded = entry_view.is_expanded();
            let wrap = entry_view.effective_wrap(params.global_wrap);
            let height = height_calculator(entry_view.entry(), expanded, wrap);
            let layout = EntryLayout::new(height, LineOffset::new(cumulative_y));
            entry_view.set_layout(layout);
            cumulative_y += height.get() as usize;
        }

        self.total_height = cumulative_y;
    }

    /// Toggle expand state for entry at index and relayout.
    /// Returns new expanded state, or None if index out of bounds.
    ///
    /// # Scroll Stability (US2 scenario 4)
    /// When toggling an entry above the viewport, the scroll position is adjusted
    /// to keep the currently visible entries stable using ScrollPosition::AtEntry.
    ///
    /// # Arguments
    /// - `index`: Entry to toggle
    /// - `params`: Layout parameters
    /// - `viewport`: Current viewport dimensions (needed for scroll stability)
    /// - `height_calculator`: Function to compute entry heights
    pub fn toggle_expand<F>(
        &mut self,
        index: EntryIndex,
        params: LayoutParams,
        viewport: ViewportDimensions,
        height_calculator: F,
    ) -> Option<bool>
    where
        F: Fn(&ConversationEntry, bool, WrapMode) -> LineHeight,
    {
        // Capture scroll anchor BEFORE toggling if entry is above viewport
        let scroll_anchor = self.compute_scroll_anchor_before_toggle(index, viewport);

        let entry = self.entries.get_mut(index.get())?;
        let new_state = entry.toggle_expanded();
        self.relayout_from(index, params, height_calculator);

        // Restore scroll stability if we had an anchor
        if let Some(anchor) = scroll_anchor {
            self.scroll = anchor;
        }

        Some(new_state)
    }

    /// Compute scroll anchor for preserving viewport stability when toggling entry.
    /// Returns Some(ScrollPosition::AtEntry) if toggled entry is above viewport.
    fn compute_scroll_anchor_before_toggle(
        &self,
        toggled_index: EntryIndex,
        viewport: ViewportDimensions,
    ) -> Option<ScrollPosition> {
        if self.entries.is_empty() {
            return None;
        }

        // Get current visible range
        let visible = self.visible_range(viewport);

        // If toggled entry is at or after first visible entry, no anchor needed
        // (viewport doesn't need adjustment for toggles within or below viewport)
        if toggled_index >= visible.start_index {
            return None;
        }

        // Entry is above viewport - anchor to first visible entry
        let first_visible = visible.start_index;
        let first_visible_y = self.get(first_visible)?.layout().cumulative_y().get();
        let scroll_offset = visible.scroll_offset.get();
        let line_in_entry = first_visible_y.saturating_sub(scroll_offset);

        Some(ScrollPosition::AtEntry {
            entry_index: first_visible,
            line_in_entry,
        })
    }

    /// Set wrap override for entry at index and relayout.
    /// Returns previous wrap override, or None if index out of bounds.
    pub fn set_wrap_override<F>(
        &mut self,
        index: EntryIndex,
        wrap: Option<WrapMode>,
        params: LayoutParams,
        height_calculator: F,
    ) -> Option<Option<WrapMode>>
    where
        F: Fn(&ConversationEntry, bool, WrapMode) -> LineHeight,
    {
        let entry = self.entries.get_mut(index.get())?;
        let previous = entry.wrap_override();
        entry.set_wrap_override(wrap);
        self.relayout_from(index, params, height_calculator);
        Some(previous)
    }

    /// Compute visible range using binary search.
    /// O(log n) complexity.
    ///
    /// # Arguments
    /// - `viewport`: Viewport dimensions
    ///
    /// # Returns
    /// Range of entry indices that are visible.
    pub fn visible_range(&self, viewport: ViewportDimensions) -> VisibleRange {
        if self.entries.is_empty() {
            return VisibleRange::default();
        }

        let scroll_offset =
            self.scroll
                .resolve(self.total_height, viewport.height as usize, |idx| {
                    self.entries
                        .get(idx.get())
                        .map(|e| e.layout().cumulative_y())
                });

        let scroll_line = scroll_offset.get();
        let viewport_bottom = scroll_line + viewport.height as usize;

        // Binary search for first visible entry
        // Find first entry whose bottom_y > scroll_line
        let start_index = self
            .entries
            .partition_point(|e| e.layout().bottom_y().get() <= scroll_line);

        // Binary search for first entry past viewport
        // Find first entry whose cumulative_y >= viewport_bottom
        let end_index = self
            .entries
            .partition_point(|e| e.layout().cumulative_y().get() < viewport_bottom);

        VisibleRange::new(
            EntryIndex::new(start_index),
            EntryIndex::new(end_index),
            scroll_offset,
            viewport.height,
        )
    }

    /// Hit-test a screen coordinate.
    /// O(log n) complexity.
    ///
    /// # Arguments
    /// - `screen_y`: Y coordinate relative to viewport top
    /// - `screen_x`: X coordinate
    /// - `scroll_offset`: Current scroll offset
    ///
    /// # Returns
    /// What entry (if any) was hit.
    pub fn hit_test(
        &self,
        screen_y: u16,
        screen_x: u16,
        scroll_offset: LineOffset,
    ) -> HitTestResult {
        if self.entries.is_empty() {
            return HitTestResult::miss();
        }

        let absolute_y = scroll_offset.get() + screen_y as usize;

        // Binary search for entry containing absolute_y
        // Find first entry whose bottom_y > absolute_y
        let index = self
            .entries
            .partition_point(|e| e.layout().bottom_y().get() <= absolute_y);

        if index >= self.entries.len() {
            return HitTestResult::miss();
        }

        let entry = &self.entries[index];
        let entry_y = entry.layout().cumulative_y().get();

        if absolute_y < entry_y {
            return HitTestResult::miss();
        }

        let line_in_entry = absolute_y - entry_y;

        HitTestResult::hit(EntryIndex::new(index), line_in_entry, screen_x)
    }

    /// Get cumulative_y for entry at index (for scroll resolution).
    pub fn entry_cumulative_y(&self, index: EntryIndex) -> Option<LineOffset> {
        self.entries
            .get(index.get())
            .map(|e| e.layout().cumulative_y())
    }

    /// Check if entry with given UUID is expanded.
    ///
    /// This is a compatibility helper for the view layer which still works with UUIDs.
    /// Returns false if no entry with this UUID is found.
    ///
    /// **Note**: This is O(n) lookup. The view layer should eventually be refactored
    /// to work with EntryIndex instead of UUID for expand state queries.
    pub fn is_expanded_by_uuid(&self, uuid: &crate::model::EntryUuid) -> bool {
        self.entries
            .iter()
            .find(|entry_view| {
                entry_view
                    .entry()
                    .uuid()
                    .map(|entry_uuid| entry_uuid == uuid)
                    .unwrap_or(false)
            })
            .map(|entry_view| entry_view.is_expanded())
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{
        EntryMetadata, EntryType, EntryUuid, LogEntry, MalformedEntry, Message, MessageContent,
        Role, SessionId,
    };

    // ===== Test Helpers =====

    fn make_session_id(s: &str) -> SessionId {
        SessionId::new(s).expect("valid session id")
    }

    fn make_entry_uuid(s: &str) -> EntryUuid {
        EntryUuid::new(s).expect("valid uuid")
    }

    fn make_timestamp() -> chrono::DateTime<chrono::Utc> {
        "2025-12-25T10:00:00Z".parse().expect("valid timestamp")
    }

    fn make_message(text: &str) -> Message {
        Message::new(Role::User, MessageContent::Text(text.to_string()))
    }

    fn make_valid_entry(uuid: &str) -> ConversationEntry {
        let log_entry = LogEntry::new(
            make_entry_uuid(uuid),
            None,
            make_session_id("session-1"),
            None,
            make_timestamp(),
            EntryType::User,
            make_message("Test message"),
            EntryMetadata::default(),
        );
        ConversationEntry::Valid(Box::new(log_entry))
    }

    fn make_malformed_entry() -> ConversationEntry {
        ConversationEntry::Malformed(MalformedEntry::new(
            42,
            "bad json",
            "Parse error",
            Some(make_session_id("session-1")),
        ))
    }

    // Mock height calculator: returns fixed height for testing
    fn fixed_height_calculator(
        height: u16,
    ) -> impl Fn(&ConversationEntry, bool, WrapMode) -> LineHeight {
        move |_entry, _expanded, _wrap| LineHeight::new(height).unwrap()
    }

    // === ConversationViewState::new Tests ===

    #[test]
    fn new_creates_view_state_from_entries() {
        let entries = vec![
            make_valid_entry("uuid-1"),
            make_valid_entry("uuid-2"),
            make_valid_entry("uuid-3"),
        ];

        let state = ConversationViewState::new(None, None, entries);

        assert_eq!(state.len(), 3, "Should have 3 entries");
        assert!(!state.is_empty(), "Should not be empty");
    }

    #[test]
    fn new_starts_with_no_layout() {
        let entries = vec![make_valid_entry("uuid-1")];
        let state = ConversationViewState::new(None, None, entries);

        assert_eq!(
            state.total_height(),
            0,
            "Total height should be 0 before layout"
        );
        assert!(
            state.last_layout_params.is_none(),
            "Should have no layout params until first layout"
        );
    }

    #[test]
    fn new_starts_scrolled_to_top() {
        let entries = vec![make_valid_entry("uuid-1")];
        let state = ConversationViewState::new(None, None, entries);

        assert_eq!(state.scroll(), &ScrollPosition::Top, "Should start at top");
    }

    #[test]
    fn new_starts_with_no_focused_message() {
        let entries = vec![make_valid_entry("uuid-1")];
        let state = ConversationViewState::new(None, None, entries);

        assert_eq!(
            state.focused_message(),
            None,
            "Should have no focused message initially"
        );
    }

    #[test]
    fn new_assigns_correct_indices() {
        let entries = vec![
            make_valid_entry("uuid-1"),
            make_valid_entry("uuid-2"),
            make_valid_entry("uuid-3"),
        ];
        let state = ConversationViewState::new(None, None, entries);

        assert_eq!(
            state.get(EntryIndex::new(0)).unwrap().index(),
            EntryIndex::new(0)
        );
        assert_eq!(
            state.get(EntryIndex::new(1)).unwrap().index(),
            EntryIndex::new(1)
        );
        assert_eq!(
            state.get(EntryIndex::new(2)).unwrap().index(),
            EntryIndex::new(2)
        );
    }

    // === ConversationViewState::empty Tests ===

    #[test]
    fn empty_creates_empty_state() {
        let state = ConversationViewState::empty();

        assert_eq!(state.len(), 0);
        assert!(state.is_empty());
    }

    // === recompute_layout Tests ===

    #[test]
    fn recompute_layout_sets_total_height() {
        let entries = vec![
            make_valid_entry("uuid-1"),
            make_valid_entry("uuid-2"),
            make_valid_entry("uuid-3"),
        ];
        let mut state = ConversationViewState::new(None, None, entries);

        let params = LayoutParams::new(80, WrapMode::Wrap);
        state.recompute_layout(params, fixed_height_calculator(5));

        assert_eq!(
            state.total_height(),
            15,
            "Total height should be 3 entries * 5 lines each"
        );
    }

    #[test]
    fn recompute_layout_maintains_cumulative_y_invariant() {
        let entries = vec![
            make_valid_entry("uuid-1"),
            make_valid_entry("uuid-2"),
            make_valid_entry("uuid-3"),
        ];
        let mut state = ConversationViewState::new(None, None, entries);

        let params = LayoutParams::new(80, WrapMode::Wrap);
        state.recompute_layout(params, fixed_height_calculator(5));

        // Verify cumulative_y invariant: cumulative_y[i] = sum(height[0..i])
        assert_eq!(
            state
                .get(EntryIndex::new(0))
                .unwrap()
                .layout()
                .cumulative_y()
                .get(),
            0
        );
        assert_eq!(
            state
                .get(EntryIndex::new(1))
                .unwrap()
                .layout()
                .cumulative_y()
                .get(),
            5
        );
        assert_eq!(
            state
                .get(EntryIndex::new(2))
                .unwrap()
                .layout()
                .cumulative_y()
                .get(),
            10
        );
    }

    #[test]
    fn recompute_layout_stores_params() {
        let entries = vec![make_valid_entry("uuid-1")];
        let mut state = ConversationViewState::new(None, None, entries);

        let params = LayoutParams::new(80, WrapMode::Wrap);
        state.recompute_layout(params, fixed_height_calculator(5));

        assert_eq!(state.last_layout_params, Some(params));
    }

    // === visible_range Tests (Binary Search) ===

    #[test]
    fn visible_range_empty_state_returns_empty_range() {
        let state = ConversationViewState::empty();
        let viewport = ViewportDimensions::new(80, 24);

        let range = state.visible_range(viewport);

        assert!(range.is_empty());
        assert_eq!(range.start_index, EntryIndex::new(0));
        assert_eq!(range.end_index, EntryIndex::new(0));
    }

    #[test]
    fn visible_range_from_top_shows_first_entries() {
        let entries = vec![
            make_valid_entry("uuid-1"),
            make_valid_entry("uuid-2"),
            make_valid_entry("uuid-3"),
            make_valid_entry("uuid-4"),
        ];
        let mut state = ConversationViewState::new(None, None, entries);

        let params = LayoutParams::new(80, WrapMode::Wrap);
        state.recompute_layout(params, fixed_height_calculator(10)); // Each entry is 10 lines
        state.set_scroll(ScrollPosition::Top);

        let viewport = ViewportDimensions::new(80, 24); // 24 line viewport

        let range = state.visible_range(viewport);

        // Viewport shows lines 0-23, entries are at y=[0, 10, 20, 30]
        // Entry 0: y=0..10 (visible)
        // Entry 1: y=10..20 (visible)
        // Entry 2: y=20..30 (partially visible, starts at line 20)
        // Entry 3: y=30..40 (not visible)
        assert_eq!(range.start_index, EntryIndex::new(0));
        assert_eq!(range.end_index, EntryIndex::new(3)); // Exclusive, so 0,1,2 are visible
    }

    #[test]
    fn visible_range_scrolled_shows_middle_entries() {
        let entries = vec![
            make_valid_entry("uuid-1"),
            make_valid_entry("uuid-2"),
            make_valid_entry("uuid-3"),
            make_valid_entry("uuid-4"),
            make_valid_entry("uuid-5"),
        ];
        let mut state = ConversationViewState::new(None, None, entries);

        let params = LayoutParams::new(80, WrapMode::Wrap);
        state.recompute_layout(params, fixed_height_calculator(10)); // Each entry is 10 lines
        state.set_scroll(ScrollPosition::AtLine(LineOffset::new(15)));

        let viewport = ViewportDimensions::new(80, 24); // Viewport shows lines 15-38

        let range = state.visible_range(viewport);

        // Entry 0: y=0..10 (not visible, ends before viewport)
        // Entry 1: y=10..20 (partially visible, overlaps viewport start)
        // Entry 2: y=20..30 (visible)
        // Entry 3: y=30..40 (partially visible, starts in viewport)
        // Entry 4: y=40..50 (not visible)
        assert_eq!(range.start_index, EntryIndex::new(1));
        assert_eq!(range.end_index, EntryIndex::new(4)); // 1,2,3 visible
    }

    #[test]
    fn visible_range_at_bottom_shows_last_entries() {
        let entries = vec![
            make_valid_entry("uuid-1"),
            make_valid_entry("uuid-2"),
            make_valid_entry("uuid-3"),
            make_valid_entry("uuid-4"),
        ];
        let mut state = ConversationViewState::new(None, None, entries);

        let params = LayoutParams::new(80, WrapMode::Wrap);
        state.recompute_layout(params, fixed_height_calculator(10)); // Total height = 40
        state.set_scroll(ScrollPosition::Bottom);

        let viewport = ViewportDimensions::new(80, 24); // Viewport shows lines 16-39 (40-24=16)

        let range = state.visible_range(viewport);

        // Entry 0: y=0..10 (not visible)
        // Entry 1: y=10..20 (partially visible, ends at line 20)
        // Entry 2: y=20..30 (visible)
        // Entry 3: y=30..40 (visible)
        assert_eq!(range.start_index, EntryIndex::new(1));
        assert_eq!(range.end_index, EntryIndex::new(4)); // 1,2,3 visible
    }

    // === relayout_from Tests ===

    #[test]
    fn relayout_from_updates_from_index_onward() {
        let entries = vec![
            make_valid_entry("uuid-1"),
            make_valid_entry("uuid-2"),
            make_valid_entry("uuid-3"),
        ];
        let mut state = ConversationViewState::new(None, None, entries);

        let params = LayoutParams::new(80, WrapMode::Wrap);
        state.recompute_layout(params, fixed_height_calculator(10));

        // Initial: [0, 10, 20], total=30
        // Change entry 1 to height 20
        state.entries[1].set_expanded(true); // Simulate expansion

        let variable_height = |_entry: &ConversationEntry, expanded: bool, _wrap: WrapMode| {
            if expanded {
                LineHeight::new(20).unwrap()
            } else {
                LineHeight::new(10).unwrap()
            }
        };

        state.relayout_from(EntryIndex::new(1), params, variable_height);

        // After relayout from 1: [0, 10, 30], total=40
        assert_eq!(
            state
                .get(EntryIndex::new(0))
                .unwrap()
                .layout()
                .cumulative_y()
                .get(),
            0
        );
        assert_eq!(
            state
                .get(EntryIndex::new(1))
                .unwrap()
                .layout()
                .cumulative_y()
                .get(),
            10
        );
        assert_eq!(
            state
                .get(EntryIndex::new(2))
                .unwrap()
                .layout()
                .cumulative_y()
                .get(),
            30
        ); // 10 + 20
        assert_eq!(state.total_height(), 40);
    }

    #[test]
    fn relayout_from_zero_is_equivalent_to_full_relayout() {
        let entries = vec![
            make_valid_entry("uuid-1"),
            make_valid_entry("uuid-2"),
            make_valid_entry("uuid-3"),
        ];

        let mut state1 = ConversationViewState::new(None, None, entries.clone());
        let mut state2 = ConversationViewState::new(None, None, entries);

        let params = LayoutParams::new(80, WrapMode::Wrap);

        state1.recompute_layout(params, fixed_height_calculator(10));
        state2.relayout_from(EntryIndex::new(0), params, fixed_height_calculator(10));

        // Both should produce identical layout
        assert_eq!(state1.total_height(), state2.total_height());
        for i in 0..3 {
            let idx = EntryIndex::new(i);
            assert_eq!(
                state1.get(idx).unwrap().layout().cumulative_y(),
                state2.get(idx).unwrap().layout().cumulative_y()
            );
        }
    }

    // === toggle_expand Tests ===

    #[test]
    fn toggle_expand_returns_new_state() {
        let entries = vec![make_valid_entry("uuid-1")];
        let mut state = ConversationViewState::new(None, None, entries);

        let params = LayoutParams::new(80, WrapMode::Wrap);
        state.recompute_layout(params, fixed_height_calculator(10));

        let viewport = ViewportDimensions::new(80, 24);
        let result = state.toggle_expand(
            EntryIndex::new(0),
            params,
            viewport,
            fixed_height_calculator(10),
        );

        assert_eq!(result, Some(true), "Should toggle to expanded");
        assert!(state.get(EntryIndex::new(0)).unwrap().is_expanded());
    }

    #[test]
    fn toggle_expand_returns_none_for_invalid_index() {
        let mut state = ConversationViewState::empty();
        let params = LayoutParams::new(80, WrapMode::Wrap);

        let viewport = ViewportDimensions::new(80, 24);
        let result = state.toggle_expand(
            EntryIndex::new(0),
            params,
            viewport,
            fixed_height_calculator(10),
        );

        assert_eq!(result, None);
    }

    #[test]
    fn toggle_expand_triggers_relayout() {
        let entries = vec![make_valid_entry("uuid-1"), make_valid_entry("uuid-2")];
        let mut state = ConversationViewState::new(None, None, entries);

        let params = LayoutParams::new(80, WrapMode::Wrap);
        let variable_height = |_entry: &ConversationEntry, expanded: bool, _wrap: WrapMode| {
            if expanded {
                LineHeight::new(20).unwrap()
            } else {
                LineHeight::new(10).unwrap()
            }
        };

        state.recompute_layout(params, variable_height);
        // Initial: [0, 10], total=20

        let viewport = ViewportDimensions::new(80, 24);
        state.toggle_expand(EntryIndex::new(0), params, viewport, variable_height);
        // After expanding entry 0: [0, 20], total=30

        assert_eq!(
            state
                .get(EntryIndex::new(0))
                .unwrap()
                .layout()
                .cumulative_y()
                .get(),
            0
        );
        assert_eq!(
            state
                .get(EntryIndex::new(1))
                .unwrap()
                .layout()
                .cumulative_y()
                .get(),
            20
        );
        assert_eq!(state.total_height(), 30);
    }

    #[test]
    fn toggle_expand_above_viewport_keeps_visible_entries_stable() {
        // Setup: Create 10 entries, each height 2 when collapsed, 5 when expanded
        let entries: Vec<ConversationEntry> = (0..10)
            .map(|i| make_valid_entry(&format!("uuid-{}", i)))
            .collect();
        let mut state = ConversationViewState::new(None, None, entries);

        let params = LayoutParams::new(80, WrapMode::Wrap);
        let variable_height = |_entry: &ConversationEntry, expanded: bool, _wrap: WrapMode| {
            if expanded {
                LineHeight::new(5).unwrap()
            } else {
                LineHeight::new(2).unwrap()
            }
        };

        state.recompute_layout(params, variable_height);
        // All collapsed: heights [2, 2, 2, ...], cumulative_y [0, 2, 4, 6, 8, 10, 12, 14, 16, 18]
        // Total height: 20

        // Scroll to show entries 5-7 (viewport height 10 lines)
        // Entry 5: y=10..12
        // Entry 6: y=12..14
        // Entry 7: y=14..16
        // Entry 8: y=16..18 (partially visible)
        state.set_scroll(ScrollPosition::AtLine(LineOffset::new(10)));

        let viewport = ViewportDimensions::new(80, 10);
        let range_before = state.visible_range(viewport);

        // Verify initial visible range: entries 5-9 (0-indexed, end exclusive)
        assert_eq!(
            range_before.start_index,
            EntryIndex::new(5),
            "Initially, entry 5 should be first visible"
        );

        // Record the first visible entry's absolute position in viewport
        let first_visible_entry = range_before.start_index;
        let first_visible_y_before = state
            .get(first_visible_entry)
            .unwrap()
            .layout()
            .cumulative_y()
            .get();
        let scroll_offset_before = range_before.scroll_offset.get();
        let offset_in_viewport_before = first_visible_y_before.saturating_sub(scroll_offset_before);

        // Toggle expand on entry 2 (above viewport: cumulative_y=4, which is before scroll_offset=10)
        state.toggle_expand(EntryIndex::new(2), params, viewport, variable_height);

        // After expanding entry 2 (height 2 -> 5), cumulative_y shifts:
        // Entry 0: y=0..2
        // Entry 1: y=2..4
        // Entry 2: y=4..9 (expanded, +3 height)
        // Entry 3: y=9..11 (+3 shift)
        // Entry 4: y=11..13 (+3 shift)
        // Entry 5: y=13..15 (+3 shift) <-- first visible should still be here
        // Entry 6: y=15..17
        // Entry 7: y=17..19
        // Total height: 23

        // Verify layout update
        assert_eq!(
            state.get(EntryIndex::new(2)).unwrap().is_expanded(),
            true,
            "Entry 2 should be expanded"
        );
        assert_eq!(
            state.total_height(),
            23,
            "Total height should increase from 20 to 23"
        );

        // The KEY assertion: first visible entry should still be entry 5
        // and it should still be at the same relative position in the viewport
        let range_after = state.visible_range(viewport);
        let first_visible_y_after = state
            .get(first_visible_entry)
            .unwrap()
            .layout()
            .cumulative_y()
            .get();
        let scroll_offset_after = range_after.scroll_offset.get();
        let offset_in_viewport_after = first_visible_y_after.saturating_sub(scroll_offset_after);

        assert_eq!(
            range_after.start_index, first_visible_entry,
            "First visible entry should remain entry 5 after toggling entry 2 above viewport"
        );

        assert_eq!(
            offset_in_viewport_after, offset_in_viewport_before,
            "Entry 5 should maintain same relative position in viewport (stable view)"
        );
    }

    // === hit_test Tests (Binary Search) ===

    #[test]
    fn hit_test_empty_state_returns_miss() {
        let state = ConversationViewState::empty();

        let result = state.hit_test(10, 10, LineOffset::new(0));

        assert_eq!(result, HitTestResult::Miss);
    }

    #[test]
    fn hit_test_finds_first_entry() {
        let entries = vec![make_valid_entry("uuid-1"), make_valid_entry("uuid-2")];
        let mut state = ConversationViewState::new(None, None, entries);

        let params = LayoutParams::new(80, WrapMode::Wrap);
        state.recompute_layout(params, fixed_height_calculator(10));

        // Click at screen_y=5 (no scroll) should hit entry 0 at line 5
        let result = state.hit_test(5, 10, LineOffset::new(0));

        assert_eq!(
            result,
            HitTestResult::Hit {
                entry_index: EntryIndex::new(0),
                line_in_entry: 5,
                column: 10
            }
        );
    }

    #[test]
    fn hit_test_finds_second_entry_with_scroll() {
        let entries = vec![
            make_valid_entry("uuid-1"),
            make_valid_entry("uuid-2"),
            make_valid_entry("uuid-3"),
        ];
        let mut state = ConversationViewState::new(None, None, entries);

        let params = LayoutParams::new(80, WrapMode::Wrap);
        state.recompute_layout(params, fixed_height_calculator(10)); // [0, 10, 20]

        // Click at screen_y=5 with scroll_offset=10
        // Absolute y = 10 + 5 = 15, which is in entry 1 (y=10..20)
        let result = state.hit_test(5, 20, LineOffset::new(10));

        assert_eq!(
            result,
            HitTestResult::Hit {
                entry_index: EntryIndex::new(1),
                line_in_entry: 5,
                column: 20
            }
        );
    }

    #[test]
    fn hit_test_beyond_content_returns_miss() {
        let entries = vec![make_valid_entry("uuid-1")];
        let mut state = ConversationViewState::new(None, None, entries);

        let params = LayoutParams::new(80, WrapMode::Wrap);
        state.recompute_layout(params, fixed_height_calculator(10)); // Total height = 10

        // Click at absolute y=15 (beyond entry 0 which ends at 10)
        let result = state.hit_test(15, 0, LineOffset::new(0));

        assert_eq!(result, HitTestResult::Miss);
    }

    #[test]
    fn hit_test_at_first_line_of_entry() {
        let entries = vec![
            make_valid_entry("uuid-1"),
            make_valid_entry("uuid-2"),
            make_valid_entry("uuid-3"),
        ];
        let mut state = ConversationViewState::new(None, None, entries);

        let params = LayoutParams::new(80, WrapMode::Wrap);
        state.recompute_layout(params, fixed_height_calculator(10));
        // Entry 0: y=0..10
        // Entry 1: y=10..20
        // Entry 2: y=20..30

        // Click at exact start of entry 1 (absolute_y=10)
        let result = state.hit_test(10, 5, LineOffset::new(0));

        assert_eq!(
            result,
            HitTestResult::Hit {
                entry_index: EntryIndex::new(1),
                line_in_entry: 0,
                column: 5
            },
            "Click at first line of entry should hit that entry at line 0"
        );
    }

    #[test]
    fn hit_test_at_last_line_of_entry() {
        let entries = vec![
            make_valid_entry("uuid-1"),
            make_valid_entry("uuid-2"),
            make_valid_entry("uuid-3"),
        ];
        let mut state = ConversationViewState::new(None, None, entries);

        let params = LayoutParams::new(80, WrapMode::Wrap);
        state.recompute_layout(params, fixed_height_calculator(10));
        // Entry 0: y=0..10 (lines 0-9)
        // Entry 1: y=10..20 (lines 10-19)
        // Entry 2: y=20..30 (lines 20-29)

        // Click at last line of entry 0 (absolute_y=9)
        let result = state.hit_test(9, 15, LineOffset::new(0));

        assert_eq!(
            result,
            HitTestResult::Hit {
                entry_index: EntryIndex::new(0),
                line_in_entry: 9,
                column: 15
            },
            "Click at last line of entry should hit that entry"
        );
    }

    #[test]
    fn hit_test_at_entry_boundaries_with_scroll() {
        let entries = vec![
            make_valid_entry("uuid-1"),
            make_valid_entry("uuid-2"),
            make_valid_entry("uuid-3"),
        ];
        let mut state = ConversationViewState::new(None, None, entries);

        let params = LayoutParams::new(80, WrapMode::Wrap);
        state.recompute_layout(params, fixed_height_calculator(10));
        // Entry 0: y=0..10
        // Entry 1: y=10..20
        // Entry 2: y=20..30

        // With scroll_offset=5, screen_y=5 means absolute_y=10 (first line of entry 1)
        let result = state.hit_test(5, 0, LineOffset::new(5));

        assert_eq!(
            result,
            HitTestResult::Hit {
                entry_index: EntryIndex::new(1),
                line_in_entry: 0,
                column: 0
            },
            "Boundary with scroll should correctly identify entry"
        );

        // With scroll_offset=10, screen_y=9 means absolute_y=19 (last line of entry 1)
        let result = state.hit_test(9, 10, LineOffset::new(10));

        assert_eq!(
            result,
            HitTestResult::Hit {
                entry_index: EntryIndex::new(1),
                line_in_entry: 9,
                column: 10
            },
            "Last line boundary with scroll should correctly identify entry"
        );
    }

    #[test]
    fn hit_test_single_entry_all_positions() {
        let entries = vec![make_valid_entry("uuid-1")];
        let mut state = ConversationViewState::new(None, None, entries);

        let params = LayoutParams::new(80, WrapMode::Wrap);
        state.recompute_layout(params, fixed_height_calculator(5));
        // Entry 0: y=0..5 (lines 0-4)

        // Test all valid positions within the entry
        for line in 0..5 {
            let result = state.hit_test(line as u16, 0, LineOffset::new(0));
            assert_eq!(
                result,
                HitTestResult::Hit {
                    entry_index: EntryIndex::new(0),
                    line_in_entry: line,
                    column: 0
                },
                "Line {} should be hit", line
            );
        }

        // Test position beyond entry
        let result = state.hit_test(5, 0, LineOffset::new(0));
        assert_eq!(result, HitTestResult::Miss, "Line 5 should miss (beyond entry)");
    }

    #[test]
    #[ignore = "Performance test - run manually with: cargo test hit_test_performance -- --ignored --nocapture"]
    fn hit_test_performance_100k_entries() {
        use std::time::Instant;

        // Create 100k entries
        let entries: Vec<ConversationEntry> = (0..100_000)
            .map(|i| make_valid_entry(&format!("uuid-{}", i)))
            .collect();

        let mut state = ConversationViewState::new(None, None, entries);

        let params = LayoutParams::new(80, WrapMode::Wrap);
        state.recompute_layout(params, fixed_height_calculator(10));
        // Total height: 1,000,000 lines

        // Test hit-testing at various positions
        let test_cases = vec![
            (0, 0),           // First entry
            (500_000, 25),    // Middle (entry 50,000)
            (999_999, 50),    // Last entry, last line
            (250_000, 10),    // Quarter
            (750_000, 30),    // Three quarters
        ];

        let mut total_duration = std::time::Duration::ZERO;
        let iterations = 100;

        for &(absolute_y, column) in &test_cases {
            for _ in 0..iterations {
                let start = Instant::now();
                let _result = state.hit_test(
                    (absolute_y % 1000) as u16,  // screen_y
                    column,
                    LineOffset::new(absolute_y - (absolute_y % 1000))  // scroll_offset
                );
                total_duration += start.elapsed();
            }
        }

        let avg_duration = total_duration / (test_cases.len() as u32 * iterations);

        println!("Average hit_test duration for 100k entries: {:?}", avg_duration);
        println!("Total test duration: {:?}", total_duration);

        // Verify <1ms requirement
        assert!(
            avg_duration.as_millis() < 1,
            "Hit test should complete in <1ms, got {:?}", avg_duration
        );
    }

    // === needs_relayout Tests ===

    #[test]
    fn needs_relayout_true_when_params_change() {
        let entries = vec![make_valid_entry("uuid-1")];
        let mut state = ConversationViewState::new(None, None, entries);

        let params1 = LayoutParams::new(80, WrapMode::Wrap);
        state.recompute_layout(params1, fixed_height_calculator(10));

        let params2 = LayoutParams::new(120, WrapMode::Wrap); // Different width
        assert!(state.needs_relayout(&params2));
    }

    #[test]
    fn needs_relayout_false_when_params_unchanged() {
        let entries = vec![make_valid_entry("uuid-1")];
        let mut state = ConversationViewState::new(None, None, entries);

        let params = LayoutParams::new(80, WrapMode::Wrap);
        state.recompute_layout(params, fixed_height_calculator(10));

        assert!(!state.needs_relayout(&params));
    }

    // === append Tests ===

    #[test]
    fn append_adds_entries_to_end() {
        let mut state = ConversationViewState::new(None, None, vec![make_valid_entry("uuid-1")]);

        state.append(vec![make_valid_entry("uuid-2"), make_valid_entry("uuid-3")]);

        assert_eq!(state.len(), 3);
        assert_eq!(
            state.get(EntryIndex::new(2)).unwrap().index(),
            EntryIndex::new(2)
        );
    }

    #[test]
    fn append_invalidates_layout() {
        let mut state = ConversationViewState::new(None, None, vec![make_valid_entry("uuid-1")]);

        let params = LayoutParams::new(80, WrapMode::Wrap);
        state.recompute_layout(params, fixed_height_calculator(10));

        state.append(vec![make_valid_entry("uuid-2")]);

        assert!(
            state.last_layout_params.is_none(),
            "Appending should invalidate layout params"
        );
    }

    // === set_wrap_override Tests ===

    #[test]
    fn set_wrap_override_updates_entry_state() {
        let entries = vec![make_valid_entry("uuid-1")];
        let mut state = ConversationViewState::new(None, None, entries);

        let params = LayoutParams::new(80, WrapMode::Wrap);
        state.recompute_layout(params, fixed_height_calculator(10));

        // Initially no override, uses global
        assert_eq!(state.get(EntryIndex::new(0)).unwrap().wrap_override(), None);
        assert_eq!(
            state
                .get(EntryIndex::new(0))
                .unwrap()
                .effective_wrap(WrapMode::Wrap),
            WrapMode::Wrap
        );

        // Set override to NoWrap
        state.set_wrap_override(
            EntryIndex::new(0),
            Some(WrapMode::NoWrap),
            params,
            fixed_height_calculator(10),
        );

        assert_eq!(
            state.get(EntryIndex::new(0)).unwrap().wrap_override(),
            Some(WrapMode::NoWrap)
        );
    }

    #[test]
    fn set_wrap_override_returns_previous_value() {
        let entries = vec![make_valid_entry("uuid-1")];
        let mut state = ConversationViewState::new(None, None, entries);

        let params = LayoutParams::new(80, WrapMode::Wrap);
        state.recompute_layout(params, fixed_height_calculator(10));

        // First call: previous was None
        let result = state.set_wrap_override(
            EntryIndex::new(0),
            Some(WrapMode::NoWrap),
            params,
            fixed_height_calculator(10),
        );
        assert_eq!(result, Some(None));

        // Second call: previous was Some(NoWrap)
        let result = state.set_wrap_override(
            EntryIndex::new(0),
            Some(WrapMode::Wrap),
            params,
            fixed_height_calculator(10),
        );
        assert_eq!(result, Some(Some(WrapMode::NoWrap)));

        // Third call: clearing override
        let result = state.set_wrap_override(
            EntryIndex::new(0),
            None,
            params,
            fixed_height_calculator(10),
        );
        assert_eq!(result, Some(Some(WrapMode::Wrap)));
    }

    #[test]
    fn set_wrap_override_returns_none_for_invalid_index() {
        let mut state = ConversationViewState::empty();
        let params = LayoutParams::new(80, WrapMode::Wrap);

        let result = state.set_wrap_override(
            EntryIndex::new(0),
            Some(WrapMode::NoWrap),
            params,
            fixed_height_calculator(10),
        );

        assert_eq!(result, None);

        // Also test out of bounds on non-empty state
        let mut state = ConversationViewState::new(None, None, vec![make_valid_entry("uuid-1")]);
        state.recompute_layout(params, fixed_height_calculator(10));

        let result = state.set_wrap_override(
            EntryIndex::new(999),
            Some(WrapMode::NoWrap),
            params,
            fixed_height_calculator(10),
        );

        assert_eq!(result, None);
    }

    #[test]
    fn set_wrap_override_triggers_relayout_from_index() {
        let entries = vec![
            make_valid_entry("uuid-1"),
            make_valid_entry("uuid-2"),
            make_valid_entry("uuid-3"),
        ];
        let mut state = ConversationViewState::new(None, None, entries);

        let params = LayoutParams::new(80, WrapMode::Wrap);

        // Variable height based on wrap mode
        let variable_height =
            |_entry: &ConversationEntry, _expanded: bool, wrap: WrapMode| match wrap {
                WrapMode::Wrap => LineHeight::new(10).unwrap(),
                WrapMode::NoWrap => LineHeight::new(5).unwrap(),
            };

        state.recompute_layout(params, variable_height);
        // Initial: all Wrap mode, heights [10, 10, 10], cumulative_y [0, 10, 20], total=30

        assert_eq!(
            state
                .get(EntryIndex::new(0))
                .unwrap()
                .layout()
                .cumulative_y()
                .get(),
            0
        );
        assert_eq!(
            state
                .get(EntryIndex::new(1))
                .unwrap()
                .layout()
                .cumulative_y()
                .get(),
            10
        );
        assert_eq!(
            state
                .get(EntryIndex::new(2))
                .unwrap()
                .layout()
                .cumulative_y()
                .get(),
            20
        );
        assert_eq!(state.total_height(), 30);

        // Set wrap override on entry 1 to NoWrap (height becomes 5)
        state.set_wrap_override(
            EntryIndex::new(1),
            Some(WrapMode::NoWrap),
            params,
            variable_height,
        );

        // After: heights [10, 5, 10], cumulative_y [0, 10, 15], total=25
        assert_eq!(
            state
                .get(EntryIndex::new(0))
                .unwrap()
                .layout()
                .cumulative_y()
                .get(),
            0
        );
        assert_eq!(
            state
                .get(EntryIndex::new(1))
                .unwrap()
                .layout()
                .cumulative_y()
                .get(),
            10
        );
        assert_eq!(
            state
                .get(EntryIndex::new(2))
                .unwrap()
                .layout()
                .cumulative_y()
                .get(),
            15
        ); // Shifted up by 5
        assert_eq!(state.total_height(), 25);
    }

    // === Horizontal Scrolling Tests ===

    #[test]
    fn horizontal_offset_starts_at_zero() {
        let entries = vec![make_valid_entry("uuid-1")];
        let state = ConversationViewState::new(None, None, entries);

        assert_eq!(
            state.horizontal_offset(),
            0,
            "Horizontal offset should start at 0"
        );
    }

    #[test]
    fn set_horizontal_offset_updates_value() {
        let entries = vec![make_valid_entry("uuid-1")];
        let mut state = ConversationViewState::new(None, None, entries);

        state.set_horizontal_offset(42);

        assert_eq!(
            state.horizontal_offset(),
            42,
            "Horizontal offset should be updated to 42"
        );
    }

    #[test]
    fn scroll_right_increases_offset() {
        let entries = vec![make_valid_entry("uuid-1")];
        let mut state = ConversationViewState::new(None, None, entries);

        assert_eq!(state.horizontal_offset(), 0);

        state.scroll_right(5);
        assert_eq!(
            state.horizontal_offset(),
            5,
            "Scrolling right by 5 should set offset to 5"
        );

        state.scroll_right(3);
        assert_eq!(
            state.horizontal_offset(),
            8,
            "Scrolling right by 3 more should set offset to 8"
        );
    }

    #[test]
    fn scroll_left_decreases_offset() {
        let entries = vec![make_valid_entry("uuid-1")];
        let mut state = ConversationViewState::new(None, None, entries);

        state.set_horizontal_offset(10);
        assert_eq!(state.horizontal_offset(), 10);

        state.scroll_left(3);
        assert_eq!(
            state.horizontal_offset(),
            7,
            "Scrolling left by 3 should set offset to 7"
        );

        state.scroll_left(5);
        assert_eq!(
            state.horizontal_offset(),
            2,
            "Scrolling left by 5 more should set offset to 2"
        );
    }

    #[test]
    fn scroll_left_saturates_at_zero() {
        let entries = vec![make_valid_entry("uuid-1")];
        let mut state = ConversationViewState::new(None, None, entries);

        state.set_horizontal_offset(5);
        assert_eq!(state.horizontal_offset(), 5);

        // Scroll left by more than current offset
        state.scroll_left(10);
        assert_eq!(
            state.horizontal_offset(),
            0,
            "Scrolling left past 0 should saturate at 0"
        );

        // Scrolling left from 0 should stay at 0
        state.scroll_left(5);
        assert_eq!(
            state.horizontal_offset(),
            0,
            "Scrolling left from 0 should stay at 0"
        );
    }

    #[test]
    fn scroll_right_handles_u16_max() {
        let entries = vec![make_valid_entry("uuid-1")];
        let mut state = ConversationViewState::new(None, None, entries);

        // Set to near max
        state.set_horizontal_offset(u16::MAX - 5);
        assert_eq!(state.horizontal_offset(), u16::MAX - 5);

        // Scroll right should saturate at u16::MAX
        state.scroll_right(10);
        assert_eq!(
            state.horizontal_offset(),
            u16::MAX,
            "Scrolling right should saturate at u16::MAX"
        );
    }

    #[test]
    fn set_wrap_override_affects_effective_wrap() {
        let entries = vec![make_valid_entry("uuid-1")];
        let mut state = ConversationViewState::new(None, None, entries);

        let params = LayoutParams::new(80, WrapMode::Wrap);
        state.recompute_layout(params, fixed_height_calculator(10));

        let entry = state.get(EntryIndex::new(0)).unwrap();

        // Initially uses global
        assert_eq!(entry.effective_wrap(WrapMode::Wrap), WrapMode::Wrap);
        assert_eq!(entry.effective_wrap(WrapMode::NoWrap), WrapMode::NoWrap);

        // Set override to NoWrap
        state.set_wrap_override(
            EntryIndex::new(0),
            Some(WrapMode::NoWrap),
            params,
            fixed_height_calculator(10),
        );

        let entry = state.get(EntryIndex::new(0)).unwrap();

        // Now always returns override regardless of global
        assert_eq!(entry.effective_wrap(WrapMode::Wrap), WrapMode::NoWrap);
        assert_eq!(entry.effective_wrap(WrapMode::NoWrap), WrapMode::NoWrap);

        // Set override to Wrap
        state.set_wrap_override(
            EntryIndex::new(0),
            Some(WrapMode::Wrap),
            params,
            fixed_height_calculator(10),
        );

        let entry = state.get(EntryIndex::new(0)).unwrap();

        // Now always returns Wrap
        assert_eq!(entry.effective_wrap(WrapMode::Wrap), WrapMode::Wrap);
        assert_eq!(entry.effective_wrap(WrapMode::NoWrap), WrapMode::Wrap);

        // Clear override
        state.set_wrap_override(
            EntryIndex::new(0),
            None,
            params,
            fixed_height_calculator(10),
        );

        let entry = state.get(EntryIndex::new(0)).unwrap();

        // Back to using global
        assert_eq!(entry.effective_wrap(WrapMode::Wrap), WrapMode::Wrap);
        assert_eq!(entry.effective_wrap(WrapMode::NoWrap), WrapMode::NoWrap);
    }
}
