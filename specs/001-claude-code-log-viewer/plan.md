# Implementation Plan: Claude Code Log Viewer TUI

**Branch**: `001-claude-code-log-viewer` | **Date**: 2025-12-25 | **Spec**: [spec.md](./spec.md)
**Input**: Feature specification from `/specs/001-claude-code-log-viewer/spec.md`

## Summary

Build a high-performance TUI application to view Claude Code JSONL log files in real-time or for post-mortem analysis. The application displays main agent and subagent conversations in split panes with tabs, supports live tailing, search, statistics, markdown rendering with syntax highlighting, and full keyboard navigation. Target: 60fps rendering, <500ms latency.

## Technical Context

**Language/Version**: Rust stable (latest, currently 1.83+)
**Primary Dependencies**: ratatui (TUI framework), crossterm (terminal backend), serde_json (JSONL parsing), notify (file watching), syntect (syntax highlighting), toml (config parsing), dirs (XDG config paths)
**Build System**: Nix flake with naersk for reproducible Rust builds
**Storage**: N/A - loads file into memory, no persistence
**Testing**: cargo test, proptest (property-based), insta (snapshot testing)
**Target Platform**: Linux/macOS terminals with 256-color or true-color support
**Project Type**: Single CLI application at repository root
**Performance Goals**: 60fps UI, <500ms write-to-display latency, <1s search in 50MB file, <1s startup for <10MB files
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

## Generated Artifacts

| Artifact | Path | Status |
|----------|------|--------|
| Implementation Plan | specs/001-claude-code-log-viewer/plan.md | ✅ Complete |
| Research Decisions | specs/001-claude-code-log-viewer/research.md | ✅ Complete |
| Data Model | specs/001-claude-code-log-viewer/data-model.md | ✅ Complete |
| CLI Contract | specs/001-claude-code-log-viewer/contracts/cli.md | ✅ Complete |
| Quickstart Guide | specs/001-claude-code-log-viewer/quickstart.md | ✅ Complete |
| Nix Flake | flake.nix | ⏳ Phase 0 |
| Dev Shell | nix/devshell.nix | ⏳ Phase 0 |
| Formatter Config | nix/treefmt.nix | ⏳ Phase 0 |

---

## Next Steps

Run `/speckit.tasks` to generate the implementation task breakdown.

