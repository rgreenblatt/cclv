# Contract: Session Modal

**Date**: 2025-01-09
**Status**: Design Complete
**Related**: [data-model.md](../data-model.md) | [spec.md](../spec.md)

This document specifies the session list modal widget behavior and layout.

---

## Visual Layout

```
┌─────────────────────────────────────────────────────────────┐
│                     Session List                            │
├─────────────────────────────────────────────────────────────┤
│   Session 1: 45 messages, 2 subagents (14:32)              │
│ > Session 2: 128 messages, 5 subagents (15:47)  [CURRENT]  │
│   Session 3: 23 messages, 0 subagents (16:15)              │
├─────────────────────────────────────────────────────────────┤
│ ↑/↓: Navigate  Enter: Select  Esc: Cancel  S: Close        │
└─────────────────────────────────────────────────────────────┘
```

### Layout Specifications

| Element | Value |
|---------|-------|
| Width | 60 columns (centered) |
| Height | min(session_count + 4, terminal_height - 4) |
| Title | "Session List" (centered, bold) |
| Footer | Keybinding hints |
| Selection indicator | `>` prefix + highlight style |
| Current session marker | `[CURRENT]` suffix |

---

## Row Format

Each session row displays:

```
{prefix} Session {N}: {M} messages, {S} subagents{time}  {marker}
```

Where:
- `{prefix}`: `>` if selected, ` ` otherwise
- `{N}`: Session number (1-indexed)
- `{M}`: Message count in main conversation
- `{S}`: Subagent count
- `{time}`: ` (HH:MM)` if start_time available, empty otherwise
- `{marker}`: `[CURRENT]` if this is the currently viewed session

### Examples

```
  Session 1: 45 messages, 2 subagents (14:32)
> Session 2: 128 messages, 5 subagents (15:47)  [CURRENT]
  Session 3: 23 messages, 0 subagents (16:15)
```

---

## Keyboard Bindings

| Key | Action | Notes |
|-----|--------|-------|
| `S` | Toggle modal | Opens if closed, closes if open |
| `↑` / `k` | Select previous | Wraps to last if at first |
| `↓` / `j` | Select next | Wraps to first if at last |
| `Enter` | Confirm selection | Closes modal, switches to selected session |
| `Esc` | Cancel | Closes modal without changing session |
| `Home` / `g` | Jump to first | Select first session |
| `End` / `G` | Jump to last | Select last session |
| `1`-`9` | Quick select | Jump to session N (if exists) |

### Key Priority

When modal is visible, it captures all keys before they reach the main view.

---

## State Transitions

```
Closed
  │
  ├── [S pressed] → Open (selected = current_session_index)
  │
Open
  │
  ├── [↑/k] → selection = max(0, selection - 1)
  ├── [↓/j] → selection = min(session_count - 1, selection + 1)
  ├── [Enter] → viewed_session = Pinned(selection); Close
  ├── [Esc] → Close (no change)
  └── [S] → Close (no change)
```

---

## Styling

| Element | Style |
|---------|-------|
| Modal background | Dark gray (Color::DarkGray) |
| Border | White, rounded corners |
| Title | Bold, cyan |
| Normal row | White on default |
| Selected row | Black on cyan, bold |
| Current marker | Yellow, italic |
| Footer | Dim gray |

---

## Scrolling Behavior

When session_count > visible_rows:

1. Keep selected row visible
2. Scroll to show context (1-2 rows above/below selection)
3. Show scroll indicators (`▲` / `▼`) when content extends beyond view

```
┌─────────────────────────────────────────────────────────────┐
│                     Session List                        ▲   │
├─────────────────────────────────────────────────────────────┤
│   Session 5: 45 messages, 2 subagents (14:32)              │
│ > Session 6: 128 messages, 5 subagents (15:47)  [CURRENT]  │
│   Session 7: 23 messages, 0 subagents (16:15)              │
├─────────────────────────────────────────────────────────────┤
│ ↑/↓: Navigate  Enter: Select  Esc: Cancel               ▼   │
└─────────────────────────────────────────────────────────────┘
```

---

## Accessibility

- Selection uses both color AND prefix marker (`>`) for visibility
- Current session uses both color AND text marker (`[CURRENT]`)
- Footer shows all available keybindings
- Focus is trapped in modal when open

---

## Edge Cases

| Case | Behavior |
|------|----------|
| 0 sessions | Modal shows "No sessions" message |
| 1 session | Modal shows single session, still allows closing |
| 100+ sessions | Scrollable list with scroll indicators |
| Session with 0 messages | Shows "0 messages" |
| Session without timestamp | Omits time field |

---

## Acceptance Criteria

1. **AC-MODAL-001**: Modal opens when `S` pressed, closes on `Esc`
2. **AC-MODAL-002**: Selection moves with `↑`/`↓` keys
3. **AC-MODAL-003**: `Enter` switches to selected session and closes
4. **AC-MODAL-004**: Current session is visually distinguished
5. **AC-MODAL-005**: Modal is centered and properly sized
6. **AC-MODAL-006**: Long lists scroll to keep selection visible
