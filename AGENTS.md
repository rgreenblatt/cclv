# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**cclv** (Claude Code Log Viewer) is a Rust TUI application for viewing Claude Code JSONL session logs. It supports live tailing, subagent tabs, search, statistics, markdown rendering with syntax highlighting, and full keyboard navigation.

**Status**: Specification phase - see `specs/001-claude-code-log-viewer/` for design documents.

## Build Commands

```bash
# Development environment (Nix)
nix develop                    # Enter dev shell with Rust toolchain
nix build                      # Build dynamic-linked binary
nix build .#static            # Build static binary (Linux, no glibc)
nix fmt                        # Format all code (Rust + Nix + TOML)

# Cargo commands (inside nix develop or with Rust installed)
cargo build --release         # Build release binary
cargo test                    # Run all tests
cargo test <test_name>        # Run single test
cargo clippy                  # Lint
cargo fmt                     # Format Rust code
```

## Architecture

**Pure Core / Impure Shell** - Domain logic is pure and testable:

```
src/
├── model/           # Domain types (PURE) - LogEntry, Session, Message, Stats
├── parser/          # JSONL parsing (PURE) - ContentBlock variants
├── state/           # UI state machine (PURE) - AppState, ScrollState, SearchState
├── source/          # Log input (IMPURE) - File tailing, stdin
├── view/            # TUI rendering (IMPURE) - ratatui widgets
└── config/          # Configuration (IMPURE) - TOML loading
```

**Key Design Decisions**:
- **Newtypes for IDs**: `EntryUuid`, `SessionId`, `AgentId`, `ToolUseId` - never raw strings
- **Smart constructors only**: Never export raw type constructors
- **Sum types for states**: `FocusPane`, `SearchState`, `EntryType` - illegal states unrepresentable
- **Shared rendering**: Main pane and subagent pane use identical `ConversationView` widget

## Testing Strategy

Three-tier testing aligned with Elm Architecture:

1. **Unit tests** - Pure functions (model/, parser/, state/)
2. **Property tests** - Invariants via proptest (scroll bounds, stats consistency)
3. **Snapshot tests** - Rendered output via insta + TestBackend

**IMPORTANT: Always commit the proptest-regressions files that proptest generates when property tests fail.**

```bash
cargo test                           # All tests
cargo test --lib                     # Unit tests only
cargo test parser::                  # Tests in parser module
cargo insta review                   # Review snapshot changes
```

## Key Dependencies

| Crate | Purpose |
|-------|---------|
| ratatui + crossterm | TUI framework |
| tui-markdown + syntect | Markdown/syntax highlighting |
| serde_json | JSONL parsing |
| notify | File tailing |
| clap | CLI parsing |
| proptest | Property testing |
| insta | Snapshot testing |

## Quality Gates

Before committing:
- `cargo build` - zero warnings
- `cargo test` - all pass
- `cargo clippy` - no issues
- `nix fmt` - code formatted

## Issue Tracking (beads)

```bash
bd ready              # Find available work
bd show <id>          # View issue details
bd update <id> --status in_progress  # Claim work
bd close <id>         # Complete work
bd sync               # Sync with git
```

## Session Completion

Before ending a session:
```bash
bd sync                # Sync beads with git
git status             # Verify all changes committed
```

## Specification Documents

- `specs/001-claude-code-log-viewer/spec.md` - Requirements and user stories
- `specs/001-claude-code-log-viewer/plan.md` - Implementation plan with phases
- `specs/001-claude-code-log-viewer/data-model.md` - Type definitions
- `specs/001-claude-code-log-viewer/research.md` - Technology decisions
- `specs/001-claude-code-log-viewer/contracts/cli.md` - CLI interface contract
- `.specify/memory/constitution.md` - Design principles

## Active Technologies
- Rust 1.75+ (2021 edition) + ratatui 0.29, crossterm 0.28, serde_json, chrono, tui-markdown, syntect (002-view-state-layer)
- N/A (in-memory view-state, no persistence) (002-view-state-layer)
- Rust 1.83+ (2021 edition) + ratatui 0.29, crossterm 0.28, serde_json, chrono, tui-markdown, fenwick (003-session-navigation)
- N/A (in-memory view-state) (003-session-navigation)

## Recent Changes
- 002-view-state-layer: Added Rust 1.75+ (2021 edition) + ratatui 0.29, crossterm 0.28, serde_json, chrono, tui-markdown, syntect
