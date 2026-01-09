# Implementation Plan: Session Navigation

**Branch**: `003-session-navigation` | **Date**: 2025-01-09 | **Spec**: [spec.md](./spec.md)
**Input**: Feature specification from `/specs/003-session-navigation/spec.md`

## Summary

Add multi-session navigation to cclv: a modal session list (`S` key), session switching with preserved view state, live-tail restriction to last session only, session-scoped subagent tabs, and multi-level stats aggregation (MainAgent, Subagent, Session, AllSessionsCombined).

## Technical Context

**Language/Version**: Rust 1.83+ (2021 edition)
**Primary Dependencies**: ratatui 0.29, crossterm 0.28, serde_json, chrono, tui-markdown, fenwick
**Storage**: N/A (in-memory view-state)
**Testing**: cargo test + proptest + insta snapshots
**Target Platform**: Linux/macOS terminal (TUI)
**Project Type**: Single Rust crate (library + binary)
**Performance Goals**: Session switch < 2 seconds, modal render < 16ms
**Constraints**: Memory proportional to log file size, no blocking in render loop
**Scale/Scope**: 1-100+ sessions per file, up to ~1000MB log files

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Gate | Status | Notes |
|------|--------|-------|
| Types designed first | ✅ Pass | New types defined in data-model.md before implementation |
| Cardinality analyzed | ✅ Pass | Sum types for stats filter, modal state; precision ~1.0 |
| No illegal states | ✅ Pass | `ViewedSession` cannot reference non-existent session |
| Smart constructors | ✅ Pass | `SessionIndex` validated at construction |
| Total functions | ✅ Pass | All public APIs return Option/Result, no panics |
| Pure domain logic | ✅ Pass | Session selection, stats aggregation are pure |
| Property tests | ✅ Pass | Invariants for session ordering, stats summation |
| Structured logging | ✅ Pass | tracing with session_id, session_index fields |
| Build passes | Pending | Implementation not started |
| Tests pass | Pending | Implementation not started |
| Linting clean | Pending | Implementation not started |

## Project Structure

### Documentation (this feature)

```text
specs/003-session-navigation/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output
├── quickstart.md        # Phase 1 output
├── contracts/           # Phase 1 output (TUI contracts, not REST)
└── tasks.md             # Phase 2 output (NOT created by /speckit.plan)
```

### Source Code (repository root)

```text
src/
├── model/
│   ├── stats.rs         # MODIFY: Add Session(SessionId), rename Global->AllSessionsCombined
│   └── mod.rs           # MODIFY: Re-export new StatsFilter variants
├── state/
│   ├── app_state.rs     # MODIFY: Add session_modal_visible, viewed_session_index
│   ├── session_modal_handler.rs  # NEW: Session list modal input handling
│   └── mod.rs           # MODIFY: Re-export session modal handler
├── view/
│   ├── session_modal.rs # NEW: Session list modal widget
│   └── mod.rs           # MODIFY: Re-export session modal
├── view_state/
│   ├── session.rs       # MODIFY: Add per-session stats tracking
│   └── log.rs           # MODIFY: Add viewed_session_index, session selection methods
└── tests/
    ├── acceptance_session_nav.rs     # NEW: US1-5 acceptance tests
    └── session_modal_snapshots.rs    # NEW: Modal rendering snapshots
```

**Structure Decision**: Single crate with modular organization. New session navigation code follows existing patterns: handlers in `state/`, widgets in `view/`, view-state in `view_state/`.

## Complexity Tracking

> No violations requiring justification. All changes fit existing architecture.

---

## Phase 0: Research Summary

### R1: Session Detection (Already Implemented)
**Decision**: Use existing `session_id` field comparison in `LogViewState::add_entry`
**Rationale**: Multi-session detection already works (see `view_state/log.rs:78-118`)
**Alternatives**: None needed - existing implementation is correct

### R2: Modal Widget Pattern
**Decision**: Follow existing help overlay pattern (`help_visible`, `help_scroll_offset`)
**Rationale**: Consistent with cclv's existing modal patterns; proven approach
**Alternatives**: Separate modal stack - rejected, overkill for single modal

### R3: Stats Aggregation Strategy
**Decision**: Extend `StatsFilter` enum, add `Session(SessionId)` variant
**Rationale**: Type-safe scoping; existing `filtered_usage`/`filtered_tool_counts` pattern
**Alternatives**: Computed stats on demand - rejected, would require recomputing on every render

### R4: Live Tailing Restriction
**Decision**: Gate `auto_scroll` on `viewed_session_index == session_count - 1`
**Rationale**: Simple boolean check; matches spec requirement exactly
**Alternatives**: Separate "tailable" flag per session - rejected, unnecessary complexity

### R5: Session-Scoped Subagent Tabs
**Decision**: Filter subagent list by viewed session's `session_id`
**Rationale**: Already have per-session `SessionViewState` with subagent map
**Alternatives**: Global subagent list with session prefix - rejected, confusing UX

---

## Phase 1: Design

### Key Types

```rust
// ===== New Types =====

/// Index into LogViewState.sessions (0-based, validated)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SessionIndex(usize);

impl SessionIndex {
    pub fn new(index: usize, session_count: usize) -> Option<Self> {
        if index < session_count { Some(Self(index)) } else { None }
    }
    pub fn get(&self) -> usize { self.0 }
    pub fn is_last(&self, session_count: usize) -> bool { self.0 + 1 == session_count }
}

/// Session metadata for display in modal
pub struct SessionSummary {
    pub index: SessionIndex,
    pub session_id: SessionId,
    pub message_count: usize,
    pub start_time: Option<DateTime<Utc>>,
    pub subagent_count: usize,
}

// ===== Modified Types =====

/// Extended StatsFilter (FR-008)
pub enum StatsFilter {
    AllSessionsCombined,           // Renamed from Global
    MainAgent(SessionId),          // Now requires session context
    Subagent(AgentId),             // Unchanged
    Session(SessionId),            // NEW: Per-session totals (main + subagents)
}

/// AppState additions
pub struct AppState {
    // ... existing fields ...
    pub session_modal_visible: bool,
    pub session_modal_selected: usize,  // Selection index within modal
    pub viewed_session_index: Option<SessionIndex>,  // None = last session
}
```

### State Machine

```
Normal Mode
    │
    ├── [S] → Session Modal Open
    │           │
    │           ├── [↑/↓] → Navigate list
    │           ├── [Enter] → Select & close → Normal Mode (viewed session changed)
    │           ├── [Esc] → Close → Normal Mode (no change)
    │           └── [S] → Close → Normal Mode (toggle)
    │
    └── Live tailing enabled iff viewed_session_index.is_last(session_count)
```

### Contracts

See `contracts/session-modal.md` for:
- Modal layout specification
- Keyboard bindings
- Session summary format

See `contracts/stats-filter.md` for:
- Aggregation level semantics
- Filter cycling behavior

### Acceptance Test Strategy (TDD)

Per user request: **stubs with ignored tests first, then implementation**.

1. **Phase 2.1**: Write stub types and ignored acceptance tests with snapshots
2. **Phase 2.2**: Implement session modal display (US1)
3. **Phase 2.3**: Implement session selection (US2)
4. **Phase 2.4**: Implement live tailing behavior (US3)
5. **Phase 2.5**: Implement stats aggregation (US4)
6. **Phase 2.6**: Implement session identification (US5)

Each phase: un-ignore tests → implement → GREEN → commit.

### Property Invariants

```rust
// 1. viewed_session_index always valid
// forall state: state.viewed_session_index.map(|i| i.get()) < state.log_view.session_count()

// 2. Stats summation consistency
// forall sessions: sum(Session(s).usage for s in sessions) == AllSessionsCombined.usage

// 3. Session ordering preserved
// forall i < j: sessions[i].start_line <= sessions[j].start_line

// 4. Live tail only on last session
// auto_scroll == true implies viewed_session_index.is_last(session_count)
```

---

## Implementation Phases (Preview)

### Phase 2.1: Stubs & Ignored Tests
- Add `SessionIndex`, `SessionSummary` types (stubs)
- Add `session_modal_visible`, `viewed_session_index` to `AppState`
- Write acceptance tests for US1-5 with `#[ignore]`
- All tests compile but are ignored
- **GREEN build**

### Phase 2.2: Session Modal Display (US1)
- Implement `SessionSummary::from_session`
- Implement `session_modal.rs` widget
- Un-ignore US1 tests
- **GREEN tests**

### Phase 2.3: Session Selection (US2)
- Implement modal keyboard handler
- Implement `AppState::select_session`
- Implement view state switching
- Un-ignore US2 tests
- **GREEN tests**

### Phase 2.4: Live Tailing Behavior (US3)
- Add `is_last_session` check to auto_scroll logic
- Update LIVE indicator based on viewed session
- Un-ignore US3 tests
- **GREEN tests**

### Phase 2.5: Stats Aggregation (US4)
- Rename `Global` → `AllSessionsCombined`
- Add `Session(SessionId)` variant
- Update `MainAgent` to `MainAgent(SessionId)`
- Implement per-session stats computation
- Un-ignore US4 tests
- **GREEN tests**

### Phase 2.6: Session Identification (US5)
- Add session metadata extraction
- Update modal display with counts/timestamps
- Un-ignore US5 tests
- **GREEN tests**

---

## Dependencies & Risks

| Risk | Mitigation |
|------|------------|
| Breaking existing stats tests | Update test fixtures incrementally |
| Modal render performance | Use visible_range pattern from conversation view |
| Session switching lag | Lazy layout computation (already exists) |
| Live tail edge cases | Explicit state machine, property tests |

---

## Next Steps

1. Run `/speckit.tasks` to generate detailed task breakdown
2. Create stubs and ignored tests (Phase 2.1)
3. Implement iteratively with TDD discipline
