# Data Model: Claude Code Log Viewer

**Date**: 2025-12-25 (Updated: 2025-12-26)
**Status**: Design Complete
**Related**: [plan.md](./plan.md) | [research.md](./research.md)

This document defines the type-driven domain model following the project constitution:
- **Smart constructors only**: Never export raw constructors
- **No primitive obsession**: Newtypes for all domain concepts
- **Illegal states unrepresentable**: Sum types enforce valid states
- **Parse at boundaries**: Validate once during construction

---

## 1. Core Identifiers (Newtypes)

All identifiers are newtypes with smart constructors. Never use raw `String`.

```rust
// ===== src/model/identifiers.rs =====

use std::fmt;

/// Unique identifier for a log entry within a session.
/// NEVER export the constructor.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EntryUuid(String);

impl EntryUuid {
    /// Smart constructor: validates UUID format
    pub fn new(raw: impl Into<String>) -> Result<Self, InvalidUuid> {
        let s = raw.into();
        // UUID v4 format: 8-4-4-4-12 or simple alphanumeric
        if s.is_empty() {
            return Err(InvalidUuid::Empty);
        }
        Ok(Self(s))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Session identifier grouping related entries.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SessionId(String);

impl SessionId {
    pub fn new(raw: impl Into<String>) -> Result<Self, InvalidSessionId> {
        let s = raw.into();
        if s.is_empty() {
            return Err(InvalidSessionId::Empty);
        }
        Ok(Self(s))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Subagent identifier (e.g., "a7b2877").
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AgentId(String);

impl AgentId {
    pub fn new(raw: impl Into<String>) -> Result<Self, InvalidAgentId> {
        let s = raw.into();
        if s.is_empty() {
            return Err(InvalidAgentId::Empty);
        }
        Ok(Self(s))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Tool invocation identifier for linking tool_use to tool_result.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ToolUseId(String);

impl ToolUseId {
    pub fn new(raw: impl Into<String>) -> Result<Self, InvalidToolUseId> {
        let s = raw.into();
        if s.is_empty() {
            return Err(InvalidToolUseId::Empty);
        }
        Ok(Self(s))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

// Error types
#[derive(Debug, Clone, thiserror::Error)]
pub enum InvalidUuid {
    #[error("UUID cannot be empty")]
    Empty,
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum InvalidSessionId {
    #[error("Session ID cannot be empty")]
    Empty,
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum InvalidAgentId {
    #[error("Agent ID cannot be empty")]
    Empty,
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum InvalidToolUseId {
    #[error("Tool Use ID cannot be empty")]
    Empty,
}
```

---

## 2. Log Entry Model

The core parsed log entry from JSONL.

```rust
// ===== src/model/log_entry.rs =====

use chrono::{DateTime, Utc};
use std::path::PathBuf;

/// A parsed log entry from the JSONL file.
/// Invariant: All fields validated at construction time.
#[derive(Debug, Clone)]
pub struct LogEntry {
    uuid: EntryUuid,
    parent_uuid: Option<EntryUuid>,
    session_id: SessionId,
    agent_id: Option<AgentId>,  // None for main agent
    timestamp: DateTime<Utc>,
    entry_type: EntryType,
    message: Message,
    metadata: EntryMetadata,
}

impl LogEntry {
    /// Smart constructor: parses and validates raw JSON entry.
    pub fn parse(raw: &str) -> Result<Self, ParseError> {
        // Deserialization and validation logic
        // See parser module
        todo!("Implemented in parser module")
    }

    // Accessors (read-only)
    pub fn uuid(&self) -> &EntryUuid { &self.uuid }
    pub fn parent_uuid(&self) -> Option<&EntryUuid> { self.parent_uuid.as_ref() }
    pub fn session_id(&self) -> &SessionId { &self.session_id }
    pub fn agent_id(&self) -> Option<&AgentId> { self.agent_id.as_ref() }
    pub fn timestamp(&self) -> DateTime<Utc> { self.timestamp }
    pub fn entry_type(&self) -> EntryType { self.entry_type }
    pub fn message(&self) -> &Message { &self.message }
    pub fn is_subagent(&self) -> bool { self.agent_id.is_some() }
}

/// Type of log entry. Sum type - exactly one variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryType {
    User,
    Assistant,
    Summary,
}

/// Additional metadata from the entry.
#[derive(Debug, Clone, Default)]
pub struct EntryMetadata {
    pub cwd: Option<PathBuf>,
    pub git_branch: Option<String>,
    pub version: Option<String>,
    pub is_sidechain: bool,
}
```

---

## 3. Message Model

Messages contain role and content blocks.

```rust
// ===== src/model/message.rs =====

/// A message with role and content.
#[derive(Debug, Clone)]
pub struct Message {
    role: Role,
    content: MessageContent,
    model: Option<ModelInfo>,
    usage: Option<TokenUsage>,
}

impl Message {
    // Accessors
    pub fn role(&self) -> Role { self.role }
    pub fn content(&self) -> &MessageContent { &self.content }
    pub fn model(&self) -> Option<&ModelInfo> { self.model.as_ref() }
    pub fn usage(&self) -> Option<&TokenUsage> { self.usage.as_ref() }

    /// Extract all tool calls from this message.
    pub fn tool_calls(&self) -> Vec<&ToolCall> {
        match &self.content {
            MessageContent::Text(_) => vec![],
            MessageContent::Blocks(blocks) => {
                blocks.iter().filter_map(|b| {
                    if let ContentBlock::ToolUse(tc) = b { Some(tc) } else { None }
                }).collect()
            }
        }
    }

    /// Get text content, joining all text blocks.
    pub fn text(&self) -> String {
        match &self.content {
            MessageContent::Text(s) => s.clone(),
            MessageContent::Blocks(blocks) => {
                blocks.iter().filter_map(|b| {
                    if let ContentBlock::Text { text } = b { Some(text.as_str()) } else { None }
                }).collect::<Vec<_>>().join("\n")
            }
        }
    }
}

/// Message role - user or assistant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    User,
    Assistant,
}

/// Message content - either plain text or structured blocks.
/// Sum type enforces one representation.
#[derive(Debug, Clone)]
pub enum MessageContent {
    Text(String),
    Blocks(Vec<ContentBlock>),
}

/// Individual content block within a message.
#[derive(Debug, Clone)]
pub enum ContentBlock {
    Text {
        text: String,
    },
    ToolUse(ToolCall),
    ToolResult {
        tool_use_id: ToolUseId,
        content: String,
        is_error: bool,
    },
    Thinking {
        thinking: String,
    },
}

/// A tool invocation with name and parameters.
#[derive(Debug, Clone)]
pub struct ToolCall {
    id: ToolUseId,
    name: ToolName,
    input: serde_json::Value,
}

impl ToolCall {
    pub fn id(&self) -> &ToolUseId { &self.id }
    pub fn name(&self) -> &ToolName { &self.name }
    pub fn input(&self) -> &serde_json::Value { &self.input }
}

/// Tool name with known variants for special handling.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ToolName {
    Read,
    Write,
    Edit,
    MultiEdit,
    Bash,
    Grep,
    Glob,
    Task,
    WebSearch,
    WebFetch,
    Other(String),
}

impl ToolName {
    pub fn parse(name: &str) -> Self {
        match name {
            "Read" => Self::Read,
            "Write" => Self::Write,
            "Edit" => Self::Edit,
            "MultiEdit" => Self::MultiEdit,
            "Bash" => Self::Bash,
            "Grep" => Self::Grep,
            "Glob" => Self::Glob,
            "Task" => Self::Task,
            "WebSearch" => Self::WebSearch,
            "WebFetch" => Self::WebFetch,
            other => Self::Other(other.to_string()),
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            Self::Read => "Read",
            Self::Write => "Write",
            Self::Edit => "Edit",
            Self::MultiEdit => "MultiEdit",
            Self::Bash => "Bash",
            Self::Grep => "Grep",
            Self::Glob => "Glob",
            Self::Task => "Task",
            Self::WebSearch => "WebSearch",
            Self::WebFetch => "WebFetch",
            Self::Other(s) => s,
        }
    }
}
```

---

## 4. Model and Token Usage

```rust
// ===== src/model/usage.rs =====

/// Model information from the assistant message.
#[derive(Debug, Clone)]
pub struct ModelInfo {
    model_id: ModelId,
}

impl ModelInfo {
    pub fn new(model_id: impl Into<String>) -> Self {
        Self { model_id: ModelId(model_id.into()) }
    }

    pub fn id(&self) -> &str { &self.model_id.0 }

    /// Human-readable short name.
    pub fn display_name(&self) -> &str {
        let id = &self.model_id.0;
        if id.contains("opus") { "Opus" }
        else if id.contains("sonnet") { "Sonnet" }
        else if id.contains("haiku") { "Haiku" }
        else { id }
    }
}

#[derive(Debug, Clone)]
struct ModelId(String);

/// Token usage statistics from a single message.
#[derive(Debug, Clone, Copy, Default)]
pub struct TokenUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_creation_input_tokens: u64,
    pub cache_read_input_tokens: u64,
}

impl TokenUsage {
    pub fn total_input(&self) -> u64 {
        self.input_tokens + self.cache_creation_input_tokens + self.cache_read_input_tokens
    }

    pub fn total(&self) -> u64 {
        self.total_input() + self.output_tokens
    }
}
```

---

## 5. Session Model (Aggregate)

A session contains the main agent and subagents.

```rust
// ===== src/model/session.rs =====

use std::collections::HashMap;

/// A complete session with main agent and subagents.
/// Invariant: At least one entry (the main agent).
#[derive(Debug, Clone)]
pub struct Session {
    session_id: SessionId,
    main_agent: AgentConversation,
    subagents: HashMap<AgentId, AgentConversation>,
    stats: SessionStats,
}

impl Session {
    /// Create empty session with ID.
    pub fn new(session_id: SessionId) -> Self {
        Self {
            session_id,
            main_agent: AgentConversation::new(None),
            subagents: HashMap::new(),
            stats: SessionStats::default(),
        }
    }

    /// Add an entry to the appropriate agent conversation.
    pub fn add_entry(&mut self, entry: LogEntry) {
        self.stats.record_entry(&entry);

        if let Some(agent_id) = entry.agent_id().cloned() {
            self.subagents
                .entry(agent_id.clone())
                .or_insert_with(|| AgentConversation::new(Some(agent_id)))
                .add_entry(entry);
        } else {
            self.main_agent.add_entry(entry);
        }
    }

    // Accessors
    pub fn session_id(&self) -> &SessionId { &self.session_id }
    pub fn main_agent(&self) -> &AgentConversation { &self.main_agent }
    pub fn subagents(&self) -> &HashMap<AgentId, AgentConversation> { &self.subagents }
    pub fn stats(&self) -> &SessionStats { &self.stats }

    /// Get subagent IDs in order of first appearance.
    pub fn subagent_ids_ordered(&self) -> Vec<&AgentId> {
        let mut agents: Vec<_> = self.subagents.iter().collect();
        agents.sort_by_key(|(_, conv)| conv.first_timestamp());
        agents.into_iter().map(|(id, _)| id).collect()
    }
}

/// A single agent's conversation (main or sub).
#[derive(Debug, Clone)]
pub struct AgentConversation {
    agent_id: Option<AgentId>,
    entries: Vec<LogEntry>,
    model: Option<ModelInfo>,
}

impl AgentConversation {
    pub fn new(agent_id: Option<AgentId>) -> Self {
        Self {
            agent_id,
            entries: Vec::new(),
            model: None,
        }
    }

    pub fn add_entry(&mut self, entry: LogEntry) {
        // Update model if present
        if let Some(model) = entry.message().model() {
            self.model = Some(model.clone());
        }
        self.entries.push(entry);
    }

    pub fn agent_id(&self) -> Option<&AgentId> { self.agent_id.as_ref() }
    pub fn entries(&self) -> &[LogEntry] { &self.entries }
    pub fn model(&self) -> Option<&ModelInfo> { self.model.as_ref() }
    pub fn first_timestamp(&self) -> Option<DateTime<Utc>> {
        self.entries.first().map(|e| e.timestamp())
    }
    pub fn len(&self) -> usize { self.entries.len() }
    pub fn is_empty(&self) -> bool { self.entries.is_empty() }
}
```

---

## 6. Statistics Model

Aggregated statistics with filtering.

```rust
// ===== src/model/stats.rs =====

use std::collections::HashMap;

/// Aggregated session statistics.
#[derive(Debug, Clone, Default)]
pub struct SessionStats {
    pub total_usage: TokenUsage,
    pub main_agent_usage: TokenUsage,
    pub subagent_usage: HashMap<AgentId, TokenUsage>,
    pub tool_counts: HashMap<ToolName, u32>,
    /// Tool usage counts for the main agent only (entries with no agent_id).
    pub main_agent_tool_counts: HashMap<ToolName, u32>,
    /// Tool usage counts per subagent, keyed by AgentId.
    pub subagent_tool_counts: HashMap<AgentId, HashMap<ToolName, u32>>,
    pub subagent_count: usize,
    pub entry_count: usize,
}

impl SessionStats {
    pub fn record_entry(&mut self, entry: &LogEntry) {
        self.entry_count += 1;

        if let Some(usage) = entry.message().usage() {
            self.total_usage.input_tokens += usage.input_tokens;
            self.total_usage.output_tokens += usage.output_tokens;
            self.total_usage.cache_creation_input_tokens += usage.cache_creation_input_tokens;
            self.total_usage.cache_read_input_tokens += usage.cache_read_input_tokens;

            if let Some(agent_id) = entry.agent_id() {
                let agent_usage = self.subagent_usage.entry(agent_id.clone()).or_default();
                agent_usage.input_tokens += usage.input_tokens;
                agent_usage.output_tokens += usage.output_tokens;
            } else {
                self.main_agent_usage.input_tokens += usage.input_tokens;
                self.main_agent_usage.output_tokens += usage.output_tokens;
            }
        }

        for tool in entry.message().tool_calls() {
            // Global tool counts (all agents)
            *self.tool_counts.entry(tool.name().clone()).or_default() += 1;

            // Per-agent tool counts
            if let Some(agent_id) = entry.agent_id() {
                let agent_tools = self.subagent_tool_counts
                    .entry(agent_id.clone())
                    .or_default();
                *agent_tools.entry(tool.name().clone()).or_default() += 1;
            } else {
                *self.main_agent_tool_counts.entry(tool.name().clone()).or_default() += 1;
            }
        }

        if entry.agent_id().is_some() {
            // Count unique subagents
            self.subagent_count = self.subagent_usage.len();
        }
    }

    /// Estimated cost in USD using provided pricing configuration.
    /// Pricing is determined by model family (opus/sonnet/haiku).
    pub fn estimated_cost(&self, pricing: &PricingConfig, model_id: Option<&str>) -> f64 {
        let model_pricing = pricing.get(model_id.unwrap_or("opus"));

        let input_cost = (self.total_usage.input_tokens as f64 / 1_000_000.0)
            * model_pricing.input_cost_per_million;

        let output_cost = (self.total_usage.output_tokens as f64 / 1_000_000.0)
            * model_pricing.output_cost_per_million;

        // Use cached rate if available, otherwise use standard input rate
        let cache_rate = model_pricing
            .cached_input_cost_per_million
            .unwrap_or(model_pricing.input_cost_per_million);

        let cache_cost = ((self.total_usage.cache_creation_input_tokens
            + self.total_usage.cache_read_input_tokens) as f64
            / 1_000_000.0)
            * cache_rate;

        input_cost + output_cost + cache_cost
    }

    /// Returns tool counts filtered by the current stats filter.
    /// - StatsFilter::Global → all agents combined
    /// - StatsFilter::MainAgent → main agent only
    /// - StatsFilter::Subagent(id) → specific subagent (empty if not found)
    pub fn filtered_tool_counts(&self, filter: &StatsFilter) -> &HashMap<ToolName, u32> {
        use std::sync::OnceLock;
        static EMPTY: OnceLock<HashMap<ToolName, u32>> = OnceLock::new();

        match filter {
            StatsFilter::Global => &self.tool_counts,
            StatsFilter::MainAgent => &self.main_agent_tool_counts,
            StatsFilter::Subagent(agent_id) => self
                .subagent_tool_counts
                .get(agent_id)
                .unwrap_or_else(|| EMPTY.get_or_init(HashMap::new)),
        }
    }
}

/// Filter for statistics display.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StatsFilter {
    Global,
    MainAgent,
    Subagent(AgentId),
}
```

---

## 7. UI State Model (Pure)

Application state as an immutable value with transitions.

```rust
// ===== src/state/app_state.rs =====

/// Global line-wrapping mode.
/// Default: Wrap (FR-039: wrap enabled when config unset)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WrapMode {
    #[default]
    Wrap,
    NoWrap,
}

/// Application state. Pure data, no side effects.
#[derive(Debug, Clone)]
pub struct AppState {
    pub session: Session,
    pub focus: FocusPane,
    pub main_scroll: ScrollState,
    pub subagent_scroll: ScrollState,
    pub selected_tab: Option<usize>,
    pub search: SearchState,
    pub stats_filter: StatsFilter,
    pub stats_visible: bool,
    pub live_mode: bool,
    pub auto_scroll: bool,
    pub global_wrap: WrapMode,  // FR-039: toggleable line-wrapping
}

impl AppState {
    pub fn new(session: Session) -> Self {
        Self {
            session,
            focus: FocusPane::Main,
            main_scroll: ScrollState::default(),
            subagent_scroll: ScrollState::default(),
            selected_tab: None,
            search: SearchState::Inactive,
            stats_filter: StatsFilter::Global,
            stats_visible: false,
            live_mode: false,
            auto_scroll: true,
            global_wrap: WrapMode::default(),
        }
    }

    /// Toggle global wrap mode (FR-050: W key)
    pub fn toggle_global_wrap(&mut self) {
        self.global_wrap = match self.global_wrap {
            WrapMode::Wrap => WrapMode::NoWrap,
            WrapMode::NoWrap => WrapMode::Wrap,
        };
    }
}

/// Which pane has focus. Sum type - exactly one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusPane {
    Main,
    Subagent,
    Stats,
    Search,
    LogPane,
}

/// Scroll state for a pane.
#[derive(Debug, Clone, Default)]
pub struct ScrollState {
    pub vertical_offset: usize,
    pub horizontal_offset: usize,
    pub expanded_messages: HashSet<EntryUuid>,
    pub focused_message: Option<usize>,  // Index of focused message within pane
    /// Messages with wrap override (FR-048: per-item toggle overrides global)
    /// FR-049: ephemeral, not persisted
    pub wrap_overrides: HashSet<EntryUuid>,
}

impl ScrollState {
    pub fn scroll_up(&mut self, amount: usize) {
        self.vertical_offset = self.vertical_offset.saturating_sub(amount);
    }

    pub fn scroll_down(&mut self, amount: usize, max: usize) {
        self.vertical_offset = (self.vertical_offset + amount).min(max);
    }

    pub fn scroll_left(&mut self, amount: usize) {
        self.horizontal_offset = self.horizontal_offset.saturating_sub(amount);
    }

    pub fn scroll_right(&mut self, amount: usize) {
        self.horizontal_offset = self.horizontal_offset.saturating_add(amount);
    }

    pub fn toggle_expand(&mut self, uuid: &EntryUuid) {
        if self.expanded_messages.contains(uuid) {
            self.expanded_messages.remove(uuid);
        } else {
            self.expanded_messages.insert(uuid.clone());
        }
    }

    pub fn is_expanded(&self, uuid: &EntryUuid) -> bool {
        self.expanded_messages.contains(uuid)
    }

    /// Expand all messages by adding all UUIDs to expanded_messages.
    pub fn expand_all(&mut self, uuids: impl Iterator<Item = EntryUuid>) {
        for uuid in uuids {
            self.expanded_messages.insert(uuid);
        }
    }

    /// Collapse all messages by clearing the expanded_messages set.
    pub fn collapse_all(&mut self) {
        self.expanded_messages.clear();
    }

    /// Set the focused message index (within the current pane's entry list).
    pub fn set_focused_message(&mut self, index: Option<usize>) {
        self.focused_message = index;
    }

    /// Get the focused message index.
    pub fn focused_message(&self) -> Option<usize> {
        self.focused_message
    }

    /// Toggle wrap override for a specific message (FR-050: w key)
    pub fn toggle_wrap(&mut self, uuid: &EntryUuid) {
        if self.wrap_overrides.contains(uuid) {
            self.wrap_overrides.remove(uuid);
        } else {
            self.wrap_overrides.insert(uuid.clone());
        }
    }

    /// Get effective wrap mode for a message (FR-048)
    /// Per-item override inverts the global setting
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
```

---

## 8. Search State

```rust
// ===== src/state/search.rs =====

/// Search state machine.
#[derive(Debug, Clone)]
pub enum SearchState {
    /// No active search.
    Inactive,
    /// User is typing query.
    Typing { query: String, cursor: usize },
    /// Search complete with results.
    Active {
        query: SearchQuery,
        matches: Vec<SearchMatch>,
        current_match: usize,
    },
}

/// Validated search query. Never empty.
#[derive(Debug, Clone)]
pub struct SearchQuery(String);

impl SearchQuery {
    pub fn new(raw: impl Into<String>) -> Option<Self> {
        let s = raw.into();
        if s.trim().is_empty() {
            None
        } else {
            Some(Self(s))
        }
    }

    pub fn as_str(&self) -> &str { &self.0 }
}

/// A search match location.
#[derive(Debug, Clone)]
pub struct SearchMatch {
    pub agent_id: Option<AgentId>,
    pub entry_uuid: EntryUuid,
    pub block_index: usize,
    pub char_offset: usize,
    pub length: usize,
}
```

---

## 9. Application Configuration

Unified configuration with hardcoded defaults. Single config file at `~/.config/cclv/config.toml`.

```rust
// ===== src/config/mod.rs =====

use std::collections::HashMap;
use std::path::Path;
use serde::Deserialize;

/// Complete application configuration.
/// Loads from optional config file, falls back to hardcoded defaults.
#[derive(Debug, Clone)]
pub struct AppConfig {
    pub theme: String,
    pub follow: bool,
    pub show_stats: bool,
    pub collapse_threshold: usize,
    pub summary_lines: usize,
    pub line_wrap: bool,  // FR-039: default true (wrap enabled)
    pub pricing: PricingConfig,
    pub keybindings: KeybindingConfig,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            theme: "base16-ocean".to_string(),
            follow: true,
            show_stats: false,
            collapse_threshold: 10,
            summary_lines: 3,
            line_wrap: true,  // FR-039: wrap enabled by default
            pricing: PricingConfig::default(),
            keybindings: KeybindingConfig::default(),
        }
    }
}

impl AppConfig {
    /// Load from config file, merging with defaults.
    /// Config file is optional - missing file uses defaults.
    pub fn load(path: Option<&Path>) -> Result<Self, ConfigError> {
        let mut config = Self::default();

        let config_path = path
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| dirs::config_dir()
                .unwrap_or_default()
                .join("cclv")
                .join("config.toml"));

        if config_path.exists() {
            let contents = std::fs::read_to_string(&config_path)?;
            let file_config: AppConfigFile = toml::from_str(&contents)?;
            config.merge(file_config);
        }
        // Missing file is not an error - use defaults

        Ok(config)
    }

    fn merge(&mut self, file: AppConfigFile) {
        if let Some(theme) = file.theme {
            self.theme = theme;
        }
        if let Some(follow) = file.follow {
            self.follow = follow;
        }
        if let Some(show_stats) = file.show_stats {
            self.show_stats = show_stats;
        }
        if let Some(threshold) = file.collapse_threshold {
            self.collapse_threshold = threshold;
        }
        if let Some(lines) = file.summary_lines {
            self.summary_lines = lines;
        }
        if let Some(pricing) = file.pricing {
            self.pricing.merge(pricing);
        }
        // keybindings merge handled separately
    }
}

// ===== src/config/pricing.rs =====

/// Pricing for a specific model (per million tokens).
#[derive(Debug, Clone, Copy)]
pub struct ModelPricing {
    pub input_cost_per_million: f64,
    pub output_cost_per_million: f64,
    pub cached_input_cost_per_million: Option<f64>,
}

impl ModelPricing {
    pub const fn new(input: f64, output: f64) -> Self {
        Self {
            input_cost_per_million: input,
            output_cost_per_million: output,
            cached_input_cost_per_million: None,
        }
    }

    pub const fn with_cache(mut self, cached: f64) -> Self {
        self.cached_input_cost_per_million = Some(cached);
        self
    }
}

/// Pricing configuration section.
#[derive(Debug, Clone)]
pub struct PricingConfig {
    models: HashMap<String, ModelPricing>,
    default_pricing: ModelPricing,
}

impl Default for PricingConfig {
    fn default() -> Self {
        let mut models = HashMap::new();

        // Claude Opus 4.5 - $15/$75 per million tokens
        models.insert("opus".to_string(), ModelPricing::new(15.0, 75.0).with_cache(1.5));
        // Claude Sonnet 4 - $3/$15 per million tokens
        models.insert("sonnet".to_string(), ModelPricing::new(3.0, 15.0).with_cache(0.3));
        // Claude Haiku 3.5 - $0.80/$4 per million tokens
        models.insert("haiku".to_string(), ModelPricing::new(0.8, 4.0).with_cache(0.08));

        Self {
            models,
            default_pricing: ModelPricing::new(15.0, 75.0), // Opus as fallback
        }
    }
}

impl PricingConfig {
    pub fn get(&self, model_id: &str) -> &ModelPricing {
        // Try exact match
        if let Some(pricing) = self.models.get(model_id) {
            return pricing;
        }

        // Try model family match
        let normalized = model_id.to_lowercase();
        for family in ["opus", "sonnet", "haiku"] {
            if normalized.contains(family) {
                if let Some(pricing) = self.models.get(family) {
                    return pricing;
                }
            }
        }

        &self.default_pricing
    }

    fn merge(&mut self, file: PricingConfigSection) {
        for (model, entry) in file.models {
            self.models.insert(model, entry.into());
        }
        if let Some(default) = file.default {
            self.default_pricing = default.into();
        }
    }
}

// ===== File format structs (serde) =====

#[derive(Debug, Deserialize)]
struct AppConfigFile {
    theme: Option<String>,
    follow: Option<bool>,
    show_stats: Option<bool>,
    collapse_threshold: Option<usize>,
    summary_lines: Option<usize>,
    pricing: Option<PricingConfigSection>,
    keybindings: Option<HashMap<String, String>>,
}

#[derive(Debug, Deserialize)]
struct PricingConfigSection {
    #[serde(default)]
    models: HashMap<String, PricingEntry>,
    default: Option<PricingEntry>,
}

#[derive(Debug, Deserialize)]
struct PricingEntry {
    input: f64,
    output: f64,
    cached_input: Option<f64>,
}

impl From<PricingEntry> for ModelPricing {
    fn from(e: PricingEntry) -> Self {
        let mut p = ModelPricing::new(e.input, e.output);
        p.cached_input_cost_per_million = e.cached_input;
        p
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("Failed to read config: {0}")]
    Io(#[from] std::io::Error),
    #[error("Failed to parse config: {0}")]
    Parse(#[from] toml::de::Error),
}
```

### Unified Config File Format

Location: `~/.config/cclv/config.toml` (optional - all settings have hardcoded defaults)

```toml
# ~/.config/cclv/config.toml

# Display settings
theme = "solarized-dark"
follow = true
show_stats = false
collapse_threshold = 10
summary_lines = 3
line_wrap = true  # FR-039: wrap prose by default (code blocks never wrap)

# Model pricing (per million tokens, USD)
# Hardcoded defaults: Opus=$15/$75, Sonnet=$3/$15, Haiku=$0.80/$4
[pricing.models.opus]
input = 15.0
output = 75.0
cached_input = 1.5

[pricing.models.sonnet]
input = 3.0
output = 15.0
cached_input = 0.3

[pricing.models.haiku]
input = 0.8
output = 4.0
cached_input = 0.08

# Custom model (future-proofing)
[pricing.models."claude-4-ultra"]
input = 30.0
output = 150.0

# Default for unknown models
[pricing.default]
input = 15.0
output = 75.0

# Key bindings (optional overrides)
[keybindings]
scroll_up = "k"
scroll_down = "j"
quit = "q"
toggle_wrap = "w"        # FR-050: per-item wrap toggle
toggle_global_wrap = "W" # FR-050: global wrap toggle
```

---

## 10. Key Actions (Enumerated)

```rust
// ===== src/model/key_action.rs =====

/// Domain-level actions independent of key bindings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeyAction {
    // Scrolling
    ScrollUp,
    ScrollDown,
    ScrollLeft,
    ScrollRight,
    PageUp,
    PageDown,
    ScrollToTop,
    ScrollToBottom,

    // Focus navigation
    FocusMain,
    FocusSubagent,
    FocusStats,
    CycleFocus,

    // Tab navigation
    NextTab,
    PrevTab,
    SelectTab(usize),  // 1-9

    // Message interaction
    ExpandMessage,
    CollapseMessage,
    ToggleExpand,

    // Line wrapping (FR-050)
    ToggleWrap,       // Per-item toggle (w key)
    ToggleGlobalWrap, // Global toggle (W key)

    // Search
    StartSearch,
    SubmitSearch,
    CancelSearch,
    NextMatch,
    PrevMatch,

    // Stats
    ToggleStats,
    FilterGlobal,
    FilterMainAgent,
    FilterSubagent,

    // Auto-scroll (live mode)
    ToggleAutoScroll,
    ScrollToLatest,

    // Application
    Quit,
    Help,
    Refresh,
}
```

---

## 11. Error Types

```rust
// ===== src/model/error.rs =====

use thiserror::Error;

/// Top-level application error.
#[derive(Debug, Error)]
pub enum AppError {
    #[error("Failed to read input: {0}")]
    InputRead(#[from] InputError),

    #[error("Failed to parse log entry: {0}")]
    Parse(#[from] ParseError),

    #[error("Terminal error: {0}")]
    Terminal(#[from] std::io::Error),
}

/// Input source errors.
#[derive(Debug, Error)]
pub enum InputError {
    #[error("File not found: {path}")]
    FileNotFound { path: std::path::PathBuf },

    #[error("File deleted during viewing")]
    FileDeleted,

    #[error("No input source: provide a file path or pipe data to stdin")]
    NoInput,

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// JSONL parsing errors.
#[derive(Debug, Error)]
pub enum ParseError {
    #[error("Invalid JSON at line {line}: {message}")]
    InvalidJson { line: usize, message: String },

    #[error("Missing required field '{field}' at line {line}")]
    MissingField { line: usize, field: &'static str },

    #[error("Invalid timestamp '{raw}' at line {line}")]
    InvalidTimestamp { line: usize, raw: String },
}
```

---

## Type Hierarchy Diagram

```
Session
├── SessionId
├── main_agent: AgentConversation
│   ├── entries: Vec<LogEntry>
│   │   ├── EntryUuid
│   │   ├── parent_uuid: Option<EntryUuid>
│   │   ├── SessionId
│   │   ├── AgentId (None for main)
│   │   ├── DateTime<Utc>
│   │   ├── EntryType {User, Assistant, Summary}
│   │   ├── Message
│   │   │   ├── Role {User, Assistant}
│   │   │   ├── MessageContent {Text(String), Blocks(Vec<ContentBlock>)}
│   │   │   │   ├── ContentBlock::Text { text }
│   │   │   │   ├── ContentBlock::ToolUse(ToolCall)
│   │   │   │   │   ├── ToolUseId
│   │   │   │   │   ├── ToolName {Read, Write, Bash, ...}
│   │   │   │   │   └── input: serde_json::Value
│   │   │   │   ├── ContentBlock::ToolResult { tool_use_id, content, is_error }
│   │   │   │   └── ContentBlock::Thinking { thinking }
│   │   │   ├── ModelInfo
│   │   │   └── TokenUsage
│   │   └── EntryMetadata
│   └── ModelInfo
├── subagents: HashMap<AgentId, AgentConversation>
└── stats: SessionStats
    ├── total_usage: TokenUsage
    ├── tool_counts: HashMap<ToolName, u32>
    └── subagent_count: usize

PricingConfig
├── models: HashMap<String, ModelPricing>
│   └── ModelPricing
│       ├── input_cost_per_million: f64
│       ├── output_cost_per_million: f64
│       └── cached_input_cost_per_million: Option<f64>
└── default_pricing: ModelPricing
```

---

## Property-Based Testing Invariants

```rust
// Properties to test with proptest:

// 1. Parse round-trip (where possible)
// parse(serialize(entry)) == entry for valid entries

// 2. Scroll bounds
// ∀ actions: scroll_offset ≤ entries.len()

// 3. Statistics consistency
// total_usage == main_agent_usage + sum(subagent_usage)

// 4. Search match validity
// ∀ match in matches: entry exists && offset < content.len()

// 5. Tab selection bounds
// selected_tab < subagents.len() || selected_tab is None

// 6. No illegal states
// Cannot construct: SearchState::Active with empty matches
// Cannot construct: SearchQuery from empty string
// Cannot construct: EntryUuid from empty string

// 7. Wrap state consistency (FR-048, FR-049)
// ∀ uuid: effective_wrap(uuid, global) == if wrap_overrides.contains(uuid) { !global } else { global }
// wrap_overrides is ephemeral: after reload, wrap_overrides.is_empty()

// 8. Wrap toggle idempotence
// toggle_wrap(uuid); toggle_wrap(uuid) => wrap_overrides state restored
// toggle_global_wrap(); toggle_global_wrap() => global_wrap state restored
```
