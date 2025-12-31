<!--
SYNC IMPACT REPORT
==================
Version Change: 1.0.0 → 1.1.0 (Minor: added sections, expanded guidance)
Modified Principles:
  - I. Type-Driven Design → added cardinality analysis requirement
  - IV. Total Functions → added Rust-specific error crate guidance
Added Sections:
  - VII. Cardinality Analysis (new principle)
  - VIII. Skill-Based Development (new principle)
  - Development Workflow → added ownership patterns, iterator patterns
Removed Sections: None
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
- Type cardinality MUST be analyzed to ensure precision approaches 1.0 (see Principle VII)

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
- Library code MUST use `thiserror` for structured error enums
- Application/binary code SHOULD use `anyhow` with context chaining
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
- Custom `Arbitrary` strategies MUST generate only valid domain values

**Rationale:** Property-based tests explore edge cases that example-based tests miss. Laws
provide a specification that tests verify.

### VII. Cardinality Analysis

Type designs MUST be evaluated for precision to minimize invalid states.

**Requirements:**
- Valid states MUST be enumerated before designing types
- Type cardinality MUST be calculated (products multiply, sums add)
- Precision (valid states / total cardinality) MUST approach 1.0
- Products of Maybes MUST be refactored to sum types when states are mutually exclusive
- Boolean fields MUST be replaced with named enums (no `newtype Foo = Foo Bool`)
- `NonEmpty` MUST be used where empty collections are invalid
- `These` MUST be used for at-least-one-of semantics
- `Either` or sum types MUST be used for exactly-one-of semantics

**Anti-patterns that reduce precision:**
- Multiple boolean flags (2^n explosion)
- Status enum + correlated nullable fields
- Product of Maybes for mutually exclusive states
- `[a]` when non-empty is required

**Rationale:** Low precision means runtime validation, impossible pattern match cases, and
bugs when invalid states slip through. High precision means valid by construction.

### VIII. Skill-Based Development

Implementation MUST leverage available domain-specific skills when applicable patterns are
detected.

**Requirements:**
- Before writing code, check if any available skill matches the task at hand
- Use domain modeling skills when designing types, APIs, or refactoring for safety
- Use language-specific pattern skills when writing idiomatic code
- Use observability skills when implementing logging or diagnostics
- Use testing skills when designing test strategies or writing property tests
- Skills MUST be primed (invoked) when they provide relevant guidance

**Rationale:** Skills encode accumulated expertise and best practices. Priming relevant
skills before implementation ensures consistent application of proven patterns.

## Development Workflow

- **Environment**: Use `nix develop` for reproducible development environment
- **Building**: Use `nix build` for reproducible builds via naersk
- **Logging**: All logging MUST use the `tracing` crate; no `println!` or other logging mechanisms
- **Trace Levels**: Use `trace` level for internal algorithm debugging (developer audience);
  use `debug` level for library/application users
- **Structured Logging**: Log fields MUST be structured (`debug!(user_id = %id, "action")`)
  not interpolated (`debug!("action for user {}", id)`)
- **Spans**: Use `#[instrument]` or explicit spans for context propagation
- **Backwards Compatibility**: This project has NO backwards compatibility concerns; breaking
  changes are acceptable
- **Testing**: Use three-tier testing: unit tests (pure functions), property tests (proptest),
  snapshot tests (insta + ratatui TestBackend)
- **Formatting**: Run `nix fmt` (or `cargo fmt` + `nixfmt`) before committing
- **Pre-Commit Discipline**: ALL tests MUST pass before committing; run `cargo test`
- **Zero Warnings**: Compiler warnings are NOT acceptable; code MUST compile with zero warnings
- **Ownership Patterns**: Prefer `&T` for read access, `T` for ownership transfer; avoid
  excessive cloning; use `&str` not `&String`, `&[T]` not `&Vec<T>`
- **Iterator Patterns**: Prefer lazy iteration chains; avoid collecting intermediate results;
  use combinators (`map`, `filter`, `and_then`) over loops when appropriate

## Quality Gates

*GATE: All code MUST pass before merge.*

- [ ] **Types designed first**: Type signatures written before implementations
- [ ] **Cardinality analyzed**: Type precision calculated and approaching 1.0
- [ ] **No illegal states**: Sum types used for state machines and alternatives
- [ ] **Smart constructors**: Validated types constructed only through smart constructors
- [ ] **Total functions**: No partial functions in public APIs
- [ ] **Pure domain logic**: Business rules testable without IO
- [ ] **Property tests**: Laws and invariants tested via properties
- [ ] **Structured logging**: All tracing uses structured fields, appropriate levels
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

**Version**: 1.1.0 | **Ratified**: 2025-12-25 | **Last Amended**: 2025-12-26
