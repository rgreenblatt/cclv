# Quickstart: Session Navigation

**Date**: 2025-01-09
**Status**: Design Complete
**Related**: [plan.md](./plan.md) | [spec.md](./spec.md)

This guide shows how to implement session navigation features step-by-step.

---

## Prerequisites

- Rust 1.83+ installed
- cclv codebase checked out
- On branch `003-session-navigation`
- `nix develop` shell active

---

## Phase 1: Add Core Types

### Step 1.1: Add SessionIndex

Add to `src/view_state/types.rs`:

```rust
/// Validated index into session list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SessionIndex(usize);

impl SessionIndex {
    pub fn new(index: usize, session_count: usize) -> Option<Self> {
        if index < session_count { Some(Self(index)) } else { None }
    }
    pub fn get(&self) -> usize { self.0 }
    pub fn display(&self) -> usize { self.0 + 1 }
    pub fn is_last(&self, session_count: usize) -> bool { self.0 + 1 == session_count }
}
```

### Step 1.2: Add ViewedSession

Create `src/state/viewed_session.rs`:

```rust
use crate::view_state::types::SessionIndex;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ViewedSession {
    #[default]
    Latest,
    Pinned(SessionIndex),
}

impl ViewedSession {
    pub fn is_last(&self, session_count: usize) -> bool {
        match self {
            ViewedSession::Latest => true,
            ViewedSession::Pinned(idx) => idx.is_last(session_count),
        }
    }
}
```

### Step 1.3: Update AppState

Add to `src/state/app_state.rs`:

```rust
use crate::state::viewed_session::ViewedSession;
use crate::state::session_modal::SessionModalState;

pub struct AppState {
    // ... existing fields ...
    pub session_modal: SessionModalState,
    pub viewed_session: ViewedSession,
}
```

---

## Phase 2: Session Modal Widget

### Step 2.1: Create SessionModalState

Create `src/state/session_modal.rs`:

```rust
#[derive(Debug, Clone, Default)]
pub struct SessionModalState {
    visible: bool,
    selected_index: usize,
}

impl SessionModalState {
    pub fn is_visible(&self) -> bool { self.visible }
    pub fn toggle(&mut self, current: usize) {
        if self.visible { self.close(); } else { self.open(current); }
    }
    pub fn open(&mut self, current: usize) {
        self.visible = true;
        self.selected_index = current;
    }
    pub fn close(&mut self) { self.visible = false; }
    pub fn select_next(&mut self, count: usize) {
        if count > 0 { self.selected_index = (self.selected_index + 1).min(count - 1); }
    }
    pub fn select_prev(&mut self) {
        self.selected_index = self.selected_index.saturating_sub(1);
    }
}
```

### Step 2.2: Create Modal Widget

Create `src/view/session_modal.rs`:

```rust
use ratatui::{prelude::*, widgets::*};
use crate::state::AppState;

pub fn render_session_modal(frame: &mut Frame, state: &AppState) {
    if !state.session_modal.is_visible() { return; }

    let area = centered_rect(60, 50, frame.area());
    frame.render_widget(Clear, area);

    let sessions: Vec<ListItem> = state.log_view()
        .sessions()
        .enumerate()
        .map(|(i, s)| {
            let text = format!("Session {}: {} messages", i + 1, s.main().len());
            ListItem::new(text)
        })
        .collect();

    let list = List::new(sessions)
        .block(Block::bordered().title("Session List"))
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED));

    let mut list_state = ListState::default()
        .with_selected(Some(state.session_modal.selected_index()));

    frame.render_stateful_widget(list, area, &mut list_state);
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::vertical([
        Constraint::Percentage((100 - percent_y) / 2),
        Constraint::Percentage(percent_y),
        Constraint::Percentage((100 - percent_y) / 2),
    ]).split(r);

    Layout::horizontal([
        Constraint::Percentage((100 - percent_x) / 2),
        Constraint::Percentage(percent_x),
        Constraint::Percentage((100 - percent_x) / 2),
    ]).split(popup_layout[1])[1]
}
```

---

## Phase 3: Keyboard Handling

### Step 3.1: Create Modal Handler

Create `src/state/session_modal_handler.rs`:

```rust
use crossterm::event::{KeyCode, KeyEvent};
use crate::state::AppState;
use crate::view_state::types::SessionIndex;

pub fn handle_session_modal_key(state: &mut AppState, key: KeyEvent) -> bool {
    if !state.session_modal.is_visible() { return false; }

    let session_count = state.log_view().session_count();

    match key.code {
        KeyCode::Esc | KeyCode::Char('s') | KeyCode::Char('S') => {
            state.session_modal.close();
        }
        KeyCode::Up | KeyCode::Char('k') => {
            state.session_modal.select_prev();
        }
        KeyCode::Down | KeyCode::Char('j') => {
            state.session_modal.select_next(session_count);
        }
        KeyCode::Enter => {
            if let Some(idx) = SessionIndex::new(
                state.session_modal.selected_index(),
                session_count
            ) {
                state.viewed_session = ViewedSession::Pinned(idx);
            }
            state.session_modal.close();
        }
        _ => return false,
    }
    true
}
```

### Step 3.2: Integrate Handler

In main event loop, add before other key handlers:

```rust
if handle_session_modal_key(&mut state, key) {
    continue;
}
```

---

## Phase 4: Stats Filter Update

### Step 4.1: Update StatsFilter Enum

Modify `src/model/stats.rs`:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StatsFilter {
    AllSessionsCombined,           // Was: Global
    Session(SessionId),            // NEW
    MainAgent(SessionId),          // Was: MainAgent (no param)
    Subagent(AgentId),             // Unchanged
}
```

### Step 4.2: Update filtered_usage

```rust
impl SessionStats {
    pub fn filtered_usage(&self, filter: &StatsFilter) -> TokenUsage {
        match filter {
            StatsFilter::AllSessionsCombined => self.total_usage,
            StatsFilter::Session(session_id) => {
                // Sum main + all subagents for this session
                self.session_usage.get(session_id).copied().unwrap_or_default()
            }
            StatsFilter::MainAgent(session_id) => {
                self.main_agent_usage_by_session.get(session_id)
                    .copied().unwrap_or_default()
            }
            StatsFilter::Subagent(agent_id) => {
                self.subagent_usage.get(agent_id)
                    .copied().unwrap_or_default()
            }
        }
    }
}
```

---

## Phase 5: Live Tailing Gate

### Step 5.1: Add Tailing Check

Add to `src/state/app_state.rs`:

```rust
impl AppState {
    /// Check if live tailing should be active.
    pub fn is_tailing_enabled(&self) -> bool {
        self.auto_scroll && self.viewed_session.is_last(self.log_view().session_count())
    }
}
```

### Step 5.2: Update Scroll Handler

In `src/state/scroll_handler.rs`, gate auto-scroll:

```rust
if state.is_tailing_enabled() {
    // Perform auto-scroll to bottom
}
```

---

## Testing Commands

```bash
# Run all tests
cargo test

# Run session navigation tests only
cargo test session

# Run with verbose output
cargo test -- --nocapture

# Review snapshots
cargo insta review
```

---

## Checklist

- [ ] `SessionIndex` type added
- [ ] `ViewedSession` enum added
- [ ] `SessionModalState` added
- [ ] Modal widget renders
- [ ] Keyboard handler captures modal keys
- [ ] `S` toggles modal
- [ ] `Enter` switches session
- [ ] `StatsFilter` updated
- [ ] Stats display correct per filter
- [ ] Live tailing disabled on historical sessions
- [ ] All tests pass
- [ ] All warnings fixed
