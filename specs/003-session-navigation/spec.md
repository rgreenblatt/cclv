# Feature Specification: Session Navigation

**Feature Branch**: `003-session-navigation`
**Created**: 2025-01-09
**Status**: Draft
**Input**: User description: "We want to be able to navigate the possibly multiple concatenated sessions in a single jsonl file. For this, we want a modal window that displays a list of sessions in the same file and allows the user to select one. Live tailing must only be available when the last session of the logfile is active. Stats must allow to aggregate by each sessions' main agent, each sub agent, each session (main+subagents) and all combined sessions (all agents)"

## User Scenarios & Testing *(mandatory)*

### User Story 1 - View Session List (Priority: P1)

As a user viewing a JSONL log file containing multiple concatenated sessions, I want to open a modal window that displays all sessions in the file so I can understand what conversation history exists and select a specific session to examine.

**Why this priority**: This is the foundational capability - without seeing what sessions exist, users cannot navigate between them. This enables all other session navigation functionality.

**Independent Test**: Can be fully tested by opening a JSONL file with multiple sessions and pressing the session list hotkey. Delivers immediate value by showing the user the scope of their log file.

**Acceptance Scenarios**:

1. **Given** a JSONL file with 3 concatenated sessions, **When** the user presses the session list hotkey, **Then** a modal window appears showing all 3 sessions with identifying information (session ID, timestamp, duration, message count)
2. **Given** the session list modal is open, **When** the user views the list, **Then** the currently active session is visually highlighted
3. **Given** a JSONL file with only 1 session, **When** the user opens the session list modal, **Then** the modal shows a single entry (the feature remains accessible even for single-session files)

---

### User Story 2 - Select and Navigate to Session (Priority: P1)

As a user viewing the session list modal, I want to select a session and have the main view switch to display that session's conversation so I can examine historical sessions without losing my place.

**Why this priority**: Navigation is the core purpose of this feature - viewing the list alone provides limited value without the ability to actually switch sessions.

**Independent Test**: Can be fully tested by opening the session list, selecting a different session, and verifying the main conversation view updates to show the selected session's content.

**Acceptance Scenarios**:

1. **Given** the session list modal is open showing multiple sessions, **When** the user selects a session using keyboard navigation and confirms, **Then** the modal closes and the main view displays the selected session's conversation
2. **Given** the session list modal is open, **When** the user presses Escape or the dismiss hotkey, **Then** the modal closes without changing the active session
3. **Given** a session is selected from the list, **When** the main view updates, **Then** if this is the first visit to that session the scroll position resets to the beginning; if the user previously viewed this session, scroll position is restored to where they left off

---

### User Story 3 - Live Tailing Behavior (Priority: P2)

As a user monitoring an active Claude Code session, I want live tailing to be available only when viewing the last (most recent) session so that new entries append correctly without confusion when viewing historical sessions.

**Why this priority**: This is critical for correct behavior but depends on session navigation being functional first. Incorrect tailing on historical sessions would corrupt the viewing experience.

**Independent Test**: Can be tested by switching between the last session and a historical session and verifying live tail behavior changes accordingly.

**Acceptance Scenarios**:

1. **Given** a file with 3 sessions where session 3 is actively being written to, **When** the user is viewing session 3, **Then** live tailing is enabled and new entries appear automatically
2. **Given** a file with 3 sessions where session 3 is actively being written to, **When** the user navigates to session 1 or 2, **Then** live tailing is disabled and the view remains static
3. **Given** the user is viewing a historical session (not the last), **When** the user returns to the last session, **Then** live tailing resumes (if enabled in settings)
4. **Given** live tailing is disabled for a historical session, **When** the session list indicates which is the "active" session, **Then** the user understands why tailing is not available

---

### User Story 4 - Session-Level Statistics (Priority: P2)

As a user analyzing Claude Code usage, I want statistics to show aggregated data at multiple levels: by individual agent (main or sub), by session (all agents in a session combined), and across all sessions so I can understand usage patterns at different granularities.

**Why this priority**: Statistics provide analytical value but are secondary to the core navigation functionality. Users need to navigate sessions before analyzing them in detail.

**Independent Test**: Can be tested by opening the stats view and verifying aggregation levels are available and show different values based on the selected scope.

**Acceptance Scenarios**:

1. **Given** a stats view is open, **When** the user selects "by agent" aggregation, **Then** stats show breakdown for each main agent and each subagent separately
2. **Given** a stats view is open, **When** the user selects "by session" aggregation, **Then** stats show combined totals for each session (main agent + all subagents within that session)
3. **Given** a stats view is open, **When** the user selects "all combined" aggregation, **Then** stats show totals across all sessions and all agents in the file
4. **Given** multiple sessions with varying numbers of subagents, **When** viewing "by agent" stats, **Then** each subagent from each session is listed individually with its parent session indicated

---

### User Story 5 - Session Identification (Priority: P3)

As a user viewing the session list, I want each session to display meaningful identifying information (message count, main agent info) so I can quickly find the session I want to examine.

**Why this priority**: While helpful for usability, the feature works with minimal identification (session 1, 2, 3). Enhanced identification improves the experience but isn't required for core functionality.

**Independent Test**: Can be tested by viewing the session list and verifying each session displays useful identifying metadata.

**Acceptance Scenarios**:

1. **Given** the session list modal is open, **When** viewing session entries, **Then** each entry shows: session number and message count
2. **Given** sessions of varying lengths, **When** viewing the session list, **Then** session number and message count are displayed to help distinguish sessions
3. **Given** sessions with different main conversation topics, **When** possible to derive from data, **Then** a brief preview or initial user message excerpt is shown

---

### Edge Cases

- What happens when a JSONL file has malformed session boundaries? System treats ambiguous boundaries as a single session and logs a warning.
- How does the system handle a file with zero sessions (empty or invalid)? Display an appropriate "no sessions found" message.
- What happens when viewing a historical session and the file is truncated or modified externally? The historical session view remains stable; changes only affect live tailing of the last session.
- What happens when a session has no subagents? Stats "by agent" shows only the main agent for that session.
- How are session boundaries detected in concatenated JSONL files? Session boundaries are detected by comparing the `session_id` field of consecutive entries - when the UUID changes, a new session begins. There are no explicit start/end markers; instead, each entry carries its `session_id` and the parser detects boundaries via UUID change comparison.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: System MUST detect and parse multiple sessions within a single JSONL file by comparing the `session_id` field of consecutive entries (a new session begins when the UUID changes). Note: Multi-session detection is already implemented in the existing parser.
- **FR-002**: System MUST provide a modal window accessible via keyboard shortcut (default: `S`) that displays all sessions in the current file
- **FR-003**: System MUST allow users to select a session from the modal using keyboard navigation (up/down arrows, Enter to confirm, Escape to cancel)
- **FR-004**: System MUST update the main conversation view to display the selected session's content when a session is selected
- **FR-005**: System MUST visually indicate the currently active session in the session list modal
- **FR-006**: System MUST disable live tailing when viewing any session other than the last (most recent) session in the file
- **FR-007**: System MUST re-enable live tailing (if previously enabled) when the user navigates back to the last session
- **FR-008**: System MUST provide statistics aggregation at four levels via `StatsFilter`: MainAgent(SessionId) for a specific session's main agent, Subagent(AgentId) for a specific subagent, Session(SessionId) for per-session totals, and AllSessionsCombined (renamed from Global) for cross-session totals
- **FR-009**: System MUST display session metadata in the list including: session number, start timestamp, and message count
- **FR-010**: System MUST preserve the user's scroll position and view state for each session when switching between sessions. On first visit to a session, scroll position starts at the beginning; on subsequent visits, scroll position is restored to where the user left off
- **FR-011**: System MUST scope subagent tabs to the currently viewed session (switching sessions updates the available subagent tabs)
- **FR-012**: System MUST display current session indicator in status bar as "Session N/M" (e.g., "Session 2/3") showing position and total count

### Key Entities

- **Session**: A distinct Claude Code conversation with a unique session UUID, containing messages from a main agent and potentially multiple subagents. Key attributes: session_id, start_time, end_time, message_count, main_agent_id, subagent_ids
- **Agent**: An entity that produces messages within a session. Can be a main agent or subagent. Key attributes: agent_id, agent_type (main/sub), parent_session_id
- **SessionBoundary**: Not an explicit marker but a detected transition point where the `session_id` field changes between consecutive entries. The parser already detects these boundaries automatically.
- **StatsAggregation**: A computed view of statistics that can be scoped to: single agent, single session (all its agents), or all sessions combined

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Users can view and switch between sessions in under 2 seconds from pressing the session list hotkey to viewing the selected session
- **SC-002**: Session list modal displays correctly for files containing 1 to 100+ sessions without UI degradation
- **SC-003**: Live tailing state correctly reflects session context 100% of the time (enabled only on last session)
- **SC-004**: Statistics accurately aggregate data across the four defined scopes with consistent totals (sum of agents = session total, sum of sessions = combined total)
- **SC-005**: Users can identify the session they want from the list within 5 seconds for files with up to 20 sessions (based on displayed metadata quality)
- **SC-006**: Session switching preserves view state, allowing users to return to a previous session and continue from where they left off

## Assumptions

- JSONL files follow Claude Code's standard logging format with `session_id` field in each entry
- Session boundaries are reliably detectable via `session_id` field changes (no interleaved sessions)
- The existing parser already detects multi-session files and creates separate `SessionViewState` objects
- Entries missing `session_id` fall back to `"unknown-session"` constant
- The existing statistics infrastructure can be extended to support scoped aggregation
- Users primarily use keyboard navigation in the TUI (consistent with existing cclv design)
- Sessions are stored chronologically in the JSONL file (session N+1 always comes after session N)
- Live tailing uses stdin streaming (`tail -c+0 -f file.jsonl | cclv`), not file watching

## Clarifications

### Session 2025-01-09

- Q: How are session boundaries detected? → A: By comparing `session_id` field of consecutive entries; when the UUID changes, a new session begins. Multi-session detection is already implemented in the parser (`view_state/log.rs:78-118`) - no explicit start/end markers exist.
- Q: What are the stats aggregation levels? → A: 4 levels: MainAgent(SessionId), Subagent(AgentId), Session(SessionId), AllSessionsCombined. MainAgent now requires SessionId to specify which session's main agent. Existing `StatsFilter::Global` renamed to `AllSessionsCombined`, new `Session(SessionId)` constructor added.
- Q: How does live tailing work with multi-session files? → A: Stdin streaming only, no file watching. Users use `tail -c+0 -f file.jsonl | cclv` to dump full file and follow. FileSource remains read-once; live tailing requires StdinSource.
- Q: What is the session list modal hotkey? → A: Default binding is `S` (Shift+S). Users can customize via config (keybindings are configurable).
- Q: Are subagent tabs scoped to the viewed session? → A: Yes, session-scoped. Subagent tabs show only the currently viewed session's subagents, not all sessions' subagents.
- Q: How is current session indicated in main UI? → A: Status bar shows "Session N/M" (e.g., "Session 2/3") alongside existing indicators (LIVE, Wrap mode).
- Q: When switching to a session, what should happen to scroll position? → A: Preserve on return - reset to beginning on first visit, restore saved position on subsequent visits. This resolves the apparent conflict between US2:AC3 (reset) and FR-010/SC-006 (preserve).
