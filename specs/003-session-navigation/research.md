# Research: Session Navigation

**Date**: 2025-01-09
**Status**: Complete
**Related**: [plan.md](./plan.md) | [spec.md](./spec.md)

This document captures research findings that informed the implementation plan.

---

## R1: Session Detection Mechanism

**Question**: How are session boundaries detected in JSONL files?

**Finding**: Already implemented in `src/view_state/log.rs:78-118`

The existing `LogViewState::add_entry` method:
1. Extracts `session_id` from each entry
2. Compares with `current_session_id`
3. Creates new `SessionViewState` when UUID changes
4. Routes entries to correct session

```rust
// From log.rs:78-118
pub fn add_entry(&mut self, entry: ConversationEntry, agent_id: Option<AgentId>) {
    let session_id = entry.session_id().cloned();

    // Detect session boundary
    if session_id != self.current_session_id {
        if let Some(new_id) = session_id.clone() {
            // ... create new session
        }
    }
    // ... route to session
}
```

**Decision**: No changes needed to session detection. Focus on navigation and display.

---

## R2: Modal Widget Patterns in cclv

**Question**: How should the session list modal be implemented?

**Finding**: Existing help overlay pattern in `src/state/app_state.rs`

Current modal pattern:
- `help_visible: bool` - toggles overlay visibility
- `help_scroll_offset: u16` - tracks scroll position
- Keyboard handler checks `help_visible` before processing

The help overlay is rendered as a centered popup over the main content.

**Decision**: Follow the same pattern:
- `session_modal_visible: bool`
- `session_modal_selected: usize` - selected row in list
- Modal keyboard handler for navigation

**Alternative Considered**: Generic modal stack
- Rejected: Overkill for two modals (help, sessions)
- Would add abstraction without clear benefit

---

## R3: Stats Aggregation Architecture

**Question**: How should multi-level stats aggregation work?

**Finding**: Current `StatsFilter` in `src/model/stats.rs:238-248`

```rust
pub enum StatsFilter {
    Global,           // All agents combined
    MainAgent,        // Main agent only
    Subagent(AgentId), // Specific subagent
}
```

Current `filtered_usage` and `filtered_tool_counts` methods provide per-filter access.

**Spec Requirement (FR-008)**: Four levels needed:
1. `MainAgent(SessionId)` - specific session's main agent
2. `Subagent(AgentId)` - specific subagent (unchanged)
3. `Session(SessionId)` - per-session totals (main + subagents)
4. `AllSessionsCombined` - cross-session totals

**Decision**: Extend `StatsFilter` enum:

```rust
pub enum StatsFilter {
    AllSessionsCombined,           // Renamed from Global
    MainAgent(SessionId),          // Now session-scoped
    Subagent(AgentId),             // Unchanged
    Session(SessionId),            // NEW
}
```

**Breaking Change**: `Global` → `AllSessionsCombined`, `MainAgent` gains `SessionId` parameter.
- Update all call sites
- Update existing tests

---

## R4: Live Tailing Implementation

**Question**: How to restrict live tailing to last session only?

**Finding**: Current auto-scroll logic in `src/state/scroll_handler.rs`

The `auto_scroll` flag in `AppState` controls whether new entries cause automatic scrolling.

**Spec Requirement (FR-006, FR-007)**:
- Disable live tailing when viewing historical session
- Re-enable when returning to last session

**Decision**: Simple boolean gate:

```rust
fn is_tailing_enabled(&self) -> bool {
    self.auto_scroll && self.is_viewing_last_session()
}

fn is_viewing_last_session(&self) -> bool {
    match self.viewed_session_index {
        None => true,  // None means "follow latest"
        Some(idx) => idx.is_last(self.log_view.session_count())
    }
}
```

**Alternative Considered**: Per-session tailing flag
- Rejected: Unnecessary state; last-session check is sufficient

---

## R5: Session-Scoped Subagent Tabs

**Question**: How should subagent tabs work with multi-session?

**Finding**: Current `SessionViewState` in `src/view_state/session.rs`

Each session already tracks its own subagents:
```rust
pub struct SessionViewState {
    subagents: HashMap<AgentId, ConversationViewState>,
    pending_subagent_entries: HashMap<AgentId, Vec<ConversationEntry>>,
    // ...
}
```

**Spec Requirement (FR-011)**: Subagent tabs scoped to viewed session.

**Decision**: When rendering tabs:
1. Get viewed session via `viewed_session_index`
2. Use that session's `subagent_ids()` for tab list
3. `ConversationSelection::Subagent(id)` validates against viewed session

**Alternative Considered**: Flatten all sessions' subagents into global list with prefixes
- Rejected: Confusing UX, breaks mental model of session isolation

---

## R6: Status Bar Session Indicator

**Question**: How to display current session in status bar?

**Finding**: Existing status bar in `src/view/layout.rs`

Status bar currently shows: mode indicators, wrap mode, LIVE indicator.

**Spec Requirement (FR-012)**: Show "Session N/M" format.

**Decision**: Add after LIVE indicator:
```
│ LIVE │ Wrap │ Session 2/3 │
```

Format: `Session {viewed_index + 1}/{session_count}`

---

## R7: Performance Considerations

**Question**: Will session switching cause UI lag?

**Finding**: Existing lazy layout in `ConversationViewState`

Layout is computed on-demand via `recompute_layout` when:
- Viewport size changes
- Wrap mode changes
- Entry expand state changes

Each session's `ConversationViewState` maintains its own layout state.

**Decision**: No special optimization needed:
- Session switch just changes `viewed_session_index`
- Existing lazy layout handles the rest
- First render of historical session may compute layout, but this is fast

**Benchmark Target**: < 2 seconds for session switch (SC-001)
- Current layout computation is O(n) in entry count
- Acceptable for expected file sizes (< 10MB, < 100k entries)

---

## R8: Modal Keyboard Bindings

**Question**: What keys should control the session modal?

**Finding**: Existing keybinding system in `src/config/keybindings.rs`

Keybindings are configurable via TOML config.

**Decision**: Default bindings for session modal:
- `S` (Shift+S) - Toggle session modal (matches spec)
- `↑/k` - Move selection up
- `↓/j` - Move selection down
- `Enter` - Select and close
- `Esc` - Cancel and close
- `Home/g` - Jump to first session
- `End/G` - Jump to last session

These follow vim-style conventions consistent with existing cclv bindings.

---

## Summary

| Research Area | Decision | Confidence |
|--------------|----------|------------|
| Session detection | Use existing implementation | High |
| Modal pattern | Follow help overlay pattern | High |
| Stats aggregation | Extend StatsFilter enum | High |
| Live tailing | Gate on is_viewing_last_session | High |
| Subagent tabs | Session-scoped from SessionViewState | High |
| Status bar | "Session N/M" format | High |
| Performance | Rely on lazy layout | Medium |
| Keybindings | Vim-style defaults | High |

All research questions resolved. Ready for Phase 1 design.
