# Research: Claude Code Log Viewer TUI

**Date**: 2025-12-25
**Status**: Complete
**Related**: [plan.md](./plan.md) | [spec.md](./spec.md)

## Executive Summary

This document captures technology decisions for the Claude Code Log Viewer TUI. All major unknowns have been resolved with clear recommendations based on ecosystem maturity, performance requirements, and alignment with the project constitution (type-driven design, property testing, pure core/impure shell).

## 1. TUI Framework

### Decision: **ratatui** + **crossterm**

### Rationale

ratatui is the de facto standard Rust TUI framework, a community fork of the unmaintained tui-rs with active development and a large ecosystem.

| Framework | GitHub Stars | Monthly Downloads | Maintenance | Widget Ecosystem |
|-----------|--------------|-------------------|-------------|------------------|
| ratatui   | 11k+         | 1.5M+             | Active (v0.29, v0.30 upcoming) | Extensive |
| cursive   | 4k+          | 200k+             | Active but slower | Different paradigm |
| crossterm standalone | N/A | N/A            | Active        | Backend only |

### Key Features (v0.29.0)

- **Tabs widget**: Built-in, supports deselection via `Tabs::select(None)`
- **Table widget**: Column/cell selection with `select_column`/`select_cell`, horizontal scrolling
- **Layout**: Flexible with `Layout::spacing()`, supports overlapping segments
- **Scrollable containers**: Via `Viewport` and custom scroll state
- **Double buffering**: Efficient terminal rendering, minimizes flicker
- **TestBackend**: For widget unit testing

### Upcoming (v0.30.0)

Crate split into `ratatui-core` (stable) + `ratatui-widgets` (may break more often). This allows widget crates to depend on a stable core.

### Alternatives Considered

| Alternative | Rejected Because |
|-------------|------------------|
| cursive | Retained-mode architecture less suitable for high-performance log streaming; smaller widget ecosystem |
| egui (terminal) | Immediate-mode GUI, not TUI-native |
| Custom rendering | Reinventing proven solutions; violates "don't reinvent" principle |

### Sources

- [ratatui.rs](https://ratatui.rs/)
- [ratatui v0.29.0 release notes](https://ratatui.rs/highlights/v029/)
- [GitHub ratatui/ratatui](https://github.com/ratatui/ratatui)

---

## 2. Markdown Rendering

### Decision: **tui-markdown** with **syntect** for code blocks

### Rationale

tui-markdown is ratatui-native, uses pulldown-cmark for parsing, and has built-in syntect integration via the `highlight-code` feature.

### Architecture

```
Markdown Text
    │
    ▼
pulldown-cmark (parser)
    │
    ▼
tui-markdown (renderer)
    │
    ├──► Plain text blocks → ratatui Text/Span
    │
    └──► Code blocks → syntect highlighting → syntect-tui → ratatui Spans
```

### Feature Requirements

| Requirement | tui-markdown | Custom pulldown-cmark |
|-------------|--------------|----------------------|
| Headings    | ✓            | Needs implementation |
| Bold/Italic | ✓            | Needs implementation |
| Code blocks | ✓ (syntect)  | Needs syntect integration |
| Lists       | ✓            | Needs implementation |
| Links       | ✓ (styled)   | Needs implementation |

### syntect-tui Integration

For direct syntect → ratatui conversion, use [syntect-tui](https://github.com/chanq-io/syntect-tui):

```rust
use syntect_tui::into_ratatui_text;
// Convert syntect output directly to ratatui Text
```

### Performance Considerations

- **Lazy highlighting**: Only highlight visible code blocks (virtualized)
- **Theme caching**: Load syntect theme once at startup
- **No full document parse**: Stream parse as messages arrive

### Alternatives Considered

| Alternative | Rejected Because |
|-------------|------------------|
| termimad | Not ratatui-native; outputs ANSI strings requiring conversion |
| comrak | GFM-focused, overkill for our subset; no ratatui integration |
| Custom parser | Reinventing; tui-markdown already exists and is maintained |

### Sources

- [tui-markdown docs.rs](https://docs.rs/tui-markdown/latest/tui_markdown/)
- [syntect-tui GitHub](https://github.com/chanq-io/syntect-tui)
- [syntect GitHub](https://github.com/trishume/syntect)

---

## 3. Syntax Highlighting

### Decision: **syntect** with **fancy-regex** backend

### Rationale

syntect is the industry standard for Rust syntax highlighting, used by bat, delta, and many others. It uses TextMate grammars, supporting 100+ languages out of the box.

### Configuration

```toml
[dependencies]
syntect = { version = "5", default-features = false, features = ["default-fancy"] }
```

Using `default-fancy` instead of `default-onig`:
- Pure Rust (no C dependencies)
- Slightly slower but sufficient for our use case
- Easier cross-platform compilation

### Performance Optimization

1. **Preload common themes**: `base16-ocean.dark` or `Solarized (dark)`
2. **Cache SyntaxSet**: Load once, reuse for all highlighting
3. **Incremental highlighting**: For long code blocks, use `HighlightState` for line-by-line

### Language Detection

For Claude Code logs, code blocks include language hints:

```markdown
```rust
fn main() { }
```
```

Parse the language fence and map to syntect syntax definitions.

### Sources

- [syntect docs.rs](https://docs.rs/syntect/)
- [GitHub trishume/syntect](https://github.com/trishume/syntect)

---

## 4. JSONL Parsing

### Decision: **serde_json** with streaming line-by-line

### Rationale

serde_json is the standard JSON library for Rust. For JSONL, we parse line-by-line with BufReader, avoiding full file loads.

### Architecture

```rust
use std::io::{BufRead, BufReader};
use serde_json::from_str;

fn parse_jsonl<R: Read>(reader: BufReader<R>) -> impl Iterator<Item = Result<LogEntry, Error>> {
    reader.lines().map(|line| {
        let line = line?;
        from_str(&line).map_err(Into::into)
    })
}
```

### Performance (serde_json vs simd-json)

| Library    | Parse Speed | Memory | Complexity |
|------------|-------------|--------|------------|
| serde_json | ~500 MB/s   | Moderate | Simple |
| simd-json  | ~1-2 GB/s   | Higher | Requires mutable buffer, SIMD CPU |

**Decision**: Use serde_json. Our bottleneck is rendering, not parsing. JSON lines are typically <100KB each.

### Error Handling

Malformed lines should not crash the viewer:

```rust
enum ParsedLine {
    Valid(LogEntry),
    Malformed { line_num: usize, raw: String, error: String },
}
```

Display malformed lines inline with error styling (red border, error message).

### Sources

- [serde_json docs.rs](https://docs.rs/serde_json/)
- [simd-json benchmark](https://github.com/simd-lite/simd-json)

---

## 5. File Tailing / Live Following

### Decision: **notify** (v8.x) + **notify-debouncer-mini** + custom tail logic

### Rationale

notify is the standard file system event library, used by cargo-watch, rust-analyzer, watchexec, and others. We use debouncing to batch rapid writes.

### Architecture

```
notify (inotify/FSEvents)
    │
    ▼
notify-debouncer-mini (batch events, 100ms)
    │
    ▼
Custom tail logic
    │
    ├──► Seek to last position
    ├──► Read new lines
    └──► Update UI state
```

### Configuration

```toml
[dependencies]
notify = "8"
notify-debouncer-mini = "0.6"
```

### Tail Implementation

```rust
struct FileTailer {
    path: PathBuf,
    position: u64,  // Last read position
    watcher: RecommendedWatcher,
}

impl FileTailer {
    fn read_new_lines(&mut self) -> io::Result<Vec<String>> {
        let mut file = File::open(&self.path)?;
        file.seek(SeekFrom::Start(self.position))?;
        let reader = BufReader::new(&file);
        let mut lines = Vec::new();
        for line in reader.lines() {
            lines.push(line?);
        }
        self.position = file.stream_position()?;
        Ok(lines)
    }
}
```

### Edge Cases

| Scenario | Handling |
|----------|----------|
| File deleted | Show error notification, stop following |
| File truncated | Detect via position > file_len, reset to start |
| File rotated | Detect via inode change (Linux) or path reopening |
| NFS/network | Use PollWatcher fallback (configurable) |

### Sources

- [notify docs.rs](https://docs.rs/notify/)
- [notify-debouncer-mini docs.rs](https://docs.rs/notify-debouncer-mini/)

---

## 6. Stdin Handling

### Decision: Sync stdin with non-blocking polling

### Rationale

TUI applications typically use an event loop. Mixing async tokio with ratatui's sync event loop adds complexity without benefit.

### Architecture

```rust
enum InputSource {
    File { path: PathBuf, tailer: FileTailer },
    Stdin { reader: BufReader<Stdin>, complete: bool },
}

impl InputSource {
    fn poll(&mut self) -> Option<Vec<String>> {
        match self {
            InputSource::File { tailer, .. } => tailer.poll_new_lines(),
            InputSource::Stdin { reader, complete } => {
                if *complete { return None; }
                // Non-blocking read using crossterm's event polling
                // or a separate thread with channel
            }
        }
    }
}
```

### Stdin Detection

```rust
fn detect_input_source(args: &Args) -> InputSource {
    if let Some(path) = &args.file {
        InputSource::File { path: path.clone(), tailer: FileTailer::new(path) }
    } else if atty::isnt(atty::Stream::Stdin) {
        InputSource::Stdin { reader: BufReader::new(io::stdin()), complete: false }
    } else {
        // No file and no piped input - error
        panic!("No input source: provide a file path or pipe data to stdin");
    }
}
```

### Sources

- [atty crate](https://docs.rs/atty/) for TTY detection
- [crossterm event handling](https://docs.rs/crossterm/latest/crossterm/event/)

---

## 7. Testing Strategy

### Decision: Three-tier testing with Elm Architecture

### Tier 1: Unit Tests (Pure Functions)

Test domain logic without TUI:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_finds_matches() {
        let session = Session::from_entries(vec![...]);
        let results = session.search("error");
        assert_eq!(results.len(), 3);
    }
}
```

### Tier 2: Property Tests (proptest)

Test invariants and state machine properties:

```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn scroll_position_stays_in_bounds(
        entries in prop::collection::vec(any::<LogEntry>(), 0..100),
        scroll_actions in prop::collection::vec(any::<ScrollAction>(), 0..50)
    ) {
        let mut state = AppState::new(entries.clone());
        for action in scroll_actions {
            state = state.apply(action);
            prop_assert!(state.scroll_offset <= entries.len());
        }
    }
}
```

### Tier 3: Snapshot Tests (insta + TestBackend)

Test rendered output for regression:

```rust
use insta::assert_snapshot;
use ratatui::backend::TestBackend;

#[test]
fn message_renders_correctly() {
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();

    let message = Message::user("Hello, Claude!");
    terminal.draw(|f| render_message(f, f.area(), &message)).unwrap();

    let buffer = terminal.backend().buffer().clone();
    assert_snapshot!(buffer_to_string(&buffer));
}
```

### Elm Architecture Pattern

Separate Model, Update, and View for testability:

```rust
// Model (pure data)
struct AppState { ... }

// Update (pure function: state + event -> new state)
fn update(state: AppState, event: Event) -> AppState { ... }

// View (pure function: state -> widget tree)
fn view(state: &AppState) -> impl Widget { ... }

// Main loop (impure shell)
fn run(terminal: &mut Terminal) -> Result<()> {
    let mut state = AppState::default();
    loop {
        terminal.draw(|f| f.render_widget(view(&state), f.area()))?;
        if let Some(event) = poll_event()? {
            state = update(state, event);
        }
    }
}
```

### Sources

- [ratatui testing docs](https://ratatui.rs/recipes/testing/snapshots/)
- [ratatui Elm Architecture](https://ratatui.rs/concepts/application-patterns/the-elm-architecture/)
- [proptest state machine testing](https://proptest-rs.github.io/proptest/proptest/state-machine.html)
- [insta snapshot testing](https://insta.rs/)

---

## 8. Key Binding Configuration

### Decision: Enum-based actions with TOML configuration

### Architecture

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeyAction {
    // Navigation
    ScrollUp,
    ScrollDown,
    ScrollLeft,
    ScrollRight,
    PageUp,
    PageDown,

    // Focus
    FocusMainPane,
    FocusSubagentPane,
    FocusStatsPanel,
    NextTab,
    PrevTab,

    // Message interaction
    ExpandMessage,
    CollapseMessage,
    ToggleExpand,

    // Search
    StartSearch,
    NextMatch,
    PrevMatch,
    ClearSearch,

    // Application
    Quit,
    Help,
}

#[derive(Debug, Clone)]
pub struct KeyBindings {
    bindings: HashMap<KeyEvent, KeyAction>,
}

impl Default for KeyBindings {
    fn default() -> Self {
        let mut bindings = HashMap::new();
        // Vim-style defaults
        bindings.insert(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE), KeyAction::ScrollDown);
        bindings.insert(KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE), KeyAction::ScrollUp);
        bindings.insert(KeyEvent::new(KeyCode::Char('h'), KeyModifiers::NONE), KeyAction::ScrollLeft);
        bindings.insert(KeyEvent::new(KeyCode::Char('l'), KeyModifiers::NONE), KeyAction::ScrollRight);
        // ... more defaults
        Self { bindings }
    }
}
```

### TOML Configuration (Future)

```toml
# ~/.config/cclogview/keybindings.toml
[keybindings]
scroll_up = "k"
scroll_down = "j"
next_tab = "Tab"
prev_tab = "Shift+Tab"
quit = "q"
```

---

## 9. Performance Architecture

### Virtualized Rendering

Only render visible messages to maintain 60fps with large logs:

```rust
struct VirtualizedList {
    items: Vec<LogEntry>,
    scroll_offset: usize,
    visible_height: u16,
}

impl VirtualizedList {
    fn visible_items(&self) -> &[LogEntry] {
        let start = self.scroll_offset;
        let end = (start + self.visible_height as usize).min(self.items.len());
        &self.items[start..end]
    }
}
```

### Lazy Statistics Calculation

Calculate statistics incrementally as entries arrive:

```rust
struct IncrementalStats {
    input_tokens: u64,
    output_tokens: u64,
    tool_counts: HashMap<String, u32>,
}

impl IncrementalStats {
    fn add_entry(&mut self, entry: &LogEntry) {
        if let Some(usage) = entry.usage() {
            self.input_tokens += usage.input_tokens;
            self.output_tokens += usage.output_tokens;
        }
        for tool in entry.tool_calls() {
            *self.tool_counts.entry(tool.name.clone()).or_default() += 1;
        }
    }
}
```

### Memory Budget

Target: <256MB for 100MB log files

| Component | Allocation Strategy |
|-----------|-------------------|
| Log entries | Store parsed entries, not raw JSON |
| Messages | Store content as String, not owned copies |
| Rendered widgets | Recreate each frame (immediate mode) |
| Search index | On-demand, cleared when search closes |

---

## 10. Dependencies Summary

```toml
[dependencies]
# TUI
ratatui = "0.29"
crossterm = "0.28"

# Markdown & Highlighting
tui-markdown = { version = "0.3", features = ["highlight-code"] }
syntect = { version = "5", default-features = false, features = ["default-fancy"] }
syntect-tui = "0.3"

# Parsing
serde = { version = "1", features = ["derive"] }
serde_json = "1"

# File handling
notify = "8"
notify-debouncer-mini = "0.6"

# CLI
clap = { version = "4", features = ["derive"] }

# Error handling
thiserror = "2"

# Logging
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }

[dev-dependencies]
proptest = "1"
insta = { version = "1", features = ["json"] }
```

---

## Appendix: Claude Code JSONL Format

Based on analysis of actual log files at `~/.claude/projects/*/`:

### Main Session Entry

```json
{
  "parentUuid": "abc-123" | null,
  "isSidechain": false,
  "userType": "external",
  "cwd": "/path/to/project",
  "sessionId": "uuid",
  "version": "2.0.76",
  "gitBranch": "branch-name",
  "type": "user" | "assistant" | "summary",
  "message": {
    "role": "user" | "assistant",
    "content": "string" | [ContentBlock]
  },
  "uuid": "unique-id",
  "timestamp": "2025-12-25T15:37:46.103Z"
}
```

### Subagent Entry (additional fields)

```json
{
  "agentId": "a7b2877",
  "isSidechain": true,
  // ... same as main entry
}
```

### ContentBlock Variants

```json
// Text
{ "type": "text", "text": "..." }

// Tool use
{ "type": "tool_use", "id": "toolu_xxx", "name": "Read", "input": {...} }

// Tool result
{ "type": "tool_result", "tool_use_id": "toolu_xxx", "content": "...", "is_error": false }

// Thinking (extended thinking)
{ "type": "thinking", "thinking": "...", "signature": "..." }
```

### Usage Information (in assistant message)

```json
{
  "message": {
    "model": "claude-opus-4-5-20251101",
    "usage": {
      "input_tokens": 9,
      "output_tokens": 3,
      "cache_creation_input_tokens": 37192,
      "cache_read_input_tokens": 0
    }
  }
}
```
