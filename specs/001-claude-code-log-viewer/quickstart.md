# Quickstart: Claude Code Log Viewer

Get started viewing Claude Code logs in under 5 minutes.

---

## Installation

### Using Nix (Recommended)

```bash
# Enter development shell
nix develop

# Build the package
nix build

# Run directly
nix run . -- ~/.claude/projects/.../session.jsonl
```

### From Source (without Nix)

```bash
cargo build --release
# Binary at: target/release/cclv
```

### Add to PATH

```bash
# Add to ~/.bashrc or ~/.zshrc
export PATH="$PATH:/path/to/cclv/target/release"

# Or with Nix profile
nix profile install .
```

---

## Basic Usage

### View a completed session

```bash
cclogview ~/.claude/projects/-home-user-myproject/abc123.jsonl
```

### Tail a live session

```bash
tail -f ~/.claude/projects/-home-user-myproject/abc123.jsonl | cclogview
```

### View current Claude Code session

```bash
# Find the most recent log for current directory
LATEST="$(ls -t ~/.claude/projects/$(pwd | tr '/' '-')/*.jsonl 2>/dev/null | head -1)"
tail -f "$LATEST" | cclogview
```

---

## Interface Overview

```
┌─────────────────────────────────────────────────────────────────────────────┐
│ Claude Code Log Viewer                              claude-opus-4-5 [LIVE] │
├───────────────────────────────┬─────────────────────────────────────────────┤
│                               │                                             │
│  Main Agent                   │  Subagents  [agent-a7b2877] [agent-b3c4d5] │
│                               │                                             │
│  👤 User: Fix the bug in...   │  👤 User: Research best practices...       │
│                               │                                             │
│  🤖 Assistant:                │  🤖 Assistant:                             │
│     Let me investigate...     │     I'll search for patterns...            │
│                               │                                             │
│     ┌─ Read ──────────────┐   │     ┌─ WebSearch ─────────────┐            │
│     │ 📄 src/main.rs      │   │     │ "rust error handling"   │            │
│     │ (+42 more lines)    │   │     │ (+3 results)            │            │
│     └─────────────────────┘   │     └──────────────────────────┘            │
│                               │                                             │
│     I found the issue...      │     Based on my research...                │
│     (+8 more lines)           │                                             │
│                               │                                             │
├───────────────────────────────┴─────────────────────────────────────────────┤
│ Entries: 42 │ Input: 125K │ Output: 18K │ Cost: ~$2.35 │ Press ? for help  │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Essential Keyboard Shortcuts

### Navigation
- `j`/`k` or `↓`/`↑` - Scroll up/down
- `h`/`l` or `←`/`→` - Scroll left/right (for long lines)
- `Tab` - Switch between panes

### Message Interaction
- `Enter` or `Space` - Expand/collapse message
- `e` - Expand all messages
- `c` - Collapse all messages

### Search
- `/` - Start search
- `n`/`N` - Next/previous match

### Quick Actions
- `s` - Toggle statistics panel
- `?` - Show help overlay
- `q` - Quit

---

## Understanding the Display

### Message Types

| Icon | Type | Description |
|------|------|-------------|
| 👤 | User | User input or command |
| 🤖 | Assistant | Claude's response |
| 📋 | Summary | Conversation summary |

### Tool Call Cards

Tool calls appear as collapsible cards within assistant messages:

```
┌─ Read ──────────────────────────┐
│ 📄 /path/to/file.rs            │
│ ┌───────────────────────────┐  │
│ │ fn main() {               │  │
│ │     println!("Hello");    │  │
│ │ }                         │  │
│ │ (+42 more lines)          │  │
│ └───────────────────────────┘  │
└─────────────────────────────────┘
```

### Collapsed Messages

Long messages (>10 lines) are collapsed by default:

```
🤖 Assistant:
   Based on my analysis of the codebase, I can see that the issue
   stems from incorrect error handling in the authentication module.
   The problem occurs when...
   (+8 more lines)
```

Press `Enter` to expand.

---

## JSONL Format Reference

Claude Code logs use JSONL (JSON Lines) format with these entry types:

### Entry Types

| Type | Description |
|------|-------------|
| `user` | User input to Claude |
| `assistant` | Claude's response |
| `system` | System events (session init, hooks) |
| `result` | Session completion with cost/duration |
| `summary` | Conversation summary |

### Common Fields

| Field | Type | Required | Notes |
|-------|------|----------|-------|
| `type` | string | Yes | Entry type (see above) |
| `uuid` | string | Yes | Unique entry identifier |
| `session_id` | string | No | Session ID (snake_case, defaults to "unknown-session") |
| `timestamp` | string | No | Not present in actual output |
| `parent_tool_use_id` | string | No | Links to parent entry for tool calls |
| `agentId` | string | No | Subagent identifier (camelCase) |
| `message` | object | Varies | Message content for user/assistant entries |

### Example: System Init Entry

```json
{
  "type": "system",
  "subtype": "init",
  "session_id": "e9bc0c98",
  "uuid": "38df9820",
  "cwd": "/home/user/project",
  "model": "claude-opus-4-5-20251101",
  "tools": ["Read", "Write", "Bash"]
}
```

### Example: User Message

```json
{
  "type": "user",
  "message": {
    "role": "user",
    "content": "Hello"
  },
  "session_id": "e9bc0c98",
  "uuid": "uuid-001"
}
```

### Example: Assistant with Tool Use

```json
{
  "type": "assistant",
  "message": {
    "role": "assistant",
    "content": [
      {
        "type": "text",
        "text": "I'll investigate the issue."
      },
      {
        "type": "tool_use",
        "id": "toolu_abc",
        "name": "Read",
        "input": {"file_path": "/path/to/file.rs"}
      }
    ],
    "model": "claude-opus-4-5-20251101",
    "usage": {
      "input_tokens": 1250,
      "output_tokens": 320,
      "cache_creation_input_tokens": 0,
      "cache_read_input_tokens": 850
    }
  },
  "session_id": "e9bc0c98",
  "uuid": "uuid-002"
}
```

### Example: Session Result

```json
{
  "type": "result",
  "is_error": false,
  "duration_ms": 306681,
  "num_turns": 36,
  "total_cost_usd": 1.39,
  "result": "Session complete",
  "session_id": "e9bc0c98",
  "uuid": "9cafe6c3"
}
```

**Key Points**:
- Field `session_id` uses **snake_case** (not camelCase)
- Field `agentId` uses **camelCase** (inconsistent naming)
- Field `timestamp` is **not present** in actual output (parser uses fallback)
- Field `parent_tool_use_id` (not `parentUuid`) links tool results to tool use
- Token usage includes cache metrics (`cache_creation_input_tokens`, `cache_read_input_tokens`)

---

## Statistics Panel

Press `s` to toggle the statistics panel:

```
┌─ Statistics ─────────────────────┐
│                                  │
│  Filter: [Global] Main Subagent  │
│                                  │
│  Tokens                          │
│  ├─ Input:  125,432              │
│  ├─ Output:  18,291              │
│  └─ Total:  143,723              │
│                                  │
│  Estimated Cost: $2.35           │
│                                  │
│  Tool Usage                      │
│  ├─ Read:     42                 │
│  ├─ Write:    12                 │
│  ├─ Bash:     28                 │
│  ├─ Grep:     15                 │
│  └─ Task:      3                 │
│                                  │
│  Subagents: 3                    │
│                                  │
└──────────────────────────────────┘
```

### Filtering Statistics

- `!` - Show global (all agents)
- `@` - Show main agent only
- `#` - Show current subagent only

---

## Live Mode

When tailing a live session (via stdin piping with `tail -f`):

1. **Auto-scroll enabled** - New messages automatically scroll into view
2. **New message indicator** - Shows count of new messages when scrolled away
3. **Auto-scroll toggle** - Press `a` to pause/resume auto-scroll

### Resuming Auto-scroll

When auto-scroll is paused (you've scrolled up):
- Press `a` to toggle auto-scroll back on
- Press `G` to jump to bottom (also resumes auto-scroll)
- A "↓ New messages" indicator appears at the bottom

---

## Search

### Starting a Search

1. Press `/` or `Ctrl+f`
2. Type your search query
3. Press `Enter` to search

### Navigating Results

- `n` - Jump to next match
- `N` - Jump to previous match
- Matches are highlighted in all visible panes
- Tabs with matches show an indicator

### Clearing Search

- Press `Esc` during search input
- Press `/` and submit empty query

---

## Tips & Tricks

### Shell Alias

Add to `~/.bashrc` or `~/.zshrc`:

```bash
# Tail current session logs
alias cclog='tail -f "$(ls -t ~/.claude/projects/$(pwd | tr "/" "-")/*.jsonl 2>/dev/null | head -1)" | cclogview'

# Tail with stats
alias cclogs='tail -f "$(ls -t ~/.claude/projects/$(pwd | tr "/" "-")/*.jsonl 2>/dev/null | head -1)" | cclogview --stats'
```

### Pipe from Another Source

```bash
# View compressed logs
zcat session.jsonl.gz | cclogview

# Filter before viewing
jq 'select(.type == "assistant")' session.jsonl | cclogview
```

### Multiple Sessions

To view a different session than the most recent:

```bash
# List available sessions
ls -lt ~/.claude/projects/-home-user-myproject/*.jsonl

# View specific session
cclogview ~/.claude/projects/-home-user-myproject/specific-session.jsonl
```

---

## Troubleshooting

### "No input source" error

Make sure you either:
1. Provide a file path: `cclogview /path/to/file.jsonl`
2. Pipe data to stdin: `cat file.jsonl | cclogview`

### Colors look wrong

Try:
```bash
# Check terminal color support
echo $TERM  # Should be xterm-256color or similar

# Disable colors if needed
cclogview --no-color file.jsonl
```

### File not updating in live mode

1. Ensure Claude Code is actually writing to the file
2. Check file permissions
3. Use `tail -f` to stream the file: `tail -f file.jsonl | cclogview`

---

## Next Steps

- Run `cclogview --help` for full CLI reference
- Press `?` in the viewer for keyboard shortcut reference
- See [CLI Contract](./contracts/cli.md) for complete documentation
