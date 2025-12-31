# Property Testing for Bug Analysis

Write property tests to analyze bugs, find minimal reproductions, and create formal specifications of invariant violations. This skill is for **analysis and verification only** - not for implementing fixes.

## When to Use This Skill

Use when:
- Analyzing a bug to find minimal reproduction cases
- Formally specifying invariants that a bug violates
- Verifying state machine properties (scroll, navigation, undo/redo)
- Testing round-trip properties (parse/serialize, encode/decode)
- Creating reproducible bug documentation for handoff

## Quick Reference

```rust
proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    #[ignore = "BEAD-ID: description of bug"]
    fn property_name(
        state in arb_state(),
        operations in arb_operation_sequence(10),
    ) {
        // Setup
        // Execute operations
        // Assert invariant
        prop_assert!(invariant_holds, "Bug: invariant violated");
    }
}
```

## Core Principle: Simulate Actual Behavior

**Critical insight**: Test helpers often implement *correct* behavior. To catch bugs, simulate *actual* buggy behavior.

### Example: Scroll Overshoot Bug

The bug: Handler creates `AtLine(offset + 1)` without clamping to max_offset.

```rust
// WRONG: Test helper with correct clamping (won't catch bug)
fn execute_scroll(state: &mut State, direction: Direction) {
    let new_offset = match direction {
        Down => (offset + 1).min(max_offset),  // Clamped - correct!
        Up => offset.saturating_sub(1),
    };
    state.set_scroll(new_offset);
}

// RIGHT: Simulate actual handler behavior (catches bug)
fn simulate_handler_scroll_down(state: &mut State) {
    match state.scroll() {
        ScrollPosition::Bottom => { /* stays Bottom */ }
        ScrollPosition::AtLine(offset) => {
            // BUG: No clamping - matches actual handler
            let new_offset = offset.saturating_add(1);
            state.set_scroll(AtLine(new_offset));
        }
        _ => { /* resolve then add */ }
    }
}
```

## Property Formulation

### Pattern: State Transition Invariants

```rust
// Property: Every scroll-up from non-top should decrease resolved offset
fn scroll_up_always_effective(state, operations) {
    for op in operations {
        let before = state.scroll().resolve();
        apply(op);
        let after = state.scroll().resolve();

        if op == ScrollUp && before > 0 {
            prop_assert!(after < before, "scroll up had no effect");
        }
    }
}
```

### Pattern: Round-Trip Properties

```rust
// Property: parse(serialize(x)) == x
fn roundtrip(value in arb_value()) {
    let serialized = serialize(&value);
    let parsed = parse(&serialized)?;
    prop_assert_eq!(value, parsed);
}
```

### Pattern: Idempotence

```rust
// Property: operation applied twice equals applied once
fn idempotent(state in arb_state()) {
    let once = apply(state.clone());
    let twice = apply(apply(state));
    prop_assert_eq!(once, twice);
}
```

### Pattern: Commutativity

```rust
// Property: order doesn't matter (where applicable)
fn commutative(state, op1 in arb_op(), op2 in arb_op()) {
    let result_12 = apply(apply(state.clone(), op1), op2);
    let result_21 = apply(apply(state, op2), op1);
    prop_assert_eq!(result_12, result_21);
}
```

## Strategy Design

### Reuse Production Types

```rust
// Build on real types with real layout calculators
fn arb_conversation_view_state() -> impl Strategy<Value = ConversationViewState> {
    (arb_entry_list(20), arb_wrap_mode()).prop_map(|(entries, wrap)| {
        let mut state = ConversationViewState::new(entries);
        state.relayout(LayoutParams::new(80, wrap));  // Real production code!
        state
    })
}
```

### Generate Operation Sequences

```rust
fn arb_scroll_sequence(max_len: usize) -> impl Strategy<Value = Vec<ScrollOp>> {
    prop::collection::vec(
        prop_oneof![
            Just(ScrollOp::Up),
            Just(ScrollOp::Down),
            Just(ScrollOp::PageUp),
            Just(ScrollOp::PageDown),
            Just(ScrollOp::Top),
            Just(ScrollOp::Bottom),
        ],
        1..=max_len,
    )
}
```

### Constrain to Valid States

```rust
// Skip cases that don't apply
fn property(state in arb_state()) {
    // Skip if precondition not met
    if state.total_height() <= viewport.height {
        return Ok(());  // Can't scroll, skip
    }

    // ... test the property
}
```

## Bug Reproduction Workflow

### Creating an Ignored Failing Test

1. Write property test that catches the bug
2. Add `#[ignore = "BEAD-ID: bug description"]`
3. Run with `--include-ignored` to verify it fails
4. Commit the failing test (keeps build green)

```rust
#[test]
#[ignore = "cclv-5ur.77: scroll k absorbed after overshoot"]
fn scroll_up_effective_after_overshoot() {
    // ... test that fails due to bug
}
```

This creates a **documented, reproducible specification** of the bug without blocking CI.

### Regression File Management

Proptest saves minimal failing cases:

```
proptest-regressions/
└── tests/
    └── scroll_properties.txt
```

**Always commit regression files** - they ensure the exact failing case is re-tested.

```bash
git add proptest-regressions/
git commit -m "test: add proptest regression for scroll bug"
```

## Debugging Property Test Failures

### Read the Shrunk Input

Proptest shrinks to minimal failing case:

```
minimal failing input: state = {...}, first_overshoot = 1, second_overshoot = 2
```

This is your minimal reproduction!

### Add Diagnostic Output

```rust
prop_assert!(
    condition,
    "Bug description\n\
     State before: {:?}\n\
     State after: {:?}\n\
     Expected: {}",
    before, after, expected
);
```

## Configuration

```rust
proptest! {
    // 100 cases is good default balance
    #![proptest_config(ProptestConfig::with_cases(100))]

    // For thorough testing (CI)
    #![proptest_config(ProptestConfig::with_cases(1000))]

    // For quick iteration (development)
    #![proptest_config(ProptestConfig::with_cases(10))]
}
```

## Anti-Patterns

### Testing Implementation, Not Behavior

```rust
// BAD: Tests internal state structure
prop_assert_eq!(state.internal_offset, expected);

// GOOD: Tests observable behavior
let visible = render(&state);
prop_assert!(visible.contains("expected content"));
```

### Overly Specific Properties

```rust
// BAD: Too specific, breaks on valid changes
prop_assert_eq!(output, "exact expected string");

// GOOD: Tests the invariant
prop_assert!(output.contains("required part"));
prop_assert!(output.len() <= max_len);
```

### Ignoring Edge Cases

```rust
// BAD: Skips too many cases
if anything_interesting() { return Ok(()); }

// GOOD: Explicit about what's skipped and why
if content_fits_in_viewport() {
    return Ok(());  // Scrolling not applicable
}
```

## Checklist

Before committing a bug reproduction property test:

- [ ] Property tests actual buggy behavior, not idealized behavior
- [ ] Property (invariant violated) is clearly stated in docstring
- [ ] Uses `#[ignore = "BEAD-ID: description"]` to document the bug
- [ ] Proptest regression file committed for reproducibility
- [ ] Minimal failing case documented in bead/issue
- [ ] Test runs in reasonable time (< 10s for 100 cases)

## Related

- `typed-domain-modeling` for designing testable invariants
- `root-cause-analysis` for investigating why invariant is violated
- `beads-project-tracking` for documenting bugs with reproducers
