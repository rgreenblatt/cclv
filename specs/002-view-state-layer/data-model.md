# Data Model: View-State Layer

**Date**: 2025-12-27
**Status**: Design Complete
**Related**: [plan.md](./plan.md) | [research.md](./research.md) | [spec.md](./spec.md)

This document defines the type-driven data model for the view-state layer following the project constitution:
- **Smart constructors only**: Never export raw constructors
- **No primitive obsession**: Newtypes for all domain concepts
- **Illegal states unrepresentable**: Sum types enforce valid states
- **Parse at boundaries**: Validate once during construction
- **Ownership model**: View-state owns domain data directly

---

## 1. Core Newtypes

Newtypes for view-state specific concepts. Domain identifiers (`EntryUuid`, `SessionId`, `AgentId`) are reused from `model/identifiers.rs`.

```rust
// ===== src/view_state/types.rs =====

use std::num::NonZeroU16;

/// Height of an entry in lines. Always >= 1.
/// Invariant: Every entry occupies at least one line.
///
/// Note: Malformed entries have height 0 and are not rendered.
/// Use `LineHeight::ZERO` for malformed entries, which is a special
/// sentinel value that skips rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct LineHeight(u16);

/// Error when attempting to create LineHeight with invalid value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("LineHeight must be >= 1 for valid entries (got {0})")]
pub struct InvalidLineHeight(pub u16);

impl LineHeight {
    /// Sentinel value for malformed/non-rendered entries.
    pub const ZERO: Self = Self(0);

    /// Minimum valid height (1 line).
    pub const ONE: Self = Self(1);

    /// Create a new LineHeight for a valid entry.
    /// Returns error if height is 0.
    pub fn new(height: u16) -> Result<Self, InvalidLineHeight> {
        if height == 0 {
            Err(InvalidLineHeight(height))
        } else {
            Ok(Self(height))
        }
    }

    /// Get the height value.
    pub fn get(&self) -> u16 {
        self.0
    }

    /// Check if this is a zero-height (malformed/non-rendered) entry.
    pub fn is_zero(&self) -> bool {
        self.0 == 0
    }
}

impl Default for LineHeight {
    fn default() -> Self {
        Self::ONE
    }
}

/// Absolute line offset from the start of a conversation.
/// 0-indexed: line 0 is the first line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct LineOffset(usize);

impl LineOffset {
    pub fn new(offset: usize) -> Self {
        Self(offset)
    }

    pub fn get(&self) -> usize {
        self.0
    }

    pub fn saturating_add(&self, amount: usize) -> Self {
        Self(self.0.saturating_add(amount))
    }

    pub fn saturating_sub(&self, amount: usize) -> Self {
        Self(self.0.saturating_sub(amount))
    }
}

/// Index of an entry within its conversation.
/// This is the canonical reference for entries in view-state.
/// 0-indexed: entry 0 is the first entry in the conversation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct EntryIndex(usize);

impl EntryIndex {
    /// Create a new entry index.
    pub fn new(index: usize) -> Self {
        Self(index)
    }

    /// Get the index value.
    pub fn get(&self) -> usize {
        self.0
    }

    /// Display index (1-based, for user-facing display).
    pub fn display(&self) -> usize {
        self.0 + 1
    }

    /// Next entry index.
    pub fn next(&self) -> Self {
        Self(self.0 + 1)
    }

    /// Previous entry index, saturating at 0.
    pub fn prev(&self) -> Self {
        Self(self.0.saturating_sub(1))
    }
}

impl From<usize> for EntryIndex {
    fn from(index: usize) -> Self {
        Self(index)
    }
}

/// Viewport dimensions in terminal cells.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ViewportDimensions {
    pub width: u16,
    pub height: u16,
}

impl ViewportDimensions {
    pub fn new(width: u16, height: u16) -> Self {
        Self { width, height }
    }
}
```

---

## 2. Entry Layout

Layout information for a single entry, computed based on viewport width and expand state.

```rust
// ===== src/view_state/layout.rs =====

use super::types::{LineHeight, LineOffset};

/// Layout metadata for a single entry.
///
/// Computed from entry content + viewport width + expand state.
/// Stored alongside the entry in EntryView.
///
/// # Invariants
/// - `height >= 1` (enforced by LineHeight)
/// - `cumulative_y[i] = sum(height[0..i])` (maintained by ConversationViewState)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EntryLayout {
    /// Height of this entry in lines.
    height: LineHeight,
    /// Cumulative Y offset from start of conversation.
    /// Equal to sum of all preceding entry heights.
    cumulative_y: LineOffset,
}

impl EntryLayout {
    /// Create new layout. Called internally during layout computation.
    pub(crate) fn new(height: LineHeight, cumulative_y: LineOffset) -> Self {
        Self { height, cumulative_y }
    }

    /// Height in lines.
    pub fn height(&self) -> LineHeight {
        self.height
    }

    /// Cumulative Y offset (lines from start of conversation).
    pub fn cumulative_y(&self) -> LineOffset {
        self.cumulative_y
    }

    /// Y offset of the line immediately after this entry.
    /// Equal to cumulative_y + height.
    pub fn bottom_y(&self) -> LineOffset {
        LineOffset::new(self.cumulative_y.get() + self.height.get() as usize)
    }
}

impl Default for EntryLayout {
    fn default() -> Self {
        Self {
            height: LineHeight::default(),
            cumulative_y: LineOffset::default(),
        }
    }
}
```

---

## 2.5. Height Calculator Contract

The `height_calculator` function passed to `recompute_layout` and `relayout_from` must compute the rendered height of an entry in terminal lines.

```rust
/// Height calculator contract.
///
/// The height calculator receives:
/// - `entry`: The conversation entry to measure
/// - `expanded`: Whether the entry is currently expanded
/// - `wrap_mode`: The effective wrap mode for this entry
///
/// It MUST return the actual rendered height accounting for:
///
/// 1. **Text Wrapping**: Content wrapped at viewport width produces multiple lines
/// 2. **Markdown Rendering**: Headers, lists, code blocks affect line count
/// 3. **Syntax Highlighting**: May add decorations but should NOT affect line count
/// 4. **Collapsed State**: When `expanded = false`, return collapsed summary height
///    (typically 1-3 lines showing entry type + truncated preview)
/// 5. **Malformed Entries**: Return `LineHeight::ZERO` for entries that should not render
///
/// # Contract Requirements
///
/// - MUST return `LineHeight::ZERO` for malformed entries
/// - MUST return at least `LineHeight::ONE` for valid entries
/// - MUST account for viewport width (passed via LayoutParams.width)
/// - MUST be deterministic (same inputs → same output)
/// - SHOULD be fast (called for every entry during layout)
///
/// # Collapsed Entry Height
///
/// When `expanded = false`, the entry shows a summary:
/// ```text
/// [User] Request: First 50 chars of message... (+23 more lines)
/// ```
/// This is typically 1-3 lines depending on entry type.
///
/// # Example Implementation
///
/// ```rust
/// fn calculate_height(
///     entry: &ConversationEntry,
///     expanded: bool,
///     wrap_mode: WrapMode,
///     viewport_width: u16,
/// ) -> LineHeight {
///     match entry {
///         ConversationEntry::Malformed(_) => LineHeight::ZERO,
///         ConversationEntry::Valid(log_entry) => {
///             if !expanded {
///                 // Collapsed: type header + summary line
///                 LineHeight::new(2).unwrap()
///             } else {
///                 // Expanded: render markdown and count lines
///                 let rendered = render_markdown(log_entry.content(), viewport_width);
///                 let line_count = rendered.lines().count().max(1) as u16;
///                 LineHeight::new(line_count).unwrap()
///             }
///         }
///     }
/// }
/// ```
pub type HeightCalculator = fn(&ConversationEntry, bool, WrapMode) -> LineHeight;
```

---

## 3. Entry View

Combines owned domain entry with computed layout and per-entry presentation state.

```rust
// ===== src/view_state/entry_view.rs =====

use crate::model::ConversationEntry;
use crate::state::WrapMode;
use super::layout::EntryLayout;
use super::types::EntryIndex;

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
    /// Create new EntryView with default state.
    /// Layout will be computed on first render or explicit recompute.
    /// Starts collapsed with no wrap override.
    ///
    /// # Arguments
    /// - `entry`: The domain entry (owned)
    /// - `index`: Position of this entry within its conversation
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
    pub(crate) fn with_layout(entry: ConversationEntry, index: EntryIndex, layout: EntryLayout) -> Self {
        Self {
            entry,
            index,
            layout,
            expanded: false,
            wrap_override: None,
        }
    }

    // === Index Access ===

    /// Index of this entry within its conversation.
    pub fn index(&self) -> EntryIndex {
        self.index
    }

    /// Display index (1-based, for user-facing display).
    pub fn display_index(&self) -> usize {
        self.index.display()
    }

    // === Domain Entry Access ===

    /// Reference to the domain entry.
    pub fn entry(&self) -> &ConversationEntry {
        &self.entry
    }

    // === Layout Access ===

    /// Current layout (may be stale if viewport changed).
    pub fn layout(&self) -> &EntryLayout {
        &self.layout
    }

    /// Update layout. Called during recompute.
    pub(crate) fn set_layout(&mut self, layout: EntryLayout) {
        self.layout = layout;
    }

    // === Expand/Collapse State ===

    /// Whether this entry is expanded.
    pub fn is_expanded(&self) -> bool {
        self.expanded
    }

    /// Set expanded state.
    pub fn set_expanded(&mut self, expanded: bool) {
        self.expanded = expanded;
    }

    /// Toggle expanded state. Returns new state.
    pub fn toggle_expanded(&mut self) -> bool {
        self.expanded = !self.expanded;
        self.expanded
    }

    // === Wrap Override ===

    /// Per-entry wrap override, if set.
    pub fn wrap_override(&self) -> Option<WrapMode> {
        self.wrap_override
    }

    /// Set wrap override. Pass `None` to use global wrap mode.
    pub fn set_wrap_override(&mut self, mode: Option<WrapMode>) {
        self.wrap_override = mode;
    }

    /// Effective wrap mode for this entry.
    /// Returns per-entry override if set, otherwise uses global.
    pub fn effective_wrap(&self, global: WrapMode) -> WrapMode {
        self.wrap_override.unwrap_or(global)
    }
}
```

---

## 4. Scroll Position

Semantic sum type representing scroll location. Survives layout changes.

```rust
// ===== src/view_state/scroll.rs =====

use super::types::{EntryIndex, LineOffset};

/// Semantic scroll position within a conversation.
///
/// A sum type that preserves scroll intent across layout changes:
/// - `Top`: Always shows from line 0
/// - `Bottom`: Always shows last lines in viewport
/// - `AtLine`: Specific absolute line offset
/// - `AtEntry`: Keep specific entry visible (survives relayout)
/// - `Fraction`: Proportional position (for scrollbar)
///
/// # Resolution
/// All variants resolve to `LineOffset` via `resolve()` method.
/// The resolution uses current layout state for `AtEntry` and `Bottom`.
///
/// # Clamping Behavior
/// When a scroll position would resolve beyond document bounds,
/// it is clamped to the valid range `[0, max(0, total_height - viewport_height)]`.
/// This ensures no blank viewports regardless of the requested position.
///
/// # Cardinality Analysis
/// - Top: 1 state
/// - Bottom: 1 state
/// - AtLine: 2^64 states (usize)
/// - AtEntry: 2^64 * 2^64 states (entry_index, line_in_entry)
/// - Fraction: ~2^64 states (f64 in 0.0..=1.0)
/// Total: Effectively unbounded, but each variant is well-defined.
#[derive(Debug, Clone, PartialEq)]
pub enum ScrollPosition {
    /// View from the very top (line 0).
    Top,

    /// View from the very bottom.
    /// Resolves to: total_height - viewport_height (clamped to 0).
    Bottom,

    /// Specific line offset from top.
    /// Clamped to valid range on resolution.
    AtLine(LineOffset),

    /// Keep specific entry at top of viewport.
    /// Survives relayout: resolves using entry's cumulative_y.
    /// If entry_index is beyond document end, clamps to last entry.
    AtEntry {
        /// Index of entry in the conversation.
        entry_index: EntryIndex,
        /// Line offset within the entry (0 = top of entry).
        line_in_entry: usize,
    },

    /// Fractional position (0.0 = top, 1.0 = bottom).
    /// Used by scrollbar for proportional navigation.
    /// Clamped to [0.0, 1.0] on resolution.
    Fraction(f64),
}

impl Default for ScrollPosition {
    fn default() -> Self {
        Self::Top
    }
}

impl ScrollPosition {
    /// Resolve to absolute line offset.
    ///
    /// # Arguments
    /// - `total_height`: Total height of content in lines
    /// - `viewport_height`: Height of viewport in lines
    /// - `entry_lookup`: Function to get entry's cumulative_y by index
    ///
    /// # Returns
    /// Absolute line offset from top, clamped to valid range.
    /// Never returns an offset that would cause a blank viewport.
    pub fn resolve<F>(&self, total_height: usize, viewport_height: usize, entry_lookup: F) -> LineOffset
    where
        F: Fn(EntryIndex) -> Option<LineOffset>,
    {
        let max_offset = total_height.saturating_sub(viewport_height);

        let raw_offset = match self {
            ScrollPosition::Top => 0,
            ScrollPosition::Bottom => max_offset,
            ScrollPosition::AtLine(offset) => offset.get(),
            ScrollPosition::AtEntry { entry_index, line_in_entry } => {
                entry_lookup(*entry_index)
                    .map(|y| y.get() + line_in_entry)
                    .unwrap_or(0)
            }
            ScrollPosition::Fraction(f) => {
                let f = f.clamp(0.0, 1.0);
                (f * max_offset as f64).round() as usize
            }
        };

        // Clamp to valid range to ensure no blank viewports
        LineOffset::new(raw_offset.min(max_offset))
    }

    /// Create AtEntry position for given entry index.
    pub fn at_entry(entry_index: EntryIndex) -> Self {
        Self::AtEntry { entry_index, line_in_entry: 0 }
    }

    /// Create AtLine position.
    pub fn at_line(offset: usize) -> Self {
        Self::AtLine(LineOffset::new(offset))
    }
}
```

---

## 5. Visible Range

Result of visible range calculation.

```rust
// ===== src/view_state/visible_range.rs =====

use super::types::{EntryIndex, LineOffset};

/// Range of entries visible in the current viewport.
///
/// Computed via binary search on cumulative Y offsets.
/// Indices are into the conversation's entry list.
///
/// # Invariants
/// - `start_index <= end_index`
/// - `end_index <= entries.len()`
/// - All entries in range have some portion visible in viewport
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VisibleRange {
    /// Index of first visible entry (inclusive).
    pub start_index: EntryIndex,
    /// Index of last visible entry (exclusive).
    pub end_index: EntryIndex,
    /// Scroll offset (resolved from ScrollPosition).
    pub scroll_offset: LineOffset,
    /// Viewport height in lines.
    pub viewport_height: u16,
}

impl VisibleRange {
    /// Create new visible range.
    pub fn new(start_index: EntryIndex, end_index: EntryIndex, scroll_offset: LineOffset, viewport_height: u16) -> Self {
        debug_assert!(start_index <= end_index);
        Self {
            start_index,
            end_index,
            scroll_offset,
            viewport_height,
        }
    }

    /// Number of visible entries.
    pub fn len(&self) -> usize {
        self.end_index.get() - self.start_index.get()
    }

    /// Check if range is empty.
    pub fn is_empty(&self) -> bool {
        self.start_index == self.end_index
    }

    /// Iterate over visible entry indices.
    pub fn indices(&self) -> impl Iterator<Item = EntryIndex> {
        (self.start_index.get()..self.end_index.get()).map(EntryIndex::new)
    }

    /// Check if a specific entry index is visible.
    pub fn contains(&self, index: EntryIndex) -> bool {
        index >= self.start_index && index < self.end_index
    }
}

impl Default for VisibleRange {
    fn default() -> Self {
        Self {
            start_index: EntryIndex::default(),
            end_index: EntryIndex::default(),
            scroll_offset: LineOffset::default(),
            viewport_height: 0,
        }
    }
}
```

---

## 6. Hit Test Result

Result of mouse hit-testing.

```rust
// ===== src/view_state/hit_test.rs =====

use super::types::EntryIndex;

/// Result of hit-testing a screen coordinate.
///
/// Determines what entry (if any) was clicked and where.
/// Uses `EntryIndex` as the canonical reference for entries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HitTestResult {
    /// Click was outside any entry bounds.
    Miss,

    /// Click hit an entry.
    Hit {
        /// Index of the hit entry (canonical reference).
        entry_index: EntryIndex,
        /// Line within the entry that was hit (0-indexed).
        line_in_entry: usize,
        /// Column within the line (0-indexed).
        column: u16,
    },
}

impl HitTestResult {
    /// Create a miss result.
    pub fn miss() -> Self {
        Self::Miss
    }

    /// Create a hit result.
    pub fn hit(entry_index: EntryIndex, line_in_entry: usize, column: u16) -> Self {
        Self::Hit {
            entry_index,
            line_in_entry,
            column,
        }
    }

    /// Check if this was a hit.
    pub fn is_hit(&self) -> bool {
        matches!(self, Self::Hit { .. })
    }

    /// Get entry index if hit.
    pub fn entry_index(&self) -> Option<EntryIndex> {
        match self {
            Self::Hit { entry_index, .. } => Some(*entry_index),
            Self::Miss => None,
        }
    }
}
```

---

## 7. Layout Parameters

Parameters that affect layout computation. Used for invalidation tracking.

```rust
// ===== src/view_state/layout_params.rs =====

use crate::state::WrapMode;

/// Global parameters that affect entry layout.
///
/// Used for invalidation: if current params != last layout params,
/// full relayout may be needed.
///
/// Note: Per-entry state (expanded, wrap_override) is stored in EntryView,
/// not here. This struct only tracks global parameters.
///
/// # Equality Semantics
/// Two LayoutParams are equal if they would produce identical layouts
/// (assuming per-entry state unchanged).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LayoutParams {
    /// Viewport width in columns.
    pub width: u16,
    /// Global wrap mode.
    pub global_wrap: WrapMode,
}

impl LayoutParams {
    /// Create new layout params.
    pub fn new(width: u16, global_wrap: WrapMode) -> Self {
        Self { width, global_wrap }
    }
}
```

---

## 8. Conversation View State

View-state for a single conversation (main agent or subagent).

```rust
// ===== src/view_state/conversation.rs =====

use super::{
    entry_view::EntryView,
    hit_test::HitTestResult,
    layout::EntryLayout,
    layout_params::LayoutParams,
    scroll::ScrollPosition,
    types::{LineHeight, LineOffset, ViewportDimensions},
    visible_range::VisibleRange,
};
use crate::model::ConversationEntry;

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
}

impl ConversationViewState {
    /// Create new conversation view-state from entries.
    /// Layout is not computed until `recompute_layout` is called.
    pub fn new(entries: Vec<ConversationEntry>) -> Self {
        let entry_views: Vec<EntryView> = entries
            .into_iter()
            .enumerate()
            .map(|(idx, entry)| EntryView::new(entry, EntryIndex::new(idx)))
            .collect();
        Self {
            entries: entry_views,
            scroll: ScrollPosition::Top,
            total_height: 0,
            focused_message: None,
            last_layout_params: None,
        }
    }

    /// Create empty conversation view-state.
    pub fn empty() -> Self {
        Self::new(Vec::new())
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
        self.focused_message.and_then(|i| self.entries.get_mut(i.get()))
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

    /// Iterate over all entries.
    pub fn iter(&self) -> impl Iterator<Item = &EntryView> {
        self.entries.iter()
    }

    /// Current scroll position.
    pub fn scroll(&self) -> &ScrollPosition {
        &self.scroll
    }

    /// Set scroll position.
    pub fn set_scroll(&mut self, position: ScrollPosition) {
        self.scroll = position;
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
            self.entries.push(EntryView::new(entry, EntryIndex::new(start_idx + offset)));
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
    pub fn relayout_from<F>(&mut self, from_index: EntryIndex, params: LayoutParams, height_calculator: F)
    where
        F: Fn(&ConversationEntry, bool, WrapMode) -> LineHeight,
    {
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
    pub fn toggle_expand<F>(&mut self, index: EntryIndex, params: LayoutParams, height_calculator: F) -> Option<bool>
    where
        F: Fn(&ConversationEntry, bool, WrapMode) -> LineHeight,
    {
        let entry = self.entries.get_mut(index.get())?;
        let new_state = entry.toggle_expanded();
        self.relayout_from(index, params, height_calculator);
        Some(new_state)
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

        let scroll_offset = self.scroll.resolve(
            self.total_height,
            viewport.height as usize,
            |idx| self.entries.get(idx.get()).map(|e| e.layout().cumulative_y()),
        );

        let scroll_line = scroll_offset.get();
        let viewport_bottom = scroll_line + viewport.height as usize;

        // Binary search for first visible entry
        let start_index = self.entries.partition_point(|e| {
            e.layout().bottom_y().get() <= scroll_line
        });

        // Binary search for first entry past viewport
        let end_index = self.entries.partition_point(|e| {
            e.layout().cumulative_y().get() < viewport_bottom
        });

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
    pub fn hit_test(&self, screen_y: u16, screen_x: u16, scroll_offset: LineOffset) -> HitTestResult {
        if self.entries.is_empty() {
            return HitTestResult::miss();
        }

        let absolute_y = scroll_offset.get() + screen_y as usize;

        // Binary search for entry containing absolute_y
        let index = self.entries.partition_point(|e| {
            e.layout().bottom_y().get() <= absolute_y
        });

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
        self.entries.get(index.get()).map(|e| e.layout().cumulative_y())
    }
}
```

---

## 9. Session View State

View-state for a single session containing main and subagent conversations.

```rust
// ===== src/view_state/session.rs =====

use super::conversation::ConversationViewState;
use crate::model::{AgentId, ConversationEntry, SessionId};
use std::collections::HashMap;

/// View-state for a single session.
///
/// Contains:
/// - Main conversation view-state (always present)
/// - Subagent view-states (lazily created on first view, FR-073)
/// - Pending subagent entries (before view-state creation)
///
/// # Lazy Initialization (FR-073)
/// Subagent view-states are created lazily when first accessed.
/// Until accessed, entries are stored in `pending_subagent_entries`.
#[derive(Debug, Clone)]
pub struct SessionViewState {
    /// Session identifier.
    session_id: SessionId,
    /// Main conversation view-state.
    main: ConversationViewState,
    /// Subagent view-states (lazily initialized).
    subagents: HashMap<AgentId, ConversationViewState>,
    /// Pending subagent entries (before lazy init).
    pending_subagent_entries: HashMap<AgentId, Vec<ConversationEntry>>,
    /// Cumulative line offset from start of log (for multi-session).
    start_line: usize,
}

impl SessionViewState {
    /// Create new session view-state.
    pub fn new(session_id: SessionId) -> Self {
        Self {
            session_id,
            main: ConversationViewState::empty(),
            subagents: HashMap::new(),
            pending_subagent_entries: HashMap::new(),
            start_line: 0,
        }
    }

    /// Session identifier.
    pub fn session_id(&self) -> &SessionId {
        &self.session_id
    }

    /// Reference to main conversation view-state.
    pub fn main(&self) -> &ConversationViewState {
        &self.main
    }

    /// Mutable reference to main conversation.
    pub fn main_mut(&mut self) -> &mut ConversationViewState {
        &mut self.main
    }

    /// Get subagent view-state, creating lazily if needed.
    pub fn subagent(&mut self, id: &AgentId) -> &ConversationViewState {
        if !self.subagents.contains_key(id) {
            // Create from pending entries
            let entries = self.pending_subagent_entries.remove(id).unwrap_or_default();
            let view_state = ConversationViewState::new(entries);
            self.subagents.insert(id.clone(), view_state);
        }
        self.subagents.get(id).unwrap()
    }

    /// Mutable reference to subagent view-state.
    pub fn subagent_mut(&mut self, id: &AgentId) -> &mut ConversationViewState {
        if !self.subagents.contains_key(id) {
            let entries = self.pending_subagent_entries.remove(id).unwrap_or_default();
            let view_state = ConversationViewState::new(entries);
            self.subagents.insert(id.clone(), view_state);
        }
        self.subagents.get_mut(id).unwrap()
    }

    /// Check if subagent view-state exists (has been accessed).
    pub fn has_subagent(&self, id: &AgentId) -> bool {
        self.subagents.contains_key(id)
    }

    /// List all known subagent IDs (initialized or pending).
    pub fn subagent_ids(&self) -> impl Iterator<Item = &AgentId> {
        self.subagents.keys().chain(self.pending_subagent_entries.keys())
    }

    /// Add entry to main conversation.
    pub fn add_main_entry(&mut self, entry: ConversationEntry) {
        self.main.append(vec![entry]);
    }

    /// Add entry to subagent conversation.
    /// If view-state exists, appends directly. Otherwise, stores as pending.
    pub fn add_subagent_entry(&mut self, agent_id: AgentId, entry: ConversationEntry) {
        if let Some(view_state) = self.subagents.get_mut(&agent_id) {
            view_state.append(vec![entry]);
        } else {
            self.pending_subagent_entries
                .entry(agent_id)
                .or_default()
                .push(entry);
        }
    }

    /// Start line offset (for multi-session positioning).
    pub fn start_line(&self) -> usize {
        self.start_line
    }

    /// Set start line offset.
    pub(crate) fn set_start_line(&mut self, offset: usize) {
        self.start_line = offset;
    }

    /// Height of main conversation only.
    pub fn main_height(&self) -> usize {
        self.main.total_height()
    }

    /// Total height of all conversations in this session.
    /// In continuous scroll display mode, this is the height contribution
    /// of this entire session to the log view.
    ///
    /// Includes:
    /// - Main conversation height
    /// - All initialized subagent conversation heights
    /// - Pending subagent entries (estimated at 1 line each until initialized)
    pub fn total_height(&self) -> usize {
        let main_h = self.main.total_height();
        let subagent_h: usize = self.subagents.values().map(|s| s.total_height()).sum();
        let pending_h: usize = self.pending_subagent_entries.values().map(|v| v.len()).sum();
        main_h + subagent_h + pending_h
    }
}
```

---

## 10. Log View State

Top-level view-state containing all sessions.

```rust
// ===== src/view_state/log.rs =====

use super::session::SessionViewState;
use crate::model::{AgentId, ConversationEntry, SessionId};

/// Top-level view-state for an entire log file.
///
/// Contains ordered sessions, supports:
/// - Multi-session logs (FR-070)
/// - Session boundary detection (FR-078)
/// - Active session determination (FR-080)
///
/// # Display Mode Independence (FR-076, FR-077)
/// LogViewState stores sessions in order. Display mode (continuous,
/// one-at-a-time, collapsible) is determined by view layer.
#[derive(Debug, Clone)]
pub struct LogViewState {
    /// Ordered sessions.
    sessions: Vec<SessionViewState>,
    /// Current session ID (for streaming detection).
    current_session_id: Option<SessionId>,
}

impl LogViewState {
    /// Create empty log view-state.
    pub fn new() -> Self {
        Self {
            sessions: Vec::new(),
            current_session_id: None,
        }
    }

    /// Number of sessions.
    pub fn session_count(&self) -> usize {
        self.sessions.len()
    }

    /// Check if empty.
    pub fn is_empty(&self) -> bool {
        self.sessions.is_empty()
    }

    /// Get session by index.
    pub fn get_session(&self, index: usize) -> Option<&SessionViewState> {
        self.sessions.get(index)
    }

    /// Get mutable session by index.
    pub fn get_session_mut(&mut self, index: usize) -> Option<&mut SessionViewState> {
        self.sessions.get_mut(index)
    }

    /// Iterate over sessions.
    pub fn sessions(&self) -> impl Iterator<Item = &SessionViewState> {
        self.sessions.iter()
    }

    /// Find active session containing scroll position (FR-080).
    /// Uses session start_line to determine which session is visible.
    pub fn active_session(&self, scroll_line: usize) -> Option<&SessionViewState> {
        self.sessions
            .iter()
            .rfind(|s| s.start_line() <= scroll_line)
    }

    /// Active session index.
    pub fn active_session_index(&self, scroll_line: usize) -> Option<usize> {
        self.sessions
            .iter()
            .rposition(|s| s.start_line() <= scroll_line)
    }

    /// Add entry, routing to correct session/conversation.
    /// Creates new session if session_id changes (FR-078).
    pub fn add_entry(&mut self, entry: ConversationEntry, agent_id: Option<AgentId>) {
        let session_id = entry.session_id().cloned();

        // Detect session boundary
        if session_id != self.current_session_id {
            if let Some(new_id) = session_id.clone() {
                // Calculate start line for new session.
                // In continuous scroll mode, sessions are concatenated, so start_line
                // must account for all content from all previous sessions.
                let start_line = self.sessions.iter().map(|s| s.total_height()).sum();
                let mut new_session = SessionViewState::new(new_id);
                new_session.set_start_line(start_line);
                self.sessions.push(new_session);
                self.current_session_id = session_id;
            }
        }

        // Add to current session
        if let Some(session) = self.sessions.last_mut() {
            match agent_id {
                None => session.add_main_entry(entry),
                Some(id) => session.add_subagent_entry(id, entry),
            }
        }
    }

    /// Get current session (last one).
    pub fn current_session(&self) -> Option<&SessionViewState> {
        self.sessions.last()
    }

    /// Get mutable current session.
    pub fn current_session_mut(&mut self) -> Option<&mut SessionViewState> {
        self.sessions.last_mut()
    }
}

impl Default for LogViewState {
    fn default() -> Self {
        Self::new()
    }
}
```

---

## 11. Cached Render

Cache entry for rendered output.

```rust
// ===== src/view_state/cache.rs =====

use crate::model::EntryUuid;
use crate::state::WrapMode;
use lru::LruCache;
use ratatui::text::Line;
use std::num::NonZeroUsize;

/// Key for render cache lookup.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RenderCacheKey {
    /// Entry UUID.
    pub uuid: EntryUuid,
    /// Viewport width when rendered.
    pub width: u16,
    /// Whether entry was expanded.
    pub expanded: bool,
    /// Wrap mode when rendered.
    pub wrap_mode: WrapMode,
}

impl RenderCacheKey {
    pub fn new(uuid: EntryUuid, width: u16, expanded: bool, wrap_mode: WrapMode) -> Self {
        Self { uuid, width, expanded, wrap_mode }
    }
}

/// Cached rendered lines for an entry.
#[derive(Debug, Clone)]
pub struct CachedRender {
    /// Rendered lines.
    pub lines: Vec<Line<'static>>,
}

/// Configuration for render cache (FR-054).
/// Loaded from config file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(default)]
pub struct RenderCacheConfig {
    /// Maximum number of cached entries.
    /// Default: 1000
    pub capacity: usize,
}

impl Default for RenderCacheConfig {
    fn default() -> Self {
        Self { capacity: 1000 }
    }
}

/// LRU cache for rendered entry output.
///
/// Bounded capacity with LRU eviction (FR-052).
/// Cache key includes all parameters that affect rendering.
/// Capacity configurable via config file (FR-054).
pub struct RenderCache {
    cache: LruCache<RenderCacheKey, CachedRender>,
}

impl RenderCache {
    /// Create new cache with given capacity.
    pub fn new(capacity: usize) -> Self {
        let capacity = NonZeroUsize::new(capacity).unwrap_or(NonZeroUsize::new(1000).unwrap());
        Self {
            cache: LruCache::new(capacity),
        }
    }

    /// Create from config.
    pub fn from_config(config: &RenderCacheConfig) -> Self {
        Self::new(config.capacity)
    }

    /// Get cached render if present.
    pub fn get(&mut self, key: &RenderCacheKey) -> Option<&CachedRender> {
        self.cache.get(key)
    }

    /// Insert rendered output into cache.
    pub fn put(&mut self, key: RenderCacheKey, render: CachedRender) {
        self.cache.put(key, render);
    }

    /// Clear the cache.
    pub fn clear(&mut self) {
        self.cache.clear();
    }

    /// Number of cached entries.
    pub fn len(&self) -> usize {
        self.cache.len()
    }

    /// Check if cache is empty.
    pub fn is_empty(&self) -> bool {
        self.cache.is_empty()
    }
}

impl Default for RenderCache {
    fn default() -> Self {
        Self::new(1000)
    }
}
```

---

## Type Hierarchy Diagram

```
LogViewState
├── sessions: Vec<SessionViewState>
│   ├── session_id: SessionId
│   ├── main: ConversationViewState
│   │   ├── entries: Vec<EntryView>
│   │   │   ├── entry: ConversationEntry (OWNED)
│   │   │   │   ├── Valid(LogEntry)
│   │   │   │   │   ├── uuid: EntryUuid
│   │   │   │   │   ├── session_id: SessionId
│   │   │   │   │   ├── agent_id: Option<AgentId>
│   │   │   │   │   └── message: Message
│   │   │   │   └── Malformed(MalformedEntry) → height=0, not rendered
│   │   │   ├── index: EntryIndex               # Canonical reference (0-based newtype)
│   │   │   ├── layout: EntryLayout
│   │   │   │   ├── height: LineHeight (>= 1, or ZERO for malformed)
│   │   │   │   └── cumulative_y: LineOffset
│   │   │   ├── expanded: bool                  # Per-entry expand state
│   │   │   └── wrap_override: Option<WrapMode> # Per-entry wrap override
│   │   ├── scroll: ScrollPosition
│   │   │   ├── Top
│   │   │   ├── Bottom
│   │   │   ├── AtLine(LineOffset)              # Clamped to valid range
│   │   │   ├── AtEntry { entry_index: EntryIndex, line_in_entry }
│   │   │   └── Fraction(f64)                   # Clamped to [0.0, 1.0]
│   │   ├── total_height: usize
│   │   ├── focused_message: Option<EntryIndex> # Focused entry
│   │   └── last_layout_params: Option<LayoutParams>
│   ├── subagents: HashMap<AgentId, ConversationViewState>
│   ├── pending_subagent_entries: HashMap<AgentId, Vec<ConversationEntry>>
│   └── start_line: usize                       # Accounts for ALL session content
└── current_session_id: Option<SessionId>

RenderCache (configurable via RenderCacheConfig)
├── cache: LruCache<RenderCacheKey, CachedRender>
│   ├── Key: { uuid, width, expanded, wrap_mode }
│   └── Value: { lines: Vec<Line<'static>> }
└── capacity: NonZeroUsize (default: 1000, from config)

Supporting Types:
├── EntryIndex (usize newtype, canonical entry reference)
├── LineHeight (u16, >= 1 for valid entries, ZERO for malformed)
├── InvalidLineHeight (error type for LineHeight::new)
├── LineOffset (usize, 0-indexed)
├── ViewportDimensions { width, height }
├── VisibleRange { start_index: EntryIndex, end_index: EntryIndex, scroll_offset, viewport_height }
├── HitTestResult { Miss | Hit { entry_index: EntryIndex, line_in_entry, column } }
├── LayoutParams { width, global_wrap }  # Global params only; per-entry state in EntryView
└── RenderCacheConfig { capacity } (serde, from config file)
```

---

## Property-Based Testing Invariants

```rust
// Properties to test with proptest:

// 1. LineHeight is always >= 1
// forall h: LineHeight. h.get() >= 1

// 2. Cumulative Y is monotonically increasing
// forall i < j: entries[i].cumulative_y <= entries[j].cumulative_y

// 3. Cumulative Y is sum of preceding heights
// forall i: entries[i].cumulative_y == sum(entries[0..i].map(|e| e.height))

// 4. Total height equals sum of all heights
// total_height == entries.iter().map(|e| e.layout.height.get()).sum()

// 5. Scroll position resolution is bounded
// forall scroll: scroll.resolve(total, viewport, lookup) <= max(0, total - viewport)

// 6. Visible range is within bounds
// visible_range.start_index <= visible_range.end_index <= entries.len()

// 7. Hit test index is valid when hit
// match hit_test(...) { Hit { entry_index, .. } => entry_index < entries.len() }

// 8. Lazy subagent initialization
// after subagent(id), has_subagent(id) == true

// 9. Session boundaries preserve order
// forall i < j: sessions[i].start_line <= sessions[j].start_line

// 10. RenderCache key equality
// key1 == key2 iff (uuid1, w1, exp1, wrap1) == (uuid2, w2, exp2, wrap2)

// 11. Effective wrap mode semantics
// entry.effective_wrap(global) == entry.wrap_override.unwrap_or(global)

// 12. Focused message is valid index
// focused_message.is_some() => focused_message.unwrap() < entries.len()

// 13. Toggle expand is idempotent pair
// let orig = entry.is_expanded();
// entry.toggle_expanded();
// entry.toggle_expanded();
// assert_eq!(entry.is_expanded(), orig);

// 14. Relayout from preserves cumulative_y invariant
// after relayout_from(i, ..):
// forall j >= i: entries[j].cumulative_y == entries[j-1].bottom_y() (or 0 if j==0)
```

---

## Cardinality Analysis

| Type | Valid States | Total Cardinality | Precision |
|------|--------------|-------------------|-----------|
| `LineHeight` | 1..=65535 | 65535 | 1.0 |
| `LineOffset` | 0..=2^64-1 | 2^64 | 1.0 |
| `ScrollPosition::Top` | 1 | 1 | 1.0 |
| `ScrollPosition::Bottom` | 1 | 1 | 1.0 |
| `ScrollPosition` (sum) | Finite + infinite | Well-typed | ~1.0 |
| `HitTestResult` | Miss + valid hits | Well-typed | ~1.0 |
| `EntryLayout` | h×y where h>=1 | (65535) × (2^64) | ~1.0 |
| `VisibleRange` | start≤end | ~0.5 of (n×n) | 0.5 (enforced at construction) |
| `EntryView.expanded` | true \| false | 2 | 1.0 |
| `EntryView.wrap_override` | None \| Some(Wrap) \| Some(NoWrap) | 3 | 1.0 |
| `LayoutParams` | width × global_wrap | 65536 × 2 | 1.0 |

**Key precision improvements from per-entry state**:
- `expanded: bool` directly on entry vs `HashSet<EntryUuid>` in params
  - Old: O(n) lookup set with n entries → unbounded, hard to reason about
  - New: O(1) field access, exactly 2 states per entry
- `wrap_override: Option<WrapMode>` with explicit override semantics
  - Old: HashSet + "invert global" → confusing indirection
  - New: None (use global) | Some(explicit) → clear 3-state per entry

**Note**: `VisibleRange` has `debug_assert!(start <= end)` to catch invalid construction during development. In release, this is a logical invariant maintained by the binary search algorithm.
