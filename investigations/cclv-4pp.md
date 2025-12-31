# Investigation: scroll_line_down 102ms performance

## Root Bead
ID: cclv-4pp
Status: in_progress
Branch: 002-view-state-layer

## Symptom
scroll_line_down benchmark takes ~102ms, needs to be ~1ms (100x faster).
Reproduction: `cargo bench --profile release --features="bench-internals" --bench scroll_benchmark`

## Root Cause IDENTIFIED

**Primary bottleneck: AppState deep clone on every scroll (30%+ overhead)**

Location: `src/view/mod.rs:474`
```rust
scroll_handler::handle_scroll_action(self.app_state.clone(), action, viewport);
```

The comment "This is safe because AppState is cheap to clone (Rc internals)" is **FALSE**.
AppState uses `#[derive(Clone)]` which does DEEP COPIES of:
- Vec<SessionViewState>
- Vec<ConversationViewState>
- Vec<EntryView> (31k entries with pre-rendered `Vec<Line>`)

## Flamegraph Analysis

**IMPORTANT**: The flamegraph captures the ENTIRE benchmark process, not just the measured closure.
This includes setup (fixture load, state clones for iter_batched) and measurement.

### What's in the MEASURED path (the actual bottleneck):

| Category | % | Notes |
|----------|---|-------|
| **State cloning** | **30%** | `app_state.clone()` at mod.rs:474 - IN HOT PATH |
| Memory mgmt | 10% | Alloc/dealloc from clone operations |
| Text cloning | 5% | Span/Cow clones from Vec<Line> copies |

### What's in SETUP only (not the bottleneck):

| Category | % | Notes |
|----------|---|-------|
| Syntax highlighting | 13% | One-time cost in `compute_entry_lines()` |
| | | Cached in `EntryView.rendered_lines: Vec<Line>` |
| | | Called only in `new()` and `relayout()` |
| Rendering | 3% | Draws pre-computed lines to buffer |

The syntect/tui_markdown overhead (13%) is from:
1. Initial fixture load (`load_fixture()`) - runs once
2. Setup phase of iter_batched - clones the already-highlighted lines

**Syntax highlighting is NOT re-computed on scroll** - it's cached in EntryView.

## Hypotheses

### H4: AppState deep clone on every scroll [LEADING - CONFIRMED]
- Bead: cclv-4pp.6
- Evidence: E1 (flamegraph shows 30%+ in clone operations in measured path)
- Fix: Change handle_scroll_action to take `&mut AppState` instead of owned

### H1: Vec allocation in visible_range() [ELIMINATED]
- Bead: cclv-4pp.1 (closed)
- Eliminated by: E1 - flamegraph shows no significant Vec::collect overhead

### H2: HeightIndex::prefix_sum() is expensive [ELIMINATED]
- Bead: cclv-4pp.2 (closed)
- Eliminated by: E1 - no Fenwick tree operations visible in profile

### H3: Rendering overhead is the actual bottleneck [ELIMINATED]
- Bead: cclv-4pp.3
- Eliminated: Rendering is only ~3%, syntax highlighting is cached (not re-run on scroll)
- The 13% syntect is setup cost, not scroll cost

## Evidence Log

| ID | Source | Finding | Supports | Refutes |
|----|--------|---------|----------|---------|
| E1 | cclv-4pp.5 | Flamegraph: ConversationViewState::clone 15%, to_vec_in 15%, drop 10% | H4 | H1, H2, H3 |

## Dead Ends
- H1: Vec allocation - NOT the bottleneck (E1)
- H2: Fenwick tree - NOT visible in profile (E1)
- H3: Rendering/highlighting - NOT in hot path, cached in EntryView (E1)

## Recommended Fix

**Single fix needed: Eliminate state cloning**

1. Change `handle_scroll_action(state: AppState)` to `handle_scroll_action(state: &mut AppState)`
2. Update call site at mod.rs:474 to pass `&mut self.app_state`
3. Remove the false comment about "Rc internals"

Expected improvement: ~30% reduction (102ms → ~70ms)

Further optimization to reach <2ms target would require:
- Investigating the remaining 70ms
- Possible architectural changes (Rc for expensive data, incremental updates)

## Investigation Artifacts
- `cclv-4pp-flamegraph.svg` - Generated with debug symbols
- Cargo.toml `[profile.bench]` added for future profiling

## Next Steps
1. Implement the &mut AppState fix
2. Re-run benchmark to measure improvement
3. Profile again if still >2ms to find next bottleneck
