# Investigation: Tab switching does not change conversation pane content

## Root Bead
ID: cclv-5ur.40
Status: in_progress
Branch: 002-view-state-layer

## Symptom
When pressing Tab or number keys (1-9) to switch tabs:
1. Status bar title updates correctly to show selected agent ✓
2. Status bar model stays as main agent's model ✗
3. Pane title stays "Main Agent (N entries)" ✗
4. Pane content stays as Main Agent conversation ✗

Reproduction: `cargo run --release -- tests/fixtures/subagent_tab_repro.jsonl`, press '2'

## Root Causes Identified (3 bugs)

### Bug 1: Pane Title (H2 - CONFIRMED)
**Location**: `src/view/layout.rs:461`
**Issue**: `render_header()` uses `match state.focus` instead of `state.selected_tab`
**Code**:
```rust
let (agent_label, conversation_view) = match state.focus {
    FocusPane::Subagent => { ... }
    _ => ("Main Agent".to_string(), state.session_view().main()),  // WRONG
};
```
**Fix**: Use `selected_tab` instead of `focus` to determine agent_label

### Bug 2: Model Display (H3 - CONFIRMED)
**Location**: `src/view_state/session.rs:74-79`
**Issue**: Subagents always created with `model: None`
**Code**:
```rust
let view_state = ConversationViewState::new(
    Some(id.clone()),
    None,  // <-- ALWAYS None for subagents
    entries,
    ...
);
```
**Additional Issue**: No code extracts model from Task tool parameters
**Fix**: Either extract model from Task tool call, or inherit from parent agent

### Bug 3: Content Not Rendering (H5 - CONFIRMED - PRIMARY BUG)
**Location**: `src/view_state/session.rs`
**Issue**: Lazy initialization broken
- `add_subagent_entry()` adds to `pending_subagent_entries` HashMap
- `get_subagent()` (read-only, line 109) only checks `subagents` HashMap, NOT pending
- `subagent()` (mutable, line 70) triggers lazy init but can't be used during render (requires &mut self)
- Rendering calls `get_subagent()` → returns None → nothing renders → previous frame persists

**Code Flow**:
```rust
// Entry routing (works):
fn add_subagent_entry(&mut self, agent_id: AgentId, entry: ConversationEntry) {
    if let Some(view_state) = self.subagents.get_mut(&agent_id) {
        view_state.append(vec![entry]);
    } else {
        self.pending_subagent_entries.entry(agent_id).or_default().push(entry);  // Goes here!
    }
}

// Rendering (broken):
fn get_subagent(&self, id: &AgentId) -> Option<&ConversationViewState> {
    self.subagents.get(id)  // Returns None! Doesn't check pending
}
```

**Fix Options**:
A) Initialize eagerly in `add_subagent_entry()` per spec FR-073
B) Have `get_subagent()` check pending and return constructed view (but would need interior mutability)
C) Initialize all pending subagents before render pass

**Recommended Fix**: Option A - fulfill FR-073 by initializing eagerly in `add_subagent_entry()`

## Hypotheses Summary

### H1: ConversationView hardcoded to main [ELIMINATED]
- Bead: cclv-5ur.40.1
- Evidence E3 shows layout.rs:263-297 correctly uses selected_tab
- Closed: Code is correct, bug is elsewhere

### H2: Tab selection not propagated to header [LEADING - CONFIRMED]
- Bead: cclv-5ur.40.2
- Evidence E1 confirms render_header() uses focus instead of selected_tab
- **FIX REQUIRED**

### H3: Model info not per-conversation [LEADING - CONFIRMED]
- Bead: cclv-5ur.40.3
- Evidence E2 confirms subagents created with model: None
- **FIX REQUIRED**

### H4: Session reference inconsistency [ACTIVE]
- Bead: cclv-5ur.40.4
- Evidence E4 shows current_session() vs session_view() mismatch
- Affects multi-session logs, not primary single-session bug
- **SEPARATE BUG - lower priority**

### H5: Lazy init broken - get_subagent() misses pending [LEADING - CONFIRMED]
- Bead: cclv-5ur.40.10
- Evidence E5 (cclv-5ur.40.9) confirms get_subagent() doesn't check pending
- **PRIMARY ROOT CAUSE - FIX REQUIRED**

## Evidence Log
| ID | Bead | Finding | Supports | Refutes |
|----|------|---------|----------|---------|
| E1 | 5ur.40.5 | render_header uses focus not selected_tab | H2 | H1 |
| E2 | 5ur.40.6 | Subagents created with model: None | H3 | - |
| E3 | 5ur.40.7 | Content rendering uses selected_tab correctly | - | H1 |
| E4 | 5ur.40.8 | Session ref mismatch (current_session vs session_view) | H4 | - |
| E5 | 5ur.40.9 | get_subagent read-only, doesn't trigger lazy init | H5 | - |

## Dead Ends
- H1: Ruled out by E3. Rendering code is correct; issue is data layer.

## Recommended Fix Order
1. **H5 (content)**: Change `add_subagent_entry()` to initialize eagerly (spec FR-073)
2. **H2 (title)**: Change render_header() to use selected_tab instead of focus
3. **H3 (model)**: Add model extraction from Task tool or inheritance

## Files to Modify
1. `src/view_state/session.rs` - add_subagent_entry() eager init (H5)
2. `src/view/layout.rs` - render_header() use selected_tab (H2)
3. `src/parser/mod.rs` or `src/view_state/session.rs` - model extraction (H3)
