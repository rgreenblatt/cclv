# Data Model: Session Navigation

**Date**: 2025-01-09
**Status**: Design Complete
**Related**: [plan.md](./plan.md) | [research.md](./research.md) | [spec.md](./spec.md)

This document defines the type-driven data model for session navigation following the project constitution:
- **Smart constructors only**: Never export raw constructors
- **No primitive obsession**: Newtypes for all domain concepts
- **Illegal states unrepresentable**: Sum types enforce valid states
- **Parse at boundaries**: Validate once during construction
- **Cardinality analysis**: Precision approaching 1.0

---

## 1. SessionIndex

A validated index into the session list. Cannot represent out-of-bounds values.

```rust
// ===== src/view_state/types.rs (addition) =====

/// Validated index into LogViewState.sessions.
///
/// # Invariants
/// - Always < session_count at construction time
/// - 0-indexed: 0 is the first session
///
/// # Smart Constructor
/// Use `SessionIndex::new(index, session_count)` which returns `Option<Self>`.
/// Never export the raw constructor.
///
/// # Cardinality
/// - Valid states: [0, session_count)
/// - Total states: [0, usize::MAX)
/// - Precision: session_count / usize::MAX ≈ 1.0 for typical session counts
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SessionIndex(usize);

impl SessionIndex {
    /// Create a validated session index.
    ///
    /// Returns `None` if index >= session_count.
    ///
    /// # Examples
    /// ```
    /// let idx = SessionIndex::new(0, 3); // Some(SessionIndex(0))
    /// let idx = SessionIndex::new(3, 3); // None (out of bounds)
    /// ```
    pub fn new(index: usize, session_count: usize) -> Option<Self> {
        if index < session_count {
            Some(Self(index))
        } else {
            None
        }
    }

    /// Get the raw index value.
    pub fn get(&self) -> usize {
        self.0
    }

    /// Display index (1-based, for user-facing display).
    pub fn display(&self) -> usize {
        self.0 + 1
    }

    /// Check if this is the last session.
    ///
    /// Used to determine if live tailing should be enabled.
    pub fn is_last(&self, session_count: usize) -> bool {
        self.0 + 1 == session_count
    }

    /// Check if this is the first session.
    pub fn is_first(&self) -> bool {
        self.0 == 0
    }

    /// Next session index, if valid.
    pub fn next(&self, session_count: usize) -> Option<Self> {
        Self::new(self.0 + 1, session_count)
    }

    /// Previous session index, if valid.
    pub fn prev(&self) -> Option<Self> {
        if self.0 > 0 {
            Some(Self(self.0 - 1))
        } else {
            None
        }
    }
}

impl std::fmt::Display for SessionIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display())
    }
}
```

---

## 2. SessionSummary

Metadata for display in the session list modal. Read-only view of session state.

```rust
// ===== src/view_state/session_summary.rs (new file) =====

use crate::model::SessionId;
use crate::view_state::types::SessionIndex;
use chrono::{DateTime, Utc};

/// Summary metadata for a session, used in the session list modal.
///
/// This is a read-only snapshot of session state for display purposes.
/// Computed from SessionViewState on demand.
///
/// # FR-009: Display session metadata including:
/// - Session number (index + 1)
/// - Start timestamp
/// - Message count
#[derive(Debug, Clone)]
pub struct SessionSummary {
    /// Validated index of this session.
    index: SessionIndex,

    /// Session identifier (UUID).
    session_id: SessionId,

    /// Total message count in main conversation.
    message_count: usize,

    /// Timestamp of first entry in session (if available).
    start_time: Option<DateTime<Utc>>,

    /// Number of subagents spawned in this session.
    subagent_count: usize,
}

impl SessionSummary {
    /// Create a new session summary.
    ///
    /// # Arguments
    /// - `index`: Validated session index
    /// - `session_id`: Session UUID
    /// - `message_count`: Number of messages in main conversation
    /// - `start_time`: Timestamp of first entry
    /// - `subagent_count`: Number of subagents
    pub fn new(
        index: SessionIndex,
        session_id: SessionId,
        message_count: usize,
        start_time: Option<DateTime<Utc>>,
        subagent_count: usize,
    ) -> Self {
        Self {
            index,
            session_id,
            message_count,
            start_time,
            subagent_count,
        }
    }

    /// Session index.
    pub fn index(&self) -> SessionIndex {
        self.index
    }

    /// Session ID.
    pub fn session_id(&self) -> &SessionId {
        &self.session_id
    }

    /// Message count in main conversation.
    pub fn message_count(&self) -> usize {
        self.message_count
    }

    /// Start time of session.
    pub fn start_time(&self) -> Option<DateTime<Utc>> {
        self.start_time
    }

    /// Number of subagents.
    pub fn subagent_count(&self) -> usize {
        self.subagent_count
    }

    /// Format for display in session list.
    ///
    /// Returns: "Session N: X messages, Y subagents"
    pub fn display_line(&self) -> String {
        let time_str = self
            .start_time
            .map(|t| t.format(" (%H:%M)").to_string())
            .unwrap_or_default();

        format!(
            "Session {}: {} messages, {} subagents{}",
            self.index.display(),
            self.message_count,
            self.subagent_count,
            time_str
        )
    }
}
```

---

## 3. ViewedSession

Represents which session is currently being viewed. Sum type handles "follow latest" vs "pinned to specific".

```rust
// ===== src/state/viewed_session.rs (new file) =====

use crate::view_state::types::SessionIndex;

/// Which session is currently being viewed.
///
/// # States
/// - `Latest`: Follow the most recent session (enables live tailing)
/// - `Pinned(index)`: View a specific historical session (disables live tailing)
///
/// # Cardinality
/// - Latest: 1 state
/// - Pinned: N states (where N = session count)
/// - Total: N + 1 states (all valid)
/// - Precision: 1.0
///
/// # Invariant
/// `Pinned(idx)` always holds a valid SessionIndex (validated at construction).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ViewedSession {
    /// Follow the latest (last) session. Enables live tailing.
    #[default]
    Latest,

    /// Pinned to a specific session. Disables live tailing.
    Pinned(SessionIndex),
}

impl ViewedSession {
    /// Create a pinned view to specific session.
    ///
    /// Returns `None` if index is invalid for current session count.
    pub fn pinned(index: usize, session_count: usize) -> Option<Self> {
        SessionIndex::new(index, session_count).map(Self::Pinned)
    }

    /// Check if viewing the last session.
    ///
    /// Used to determine if live tailing should be enabled.
    pub fn is_last(&self, session_count: usize) -> bool {
        match self {
            ViewedSession::Latest => true,
            ViewedSession::Pinned(idx) => idx.is_last(session_count),
        }
    }

    /// Get the effective session index.
    ///
    /// For `Latest`, returns the last session index.
    /// For `Pinned`, returns the pinned index.
    pub fn effective_index(&self, session_count: usize) -> Option<SessionIndex> {
        match self {
            ViewedSession::Latest => {
                if session_count > 0 {
                    SessionIndex::new(session_count - 1, session_count)
                } else {
                    None
                }
            }
            ViewedSession::Pinned(idx) => Some(*idx),
        }
    }

    /// Move to next session (toward latest).
    ///
    /// If at last session, switches to `Latest` mode.
    pub fn next(&self, session_count: usize) -> Self {
        match self {
            ViewedSession::Latest => ViewedSession::Latest,
            ViewedSession::Pinned(idx) => {
                if idx.is_last(session_count) {
                    ViewedSession::Latest
                } else {
                    idx.next(session_count)
                        .map(ViewedSession::Pinned)
                        .unwrap_or(ViewedSession::Latest)
                }
            }
        }
    }

    /// Move to previous session (toward first).
    ///
    /// If at first session, stays at first.
    pub fn prev(&self, session_count: usize) -> Self {
        match self {
            ViewedSession::Latest => {
                if session_count > 1 {
                    SessionIndex::new(session_count - 2, session_count)
                        .map(ViewedSession::Pinned)
                        .unwrap_or(ViewedSession::Latest)
                } else {
                    ViewedSession::Latest
                }
            }
            ViewedSession::Pinned(idx) => idx
                .prev()
                .map(ViewedSession::Pinned)
                .unwrap_or(ViewedSession::Pinned(*idx)),
        }
    }
}
```

---

## 4. Extended StatsFilter

Updated statistics filter with session-level aggregation.

```rust
// ===== src/model/stats.rs (modification) =====

use crate::model::{AgentId, SessionId};

/// Filter for statistics display (FR-008).
///
/// # Aggregation Levels
///
/// 1. `AllSessionsCombined`: Total across all sessions and agents
/// 2. `Session(SessionId)`: Per-session total (main + all subagents)
/// 3. `MainAgent(SessionId)`: Specific session's main agent only
/// 4. `Subagent(AgentId)`: Specific subagent (any session)
///
/// # Cardinality
/// - AllSessionsCombined: 1 state
/// - Session: S states (S = session count)
/// - MainAgent: S states
/// - Subagent: A states (A = total subagent count)
/// - Total: 1 + S + S + A = 1 + 2S + A states (all valid)
/// - Precision: 1.0
///
/// # Breaking Change from Previous Version
/// - `Global` renamed to `AllSessionsCombined`
/// - `MainAgent` now takes `SessionId` parameter
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StatsFilter {
    /// Statistics for all sessions combined (all agents).
    /// Renamed from `Global` for clarity.
    AllSessionsCombined,

    /// Statistics for a specific session (main + all subagents combined).
    /// NEW: Per-session aggregation level.
    Session(SessionId),

    /// Statistics for a specific session's main agent only.
    /// CHANGED: Now requires SessionId to scope to specific session.
    MainAgent(SessionId),

    /// Statistics for a specific subagent.
    /// Unchanged from previous version.
    Subagent(AgentId),
}

impl StatsFilter {
    /// Human-readable label for the filter.
    pub fn label(&self) -> String {
        match self {
            StatsFilter::AllSessionsCombined => "All Sessions".to_string(),
            StatsFilter::Session(id) => format!("Session {}", id),
            StatsFilter::MainAgent(id) => format!("Main Agent ({})", id),
            StatsFilter::Subagent(id) => format!("Subagent {}", id),
        }
    }

    /// Short label for status bar.
    pub fn short_label(&self) -> &'static str {
        match self {
            StatsFilter::AllSessionsCombined => "All",
            StatsFilter::Session(_) => "Session",
            StatsFilter::MainAgent(_) => "Main",
            StatsFilter::Subagent(_) => "Sub",
        }
    }
}

impl Default for StatsFilter {
    /// Default to showing current session's main agent stats.
    ///
    /// Note: Actual SessionId must be provided by AppState based on
    /// viewed session. This default is a placeholder.
    fn default() -> Self {
        // This will be replaced by actual session ID in AppState
        StatsFilter::AllSessionsCombined
    }
}
```

---

## 5. SessionModalState

State for the session list modal widget.

```rust
// ===== src/state/session_modal.rs (new file) =====

use crate::view_state::types::SessionIndex;

/// State for the session list modal.
///
/// # Cardinality
/// - When closed: 1 state (visible = false)
/// - When open: session_count states (one per valid selection)
/// - Total: 1 + session_count states (all valid)
/// - Precision: 1.0
#[derive(Debug, Clone, Default)]
pub struct SessionModalState {
    /// Whether the modal is visible.
    visible: bool,

    /// Currently selected row in the modal (0-indexed).
    /// Only meaningful when `visible` is true.
    selected_index: usize,

    /// Scroll offset for long session lists.
    scroll_offset: usize,
}

impl SessionModalState {
    /// Create new modal state (closed).
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if modal is visible.
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Open the modal, pre-selecting the given session.
    pub fn open(&mut self, current_session_index: usize) {
        self.visible = true;
        self.selected_index = current_session_index;
        self.scroll_offset = 0;
    }

    /// Close the modal.
    pub fn close(&mut self) {
        self.visible = false;
    }

    /// Toggle modal visibility.
    pub fn toggle(&mut self, current_session_index: usize) {
        if self.visible {
            self.close();
        } else {
            self.open(current_session_index);
        }
    }

    /// Currently selected index.
    pub fn selected_index(&self) -> usize {
        self.selected_index
    }

    /// Move selection up, clamping at 0.
    pub fn select_prev(&mut self) {
        self.selected_index = self.selected_index.saturating_sub(1);
    }

    /// Move selection down, clamping at max.
    pub fn select_next(&mut self, session_count: usize) {
        if session_count > 0 {
            self.selected_index = (self.selected_index + 1).min(session_count - 1);
        }
    }

    /// Jump to first session.
    pub fn select_first(&mut self) {
        self.selected_index = 0;
    }

    /// Jump to last session.
    pub fn select_last(&mut self, session_count: usize) {
        if session_count > 0 {
            self.selected_index = session_count - 1;
        }
    }

    /// Get selected session index, validated against session count.
    pub fn selected_session_index(&self, session_count: usize) -> Option<SessionIndex> {
        SessionIndex::new(self.selected_index, session_count)
    }

    /// Scroll offset for rendering.
    pub fn scroll_offset(&self) -> usize {
        self.scroll_offset
    }

    /// Update scroll offset to keep selection visible.
    pub fn adjust_scroll(&mut self, visible_rows: usize) {
        if self.selected_index < self.scroll_offset {
            self.scroll_offset = self.selected_index;
        } else if self.selected_index >= self.scroll_offset + visible_rows {
            self.scroll_offset = self.selected_index - visible_rows + 1;
        }
    }
}
```

---

## 6. SessionScrollStates (FR-010)

Per-session scroll position tracking with first-visit vs return-visit semantics.

```rust
// ===== src/state/session_scroll.rs (new file) =====

use std::collections::HashMap;
use crate::model::SessionId;

/// Per-session scroll state storage (FR-010).
///
/// Implements "preserve on return" semantics:
/// - Key absent = session never visited → first visit shows top (offset 0)
/// - Key present = session previously visited → return restores stored offset
///
/// # Cardinality
/// - States: 0 to S entries (S = session count)
/// - Each entry: SessionId → usize offset
/// - Precision: 1.0 (all states valid)
///
/// # Invariant
/// Offsets are only stored for sessions that have been visited and scrolled.
/// A session with offset 0 that was visited will have an entry; an unvisited
/// session will have no entry (distinguishing "visited at top" from "never visited").
pub type SessionScrollStates = HashMap<SessionId, ScrollState>;

/// Scroll state for a single session.
#[derive(Debug, Clone, Default)]
pub struct ScrollState {
    /// Vertical scroll offset (line number at top of viewport).
    pub offset: usize,
    // Future: could add horizontal offset, selected entry, etc.
}

impl ScrollState {
    pub fn new(offset: usize) -> Self {
        Self { offset }
    }
}

/// Extension trait for managing session scroll states.
pub trait SessionScrollExt {
    /// Get scroll offset for a session.
    /// Returns 0 for unvisited sessions (first-visit behavior).
    fn scroll_offset_for(&self, session_id: &SessionId) -> usize;

    /// Check if a session has been visited.
    fn is_session_visited(&self, session_id: &SessionId) -> bool;

    /// Save scroll state when leaving a session.
    fn save_scroll_state(&mut self, session_id: SessionId, offset: usize);
}

impl SessionScrollExt for SessionScrollStates {
    fn scroll_offset_for(&self, session_id: &SessionId) -> usize {
        self.get(session_id).map(|s| s.offset).unwrap_or(0)
    }

    fn is_session_visited(&self, session_id: &SessionId) -> bool {
        self.contains_key(session_id)
    }

    fn save_scroll_state(&mut self, session_id: SessionId, offset: usize) {
        self.insert(session_id, ScrollState::new(offset));
    }
}
```

---

## 7. AppState Additions

Fields added to AppState for session navigation.

```rust
// ===== src/state/app_state.rs (additions to struct) =====

/// AppState additions for session navigation (FR-002 to FR-012).
pub struct AppState {
    // ... existing fields ...

    /// Session list modal state.
    /// Contains visibility, selection, and scroll state.
    pub session_modal: SessionModalState,

    /// Which session is currently being viewed.
    /// Controls which session's content is displayed and whether live tailing is enabled.
    pub viewed_session: ViewedSession,

    /// Per-session scroll positions (FR-010).
    /// Tracks scroll offset for each visited session.
    /// Key absence = unvisited (first visit shows top).
    /// Key presence = visited (return restores offset).
    pub session_scroll_states: SessionScrollStates,
}
```

---

## Type Hierarchy Diagram

```
AppState
├── session_modal: SessionModalState
│   ├── visible: bool
│   ├── selected_index: usize
│   └── scroll_offset: usize
├── viewed_session: ViewedSession
│   ├── Latest                      # Follow newest session
│   └── Pinned(SessionIndex)        # View specific historical session
├── session_scroll_states: SessionScrollStates  # FR-010: per-session scroll
│   └── HashMap<SessionId, ScrollState>
│       └── ScrollState { offset: usize }
├── stats_filter: StatsFilter
│   ├── AllSessionsCombined         # All sessions, all agents
│   ├── Session(SessionId)          # One session, all its agents
│   ├── MainAgent(SessionId)        # One session's main agent
│   └── Subagent(AgentId)           # Specific subagent
└── log_view: LogViewState
    └── sessions: Vec<SessionViewState>
        ├── session_id: SessionId
        ├── main: ConversationViewState
        └── subagents: HashMap<AgentId, ConversationViewState>

Supporting Types:
├── SessionIndex (usize newtype, validated, 0-indexed)
├── SessionSummary { index, session_id, message_count, start_time, subagent_count }
├── ViewedSession { Latest | Pinned(SessionIndex) }
├── SessionScrollStates = HashMap<SessionId, ScrollState>
└── ScrollState { offset: usize }
```

---

## Cardinality Analysis

| Type | Valid States | Total Cardinality | Precision |
|------|--------------|-------------------|-----------|
| `SessionIndex` | [0, session_count) | usize::MAX | ~1.0 (validated) |
| `ViewedSession::Latest` | 1 | 1 | 1.0 |
| `ViewedSession::Pinned` | session_count | session_count | 1.0 |
| `ViewedSession` (sum) | 1 + session_count | 1 + session_count | 1.0 |
| `StatsFilter::AllSessionsCombined` | 1 | 1 | 1.0 |
| `StatsFilter::Session` | session_count | session_count | 1.0 |
| `StatsFilter::MainAgent` | session_count | session_count | 1.0 |
| `StatsFilter::Subagent` | subagent_count | subagent_count | 1.0 |
| `StatsFilter` (sum) | 1 + 2S + A | 1 + 2S + A | 1.0 |
| `SessionModalState` | 1 + S | 1 + S | ~1.0 |
| `SessionScrollStates` | 2^S | 2^S | 1.0 (all subsets valid) |
| `ScrollState` | usize | usize | 1.0 |

All types achieve precision ≈ 1.0 through:
- Smart constructors (`SessionIndex::new`)
- Sum types (`ViewedSession`, `StatsFilter`)
- Bounded selection (`SessionModalState`)

---

## Property-Based Testing Invariants

```rust
// Properties to test with proptest:

// 1. SessionIndex is always valid
// forall idx created via new(): idx.get() < session_count

// 2. ViewedSession.effective_index is always valid
// forall vs, count: vs.effective_index(count).is_some() implies vs.effective_index(count).unwrap().get() < count

// 3. ViewedSession.is_last is consistent
// forall vs where vs == Latest: vs.is_last(count) == true
// forall vs where vs == Pinned(idx): vs.is_last(count) == idx.is_last(count)

// 4. SessionModalState selection is bounded
// forall modal, count: modal.selected_index() < count after any navigation

// 5. Stats filter aggregation is exhaustive
// AllSessionsCombined.usage == sum(Session(s).usage for all s)
// Session(s).usage == MainAgent(s).usage + sum(Subagent(a).usage for a in s)

// 6. First-visit scroll behavior (FR-010)
// forall session_id not in session_scroll_states: scroll_offset_for(session_id) == 0
// forall session_id in session_scroll_states: scroll_offset_for(session_id) == stored_offset

// 7. Scroll state persistence on session switch
// Let old_offset = current scroll position, old_session = current session
// After switch: session_scroll_states.get(old_session) == Some(old_offset)
```

---

## Migration Guide

### Breaking Changes

1. **`StatsFilter::Global` → `StatsFilter::AllSessionsCombined`**
   - Search: `StatsFilter::Global`
   - Replace: `StatsFilter::AllSessionsCombined`

2. **`StatsFilter::MainAgent` → `StatsFilter::MainAgent(SessionId)`**
   - Search: `StatsFilter::MainAgent`
   - Replace: `StatsFilter::MainAgent(current_session_id.clone())`
   - Requires: Access to current/viewed session's ID

3. **New `AppState` fields**
   - Add `session_modal: SessionModalState::new()`
   - Add `viewed_session: ViewedSession::default()`
   - Add `session_scroll_states: SessionScrollStates::new()` (empty HashMap)

### Affected Files

- `src/model/stats.rs` - StatsFilter definition
- `src/state/app_state.rs` - AppState fields
- `src/state/session_scroll.rs` - **NEW**: SessionScrollStates, ScrollState, SessionScrollExt trait
- `src/state/mod.rs` - Export new session_scroll module
- `src/view/stats.rs` - Stats rendering
- `src/view/stats_multi_scope.rs` - Multi-scope stats
- `src/tests/stats_multi_scope_tests.rs` - Stats tests
- `src/tests/acceptance_stats_session_mismatch.rs` - Stats acceptance tests
