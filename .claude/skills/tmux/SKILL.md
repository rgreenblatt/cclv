---
name: tmux-tui-debugging
description: Debug TUI applications via tmux pane control. Send keys, capture output, observe behavior. Use ONLY when user explicitly provides a tmux pane number (e.g., "pane 5", "tmux pane 3"). Never use proactively.
---

# Tmux TUI Debugging

Debug TUI applications by controlling a tmux pane where the app is running.

## When to Use This Skill

**ONLY use when user explicitly specifies a tmux pane number**, such as:
- "Use pane 5 to debug the TUI"
- "I have cclv running in tmux pane 3"
- "Check pane 2 for the output"

**DO NOT use proactively** - wait for explicit pane assignment.

## Quick Reference

```bash
# Capture current pane content
tmux capture-pane -t PANE -p

# Send a key
tmux send-keys -t PANE KEY

# Send Enter
tmux send-keys -t PANE Enter

# Send Ctrl+C
tmux send-keys -t PANE C-c

# Send multiple keys with delay
for key in j j j G; do tmux send-keys -t PANE $key; done && sleep 0.3
```

## Core Workflow

### 1. Capture and Observe

```bash
# Capture current state
tmux capture-pane -t 5 -p

# Capture with delay (let TUI settle)
sleep 0.5 && tmux capture-pane -t 5 -p
```

### 2. Send Input

```bash
# Single key
tmux send-keys -t 5 j      # Scroll down
tmux send-keys -t 5 k      # Scroll up
tmux send-keys -t 5 g      # Go to top
tmux send-keys -t 5 G      # Go to bottom
tmux send-keys -t 5 q      # Quit

# Special keys
tmux send-keys -t 5 Enter
tmux send-keys -t 5 Escape
tmux send-keys -t 5 C-c    # Ctrl+C
tmux send-keys -t 5 Tab
```

### 3. Launch App in Pane

```bash
# Clear and run command
tmux send-keys -t 5 C-c
tmux send-keys -t 5 "cargo run --release -- file.jsonl" Enter
sleep 1  # Wait for startup
tmux capture-pane -t 5 -p
```

### 4. Interactive Testing Pattern

```bash
# Send action and capture result
tmux send-keys -t 5 KEY && sleep 0.3 && tmux capture-pane -t 5 -p
```

## Debugging TUI Bugs

### Initial State Check
```bash
# Launch app
tmux send-keys -t 5 "cargo run -- file.jsonl" Enter
sleep 1

# Capture initial state (check for blank screen bug)
tmux capture-pane -t 5 -p
```

### Scroll Behavior Test
```bash
# Go to top, then scroll down incrementally
tmux send-keys -t 5 g && sleep 0.2
tmux capture-pane -t 5 -p  # Baseline

# Scroll down 10 times
for i in {1..10}; do tmux send-keys -t 5 j; done
sleep 0.3 && tmux capture-pane -t 5 -p  # Compare

# Go to bottom
tmux send-keys -t 5 G && sleep 0.3
tmux capture-pane -t 5 -p  # Should show last entries
```

### Content Visibility Check
```bash
# Capture and analyze output
output=$(tmux capture-pane -t 5 -p)

# Count blank lines
echo "$output" | grep -c "^│[[:space:]]*│$"

# Check for specific content
echo "$output" | grep -q "Expected Text"
```

## Creating Reproducers

After observing bug via tmux:

1. **Document observed behavior** from captures
2. **Identify minimal reproduction** (which keys/actions trigger it)
3. **Write programmatic test** using TestBackend
4. **Verify test fails** the same way TUI does

### Example: Blank Screen Bug

**Observed via tmux**:
```bash
# Launch - screen blank
tmux send-keys -t 5 "cargo run -- file.jsonl" Enter
sleep 1 && tmux capture-pane -t 5 -p  # Empty!

# Press Enter - content appears
tmux send-keys -t 5 Enter
sleep 0.3 && tmux capture-pane -t 5 -p  # Now shows UI
```

**Resulting test**:
```rust
#[test]
fn bug_initial_screen_blank() {
    let app = TuiApp::new_for_test(...);
    // Don't call render - check buffer is empty
    let buffer = app.terminal().backend().buffer();
    assert!(!buffer_to_string(buffer).is_empty());  // FAILS
}
```

## Tips

- **Always add sleep** after send-keys before capture (TUI needs time to render)
- **Use `&&`** to chain commands reliably
- **Capture multiple times** if output seems stale
- **Quote special characters** in send-keys when needed
- **Use `C-c` to interrupt** before launching new command

## Cleanup

```bash
# Quit TUI gracefully
tmux send-keys -t 5 q

# Force kill if hung
tmux send-keys -t 5 C-c
```
