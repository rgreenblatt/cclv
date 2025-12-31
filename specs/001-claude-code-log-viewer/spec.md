# Feature Specification: Claude Code Log Viewer TUI

**Feature Branch**: `001-claude-code-log-viewer`
**Created**: 2025-12-25
**Status**: Draft
**Input**: User description: "We need a TUI to tail a claude code jsonl logfile and present it in an accessible and useful way to users."

## Clarifications

### Session 2025-12-25

- Q: How should long messages be displayed? → A: Show summary by default, allow expand/collapse to view full content
- Q: How should conversations handle scrolling? → A: Each conversation pane must be independently scrollable
- Q: How should auto-scroll behave during live mode? → A: Auto-scroll by default; pause when user scrolls away; resume when user returns to bottom
- Q: What defines a "long message" for collapse threshold? → A: Messages exceeding 10 lines are collapsed by default
- Q: How should long lines be handled? → A: No line wrapping; horizontal scrolling with left/right arrow keys
- Q: What input sources should be supported? → A: File path AND stdin (piped input)
- Q: How should keyboard bindings be handled? → A: Configurable via enum mapping domain actions to keys
- Q: What should collapsed message summaries display? → A: First 3 lines + "(+N more lines)" indicator
- Q: How should model cost/token pricing be configured? → A: Single unified config file (`~/.config/cclv/config.toml`) with pricing section; hardcoded defaults when config absent
- Q: What programming language should the implementation use? → A: Rust with ratatui

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Monitor Live Agent Session (Priority: P1)

A developer is running Claude Code on a complex task and wants to observe the agent's progress in real-time. They launch the log viewer pointing to the active JSONL log file. The viewer displays the main agent's conversation as it happens, showing each message, tool call, and model being used. When the agent spawns subagents, new tabs automatically appear showing each subagent's activity.

**Why this priority**: This is the primary use case - developers need to understand what their agents are doing while they're doing it. Without live tailing, the tool would just be a static log reader.

**Independent Test**: Can be fully tested by starting Claude Code on a task, then launching the viewer pointing to the active log. Delivers immediate visibility into agent behavior.

**Acceptance Scenarios**:

1. **Given** a Claude Code session is running, **When** user launches the viewer pointing to the active log file, **Then** they see the conversation updating in real-time without manual refresh
2. **Given** JSONL data is piped to stdin, **When** user launches the viewer without a file argument, **Then** the viewer reads from stdin and displays the session
3. **Given** the viewer is showing a live session, **When** the main agent spawns a subagent, **Then** a new tab appears within 1 second showing the subagent's prompt and ongoing conversation
4. **Given** the viewer is showing a live session, **When** the main agent makes a tool call, **Then** the tool name and parameters are displayed, and the result appears when available
5. **Given** the viewer is showing a live session, **When** the model information is present in the log, **Then** the model name (e.g., "claude-opus-4-5-20251101") is displayed in the header
6. **Given** live mode with auto-scroll active, **When** new messages arrive, **Then** the view scrolls to show the latest content
7. **Given** live mode, **When** user scrolls up to read earlier content, **Then** auto-scroll pauses and a "new messages" indicator appears
8. **Given** auto-scroll is paused, **When** user clicks the indicator or scrolls to bottom, **Then** auto-scroll resumes

---

### User Story 2 - Analyze Completed Session (Priority: P1)

A developer wants to review a previous Claude Code session to understand what happened, debug an issue, or learn from the agent's approach. They open a completed log file and can scroll through the entire conversation, switch between subagent tabs, and search for specific keywords.

**Why this priority**: Equally critical as live viewing - post-mortem analysis is essential for debugging, learning, and improving prompts.

**Independent Test**: Can be fully tested by opening a completed JSONL log file. Delivers full session reconstruction and analysis capability.

**Acceptance Scenarios**:

1. **Given** a completed JSONL log file, **When** user opens it in the viewer, **Then** the entire session is loaded and navigable
2. **Given** a loaded session with subagents, **When** user clicks on a subagent tab, **Then** they see that subagent's full conversation including its initial prompt
3. **Given** a loaded session, **When** user searches for "error", **Then** all matches are highlighted and navigable with next/previous controls
4. **Given** a loaded session with markdown content, **When** viewing a message with code blocks, **Then** the code is syntax-highlighted and formatted properly
5. **Given** a conversation with a long message (>10 lines), **When** viewing, **Then** the message displays first 3 lines + "(+N more lines)" with an expand control
6. **Given** a collapsed message, **When** user activates expand, **Then** the full message content is revealed
7. **Given** an expanded message, **When** user activates collapse, **Then** the message returns to summary form

---

### User Story 3 - Review Usage Statistics (Priority: P2)

A developer wants to understand the cost and efficiency of a session. They check the stats panel to see total tokens used, estimated cost, tool usage breakdown, and number of subagents. They can filter these stats to see just the main agent, a specific subagent, or the global total.

**Why this priority**: Cost visibility is important for budget management and optimization, but secondary to core viewing functionality.

**Independent Test**: Can be fully tested by loading any log file with tool usage and viewing the stats panel. Delivers cost awareness and usage insights.

**Acceptance Scenarios**:

1. **Given** a loaded session, **When** user opens the stats panel, **Then** they see total input tokens, output tokens, and estimated cost
2. **Given** stats are displayed, **When** user filters by "Main Agent", **Then** only main agent statistics are shown (excluding subagent activity)
3. **Given** stats are displayed, **When** user views tool breakdown, **Then** they see each tool name with count of invocations (e.g., "Read: 15, Write: 8, Bash: 12")
4. **Given** a session with multiple subagents, **When** user filters by a specific subagent ID, **Then** only that subagent's statistics are shown

---

### User Story 4 - Navigate Efficiently with Keyboard (Priority: P2)

A power user wants to navigate the log viewer entirely with keyboard shortcuts. They can switch between panes, scroll through conversations, switch subagent tabs, toggle the stats panel, and search - all without touching the mouse.

**Why this priority**: Keyboard navigation is essential for developer tools but secondary to core functionality.

**Independent Test**: Can be fully tested by loading any session and performing all operations using only keyboard. Delivers efficient expert workflow.

**Acceptance Scenarios**:

1. **Given** the viewer is open, **When** user presses Tab, **Then** focus moves between main pane, subagent pane, and stats panel
2. **Given** focus is on subagent pane, **When** user presses left/right arrow or number keys, **Then** they switch between subagent tabs
3. **Given** focus is on a conversation pane, **When** user presses j/k or up/down arrows, **Then** they scroll through messages
4. **Given** any state, **When** user presses "/" or Ctrl+F, **Then** the search input is activated
5. **Given** search results exist, **When** user presses n/N, **Then** they navigate to next/previous match
6. **Given** focus is on a collapsed message, **When** user presses Enter or Space, **Then** the message expands
7. **Given** focus is on an expanded message, **When** user presses Enter or Space, **Then** the message collapses
8. **Given** a message with long lines extends beyond viewport, **When** user presses left/right arrows, **Then** the view scrolls horizontally to reveal hidden content

---

### User Story 5 - Search Within Conversations (Priority: P2)

A developer is looking for a specific piece of information in a long session. They open search, type their query, and results are highlighted across all conversations (main agent and subagents). They can jump between matches and see which agent/message contains each match.

**Why this priority**: Search is important for navigating large logs but builds on top of the core viewing capability.

**Independent Test**: Can be fully tested by loading a session with known content and searching for it. Delivers efficient information retrieval.

**Acceptance Scenarios**:

1. **Given** a loaded session, **When** user searches for a term, **Then** all occurrences are highlighted in the visible pane
2. **Given** search is active, **When** matches exist in subagent conversations, **Then** the subagent tabs indicate they contain matches (visual indicator)
3. **Given** search results exist, **When** user navigates to a match in a subagent tab, **Then** that tab is automatically activated and scrolled to the match
4. **Given** search is active, **When** user clears the search, **Then** all highlighting is removed

---

### Edge Cases

- What happens when the JSONL file is malformed or contains invalid JSON lines? Display error inline and continue parsing valid lines
- How does system handle extremely large log files (>1GB)? Load entire file into memory; virtualize rendering (only render visible messages plus ±20 buffer). **Measurable criteria**: Startup <5 seconds for 1GB file, UI remains responsive at 60fps during navigation
- What happens when a subagent is referenced but its spawn event wasn't logged? Show placeholder tab with agentId, entry count, and "[incomplete data]" indicator
- How does the viewer handle log files with no subagents? Hide or minimize the subagent pane; show main agent only
- What happens when following a live log and the file is deleted? Show error notification and stop following
- How does the system handle rapid updates (many lines per second)? Batch updates at 16ms intervals (matching 60fps frame budget); accumulate entries between frames without dropping any

## Requirements *(mandatory)*

### Functional Requirements

**Core Viewing**
- **FR-001**: System MUST display the main agent's conversation in a dedicated pane
- **FR-002**: System MUST show the model name being used by each agent in a visible header
- **FR-003**: System MUST display subagent conversations in a tabbed pane, one tab per subagent
- **FR-004**: System MUST create a new tab when a subagent spawn event is detected in the log
- **FR-005**: System MUST keep subagent tabs open after the subagent completes
- **FR-006**: System MUST display each subagent's initial prompt in their tab

**Message Display & Collapsing**
- **FR-031**: System MUST display messages exceeding 10 lines in collapsed form by default, showing first 3 lines + "(+N more lines)" indicator
- **FR-032**: System MUST allow users to expand collapsed messages to view full content
- **FR-033**: System MUST allow users to collapse expanded messages back to summary form
- **FR-034**: Each conversation pane MUST be independently scrollable (vertical)
- **FR-039**: System MUST NOT wrap long lines; lines display at full length
- **FR-040**: System MUST support horizontal scrolling within message views using left/right arrow keys

**Auto-Scroll Behavior**
- **FR-035**: When following a live log, system MUST auto-scroll to show new content by default
- **FR-036**: When user manually scrolls away from the bottom during live mode, system MUST pause auto-scroll
- **FR-037**: System MUST provide visual indicator when auto-scroll is paused (e.g., "New messages below" or scroll-to-bottom button)
- **FR-038**: When user scrolls back to bottom or activates scroll-to-bottom, system MUST resume auto-scroll

**Log Handling**
- **FR-007**: System MUST support following a live JSONL log file (tail -f behavior)
- **FR-008**: System MUST support viewing completed/closed JSONL log files
- **FR-009**: System MUST parse Claude Code JSONL format to extract conversations, tool calls, and metadata
- **FR-010**: System MUST handle malformed JSON lines gracefully without crashing
- **FR-041**: System MUST accept JSONL input from stdin (piped input) as alternative to file path
- **FR-042**: When reading from stdin, system MUST support both streaming (live) and complete (EOF) modes

**Search**
- **FR-011**: System MUST provide text search across all conversations
- **FR-012**: System MUST highlight all search matches in the visible pane
- **FR-013**: System MUST allow navigation between search matches (next/previous)
- **FR-014**: System MUST indicate which tabs contain search matches

**Statistics**
- **FR-015**: System MUST track and display input token count
- **FR-016**: System MUST track and display output token count
- **FR-017**: System MUST calculate and display estimated cost based on token usage
- **FR-018**: System MUST track and display tool usage counts grouped by tool name
- **FR-019**: System MUST track and display number of subagents spawned
- **FR-020**: System MUST support filtering statistics by: all (global), main agent only, specific subagent
- **FR-046**: System MUST use hardcoded default model pricing for cost estimation
- **FR-047**: System MAY read model pricing from an optional configuration file, overriding defaults when present

**Rendering**
- **FR-021**: System MUST render markdown content (headings, bold, italic, code blocks, lists)
- **FR-022**: System MUST apply syntax highlighting to code blocks
- **FR-023**: System MUST use a visually appealing color scheme with distinct colors for different message types
- **FR-024**: System MUST use colors to distinguish between user messages, assistant messages, and tool calls

**Navigation & Keyboard Configuration**
- **FR-025**: System MUST support full keyboard navigation between all panes and controls
- **FR-026**: System MUST provide visible keyboard shortcut hints
- **FR-027**: System MUST support scrolling within conversation panes
- **FR-043**: System MUST define keyboard bindings via a configurable action-to-key mapping
- **FR-044**: Domain actions (e.g., ScrollUp, ScrollDown, ExpandMessage, NextTab) MUST be enumerated and mappable to keys
- **FR-045**: Default key bindings MUST be provided; users MAY override via configuration

**Performance**
- **FR-028**: System MUST maintain responsive UI (60fps) even with large log files
- **FR-029**: System MUST use virtualized rendering for long conversations (only render visible content)
- **FR-030**: System MUST load log files into memory for fast navigation and search

### Key Entities

- **Session**: A complete Claude Code execution represented by a single JSONL file; contains main agent and zero or more subagents
- **Agent**: Either the main agent or a subagent; has a model, conversation messages, and tool invocations
- **Message**: A single conversation turn (user, assistant, or tool result); may contain markdown text
- **Tool Invocation**: A tool call with name, parameters, and result; tracked for statistics
- **Statistics**: Aggregated metrics (tokens, cost, tool counts) that can be scoped to agent level
- **KeyAction**: Enumerated domain-level actions that can be mapped to configurable key bindings:
  - Scrolling: ScrollUp, ScrollDown, ScrollLeft, ScrollRight, PageUp, PageDown, ScrollToTop, ScrollToBottom
  - Focus: FocusMain, FocusSubagent, FocusStats, CycleFocus
  - Tabs: NextTab, PrevTab, SelectTab(1-9)
  - Messages: ExpandMessage, CollapseMessage, ToggleExpand
  - Search: StartSearch, SubmitSearch, CancelSearch, NextMatch, PrevMatch
  - Stats: ToggleStats, FilterGlobal, FilterMainAgent, FilterSubagent
  - Live mode: ToggleAutoScroll, ScrollToLatest
  - Application: Quit, Help, Refresh

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Users can view live logs with less than 500ms latency from log write to display
- **SC-002**: Users can open and navigate a 100MB log file without perceivable lag (UI remains responsive)
- **SC-003**: Search results appear within 1 second for log files up to 50MB
- **SC-004**: Application startup to first render takes less than 1 second for typical log files (<10MB)
- **SC-005**: Memory usage proportional to file size (v1 loads entire file; future versions may optimize)
- **SC-006**: Users can perform all primary actions (view, navigate, search, view stats) using only keyboard
- **SC-007**: 90% of users can identify which model is being used and current tool being called within 5 seconds of viewing

## Technical Constraints

- **Language**: Rust (latest stable)
- **TUI Framework**: ratatui
- **Distribution**: Single static binary

## Assumptions

- Claude Code JSONL format follows a consistent structure with message types, agent identifiers, and token counts
- Token pricing: hardcoded defaults with optional config file override (config file is not required)
- The application will run in modern terminal emulators with 256-color or true-color support
- Users have basic familiarity with TUI navigation patterns (vim-like or standard arrow key navigation)
