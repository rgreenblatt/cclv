# Research: View-State Layer

**Date**: 2025-12-27
**Status**: Complete
**Related**: [plan.md](./plan.md) | [spec.md](./spec.md)

This document consolidates research findings for the view-state layer implementation.

---

## 1. Layout Computation Strategy

### Decision: Deferred Layout with Width Tracking

**Rationale**: Entry heights depend on viewport width (text wrapping) and expand/collapse state. Computing heights at parse time is impossible because viewport dimensions aren't known. Deferring layout to first render with explicit invalidation is the standard TUI pattern.

**Approach**:
- Store entries without layout initially
- Compute layout lazily on first visibility or explicitly via `recompute_layout(width)`
- Track `layout_width: Option<u16>` to detect width changes requiring relayout
- Cumulative Y offsets computed as running sum during layout pass

**Alternatives Considered**:
1. **Compute at parse time with default width**: Rejected - would require recompute on first real render anyway
2. **Compute on every frame**: Rejected - O(n) per frame is unacceptable for 100k entries
3. **Approximate heights (1 line per entry)**: Rejected - causes the exact bug we're fixing

---

## 2. Binary Search for Visible Range

### Decision: Use `partition_point` on Cumulative Y Offsets

**Rationale**: Finding which entries are visible in viewport requires O(log n) lookup. Rust's `slice::partition_point` (stable since 1.52) provides exactly this capability.

**Approach**:
```rust
// Given: cumulative_y[i] = sum of heights of entries 0..i (exclusive prefix sum)
// Find first entry whose bottom edge is >= scroll_offset (start of visible range)
let first_visible = entries.partition_point(|e| e.cumulative_y + e.height <= scroll_offset);

// Find first entry whose top edge is >= scroll_offset + viewport_height (end of visible range)
let last_visible = entries.partition_point(|e| e.cumulative_y < scroll_offset + viewport_height);
```

**Key insight**: Cumulative Y offsets form a strictly increasing sequence, enabling binary search.

**Alternatives Considered**:
1. **Linear scan from scroll position**: Rejected - O(n) per frame
2. **Segment tree for range queries**: Rejected - over-engineered; simple prefix sum suffices
3. **Viewport-sized chunks with index**: Rejected - complex invalidation on height changes

---

## 3. Scroll Position Representation

### Decision: Sum Type with Semantic Variants

**Rationale**: Raw line offset loses semantic meaning during layout changes. A sum type preserves intent (e.g., "at entry X" survives relayout) while supporting various scroll semantics.

**Chosen variants**:
```rust
pub enum ScrollPosition {
    /// View from the very top (line 0)
    Top,
    /// View from the very bottom (last line visible at bottom of viewport)
    Bottom,
    /// Specific line offset from top (absolute position)
    AtLine(LineOffset),
    /// Keep specific entry at top of viewport (survives relayout)
    AtEntry { entry_index: usize, line_in_entry: usize },
    /// Fractional position (0.0 = top, 1.0 = bottom) - for scrollbar
    Fraction(f64),
}
```

**Resolution**: All variants resolve to absolute line offset via `resolve(total_height, viewport_height)` method. `AtEntry` variant uses entry's cumulative_y from layout.

**Alternatives Considered**:
1. **Raw line offset only**: Rejected - loses semantic meaning on relayout
2. **Always AtEntry**: Rejected - some operations (Page Down) are naturally line-based
3. **Separate "anchor" concept**: Rejected - sum type is simpler and more explicit

---

## 4. LRU Cache for Rendered Output

### Decision: Use `lru` Crate

**Rationale**: Need bounded cache with LRU eviction for rendered entry Lines. The `lru` crate is mature, well-maintained, and provides exactly the needed semantics.

**Crate evaluation**:
| Crate | Stars | Maintained | API | Decision |
|-------|-------|------------|-----|----------|
| `lru` | 1.4k | Yes (2024) | Simple `get`/`put` | ✓ Selected |
| `cached` | 1.3k | Yes | Macro-based, overkill | Rejected |
| `moka` | 1.2k | Yes | Async-focused | Rejected |
| DIY | - | - | More code | Rejected |

**Cache key**: `(EntryUuid, u16 /* width */, bool /* expanded */, WrapMode)`
**Cache value**: `Vec<Line<'static>>`
**Capacity**: 1000 entries (configurable), ~10-50MB depending on content

**Invalidation triggers**:
- Width change: Key includes width, so cache misses automatically
- Expand/collapse: Key includes expanded flag
- Wrap mode change: Key includes wrap mode

**Alternatives Considered**:
1. **No caching**: Rejected - markdown rendering is expensive (~1ms per entry)
2. **HashMap without eviction**: Rejected - unbounded memory growth
3. **Per-entry Option<CachedRender>**: Rejected - harder to bound memory; `lru` is simpler

---

## 5. Multi-Session Active Session Detection

### Decision: Linear Scan on Session Boundaries

**Rationale**: Number of sessions in a log is typically small (1-10). Linear scan through session boundaries to find which session contains the current scroll position is O(sessions), not O(entries).

**Approach**:
- `LogViewState` maintains `sessions: Vec<SessionViewState>`
- Each `SessionViewState` has `start_line: usize` (cumulative from log start)
- `active_session(scroll_line)` scans sessions to find containing one

```rust
fn active_session(&self, scroll_line: usize) -> &SessionViewState {
    self.sessions.iter()
        .rfind(|s| s.start_line <= scroll_line)
        .unwrap_or(&self.sessions[0])
}
```

**Alternatives Considered**:
1. **Binary search on session boundaries**: Rejected - overkill for <10 sessions typically
2. **Cache active session**: Rejected - must update on every scroll anyway
3. **Session ID in scroll position**: Rejected - complicates continuous scroll mode

---

## 6. Lazy Subagent View-State Initialization

### Decision: `Option<ConversationViewState>` with Lazy Construction

**Rationale**: Subagent conversations may never be viewed. Eagerly computing layout wastes memory and CPU. Lazy initialization creates view-state on first tab selection.

**Approach**:
```rust
pub struct SessionViewState {
    main: ConversationViewState,
    // Lazily initialized on first view
    subagents: HashMap<AgentId, Option<ConversationViewState>>,
    // Raw entries stored for deferred layout
    pending_subagent_entries: HashMap<AgentId, Vec<ConversationEntry>>,
}

impl SessionViewState {
    pub fn subagent(&mut self, id: &AgentId, width: u16) -> &ConversationViewState {
        if self.subagents.get(id).map_or(true, |s| s.is_none()) {
            // Build from pending entries
            let entries = self.pending_subagent_entries.remove(id).unwrap_or_default();
            let view_state = ConversationViewState::new(entries, width);
            self.subagents.insert(id.clone(), Some(view_state));
        }
        self.subagents.get(id).unwrap().as_ref().unwrap()
    }
}
```

**Alternatives Considered**:
1. **Eager initialization**: Rejected - wastes resources for never-viewed subagents
2. **Cow<[ConversationEntry]>**: Rejected - complexity without benefit
3. **On-demand parsing**: Rejected - JSON already parsed; this is about layout only

---

## 7. Entry Ownership Model

### Decision: EntryView Owns ConversationEntry Directly

**Rationale**: Spec FR-002 explicitly states view-state layer OWNS domain objects. This provides:
- Cache locality (entry + layout in same allocation)
- No lifetime complexity (no &'a references to external data)
- Simple streaming append (push to vec, no index maintenance)

**Structure**:
```rust
pub struct EntryView {
    /// The domain entry (owned, not referenced)
    pub(crate) entry: ConversationEntry,
    /// Layout computed for current viewport width
    pub(crate) layout: EntryLayout,
}

pub struct EntryLayout {
    /// Height of this entry in lines (minimum 1)
    pub height: u16,
    /// Cumulative Y offset from start of conversation
    pub cumulative_y: usize,
}
```

**Memory impact**: Minimal overhead. `EntryLayout` is 10 bytes. For 100k entries, that's ~1MB additional memory.

**Alternatives Considered**:
1. **Reference via index**: Rejected - spec says own, and ownership is simpler
2. **Arc<ConversationEntry>**: Rejected - no sharing needed; unnecessary indirection
3. **Separate layout vec parallel to entry vec**: Rejected - worse cache locality

---

## 8. Invalidation Tracking

### Decision: Per-Conversation Validity Flags

**Rationale**: Need to know when layout is stale. Track what parameters were used for last layout computation and compare on render.

**Tracked parameters**:
```rust
pub struct LayoutParams {
    pub width: u16,
    pub expanded_set: HashSet<EntryUuid>,
    pub global_wrap: WrapMode,
}

pub struct ConversationViewState {
    entries: Vec<EntryView>,
    last_layout_params: Option<LayoutParams>,
    total_height: usize, // cached sum of all heights
}

impl ConversationViewState {
    pub fn needs_relayout(&self, current: &LayoutParams) -> bool {
        self.last_layout_params.as_ref() != Some(current)
    }
}
```

**Granular invalidation**: Only recompute heights for entries whose expand state changed, then recompute cumulative_y for all entries after the first changed one.

**Alternatives Considered**:
1. **Always recompute**: Rejected - O(n) per frame unacceptable
2. **Dirty flags per entry**: Rejected - cumulative_y still requires full scan from first dirty
3. **Version numbers**: Rejected - LayoutParams comparison is clearer

---

## 9. Height Calculation

### Decision: Render-Based Measurement

**Rationale**: Accurate height requires actually rendering (or simulating render) because:
- Markdown parsing affects line count (headings, code blocks, lists)
- Text wrapping depends on viewport width
- Collapse state changes visible lines

**Approach**:
```rust
fn compute_height(entry: &ConversationEntry, width: u16, expanded: bool, wrap: WrapMode) -> u16 {
    let text = extract_entry_text(entry);
    if !expanded && text.lines().count() > COLLAPSE_THRESHOLD {
        return SUMMARY_LINES as u16 + 1; // +1 for "(+N more lines)" indicator
    }

    // Render markdown to get actual line count
    let rendered = render_markdown(&text);
    let lines = if wrap == WrapMode::Wrap {
        count_wrapped_lines(&rendered, width)
    } else {
        rendered.len()
    };

    lines.max(1) as u16 // Minimum 1 line per entry
}
```

**Performance**: ~0.1-1ms per entry for markdown rendering. For 30k entries initial load, parallelize with rayon if needed (future optimization).

**Alternatives Considered**:
1. **Character count heuristic**: Rejected - inaccurate for markdown
2. **Pre-render and cache at parse time**: Rejected - width not known
3. **Assume 1 line per entry**: Rejected - defeats purpose of view-state layer

---

## 10. Session Boundary Detection

### Decision: Detect via session_id Field Change

**Rationale**: Per spec FR-078, session boundaries are detected when `session_id` changes between consecutive entries. Sessions are never interleaved.

**Approach**:
```rust
fn detect_sessions(entries: &[ConversationEntry]) -> Vec<SessionBoundary> {
    let mut boundaries = vec![];
    let mut current_session: Option<SessionId> = None;

    for (idx, entry) in entries.iter().enumerate() {
        if let Some(session_id) = entry.session_id() {
            if current_session.as_ref() != Some(session_id) {
                boundaries.push(SessionBoundary { start_index: idx, session_id: session_id.clone() });
                current_session = Some(session_id.clone());
            }
        }
    }

    boundaries
}
```

**Streaming consideration**: New entries appended. If session_id changes from last entry's session, create new SessionViewState.

**Alternatives Considered**:
1. **Require init marker**: Rejected - spec says no init marker required (FR-079)
2. **Time-based gaps**: Rejected - unreliable; session_id is authoritative
3. **Explicit session end marker**: Rejected - may be missing for incomplete sessions

---

## Summary of Key Decisions

| Topic | Decision | Key Rationale |
|-------|----------|---------------|
| Layout timing | Deferred with width tracking | Width unknown at parse time |
| Visible range | `partition_point` binary search | O(log n) lookup on cumulative_y |
| Scroll position | Sum type with semantic variants | Preserves intent across relayout |
| Render cache | `lru` crate, 1000 entry capacity | Bounded memory, simple API |
| Active session | Linear scan on boundaries | <10 sessions typical |
| Lazy subagents | `Option<ConversationViewState>` | Avoid work for never-viewed |
| Ownership | EntryView owns ConversationEntry | Cache locality, no lifetimes |
| Invalidation | LayoutParams comparison | Clear equality semantics |
| Height calc | Render-based measurement | Markdown affects line count |
| Session detection | session_id field change | Spec-mandated (FR-078) |

---

## Dependencies to Add

```toml
# Cargo.toml additions
[dependencies]
lru = "0.12"  # LRU cache for rendered output
```

No other new dependencies required. Existing dependencies (ratatui, tui-markdown, proptest) sufficient.
