//! Entry view with per-entry state and precomputed rendered lines.

use super::renderer::compute_entry_lines;
use super::types::{EntryIndex, LineHeight};
use crate::model::ConversationEntry;
use crate::state::{WrapContext, WrapMode};
use ratatui::text::Line;

/// A conversation entry with precomputed rendered lines and presentation state.
///
/// EntryView OWNS the domain entry (ConversationEntry) rather than
/// referencing it. This provides:
/// - Cache locality (entry + rendered lines + view state in same allocation)
/// - No lifetime complexity
/// - Simple streaming append
/// - O(1) access to per-entry view state (no HashSet lookups)
///
/// # Ownership (FR-002)
/// View-state layer owns domain data. Entries are parsed directly
/// into EntryView during JSON processing.
///
/// # Source of Truth for Height
/// The `rendered_lines` field is THE source of truth for entry height.
/// `height()` returns `LineHeight` based on `rendered_lines.len()`.
/// This ensures perfect consistency between computed layout and actual rendering.
///
/// # Per-Entry Presentation State
/// - `expanded`: Whether entry shows full content or collapsed summary (FR-031)
/// - `wrap_override`: Optional per-entry wrap mode override (FR-048)
/// - `accumulated_tokens`: Running sum of tokens from conversation start to this entry (inclusive)
/// - `max_context_tokens`: Context window size (for percentage calculation)
/// - `pricing`: Model pricing info (for cost calculation)
///
/// # Malformed Entries
/// Malformed entries have minimal rendering (separator line only).
/// They still occupy a slot in the entry list to preserve index stability.
#[derive(Debug, Clone)]
pub struct EntryView {
    /// The domain entry (owned).
    entry: ConversationEntry,
    /// Index of this entry within its conversation.
    /// This is the canonical reference for entries.
    index: EntryIndex,
    /// Precomputed rendered lines (source of truth for height).
    /// These are cached ratatui Lines ready for rendering.
    pub(crate) rendered_lines: Vec<Line<'static>>,
    /// Whether this entry is expanded (shows full content).
    /// Collapsed entries show summary + "(+N more lines)" indicator.
    expanded: bool,
    /// Per-entry wrap mode override.
    /// `None` = use global wrap mode.
    /// `Some(mode)` = override global with this specific mode.
    wrap_override: Option<WrapMode>,
    /// Accumulated tokens from conversation start up to and including this entry.
    /// Used for rendering context divider with percentage.
    accumulated_tokens: usize,
    /// Maximum context window size (from config).
    /// Used for percentage calculation in divider.
    max_context_tokens: usize,
    /// Pricing configuration (from config).
    /// Used for cost calculation in divider.
    pricing: crate::model::PricingConfig,
}

impl EntryView {
    /// Default collapse threshold (lines before collapsing).
    const COLLAPSE_THRESHOLD: usize = 10;

    /// Default summary lines (shown when collapsed).
    const SUMMARY_LINES: usize = 3;

    /// Create new EntryView with minimal state (for initial construction).
    ///
    /// This constructor creates an EntryView with empty rendered_lines.
    /// Call `recompute_lines()` after construction to populate rendered_lines.
    ///
    /// This is used during ConversationViewState construction where layout
    /// parameters (width, wrap_mode) are not yet available.
    ///
    /// # Arguments
    /// * `entry` - Domain entry to wrap
    /// * `index` - Position within conversation
    /// * `accumulated_tokens` - Running sum of tokens up to and including this entry
    /// * `max_context_tokens` - Context window size from config
    /// * `pricing` - Model pricing information from config
    pub fn new(
        entry: ConversationEntry,
        index: EntryIndex,
        accumulated_tokens: usize,
        max_context_tokens: usize,
        pricing: crate::model::PricingConfig,
    ) -> Self {
        Self {
            entry,
            index,
            rendered_lines: vec![Line::from("")], // Minimal placeholder (1 line minimum)
            expanded: false,
            wrap_override: None,
            accumulated_tokens,
            max_context_tokens,
            pricing,
        }
    }

    /// Create new EntryView with precomputed rendered lines.
    ///
    /// This constructor:
    /// - Calls compute_entry_lines to generate rendered output
    /// - Starts in collapsed state (expanded=false)
    /// - Uses default collapse thresholds (10/3)
    /// - Has no wrap override (uses global wrap mode)
    ///
    /// # Arguments
    /// * `entry` - Domain entry to wrap
    /// * `index` - Position within conversation
    /// * `wrap_mode` - Effective wrap mode for this entry
    /// * `width` - Viewport width for text wrapping
    /// * `accumulated_tokens` - Running sum of tokens up to and including this entry
    /// * `max_context_tokens` - Context window size from config
    /// * `pricing` - Model pricing information from config
    pub fn with_rendered_lines(
        entry: ConversationEntry,
        index: EntryIndex,
        wrap_mode: WrapMode,
        width: u16,
        accumulated_tokens: usize,
        max_context_tokens: usize,
        pricing: crate::model::PricingConfig,
    ) -> Self {
        let expanded = false; // Start collapsed
        // New entries have no wrap override, so use global mode
        let wrap_ctx = WrapContext::from_global(wrap_mode);
        // TODO: Pass MessageStyles from caller instead of default
        let styles = crate::view::MessageStyles::new();
        let rendered_lines = compute_entry_lines(
            &entry,
            expanded,
            wrap_ctx,
            width,
            Self::COLLAPSE_THRESHOLD,
            Self::SUMMARY_LINES,
            &styles,
            Some(index.get()),                    // Pass entry index for prefixing
            false,                                // TODO: Pass is_subagent_view from caller
            &crate::state::SearchState::Inactive, // TODO: Pass search_state from caller
            false,                                // Default to not focused on creation
            accumulated_tokens as u64,
            max_context_tokens as u64,
            &pricing,
        );
        Self {
            entry,
            index,
            rendered_lines,
            expanded,
            wrap_override: None,
            accumulated_tokens,
            max_context_tokens,
            pricing,
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

    /// Get the entry UUID (if valid entry).
    pub fn uuid(&self) -> Option<&crate::model::EntryUuid> {
        match &self.entry {
            ConversationEntry::Valid(log_entry) => Some(log_entry.uuid()),
            ConversationEntry::Malformed(_) => None,
        }
    }

    /// Get accumulated token count (running sum up to and including this entry).
    pub fn accumulated_tokens(&self) -> usize {
        self.accumulated_tokens
    }

    /// Get the height of this entry (count of rendered lines).
    ///
    /// This is derived from `rendered_lines.len()` and is the source of truth
    /// for entry height. The returned LineHeight is guaranteed to be >= 1
    /// for all entries (minimum is separator line).
    pub fn height(&self) -> LineHeight {
        let len = self.rendered_lines.len() as u16;
        // LineHeight::new validates >= 1, but we guarantee at least 1 line (separator)
        LineHeight::new(len).unwrap_or(LineHeight::ONE)
    }

    /// Get reference to the rendered lines.
    ///
    /// These are precomputed ratatui Lines ready for rendering.
    /// The slice has 'static lifetime because all content is owned.
    pub fn rendered_lines(&self) -> &[Line<'static>] {
        &self.rendered_lines
    }

    /// Recompute rendered lines after state change.
    ///
    /// This is called by ConversationViewState when:
    /// - Viewport width changes
    /// - Wrap mode changes
    /// - Entry expand/collapse state changes
    /// - Search state changes (for highlighting)
    ///
    /// This is `pub(crate)` because only ConversationViewState should
    /// trigger recomputation (to maintain HeightIndex consistency).
    #[allow(dead_code)] // Will be used by ConversationViewState in future tasks
    pub(crate) fn recompute_lines(
        &mut self,
        wrap_mode: WrapMode,
        width: u16,
        search_state: &crate::state::SearchState,
        focused: bool,
    ) {
        // Bug fix cclv-5ur.22: Create WrapContext that encodes override status
        let wrap_ctx = match self.wrap_override {
            Some(override_mode) => WrapContext::from_override(override_mode),
            None => WrapContext::from_global(wrap_mode),
        };
        // TODO: Pass MessageStyles from caller instead of default
        let styles = crate::view::MessageStyles::new();
        self.rendered_lines = compute_entry_lines(
            &self.entry,
            self.expanded,
            wrap_ctx,
            width,
            Self::COLLAPSE_THRESHOLD,
            Self::SUMMARY_LINES,
            &styles,
            Some(self.index.get()), // Pass entry index for prefixing
            false,                  // TODO: Pass is_subagent_view from caller
            search_state,           // Bug fix cclv-5ur.73: Pass search_state for highlighting
            focused,
            self.accumulated_tokens as u64,
            self.max_context_tokens as u64,
            &self.pricing,
        );
    }

    /// Check if this entry is expanded.
    pub fn is_expanded(&self) -> bool {
        self.expanded
    }

    /// Get the wrap mode override.
    pub fn wrap_override(&self) -> Option<WrapMode> {
        self.wrap_override
    }

    /// Get the effective wrap mode (override or global fallback).
    pub fn effective_wrap(&self, global: WrapMode) -> WrapMode {
        self.wrap_override.unwrap_or(global)
    }

    // NOTE: Mutation methods are pub(crate) for now to allow ConversationViewState
    // to call them during the refactoring. After the refactoring is complete,
    // ConversationViewState will handle recompute_lines() and these can be private.

    /// Set the expanded state (internal - called by ConversationViewState).
    #[allow(dead_code)] // Will be used by ConversationViewState in future tasks
    pub(crate) fn set_expanded(&mut self, expanded: bool) {
        self.expanded = expanded;
    }

    /// Toggle expanded state and return the new state (internal - called by ConversationViewState).
    pub(crate) fn toggle_expanded(&mut self) -> bool {
        self.expanded = !self.expanded;
        self.expanded
    }

    /// Set the wrap mode override (internal - called by ConversationViewState).
    pub(crate) fn set_wrap_override(&mut self, mode: Option<WrapMode>) {
        self.wrap_override = mode;
    }
}

// Include refactor tests
#[cfg(test)]
#[path = "entry_view_refactor_tests.rs"]
mod refactor_tests;

// Keep existing tests for now (will update after implementation)
#[cfg(test)]
mod legacy_tests {
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

    #[allow(dead_code)] // Will be used when updating more tests
    fn make_malformed_entry() -> ConversationEntry {
        ConversationEntry::Malformed(MalformedEntry::new(
            42,
            "bad json",
            "Parse error",
            Some(make_session_id("session-1")),
        ))
    }

    // ===== Legacy Tests (will be updated) =====
    // These tests use the OLD API and will fail with stubs.
    // We'll update them after implementing the new API.

    #[test]
    fn new_creates_entry_with_minimal_state() {
        let entry = make_valid_entry();
        let index = EntryIndex::new(0);

        // NEW API: EntryView::new creates minimal placeholder
        let view = EntryView::new(
            entry,
            index,
            0,
            200_000,
            crate::model::PricingConfig::default(),
        );

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
        // rendered_lines will be minimal placeholder (1 line)
        assert_eq!(
            view.rendered_lines().len(),
            1,
            "Should have placeholder line"
        );
    }

    // Additional legacy tests omitted for brevity.
    // They will be updated in the implementation phase.
}
