# Investigation: Collapsed entries still cause jerky scroll

## Root Bead
ID: cclv-5ur.13
Status: in_progress
Branch: 002-view-state-layer

## Symptom
- Scroll gets stuck for 3-4 consecutive j presses before finally scrolling
- Unit test shows isolated stuck positions (max_stuck_run=1)
- TUI shows 3-4 consecutive stuck scrolls

Reproduction:
1. cargo run -- tests/fixtures/cc-session-log.jsonl
2. Navigate to collapsed entry
3. Press j repeatedly - observe 3-4 presses do nothing, then scroll jumps

## ROOT CAUSE IDENTIFIED

**H4 CONFIRMED**: Height calculator uses hardcoded `WRAP_WIDTH = 80` instead of actual viewport width.

### Technical Details
- `calculate_height()` signature: `fn(entry, expanded, wrap_mode) -> LineHeight` - **NO WIDTH PARAM**
- `LayoutParams.width` is available at call site but NOT passed
- Hardcoded at `src/view_state/layout.rs:157`: `const WRAP_WIDTH: usize = 80;`

### Impact Quantified
- 200-char line at width 80: 3 lines (ceil(200/80))
- 200-char line at width 211: 1 line (ceil(200/211))
- **Difference: 2 lines per such line**
- For entry with 10 such lines: 30 vs 10 lines = **200% error**

### Why This Causes Jerky Scroll
1. Height calculator thinks entry = N lines (using width 80)
2. Renderer shows entry = M lines (using actual width)
3. N > M typically (80 wraps more than 211)
4. Scroll increments by 1 line but `visible_range()` binary search uses wrong cumulative_y
5. Multiple scroll positions land in same "calculated" entry span
6. Result: 3-4 j presses show same content until offset crosses calculated boundary

## Active Worktrees
| Worktree | Branch | Subagent | Purpose | Status |
|----------|--------|----------|---------|--------|
| .worktrees/investigate-h4 | investigate/h4-width-mismatch | Exp-001 | Verify width mismatch | complete |

## Hypotheses

### H4: Width parameter not passed to height calculator [LEADING - CONFIRMED]
- Bead: cclv-5ur.13.4
- Theory: Height calculator uses hardcoded 80 instead of actual viewport width
- Evidence: E1 (cclv-5ur.13.5), E2 (cclv-5ur.13.7)
- **CONFIRMED** by Exp-001

### H1: Height calculator returns wrong height [PARKED - Subsumed by H4]
- Bead: cclv-5ur.13.1
- Original theory: Constants wrong (SUMMARY_LINES, etc.)
- Finding: Constants are correct (3+1+1=5 lines), but width input is wrong
- **PARKED**: Subsumed by H4

### H2: Scroll resolve step causes position drift [ELIMINATED]
- Bead: cclv-5ur.13.2 (closed)
- Original theory: resolve() returns different offset
- Finding: resolve() works correctly; it uses cumulative_y which was computed with wrong width
- **ELIMINATED**: Downstream effect of H4

### H3: visible_range() uses stale cumulative_y [ELIMINATED]
- Bead: cclv-5ur.13.3 (closed)
- Original theory: cumulative_y not updated
- Finding: cumulative_y is updated correctly; it's just computed with wrong width
- **ELIMINATED**: Symptom of H4, not independent cause

## Evidence Log
| ID | Bead | Source | Finding | Supports | Refutes |
|----|------|--------|---------|----------|---------|
| E1 | cclv-5ur.13.5 | Analyzer | WRAP_WIDTH=80 hardcoded at layout.rs:157 | H4 | H1 |
| E2 | cclv-5ur.13.7 | Exp-001 | calculate_height() signature lacks width param; LayoutParams.width not passed | H4 | H2, H3 |

## Dead Ends
- H1: Collapsed height constants (SUMMARY_LINES, separator) are correct. Issue is width, not constants.
- H2: Resolve function works correctly. Uses cumulative_y which has wrong values due to H4.
- H3: cumulative_y is computed at correct time. Values wrong because of H4 (wrong width input).

## Fix Required
1. Add `width: u16` parameter to `calculate_height()` function signature
2. Pass `params.width` from `recompute_layout()` to height calculator
3. Use passed width instead of hardcoded 80 in `count_text_lines()`

## Next Steps
- Create fix implementation bead (outside investigation scope)
- Create regression test bead (outside investigation scope)
- Mark investigation complete: `bd update cclv-5ur.13 --add-label investigation:complete`
