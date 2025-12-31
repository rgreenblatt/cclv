# Implementation Plan: View-State Layer

**Branch**: `002-view-state-layer` | **Date**: 2025-12-27 | **Spec**: [spec.md](./spec.md)
**Input**: Feature specification from `specs/002-view-state-layer/spec.md`

## Summary

This feature implements a dedicated view-state layer to separate domain concerns (LogEntry, Session, Message) from presentation concerns (scroll position, layout, rendering cache). The view-state layer owns domain data directly (parsed from JSON into view-state types), computes precomputed cumulative Y offsets for O(log n) scroll/hit-test operations, and supports multi-session logs with lazy subagent view-state creation. This fixes the current scroll/render type mismatch bug where line-based scroll offset is incorrectly bounded by entry count instead of total line count.

## Technical Context

**Language/Version**: Rust 1.75+ (2021 edition)
**Primary Dependencies**: ratatui 0.29, crossterm 0.28, serde_json, chrono, tui-markdown, syntect
**Storage**: N/A (in-memory view-state, no persistence)
**Testing**: cargo test (unit), proptest (property-based), insta (snapshot)
**Target Platform**: Linux (primary), macOS, Windows (cross-platform terminal)
**Project Type**: Single project (TUI application)
**Performance Goals**:
- 60fps rendering (16ms frame budget)
- O(log n) visible range calculation
- O(log n) hit-testing
- <500ms initial layout for 30,000 entries
- <10ms incremental layout for 100 streaming entries
**Constraints**:
- <50MB memory for view-state of 100,000 entries (excluding render cache)
- Zero blank viewports at any scroll position
**Scale/Scope**: 100,000+ entries per log file, multiple sequential sessions per file

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Status | Notes |
|-----------|--------|-------|
| I. Type-Driven Design | ✓ PASS | New types designed before implementation: `EntryView`, `EntryLayout`, `ScrollPosition`, `ConversationViewState`, `SessionViewState`, `LogViewState`, `VisibleRange`, `CachedRender` |
| II. Deep Module Architecture | ✓ PASS | View-state layer hides layout complexity behind simple interface: `visible_range()`, `hit_test()`, `scroll_to()` |
| III. Denotational Semantics | ✓ PASS | Scroll operations have clear mathematical meaning; `visible_range()` is a pure function of scroll position and viewport dimensions |
| IV. Total Functions | ✓ PASS | All operations return valid results; scroll positions clamp to valid range; hit-testing returns `Option` |
| V. Pure Core / Impure Shell | ✓ PASS | View-state types are pure data with pure layout calculations; rendering (impure) consumes computed layout |
| VI. Property-Based Testing | ✓ PASS | Invariants testable via proptest: cumulative_y[i] = sum(height[0..i]), scroll bounds always valid |
| VII. Cardinality Analysis | ✓ PASS | `ScrollPosition` is sum type (finite variants); `EntryView` owns exactly one `ConversationEntry` |
| VIII. Skill-Based Development | ✓ PASS | Using typed-domain-modeling skill for algebraic type design |

### Pre-Design Gate: PASSED

No violations detected. Proceeding to Phase 0.

## Project Structure

### Documentation (this feature)

```text
specs/002-view-state-layer/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output
├── quickstart.md        # Phase 1 output
└── contracts/           # Phase 1 output (internal API contracts)
```

### Source Code (repository root)

```text
src/
├── model/               # Domain types (PURE) - existing, unchanged
│   ├── identifiers.rs   # EntryUuid, SessionId, AgentId, ToolUseId
│   ├── log_entry.rs     # LogEntry, EntryType, EntryMetadata
│   ├── message.rs       # Message, Role, MessageContent, ContentBlock
│   ├── conversation_entry.rs  # ConversationEntry (Valid | Malformed)
│   ├── session.rs       # Session, AgentConversation (to be deprecated)
│   └── ...
├── view_state/          # NEW: View-state layer (PURE)
│   ├── mod.rs           # Module exports
│   ├── entry_view.rs    # EntryView (owns ConversationEntry + layout)
│   ├── layout.rs        # EntryLayout, height computation
│   ├── scroll.rs        # ScrollPosition sum type
│   ├── conversation.rs  # ConversationViewState
│   ├── session.rs       # SessionViewState
│   ├── log.rs           # LogViewState (top-level)
│   ├── visible_range.rs # VisibleRange calculation
│   ├── hit_test.rs      # Mouse hit-testing
│   └── cache.rs         # CachedRender, LRU eviction
├── state/               # UI state (PURE) - modified
│   ├── app_state.rs     # Updated to use LogViewState
│   └── ...
├── parser/              # JSONL parsing (PURE) - unchanged
├── source/              # Input sources (IMPURE) - unchanged
└── view/                # TUI rendering (IMPURE) - modified
    ├── message.rs       # Updated to consume view-state layout
    └── ...

tests/
├── view_state/          # NEW: View-state tests
│   ├── layout_tests.rs
│   ├── scroll_tests.rs
│   ├── visible_range_tests.rs
│   └── hit_test_tests.rs
└── ...
```

**Structure Decision**: Single project with new `view_state/` module alongside existing `model/`, `state/`, `view/` modules. The view-state layer sits between parsing (model/) and rendering (view/), owning domain data and providing computed layout information.

## Complexity Tracking

> No violations to justify. Design follows all constitution principles.

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| N/A | N/A | N/A |
