# Implementation Plan: Claude Code Log Viewer TUI

**Branch**: `001-claude-code-log-viewer` | **Date**: 2025-12-25 | **Spec**: [spec.md](./spec.md)
**Input**: Feature specification from `/specs/001-claude-code-log-viewer/spec.md`

## Summary

Build a high-performance TUI application to view Claude Code JSONL log files in real-time or for post-mortem analysis. The application displays main agent and subagent conversations in split panes with tabs, supports live streaming via stdin (using `tail -f | cclv` pattern), search, statistics, markdown rendering with syntax highlighting, and full keyboard navigation. Target: event-driven rendering, <500ms latency.

## Technical Context

**Language/Version**: Rust stable (latest, currently 1.83+)
**Primary Dependencies**: ratatui (TUI framework), crossterm (terminal backend), serde_json (JSONL parsing), syntect (syntax highlighting), toml (config parsing), dirs (XDG config paths)
**Removed Dependencies**: ~~notify~~ (file watching removed - users leverage `tail -f | cclv` pattern instead)
**Build System**: Nix flake with naersk for reproducible Rust builds
**Storage**: N/A - loads file into memory, no persistence
**Testing**: cargo test, proptest (property-based), insta (snapshot testing)
**Target Platform**: Linux/macOS terminals with 256-color or true-color support
**Project Type**: Single CLI application at repository root
**Performance Goals**: Event-driven rendering (no continuous loop), <500ms write-to-display latency, <1s search in 50MB file, <1s startup for <10MB files
**Constraints**: Virtualized rendering for large logs (v1 loads entire file into memory)
**Scale/Scope**: Single-user CLI tool, handles logs up to 1GB+

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

### Design Principles (APPLICABLE)

| Principle | Status | Notes |
|-----------|--------|-------|
| I. Type-Driven Design | WILL COMPLY | Domain types for LogEntry, Message, Agent, ContentBlock, KeyAction with smart constructors |
| II. Deep Module Architecture | WILL COMPLY | Hide JSONL parsing, rendering, file tailing behind simple interfaces |
| III. Denotational Semantics | WILL COMPLY | Define state transition semantics for UI (scroll, focus, search) |
| IV. Total Functions & Railway Programming | WILL COMPLY | Result types for parsing, file operations; thiserror for typed errors |
| V. Pure Core, Impure Shell | WILL COMPLY | Pure: state transitions, search logic. Impure: file I/O, terminal rendering |
| VI. Property-Based Testing | WILL COMPLY | proptest for parsing round-trips, search invariants, state machine properties |

### Domain Principles (NOT APPLICABLE)

| Principle | Status | Notes |
|-----------|--------|-------|
| VII. GPU-First Design | N/A | TUI application, no GPU |
| VIII. no_std Compatibility | N/A | Uses std freely |
| IX. Physical Correctness | N/A | No physics simulation |
| X. Const-Driven Initialization | N/A | Runtime configuration only |

### Quality Gates (Pre-Implementation)

- [x] **Types designed first**: Will design in data-model.md before implementation
- [x] **No illegal states**: Sum types for AgentKind, MessageType, FocusPane, ScrollState
- [x] **Smart constructors**: For validated types (TokenCount, SearchQuery, etc.)
- [x] **Total functions**: No panics in public API
- [x] **Pure domain logic**: State transitions testable without TUI
- [x] **Property tests**: Will add for parser and state machine
- [ ] **GPU compatible**: N/A
- [ ] **no_std compliant**: N/A
- [ ] **Build passes**: Pending implementation
- [ ] **Tests pass**: Pending implementation
- [ ] **Linting clean**: Pending implementation

**Gate Status**: PASS (design principles satisfied, domain principles not applicable)

## Project Structure

### Documentation (this feature)

```text
specs/001-claude-code-log-viewer/
├── plan.md              # This file
├── research.md          # Phase 0: Technology decisions
├── data-model.md        # Phase 1: Type definitions
├── quickstart.md        # Phase 1: Getting started guide
├── contracts/           # Phase 1: CLI interface contract
│   └── cli.md           # Command-line interface specification
└── tasks.md             # Phase 2 output (/speckit.tasks command)
```

### Source Code (repository root)

```text
./
├── flake.nix            # Nix flake: devShell, package build with naersk
├── flake.lock           # Locked dependencies
├── nix/
│   ├── devshell.nix     # Development shell with Rust toolchain
│   └── treefmt.nix      # Code formatting configuration
├── Cargo.toml
├── src/
│   ├── main.rs              # Entry point, CLI parsing
│   ├── lib.rs               # Public API
│   ├── model/               # Domain types (pure)
│   │   ├── mod.rs
│   │   ├── log_entry.rs     # LogEntry, Message, ContentBlock
│   │   ├── session.rs       # Session, Agent hierarchy
│   │   ├── stats.rs         # TokenStats, ToolStats
│   │   └── key_action.rs    # KeyAction enum, KeyBinding
│   ├── parser/              # JSONL parsing (pure)
│   │   ├── mod.rs
│   │   └── content_block.rs # ContentBlock variants
│   ├── state/               # UI state machine (pure)
│   │   ├── mod.rs
│   │   ├── app_state.rs     # AppState, transitions
│   │   ├── scroll.rs        # ScrollState per pane
│   │   └── search.rs        # SearchState
│   ├── source/              # Log input sources (impure shell)
│   │   ├── mod.rs
│   │   ├── file.rs          # File source with tailing
│   │   └── stdin.rs         # Stdin source
│   ├── view/                # TUI rendering (impure shell)
│   │   ├── mod.rs
│   │   ├── layout.rs        # Split pane layout
│   │   ├── message.rs       # Message rendering with markdown
│   │   ├── tabs.rs          # Subagent tab bar
│   │   └── stats.rs         # Statistics panel
│   └── config/              # Configuration (unified config.toml)
│       ├── mod.rs           # AppConfig with hardcoded defaults + optional TOML merge
│       └── keybindings.rs   # KeyAction to key mappings
└── tests/
    ├── fixtures/               # Test data (extracted from sample logs)
    │   ├── minimal_session.jsonl
    │   ├── with_subagents.jsonl
    │   ├── tool_calls.jsonl
    │   ├── malformed_lines.jsonl
    │   └── large_message.jsonl
    ├── integration/
    │   └── parse_real_logs.rs
    └── property/
        └── parser_roundtrip.rs
```

**Structure Decision**: Single crate (`cclv`) at repository root. Separates pure domain logic (`model/`, `parser/`, `state/`) from impure shell (`source/`, `view/`).

**Shared Rendering Architecture**: The main agent pane and subagent pane MUST use identical rendering code. Both render `AgentConversation` (same type for main and subagents). The `src/view/message.rs` module provides a single `ConversationView` widget that:
- Accepts `&AgentConversation` and `&ScrollState` as input
- Renders messages with markdown, syntax highlighting, expand/collapse
- Handles virtualization for performance
- Is instantiated twice in the layout: once for main pane, once for active subagent tab

This ensures visual consistency, reduces code duplication, and means features like search highlighting, expand/collapse, and styling work identically in both panes. The only difference is the layout context (left pane vs tabbed right pane).

### Sample Session Logs

Real Claude Code session logs are available for exploratory testing and fixture extraction:

```
~/*.jsonl
├── 007-proptest-terrain.log.jsonl   (61M)  # Large session with extensive tool usage
├── investigation-log.jsonl          (19M)  # Investigation/debugging session
├── session-log.jsonl                (27M)  # General session
├── session-log.003.jsonl            (13M)  # Numbered sessions
├── session-log.004.jsonl            (53M)  # Large multi-subagent session
└── ... (additional sessions)
```

**⚠️ CRITICAL: Test Fixture Policy**

These files are for **exploratory purposes and fixture extraction ONLY**:

- **NEVER** reference `~/*.jsonl` files directly in tests or source code
- **NEVER** use absolute paths to home directory in the codebase
- **ALWAYS** copy relevant JSONL lines to `tests/fixtures/` when creating test cases
- **ALWAYS** use relative paths within the project for test fixtures

```text
tests/
└── fixtures/
    ├── minimal_session.jsonl      # Minimal valid session (extracted lines)
    ├── with_subagents.jsonl       # Session with subagent spawns
    ├── tool_calls.jsonl           # Various tool call examples
    ├── malformed_lines.jsonl      # Invalid JSON for error handling tests
    └── large_message.jsonl        # Long message for collapse testing
```

**Rationale**: Tests must be self-contained and reproducible. External file dependencies break CI/CD, make tests non-portable, and create implicit coupling to developer environments.

## Complexity Tracking

> No Constitution violations requiring justification.

| Aspect | Decision | Rationale |
|--------|----------|-----------|
| Project structure | Single crate at root | Simple standalone TUI application |
| Async vs sync | Sync with polling | TUI event loops don't benefit from async; simpler model |
| Full markdown parser vs subset | Subset parser | Only need headings, bold, italic, code blocks, lists for log viewing |
| Pane rendering | Shared `ConversationView` widget | Main and subagent panes render same `AgentConversation` type; single implementation ensures consistency and DRY |

---

## Constitution Check (Post-Design)

*Re-evaluation after Phase 1 design completion.*

### Design Principles Verification

| Principle | Status | Evidence |
|-----------|--------|----------|
| I. Type-Driven Design | ✅ COMPLIANT | data-model.md: Newtypes for all IDs (EntryUuid, SessionId, AgentId), sum types for MessageContent, EntryType, FocusPane. Smart constructors only. |
| II. Deep Module Architecture | ✅ COMPLIANT | Minimal exports: `Session::add_entry()`, `LogEntry::parse()`. Implementation hidden in modules. |
| III. Denotational Semantics | ✅ COMPLIANT | data-model.md defines clear semantics: ScrollState transitions, SearchState machine states. |
| IV. Total Functions & Railway Programming | ✅ COMPLIANT | All parse functions return Result. Error types defined per module with thiserror. No panics in public API. |
| V. Pure Core, Impure Shell | ✅ COMPLIANT | Pure: model/, parser/, state/ modules. Impure: source/, view/ modules. |
| VI. Property-Based Testing | ✅ PLANNED | data-model.md: Invariants documented (scroll bounds, statistics consistency, search match validity). |

### Quality Gates (Post-Design)

- [x] **Types designed first**: Complete in data-model.md
- [x] **No illegal states**: Sum types enforce valid states throughout
- [x] **Smart constructors**: All identifiers use smart constructors (never export raw)
- [x] **Total functions**: Error types comprehensive, no unwrap in public API
- [x] **Pure domain logic**: Clean separation in module structure
- [x] **Property tests**: Invariants documented, ready for implementation
- [ ] **Build passes**: Pending implementation
- [ ] **Tests pass**: Pending implementation
- [ ] **Linting clean**: Pending implementation

**Post-Design Gate Status**: ✅ PASS

---

## Phase 0: Nix Development Environment

**Purpose**: Establish reproducible development environment before implementation begins.

### Nix Flake Design

The project uses a Nix flake with the following structure:

```nix
# flake.nix - Claude Code Log Viewer
{
  description = "Claude Code Log Viewer - TUI for viewing Claude Code JSONL logs";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/release-25.11";
    flake-parts = {
      url = "github:hercules-ci/flake-parts";
      inputs.nixpkgs-lib.follows = "nixpkgs";
    };
    treefmt-nix = {
      url = "github:numtide/treefmt-nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    naersk = {
      url = "github:nix-community/naersk";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = inputs@{ self, flake-parts, ... }:
    flake-parts.lib.mkFlake { inherit inputs; } {
      systems = [ "x86_64-linux" "aarch64-linux" "aarch64-darwin" "x86_64-darwin" ];

      perSystem = { config, self', inputs', system, pkgs, lib, ... }:
        let
          # Rust toolchain from overlay (with musl targets for static builds)
          rustToolchain = pkgs.rust-bin.stable.latest.default.override {
            extensions = [ "rust-src" "rust-analyzer" ];
            targets = [ "x86_64-unknown-linux-musl" "aarch64-unknown-linux-musl" ];
          };

          # naersk configured with our toolchain
          naersk' = pkgs.callPackage inputs.naersk {
            cargo = rustToolchain;
            rustc = rustToolchain;
          };

          # Static build configuration for Linux
          isLinux = pkgs.stdenv.isLinux;
          staticTarget = if pkgs.stdenv.hostPlatform.isx86_64
                         then "x86_64-unknown-linux-musl"
                         else "aarch64-unknown-linux-musl";
        in {
          # Default package (dynamic linking)
          packages.default = naersk'.buildPackage {
            src = ./.;
            doCheck = true;
          };

          # Static package for Linux (fully static, no glibc dependency)
          packages.static = lib.mkIf isLinux (naersk'.buildPackage {
            src = ./.;
            doCheck = true;
            CARGO_BUILD_TARGET = staticTarget;
            CARGO_BUILD_RUSTFLAGS = "-C target-feature=+crt-static";
            nativeBuildInputs = [ pkgs.pkgsStatic.stdenv.cc ];
          });

          # Development shell
          devShells.default = pkgs.mkShell {
            inputsFrom = [ self'.packages.default ];
            packages = with pkgs; [
              rustToolchain
              cargo-watch
              cargo-edit
              cargo-outdated
            ];
          };

          # Formatter (nix fmt)
          formatter = treefmtEval.config.build.wrapper;
        };
    };
}
```

### Development Shell Contents

The devShell provides:

| Tool | Purpose |
|------|---------|
| `rust-bin.stable.latest` | Rust stable toolchain with rust-analyzer |
| `cargo-watch` | Auto-rebuild on file changes |
| `cargo-edit` | Cargo add/rm/upgrade commands |
| `cargo-outdated` | Check for outdated dependencies |

### Formatting Configuration

```nix
# nix/treefmt.nix
{ pkgs, ... }: {
  projectRootFile = "flake.nix";
  programs = {
    nixfmt.enable = true;      # Nix formatting
    rustfmt.enable = true;     # Rust formatting
    taplo.enable = true;       # TOML formatting (Cargo.toml)
  };
}
```

### Usage

```bash
# Enter development shell
nix develop

# Build the package (dynamic linking)
nix build

# Build static binary (Linux only, no glibc dependency)
nix build .#static

# Run the application
nix run . -- ~/.claude/projects/.../session.jsonl

# Format all code
nix fmt

# Check formatting
nix flake check

# Verify static binary has no dynamic dependencies
ldd result/bin/cclv  # Should show "not a dynamic executable"
```

### Static Binary Distribution

The flake provides statically compiled executables for Linux:

| Architecture | Target | Command |
|--------------|--------|---------|
| x86_64-linux | `x86_64-unknown-linux-musl` | `nix build .#static` |
| aarch64-linux | `aarch64-unknown-linux-musl` | `nix build .#static` |

Static binaries:
- Have no runtime dependencies (no glibc required)
- Can run on any Linux distribution
- Are ideal for distribution and deployment
- Are slightly larger than dynamic binaries

### Phase 0 Tasks

| Task | Description | Output |
|------|-------------|--------|
| T0-001 | Create `flake.nix` with nixos-25.11, naersk, rust-overlay, musl targets | `flake.nix` |
| T0-002 | Create `nix/devshell.nix` with Rust toolchain and dev tools | `nix/devshell.nix` |
| T0-003 | Create `nix/treefmt.nix` for formatting | `nix/treefmt.nix` |
| T0-004 | Initialize Cargo.toml with project metadata | `Cargo.toml` |
| T0-005 | Create minimal `src/main.rs` for build validation | `src/main.rs` |
| T0-006 | Run `nix develop` and verify toolchain | Shell works |
| T0-007 | Run `nix build` and verify dynamic package builds | Package builds |
| T0-008 | Run `nix build .#static` and verify static binary (x86_64-linux) | Static binary, no glibc |
| T0-009 | Verify static binary with `ldd` shows "not a dynamic executable" | No dynamic deps |
| T0-010 | Run `nix fmt` and verify formatting works | Format works |

**Checkpoint**: `nix develop`, `nix build`, `nix build .#static`, and `nix fmt` all succeed. Static binary verified with `ldd`.

---

## Implementation Status

*Updated: 2025-12-27*

| Phase | Bead ID | Status | Notes |
|-------|---------|--------|-------|
| Setup | cclv-07v.1 | ✅ Complete | Nix flake, Cargo.toml, dev shell |
| Foundational | cclv-07v.2 | ✅ Complete | Core types, parser, test fixtures |
| US1 - Live Monitoring | cclv-07v.3 | ✅ Complete | ~~File tailing~~, stdin, split panes, tabs |
| US2 - Session Analysis | cclv-07v.4 | ✅ Complete | Markdown, syntax highlighting, expand/collapse |
| US3 - Usage Statistics | cclv-07v.5 | ✅ Complete | Token counts, cost estimation, filtering |
| US4 - Keyboard Navigation | cclv-07v.6 | ✅ Complete | Key bindings, focus cycling, shortcuts |
| US5 - Search | cclv-07v.7 | ✅ Complete | Search state machine, highlighting, navigation |
| Polish | cclv-07v.8 | ✅ Complete | Theme selection, snapshot tests |
| Line Wrapping | cclv-07v.9 | ✅ Complete | Core + per-entry + section-level rendering |
| ~~Logging Pane~~ | ~~cclv-07v.9.17~~ | ⛔ **SUPERSEDED** | Removed per 2025-12-27 clarification; tracing → log file |
| JSONL Format Fix | cclv-07v.11 | ✅ Complete | Parser matches actual Claude Code output |
| Acceptance Testing | cclv-31l | ✅ Complete | All 31 scenarios + E2E smoke tests |
| **Arch Simplification** | cclv-32x | 🔲 **NEW P1** | Input handling, event-driven rendering, entry indices |

---

## Phase: Line Wrapping Feature

**Purpose**: Add toggleable line-wrapping behavior with global config and per-item overrides.

**Requirements** (from spec clarifications 2025-12-26):
- FR-039: Toggleable line-wrapping with configurable global default (wrap enabled when unset)
- FR-040: When wrapping disabled, horizontal scrolling with left/right arrow keys
- FR-048: Per-conversation-item wrap toggle overrides global setting
- FR-049: Per-item wrap state is ephemeral (not persisted)
- FR-050: Default keybindings: `w` (per-item), `W` (global)
- FR-051: Global wrap state displayed in status bar
- FR-052: Wrapped lines show continuation indicator (`↩`) at wrap points
- FR-053: Code blocks never wrap (always horizontal scroll)

### Design Decisions

| Aspect | Decision | Rationale |
|--------|----------|-----------|
| Default behavior | Wrap enabled | More readable for prose; power users can disable |
| Code blocks | Never wrap | Code semantics depend on line boundaries |
| Per-item state | Ephemeral `HashSet<EntryUuid>` | No persistence needed; mirrors expand/collapse pattern |
| Visual indicator | `↩` at wrap points | Distinguishes wrap breaks from intentional line breaks |

### Data Model Additions

```rust
// ===== src/state/app_state.rs additions =====

/// Global wrap configuration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WrapMode {
    Wrap,
    NoWrap,
}

impl Default for WrapMode {
    fn default() -> Self {
        WrapMode::Wrap  // FR-039: default to wrap enabled
    }
}

// Add to AppState:
pub struct AppState {
    // ... existing fields ...
    pub global_wrap: WrapMode,
}

// Add to ScrollState:
pub struct ScrollState {
    // ... existing fields ...
    /// Messages with wrap override (opposite of global setting)
    pub wrap_overrides: HashSet<EntryUuid>,
}

impl ScrollState {
    /// Toggle wrap for a specific message
    pub fn toggle_wrap(&mut self, uuid: &EntryUuid) {
        if self.wrap_overrides.contains(uuid) {
            self.wrap_overrides.remove(uuid);
        } else {
            self.wrap_overrides.insert(uuid.clone());
        }
    }

    /// Get effective wrap mode for a message
    pub fn effective_wrap(&self, uuid: &EntryUuid, global: WrapMode) -> WrapMode {
        if self.wrap_overrides.contains(uuid) {
            match global {
                WrapMode::Wrap => WrapMode::NoWrap,
                WrapMode::NoWrap => WrapMode::Wrap,
            }
        } else {
            global
        }
    }
}

// ===== src/model/key_action.rs additions =====

pub enum KeyAction {
    // ... existing variants ...

    // Line wrapping (new)
    ToggleWrap,       // Per-item toggle (w)
    ToggleGlobalWrap, // Global toggle (W)
}
```

### Tasks

| Bead | Task | Description | Status |
|------|------|-------------|--------|
| cclv-07v.9.1 | LW-001 | Add `WrapMode` enum to `src/state/app_state.rs` | ✅ Complete |
| cclv-07v.9.2 | LW-002 | Add `wrap_overrides: HashSet<EntryUuid>` to `ScrollState` | ✅ Complete |
| cclv-07v.9.3 | LW-003 | Add `global_wrap` field to `AppState` | ✅ Complete |
| cclv-07v.9.4 | LW-004 | Add `ToggleWrap`, `ToggleGlobalWrap` to `KeyAction` enum | ✅ Complete |
| cclv-07v.9.5 | LW-005 | Add default keybindings: `w` → ToggleWrap, `W` → ToggleGlobalWrap | ✅ Complete |
| cclv-07v.9.6 | LW-006 | Add `line_wrap` config option to `AppConfig` with default `true` | ✅ Complete |
| cclv-07v.9.7 | LW-007 | Implement wrap state handlers in key event processing | ✅ Complete |
| cclv-07v.9.8 | LW-008 | Update message rendering to respect wrap mode | ✅ Complete |
| cclv-07v.9.9 | LW-009 | Add continuation indicator (`↩`) rendering at wrap points | ✅ Complete |
| cclv-07v.9.10 | LW-010 | Exempt code blocks from wrapping (entry-level) | ✅ Complete |
| cclv-07v.9.11 | LW-011 | Display global wrap state in status bar | ✅ Complete |
| cclv-07v.9.12 | LW-012 | Add tests for wrap state transitions | ✅ Complete |
| cclv-07v.9.13 | LW-013 | Add tests for wrap rendering behavior | ✅ Complete |
| cclv-07v.9.14 | LW-014 | Per-entry Paragraph architecture refactor | ✅ Complete |
| cclv-07v.9.17 | LW-015 | Logging pane feature (FR-054–FR-060) | ✅ Complete |
| cclv-07v.9.20 | LW-016 | Section-level rendering (FR-053) | ✅ Complete |

**Checkpoint**: All wrap-related tests pass; visual verification of wrap indicator and code block exemption.

### Known Issues (Blocking)

| Bead | Priority | Description | Status |
|------|----------|-------------|--------|
| cclv-07v.9.15 | P0 | Tests hang waiting for user input | ✅ Fixed |
| cclv-07v.9.16 | P1 | Errors parsing cc-session-log.jsonl (missing sessionId) | ✅ Fixed |

### View Architecture Refactor for Per-Item Wrap (cclv-07v.9.14) ✅ COMPLETE

**Status**: All 9 subtasks complete. Per-entry Paragraph architecture implemented.

**Implemented Architecture**:
```
render_conversation_view()
  ├── render outer Block (title, border)
  ├── for each visible entry:
  │   ├── calculate Y offset from cumulative heights
  │   ├── get effective_wrap(entry.uuid, global_wrap)
  │   ├── build entry's Vec<Line>
  │   ├── create Paragraph with per-entry wrap setting
  │   └── render Paragraph at calculated offset
  └── handle horizontal scroll per-entry when wrap disabled
```

**Key Files**: `src/view/message.rs` - `render_entry_lines()`, `calculate_entry_layouts()`, `EntryLayout` struct

---

### Section-Level Rendering for Code Block Exemption (cclv-07v.9.20)

**Problem**: FR-053 spec clarified (2025-12-26) that code blocks should NOT wrap while prose SHOULD wrap **within the same entry**. Current implementation uses entry-level logic: if any code block exists in entry, the entire entry doesn't wrap.

**Spec Clarification**:
> "At what granularity should code block wrap exemption apply?" → **Section-level**: each prose block and code block rendered as separate Paragraph widget, allowing code to never wrap while prose follows wrap setting within the same entry.

**Current Architecture (entry-level)**:
```
for each entry:
  if has_code_blocks(entry) → entire entry NoWrap
  else → entry follows wrap setting
  render entry as single Paragraph
```

**Target Architecture (section-level)**:
```
for each entry:
  parse markdown into sections: Vec<ContentSection>
  for each section:
    if CodeBlock → render Paragraph with NoWrap + horizontal offset
    if Prose → render Paragraph with effective_wrap() + wrap indicators
```

#### Approach

1. Create `ContentSection` enum:
```rust
enum ContentSection {
    Prose(Vec<Line<'static>>),
    CodeBlock(Vec<Line<'static>>),
}
```

2. Add `parse_entry_sections()` to split entry markdown into content sections

3. Modify render loop to iterate sections, rendering each as separate Paragraph

4. Update height calculation to sum section heights

5. Apply horizontal offset only to code sections

6. Apply wrap indicators only to prose sections

#### Subtasks

| Task | Description | Dependencies |
|------|-------------|--------------|
| LW-016.1 | Create `ContentSection` enum type | None |
| LW-016.2 | Implement `parse_entry_sections()` markdown splitter | LW-016.1 |
| LW-016.3 | Update render loop for per-section Paragraphs | LW-016.2 |
| LW-016.4 | Update height calculation for section sums | LW-016.2 |
| LW-016.5 | Apply horizontal offset to code sections only | LW-016.3 |
| LW-016.6 | Apply wrap indicators to prose sections only | LW-016.3 |
| LW-016.7 | Update search highlighting for section-level rendering | LW-016.3 |
| LW-016.8 | Add tests for mixed prose/code entries | LW-016.3 |

#### Risk Mitigation

| Risk | Mitigation |
|------|------------|
| Markdown parsing edge cases | Reuse existing `has_code_blocks()` logic, extend to extract boundaries |
| Height calculation complexity | Property tests: entry height == sum of section heights |
| Visual continuity between sections | Zero padding between sections within entry |

**Checkpoint**: Entry with both code and prose: code doesn't wrap, prose wraps (if enabled). Wrap toggle affects prose sections only. Horizontal scroll affects code sections only.

---

## ~~Phase: Logging Pane Feature~~ ⛔ SUPERSEDED

> **SUPERSEDED** (2025-12-27): Logging pane removed to simplify architecture. Tracing output now redirects to a log file (FR-054-056 updated). Users can `tail -f` the log file in another terminal. This follows the same Unix philosophy as the input handling simplification.

~~**Purpose**: Add a toggleable logging pane to display tracing output, preventing errors from breaking the main UI.~~

~~**Requirements** (from spec clarifications 2025-12-26):~~
~~- FR-054: Toggleable logging pane as a bottom panel~~
~~- FR-055: Display tracing output, log level controlled via tracing infrastructure (RUST_LOG / config)~~
~~- FR-056: Ring buffer with configurable capacity (default: 1000 entries)~~
~~- FR-057: Status bar badge showing unread log count, color-coded by severity~~
~~- FR-058: Clear unread count when user opens logging pane~~
~~- FR-059: Errors in logging pane MUST NOT interrupt main UI flow~~
~~- FR-060: Default keybinding: `L` for ToggleLogPane~~

### Design Decisions

| Aspect | Decision | Rationale |
|--------|----------|-----------|
| Pane location | Bottom panel | Standard pattern for logs/consoles in dev tools |
| Toggle key | `L` | Mnemonic for "Log", consistent with single-letter shortcuts |
| Log source | Rust tracing | Standard infrastructure; RUST_LOG controls verbosity |
| Buffer type | Ring buffer | Bounded memory; oldest entries dropped when full |
| Capacity default | 1000 entries | Sufficient for diagnosis without unbounded growth |
| Error indication | Status bar badge | Non-intrusive but always visible; color-coded severity |

### Data Model Additions

```rust
// ===== src/state/app_state.rs additions =====

/// Log entry for the logging pane
#[derive(Debug, Clone)]
pub struct LogPaneEntry {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub level: tracing::Level,
    pub message: String,
}

/// Logging pane state
#[derive(Debug)]
pub struct LogPaneState {
    /// Ring buffer of log entries
    pub entries: VecDeque<LogPaneEntry>,
    /// Maximum entries to retain (configurable)
    pub capacity: usize,
    /// Count of unread entries since pane was last opened
    pub unread_count: usize,
    /// Highest severity among unread entries
    pub unread_max_level: Option<tracing::Level>,
    /// Whether the pane is currently visible
    pub visible: bool,
}

impl LogPaneState {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: VecDeque::with_capacity(capacity),
            capacity,
            unread_count: 0,
            unread_max_level: None,
            visible: false,
        }
    }

    pub fn push(&mut self, entry: LogPaneEntry) {
        if self.entries.len() >= self.capacity {
            self.entries.pop_front();
        }
        if !self.visible {
            self.unread_count += 1;
            self.unread_max_level = Some(
                self.unread_max_level
                    .map_or(entry.level, |l| std::cmp::max(l, entry.level))
            );
        }
        self.entries.push_back(entry);
    }

    pub fn toggle_visible(&mut self) {
        self.visible = !self.visible;
        if self.visible {
            self.unread_count = 0;
            self.unread_max_level = None;
        }
    }
}

// Add to AppState:
pub struct AppState {
    // ... existing fields ...
    pub log_pane: LogPaneState,
}

// ===== src/model/key_action.rs additions =====

pub enum KeyAction {
    // ... existing variants ...

    // Logging pane (new)
    ToggleLogPane,    // Toggle log pane visibility (L)
}

// ===== src/config/mod.rs additions =====

pub struct AppConfig {
    // ... existing fields ...
    /// Maximum log entries to retain in logging pane (default: 1000)
    pub log_buffer_capacity: usize,
}
```

### Tasks

| Bead | Description | Dependencies |
|------|-------------|--------------|
| cclv-07v.9.17.1 | Add `LogPaneEntry` and `LogPaneState` types | None |
| cclv-07v.9.17.2 | Add `ToggleLogPane` to `KeyAction` enum | None |
| cclv-07v.9.17.3 | Add default keybinding: `L` → ToggleLogPane | cclv-07v.9.17.2 |
| cclv-07v.9.17.4 | Add `log_buffer_capacity` config option (default: 1000) | None |
| cclv-07v.9.17.5 | Add `log_pane` field to `AppState` | cclv-07v.9.17.1 |
| cclv-07v.9.17.6 | Create custom tracing subscriber that writes to LogPaneState | cclv-07v.9.17.1 |
| cclv-07v.9.17.7 | Implement log pane toggle handler in key event processing | cclv-07v.9.17.5, cclv-07v.9.17.3 |
| cclv-07v.9.17.8 | Create `LogPaneView` widget for rendering log entries | cclv-07v.9.17.1 |
| cclv-07v.9.17.9 | Update layout to include bottom panel when log pane visible | cclv-07v.9.17.8 |
| cclv-07v.9.17.10 | Add unread badge to status bar (count + color by severity) | cclv-07v.9.17.5 |
| cclv-07v.9.17.11 | Add `FocusLogPane` to focus cycling | cclv-07v.9.17.5 |
| cclv-07v.9.17.12 | Add tests for LogPaneState (push, capacity, unread tracking) | cclv-07v.9.17.1 |
| cclv-07v.9.17.13 | Add tests for log pane toggle and focus | cclv-07v.9.17.7 |

**Checkpoint**: Log pane toggles correctly; tracing output appears in pane; status bar shows unread count with correct severity color.

---

## Phase: JSONL Format Compatibility Fix (NEW - P0)

**Purpose**: Fix parser to match actual Claude Code CLI output format. Currently the parser cannot parse real session logs, breaking the primary use case.

**Problem Discovery** (2025-12-26 clarification session):
- Attempted to view `cc-session-log.jsonl` (produced by drinking-bird.sh)
- All entries fail to parse due to format mismatches
- Parser was designed against an assumed format, not actual Claude Code output

**Requirements** (FR-009a through FR-009d):
- FR-009a: Parser MUST use snake_case field names (`session_id`, not `sessionId`)
- FR-009b: Parser MUST handle all entry types: `system`, `user`, `assistant`, `result`
- FR-009c: Parser MUST NOT require a `timestamp` field
- FR-009d: Parser MUST handle nested `usage.cache_creation` structure

### Actual vs Expected Format Comparison

| Field | Expected (tests) | Actual (Claude Code) |
|-------|------------------|----------------------|
| Session ID | `sessionId` (camelCase) | `session_id` (snake_case) |
| Timestamp | Required `timestamp` field | **No timestamp field** |
| Entry types | user, assistant, summary | system, user, assistant, result |
| Parent reference | `parentUuid` | `parent_tool_use_id` |
| Usage structure | Flat | Nested with `cache_creation` object |
| Result entries | N/A | Has `total_cost_usd`, `duration_ms`, `modelUsage` |

### Actual Claude Code JSONL Structure

```json
// type: "system" (init, hook_response)
{
  "type": "system",
  "subtype": "init",
  "session_id": "uuid",
  "uuid": "uuid",
  "cwd": "/path",
  "model": "claude-opus-4-5-20251101",
  "tools": ["Task", "Read", ...],
  "agents": ["general-purpose", ...],
  "skills": ["commit", ...]
}

// type: "assistant"
{
  "type": "assistant",
  "message": {
    "model": "claude-opus-4-5-20251101",
    "id": "msg_xxx",
    "type": "message",
    "role": "assistant",
    "content": [{"type": "text", "text": "..."}, {"type": "thinking", "thinking": "...", "signature": "..."}],
    "usage": {
      "input_tokens": 9,
      "output_tokens": 1,
      "cache_creation_input_tokens": 37428,
      "cache_read_input_tokens": 0,
      "cache_creation": {
        "ephemeral_5m_input_tokens": 37428,
        "ephemeral_1h_input_tokens": 0
      },
      "service_tier": "standard"
    }
  },
  "parent_tool_use_id": null,
  "session_id": "uuid",
  "uuid": "uuid"
}

// type: "user"
{
  "type": "user",
  "message": {
    "role": "user",
    "content": [{"type": "tool_result", "tool_use_id": "toolu_xxx", "content": "..."}]
  },
  "parent_tool_use_id": null,
  "session_id": "uuid",
  "uuid": "uuid",
  "tool_use_result": {"success": true, "commandName": "..."},
  "isSynthetic": true
}

// type: "result" (session end)
{
  "type": "result",
  "subtype": "success",
  "is_error": false,
  "duration_ms": 306681,
  "num_turns": 36,
  "result": "...",
  "session_id": "uuid",
  "total_cost_usd": 1.3874568,
  "usage": {...},
  "modelUsage": {
    "claude-opus-4-5-20251101": {"inputTokens": 107, "outputTokens": 8320, ...}
  }
}
```

### Design Decisions

| Aspect | Decision | Rationale |
|--------|----------|-----------|
| Serde attributes | Use snake_case as default | Match actual format; simpler than aliasing |
| Entry types | Add System and Result variants | Full format coverage |
| Timestamp | Make optional | Actual entries don't have this field |
| System entries | Parse metadata (tools, agents, skills) | Useful for session info display |
| Result entries | Extract cost/usage for statistics | Accurate cost tracking from source |
| Backwards compat | Update all test fixtures | Tests should use actual format |

### Data Model Changes

```rust
// ===== src/parser/mod.rs changes =====

/// Raw JSON structure - NOW MATCHES ACTUAL FORMAT
#[derive(Debug, Deserialize)]
struct RawLogEntry {
    #[serde(rename = "type")]
    entry_type: String,
    #[serde(default)]
    message: Option<RawMessage>,  // Optional: system entries may lack message
    #[serde(default)]
    session_id: Option<String>,   // snake_case (actual format)
    uuid: String,
    #[serde(default)]
    parent_tool_use_id: Option<String>,  // Replaces parentUuid
    #[serde(default)]
    subtype: Option<String>,      // For system entries: init, hook_response
    // No timestamp field - not present in actual format
    // ... system entry fields
    #[serde(default)]
    cwd: Option<String>,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    tools: Option<Vec<String>>,
    #[serde(default)]
    agents: Option<Vec<String>>,
    #[serde(default)]
    skills: Option<Vec<String>>,
    // ... result entry fields
    #[serde(default)]
    total_cost_usd: Option<f64>,
    #[serde(default)]
    duration_ms: Option<u64>,
    #[serde(default)]
    num_turns: Option<u32>,
}

// ===== src/model/log_entry.rs changes =====

/// Entry type enum - ADD new variants
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryType {
    System,      // NEW: init, hook_response, etc.
    User,
    Assistant,
    Result,      // NEW: session completion with stats
    Summary,     // Keep for compatibility
}

/// System entry metadata (NEW)
#[derive(Debug, Clone)]
pub struct SystemMetadata {
    pub subtype: String,          // "init", "hook_response", etc.
    pub cwd: Option<PathBuf>,
    pub model: Option<String>,
    pub tools: Vec<String>,
    pub agents: Vec<String>,
    pub skills: Vec<String>,
}

/// Result entry data (NEW)
#[derive(Debug, Clone)]
pub struct ResultMetadata {
    pub subtype: String,          // "success", "error"
    pub is_error: bool,
    pub duration_ms: u64,
    pub num_turns: u32,
    pub total_cost_usd: f64,
    pub result_text: Option<String>,
}

// ===== Update TokenUsage for nested cache_creation =====

#[derive(Debug, Clone)]
pub struct TokenUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_creation_input_tokens: u64,
    pub cache_read_input_tokens: u64,
    // NEW: ephemeral cache breakdown
    pub ephemeral_5m_input_tokens: u64,
    pub ephemeral_1h_input_tokens: u64,
}
```

### Tasks

| Bead | Task | Description | Priority |
|------|------|-------------|----------|
| cclv-07v.11.1 | FMT-001 | Change `sessionId` to `session_id` (snake_case) in RawLogEntry | P0 |
| cclv-07v.11.2 | FMT-002 | Make `timestamp` optional; use entry order for sequencing | P0 |
| cclv-07v.11.3 | FMT-003 | Add `System` and `Result` entry type variants | P0 |
| cclv-07v.11.4 | FMT-004 | Change `parentUuid` to `parent_tool_use_id` | P0 |
| cclv-07v.11.5 | FMT-005 | Handle nested `usage.cache_creation` structure | P0 |
| cclv-07v.11.6 | FMT-006 | Add `SystemMetadata` for system entries (cwd, tools, agents) | P1 |
| cclv-07v.11.7 | FMT-007 | Add `ResultMetadata` for result entries (cost, duration) | P1 |
| cclv-07v.11.8 | FMT-008 | Update inline test JSON in parser tests to use actual format | P0 |
| cclv-07v.11.9 | FMT-009 | Add integration test: parse tests/fixtures/cc-session-log.jsonl | P0 |
| cclv-07v.11.10 | FMT-010 | Update Session to use result entry data for stats | P1 |
| cclv-07v.11.11 | FMT-011 | Handle system:init entry for session metadata display | P2 |
| cclv-07v.11.12 | FMT-012 | Update documentation (data-model.md, quickstart.md) | P2 |
| cclv-07v.11.13 | FMT-013 | Create script to extract ~100 representative lines as trimmed fixtures | P3 |

### Test Fixture Strategy

**Development phase** (now):
- Full session log at `tests/fixtures/cc-session-log.jsonl` (~180MB, gitignored)
- Integration tests parse the WHOLE file to ensure comprehensive coverage
- Inline test JSON in unit tests updated to match actual format

**Post-implementation** (FMT-013):
- Run extraction script to select ~100 representative lines
- Replace/trim the fixture file with sanitized subset
- Tests continue using same path - no test updates needed

### Verification Criteria

1. **Dogfooding test**: `cargo run -- tests/fixtures/cc-session-log.jsonl` displays the session
2. **Parser test**: Parse entire fixture file without errors (whole file during dev; trimmed later)
3. **Statistics accurate**: Token counts and costs match result entry values
4. **All existing tests**: Continue passing with updated inline JSON

**Checkpoint**: Application successfully parses and displays its own session logs (dogfooding).

---

## Phase: Acceptance Testing (cclv-31l)

**Purpose**: Add automated end-to-end acceptance tests for all user stories using a layered testing approach. The primary goal is to catch regressions like the scroll crash bug, verify all acceptance scenarios, and enable confident refactoring.

**Discovered Issue**: App crashes when scrolling down 4-5 times in the real session log. This must be caught by acceptance tests.

### Testing Strategy: Layered Approach

Based on research into TUI testing best practices, we implement two complementary testing layers:

| Layer | Approach | Speed | Use Case |
|-------|----------|-------|----------|
| **Layer 1** | TestBackend + handle_key | ⚡ Fast | State verification, crash detection, unit-level |
| **Layer 2** | expectrl (Rust pexpect) | 🐢 Slow | True E2E smoke tests, process-level |

### Existing Test Infrastructure (Leverage)

The codebase already has excellent foundations:

```text
tests/
├── view_snapshots.rs       # TestBackend + insta (widget snapshots) ✅
├── property_tests.rs       # proptest (invariants) ✅
├── parse_real_logs.rs      # Integration (fixture parsing) ✅
├── tui_integration.rs      # Basic TUI tests ✅
├── key_action_tests.rs     # Key binding tests ✅
├── log_pane_integration_test.rs  # Log pane tests ✅
├── wrap_property_tests.rs  # Wrap feature tests ✅
└── fixtures/
    └── cc-session-log.jsonl  # Real 180MB session log ✅
```

**Key Insight**: `TuiApp<B>` is already generic over backend, and `create_test_app()` helper exists. The infrastructure is mostly there.

### Layer 1: TestBackend Acceptance Tests (Primary)

Extend the existing TestBackend pattern to cover all user story acceptance scenarios.

#### Architecture

```rust
/// Test harness for acceptance testing
pub struct AcceptanceTestHarness {
    app: TuiApp<TestBackend>,
    width: u16,
    height: u16,
}

impl AcceptanceTestHarness {
    /// Load fixture into test app
    pub fn from_fixture(path: &str) -> Result<Self, TuiError>;

    /// Load fixture with custom terminal size
    pub fn from_fixture_with_size(path: &str, width: u16, height: u16) -> Result<Self, TuiError>;

    /// Send a single key event
    pub fn send_key(&mut self, key: KeyCode) -> bool;

    /// Send key with modifiers
    pub fn send_key_with_mods(&mut self, key: KeyCode, mods: KeyModifiers) -> bool;

    /// Send a sequence of keys
    pub fn send_keys(&mut self, keys: &[KeyCode]);

    /// Type text (for search input)
    pub fn type_text(&mut self, text: &str);

    /// Force render and return buffer as string
    pub fn render_to_string(&mut self) -> String;

    /// Assert snapshot of current render
    pub fn assert_snapshot(&mut self, name: &str);

    /// Access app state for assertions
    pub fn state(&self) -> &AppState;

    /// Check if app is still running (didn't crash/quit)
    pub fn is_running(&self) -> bool;
}
```

#### Test Coverage by User Story

**US1 - Monitor Live Agent Session (8 scenarios)**:
- `us1_scenario1_realtime_display`: Load fixture, verify entries displayed
- `us1_scenario2_stdin_input`: Test stdin source handling
- `us1_scenario3_subagent_tab_appears`: Verify subagent tabs created
- `us1_scenario4_tool_calls_display`: Check tool call rendering
- `us1_scenario5_model_name_header`: Verify model in header
- `us1_scenario6_auto_scroll_on_new`: Live mode scrolls to new content
- `us1_scenario7_auto_scroll_pause`: Scrolling up pauses auto-scroll
- `us1_scenario8_auto_scroll_resume`: Scroll to bottom resumes

**US2 - Analyze Completed Session (7 scenarios)**:
- `us2_scenario1_load_navigate`: Load full fixture, scroll without crash
- `us2_scenario2_switch_subagent_tabs`: Tab navigation works
- `us2_scenario3_search_highlight`: Search highlights matches
- `us2_scenario4_markdown_rendering`: Code blocks syntax highlighted
- `us2_scenario5_collapse_default`: Long messages collapsed
- `us2_scenario6_expand_message`: Expand works
- `us2_scenario7_collapse_message`: Collapse works

**US3 - Usage Statistics (4 scenarios)**:
- `us3_scenario1_stats_display`: Stats panel shows tokens/cost
- `us3_scenario2_filter_main_agent`: Filter works
- `us3_scenario3_tool_breakdown`: Tool counts displayed
- `us3_scenario4_filter_subagent`: Subagent filter works

**US4 - Keyboard Navigation (8 scenarios)**:
- `us4_scenario1_tab_cycles_focus`: Tab key cycles panes
- `us4_scenario2_arrow_switches_tabs`: Arrow keys in subagent pane
- `us4_scenario3_jk_scroll`: j/k scroll messages
- `us4_scenario4_slash_search`: / activates search
- `us4_scenario5_n_N_matches`: n/N navigate matches
- `us4_scenario6_enter_expands`: Enter expands message
- `us4_scenario7_enter_collapses`: Enter collapses expanded
- `us4_scenario8_horizontal_scroll`: h/l scroll horizontally

**US5 - Search (4 scenarios)**:
- `us5_scenario1_search_highlights`: Search term highlighted
- `us5_scenario2_tab_indicators`: Tabs with matches indicated
- `us5_scenario3_navigate_to_match`: Jump to match in subagent
- `us5_scenario4_clear_search`: Esc clears highlighting

**Crash Regression Tests**:
- `crash_scroll_down_many_times`: Scroll down 20 times without crash
- `crash_scroll_up_past_top`: Scroll up from top doesn't crash
- `crash_rapid_tab_switching`: Fast tab switching doesn't crash
- `crash_search_empty_results`: Search with no matches doesn't crash
- `crash_large_fixture_navigation`: Full fixture navigation works

### Layer 2: expectrl Smoke Tests (Secondary)

True end-to-end tests that spawn the actual binary. Slower but catches process-level issues.

#### Design

```rust
// tests/e2e_smoke.rs
use expectrl::spawn;
use std::time::Duration;

/// Smoke test: app starts and quits cleanly
#[test]
#[cfg(feature = "e2e-tests")]
fn smoke_app_starts_and_quits() {
    let mut session = spawn("cargo run -- tests/fixtures/minimal_session.jsonl")
        .expect("Failed to spawn app");

    // Wait for app to render
    session.expect_timeout("Session", Duration::from_secs(5))
        .expect("App should display session");

    // Quit
    session.send("q").expect("Failed to send quit");
    session.expect_eof().expect("App should exit cleanly");
}

/// Smoke test: scrolling doesn't crash
#[test]
#[cfg(feature = "e2e-tests")]
fn smoke_scroll_does_not_crash() {
    let mut session = spawn("cargo run -- tests/fixtures/cc-session-log.jsonl")
        .expect("Failed to spawn app");

    // Wait for load
    session.expect_timeout("Session", Duration::from_secs(10))
        .expect("App should load large fixture");

    // Scroll down many times
    for _ in 0..20 {
        session.send("\x1b[B").expect("Failed to send down arrow");
        std::thread::sleep(Duration::from_millis(50));
    }

    // Quit - if we get here, no crash
    session.send("q").expect("Failed to send quit");
    session.expect_eof().expect("App should exit cleanly");
}

/// Smoke test: search functionality
#[test]
#[cfg(feature = "e2e-tests")]
fn smoke_search_works() {
    let mut session = spawn("cargo run -- tests/fixtures/minimal_session.jsonl")
        .expect("Failed to spawn app");

    session.expect_timeout("Session", Duration::from_secs(5)).unwrap();

    // Open search
    session.send("/").expect("Failed to open search");
    session.expect_timeout("Search:", Duration::from_secs(1)).unwrap();

    // Type and submit
    session.send("test\n").expect("Failed to search");

    // Quit
    session.send("\x1b").expect("Failed to escape");  // Escape
    session.send("q").expect("Failed to quit");
    session.expect_eof().unwrap();
}
```

**Note**: E2E tests are gated behind the `e2e-tests` feature flag. Run with:
```bash
cargo test --features e2e-tests
```

### Dependencies

```toml
# Cargo.toml additions

[features]
e2e-tests = ["dep:expectrl"]  # Enable E2E smoke tests

[dev-dependencies]
# ... existing ...
expectrl = { version = "0.7", optional = true }  # For E2E smoke tests
```

### Tasks

| Bead | Task | Description | Priority |
|------|------|-------------|----------|
| cclv-31l.1 | AT-001 | Create AcceptanceTestHarness with from_fixture(), send_key() | P1 |
| cclv-31l.4 | AT-002 | Add render_to_string() and assert_snapshot() to harness | P1 |
| cclv-31l.5 | AT-003 | Implement US1 acceptance tests (8 scenarios) | P1 |
| cclv-31l.6 | AT-004 | Implement US2 acceptance tests (7 scenarios) | P1 |
| cclv-31l.7 | AT-005 | Implement US3 acceptance tests (4 scenarios) | P1 |
| cclv-31l.8 | AT-006 | Implement US4 acceptance tests (8 scenarios) | P1 |
| cclv-31l.9 | AT-007 | Implement US5 acceptance tests (4 scenarios) | P1 |
| cclv-31l.2 | AT-008 | Implement crash regression tests (5 scenarios) | P0 |
| cclv-31l.10 | AT-009 | Add expectrl dependency and smoke test module | P2 |
| cclv-31l.11 | AT-010 | Implement expectrl smoke tests (startup, scroll, search) | P2 |
| cclv-31l.12 | AT-011 | Add CI workflow for acceptance tests | P2 |
| cclv-31l.3 | AT-012 | Fix scroll crash bug discovered by tests | P0 |

### Risk Mitigation

| Risk | Mitigation |
|------|------------|
| TestBackend doesn't catch rendering bugs | Layer 2 expectrl tests verify actual output |
| expectrl tests are flaky (timing) | Use generous timeouts, mark as #[ignore] for regular runs |
| Large fixture slows CI | Keep full fixture tests in separate feature flag |
| Terminal size variations | Test at multiple sizes (80x24, 120x40, minimal 40x10) |

### Verification Criteria

1. **All 31 acceptance scenarios pass**: Each user story scenario has a passing test
2. **Crash regression caught**: Scroll test reproduces and verifies fix of scroll bug
3. **Full fixture test**: App loads and navigates 180MB fixture without crash
4. **CI integration**: Tests run automatically on PR
5. **Smoke tests pass**: expectrl tests verify real binary behavior

**Checkpoint**: `cargo test --test acceptance` passes all scenarios; `cargo test --features e2e-tests` runs E2E smoke tests.

---

## Phase: Architectural Simplification (cclv-32x) - NEW

**Purpose**: Simplify architecture based on 2025-12-27 spec clarifications. Remove complexity, embrace Unix philosophy.

**Requirements** (from spec clarifications 2025-12-27):
- FR-007 (updated): File arg reads entire file once; no file-watching
- FR-042 (updated): Stdin streams until EOF; enables `tail -f file | cclv` pattern
- FR-042a: App never auto-exits on EOF; user must quit
- FR-042b: LIVE indicator: gray (static/EOF), blinking green (streaming)
- FR-042c: Real-time model updates on new stdin data
- FR-028 (updated): Event-driven rendering only (no continuous loop)
- FR-028a: Idle state consumes minimal CPU
- FR-054-056 (updated): Tracing → log file; no in-UI logging pane
- FR-061-063: Entry indices per conversation (numbered bullets)
- FR-064-065: Basic mouse clicks now; architecture for future full mouse

### Design Decisions

| Aspect | Decision | Rationale |
|--------|----------|-----------|
| File tailing | Removed | Users leverage `tail -f \| cclv` - Unix philosophy |
| File watching | Removed | `notify` crate no longer needed |
| Render loop | Event-driven | Redraw on: stdin data, user input, timer (LIVE blink) |
| Logging pane | Removed | Tracing → file; `tail -f cclv.log` in another terminal |
| Entry indices | Per-conversation | Main agent + each subagent starts at 1 |
| LIVE indicator | State machine | Static → gray, Streaming → blink green, EOF → gray |
| Mouse support | Extensible | Basic clicks now; architecture supports future drag/select |

### Implementation Approach

**This is primarily a simplification/removal task.** Most work involves:
1. Removing code (file watcher, logging pane, continuous render loop)
2. Adding small new features (entry indices, LIVE indicator)
3. Refactoring existing code (event loop, input handling)

### Data Model Changes

```rust
// ===== src/state/app_state.rs changes =====

/// Input mode indicator
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Static,     // File loaded once
    Streaming,  // Stdin actively streaming
    Eof,        // Stdin reached EOF
}

// Add to AppState:
pub struct AppState {
    // ... existing fields ...
    pub input_mode: InputMode,
    // Remove: log_pane: LogPaneState,  // REMOVED
}

// ===== Entry index tracking (pure, in model layer) =====
// Entry indices are derived from position in conversation, not stored.
// Rendering calculates: enumerate(conversation.entries).map(|(i, e)| i + 1)
```

### Tasks

| Bead | Task | Description | Priority | Dependencies |
|------|------|-------------|----------|--------------|
| cclv-32x.1 | AS-001 | Remove `notify` from Cargo.toml dependencies | P1 | None |
| cclv-32x.2 | AS-002 | Remove file-watching code from `src/source/file.rs` | P1 | AS-001 |
| cclv-32x.3 | AS-003 | Simplify file source to read-once semantics | P1 | AS-002 |
| cclv-32x.4 | AS-004 | Add `InputMode` enum to AppState | P1 | None |
| cclv-32x.5 | AS-005 | Implement LIVE indicator widget (gray/blinking green) | P1 | AS-004 |
| cclv-32x.6 | AS-006 | Add LIVE indicator to status bar | P1 | AS-005 |
| cclv-32x.7 | AS-007 | Refactor event loop to event-driven (remove continuous loop) | P1 | AS-004 |
| cclv-32x.8 | AS-008 | Add timer event for LIVE indicator blink animation | P1 | AS-007 |
| cclv-32x.9 | AS-009 | Remove LogPaneState and all logging pane code | P1 | None |
| cclv-32x.10 | AS-010 | Remove `ToggleLogPane`, `FocusLogPane` from KeyAction | P1 | AS-009 |
| cclv-32x.11 | AS-011 | Configure tracing to write to log file | P1 | AS-009 |
| cclv-32x.12 | AS-012 | Add entry index rendering to message view | P1 | None |
| cclv-32x.13 | AS-013 | Implement per-conversation index counting | P1 | AS-012 |
| cclv-32x.14 | AS-014 | Add basic mouse click handling for tabs | P2 | None |
| cclv-32x.15 | AS-015 | Add basic mouse click handling for expand/collapse | P2 | AS-014 |
| cclv-32x.16 | AS-016 | Add mouse scroll support | P2 | AS-014 |
| cclv-32x.17 | AS-017 | Update tests for new input semantics | P1 | AS-003, AS-007 |
| cclv-32x.18 | AS-018 | Update acceptance tests for removed logging pane | P1 | AS-009 |

### Removal Checklist

Code to remove:
- [ ] `notify` dependency from Cargo.toml
- [ ] `src/source/file.rs` file-watching logic (keep read-once)
- [ ] `LogPaneState` struct and impl
- [ ] `LogPaneEntry` struct
- [ ] `LogPaneView` widget
- [ ] `ToggleLogPane` from KeyAction enum
- [ ] `FocusLogPane` from focus cycling
- [ ] `L` keybinding for log pane toggle
- [ ] Logging pane layout code in main view
- [ ] Status bar unread badge logic
- [ ] Continuous render loop (replace with event-driven)

### Risk Mitigation

| Risk | Mitigation |
|------|------------|
| Breaking existing tests | Update tests incrementally; AS-017, AS-018 dedicated to test updates |
| Event loop timing issues | Use crossterm's event polling with timeout for timer events |
| Mouse event conflicts | Keep mouse handling simple; defer complex interactions |
| Tracing initialization | Configure tracing early in main(); file appender from tracing-appender |

### Verification Criteria

1. **File mode works**: `cclv file.jsonl` loads file, shows gray LIVE indicator
2. **Stdin streaming works**: `tail -f file.jsonl | cclv` shows blinking green LIVE indicator
3. **EOF transition**: When stdin closes, LIVE goes gray, app stays open
4. **Entry indices visible**: Each message shows subtle index (1, 2, 3...)
5. **No logging pane**: `L` key does nothing; no bottom panel
6. **Tracing to file**: Errors logged to `~/.local/state/cclv/cclv.log`
7. **Event-driven**: CPU idle when no input (verify with `top`)
8. **Mouse clicks work**: Click on tabs switches; click on message expands/collapses
9. **All tests pass**: `cargo test` succeeds

**Checkpoint**: App starts with `tail -f | cclv`, shows blinking LIVE indicator, entries have index bullets, no logging pane visible, CPU idle when waiting for input.

---

## Generated Artifacts

| Artifact | Path | Status |
|----------|------|--------|
| Implementation Plan | specs/001-claude-code-log-viewer/plan.md | ✅ Complete |
| Research Decisions | specs/001-claude-code-log-viewer/research.md | ✅ Complete |
| Data Model | specs/001-claude-code-log-viewer/data-model.md | ✅ Complete |
| CLI Contract | specs/001-claude-code-log-viewer/contracts/cli.md | ✅ Complete |
| Quickstart Guide | specs/001-claude-code-log-viewer/quickstart.md | ✅ Complete |
| Nix Flake | flake.nix | ✅ Implemented |
| Dev Shell | nix/devshell.nix | ✅ Implemented |
| Formatter Config | nix/treefmt.nix | ✅ Implemented |
| Source Code | src/ | ✅ Complete |
| Acceptance Tests | tests/acceptance.rs, tests/e2e_smoke.rs | ✅ Complete (cclv-31l) |

---

## Current Status

*Updated: 2025-12-27*

**Active Work**: Architectural Simplification (cclv-32x) - NEW from spec clarifications

### Completed Phases

- ✅ Setup, Foundational, US1-US5, Polish phases
- ✅ Line Wrapping (FR-039 to FR-053)
- ⛔ ~~Logging Pane~~ (SUPERSEDED - tracing → log file)
- ✅ JSONL Format Compatibility (FR-009a to FR-009d)
- ✅ Acceptance Testing (cclv-31l) - 31 scenarios + E2E smoke tests

### Known Issues

*None currently blocking*

### Remaining Work

1. **P1**: Architectural Simplification (cclv-32x) - from 2025-12-27 spec clarifications:
   - Remove file-watching code (simplify to stdin streaming only)
   - Implement event-driven rendering (no continuous loop)
   - Add entry index bullets per conversation
   - Add LIVE indicator (gray/blinking green)
   - Redirect tracing to log file (remove logging pane)
   - Add basic mouse support (clicks only, architecture for future full mouse)
2. **Future**: Performance benchmarking, user documentation

