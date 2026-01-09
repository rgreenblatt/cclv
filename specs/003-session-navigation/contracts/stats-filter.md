# Contract: Stats Filter

**Date**: 2025-01-09
**Status**: Design Complete
**Related**: [data-model.md](../data-model.md) | [spec.md](../spec.md)

This document specifies the statistics aggregation levels and filter cycling behavior.

---

## Aggregation Levels (FR-008)

Four levels of statistics aggregation, from broadest to narrowest scope:

| Level | Enum Variant | Scope | Description |
|-------|--------------|-------|-------------|
| 1 | `AllSessionsCombined` | All sessions, all agents | Total usage across entire log file |
| 2 | `Session(SessionId)` | One session, all agents | Per-session total (main + subagents) |
| 3 | `MainAgent(SessionId)` | One session, main only | Main agent stats for specific session |
| 4 | `Subagent(AgentId)` | One subagent | Individual subagent stats |

---

## Filter Display Labels

### Full Labels (Stats Panel Title)

| Filter | Label |
|--------|-------|
| `AllSessionsCombined` | "Statistics: All Sessions" |
| `Session(id)` | "Statistics: Session {N}" |
| `MainAgent(id)` | "Statistics: Main Agent (Session {N})" |
| `Subagent(id)` | "Statistics: Subagent {id}" |

### Short Labels (Status Bar)

| Filter | Label |
|--------|-------|
| `AllSessionsCombined` | "[All]" |
| `Session(_)` | "[Sess]" |
| `MainAgent(_)` | "[Main]" |
| `Subagent(_)` | "[Sub]" |

---

## Filter Cycling Behavior

When user presses the stats filter cycle key (default: `Tab` in stats panel):

### Cycle Order

```
AllSessionsCombined → Session(current) → MainAgent(current) → Subagent(first) → ... → Subagent(last) → AllSessionsCombined
```

### Cycle Logic

```rust
fn cycle_stats_filter(current: &StatsFilter, session_id: &SessionId, subagent_ids: &[AgentId]) -> StatsFilter {
    match current {
        StatsFilter::AllSessionsCombined => {
            StatsFilter::Session(session_id.clone())
        }
        StatsFilter::Session(_) => {
            StatsFilter::MainAgent(session_id.clone())
        }
        StatsFilter::MainAgent(_) => {
            if let Some(first) = subagent_ids.first() {
                StatsFilter::Subagent(first.clone())
            } else {
                StatsFilter::AllSessionsCombined
            }
        }
        StatsFilter::Subagent(agent_id) => {
            // Find next subagent, or wrap to AllSessionsCombined
            let idx = subagent_ids.iter().position(|id| id == agent_id);
            match idx {
                Some(i) if i + 1 < subagent_ids.len() => {
                    StatsFilter::Subagent(subagent_ids[i + 1].clone())
                }
                _ => StatsFilter::AllSessionsCombined,
            }
        }
    }
}
```

---

## Aggregation Semantics

### AllSessionsCombined

- **Input tokens**: Sum of all sessions' input tokens
- **Output tokens**: Sum of all sessions' output tokens
- **Cache tokens**: Sum of all sessions' cache tokens
- **Tool counts**: Sum of all sessions' tool invocations
- **Entry count**: Total entries across all sessions

### Session(SessionId)

- **Input tokens**: Session's main + all subagents' input
- **Output tokens**: Session's main + all subagents' output
- **Cache tokens**: Session's cache tokens (main only, typically)
- **Tool counts**: All tool invocations in session
- **Entry count**: All entries in session

### MainAgent(SessionId)

- **Input tokens**: Main agent's input (excludes subagents)
- **Output tokens**: Main agent's output
- **Cache tokens**: Main agent's cache tokens
- **Tool counts**: Main agent's tool invocations only
- **Entry count**: Main agent entries only

### Subagent(AgentId)

- **Input tokens**: Specific subagent's input
- **Output tokens**: Specific subagent's output
- **Cache tokens**: Typically 0 (subagents don't use cache)
- **Tool counts**: Specific subagent's tool invocations
- **Entry count**: Specific subagent's entries

---

## Invariants

### Summation Properties

```rust
// Property 1: AllSessionsCombined equals sum of all Sessions
let all_combined = stats.filtered_usage(&StatsFilter::AllSessionsCombined);
let session_sum: TokenUsage = session_ids.iter()
    .map(|id| stats.filtered_usage(&StatsFilter::Session(id.clone())))
    .sum();
assert_eq!(all_combined, session_sum);

// Property 2: Session equals MainAgent + sum of Subagents
let session = stats.filtered_usage(&StatsFilter::Session(session_id.clone()));
let main = stats.filtered_usage(&StatsFilter::MainAgent(session_id.clone()));
let subagents_sum: TokenUsage = subagent_ids.iter()
    .map(|id| stats.filtered_usage(&StatsFilter::Subagent(id.clone())))
    .sum();
assert_eq!(session, main + subagents_sum);
```

---

## UI Integration

### Stats Panel Header

```
┌──────────────────────────────────────────────────────────────┐
│ Statistics: Session 2                              [Tab: →]  │
├──────────────────────────────────────────────────────────────┤
│ Input tokens:    45,230                                      │
│ Output tokens:   12,456                                      │
│ ...                                                          │
```

### Status Bar

```
│ LIVE │ Wrap │ Session 2/3 │ Stats: [Sess] │
```

---

## Session Context

The stats filter now requires session context:

- `MainAgent(SessionId)` - which session's main agent?
- `Session(SessionId)` - which session?

When session changes (via session modal):
1. Update `viewed_session`
2. Update `stats_filter` to use new session ID if filter is session-scoped

```rust
fn on_session_change(&mut self, new_session_id: SessionId) {
    // Update session-scoped filters to use new session
    self.stats_filter = match &self.stats_filter {
        StatsFilter::AllSessionsCombined => StatsFilter::AllSessionsCombined,
        StatsFilter::Session(_) => StatsFilter::Session(new_session_id.clone()),
        StatsFilter::MainAgent(_) => StatsFilter::MainAgent(new_session_id.clone()),
        StatsFilter::Subagent(id) => StatsFilter::Subagent(id.clone()), // Keep same if exists
    };
}
```

---

## Backwards Compatibility

### Breaking Changes

| Old | New | Migration |
|-----|-----|-----------|
| `StatsFilter::Global` | `StatsFilter::AllSessionsCombined` | Rename |
| `StatsFilter::MainAgent` | `StatsFilter::MainAgent(SessionId)` | Add session param |

### Example Migration

```rust
// Before
let filter = StatsFilter::MainAgent;
let usage = stats.filtered_usage(&filter);

// After
let session_id = state.viewed_session_id();
let filter = StatsFilter::MainAgent(session_id);
let usage = stats.filtered_usage(&filter);
```

---

## Acceptance Criteria

1. **AC-STATS-001**: `Tab` cycles through all filter levels
2. **AC-STATS-002**: AllSessionsCombined shows cross-session totals
3. **AC-STATS-003**: Session shows per-session totals (main + subagents)
4. **AC-STATS-004**: MainAgent shows only main agent for specified session
5. **AC-STATS-005**: Subagent shows individual subagent stats
6. **AC-STATS-006**: Summation invariants hold (Property 1 and 2)
7. **AC-STATS-007**: Filter updates when session changes
