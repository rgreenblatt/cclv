# Quickstart: View-State Layer Implementation

**Date**: 2025-12-27
**Related**: [plan.md](./plan.md) | [data-model.md](./data-model.md) | [contracts/](./contracts/)

This guide provides a fast path to implementing the view-state layer.

---

## Implementation Order

Follow this order to minimize back-and-forth:

### Phase 1: Core Types (1 day)

```
1. src/view_state/types.rs      - LineHeight, LineOffset, ViewportDimensions
2. src/view_state/layout.rs     - EntryLayout
3. src/view_state/entry_view.rs - EntryView (owns ConversationEntry)
4. src/view_state/mod.rs        - Initial exports
```

**Test checkpoint**: Unit tests for LineHeight invariant (>= 1).

### Phase 2: Scroll & Range (1 day)

```
5. src/view_state/scroll.rs        - ScrollPosition sum type
6. src/view_state/visible_range.rs - VisibleRange struct
7. src/view_state/hit_test.rs      - HitTestResult enum
8. src/view_state/layout_params.rs - LayoutParams for invalidation
```

**Test checkpoint**: ScrollPosition::resolve tests, edge cases.

### Phase 3: Conversation View State (2 days)

```
9.  src/view_state/conversation.rs - ConversationViewState
10. Add proptest for cumulative_y invariant
11. Add proptest for visible_range bounds
```

**Test checkpoint**: Binary search correctness, O(log n) verified.

### Phase 4: Session & Log (1 day)

```
12. src/view_state/session.rs - SessionViewState (lazy subagents)
13. src/view_state/log.rs     - LogViewState (multi-session)
```

**Test checkpoint**: Session boundary detection, lazy init.

### Phase 5: Caching (0.5 day)

```
14. src/view_state/cache.rs - RenderCache with lru crate
```

**Test checkpoint**: LRU eviction, cache hit/miss.

### Phase 6: Integration (1 day)

```
15. Update src/state/app_state.rs to use LogViewState
16. Update src/view/message.rs to consume view-state layout
17. Remove dead code from old scroll handling
```

**Test checkpoint**: Full integration test, no blank viewports.

---

## Key Code Snippets

### LineHeight (types.rs)

```rust
/// Height of an entry in lines. Always >= 1 for valid entries.
/// Use `LineHeight::ZERO` sentinel for malformed entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct LineHeight(u16);

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("LineHeight must be >= 1 for valid entries (got {0})")]
pub struct InvalidLineHeight(pub u16);

impl LineHeight {
    /// Sentinel for malformed/non-rendered entries.
    pub const ZERO: Self = Self(0);
    /// Minimum valid height.
    pub const ONE: Self = Self(1);

    /// Create LineHeight. Returns error if height is 0.
    pub fn new(height: u16) -> Result<Self, InvalidLineHeight> {
        if height == 0 {
            Err(InvalidLineHeight(height))
        } else {
            Ok(Self(height))
        }
    }

    pub fn get(&self) -> u16 { self.0 }
    pub fn is_zero(&self) -> bool { self.0 == 0 }
}

impl Default for LineHeight {
    fn default() -> Self { Self::ONE }
}
```

> **Note**: This follows Constitution Principle IV (Total Functions). No panics -
> `LineHeight::ZERO` handles malformed entries, `Result` handles invalid construction.

### EntryView with Per-Entry State (entry_view.rs)

```rust
pub struct EntryView {
    entry: ConversationEntry,
    index: usize,                      // Position in conversation (0-based)
    layout: EntryLayout,
    expanded: bool,                    // Per-entry expand state
    wrap_override: Option<WrapMode>,   // Per-entry wrap override (None = use global)
}

impl EntryView {
    pub fn new(entry: ConversationEntry, index: usize) -> Self {
        Self {
            entry,
            index,
            layout: EntryLayout::default(),
            expanded: false,
            wrap_override: None,
        }
    }

    /// Effective wrap mode: per-entry override or fall back to global.
    pub fn effective_wrap(&self, global: WrapMode) -> WrapMode {
        self.wrap_override.unwrap_or(global)
    }

    /// 1-based index for display.
    pub fn display_index(&self) -> usize {
        self.index + 1
    }
}
```

### Binary Search Visible Range (conversation.rs)

```rust
pub fn visible_range(&self, viewport: ViewportDimensions) -> VisibleRange {
    if self.entries.is_empty() {
        return VisibleRange::default();
    }

    let scroll_offset = self.scroll.resolve(
        self.total_height,
        viewport.height as usize,
        |idx| self.entries.get(idx).map(|e| e.layout().cumulative_y()),
    );

    let scroll_line = scroll_offset.get();
    let viewport_bottom = scroll_line + viewport.height as usize;

    // Binary search: first entry whose bottom > scroll_line
    let start_index = self.entries.partition_point(|e| {
        e.layout().bottom_y().get() <= scroll_line
    });

    // Binary search: first entry whose top >= viewport_bottom
    let end_index = self.entries.partition_point(|e| {
        e.layout().cumulative_y().get() < viewport_bottom
    });

    VisibleRange::new(start_index, end_index, scroll_offset, viewport.height)
}
```

### Lazy Subagent Init (session.rs)

```rust
pub fn subagent(&mut self, id: &AgentId) -> &ConversationViewState {
    if !self.subagents.contains_key(id) {
        let entries = self.pending_subagent_entries.remove(id).unwrap_or_default();
        let view_state = ConversationViewState::new(entries);
        self.subagents.insert(id.clone(), view_state);
    }
    self.subagents.get(id).unwrap()
}
```

### Session Boundary Detection (log.rs)

```rust
pub fn add_entry(&mut self, entry: ConversationEntry, agent_id: Option<AgentId>) {
    let session_id = entry.session_id().cloned();

    if session_id != self.current_session_id {
        if let Some(new_id) = session_id.clone() {
            let start_line = self.sessions.iter().map(|s| s.main_height()).sum();
            let mut new_session = SessionViewState::new(new_id);
            new_session.set_start_line(start_line);
            self.sessions.push(new_session);
            self.current_session_id = session_id;
        }
    }

    if let Some(session) = self.sessions.last_mut() {
        match agent_id {
            None => session.add_main_entry(entry),
            Some(id) => session.add_subagent_entry(id, entry),
        }
    }
}
```

---

## Cargo.toml Addition

```toml
[dependencies]
lru = "0.12"
```

---

## Property Tests to Add

```rust
// tests/view_state/proptest_invariants.rs

use proptest::prelude::*;

proptest! {
    #[test]
    fn cumulative_y_is_sum_of_heights(heights in prop::collection::vec(1u16..1000, 1..100)) {
        let entries = create_entries_with_heights(&heights);
        let conv = ConversationViewState::from_entries(entries);

        let mut expected_y = 0usize;
        for (i, entry) in conv.iter().enumerate() {
            prop_assert_eq!(entry.layout().cumulative_y().get(), expected_y);
            expected_y += heights[i] as usize;
        }
        prop_assert_eq!(conv.total_height(), expected_y);
    }

    #[test]
    fn visible_range_is_within_bounds(
        heights in prop::collection::vec(1u16..100, 1..1000),
        scroll_frac in 0.0..1.0f64,
        viewport_height in 10u16..100,
    ) {
        let entries = create_entries_with_heights(&heights);
        let mut conv = ConversationViewState::from_entries(entries);
        conv.set_scroll(ScrollPosition::Fraction(scroll_frac));

        let viewport = ViewportDimensions::new(80, viewport_height);
        let range = conv.visible_range(viewport);

        prop_assert!(range.start_index <= range.end_index);
        prop_assert!(range.end_index <= conv.len());
    }

    #[test]
    fn scroll_resolve_is_bounded(
        total_height in 0usize..100000,
        viewport_height in 1usize..1000,
        scroll_frac in 0.0..1.0f64,
    ) {
        let scroll = ScrollPosition::Fraction(scroll_frac);
        let resolved = scroll.resolve(total_height, viewport_height, |_| None);

        let max_offset = total_height.saturating_sub(viewport_height);
        prop_assert!(resolved.get() <= max_offset);
    }
}
```

---

## Migration Checklist

When integrating view-state layer with existing code:

- [ ] **AppState**: Replace `Session` with `LogViewState`
- [ ] **ScrollState**: Remove `vertical_offset: usize`, use `ScrollPosition`
- [ ] **ScrollState**: Remove `expanded_messages: HashSet<EntryUuid>` (now per-entry in `EntryView`)
- [ ] **ScrollState**: Remove `wrap_overrides: HashSet<EntryUuid>` (now per-entry in `EntryView`)
- [ ] **message.rs**: Get layout from `EntryView::layout()` instead of computing
- [ ] **message.rs**: Get expand/wrap state from `EntryView` directly
- [ ] **scroll_handler.rs**: Update to use `ScrollPosition` variants
- [ ] **expand_handler.rs**: Use `ConversationViewState::toggle_expand()`
- [ ] **wrap_handler.rs**: Use `EntryView::set_wrap_override()`
- [ ] **mouse_handler.rs**: Use `hit_test()` instead of linear scan
- [ ] **Remove dead code**: Old entry-based scroll bounds, HashSet lookups, inline height calculation

---

## Common Pitfalls

### 1. Off-by-One in Binary Search

```rust
// WRONG: Misses first visible entry
let start = entries.partition_point(|e| e.cumulative_y() < scroll_line);

// CORRECT: Includes partially visible entry
let start = entries.partition_point(|e| e.bottom_y().get() <= scroll_line);
```

### 2. Forgetting to Relayout After Append

```rust
// After append, layout is invalid
conv.append(new_entries);

// Must recompute before visible_range
if conv.needs_relayout(&params) {
    conv.recompute_layout(params, height_calculator);
}
```

### 3. Using Wrong Scroll Offset for Hit Test

```rust
// WRONG: Using scroll position directly
let hit = conv.hit_test(y, x, conv.scroll().into());

// CORRECT: Resolve scroll position first
let offset = conv.scroll().resolve(conv.total_height(), viewport.height, ...);
let hit = conv.hit_test(y, x, offset);
```

---

## Success Verification

After implementation, verify:

1. **No blank viewports**: Scroll through 30k+ entry file, every position shows content
2. **O(log n) visible range**: Time visible_range() with 100k entries, should be <1ms
3. **O(log n) hit test**: Time hit_test() with 100k entries, should be <1ms
4. **Lazy subagent**: Memory usage lower with never-viewed subagents
5. **Cache working**: Second scroll-through of same content is faster
6. **Multi-session**: Log with multiple sessions shows correct active session
