# Feature Specification: View-State Layer

**Feature Branch**: `002-view-state-layer`
**Created**: 2025-12-27
**Status**: Draft
**Input**: User description: "Implement view-state layer to separate domain from presentation concerns. Fixes scroll/render type mismatch (line-based offset bounded by entry count), enables cached rendering, measured entry heights, semantic scroll positions, and efficient mouse hit-testing."

## Clarifications

### Session 2025-12-27

- Q: How are multiple conversations (main + subagents) handled? → A: Each conversation (main + each subagent) has independent view-state. Subagent view-states are created eagerly on first entry arrival (see Session 2025-12-29 clarification). Additionally, log files may contain multiple SESSIONS concatenated sequentially (from `claude code --stream-json --verbose` piped in a loop). Each session has its own main conversation + subagent conversations. The view-state layer must support: multiple sequential sessions, each with independent main + subagent view-states.
- Q: What is explicitly out of scope? → A: Session navigation UI, search highlighting, keyboard shortcut changes, new CLI flags. These are separate features that consume the view-state layer but aren't part of it. The view-state layer is purely the data structure and layout computation layer.
- Q: Why view-state layer vs simpler fixes? → A: Simpler fixes (e.g., approximating line counts, clamping in render) produce complex, hard-to-understand code. Evidence: multiple view bugs exist (blank viewport, scroll offset mismatch, hit-testing errors) and the app is currently not usable. A proper architectural solution (view-state layer) provides a clean foundation that makes the code understandable and the bugs fixable.
- Q: How should multi-session display work? → A: View-state is an ordered `Vec<SessionViewState>`. Default rendering is continuous scroll with separators, but structure trivially supports one-at-a-time (session picker) and collapsible groups via view-layer changes only. No view-state restructuring needed to switch display modes.
- Q: Should view-state reference or own domain data? → A: View-state layer OWNS the domain model (consumes during JSON parsing). EntryView owns ConversationEntry directly rather than indexing into a separate domain Vec. This is simpler, more performant (cache locality), has no lifetime complexity, and is ideal for a read-only viewer.
- Q: How are session boundaries detected? → A: Via `session_id` field in each JSONL entry. When session_id changes from previous entry, a new session starts. Sessions are always concatenated (never interleaved). This approach works for partial logs without requiring an init marker.
- Q: Which session's subagents appear in tab bar (continuous scroll mode)? → A: Subagents from the "active" session, determined by main pane scroll position. As user scrolls through sessions, tab bar updates to show subagents from the currently visible session.

### Session 2025-12-29

- Q: Should subagent view-states be created lazily on tab selection or eagerly on first entry arrival? → A: Eagerly on first entry arrival. Lazy initialization on tab selection conflicts with rendering architecture (immutable &AppState during render vs &mut self for init). Eager init has negligible memory impact since entries exist anyway.
- Q: Should UI use split-pane (main 60% / subagent 40%) or unified tabs for all conversations? → A: Unified tabs. All conversations (main agent and subagents) appear as top-level tabs in a single tabbed container. Main agent is tab 0, subagents are tabs 1..N. This reduces cognitive overhead (main agent is no longer "special") and provides consistent interaction patterns. Full viewport width for active conversation.
- Q: How many entries per conversation vs per file? → A: Log files may contain 100,000+ entries across multiple concatenated sessions, but individual conversations (main agent or subagent) are expected to have fewer than 1,000 entries. Entry indices are per-conversation (scoped), so 3-digit display (1-999) is sufficient. The 100k limit applies to total file entries, not per-conversation.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Scroll Through Large Logs Without Blank Screens (Priority: P1)

A user opens a log file with 30,000+ entries and scrolls through the entire document using Page Down, Page Up, arrow keys, Home, and End. The viewport always displays content - never a blank screen regardless of scroll position.

**Why this priority**: This is a critical bug fix. The current implementation causes blank viewports when scrolling because scroll offset (lines) is bounded by entry count instead of total line count. This is the primary motivation for the refactor.

**Independent Test**: Can be fully tested by opening a large log file, scrolling to any position, and verifying content is always visible. Delivers core value: a functional log viewer.

**Acceptance Scenarios**:

1. **Given** a log file with 30,000 entries, **When** user presses Page Down repeatedly until reaching the bottom, **Then** every viewport shows content (no blank screens)
2. **Given** user is at bottom of log, **When** user presses Home, **Then** viewport shows first entries immediately
3. **Given** user is at any position, **When** user presses End, **Then** viewport shows last entries with content visible
4. **Given** user is scrolling rapidly, **When** viewport updates, **Then** content appears within 16ms (60fps target)

---

### User Story 2 - Expand/Collapse Entries with Instant Response (Priority: P2)

A user expands or collapses conversation entries. The UI updates instantly without visible delay or layout jumps. Scroll position remains stable - the entry being toggled stays visible.

**Why this priority**: Expand/collapse is a core interaction for navigating long entries. The view-state layer enables proper height tracking so layout adjusts correctly.

**Independent Test**: Can be fully tested by loading any log with collapsible entries, toggling expand/collapse on various entries, and verifying smooth, instant updates.

**Acceptance Scenarios**:

1. **Given** a collapsed entry is visible, **When** user presses Enter/Space to expand, **Then** entry expands and remains visible in viewport
2. **Given** an expanded entry is visible, **When** user collapses it, **Then** following entries shift up smoothly without viewport jump
3. **Given** user toggles entry expand/collapse, **When** UI updates, **Then** response is under 16ms (no perceptible delay)
4. **Given** entries above current viewport are toggled, **When** layout updates, **Then** current visible entries remain stable

---

### User Story 3 - Click Entries with Mouse (Priority: P3)

A user clicks on entries with the mouse to select, expand, or collapse them. Clicks accurately target the intended entry regardless of scroll position.

**Why this priority**: Mouse interaction improves accessibility and complements keyboard navigation. The view-state layer's precomputed Y offsets enable accurate hit-testing.

**Independent Test**: Can be fully tested by clicking on various entries at different scroll positions and verifying correct entry is targeted.

**Acceptance Scenarios**:

1. **Given** conversation with multiple entries, **When** user clicks on an entry, **Then** that specific entry is selected (not adjacent entries)
2. **Given** user clicks on expand/collapse indicator, **When** entry is collapsible, **Then** entry toggles expand/collapse state
3. **Given** user has scrolled to middle of log, **When** clicking entries, **Then** hit-testing correctly maps click Y position to the right entry
4. **Given** entries have variable heights, **When** user clicks near entry boundaries, **Then** correct entry is targeted based on actual rendered heights

---

### User Story 4 - Smooth Scrolling with Cached Content (Priority: P4)

A user scrolls through content and previously viewed entries render instantly from cache. Syntax-highlighted code blocks don't flicker or re-highlight when scrolling back.

**Why this priority**: Caching improves perceived performance and reduces CPU usage. This is an optimization that enhances the experience but isn't critical for functionality.

**Independent Test**: Can be fully tested by scrolling to content with syntax-highlighted code, scrolling away, then scrolling back and observing instant render.

**Acceptance Scenarios**:

1. **Given** user has viewed entries with code blocks, **When** user scrolls away and back, **Then** cached content renders instantly (no re-highlighting)
2. **Given** viewport resizes, **When** cached content is shown, **Then** cache invalidates and content re-renders for new width
3. **Given** system has limited memory, **When** many entries are viewed, **Then** old cache entries are evicted (LRU) without affecting functionality

---

### Edge Cases

- What happens when scroll position refers to a line beyond document end? (Clamp to valid range `[0, max(0, total_height - viewport_height)]`. This guarantees no blank viewports.)
- What happens when viewport height exceeds total content height? (Show all content, no scrolling needed. Scroll offset = 0.)
- What happens when an entry's height changes while it's in the visible range? (Recalculate layout, keep entry visible)
- What happens when new entries stream in while user is scrolled to bottom? (Maintain "follow" behavior if at bottom, otherwise keep current position stable)
- What happens when clicking outside any entry's bounds? (No action, no selection change)
- What happens when cache memory limit is reached? (Evict least-recently-used entries)
- What happens when a new session starts in streaming mode? (Detect session boundary, create new SessionViewState, append to LogViewState)
- What happens when user scrolls across session boundary? (Show visual separator, maintain smooth scroll, both sessions' content visible at boundary)
- What happens when active session changes (scroll crosses boundary)? (Tab bar updates to show new session's subagents; if viewing a subagent pane, switch to main pane or first available subagent)
- What happens with malformed/unparseable entries? (Malformed entries have height=0 and are not rendered. They still occupy an entry index slot for stability.)

## Requirements *(mandatory)*

### Functional Requirements

#### Domain/View Layered Architecture

- **FR-001**: System MUST maintain a layered architecture where view-state types (EntryView, ScrollPosition) wrap and own domain types (LogEntry, Session, Message), providing a clear conceptual separation between domain concerns and presentation concerns
- **FR-002**: Domain types MUST remain pure and free of presentation concerns (no layout, scroll, or rendering data)
- **FR-003**: View-state types MUST own domain objects directly (parsed from JSON into view-state), combining domain data with presentation metadata for optimal cache locality

#### Scroll Position Semantics

- **FR-010**: Scroll position MUST be represented as a semantic type (not raw integer offset)
- **FR-011**: Scroll position MUST support at minimum: top, bottom, at-specific-line, at-specific-entry variants
- **FR-012**: Scroll position MUST resolve to absolute line offset using actual measured content heights
- **FR-013**: Scroll bounds MUST be calculated from total content height in lines (not entry count)
- **FR-014**: Scroll operations (Page Up/Down, arrows, Home/End) MUST produce valid scroll positions

#### Entry Layout

- **FR-020**: Each entry MUST have a measured height (in lines) that reflects its actual rendered size
- **FR-021**: Entry heights MUST account for: text wrapping at current viewport width, expand/collapse state, wrap mode settings
- **FR-022**: Each entry MUST have a cumulative Y offset (sum of all preceding entry heights)
- **FR-023**: Layout MUST be recomputed when: viewport width changes, expand/collapse state changes, wrap mode changes
- **FR-024**: Layout for new streaming entries MUST be computed incrementally (append-only optimization)

#### Visible Range Calculation

- **FR-030**: System MUST efficiently determine which entries are visible in current viewport
- **FR-031**: Visible range calculation MUST use precomputed layout data (not iterate all entries)
- **FR-032**: Only visible entries (plus buffer) SHOULD be rendered each frame

#### Hit Testing

- **FR-040**: System MUST map screen coordinates (x, y) to entry index and action
- **FR-041**: Hit-testing MUST use precomputed cumulative Y offsets for efficiency
- **FR-042**: Hit-testing MUST account for current scroll position
- **FR-043**: Hit-test result MUST indicate: which entry was hit, line within entry, and intended action (select, expand/collapse)

#### Render Caching

- **FR-050**: System SHOULD cache rendered output (Lines) for recently viewed entries
- **FR-051**: Render cache MUST invalidate when: viewport width changes, entry expand/collapse state changes, wrap mode changes
- **FR-052**: Render cache MUST have bounded memory usage (evict old entries when limit reached)
- **FR-053**: Cache validation MUST check render parameters match current state before reuse
- **FR-054**: Render cache capacity MUST be configurable via config file (default: 1000 entries)

#### Multi-Session and Multi-Conversation Support

- **FR-070**: System MUST support log files containing multiple sequential sessions (from repeated `claude code --stream-json` invocations)
- **FR-071**: Each session MUST have independent view-state for its main conversation
- **FR-072**: Each session MUST have independent view-states for each of its subagent conversations
- **FR-073**: Subagent view-states MUST be created eagerly on first entry arrival for that subagent (not deferred to tab selection). This ensures view-state exists before rendering, avoiding mutable access during immutable render pass.
- **FR-074**: Session boundaries MUST be visually distinguishable when scrolling through multi-session logs
- **FR-075**: New sessions streaming in MUST be detected and added without rebuilding existing session view-states
- **FR-078**: Session boundaries MUST be detected via `session_id` field change between consecutive entries (sessions are never interleaved)
- **FR-079**: Session detection MUST work for partial logs (no init marker required)
- **FR-076**: View-state structure MUST support multiple display modes (continuous scroll, one-at-a-time, collapsible) without restructuring data
- **FR-077**: Switching display modes MUST require only view-layer changes, not view-state layer changes
- **FR-080**: In continuous scroll mode, "active session" MUST be determined by main pane scroll position
- **FR-081**: Subagent tab bar MUST display subagents from the active session only
- **FR-082**: Tab bar MUST update when scroll position crosses session boundary

#### Conversation Display (Unified Tab Model)

- **FR-083**: System MUST display ALL conversations (main agent and subagents) as top-level tabs in a single tabbed container
- **FR-084**: Main agent MUST appear as first tab (index 0), with subagents as subsequent tabs (index 1..N) in spawn order
- **FR-085**: All conversations MUST use identical tab rendering and interaction patterns
- **FR-086**: Tab switching MUST work identically for main agent tab and subagent tabs
- **FR-087**: Active tab MUST receive full viewport width (no horizontal split between main and subagent)
- **FR-088**: Tab bar MUST be visible at all times when any conversation exists

#### Invalidation

- **FR-060**: System MUST track what state was used to compute layout (viewport, expand states, wrap mode)
- **FR-061**: When tracked state changes, system MUST recompute affected layouts before next render
- **FR-062**: Invalidation MUST be granular: full rebuild only when necessary, incremental append for streaming

### Key Entities

- **EntryView**: Owns a ConversationEntry (domain data) plus layout metadata (measured height, cumulative Y offset) and optional cached render. Domain data is parsed directly into EntryView, not referenced.
- **EntryLayout**: Height in lines (minimum 1), cumulative Y offset from conversation start
- **ScrollPosition**: Semantic sum type representing scroll location (Top, AtLine, AtEntry, Bottom, Fraction)
- **ConversationViewState**: Collection of EntryViews for a single conversation (owns the entries), current scroll position, total content height, layout validity tracking
- **SessionViewState**: View-state for a single session containing: main conversation view-state (owns main entries), map of subagent ID to subagent conversation view-state (created eagerly on first entry arrival, owns subagent entries)
- **LogViewState**: Top-level view-state containing: ordered `Vec<SessionViewState>` (one per session in log file). View layer decides display mode: continuous scroll (default), one-at-a-time, or collapsible groups. Display mode switching requires only view-layer changes.
- **CachedRender**: Stored rendered Lines with metadata for invalidation (viewport width, collapse state)
- **VisibleRange**: Start/end indices of visible entries plus viewport context

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: No blank viewport at any scroll position in documents up to 100,000 entries (per-file, across multiple sessions; individual conversations expected <1,000 entries)
- **SC-002**: Scroll operations complete in under 16ms (60fps) for documents up to 100,000 entries (per-file)
- **SC-003**: Expand/collapse toggle updates UI in under 16ms
- **SC-004**: Mouse click hit-testing identifies correct entry in under 1ms
- **SC-005**: Initial layout computation for 30,000 entries completes in under 500ms
- **SC-006**: Incremental layout for 100 new streaming entries completes in under 10ms
- **SC-007**: Memory usage for view-state stays under 50MB for 100,000 entries (excluding render cache)
- **SC-008**: Visible range calculation is O(log n) with respect to entry count
- **SC-009**: Hit-testing is O(log n) with respect to entry count
- **SC-010**: All existing tests continue to pass (no regressions)

## Assumptions

- Entry heights are stable for a given viewport width and expand/collapse state
- Entries are never removed, only appended (streaming model)
- Render cache can be rebuilt if evicted (no data loss, just performance cost)
- Mouse coordinates are provided relative to content area, not absolute screen position
- Viewport dimensions are available before render

## Out of Scope

The following are explicitly **NOT** part of this feature:

- **Session navigation UI**: How users switch between sessions (future feature)
- **Search highlighting**: Visual indicators for search matches (separate feature)
- **Keyboard shortcut changes**: No new keybindings introduced
- **New CLI flags**: No command-line interface changes

The view-state layer is purely a **data structure and layout computation layer**. Features that consume the view-state layer are separate specifications.

## Design Rationale

**Why a full view-state layer instead of simpler fixes?**

Simpler approaches (approximating line counts, clamping in render code, heuristic bounds) produce complex, hard-to-understand code. Evidence from current codebase:
- Blank viewport bug (scroll offset type mismatch)
- Hit-testing errors (no precomputed Y offsets)
- Layout instability on expand/collapse
- App is currently not usable for its primary purpose

A proper architectural solution provides:
- **Single source of truth** for layout measurements
- **Semantic scroll positions** that survive layout changes
- **O(log n) operations** via precomputed cumulative offsets
- **Clear invalidation rules** based on tracked state
- **Testable, understandable code** vs scattered workarounds
