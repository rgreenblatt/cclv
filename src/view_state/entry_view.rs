//! Entry view with per-entry state and layout.

use super::layout::EntryLayout;
use super::types::EntryIndex;
use crate::model::ConversationEntry;
use crate::state::WrapMode;

/// A conversation entry with its computed layout and presentation state.
///
/// EntryView OWNS the domain entry (ConversationEntry) rather than
/// referencing it. This provides:
/// - Cache locality (entry + layout + view state in same allocation)
/// - No lifetime complexity
/// - Simple streaming append
/// - O(1) access to per-entry view state (no HashSet lookups)
///
/// # Ownership (FR-002)
/// View-state layer owns domain data. Entries are parsed directly
/// into EntryView during JSON processing.
///
/// # Per-Entry Presentation State
/// - `expanded`: Whether entry shows full content or collapsed summary (FR-031)
/// - `wrap_override`: Optional per-entry wrap mode override (FR-048)
///
/// # Malformed Entries
/// Malformed entries (parse failures) have `LineHeight::ZERO` and are
/// skipped during rendering. They still occupy a slot in the entry list
/// to preserve index stability.
#[derive(Debug, Clone)]
pub struct EntryView {
    /// The domain entry (owned).
    entry: ConversationEntry,
    /// Index of this entry within its conversation.
    /// This is the canonical reference for entries.
    index: EntryIndex,
    /// Computed layout for current viewport parameters.
    layout: EntryLayout,
    /// Whether this entry is expanded (shows full content).
    /// Collapsed entries show summary + "(+N more lines)" indicator.
    expanded: bool,
    /// Per-entry wrap mode override.
    /// `None` = use global wrap mode.
    /// `Some(mode)` = override global with this specific mode.
    wrap_override: Option<WrapMode>,
}

impl EntryView {
    /// Create new EntryView with default state (collapsed, no wrap override).
    pub fn new(entry: ConversationEntry, index: EntryIndex) -> Self {
        Self {
            entry,
            index,
            layout: EntryLayout::default(),
            expanded: false,
            wrap_override: None,
        }
    }

    /// Create EntryView with precomputed layout.
    /// Used internally during layout computation.
    #[allow(dead_code)] // Will be used by ConversationViewState
    pub(crate) fn with_layout(
        entry: ConversationEntry,
        index: EntryIndex,
        layout: EntryLayout,
    ) -> Self {
        Self {
            entry,
            index,
            layout,
            expanded: false,
            wrap_override: None,
        }
    }

    /// Get the entry index (0-based).
    pub fn index(&self) -> EntryIndex {
        self.index
    }

    /// Get the display index (1-based for UI).
    pub fn display_index(&self) -> usize {
        self.index.display()
    }

    /// Get reference to the domain entry.
    pub fn entry(&self) -> &ConversationEntry {
        &self.entry
    }

    /// Get reference to the layout.
    pub fn layout(&self) -> &EntryLayout {
        &self.layout
    }

    /// Update the layout (called during relayout).
    #[allow(dead_code)] // Will be used by ConversationViewState
    pub(crate) fn set_layout(&mut self, layout: EntryLayout) {
        self.layout = layout;
    }

    /// Check if this entry is expanded.
    pub fn is_expanded(&self) -> bool {
        self.expanded
    }

    /// Set the expanded state.
    pub fn set_expanded(&mut self, expanded: bool) {
        self.expanded = expanded;
    }

    /// Toggle expanded state and return the new state.
    pub fn toggle_expanded(&mut self) -> bool {
        self.expanded = !self.expanded;
        self.expanded
    }

    /// Get the wrap mode override.
    pub fn wrap_override(&self) -> Option<WrapMode> {
        self.wrap_override
    }

    /// Set the wrap mode override.
    pub fn set_wrap_override(&mut self, mode: Option<WrapMode>) {
        self.wrap_override = mode;
    }

    /// Get the effective wrap mode (override or global fallback).
    pub fn effective_wrap(&self, global: WrapMode) -> WrapMode {
        self.wrap_override.unwrap_or(global)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{
        EntryMetadata, EntryType, EntryUuid, LogEntry, MalformedEntry, Message, MessageContent,
        Role, SessionId,
    };
    use crate::view_state::types::LineHeight;
    use crate::view_state::types::LineOffset;

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

    fn make_valid_entry() -> ConversationEntry {
        let log_entry = LogEntry::new(
            make_entry_uuid("uuid-1"),
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

    // ===== EntryView::new Tests =====

    #[test]
    fn new_creates_entry_with_default_state() {
        let entry = make_valid_entry();
        let index = EntryIndex::new(0);

        let view = EntryView::new(entry, index);

        assert_eq!(view.index(), index);
        assert!(
            !view.is_expanded(),
            "Default state should be collapsed (not expanded)"
        );
        assert_eq!(
            view.wrap_override(),
            None,
            "Default should have no wrap override"
        );
    }

    #[test]
    fn new_with_different_index() {
        let entry = make_valid_entry();
        let index = EntryIndex::new(42);

        let view = EntryView::new(entry, index);

        assert_eq!(view.index(), index);
        assert_eq!(view.display_index(), 43, "Display index should be 1-based");
    }

    #[test]
    fn new_preserves_entry() {
        let entry = make_valid_entry();
        let index = EntryIndex::new(0);

        let view = EntryView::new(entry.clone(), index);

        assert!(view.entry().is_valid());
    }

    // ===== EntryView::with_layout Tests =====

    #[test]
    fn with_layout_creates_entry_with_given_layout() {
        let entry = make_valid_entry();
        let index = EntryIndex::new(0);
        let layout = EntryLayout::new(LineHeight::new(5).unwrap(), LineOffset::new(10));

        let view = EntryView::with_layout(entry, index, layout);

        assert_eq!(view.layout().height(), LineHeight::new(5).unwrap());
        assert_eq!(view.layout().cumulative_y(), LineOffset::new(10));
    }

    #[test]
    fn with_layout_still_has_default_presentation_state() {
        let entry = make_valid_entry();
        let index = EntryIndex::new(0);
        let layout = EntryLayout::new(LineHeight::new(3).unwrap(), LineOffset::new(0));

        let view = EntryView::with_layout(entry, index, layout);

        assert!(
            !view.is_expanded(),
            "Should default to collapsed even with custom layout"
        );
        assert_eq!(view.wrap_override(), None, "Should have no wrap override");
    }

    // ===== Index Accessor Tests =====

    #[test]
    fn index_returns_stored_index() {
        let entry = make_valid_entry();
        let index = EntryIndex::new(7);
        let view = EntryView::new(entry, index);

        assert_eq!(view.index(), EntryIndex::new(7));
    }

    #[test]
    fn display_index_returns_one_based() {
        let entry = make_valid_entry();
        let view = EntryView::new(entry, EntryIndex::new(0));

        assert_eq!(view.display_index(), 1);
    }

    #[test]
    fn display_index_for_later_entry() {
        let entry = make_valid_entry();
        let view = EntryView::new(entry, EntryIndex::new(99));

        assert_eq!(view.display_index(), 100);
    }

    // ===== Entry Accessor Tests =====

    #[test]
    fn entry_returns_reference_to_domain_entry() {
        let entry = make_valid_entry();
        let index = EntryIndex::new(0);
        let view = EntryView::new(entry, index);

        assert!(view.entry().is_valid());
    }

    #[test]
    fn entry_works_with_malformed() {
        let entry = make_malformed_entry();
        let index = EntryIndex::new(0);
        let view = EntryView::new(entry, index);

        assert!(view.entry().is_malformed());
    }

    // ===== Layout Tests =====

    #[test]
    fn layout_returns_reference() {
        let entry = make_valid_entry();
        let index = EntryIndex::new(0);
        let layout = EntryLayout::new(LineHeight::new(10).unwrap(), LineOffset::new(50));
        let view = EntryView::with_layout(entry, index, layout);

        let layout_ref = view.layout();
        assert_eq!(layout_ref.height(), LineHeight::new(10).unwrap());
        assert_eq!(layout_ref.cumulative_y(), LineOffset::new(50));
    }

    #[test]
    fn set_layout_updates_layout() {
        let entry = make_valid_entry();
        let index = EntryIndex::new(0);
        let initial_layout = EntryLayout::new(LineHeight::new(5).unwrap(), LineOffset::new(0));
        let mut view = EntryView::with_layout(entry, index, initial_layout);

        let new_layout = EntryLayout::new(LineHeight::new(10).unwrap(), LineOffset::new(20));
        view.set_layout(new_layout);

        assert_eq!(view.layout().height(), LineHeight::new(10).unwrap());
        assert_eq!(view.layout().cumulative_y(), LineOffset::new(20));
    }

    // ===== Expanded State Tests =====

    #[test]
    fn is_expanded_returns_false_by_default() {
        let entry = make_valid_entry();
        let view = EntryView::new(entry, EntryIndex::new(0));

        assert!(!view.is_expanded());
    }

    #[test]
    fn set_expanded_updates_state() {
        let entry = make_valid_entry();
        let mut view = EntryView::new(entry, EntryIndex::new(0));

        view.set_expanded(true);
        assert!(view.is_expanded());

        view.set_expanded(false);
        assert!(!view.is_expanded());
    }

    #[test]
    fn toggle_expanded_returns_new_state() {
        let entry = make_valid_entry();
        let mut view = EntryView::new(entry, EntryIndex::new(0));

        let new_state = view.toggle_expanded();
        assert!(new_state, "Should toggle from false to true");
        assert_eq!(
            view.is_expanded(),
            new_state,
            "Returned state should match stored state"
        );
    }

    #[test]
    fn toggle_expanded_is_idempotent_pair() {
        let entry = make_valid_entry();
        let mut view = EntryView::new(entry, EntryIndex::new(0));
        let initial = view.is_expanded();

        view.toggle_expanded();
        view.toggle_expanded();

        assert_eq!(
            view.is_expanded(),
            initial,
            "Double toggle should return to original state"
        );
    }

    #[test]
    fn toggle_expanded_alternates_correctly() {
        let entry = make_valid_entry();
        let mut view = EntryView::new(entry, EntryIndex::new(0));

        assert!(!view.is_expanded(), "Start collapsed");

        let state1 = view.toggle_expanded();
        assert!(state1, "First toggle: false -> true");
        assert!(view.is_expanded());

        let state2 = view.toggle_expanded();
        assert!(!state2, "Second toggle: true -> false");
        assert!(!view.is_expanded());

        let state3 = view.toggle_expanded();
        assert!(state3, "Third toggle: false -> true");
        assert!(view.is_expanded());
    }

    // ===== Wrap Override Tests =====

    #[test]
    fn wrap_override_returns_none_by_default() {
        let entry = make_valid_entry();
        let view = EntryView::new(entry, EntryIndex::new(0));

        assert_eq!(view.wrap_override(), None);
    }

    #[test]
    fn set_wrap_override_updates_state() {
        let entry = make_valid_entry();
        let mut view = EntryView::new(entry, EntryIndex::new(0));

        view.set_wrap_override(Some(WrapMode::Wrap));
        assert_eq!(view.wrap_override(), Some(WrapMode::Wrap));

        view.set_wrap_override(Some(WrapMode::NoWrap));
        assert_eq!(view.wrap_override(), Some(WrapMode::NoWrap));

        view.set_wrap_override(None);
        assert_eq!(view.wrap_override(), None);
    }

    // ===== Effective Wrap Tests =====

    #[test]
    fn effective_wrap_returns_override_when_some() {
        let entry = make_valid_entry();
        let mut view = EntryView::new(entry, EntryIndex::new(0));

        view.set_wrap_override(Some(WrapMode::NoWrap));

        let effective = view.effective_wrap(WrapMode::Wrap);
        assert_eq!(
            effective,
            WrapMode::NoWrap,
            "Should use override, not global"
        );
    }

    #[test]
    fn effective_wrap_returns_global_when_none() {
        let entry = make_valid_entry();
        let view = EntryView::new(entry, EntryIndex::new(0));

        let effective = view.effective_wrap(WrapMode::Wrap);
        assert_eq!(
            effective,
            WrapMode::Wrap,
            "Should use global when no override"
        );
    }

    #[test]
    fn effective_wrap_uses_override_regardless_of_global() {
        let entry = make_valid_entry();
        let mut view = EntryView::new(entry, EntryIndex::new(0));

        view.set_wrap_override(Some(WrapMode::Wrap));

        let effective1 = view.effective_wrap(WrapMode::NoWrap);
        assert_eq!(
            effective1,
            WrapMode::Wrap,
            "Override Wrap beats global NoWrap"
        );

        view.set_wrap_override(Some(WrapMode::NoWrap));
        let effective2 = view.effective_wrap(WrapMode::Wrap);
        assert_eq!(
            effective2,
            WrapMode::NoWrap,
            "Override NoWrap beats global Wrap"
        );
    }
}
