# cclv

TUI for viewing Claude Code JSONL session logs.

![Demo](demo.gif)

Built for those who run Claude Code with `--output-format stream-json` and want to inspect what happened. Particularly useful when orchestrating multiple sessions via scripts.

## Run

```bash
nix run github:albertov/cclv#static  # static binary, no glibc
```

Or download a [release](https://link.to/github/releases)

## Build
```
nix build github:albertov/cclv        # dynamic binary
nix build github:albertov/cclv#static # static binary, no glibc

# From source
cargo build --release
```

## Usage

```bash
# Read a log file
cclv session.jsonl

# Follow a live scripted session
claude -p "do something" --verbose --output-format stream-json | tee session.jsonl | cclv

# Or tail a log being written by another process
tail -c+0 -f session.jsonl | cclv

# Start at specific line with search active
cclv session.jsonl -l 50 -s "error"

# Show stats panel on startup
cclv session.jsonl --stats
```

### CLI Options

| Flag | Description |
|------|-------------|
| `FILE` | JSONL log file (reads stdin if omitted) |
| `-l, --line N` | Start at line N |
| `-s, --search QUERY` | Start with search query active |
| `--stats` | Show statistics panel on startup |
| `--theme NAME` | Syntax theme: base16-ocean (default), solarized-dark, solarized-light, monokai |
| `--no-color` | Disable colors |
| `--config PATH` | Custom config file |

## Features

**Navigation**: Main conversation and subagent tabs. Each tab shows the model name and entry count. Switch tabs with number keys (1-9) or Tab/Shift-Tab.

**Rendering**: Markdown with syntax highlighting. Long messages collapse automatically; expand with Enter or Space. Tool invocations display as formatted JSON.

**Statistics**: Token counts and cost estimation per agent. Toggle with `s`, filter with `f` (global), `m` (main), `S` (subagent). Note: stats parsing is currently broken for some log formats.

**Live tailing**: When reading from stdin, shows LIVE indicator and auto-scrolls. Scroll up to pause, `a` to resume.

## Keybindings

Press `?` for a scrollable help overlay. Key bindings:

**Navigation**
- `j/k` or arrows: scroll up/down
- `h/l` or left/right: scroll horizontally (long lines)
- `g/G`: top/bottom
- `Ctrl-d/u` or PageDown/Up: page down/up

**Tabs**
- `1-9`: select tab directly
- `Tab` or `]`: next tab
- `Shift-Tab` or `[`: previous tab

**Messages**
- `Enter` or `Space`: toggle expand/collapse
- `e`: expand all
- `c`: collapse all

**Search**
- `/` or `Ctrl-f`: start search
- `Ctrl-s`: submit search (Enter not yet wired up)
- `Esc`: cancel
- `n/N`: next/previous match

**Stats**
- `s`: toggle stats panel
- `f/m/S`: filter global/main/subagent

**Other**
- `w/W`: toggle item/global line wrap
- `a`: toggle auto-scroll (live mode). This happens automatically when at the end of the scroll
- `r`: refresh display
- `q`: quit

## Building

Requires Rust 1.83+.

```bash
nix develop     # Enter dev shell
cargo test      # Run tests (1200+)
cargo clippy    # Lint
nix fmt         # Format everything
```

## License

MIT
