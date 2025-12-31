<!--
SYNC IMPACT REPORT
==================
Version Change: N/A → 1.0.0 (Initial version for cclv project)
Modified Principles: N/A (new constitution)
Added Sections:
  - Design Principles (6 principles applicable to TUI development)
  - Development Workflow
  - Quality Gates
  - Governance
Removed Sections:
  - Domain Principles (VII-X from Propag25: GPU-First, no_std, Physical Correctness, Const-Driven)
  - CUDA & GPU Requirements (not applicable to TUI)
Templates Requiring Updates:
  - .specify/templates/plan-template.md: ✅ No updates needed (Constitution Check section generic)
  - .specify/templates/spec-template.md: ✅ No updates needed (requirements structure unchanged)
  - .specify/templates/tasks-template.md: ✅ No updates needed (phase structure compatible)
Follow-up TODOs: None
-->

# Claude Code Log Viewer Constitution

## Design Principles

> These foundational principles govern all design decisions. They are non-negotiable.

### I. Type-Driven Design

Every design decision MUST be reflected in the type system. Types are not documentation—they
are compile-time proofs of correctness.

**Requirements:**
- Types MUST be designed before implementation begins
- Domain concepts MUST have explicit types (no primitive obsession)
- Illegal states MUST be unrepresentable through algebraic data types
- Smart constructors MUST enforce invariants at construction time
- Parse at boundaries, don't validate repeatedly
- Newtypes MUST be used for identifiers (EntryUuid, SessionId, AgentId, ToolUseId)

**Rationale:** Well-typed programs eliminate entire categories of runtime errors. The type
system is the primary tool for encoding business rules and design constraints.

### II. Deep Module Architecture

Modules MUST hide complexity behind simple interfaces. The ratio of interface simplicity to
implementation complexity determines module quality.

**Requirements:**
- Public interfaces MUST be minimal—export only what users need
- Implementation details MUST be hidden through module exports and opaque types
- Smart constructors MUST be the only way to create validated domain types
- Common cases MUST be simple; rare cases MAY be harder
- Complexity MUST be pulled downward into implementations, not pushed to callers

**Rationale:** Deep modules reduce cognitive load across the codebase. One module handling
complexity beats forcing all consumers to understand it.

### III. Denotational Semantics

Every function SHOULD have a clear mathematical meaning. Laws and properties MUST be
documented and tested.

**Requirements:**
- Core operations SHOULD have documented semantic specifications
- Mathematical laws (identity, associativity, etc.) MUST be tested via properties
- State transition semantics for UI (scroll, focus, search) MUST be well-defined
- Semantics MUST be defined before implementation when designing new abstractions

**Rationale:** Mathematical precision eliminates ambiguity and enables property-based
testing. Clear semantics make code easier to reason about and compose.

### IV. Total Functions and Railway Programming

Functions MUST handle all possible inputs explicitly. Partial functions are prohibited in
public APIs.

**Requirements:**
- Public functions MUST NOT use partial operations (panic, unwrap without justification)
- Expected errors MUST be encoded in return types (Result, Option)
- Error handling MUST compose via monadic/applicative operations (?, map_err, and_then)
- Error types MUST be specific and actionable, not stringly-typed
- Error types MUST be defined per module/layer using `#[derive(thiserror::Error)]`
- Error types MUST implement `From` for sub-component errors, enabling `?` without `map_err`
- Error types MUST contain contextual information (file paths, line numbers, parameter values)

**Rationale:** Total functions make all outcomes explicit. Railway-oriented programming
provides composable error handling without hidden control flow.

### V. Pure Core, Impure Shell

Business logic MUST be pure. Effects MUST be pushed to system boundaries.

**Requirements:**
- Domain logic MUST be implementable as pure functions
- IO and effects MUST live at application edges, not in domain code
- Pure functions MUST be testable without mocking
- State mutations MUST be isolated and controlled
- model/, parser/, state/ modules MUST be pure
- source/, view/ modules contain impure shell code

**Rationale:** Purity enables testing, reasoning, and safe concurrent execution. Separating
concerns makes code more maintainable and reusable.

### VI. Property-Based Testing

Test strategies MUST derive from types and mathematical properties, not just example cases.

**Requirements:**
- Algebraic laws MUST be tested via property-based tests (proptest)
- Serialization round-trips MUST be verified (parse . serialize = id)
- State machine invariants MUST be tested (scroll bounds, search match validity)
- Snapshot tests MUST protect rendering output (insta + TestBackend)
- Tests MUST be independent and deterministic

**Rationale:** Property-based tests explore edge cases that example-based tests miss. Laws
provide a specification that tests verify.

## Development Workflow

- **Environment**: Use `nix develop` for reproducible development environment
- **Building**: Use `nix build` for reproducible builds via naersk
- **Logging**: All logging MUST use the `tracing` crate; no `println!` or other logging mechanisms
- **Trace Levels**: Use `trace` level for internal algorithm debugging (developer audience);
  use `debug` level for library/application users
- **Backwards Compatibility**: This project has NO backwards compatibility concerns; breaking
  changes are acceptable
- **Testing**: Use three-tier testing: unit tests (pure functions), property tests (proptest),
  snapshot tests (insta + ratatui TestBackend)
- **Formatting**: Run `nix fmt` (or `cargo fmt` + `nixfmt`) before committing
- **Pre-Commit Discipline**: ALL tests MUST pass before committing; run `cargo test`
- **Zero Warnings**: Compiler warnings are NOT acceptable; code MUST compile with zero warnings

## Quality Gates

*GATE: All code MUST pass before merge.*

- [ ] **Types designed first**: Type signatures written before implementations
- [ ] **No illegal states**: Sum types used for state machines and alternatives
- [ ] **Smart constructors**: Validated types constructed only through smart constructors
- [ ] **Total functions**: No partial functions in public APIs
- [ ] **Pure domain logic**: Business rules testable without IO
- [ ] **Property tests**: Laws and invariants tested via properties
- [ ] **Build passes**: `cargo build` succeeds without warnings
- [ ] **Tests pass**: `cargo test` succeeds
- [ ] **Linting clean**: Clippy reports no issues

## Governance

This constitution governs all development for the Claude Code Log Viewer. Amendments require:

1. Documentation of the proposed change with rationale
2. Review of impact on Design Principles compliance
3. Update to this constitution file with version increment
4. Propagation of changes to dependent templates and documentation

**Versioning Policy**:
- MAJOR: Removing or redefining principles; backward incompatible governance changes
- MINOR: Adding new principles or sections; materially expanding guidance
- PATCH: Clarifications, wording improvements, typo fixes

**Compliance Review**: All code reviews MUST verify adherence to Design Principles.
Complexity that violates principles MUST be explicitly justified in the PR description.

**Version**: 1.0.0 | **Ratified**: 2025-12-25 | **Last Amended**: 2025-12-25
