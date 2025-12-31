# Internal API Contract: View-State Layer

**Date**: 2025-12-27
**Status**: Design Complete
**Related**: [data-model.md](../data-model.md)

This document defines the internal Rust API contracts for the view-state layer module.

---

## Module Structure

```
src/view_state/
├── mod.rs           # Public exports
├── types.rs         # LineHeight, LineOffset, ViewportDimensions
├── layout.rs        # EntryLayout
├── entry_view.rs    # EntryView
├── scroll.rs        # ScrollPosition
├── visible_range.rs # VisibleRange
├── hit_test.rs      # HitTestResult
├── layout_params.rs # LayoutParams
├── conversation.rs  # ConversationViewState
├── session.rs       # SessionViewState
├── log.rs           # LogViewState
└── cache.rs         # RenderCache, RenderCacheKey, CachedRender
```

---

## Public Exports (mod.rs)

```rust
//! View-state layer for presentation concerns.
//!
//! Separates domain data (LogEntry, Message) from presentation
//! metadata (layout, scroll, cache).

pub mod types;
pub mod layout;
pub mod entry_view;
pub mod scroll;
pub mod visible_range;
pub mod hit_test;
pub mod layout_params;
pub mod conversation;
pub mod session;
pub mod log;
pub mod cache;

// Re-exports for convenience
pub use types::{EntryIndex, LineHeight, InvalidLineHeight, LineOffset, ViewportDimensions};
pub use layout::EntryLayout;
pub use entry_view::EntryView;
pub use scroll::ScrollPosition;
pub use visible_range::VisibleRange;
pub use hit_test::HitTestResult;
pub use layout_params::LayoutParams;
pub use conversation::ConversationViewState;
pub use session::SessionViewState;
pub use log::LogViewState;
pub use cache::{RenderCache, RenderCacheKey, CachedRender, RenderCacheConfig};
```

---

## Core APIs

### 1. ConversationViewState

The primary API for working with a single conversation's view-state.

```rust
impl ConversationViewState {
    // === Construction ===

    /// Create from owned entries.
    /// Each entry is assigned its EntryIndex automatically.
    pub fn new(entries: Vec<ConversationEntry>) -> Self;

    /// Create empty.
    pub fn empty() -> Self;

    // === Queries ===

    /// Number of entries.
    pub fn len(&self) -> usize;

    /// Check if empty.
    pub fn is_empty(&self) -> bool;

    /// Get entry by index.
    pub fn get(&self, index: EntryIndex) -> Option<&EntryView>;

    /// Iterate entries.
    pub fn iter(&self) -> impl Iterator<Item = &EntryView>;

    /// Total height in lines (after layout).
    pub fn total_height(&self) -> usize;

    /// Current scroll position.
    pub fn scroll(&self) -> &ScrollPosition;

    // === Focus Management ===

    /// Get focused entry index.
    pub fn focused_message(&self) -> Option<EntryIndex>;

    /// Set focused entry index (clamped to valid range).
    pub fn set_focused_message(&mut self, index: Option<EntryIndex>);

    /// Get focused entry view, if any.
    pub fn focused_entry(&self) -> Option<&EntryView>;

    /// Get mutable focused entry view, if any.
    pub fn focused_entry_mut(&mut self) -> Option<&mut EntryView>;

    // === Layout ===

    /// Check if global layout params changed.
    /// Per-entry state changes require targeted relayout.
    pub fn needs_relayout(&self, params: &LayoutParams) -> bool;

    /// Recompute layout for all entries.
    /// Height calculator must return LineHeight::ZERO for malformed entries.
    ///
    /// # Complexity: O(n)
    pub fn recompute_layout<F>(&mut self, params: LayoutParams, height_calculator: F)
    where
        F: Fn(&ConversationEntry, bool /* expanded */, WrapMode) -> LineHeight;

    /// Relayout from specific entry onward.
    /// Used after toggling expand/wrap on a single entry.
    ///
    /// # Complexity: O(n - from_index)
    pub fn relayout_from<F>(&mut self, from_index: EntryIndex, params: LayoutParams, height_calculator: F)
    where
        F: Fn(&ConversationEntry, bool, WrapMode) -> LineHeight;

    // === Per-Entry State Mutations ===

    /// Toggle expand state for entry at index and relayout.
    /// Returns new expanded state.
    pub fn toggle_expand<F>(&mut self, index: EntryIndex, params: LayoutParams, height_calculator: F) -> Option<bool>
    where
        F: Fn(&ConversationEntry, bool, WrapMode) -> LineHeight;

    /// Set wrap override for entry at index and relayout.
    /// Returns previous wrap override.
    pub fn set_wrap_override<F>(
        &mut self,
        index: EntryIndex,
        wrap: Option<WrapMode>,
        params: LayoutParams,
        height_calculator: F,
    ) -> Option<Option<WrapMode>>
    where
        F: Fn(&ConversationEntry, bool, WrapMode) -> LineHeight;

    // === Scrolling ===

    /// Set scroll position.
    /// All positions are clamped to valid range on resolution.
    pub fn set_scroll(&mut self, position: ScrollPosition);

    /// Compute visible range.
    ///
    /// # Complexity: O(log n)
    pub fn visible_range(&self, viewport: ViewportDimensions) -> VisibleRange;

    // === Hit Testing ===

    /// Hit-test screen coordinate.
    /// Returns EntryIndex for hit entries.
    ///
    /// # Complexity: O(log n)
    pub fn hit_test(
        &self,
        screen_y: u16,
        screen_x: u16,
        scroll_offset: LineOffset,
    ) -> HitTestResult;

    // === Streaming ===

    /// Append new entries (invalidates layout).
    /// New entries are assigned sequential EntryIndex values.
    pub fn append(&mut self, entries: Vec<ConversationEntry>);
}
```

### 2. SessionViewState

API for session-level view-state with lazy subagent initialization.

```rust
impl SessionViewState {
    // === Construction ===

    /// Create new session.
    pub fn new(session_id: SessionId) -> Self;

    // === Queries ===

    /// Session identifier.
    pub fn session_id(&self) -> &SessionId;

    /// Main conversation (always present).
    pub fn main(&self) -> &ConversationViewState;
    pub fn main_mut(&mut self) -> &mut ConversationViewState;

    /// Subagent conversation (lazily created).
    pub fn subagent(&mut self, id: &AgentId) -> &ConversationViewState;
    pub fn subagent_mut(&mut self, id: &AgentId) -> &mut ConversationViewState;

    /// Check if subagent has been accessed.
    pub fn has_subagent(&self, id: &AgentId) -> bool;

    /// List all known subagent IDs.
    pub fn subagent_ids(&self) -> impl Iterator<Item = &AgentId>;

    /// Start line offset (for multi-session).
    pub fn start_line(&self) -> usize;

    /// Height of main conversation only.
    pub fn main_height(&self) -> usize;

    /// Total height of all conversations in this session.
    /// Includes main + all subagents + pending entries.
    pub fn total_height(&self) -> usize;

    // === Entry Addition ===

    /// Add entry to main conversation.
    pub fn add_main_entry(&mut self, entry: ConversationEntry);

    /// Add entry to subagent (lazy init or pending).
    pub fn add_subagent_entry(&mut self, agent_id: AgentId, entry: ConversationEntry);
}
```

### 3. LogViewState

Top-level API for multi-session log view-state.

```rust
impl LogViewState {
    // === Construction ===

    /// Create empty log.
    pub fn new() -> Self;

    // === Queries ===

    /// Number of sessions.
    pub fn session_count(&self) -> usize;

    /// Check if empty.
    pub fn is_empty(&self) -> bool;

    /// Get session by index.
    pub fn get_session(&self, index: usize) -> Option<&SessionViewState>;
    pub fn get_session_mut(&mut self, index: usize) -> Option<&mut SessionViewState>;

    /// Iterate sessions.
    pub fn sessions(&self) -> impl Iterator<Item = &SessionViewState>;

    /// Find active session by scroll position.
    pub fn active_session(&self, scroll_line: usize) -> Option<&SessionViewState>;

    /// Active session index.
    pub fn active_session_index(&self, scroll_line: usize) -> Option<usize>;

    /// Current (last) session.
    pub fn current_session(&self) -> Option<&SessionViewState>;
    pub fn current_session_mut(&mut self) -> Option<&mut SessionViewState>;

    // === Entry Addition ===

    /// Add entry, routing to correct session/conversation.
    /// Creates new session on session_id change.
    pub fn add_entry(&mut self, entry: ConversationEntry, agent_id: Option<AgentId>);
}
```

### 4. RenderCache

API for bounded render caching.

```rust
impl RenderCache {
    // === Construction ===

    /// Create with explicit capacity.
    pub fn new(capacity: usize) -> Self;

    /// Create from configuration (FR-054).
    pub fn from_config(config: &RenderCacheConfig) -> Self;

    // === Operations ===

    /// Get cached render (updates LRU order).
    pub fn get(&mut self, key: &RenderCacheKey) -> Option<&CachedRender>;

    /// Insert render into cache.
    pub fn put(&mut self, key: RenderCacheKey, render: CachedRender);

    /// Clear cache.
    pub fn clear(&mut self);

    /// Current cache size.
    pub fn len(&self) -> usize;

    /// Check if empty.
    pub fn is_empty(&self) -> bool;
}
```

---

## Usage Patterns

### Pattern 1: Initial Load

```rust
// 1. Parse entries from JSONL
let entries: Vec<ConversationEntry> = parse_jsonl(input);

// 2. Create log view-state
let mut log = LogViewState::new();
for entry in entries {
    let agent_id = entry.as_valid().and_then(|e| e.agent_id().cloned());
    log.add_entry(entry, agent_id);
}

// 3. Layout on first render
let session = log.current_session_mut().unwrap();
let params = LayoutParams::new(viewport.width, expanded.clone(), wrap_mode);
session.main_mut().recompute_layout(params, |entry, params| {
    compute_entry_height(entry, params)
});
```

### Pattern 2: Visible Range Query

```rust
// O(log n) visible range calculation
let viewport = ViewportDimensions::new(80, 24);
let range = conversation.visible_range(viewport);

// Render only visible entries
for idx in range.indices() {
    let entry_view = conversation.get(idx).unwrap();
    render_entry(frame, entry_view, range.scroll_offset);
}
```

### Pattern 3: Hit Testing

```rust
// O(log n) hit test
let scroll_offset = conversation.scroll().resolve(
    conversation.total_height(),
    viewport.height as usize,
    |idx| conversation.entry_cumulative_y(idx),
);

match conversation.hit_test(mouse_y, mouse_x, scroll_offset) {
    HitTestResult::Hit { entry_index, entry_uuid, .. } => {
        // Handle click on entry
        toggle_expand(entry_uuid);
    }
    HitTestResult::Miss => {
        // Click outside entries
    }
}
```

### Pattern 4: Streaming Append

```rust
// New entries from stdin
let new_entries = source.poll()?;

for entry in new_entries {
    let agent_id = entry.as_valid().and_then(|e| e.agent_id().cloned());
    log.add_entry(entry, agent_id);
}

// Layout will be recomputed on next render (needs_relayout returns true)
```

### Pattern 5: Render Caching

```rust
let mut cache = RenderCache::new(1000);

for idx in visible_range.indices() {
    let entry_view = conversation.get(idx).unwrap();
    let uuid = entry_view.entry().uuid();

    if let Some(uuid) = uuid {
        let key = RenderCacheKey::new(
            uuid.clone(),
            viewport.width,
            params.is_expanded(uuid),
            wrap_mode,
        );

        let lines = if let Some(cached) = cache.get(&key) {
            cached.lines.clone()
        } else {
            let rendered = render_entry(entry_view.entry(), &params);
            cache.put(key, CachedRender { lines: rendered.clone() });
            rendered
        };

        draw_lines(frame, &lines);
    }
}
```

---

## Complexity Guarantees

| Operation | Complexity | Notes |
|-----------|------------|-------|
| `ConversationViewState::visible_range` | O(log n) | Binary search on cumulative_y |
| `ConversationViewState::hit_test` | O(log n) | Binary search on cumulative_y |
| `ConversationViewState::recompute_layout` | O(n) | Full scan required |
| `ConversationViewState::append` | O(k) | k = new entries |
| `LogViewState::active_session` | O(s) | s = sessions (typically <10) |
| `RenderCache::get` | O(1) | HashMap lookup |
| `RenderCache::put` | O(1) | HashMap insert, LRU eviction |

---

## Error Handling

The view-state layer uses **total functions** following constitution principle IV:

- No panics in public APIs (except debug_assert for invariant violations)
- `Option` for potentially missing data (entry at index, session lookup)
- All scroll positions resolve to valid offsets (clamped)
- Empty conversations return empty visible ranges

---

## Thread Safety

View-state types are **not thread-safe** by default:

- `LogViewState`, `SessionViewState`, `ConversationViewState` are `!Sync`
- Single-threaded TUI event loop model assumed
- If parallelization needed (layout computation), use `rayon` internally
- `RenderCache` wraps `LruCache` which is `!Sync`

For future multi-threaded rendering, wrap in `Arc<Mutex<_>>` or use concurrent LRU.
