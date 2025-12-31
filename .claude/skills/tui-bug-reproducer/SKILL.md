---
name: tui-bug-reproducer
description: Reproduce TUI bugs with visual verification and snapshot tests. Creates minimal fixtures, failing tests, and tracked beads. Use when user reports a TUI rendering bug and provides a tmux pane number for visual verification.
---

# TUI Bug Reproducer

Systematically reproduce TUI bugs with visual verification, snapshot tests, and issue tracking.

## When to Use This Skill

**ONLY use when ALL conditions are met:**
1. User reports a TUI rendering/display bug
2. User provides a tmux pane number for visual verification
3. Bug involves visual output (rendering, wrapping, layout, etc.)

**Requires**: tmux skill (invoked first with user's pane number)

**DO NOT use** for:
- Logic bugs without visual component
- Bugs that don't need snapshot tests
- When no tmux pane is available

## CRITICAL: Reproduce Only, Do Not Investigate

**Your ONLY job is to create a minimal failing test. Stop there.**

### Guardrails

1. **Stay black-box**: Treat the TUI as an opaque system. You observe inputs and outputs, nothing more.

2. **No source diving**: Do NOT read implementation code to understand *why* the bug happens. That's investigation, not reproduction.

3. **No root cause analysis**: If you catch yourself thinking "this is probably caused by...", STOP. You're off track.

4. **Acceptance test mindset**: Write tests like a user would describe the bug:
   - "When I scroll down, the screen shows blank lines" ✓
   - "The scroll_offset calculation has an off-by-one error" ✗

5. **Done means done**: After the test fails for the right reason and the bead is created, you are FINISHED. Do not "quickly check" the cause.

### What You Produce

| Artifact | Purpose |
|----------|---------|
| Minimal fixture | Smallest input that triggers bug |
| Failing snapshot test | Documents buggy behavior |
| Bead with repro steps | Tracks the issue |

### What You Do NOT Produce

- Root cause analysis
- Fix suggestions
- "Investigation hints" beyond file/function names from stack traces
- Any changes to non-test code

### Why This Matters

Investigation is a separate skill with different constraints. Mixing reproduction with investigation leads to:
- Premature fixes that don't address root cause
- Tests that pass for wrong reasons
- Wasted context chasing theories

**Create the reproducer. File the bead. Stop.**

## Quick Reference

```bash
# 1. Launch TUI in tmux pane
tmux send-keys -t PANE "cargo run -- FIXTURE" Enter
sleep 1 && tmux capture-pane -t PANE -p

# 2. Run specific snapshot test
cargo test --test view_snapshots TEST_NAME -- --nocapture

# 3. Accept new snapshots (use cargo insta)
cargo insta accept          # Accept all pending snapshots
cargo insta review          # Interactively review snapshots
cargo insta pending-snapshots  # List pending snapshots

# 4. Create bug bead
bd create --title="BUG: description" --type=bug --priority=2 --parent=EPIC
```

## cargo insta Commands

| Command | Purpose |
|---------|---------|
| `cargo insta accept` | Accept all pending snapshots |
| `cargo insta reject` | Reject all pending snapshots |
| `cargo insta review` | Interactive review (accept/reject each) |
| `cargo insta test` | Run tests then review pending |
| `cargo insta pending-snapshots` | List all pending snapshots |
| `cargo insta show SNAPSHOT` | Display a specific snapshot |

**Typical workflow:**
1. Run test: `cargo test --test view_snapshots TEST_NAME`
2. Review: `cargo insta review` (or `accept` if confident)
3. Run again to hit assertion

### CRITICAL: Review Snapshot Changes

**ALWAYS review snapshot diffs before accepting.** Spurious changes may indicate bugs:

- Unexpected blank lines → rendering bug
- Missing content → truncation bug
- Wrong content position → layout bug
- Content appearing/disappearing → state bug

```bash
# NEVER blindly accept - always review first
cargo insta review   # Shows diff, lets you accept/reject each

# Only use accept when you've already verified the change is correct
cargo insta accept   # Accepts ALL pending - use with caution
```

If a snapshot changes unexpectedly during unrelated work, **that's a regression** - investigate before accepting.

## Workflow

### Phase 1: Visual Reproduction

First, invoke the **tmux skill** with the user's pane number:

```
Use tmux skill with pane N
```

Then reproduce the bug visually:

```bash
# Launch TUI with relevant fixture
tmux send-keys -t PANE C-c
tmux send-keys -t PANE "cargo run -- tests/fixtures/FIXTURE.jsonl" Enter
sleep 1

# Capture and observe bug
tmux capture-pane -t PANE -p

# Navigate to problem area
tmux send-keys -t PANE j  # scroll
tmux send-keys -t PANE g  # top
tmux send-keys -t PANE G  # bottom
```

**Document observations:**
- What should happen (expected)
- What actually happens (actual)
- Exact truncation/rendering issue

### Phase 2: Create Minimal Fixture

Extract only the JSONL entries needed to reproduce:

```bash
# Find relevant entries
jq -c 'select(.type == "assistant")' SOURCE.jsonl | head -1

# Create minimal fixture
# tests/fixtures/BUGNAME_repro.jsonl
```

**Fixture requirements:**
- Single entry if possible
- Contains the exact content triggering the bug
- Named: `{bug_description}_repro.jsonl`

### Phase 3: Write Failing Snapshot Test

Add test to `tests/view_snapshots.rs`:

```rust
/// Bug reproduction: BRIEF_DESCRIPTION
///
/// EXPECTED: What should happen
/// ACTUAL: What actually happens
///
/// Steps to reproduce manually:
/// 1. cargo run -- tests/fixtures/FIXTURE.jsonl
/// 2. [reproduction steps]
/// 3. Observe: [bug description]
#[test]
fn bug_DESCRIPTIVE_NAME() {
    use cclv::source::FileSource;
    use cclv::state::AppState;
    use cclv::view::TuiApp;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
    use std::path::PathBuf;

    // Load minimal fixture
    let mut file_source =
        FileSource::new(PathBuf::from("tests/fixtures/FIXTURE.jsonl"))
            .expect("Should load fixture");
    let log_entries = file_source.drain_entries().expect("Should parse entries");
    let entry_count = log_entries.len();

    let entries: Vec<ConversationEntry> = log_entries
        .into_iter()
        .map(|e| ConversationEntry::Valid(Box::new(e)))
        .collect();

    // Create terminal with dimensions that trigger bug
    let backend = TestBackend::new(WIDTH, HEIGHT);
    let terminal = Terminal::new(backend).unwrap();
    let mut app_state = AppState::new();
    app_state.add_entries(entries);
    let key_bindings = cclv::config::keybindings::KeyBindings::default();
    let input_source =
        cclv::source::InputSource::Stdin(cclv::source::StdinSource::from_reader(&b""[..]));

    let mut app =
        TuiApp::new_for_test(terminal, app_state, input_source, entry_count, key_bindings);

    app.render_test().expect("Render should succeed");

    let buffer = app.terminal().backend().buffer();
    let output = buffer_to_string(buffer);

    // Snapshot captures buggy state
    insta::assert_snapshot!("bug_NAME", output);

    // Assertion that fails due to bug
    assert!(
        output.contains("EXPECTED_CONTENT"),
        "BUG: Description of what's wrong.\n\
         Expected: WHAT_SHOULD_APPEAR\n\
         Actual output:\n{output}"
    );
}
```

### Phase 4: Verify Test Fails

```bash
# Run test - should fail
cargo test --test view_snapshots bug_NAME -- --nocapture

# Accept snapshot (captures buggy state)
mv tests/snapshots/view_snapshots__bug_NAME.snap.new \
   tests/snapshots/view_snapshots__bug_NAME.snap

# Run again - assertion should fail
cargo test --test view_snapshots bug_NAME -- --nocapture
```

### Phase 5: Create Bug Bead

Find parent epic:

```bash
bd list --status=in_progress
```

Create detailed bead:

```bash
bd create --title="BUG: Brief description" \
  --type=bug --priority=2 --parent=EPIC_ID

bd update BEAD_ID --description="## Summary
WHAT: One-line description

## Expected Behavior
What should happen

## Actual Behavior
What actually happens

## Steps to Reproduce
1. command
2. command
3. Observe: issue

## Reproduction Test
- File: tests/view_snapshots.rs
- Function: bug_NAME
- Fixture: tests/fixtures/NAME_repro.jsonl
- Snapshot: tests/snapshots/view_snapshots__bug_NAME.snap"
```

### Phase 6: Mark Test Ignored

Add ignore attribute with bead reference:

```rust
#[test]
#[ignore = "BEAD_ID: brief description"]
fn bug_NAME() {
```

Verify:

```bash
cargo test --test view_snapshots bug_NAME
# Should show: ignored, BEAD_ID: brief description
```

### Phase 7: Commit

Use the commit skill:

```bash
git add tests/fixtures/NAME_repro.jsonl \
        tests/snapshots/view_snapshots__bug_NAME.snap \
        tests/view_snapshots.rs \
        .beads/issues.jsonl

# Then invoke commit skill
```

Commit message pattern:

```
test(view): add repro test for DESCRIPTION

Add failing snapshot test demonstrating BRIEF_ISSUE.

- Minimal fixture: tests/fixtures/NAME_repro.jsonl
- Test: bug_NAME (ignored)
- Bead: BEAD_ID tracks the bug
```

## Checklist

Before completing:

- [ ] Bug visually reproduced in tmux
- [ ] Minimal fixture created (< 5 entries ideal)
- [ ] Snapshot test written with clear doc comments
- [ ] Test fails on assertion (not just snapshot mismatch)
- [ ] Bead created with detailed description
- [ ] Test marked `#[ignore = "BEAD_ID: reason"]`
- [ ] All tests pass (`cargo test`)
- [ ] Changes committed with commit skill

## Example: Thinking Block Wrap Bug

**Visual observation** (in narrow terminal):
```
│This is a very long line that should definitely wrap when │
│                                                          │  ← BLANK
```
Line truncated at "when" instead of wrapping.

**Minimal fixture** (`thinking_wrap_repro.jsonl`):
```json
{"type":"assistant","message":{"content":[{"type":"thinking","thinking":"Short.\n\nVery long line that exceeds terminal width..."}]}}
```

**Test assertion**:
```rust
assert!(output.contains("window"),
    "BUG: Thinking block truncated instead of wrapped.");
```

**Bead**: `cclv-5ur.9`

**Ignore annotation**:
```rust
#[ignore = "cclv-5ur.9: thinking blocks truncated instead of wrapped"]
```

## Related Skills

- **tmux**: Required for visual verification
- **commit**: For committing the reproducer
- **beads-project-tracking**: For creating/managing beads
