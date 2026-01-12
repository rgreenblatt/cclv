//! View-state for a single conversation

#![allow(dead_code)] // Will be used by tests and other modules

use super::{
    entry_view::EntryView,
    height_index::HeightIndex,
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
    /// Fenwick tree for O(log n) cumulative height queries.
    /// Invariant: height_index[i] == entries[i].rendered_lines.len()
    pub(crate) height_index: HeightIndex,
    /// Viewport width used for last layout.
    /// Needed for recomputing rendered_lines when entries change.
    viewport_width: u16,
    /// Global wrap mode used for last layout.
    /// Needed for recomputing rendered_lines when entries change.
    global_wrap: WrapMode,
    /// Cached total height in lines (sum of all entry heights).
    /// DEPRECATED: Use height_index.total() instead.
    /// Kept temporarily for compatibility during migration.
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
    /// Maximum context window size (from config).
    /// Used for rendering context dividers with percentages.
    max_context_tokens: usize,
    /// Pricing configuration (from config).
    /// Used for cost calculation in dividers.
    pricing: crate::model::PricingConfig,
}

impl ConversationViewState {
    /// Create new conversation view-state from entries.
    /// Layout is not computed until `recompute_layout` is called.
    pub fn new(
        agent_id: Option<crate::model::AgentId>,
        model: Option<crate::model::ModelInfo>,
        entries: Vec<ConversationEntry>,
        max_context_tokens: usize,
        pricing: crate::model::PricingConfig,
    ) -> Self {
        // Compute accumulated tokens as running sum
        let mut accumulated = 0;
        let entry_views: Vec<EntryView> = entries
            .into_iter()
            .enumerate()
            .map(|(idx, entry)| {
                accumulated += entry.token_count();
                EntryView::new(
                    entry,
                    EntryIndex::new(idx),
                    accumulated,
                    max_context_tokens,
                    pricing.clone(),
                )
            })
            .collect();
        let capacity = entry_views.len().max(100);
        Self {
            agent_id,
            model,
            entries: entry_views,
            scroll: ScrollPosition::Top,
            height_index: HeightIndex::new(capacity),
            viewport_width: 0,
            global_wrap: WrapMode::Wrap, // Default to Wrap
            total_height: 0,
            focused_message: None,
            last_layout_params: None,
            horizontal_offset: 0,
            max_context_tokens,
            pricing,
        }
    }

    /// Create empty conversation view-state for main agent.
    pub fn empty() -> Self {
        Self::new(
            None,
            None,
            Vec::new(),
            200_000, // Default max_context_tokens
            crate::model::PricingConfig::default(),
        )
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

    /// Set model information if not already set.
    /// Returns true if model was set, false if already had a model.
    pub fn set_model_if_none(&mut self, model: crate::model::ModelInfo) -> bool {
        if self.model.is_none() {
            self.model = Some(model);
            true
        } else {
            false
        }
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

    /// Returns the height of a specific entry.
    /// This is primarily a test helper, but exposed publicly for debugging.
    pub fn entry_height(&self, index: EntryIndex) -> Option<LineHeight> {
        let idx = index.get();
        if idx >= self.entries.len() || idx >= self.height_index.len() {
            return None;
        }
        let y_start = if idx == 0 {
            0
        } else {
            self.height_index.prefix_sum(idx - 1)
        };
        let y_end = self.height_index.prefix_sum(idx);
        LineHeight::new((y_end - y_start).try_into().ok()?).ok()
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
        // Get accumulated tokens from last entry (or 0 if empty)
        let mut accumulated = self
            .entries
            .last()
            .map(|e| e.accumulated_tokens())
            .unwrap_or(0);

        for (offset, entry) in entries.into_iter().enumerate() {
            accumulated += entry.token_count();
            self.entries.push(EntryView::new(
                entry,
                EntryIndex::new(start_idx + offset),
                accumulated,
                self.max_context_tokens,
                self.pricing.clone(),
            ));
        }
        // Invalidate layout
        self.last_layout_params = None;
    }

    /// Recompute layout for all entries.
    ///
    /// # Deprecated
    /// This method exists for backward compatibility. New code should use `relayout()`.
    pub fn recompute_layout(&mut self, params: LayoutParams) {
        self.relayout(
            params.width,
            params.global_wrap,
            &crate::state::SearchState::Inactive,
        );
    }

    /// Relayout from a specific entry index onward.
    ///
    /// # Deprecated
    /// This method exists for backward compatibility. New code should use
    /// `toggle_entry_expanded()` or `set_entry_wrap_override()`.
    pub fn relayout_from(&mut self, from_index: EntryIndex, params: LayoutParams) {
        let idx = from_index.get();
        if idx >= self.entries.len() {
            return;
        }

        // For simplicity, just do a full relayout (same as new API)
        // The optimization of relayout_from vs full relayout is not critical
        self.relayout(
            params.width,
            params.global_wrap,
            &crate::state::SearchState::Inactive,
        );
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
    pub fn toggle_expand(
        &mut self,
        index: EntryIndex,
        params: LayoutParams,
        viewport: ViewportDimensions,
    ) -> Option<bool> {
        // Capture scroll anchor BEFORE toggling if entry is above viewport
        let scroll_anchor = self.compute_scroll_anchor_before_toggle(index, viewport);

        let entry = self.entries.get_mut(index.get())?;
        let new_state = entry.toggle_expanded();
        self.relayout_from(index, params);

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
        let first_visible_y = if first_visible.get() == 0 {
            0
        } else {
            self.height_index.prefix_sum(first_visible.get() - 1)
        };
        let scroll_offset = visible.scroll_offset.get();
        let line_in_entry = first_visible_y.saturating_sub(scroll_offset);

        Some(ScrollPosition::AtEntry {
            entry_index: first_visible,
            line_in_entry,
        })
    }

    /// Set wrap override for entry at index and relayout.
    /// Returns previous wrap override, or None if index out of bounds.
    pub fn set_wrap_override(
        &mut self,
        index: EntryIndex,
        wrap: Option<WrapMode>,
        params: LayoutParams,
    ) -> Option<Option<WrapMode>> {
        let entry = self.entries.get_mut(index.get())?;
        let previous = entry.wrap_override();
        entry.set_wrap_override(wrap);
        self.relayout_from(index, params);
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
        if self.entries.is_empty() || self.height_index.is_empty() {
            return VisibleRange::default();
        }

        let scroll_offset =
            self.scroll
                .resolve(self.total_height, viewport.height as usize, |idx| {
                    self.entry_cumulative_y(idx)
                });

        let scroll_line = scroll_offset.get();
        let viewport_bottom = scroll_line + viewport.height as usize;

        // Binary search for first visible entry
        // Find first entry whose bottom_y > scroll_line
        let indices: Vec<usize> = (0..self.entries.len()).collect();
        let start_index =
            indices.partition_point(|&i| self.height_index.prefix_sum(i) <= scroll_line);

        // Binary search for first entry past viewport
        // Find first entry whose cumulative_y >= viewport_bottom
        let end_index = indices.partition_point(|&i| {
            let cumulative_y = if i == 0 {
                0
            } else {
                self.height_index.prefix_sum(i - 1)
            };
            cumulative_y < viewport_bottom
        });

        VisibleRange::new(
            EntryIndex::new(start_index),
            EntryIndex::new(end_index),
            scroll_offset,
            viewport.height,
        )
    }

    /// Check if scroll position is at bottom of content.
    ///
    /// Used for FR-036: auto-scroll pause when user scrolls away from bottom.
    ///
    /// # Arguments
    /// - `viewport`: Viewport dimensions
    ///
    /// # Returns
    /// `true` if the bottom of the content is visible in the viewport.
    /// This means the last line of content is within the viewport.
    pub fn is_at_bottom(&self, viewport: ViewportDimensions) -> bool {
        if self.total_height == 0 {
            return true; // Empty content is always "at bottom"
        }

        // Content fits entirely in viewport - always at bottom
        if self.total_height <= viewport.height as usize {
            return true;
        }

        // Resolve current scroll position to line offset
        let scroll_offset =
            self.scroll
                .resolve(self.total_height, viewport.height as usize, |idx| {
                    self.entry_cumulative_y(idx)
                });

        // Calculate the maximum scroll offset (bottom position)
        // When scrolled to bottom: scroll_offset = total_height - viewport_height
        let max_scroll = self.total_height.saturating_sub(viewport.height as usize);

        // We're at bottom if scroll_offset >= max_scroll
        scroll_offset.get() >= max_scroll
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
        if self.entries.is_empty() || self.height_index.is_empty() {
            return HitTestResult::miss();
        }

        let absolute_y = scroll_offset.get() + screen_y as usize;

        // Binary search for entry containing absolute_y
        // Find first entry whose bottom_y > absolute_y
        let indices: Vec<usize> = (0..self.entries.len()).collect();
        let index = indices.partition_point(|&i| self.height_index.prefix_sum(i) <= absolute_y);

        if index >= self.entries.len() {
            return HitTestResult::miss();
        }

        let entry_y = if index == 0 {
            0
        } else {
            self.height_index.prefix_sum(index - 1)
        };

        if absolute_y < entry_y {
            return HitTestResult::miss();
        }

        let line_in_entry = absolute_y - entry_y;

        HitTestResult::hit(EntryIndex::new(index), line_in_entry, screen_x)
    }

    /// Get cumulative_y for entry at index (for scroll resolution).
    pub fn entry_cumulative_y(&self, index: EntryIndex) -> Option<LineOffset> {
        if index.get() >= self.entries.len() {
            return None;
        }
        let cumulative_y = if index.get() == 0 {
            0
        } else {
            self.height_index.prefix_sum(index.get() - 1)
        };
        Some(LineOffset::new(cumulative_y))
    }

    /// Get approximate scroll line for active session determination (FR-080).
    ///
    /// Uses heuristics to estimate line offset without requiring viewport dimensions.
    /// This is "good enough" for determining which session contains the scroll position.
    ///
    /// # Returns
    /// Approximate line offset from top of conversation.
    pub fn approximate_scroll_line(&self) -> usize {
        self.scroll
            .approximate_line(self.total_height, |idx| self.entry_cumulative_y(idx))
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

    // === HEIGHT INDEX INTEGRATION METHODS ===

    /// Full relayout when width or global wrap changes. O(n log n).
    ///
    /// This replaces `recompute_layout` with a simpler API that doesn't require
    /// an external height_calculator function. Heights are computed from
    /// entry.rendered_lines after recomputing them.
    ///
    /// # Arguments
    /// * `width` - Viewport width
    /// * `wrap` - Global wrap mode
    /// * `search_state` - Current search state (for highlighting matches)
    pub fn relayout(
        &mut self,
        width: u16,
        wrap: WrapMode,
        search_state: &crate::state::SearchState,
    ) {
        self.viewport_width = width;
        self.global_wrap = wrap;
        self.height_index.clear();

        for (idx, entry_view) in self.entries.iter_mut().enumerate() {
            let effective_wrap = entry_view.effective_wrap(wrap);
            let is_focused = self.focused_message.is_some_and(|f| f.get() == idx);
            entry_view.recompute_lines(effective_wrap, width, search_state, is_focused);

            let height = entry_view.height().get() as usize;
            self.height_index.push(height);
        }

        self.total_height = self.height_index.total();
        self.last_layout_params = Some(LayoutParams::new(width, wrap));
    }

    /// Toggle expanded state for entry. O(log n).
    ///
    /// Atomically updates both the entry state and HeightIndex to maintain invariant.
    ///
    /// # Arguments
    /// * `index` - Entry index to toggle
    /// * `search_state` - Current search state (for highlighting matches)
    pub fn toggle_entry_expanded(
        &mut self,
        index: usize,
        search_state: &crate::state::SearchState,
    ) {
        if index >= self.entries.len() {
            tracing::warn!(
                "toggle_entry_expanded: index {} >= entries.len() {}",
                index,
                self.entries.len()
            );
            return;
        }

        let entry = &mut self.entries[index];
        let old_expanded = entry.is_expanded();
        let old_height = entry.height();

        // Toggle expanded state
        entry.toggle_expanded();

        tracing::trace!(
            "toggle_entry_expanded: index={}, expanded: {} -> {}, height_before={:?}",
            index,
            old_expanded,
            entry.is_expanded(),
            old_height
        );

        // Recompute lines with new expand state
        let effective_wrap = entry.effective_wrap(self.global_wrap);
        let is_focused = self.focused_message.is_some_and(|f| f.get() == index);
        entry.recompute_lines(
            effective_wrap,
            self.viewport_width,
            search_state,
            is_focused,
        );

        let new_height = entry.height().get() as usize;

        tracing::trace!(
            "After recompute: new_height={}, updating HeightIndex",
            new_height
        );

        // Update HeightIndex atomically
        self.height_index.set(index, new_height);

        // Update total_height
        self.total_height = self.height_index.total();

        tracing::trace!(
            "After HeightIndex update: total_height={}",
            self.total_height
        );
    }

    /// Set wrap override for entry. O(log n).
    ///
    /// Atomically updates both the entry state and HeightIndex to maintain invariant.
    ///
    /// # Arguments
    /// * `index` - Entry index to modify
    /// * `mode` - Wrap mode override (None to use global)
    /// * `search_state` - Current search state (for highlighting matches)
    pub fn set_entry_wrap_override(
        &mut self,
        index: usize,
        mode: Option<WrapMode>,
        search_state: &crate::state::SearchState,
    ) {
        if index >= self.entries.len() {
            return;
        }

        let entry = &mut self.entries[index];

        // Set wrap override
        entry.set_wrap_override(mode);

        // Recompute lines with new wrap mode
        let effective_wrap = entry.effective_wrap(self.global_wrap);
        let is_focused = self.focused_message.is_some_and(|f| f.get() == index);
        entry.recompute_lines(
            effective_wrap,
            self.viewport_width,
            search_state,
            is_focused,
        );

        let new_height = entry.height().get() as usize;

        tracing::trace!(
            "After recompute: new_height={}, updating HeightIndex",
            new_height
        );

        // Update HeightIndex atomically
        self.height_index.set(index, new_height);

        // Update total_height
        self.total_height = self.height_index.total();

        tracing::trace!(
            "After HeightIndex update: total_height={}",
            self.total_height
        );
    }

    /// Append new entries (streaming mode). O(n log n) where n is new entries.
    ///
    /// Updates HeightIndex for all new entries.
    ///
    /// # Arguments
    /// * `entries` - New conversation entries to append
    /// * `search_state` - Current search state (for highlighting matches)
    pub fn append_entries(
        &mut self,
        entries: Vec<ConversationEntry>,
        search_state: &crate::state::SearchState,
    ) {
        let start_idx = self.entries.len();

        // Get accumulated tokens from last entry (or 0 if empty)
        let mut accumulated = self
            .entries
            .last()
            .map(|e| e.accumulated_tokens())
            .unwrap_or(0);

        for (offset, entry) in entries.into_iter().enumerate() {
            let index = EntryIndex::new(start_idx + offset);
            accumulated += entry.token_count();

            let mut entry_view = EntryView::new(
                entry,
                index,
                accumulated,
                self.max_context_tokens,
                self.pricing.clone(),
            );

            // Compute rendered lines
            let effective_wrap = entry_view.effective_wrap(self.global_wrap);
            let is_focused = self
                .focused_message
                .is_some_and(|f| f.get() == (start_idx + offset));
            entry_view.recompute_lines(
                effective_wrap,
                self.viewport_width,
                search_state,
                is_focused,
            );

            let height = entry_view.height().get() as usize;

            // Update HeightIndex
            self.height_index.push(height);

            self.entries.push(entry_view);
        }

        self.total_height = self.height_index.total();
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

    /// Helper: Create an entry with a specific number of text lines for predictable heights.
    /// Each line will render as approximately one line in the TUI (plus header).
    fn make_entry_with_n_lines(uuid: &str, num_lines: usize) -> ConversationEntry {
        let text = (0..num_lines)
            .map(|i| format!("Line {}", i))
            .collect::<Vec<_>>()
            .join("\n");

        let message = Message::new(Role::User, MessageContent::Text(text));
        let log_entry = LogEntry::new(
            make_entry_uuid(uuid),
            None,
            make_session_id("session-1"),
            None,
            make_timestamp(),
            EntryType::User,
            message,
            EntryMetadata::default(),
        );
        ConversationEntry::Valid(Box::new(log_entry))
    }

    /// Test helper: Create ConversationViewState with default config values
    fn make_test_state(
        agent_id: Option<crate::model::AgentId>,
        model: Option<crate::model::ModelInfo>,
        entries: Vec<ConversationEntry>,
    ) -> ConversationViewState {
        ConversationViewState::new(
            agent_id,
            model,
            entries,
            200_000, // Default test max_context_tokens
            crate::model::PricingConfig::default(),
        )
    }

    // === ConversationViewState::new Tests ===

    #[test]
    fn new_creates_view_state_from_entries() {
        let entries = vec![
            make_valid_entry("uuid-1"),
            make_valid_entry("uuid-2"),
            make_valid_entry("uuid-3"),
        ];

        let state = ConversationViewState::new(
            None,
            None,
            entries,
            200_000,
            crate::model::PricingConfig::default(),
        );

        assert_eq!(state.len(), 3, "Should have 3 entries");
        assert!(!state.is_empty(), "Should not be empty");
    }

    #[test]
    fn new_starts_with_no_layout() {
        let entries = vec![make_valid_entry("uuid-1")];
        let state = ConversationViewState::new(
            None,
            None,
            entries,
            200_000,
            crate::model::PricingConfig::default(),
        );

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
        let state = ConversationViewState::new(
            None,
            None,
            entries,
            200_000,
            crate::model::PricingConfig::default(),
        );

        assert_eq!(state.scroll(), &ScrollPosition::Top, "Should start at top");
    }

    #[test]
    fn new_starts_with_no_focused_message() {
        let entries = vec![make_valid_entry("uuid-1")];
        let state = ConversationViewState::new(
            None,
            None,
            entries,
            200_000,
            crate::model::PricingConfig::default(),
        );

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
        let state = ConversationViewState::new(
            None,
            None,
            entries,
            200_000,
            crate::model::PricingConfig::default(),
        );

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
        let mut state = ConversationViewState::new(
            None,
            None,
            entries,
            200_000,
            crate::model::PricingConfig::default(),
        );

        let params = LayoutParams::new(80, WrapMode::Wrap);
        state.recompute_layout(params);

        // Each entry with "Test message" renders to a specific height
        // (header + content lines). The exact value depends on the renderer.
        let total = state.total_height();
        assert!(
            total > 0,
            "Total height should be positive after layout, got {}",
            total
        );

        // Verify it's consistent across identical entries
        let h0 = state.entry_height(EntryIndex::new(0)).unwrap().get() as usize;
        let h1 = state.entry_height(EntryIndex::new(1)).unwrap().get() as usize;
        let h2 = state.entry_height(EntryIndex::new(2)).unwrap().get() as usize;
        assert_eq!(h0, h1, "Identical entries should have same height");
        assert_eq!(h1, h2, "Identical entries should have same height");
        assert_eq!(total, h0 + h1 + h2, "Total should equal sum of heights");
    }

    #[test]
    fn recompute_layout_maintains_cumulative_y_invariant() {
        let entries = vec![
            make_valid_entry("uuid-1"),
            make_valid_entry("uuid-2"),
            make_valid_entry("uuid-3"),
        ];
        let mut state = ConversationViewState::new(
            None,
            None,
            entries,
            200_000,
            crate::model::PricingConfig::default(),
        );

        let params = LayoutParams::new(80, WrapMode::Wrap);
        state.recompute_layout(params);

        // Verify cumulative_y invariant: cumulative_y[i] = sum(height[0..i])
        let h0 = state.entry_height(EntryIndex::new(0)).unwrap().get() as usize;
        let h1 = state.entry_height(EntryIndex::new(1)).unwrap().get() as usize;

        assert_eq!(
            state.entry_cumulative_y(EntryIndex::new(0)).unwrap().get(),
            0,
            "First entry should start at y=0"
        );
        assert_eq!(
            state.entry_cumulative_y(EntryIndex::new(1)).unwrap().get(),
            h0,
            "Second entry should start after first"
        );
        assert_eq!(
            state.entry_cumulative_y(EntryIndex::new(2)).unwrap().get(),
            h0 + h1,
            "Third entry should start after first two"
        );
    }

    #[test]
    fn recompute_layout_stores_params() {
        let entries = vec![make_valid_entry("uuid-1")];
        let mut state = ConversationViewState::new(
            None,
            None,
            entries,
            200_000,
            crate::model::PricingConfig::default(),
        );

        let params = LayoutParams::new(80, WrapMode::Wrap);
        state.recompute_layout(params);

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
        // Create entries with 9 lines of text each (renders as 9 text lines + 1 header = 10 total)
        let entries = vec![
            make_entry_with_n_lines("uuid-1", 9),
            make_entry_with_n_lines("uuid-2", 9),
            make_entry_with_n_lines("uuid-3", 9),
            make_entry_with_n_lines("uuid-4", 9),
        ];
        let mut state = ConversationViewState::new(
            None,
            None,
            entries,
            200_000,
            crate::model::PricingConfig::default(),
        );

        let params = LayoutParams::new(80, WrapMode::Wrap);
        state.recompute_layout(params);
        state.set_scroll(ScrollPosition::Top);

        let viewport = ViewportDimensions::new(80, 24); // 24 line viewport

        let range = state.visible_range(viewport);

        // Viewport shows lines 0-23, entries are at y=[0, 10, 20, 30] (each 10 lines)
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
            make_entry_with_n_lines("uuid-1", 9),
            make_entry_with_n_lines("uuid-2", 9),
            make_entry_with_n_lines("uuid-3", 9),
            make_entry_with_n_lines("uuid-4", 9),
            make_entry_with_n_lines("uuid-5", 9),
        ];
        let mut state = ConversationViewState::new(
            None,
            None,
            entries,
            200_000,
            crate::model::PricingConfig::default(),
        );

        let params = LayoutParams::new(80, WrapMode::Wrap);
        state.recompute_layout(params); // Each entry is 10 lines
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
            make_entry_with_n_lines("uuid-1", 9),
            make_entry_with_n_lines("uuid-2", 9),
            make_entry_with_n_lines("uuid-3", 9),
            make_entry_with_n_lines("uuid-4", 9),
        ];
        let mut state = ConversationViewState::new(
            None,
            None,
            entries,
            200_000,
            crate::model::PricingConfig::default(),
        );

        let params = LayoutParams::new(80, WrapMode::Wrap);
        state.recompute_layout(params); // Total height = 40
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
        let mut state = ConversationViewState::new(
            None,
            None,
            entries,
            200_000,
            crate::model::PricingConfig::default(),
        );

        let params = LayoutParams::new(80, WrapMode::Wrap);
        state.recompute_layout(params);

        // Record initial cumulative_y values
        let y0_before = state.entry_cumulative_y(EntryIndex::new(0)).unwrap().get();
        let y1_before = state.entry_cumulative_y(EntryIndex::new(1)).unwrap().get();

        // Change entry 1's expanded state
        state.entries[1].set_expanded(true);
        state.relayout_from(EntryIndex::new(1), params);

        // Verify entry 0 is unchanged (before relayout start)
        assert_eq!(
            state.entry_cumulative_y(EntryIndex::new(0)).unwrap().get(),
            y0_before,
            "Entry 0 should be unchanged (before relayout index)"
        );

        // Entry 1 should still be at same position (it's the relayout start)
        assert_eq!(
            state.entry_cumulative_y(EntryIndex::new(1)).unwrap().get(),
            y1_before,
            "Entry 1 should be at same position"
        );

        // Entry 2's position may have changed based on entry 1's height
        let entry1_height = state.entries[1].height().get() as usize;
        assert_eq!(
            state.entry_cumulative_y(EntryIndex::new(2)).unwrap().get(),
            y1_before + entry1_height,
            "Entry 2's y should be entry 1's y + entry 1's height"
        );

        // Verify invariant
        let total: usize = state
            .entries()
            .iter()
            .map(|e| e.height().get() as usize)
            .sum();
        assert_eq!(
            state.total_height(),
            total,
            "Total height must equal sum of entry heights"
        );
    }

    #[test]
    fn relayout_from_zero_is_equivalent_to_full_relayout() {
        let entries = vec![
            make_valid_entry("uuid-1"),
            make_valid_entry("uuid-2"),
            make_valid_entry("uuid-3"),
        ];

        let mut state1 = ConversationViewState::new(
            None,
            None,
            entries.clone(),
            200_000,
            crate::model::PricingConfig::default(),
        );
        let mut state2 = ConversationViewState::new(
            None,
            None,
            entries,
            200_000,
            crate::model::PricingConfig::default(),
        );

        let params = LayoutParams::new(80, WrapMode::Wrap);

        state1.recompute_layout(params);
        state2.relayout_from(EntryIndex::new(0), params);

        // Both should produce identical layout
        assert_eq!(state1.total_height(), state2.total_height());
        for i in 0..3 {
            let idx = EntryIndex::new(i);
            assert_eq!(
                state1.entry_cumulative_y(idx),
                state2.entry_cumulative_y(idx)
            );
        }
    }

    // === toggle_expand Tests ===

    #[test]
    fn toggle_expand_returns_new_state() {
        let entries = vec![make_valid_entry("uuid-1")];
        let mut state = ConversationViewState::new(
            None,
            None,
            entries,
            200_000,
            crate::model::PricingConfig::default(),
        );

        let params = LayoutParams::new(80, WrapMode::Wrap);
        state.recompute_layout(params);

        let viewport = ViewportDimensions::new(80, 24);
        let result = state.toggle_expand(EntryIndex::new(0), params, viewport);

        assert_eq!(result, Some(true), "Should toggle to expanded");
        assert!(state.get(EntryIndex::new(0)).unwrap().is_expanded());
    }

    #[test]
    fn toggle_expand_returns_none_for_invalid_index() {
        let mut state = ConversationViewState::empty();
        let params = LayoutParams::new(80, WrapMode::Wrap);

        let viewport = ViewportDimensions::new(80, 24);
        let result = state.toggle_expand(EntryIndex::new(0), params, viewport);

        assert_eq!(result, None);
    }

    #[test]
    fn toggle_expand_triggers_relayout() {
        let entries = vec![make_valid_entry("uuid-1"), make_valid_entry("uuid-2")];
        let mut state = ConversationViewState::new(
            None,
            None,
            entries,
            200_000,
            crate::model::PricingConfig::default(),
        );

        let params = LayoutParams::new(80, WrapMode::Wrap);
        state.recompute_layout(params);

        let viewport = ViewportDimensions::new(80, 24);

        // Verify expand state changes
        assert!(
            !state.entries()[0].is_expanded(),
            "Entry should start collapsed"
        );

        state.toggle_expand(EntryIndex::new(0), params, viewport);

        assert!(
            state.entries()[0].is_expanded(),
            "Entry should be expanded after toggle"
        );

        // Verify height_index invariant: total_height == sum(entry.height())
        let total: usize = state
            .entries()
            .iter()
            .map(|e| e.height().get() as usize)
            .sum();
        assert_eq!(
            state.total_height(),
            total,
            "Total height must equal sum of entry heights"
        );

        // Verify first entry always starts at y=0
        assert_eq!(
            state.entry_cumulative_y(EntryIndex::new(0)).unwrap().get(),
            0,
            "First entry must start at y=0"
        );

        // Verify second entry's y position equals first entry's height
        let entry0_height = state.entries()[0].height().get() as usize;
        assert_eq!(
            state.entry_cumulative_y(EntryIndex::new(1)).unwrap().get(),
            entry0_height,
            "Second entry's y must equal first entry's height"
        );
    }

    #[test]
    fn toggle_expand_above_viewport_keeps_visible_entries_stable() {
        // Setup: Create 10 entries
        let entries: Vec<ConversationEntry> = (0..10)
            .map(|i| make_valid_entry(&format!("uuid-{}", i)))
            .collect();
        let mut state = ConversationViewState::new(
            None,
            None,
            entries,
            200_000,
            crate::model::PricingConfig::default(),
        );

        let params = LayoutParams::new(80, WrapMode::Wrap);
        state.recompute_layout(params);

        // Scroll down past the first few entries
        state.set_scroll(ScrollPosition::AtLine(LineOffset::new(10)));

        let viewport = ViewportDimensions::new(80, 10);
        let range_before = state.visible_range(viewport);
        let first_visible_entry = range_before.start_index;

        // Verify we're scrolled down (not viewing first entry)
        assert!(
            first_visible_entry.get() > 0,
            "Should be scrolled past first entry"
        );

        // Record the first visible entry's position
        let first_visible_y_before = state.entry_cumulative_y(first_visible_entry).unwrap().get();
        let scroll_offset_before = range_before.scroll_offset.get();
        let offset_in_viewport_before = first_visible_y_before.saturating_sub(scroll_offset_before);

        // Find an entry above the viewport to expand
        let entry_to_expand = EntryIndex::new(0);
        let expand_y = state.entry_cumulative_y(entry_to_expand).unwrap().get();
        assert!(
            expand_y < scroll_offset_before,
            "Entry to expand must be above viewport"
        );

        // Toggle expand on entry above viewport
        state.toggle_expand(entry_to_expand, params, viewport);

        // Verify layout changed
        assert!(
            state.get(entry_to_expand).unwrap().is_expanded(),
            "Entry should be expanded"
        );

        // Total height may or may not change depending on content (plain text won't change)
        // Just verify invariant holds
        let total_after: usize = state
            .entries()
            .iter()
            .map(|e| e.height().get() as usize)
            .sum();
        assert_eq!(
            state.total_height(),
            total_after,
            "Total height must equal sum of entry heights"
        );

        // The KEY assertion: first visible entry should remain stable
        // in viewport position even though an entry above it changed
        let range_after = state.visible_range(viewport);

        assert_eq!(
            range_after.start_index, first_visible_entry,
            "First visible entry should remain stable after toggling entry above viewport"
        );

        let first_visible_y_after = state.entry_cumulative_y(first_visible_entry).unwrap().get();
        let scroll_offset_after = range_after.scroll_offset.get();
        let offset_in_viewport_after = first_visible_y_after.saturating_sub(scroll_offset_after);

        assert_eq!(
            offset_in_viewport_after, offset_in_viewport_before,
            "Entry should maintain same relative position in viewport (stable view)"
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
    fn hit_test_before_layout_returns_miss() {
        // Reproduces bug: hit_test called before recompute_layout
        // HeightIndex is empty (len=0) even though entries exist
        let entries = vec![make_valid_entry("uuid-1"), make_valid_entry("uuid-2")];
        let state = ConversationViewState::new(
            None,
            None,
            entries,
            200_000,
            crate::model::PricingConfig::default(),
        );

        // NO layout computation - height_index.len() == 0

        // This should return Miss, not panic
        let result = state.hit_test(0, 10, LineOffset::new(0));

        assert_eq!(
            result,
            HitTestResult::Miss,
            "hit_test before layout should return Miss, not panic"
        );
    }

    #[test]
    fn hit_test_finds_first_entry() {
        let entries = vec![make_valid_entry("uuid-1"), make_valid_entry("uuid-2")];
        let mut state = ConversationViewState::new(
            None,
            None,
            entries,
            200_000,
            crate::model::PricingConfig::default(),
        );

        let params = LayoutParams::new(80, WrapMode::Wrap);
        state.recompute_layout(params);

        // Click at line 0 (first line of first entry, no scroll)
        let result = state.hit_test(0, 10, LineOffset::new(0));

        assert_eq!(
            result,
            HitTestResult::Hit {
                entry_index: EntryIndex::new(0),
                line_in_entry: 0,
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
        let mut state = ConversationViewState::new(
            None,
            None,
            entries,
            200_000,
            crate::model::PricingConfig::default(),
        );

        let params = LayoutParams::new(80, WrapMode::Wrap);
        state.recompute_layout(params);

        // Get actual entry positions
        let entry1_y = state.entry_cumulative_y(EntryIndex::new(1)).unwrap().get();

        // Click at first line of entry 1 with scroll
        // Use entry1_y as scroll offset to put entry 1 at top of screen
        let result = state.hit_test(0, 20, LineOffset::new(entry1_y));

        assert_eq!(
            result,
            HitTestResult::Hit {
                entry_index: EntryIndex::new(1),
                line_in_entry: 0,
                column: 20
            }
        );
    }

    #[test]
    fn hit_test_beyond_content_returns_miss() {
        let entries = vec![make_valid_entry("uuid-1")];
        let mut state = ConversationViewState::new(
            None,
            None,
            entries,
            200_000,
            crate::model::PricingConfig::default(),
        );

        let params = LayoutParams::new(80, WrapMode::Wrap);
        state.recompute_layout(params); // Total height = 10

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
        let mut state = ConversationViewState::new(
            None,
            None,
            entries,
            200_000,
            crate::model::PricingConfig::default(),
        );

        let params = LayoutParams::new(80, WrapMode::Wrap);
        state.recompute_layout(params);

        // Click at exact start of entry 1
        let entry1_y = state.entry_cumulative_y(EntryIndex::new(1)).unwrap().get();
        let result = state.hit_test(entry1_y as u16, 5, LineOffset::new(0));

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
        let mut state = ConversationViewState::new(
            None,
            None,
            entries,
            200_000,
            crate::model::PricingConfig::default(),
        );

        let params = LayoutParams::new(80, WrapMode::Wrap);
        state.recompute_layout(params);

        // Click at last line of entry 0
        let entry0_height = state.entries[0].height().get();
        let last_line = entry0_height - 1;
        let result = state.hit_test(last_line, 15, LineOffset::new(0));

        assert_eq!(
            result,
            HitTestResult::Hit {
                entry_index: EntryIndex::new(0),
                line_in_entry: last_line as usize,
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
        let mut state = ConversationViewState::new(
            None,
            None,
            entries,
            200_000,
            crate::model::PricingConfig::default(),
        );

        let params = LayoutParams::new(80, WrapMode::Wrap);
        state.recompute_layout(params);

        // Test clicking at first line of entry 1 with scroll
        let entry0_height = state.entries[0].height().get() as usize;
        let entry1_y = state.entry_cumulative_y(EntryIndex::new(1)).unwrap().get();

        // Scroll so entry 0's last line is at screen_y=0, entry 1's first line at screen_y=1
        let scroll_offset = entry0_height.saturating_sub(1);
        let result = state.hit_test(1, 0, LineOffset::new(scroll_offset));

        assert_eq!(
            result,
            HitTestResult::Hit {
                entry_index: EntryIndex::new(1),
                line_in_entry: 0,
                column: 0
            },
            "Boundary with scroll should correctly identify entry"
        );

        // Test last line of entry 1 with scroll
        let entry1_height = state.entries[1].height().get();
        let last_line_in_entry = entry1_height - 1;

        // Scroll to show entry 1 at top of screen
        let result = state.hit_test(last_line_in_entry, 10, LineOffset::new(entry1_y));

        assert_eq!(
            result,
            HitTestResult::Hit {
                entry_index: EntryIndex::new(1),
                line_in_entry: last_line_in_entry as usize,
                column: 10
            },
            "Last line boundary with scroll should correctly identify entry"
        );
    }

    #[test]
    fn hit_test_single_entry_all_positions() {
        let entries = vec![make_valid_entry("uuid-1")];
        let mut state = ConversationViewState::new(
            None,
            None,
            entries,
            200_000,
            crate::model::PricingConfig::default(),
        );

        let params = LayoutParams::new(80, WrapMode::Wrap);
        state.recompute_layout(params);

        let entry_height = state.entries[0].height().get();

        // Test all valid positions within the entry
        for line in 0..entry_height {
            let result = state.hit_test(line, 0, LineOffset::new(0));
            assert_eq!(
                result,
                HitTestResult::Hit {
                    entry_index: EntryIndex::new(0),
                    line_in_entry: line as usize,
                    column: 0
                },
                "Line {} should be hit",
                line
            );
        }

        // Test position beyond entry
        let result = state.hit_test(entry_height, 0, LineOffset::new(0));
        assert_eq!(result, HitTestResult::Miss, "Line beyond entry should miss");
    }

    // === needs_relayout Tests ===

    #[test]
    fn needs_relayout_true_when_params_change() {
        let entries = vec![make_valid_entry("uuid-1")];
        let mut state = ConversationViewState::new(
            None,
            None,
            entries,
            200_000,
            crate::model::PricingConfig::default(),
        );

        let params1 = LayoutParams::new(80, WrapMode::Wrap);
        state.recompute_layout(params1);

        let params2 = LayoutParams::new(120, WrapMode::Wrap); // Different width
        assert!(state.needs_relayout(&params2));
    }

    #[test]
    fn needs_relayout_false_when_params_unchanged() {
        let entries = vec![make_valid_entry("uuid-1")];
        let mut state = ConversationViewState::new(
            None,
            None,
            entries,
            200_000,
            crate::model::PricingConfig::default(),
        );

        let params = LayoutParams::new(80, WrapMode::Wrap);
        state.recompute_layout(params);

        assert!(!state.needs_relayout(&params));
    }

    // === append Tests ===

    #[test]
    fn append_adds_entries_to_end() {
        let mut state = ConversationViewState::new(
            None,
            None,
            vec![make_valid_entry("uuid-1")],
            200_000,
            crate::model::PricingConfig::default(),
        );

        state.append(vec![make_valid_entry("uuid-2"), make_valid_entry("uuid-3")]);

        assert_eq!(state.len(), 3);
        assert_eq!(
            state.get(EntryIndex::new(2)).unwrap().index(),
            EntryIndex::new(2)
        );
    }

    #[test]
    fn append_invalidates_layout() {
        let mut state = ConversationViewState::new(
            None,
            None,
            vec![make_valid_entry("uuid-1")],
            200_000,
            crate::model::PricingConfig::default(),
        );

        let params = LayoutParams::new(80, WrapMode::Wrap);
        state.recompute_layout(params);

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
        let mut state = ConversationViewState::new(
            None,
            None,
            entries,
            200_000,
            crate::model::PricingConfig::default(),
        );

        let params = LayoutParams::new(80, WrapMode::Wrap);
        state.recompute_layout(params);

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
        state.set_wrap_override(EntryIndex::new(0), Some(WrapMode::NoWrap), params);

        assert_eq!(
            state.get(EntryIndex::new(0)).unwrap().wrap_override(),
            Some(WrapMode::NoWrap)
        );
    }

    #[test]
    fn set_wrap_override_returns_previous_value() {
        let entries = vec![make_valid_entry("uuid-1")];
        let mut state = ConversationViewState::new(
            None,
            None,
            entries,
            200_000,
            crate::model::PricingConfig::default(),
        );

        let params = LayoutParams::new(80, WrapMode::Wrap);
        state.recompute_layout(params);

        // First call: previous was None
        let result = state.set_wrap_override(EntryIndex::new(0), Some(WrapMode::NoWrap), params);
        assert_eq!(result, Some(None));

        // Second call: previous was Some(NoWrap)
        let result = state.set_wrap_override(EntryIndex::new(0), Some(WrapMode::Wrap), params);
        assert_eq!(result, Some(Some(WrapMode::NoWrap)));

        // Third call: clearing override
        let result = state.set_wrap_override(EntryIndex::new(0), None, params);
        assert_eq!(result, Some(Some(WrapMode::Wrap)));
    }

    #[test]
    fn set_wrap_override_returns_none_for_invalid_index() {
        let mut state = ConversationViewState::empty();
        let params = LayoutParams::new(80, WrapMode::Wrap);

        let result = state.set_wrap_override(EntryIndex::new(0), Some(WrapMode::NoWrap), params);

        assert_eq!(result, None);

        // Also test out of bounds on non-empty state
        let mut state = ConversationViewState::new(
            None,
            None,
            vec![make_valid_entry("uuid-1")],
            200_000,
            crate::model::PricingConfig::default(),
        );
        state.recompute_layout(params);

        let result = state.set_wrap_override(EntryIndex::new(999), Some(WrapMode::NoWrap), params);

        assert_eq!(result, None);
    }

    #[test]
    fn set_wrap_override_triggers_relayout_from_index() {
        let entries = vec![
            make_valid_entry("uuid-1"),
            make_valid_entry("uuid-2"),
            make_valid_entry("uuid-3"),
        ];
        let mut state = ConversationViewState::new(
            None,
            None,
            entries,
            200_000,
            crate::model::PricingConfig::default(),
        );

        let params = LayoutParams::new(80, WrapMode::Wrap);
        state.recompute_layout(params);

        // Record initial cumulative_y values
        let y0_before = state.entry_cumulative_y(EntryIndex::new(0)).unwrap().get();
        let y1_before = state.entry_cumulative_y(EntryIndex::new(1)).unwrap().get();

        // First entry always at y=0
        assert_eq!(y0_before, 0);

        // Set wrap override on entry 1
        state.set_wrap_override(EntryIndex::new(1), Some(WrapMode::NoWrap), params);

        // Verify entry 0 unchanged (before the wrap override)
        assert_eq!(
            state.entry_cumulative_y(EntryIndex::new(0)).unwrap().get(),
            y0_before,
            "Entry 0 should be unchanged"
        );

        // Entry 1 should be at same position
        assert_eq!(
            state.entry_cumulative_y(EntryIndex::new(1)).unwrap().get(),
            y1_before,
            "Entry 1 should be at same position"
        );

        // Entry 2's position is entry 1's y + entry 1's new height
        let entry1_height = state.entries[1].height().get() as usize;
        assert_eq!(
            state.entry_cumulative_y(EntryIndex::new(2)).unwrap().get(),
            y1_before + entry1_height,
            "Entry 2's y should be entry 1's y + entry 1's new height"
        );

        // Verify invariant
        let total: usize = state
            .entries()
            .iter()
            .map(|e| e.height().get() as usize)
            .sum();
        assert_eq!(
            state.total_height(),
            total,
            "Total height must equal sum of entry heights"
        );
    }

    // === Horizontal Scrolling Tests ===

    #[test]
    fn horizontal_offset_starts_at_zero() {
        let entries = vec![make_valid_entry("uuid-1")];
        let state = ConversationViewState::new(
            None,
            None,
            entries,
            200_000,
            crate::model::PricingConfig::default(),
        );

        assert_eq!(
            state.horizontal_offset(),
            0,
            "Horizontal offset should start at 0"
        );
    }

    #[test]
    fn set_horizontal_offset_updates_value() {
        let entries = vec![make_valid_entry("uuid-1")];
        let mut state = ConversationViewState::new(
            None,
            None,
            entries,
            200_000,
            crate::model::PricingConfig::default(),
        );

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
        let mut state = ConversationViewState::new(
            None,
            None,
            entries,
            200_000,
            crate::model::PricingConfig::default(),
        );

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
        let mut state = ConversationViewState::new(
            None,
            None,
            entries,
            200_000,
            crate::model::PricingConfig::default(),
        );

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
        let mut state = ConversationViewState::new(
            None,
            None,
            entries,
            200_000,
            crate::model::PricingConfig::default(),
        );

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
        let mut state = ConversationViewState::new(
            None,
            None,
            entries,
            200_000,
            crate::model::PricingConfig::default(),
        );

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
        let mut state = ConversationViewState::new(
            None,
            None,
            entries,
            200_000,
            crate::model::PricingConfig::default(),
        );

        let params = LayoutParams::new(80, WrapMode::Wrap);
        state.recompute_layout(params);

        let entry = state.get(EntryIndex::new(0)).unwrap();

        // Initially uses global
        assert_eq!(entry.effective_wrap(WrapMode::Wrap), WrapMode::Wrap);
        assert_eq!(entry.effective_wrap(WrapMode::NoWrap), WrapMode::NoWrap);

        // Set override to NoWrap
        state.set_wrap_override(EntryIndex::new(0), Some(WrapMode::NoWrap), params);

        let entry = state.get(EntryIndex::new(0)).unwrap();

        // Now always returns override regardless of global
        assert_eq!(entry.effective_wrap(WrapMode::Wrap), WrapMode::NoWrap);
        assert_eq!(entry.effective_wrap(WrapMode::NoWrap), WrapMode::NoWrap);

        // Set override to Wrap
        state.set_wrap_override(EntryIndex::new(0), Some(WrapMode::Wrap), params);

        let entry = state.get(EntryIndex::new(0)).unwrap();

        // Now always returns Wrap
        assert_eq!(entry.effective_wrap(WrapMode::Wrap), WrapMode::Wrap);
        assert_eq!(entry.effective_wrap(WrapMode::NoWrap), WrapMode::Wrap);

        // Clear override
        state.set_wrap_override(EntryIndex::new(0), None, params);

        let entry = state.get(EntryIndex::new(0)).unwrap();

        // Back to using global
        assert_eq!(entry.effective_wrap(WrapMode::Wrap), WrapMode::Wrap);
        assert_eq!(entry.effective_wrap(WrapMode::NoWrap), WrapMode::NoWrap);
    }

    // === approximate_scroll_line Tests ===

    #[test]
    fn approximate_scroll_line_at_top() {
        let entries = vec![make_valid_entry("uuid-1"), make_valid_entry("uuid-2")];
        let mut state = ConversationViewState::new(
            None,
            None,
            entries,
            200_000,
            crate::model::PricingConfig::default(),
        );
        let params = LayoutParams::new(80, WrapMode::Wrap);
        state.recompute_layout(params);

        state.set_scroll(ScrollPosition::Top);

        assert_eq!(
            state.approximate_scroll_line(),
            0,
            "Top scroll should approximate to line 0"
        );
    }

    #[test]
    fn approximate_scroll_line_at_bottom() {
        let entries = vec![make_valid_entry("uuid-1"), make_valid_entry("uuid-2")];
        let mut state = ConversationViewState::new(
            None,
            None,
            entries,
            200_000,
            crate::model::PricingConfig::default(),
        );
        let params = LayoutParams::new(80, WrapMode::Wrap);
        state.recompute_layout(params);

        state.set_scroll(ScrollPosition::Bottom);

        assert_eq!(
            state.approximate_scroll_line(),
            usize::MAX,
            "Bottom scroll should approximate to usize::MAX"
        );
    }

    #[test]
    fn approximate_scroll_line_at_specific_line() {
        let entries = vec![make_valid_entry("uuid-1"), make_valid_entry("uuid-2")];
        let mut state = ConversationViewState::new(
            None,
            None,
            entries,
            200_000,
            crate::model::PricingConfig::default(),
        );
        let params = LayoutParams::new(80, WrapMode::Wrap);
        state.recompute_layout(params);

        state.set_scroll(ScrollPosition::at_line(15));

        assert_eq!(
            state.approximate_scroll_line(),
            15,
            "AtLine(15) should approximate to 15"
        );
    }

    #[test]
    fn approximate_scroll_line_at_entry() {
        let entries = vec![
            make_valid_entry("uuid-1"),
            make_valid_entry("uuid-2"),
            make_valid_entry("uuid-3"),
        ];
        let mut state = ConversationViewState::new(
            None,
            None,
            entries,
            200_000,
            crate::model::PricingConfig::default(),
        );
        let params = LayoutParams::new(80, WrapMode::Wrap);
        state.recompute_layout(params);

        state.set_scroll(ScrollPosition::at_entry(EntryIndex::new(1)));

        let expected = state.entry_cumulative_y(EntryIndex::new(1)).unwrap().get();
        assert_eq!(
            state.approximate_scroll_line(),
            expected,
            "AtEntry(1) should approximate to entry 1's cumulative_y"
        );
    }

    #[test]
    fn approximate_scroll_line_with_fraction() {
        let entries = vec![
            make_valid_entry("uuid-1"),
            make_valid_entry("uuid-2"),
            make_valid_entry("uuid-3"),
        ];
        let mut state = ConversationViewState::new(
            None,
            None,
            entries,
            200_000,
            crate::model::PricingConfig::default(),
        );
        let params = LayoutParams::new(80, WrapMode::Wrap);
        state.recompute_layout(params);

        state.set_scroll(ScrollPosition::Fraction(0.5));

        let total_height = state.total_height();
        let expected = total_height / 2;
        assert_eq!(
            state.approximate_scroll_line(),
            expected,
            "Fraction(0.5) should approximate to half of total_height"
        );
    }

    // === Bug Reproduction: append() leaves stale total_height (cclv-5ur.21) ===

    #[test]
    fn append_entries_updates_layout_immediately_for_auto_scroll() {
        // RED TEST for cclv-5ur.21: Auto-scroll must fill last viewport line with content
        //
        // REQUIREMENT: After appending entries, total_height must reflect the new content
        // so auto-scroll resolution includes the new entries in visible_range.
        //
        // This test verifies append_entries() (the FIXED method) updates total_height.

        // Create initial state with 2 entries
        let entries = vec![
            make_entry_with_n_lines("uuid-1", 5),
            make_entry_with_n_lines("uuid-2", 5),
        ];
        let mut state = ConversationViewState::new(
            None,
            None,
            entries,
            200_000,
            crate::model::PricingConfig::default(),
        );

        // Relayout so we have known total_height
        state.relayout(80, WrapMode::Wrap, &crate::state::SearchState::Inactive);

        let height_before = state.total_height();
        assert!(
            height_before > 0,
            "Should have non-zero height after relayout"
        );

        // Append new entry using NEW append_entries() method
        let new_entries = vec![make_entry_with_n_lines("uuid-3", 5)];
        state.append_entries(new_entries, &crate::state::SearchState::Inactive);

        let height_after = state.total_height();

        // EXPECTATION: total_height should INCREASE after appending new entry
        assert!(
            height_after > height_before,
            "append_entries() should update total_height immediately.\n\
             Before: {}, After: {}\n\
             New entries must be included in layout for auto-scroll to work.",
            height_before,
            height_after
        );
    }

    #[test]
    fn append_entries_updates_total_height_immediately() {
        // CONTRAST TEST: append_entries() correctly updates total_height
        //
        // This is the FIXED behavior that SessionViewState should use.

        // Create initial state with 2 entries
        let entries = vec![
            make_entry_with_n_lines("uuid-1", 5),
            make_entry_with_n_lines("uuid-2", 5),
        ];
        let mut state = ConversationViewState::new(
            None,
            None,
            entries,
            200_000,
            crate::model::PricingConfig::default(),
        );

        // Relayout so we have known total_height
        state.relayout(80, WrapMode::Wrap, &crate::state::SearchState::Inactive);

        let height_before = state.total_height();
        assert!(
            height_before > 0,
            "Should have non-zero height after relayout"
        );

        // Now use NEW append_entries() method
        let new_entries = vec![make_entry_with_n_lines("uuid-3", 5)];
        state.append_entries(new_entries, &crate::state::SearchState::Inactive);

        let height_after = state.total_height();

        // CORRECT: total_height should INCREASE after appending new entry
        assert!(
            height_after > height_before,
            "append_entries() should increase total_height.\n\
             Before: {}, After: {}",
            height_before,
            height_after
        );
    }
}

// HeightIndex integration tests
#[cfg(test)]
#[path = "conversation_height_index_tests.rs"]
mod height_index_tests;
