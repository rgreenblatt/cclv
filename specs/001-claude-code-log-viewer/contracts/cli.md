# CLI Contract: cclv

**Version**: 1.0.0
**Date**: 2025-12-25

This document specifies the command-line interface contract for the Claude Code Log Viewer.

---

## Synopsis

```
cclv [OPTIONS] [FILE]
cclv --help
cclv --version
```

---

## Description

`cclv` is a terminal user interface (TUI) for viewing Claude Code JSONL log files. It supports both live tailing of active logs and viewing completed sessions.

---

## Arguments

### `[FILE]`

Path to the JSONL log file to view.

- **Type**: Path (file)
- **Required**: No (if stdin is piped)
- **Default**: Read from stdin if input is piped
- **Examples**:
  - `cclv ~/.claude/projects/.../session.jsonl`
  - `cat session.jsonl | cclv`

---

## Options

### `-l, --line <N>`

Start at specific line number.

- **Type**: Positive integer
- **Default**: 1 (start of file)
- **Example**: `cclv -l 100 session.jsonl`

### `-s, --search <QUERY>`

Start with search query active.

- **Type**: String
- **Example**: `cclv -s "error" session.jsonl`

### `--stats`

Show statistics panel on startup.

- **Type**: Flag
- **Default**: Hidden

### `--no-color`

Disable colors (for piping output or accessibility).

- **Type**: Flag
- **Default**: Colors enabled if terminal supports them

### `--theme <THEME>`

Color theme for syntax highlighting.

- **Type**: String
- **Values**: `base16-ocean`, `solarized-dark`, `solarized-light`, `monokai`
- **Default**: `base16-ocean`

### `--config <PATH>`

Path to configuration file (overrides default location).

- **Type**: Path
- **Default**: `~/.config/cclv/config.toml`
- **Note**: Config file is optional; hardcoded defaults used if missing

### `-h, --help`

Print help information.

### `-V, --version`

Print version information.

---

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success (normal exit via `q`) |
| 1 | General error |
| 2 | File not found |
| 3 | Invalid JSONL format (unrecoverable) |
| 130 | Interrupted (Ctrl+C) |

---

## Keyboard Shortcuts (Default)

### Navigation

| Key | Action |
|-----|--------|
| `j` / `↓` | Scroll down |
| `k` / `↑` | Scroll up |
| `h` / `←` | Scroll left (for long lines) |
| `l` / `→` | Scroll right |
| `Ctrl+d` / `Page Down` | Page down |
| `Ctrl+u` / `Page Up` | Page up |
| `g` / `Home` | Go to top |
| `G` / `End` | Go to bottom |

### Pane Focus

| Key | Action |
|-----|--------|
| `Tab` | Cycle focus between panes (Main → Subagent → Stats) |
| `1` | Focus main agent pane |
| `2` | Focus subagent pane |
| `3` | Focus stats panel |

### Tabs (Subagent Pane)

*Note: These shortcuts only apply when subagent pane is focused.*

| Key | Action |
|-----|--------|
| `[` / `Shift+Tab` | Previous tab |
| `]` | Next tab |
| `1`-`9` | Select tab by number |

### Message Interaction

| Key | Action |
|-----|--------|
| `Enter` / `Space` | Toggle expand/collapse message |
| `e` | Expand all messages |
| `c` | Collapse all messages |

### Search

| Key | Action |
|-----|--------|
| `/` / `Ctrl+f` | Start search |
| `Enter` | Submit search |
| `Esc` | Cancel search |
| `n` | Next match |
| `N` / `Shift+n` | Previous match |

### Stats

| Key | Action |
|-----|--------|
| `s` | Toggle stats panel |
| `!` | Filter: Global |
| `@` | Filter: Main agent only |
| `#` | Filter: Current subagent |

### Live Mode

| Key | Action |
|-----|--------|
| `a` | Toggle auto-scroll |

### Application

| Key | Action |
|-----|--------|
| `q` / `Ctrl+c` | Quit |
| `?` | Show help overlay |
| `r` | Refresh display |

---

## Environment Variables

### `CCLV_CONFIG`

Path to configuration file (includes all settings: theme, pricing, keybindings).

- **Type**: Path
- **Default**: `~/.config/cclv/config.toml` (optional - uses hardcoded defaults if missing)

### `CCLV_THEME`

Override default theme.

- **Type**: String
- **Default**: `base16-ocean`

### `NO_COLOR`

Disable colors (standard).

- **Type**: Any value to disable
- **Reference**: https://no-color.org/

---

## Configuration File

Optional TOML configuration at `~/.config/cclv/config.toml`:

```toml
# Default theme
theme = "solarized-dark"

# Show stats on startup
show_stats = false

# Collapse threshold (lines)
collapse_threshold = 10

# Summary lines for collapsed messages
summary_lines = 3

# Custom key bindings
[keybindings]
scroll_up = "k"
scroll_down = "j"
quit = "q"
# ... etc
```

---

## Examples

### View a completed session

```bash
cclv ~/.claude/projects/-home-user-myproject/abc123.jsonl
```

### Tail a live session

```bash
tail -f ~/.claude/projects/-home-user-myproject/abc123.jsonl | cclv
```

### Pipe from another command

```bash
cat session.jsonl | cclv
zcat session.jsonl.gz | cclv
```

### Start with search

```bash
cclv -s "error" session.jsonl
```

### Show stats immediately

```bash
cclv --stats session.jsonl
```

---

## Integration with Claude Code

To view logs for the current Claude Code session:

```bash
# Find most recent log file
LATEST=$(ls -t ~/.claude/projects/$(pwd | tr '/' '-')/*.jsonl 2>/dev/null | head -1)
cclv "$LATEST"
```

For live tailing of an active session:

```bash
# Tail the log file and pipe to cclv
LATEST=$(ls -t ~/.claude/projects/$(pwd | tr '/' '-')/*.jsonl 2>/dev/null | head -1)
tail -f "$LATEST" | cclv
```

Add to shell alias:

```bash
alias cclog='tail -f "$(ls -t ~/.claude/projects/$(pwd | tr "/" "-")/*.jsonl 2>/dev/null | head -1)" | cclv'
```

---

## See Also

- `tail(1)` - File tailing
- `less(1)` - File pager
- `jq(1)` - JSON processing
