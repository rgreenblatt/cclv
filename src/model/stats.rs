//! Session statistics and cost estimation.
//!
//! This module provides aggregated statistics for sessions, including token usage,
//! tool counts, and estimated costs based on pricing configuration.

use crate::model::{AgentId, LogEntry, SessionId, TokenUsage, ToolName};
use std::collections::HashMap;

// ===== SessionStats =====

/// Aggregated session statistics for display in the statistics panel (FR-018).
///
/// Provides comprehensive metrics about token usage, tool invocations, and agent activity
/// across an entire session. Statistics can be filtered to show global totals, main agent
/// only, or specific subagents (FR-020).
///
/// # Invariants
///
/// - Statistics are incrementally recorded as entries are processed via `record_entry`
/// - `total_usage` equals the sum of `main_agent_usage` and all `subagent_usage` values
/// - `subagent_count` equals the number of unique keys in `subagent_usage`
/// - Tool counts are maintained globally and per-agent for filtering
///
/// # Cost Calculation
///
/// Estimated costs are calculated using `estimated_cost()` which applies pricing
/// configuration (FR-017, FR-046, FR-047) to token counts:
/// - Input tokens at model's input rate (FR-015)
/// - Output tokens at model's output rate (FR-016)
/// - Cached tokens (creation + read) at cached rate when available
///
/// See `PricingConfig` and `ModelPricing` for cost rates.
///
/// # Relationship to TokenUsage
///
/// All usage fields are `TokenUsage` instances which track four token types:
/// - `input_tokens`: Standard input tokens
/// - `output_tokens`: Generated output tokens
/// - `cache_creation_input_tokens`: Tokens used to create prompt cache
/// - `cache_read_input_tokens`: Tokens read from prompt cache
#[derive(Debug, Clone, Default)]
pub struct SessionStats {
    /// Total token usage across all agents (main agent + all subagents).
    ///
    /// Corresponds to StatsFilter::AllSessionsCombined (FR-020). This is the sum of
    /// `main_agent_usage` and all values in `subagent_usage`.
    pub total_usage: TokenUsage,

    /// Token usage for the main agent only (excludes subagents).
    ///
    /// Corresponds to StatsFilter::MainAgent (FR-020). Incremented only
    /// for log entries where `agent_id` is `None`.
    pub main_agent_usage: TokenUsage,

    /// Token usage per subagent, keyed by AgentId.
    ///
    /// Used for StatsFilter::Subagent(id) filtering (FR-020). Each subagent's
    /// usage is tracked separately. The number of unique keys determines `subagent_count`.
    pub subagent_usage: HashMap<AgentId, TokenUsage>,

    /// Per-session token usage (main + all subagents for each session).
    ///
    /// Used for StatsFilter::Session(id) filtering. Tracks total usage for
    /// each session, combining main agent and all subagents within that session.
    pub session_usage: HashMap<SessionId, TokenUsage>,

    /// Main agent token usage by session.
    ///
    /// Used for StatsFilter::MainAgent(session_id) filtering. Tracks only main
    /// agent usage (agent_id == None) for specific sessions.
    pub main_agent_usage_by_session: HashMap<SessionId, TokenUsage>,

    /// Total tool invocation counts across all agents, grouped by tool name (FR-018).
    ///
    /// Tracks how many times each tool (Read, Write, Bash, etc.) was invoked across
    /// the entire session. Corresponds to StatsFilter::AllSessionsCombined.
    pub tool_counts: HashMap<ToolName, u32>,

    /// Tool invocation counts for the main agent only (FR-019).
    ///
    /// Subset of `tool_counts` containing only tools called by the main agent
    /// (entries with `agent_id == None`). Used for StatsFilter::MainAgent filtering.
    pub main_agent_tool_counts: HashMap<ToolName, u32>,

    /// Tool invocation counts per subagent, nested by AgentId then ToolName (FR-019).
    ///
    /// Tracks tool usage separately for each subagent. Used for StatsFilter::Subagent(id)
    /// filtering to show which tools a specific subagent invoked.
    pub subagent_tool_counts: HashMap<AgentId, HashMap<ToolName, u32>>,

    /// Number of unique subagents spawned during the session (FR-019).
    ///
    /// Derived from the number of unique keys in `subagent_usage`. Updated
    /// automatically when `record_entry` processes entries from new subagents.
    pub subagent_count: usize,

    /// Total number of log entries processed (not displayed in stats panel).
    ///
    /// Incremented once per `record_entry` call. Useful for sanity checking
    /// and debugging statistics calculations.
    pub entry_count: usize,

    /// Actual session cost extracted from result entry (FMT-010).
    ///
    /// When a type:result entry is processed, this field is populated with
    /// the `total_cost_usd` value from the ResultMetadata. This is the ground
    /// truth for session cost, as opposed to `estimated_cost()` which calculates
    /// from token counts.
    ///
    /// `None` until a result entry is encountered. Updated to the latest result
    /// entry's cost if multiple result entries are seen.
    pub actual_cost_usd: Option<f64>,
}

impl SessionStats {
    /// Record statistics from a log entry.
    ///
    /// This method:
    /// - Increments entry_count
    /// - Accumulates token usage to total_usage
    /// - Accumulates token usage to session_usage (per-session totals)
    /// - Routes usage to main_agent_usage or subagent_usage based on agent_id
    /// - Routes main agent usage to main_agent_usage_by_session (per-session main agent)
    /// - Counts tool calls from the message
    /// - Updates subagent_count from unique subagents
    pub fn record_entry(&mut self, entry: &LogEntry) {
        // Increment entry count
        self.entry_count += 1;

        // Extract and accumulate usage if present
        if let Some(usage) = entry.message().usage() {
            // Accumulate to total
            self.total_usage.input_tokens += usage.input_tokens;
            self.total_usage.output_tokens += usage.output_tokens;
            self.total_usage.cache_creation_input_tokens += usage.cache_creation_input_tokens;
            self.total_usage.cache_read_input_tokens += usage.cache_read_input_tokens;

            // Accumulate to session usage (all agents for this session)
            let session_usage = self
                .session_usage
                .entry(entry.session_id().clone())
                .or_default();
            session_usage.input_tokens += usage.input_tokens;
            session_usage.output_tokens += usage.output_tokens;
            session_usage.cache_creation_input_tokens += usage.cache_creation_input_tokens;
            session_usage.cache_read_input_tokens += usage.cache_read_input_tokens;

            // Route to main agent or subagent
            if let Some(agent_id) = entry.agent_id() {
                // Subagent usage
                let agent_usage = self.subagent_usage.entry(agent_id.clone()).or_default();
                agent_usage.input_tokens += usage.input_tokens;
                agent_usage.output_tokens += usage.output_tokens;
            } else {
                // Main agent usage (global)
                self.main_agent_usage.input_tokens += usage.input_tokens;
                self.main_agent_usage.output_tokens += usage.output_tokens;

                // Main agent usage by session
                let main_session_usage = self
                    .main_agent_usage_by_session
                    .entry(entry.session_id().clone())
                    .or_default();
                main_session_usage.input_tokens += usage.input_tokens;
                main_session_usage.output_tokens += usage.output_tokens;
            }
        }

        // Count tool calls (global and per-agent)
        for tool in entry.message().tool_calls() {
            // Global count
            *self.tool_counts.entry(tool.name().clone()).or_default() += 1;

            // Per-agent count
            if let Some(agent_id) = entry.agent_id() {
                // Subagent tool count
                let agent_tools = self
                    .subagent_tool_counts
                    .entry(agent_id.clone())
                    .or_default();
                *agent_tools.entry(tool.name().clone()).or_default() += 1;
            } else {
                // Main agent tool count
                *self
                    .main_agent_tool_counts
                    .entry(tool.name().clone())
                    .or_default() += 1;
            }
        }

        // Update subagent count (unique count)
        self.subagent_count = self.subagent_usage.len();

        // Extract actual cost from result entry metadata (FMT-010)
        if let Some(result_meta) = entry.result_metadata() {
            self.actual_cost_usd = Some(result_meta.total_cost_usd);
        }
    }

    /// Calculate estimated cost in USD using the provided pricing configuration.
    ///
    /// Cost includes:
    /// - Input tokens at input rate
    /// - Output tokens at output rate
    /// - Cached tokens (cache_creation + cache_read) at cached rate (or input rate if not specified)
    ///
    /// Model pricing is determined by matching the model_id string (e.g., "opus", "sonnet").
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

    /// Get filtered token usage based on the current stats filter.
    ///
    /// Returns:
    /// - `StatsFilter::AllSessionsCombined`: total_usage (all sessions, all agents)
    /// - `StatsFilter::Session(session_id)`: session_usage for specific session
    /// - `StatsFilter::MainAgent(session_id)`: main_agent_usage_by_session for specific session
    /// - `StatsFilter::Subagent(id)`: usage for specific subagent, or default if not found
    pub fn filtered_usage(&self, filter: &StatsFilter) -> TokenUsage {
        match filter {
            StatsFilter::AllSessionsCombined => self.total_usage,
            StatsFilter::Session(session_id) => {
                self.session_usage.get(session_id).copied().unwrap_or_default()
            }
            StatsFilter::MainAgent(session_id) => {
                self.main_agent_usage_by_session
                    .get(session_id)
                    .copied()
                    .unwrap_or_default()
            }
            StatsFilter::Subagent(agent_id) => self
                .subagent_usage
                .get(agent_id)
                .copied()
                .unwrap_or_default(),
        }
    }

    /// Get filtered tool counts based on the current stats filter.
    ///
    /// Returns:
    /// - `StatsFilter::AllSessionsCombined`: tool_counts (all agents)
    /// - `StatsFilter::Session(session_id)`: TODO - session-scoped tool counts
    /// - `StatsFilter::MainAgent(session_id)`: main_agent_tool_counts only
    /// - `StatsFilter::Subagent(id)`: tool_counts for specific subagent, or empty if not found
    pub fn filtered_tool_counts(&self, filter: &StatsFilter) -> &HashMap<ToolName, u32> {
        use std::sync::OnceLock;
        static EMPTY: OnceLock<HashMap<ToolName, u32>> = OnceLock::new();

        match filter {
            StatsFilter::AllSessionsCombined => &self.tool_counts,
            StatsFilter::Session(_session_id) => todo!("Session-scoped tool counts"),
            StatsFilter::MainAgent(_session_id) => &self.main_agent_tool_counts,
            StatsFilter::Subagent(agent_id) => self
                .subagent_tool_counts
                .get(agent_id)
                .unwrap_or_else(|| EMPTY.get_or_init(HashMap::new)),
        }
    }
}

// ===== StatsFilter =====

/// Filter for statistics display.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum StatsFilter {
    /// Statistics for all sessions combined (all agents).
    #[default]
    AllSessionsCombined,

    /// Statistics for a specific session (main + all subagents combined).
    Session(SessionId),

    /// Statistics for a specific session's main agent only.
    MainAgent(SessionId),

    /// Statistics for a specific subagent.
    Subagent(AgentId),
}

impl StatsFilter {
    /// Get the full label for display in stats panel title.
    pub fn label(&self) -> String {
        match self {
            StatsFilter::AllSessionsCombined => "Statistics: All Sessions".to_string(),
            StatsFilter::Session(session_id) => {
                format!("Statistics: Session {}", session_id.as_str())
            }
            StatsFilter::MainAgent(session_id) => {
                format!("Statistics: Main Agent (Session {})", session_id.as_str())
            }
            StatsFilter::Subagent(agent_id) => {
                format!("Subagent {}", agent_id.as_str())
            }
        }
    }

    /// Get the short label for display in status bar.
    pub fn short_label(&self) -> &'static str {
        match self {
            StatsFilter::AllSessionsCombined => "All",
            StatsFilter::Session(_) => "Sess",
            StatsFilter::MainAgent(_) => "Main",
            StatsFilter::Subagent(_) => "Sub",
        }
    }
}

// ===== PricingConfig =====

/// Pricing configuration for cost estimation.
///
/// Contains pricing for known model families (opus, sonnet, haiku) and a default fallback.
#[derive(Debug, Clone)]
pub struct PricingConfig {
    #[allow(dead_code)]
    models: HashMap<String, ModelPricing>,
    #[allow(dead_code)]
    default_pricing: ModelPricing,
}

impl Default for PricingConfig {
    fn default() -> Self {
        let mut models = HashMap::new();

        // Claude Opus 4.5 - $15/$75 per million tokens
        models.insert(
            "opus".to_string(),
            ModelPricing::new(15.0, 75.0).with_cache(1.5),
        );
        // Claude Sonnet 4 - $3/$15 per million tokens
        models.insert(
            "sonnet".to_string(),
            ModelPricing::new(3.0, 15.0).with_cache(0.3),
        );
        // Claude Haiku 3.5 - $0.80/$4 per million tokens
        models.insert(
            "haiku".to_string(),
            ModelPricing::new(0.8, 4.0).with_cache(0.08),
        );

        Self {
            models,
            default_pricing: ModelPricing::new(15.0, 75.0), // Opus as fallback
        }
    }
}

impl PricingConfig {
    /// Get pricing for a model ID.
    ///
    /// Tries:
    /// 1. Exact match on model_id
    /// 2. Model family match (contains "opus", "sonnet", or "haiku")
    /// 3. Default pricing as fallback
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
}

// ===== ModelPricing =====

/// Pricing for a specific model (per million tokens, in USD).
///
/// Used by `SessionStats::estimated_cost` to calculate session costs based on
/// token usage (FR-017). Pricing is applied to token counts from `TokenUsage`:
/// - `input_cost_per_million` applies to `input_tokens`
/// - `output_cost_per_million` applies to `output_tokens`
/// - `cached_input_cost_per_million` applies to `cache_creation_input_tokens`
///   and `cache_read_input_tokens` (falls back to `input_cost_per_million` if None)
///
/// # Default Pricing (FR-046)
///
/// Hardcoded defaults in `PricingConfig::default()`:
/// - **Claude Opus 4.5**: $15 input / $75 output / $1.50 cached (per million)
/// - **Claude Sonnet 4**: $3 input / $15 output / $0.30 cached (per million)
/// - **Claude Haiku 3.5**: $0.80 input / $4 output / $0.08 cached (per million)
///
/// # Configuration (FR-047)
///
/// Pricing MAY be overridden via configuration file, though this is currently
/// not implemented. Default pricing is always available as fallback.
#[derive(Debug, Clone, Copy)]
pub struct ModelPricing {
    /// Cost per million input tokens in USD (FR-015, FR-017).
    ///
    /// Applied to `TokenUsage::input_tokens`. Standard input tokens are those
    /// not served from prompt cache.
    pub input_cost_per_million: f64,

    /// Cost per million output tokens in USD (FR-016, FR-017).
    ///
    /// Applied to `TokenUsage::output_tokens`. Output tokens are those generated
    /// by the model in responses.
    pub output_cost_per_million: f64,

    /// Optional cost per million cached input tokens in USD (FR-017).
    ///
    /// Applied to both `TokenUsage::cache_creation_input_tokens` and
    /// `TokenUsage::cache_read_input_tokens`. When None, falls back to
    /// `input_cost_per_million` for cached token cost calculation.
    ///
    /// Cached input is typically cheaper than standard input (e.g., 10x reduction
    /// for Opus: $1.50 vs $15.00 per million).
    pub cached_input_cost_per_million: Option<f64>,
}

impl ModelPricing {
    /// Create new model pricing with input and output costs.
    pub const fn new(input: f64, output: f64) -> Self {
        Self {
            input_cost_per_million: input,
            output_cost_per_million: output,
            cached_input_cost_per_million: None,
        }
    }

    /// Add cached input pricing.
    pub const fn with_cache(mut self, cached: f64) -> Self {
        self.cached_input_cost_per_million = Some(cached);
        self
    }
}

// ===== Config Conversions =====

/// Convert PricingEntry from config file to ModelPricing.
impl From<crate::config::PricingEntry> for ModelPricing {
    fn from(entry: crate::config::PricingEntry) -> Self {
        let mut pricing = ModelPricing::new(entry.input, entry.output);
        pricing.cached_input_cost_per_million = entry.cached_input;
        pricing
    }
}

/// Convert PricingConfigSection from config file to PricingConfig.
impl From<crate::config::PricingConfigSection> for PricingConfig {
    fn from(section: crate::config::PricingConfigSection) -> Self {
        let mut models = HashMap::new();

        // Convert each model entry
        for (key, entry) in section.models {
            models.insert(key, entry.into());
        }

        // Use default from config if provided, otherwise use hardcoded default
        let default_pricing = section
            .default
            .map(|entry| entry.into())
            .unwrap_or_else(|| ModelPricing::new(15.0, 75.0));

        Self {
            models,
            default_pricing,
        }
    }
}

// ===== Tests =====

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{
        AgentId, ContentBlock, EntryMetadata, EntryType, EntryUuid, LogEntry, Message,
        MessageContent, Role, SessionId, ToolCall, ToolName, ToolUseId,
    };
    use chrono::Utc;

    // ===== Test Helpers =====

    fn make_uuid(s: &str) -> EntryUuid {
        EntryUuid::new(s).expect("valid uuid")
    }

    fn make_session_id(s: &str) -> SessionId {
        SessionId::new(s).expect("valid session id")
    }

    fn make_agent_id(s: &str) -> AgentId {
        AgentId::new(s).expect("valid agent id")
    }

    fn make_tool_use_id(s: &str) -> ToolUseId {
        ToolUseId::new(s).expect("valid tool use id")
    }

    fn make_message_with_usage(usage: TokenUsage) -> Message {
        Message::new(Role::Assistant, MessageContent::Text("Test".to_string())).with_usage(usage)
    }

    fn make_message_with_tool_calls(tool_names: Vec<ToolName>) -> Message {
        let blocks: Vec<ContentBlock> = tool_names
            .into_iter()
            .enumerate()
            .map(|(i, name)| {
                ContentBlock::ToolUse(ToolCall::new(
                    make_tool_use_id(&format!("tool-{}", i)),
                    name,
                    serde_json::json!({}),
                ))
            })
            .collect();
        Message::new(Role::Assistant, MessageContent::Blocks(blocks))
    }

    fn make_log_entry(
        uuid: &str,
        session_id: &str,
        agent_id: Option<&str>,
        message: Message,
    ) -> LogEntry {
        LogEntry::new(
            make_uuid(uuid),
            None,
            make_session_id(session_id),
            agent_id.map(make_agent_id),
            Utc::now(),
            EntryType::Assistant,
            message,
            EntryMetadata::default(),
        )
    }

    // ===== SessionStats::record_entry Tests =====

    #[test]
    fn record_entry_increments_entry_count() {
        let mut stats = SessionStats::default();
        let message = Message::new(Role::User, MessageContent::Text("Hello".to_string()));
        let entry = make_log_entry("e1", "s1", None, message);

        stats.record_entry(&entry);

        assert_eq!(stats.entry_count, 1);
    }

    #[test]
    fn record_entry_increments_entry_count_multiple_times() {
        let mut stats = SessionStats::default();
        let message = Message::new(Role::User, MessageContent::Text("Hello".to_string()));
        let entry1 = make_log_entry("e1", "s1", None, message.clone());
        let entry2 = make_log_entry("e2", "s1", None, message);

        stats.record_entry(&entry1);
        stats.record_entry(&entry2);

        assert_eq!(stats.entry_count, 2);
    }

    #[test]
    fn record_entry_accumulates_usage_to_total() {
        let mut stats = SessionStats::default();
        let usage = TokenUsage {
            input_tokens: 100,
            output_tokens: 50,
            cache_creation_input_tokens: 20,
            cache_read_input_tokens: 10,
            ephemeral_5m_input_tokens: 0,
            ephemeral_1h_input_tokens: 0,
        };
        let message = make_message_with_usage(usage);
        let entry = make_log_entry("e1", "s1", None, message);

        stats.record_entry(&entry);

        assert_eq!(stats.total_usage.input_tokens, 100);
        assert_eq!(stats.total_usage.output_tokens, 50);
        assert_eq!(stats.total_usage.cache_creation_input_tokens, 20);
        assert_eq!(stats.total_usage.cache_read_input_tokens, 10);
    }

    #[test]
    fn record_entry_routes_main_agent_usage_correctly() {
        let mut stats = SessionStats::default();
        let usage = TokenUsage {
            input_tokens: 200,
            output_tokens: 100,
            cache_creation_input_tokens: 0,
            cache_read_input_tokens: 0,
            ephemeral_5m_input_tokens: 0,
            ephemeral_1h_input_tokens: 0,
        };
        let message = make_message_with_usage(usage);
        let entry = make_log_entry("e1", "s1", None, message); // None = main agent

        stats.record_entry(&entry);

        assert_eq!(stats.main_agent_usage.input_tokens, 200);
        assert_eq!(stats.main_agent_usage.output_tokens, 100);
    }

    #[test]
    fn record_entry_routes_subagent_usage_correctly() {
        let mut stats = SessionStats::default();
        let usage = TokenUsage {
            input_tokens: 300,
            output_tokens: 150,
            cache_creation_input_tokens: 0,
            cache_read_input_tokens: 0,
            ephemeral_5m_input_tokens: 0,
            ephemeral_1h_input_tokens: 0,
        };
        let message = make_message_with_usage(usage);
        let entry = make_log_entry("e1", "s1", Some("agent-123"), message);

        stats.record_entry(&entry);

        let agent_id = make_agent_id("agent-123");
        let agent_usage = stats.subagent_usage.get(&agent_id).expect("subagent usage");
        assert_eq!(agent_usage.input_tokens, 300);
        assert_eq!(agent_usage.output_tokens, 150);
    }

    #[test]
    fn record_entry_accumulates_subagent_usage_for_same_agent() {
        let mut stats = SessionStats::default();
        let usage1 = TokenUsage {
            input_tokens: 100,
            output_tokens: 50,
            cache_creation_input_tokens: 0,
            cache_read_input_tokens: 0,
            ephemeral_5m_input_tokens: 0,
            ephemeral_1h_input_tokens: 0,
        };
        let usage2 = TokenUsage {
            input_tokens: 200,
            output_tokens: 75,
            cache_creation_input_tokens: 0,
            cache_read_input_tokens: 0,
            ephemeral_5m_input_tokens: 0,
            ephemeral_1h_input_tokens: 0,
        };
        let entry1 = make_log_entry(
            "e1",
            "s1",
            Some("agent-abc"),
            make_message_with_usage(usage1),
        );
        let entry2 = make_log_entry(
            "e2",
            "s1",
            Some("agent-abc"),
            make_message_with_usage(usage2),
        );

        stats.record_entry(&entry1);
        stats.record_entry(&entry2);

        let agent_id = make_agent_id("agent-abc");
        let agent_usage = stats.subagent_usage.get(&agent_id).expect("subagent usage");
        assert_eq!(agent_usage.input_tokens, 300);
        assert_eq!(agent_usage.output_tokens, 125);
    }

    #[test]
    fn record_entry_counts_tool_calls() {
        let mut stats = SessionStats::default();
        let message = make_message_with_tool_calls(vec![ToolName::Read, ToolName::Write]);
        let entry = make_log_entry("e1", "s1", None, message);

        stats.record_entry(&entry);

        assert_eq!(stats.tool_counts.get(&ToolName::Read), Some(&1));
        assert_eq!(stats.tool_counts.get(&ToolName::Write), Some(&1));
    }

    #[test]
    fn record_entry_accumulates_tool_counts() {
        let mut stats = SessionStats::default();
        let message1 = make_message_with_tool_calls(vec![ToolName::Read]);
        let message2 = make_message_with_tool_calls(vec![ToolName::Read, ToolName::Bash]);
        let entry1 = make_log_entry("e1", "s1", None, message1);
        let entry2 = make_log_entry("e2", "s1", None, message2);

        stats.record_entry(&entry1);
        stats.record_entry(&entry2);

        assert_eq!(stats.tool_counts.get(&ToolName::Read), Some(&2));
        assert_eq!(stats.tool_counts.get(&ToolName::Bash), Some(&1));
    }

    #[test]
    fn record_entry_updates_subagent_count() {
        let mut stats = SessionStats::default();
        let entry1 = make_log_entry(
            "e1",
            "s1",
            Some("agent-1"),
            make_message_with_usage(TokenUsage::default()),
        );
        let entry2 = make_log_entry(
            "e2",
            "s1",
            Some("agent-2"),
            make_message_with_usage(TokenUsage::default()),
        );
        let entry3 = make_log_entry(
            "e3",
            "s1",
            Some("agent-1"),
            make_message_with_usage(TokenUsage::default()),
        );

        stats.record_entry(&entry1);
        assert_eq!(stats.subagent_count, 1);

        stats.record_entry(&entry2);
        assert_eq!(stats.subagent_count, 2);

        stats.record_entry(&entry3);
        assert_eq!(stats.subagent_count, 2); // Still 2, not 3
    }

    #[test]
    fn record_entry_handles_missing_usage() {
        let mut stats = SessionStats::default();
        let message = Message::new(Role::User, MessageContent::Text("Hello".to_string()));
        let entry = make_log_entry("e1", "s1", None, message);

        stats.record_entry(&entry);

        assert_eq!(stats.entry_count, 1);
        assert_eq!(stats.total_usage.input_tokens, 0);
        assert_eq!(stats.total_usage.output_tokens, 0);
    }

    // ===== SessionStats::estimated_cost Tests =====

    #[test]
    fn estimated_cost_calculates_input_cost() {
        let stats = SessionStats {
            total_usage: TokenUsage {
                input_tokens: 1_000_000, // 1 million
                output_tokens: 0,
                cache_creation_input_tokens: 0,
                cache_read_input_tokens: 0,
                ephemeral_5m_input_tokens: 0,
                ephemeral_1h_input_tokens: 0,
            },
            ..Default::default()
        };
        let pricing = PricingConfig::default();

        let cost = stats.estimated_cost(&pricing, Some("opus"));

        // Opus: $15 per million input tokens
        assert_eq!(cost, 15.0);
    }

    #[test]
    fn estimated_cost_calculates_output_cost() {
        let stats = SessionStats {
            total_usage: TokenUsage {
                input_tokens: 0,
                output_tokens: 1_000_000, // 1 million
                cache_creation_input_tokens: 0,
                cache_read_input_tokens: 0,
                ephemeral_5m_input_tokens: 0,
                ephemeral_1h_input_tokens: 0,
            },

            ..Default::default()
        };
        let pricing = PricingConfig::default();

        let cost = stats.estimated_cost(&pricing, Some("opus"));

        // Opus: $75 per million output tokens
        assert_eq!(cost, 75.0);
    }

    #[test]
    fn estimated_cost_calculates_cached_input_cost() {
        let stats = SessionStats {
            total_usage: TokenUsage {
                input_tokens: 0,
                output_tokens: 0,
                cache_creation_input_tokens: 500_000,
                cache_read_input_tokens: 500_000,
                ephemeral_5m_input_tokens: 0,
                ephemeral_1h_input_tokens: 0,
            },

            ..Default::default()
        };
        let pricing = PricingConfig::default();

        let cost = stats.estimated_cost(&pricing, Some("opus"));

        // Opus: $1.5 per million cached tokens, total 1M cached = $1.5
        assert_eq!(cost, 1.5);
    }

    #[test]
    fn estimated_cost_combines_all_costs() {
        let stats = SessionStats {
            total_usage: TokenUsage {
                input_tokens: 1_000_000,
                output_tokens: 1_000_000,
                cache_creation_input_tokens: 500_000,
                cache_read_input_tokens: 500_000,
                ephemeral_5m_input_tokens: 0,
                ephemeral_1h_input_tokens: 0,
            },

            ..Default::default()
        };
        let pricing = PricingConfig::default();

        let cost = stats.estimated_cost(&pricing, Some("opus"));

        // $15 (input) + $75 (output) + $1.5 (cached) = $91.5
        assert_eq!(cost, 91.5);
    }

    #[test]
    fn estimated_cost_uses_sonnet_pricing() {
        let stats = SessionStats {
            total_usage: TokenUsage {
                input_tokens: 1_000_000,
                output_tokens: 1_000_000,
                cache_creation_input_tokens: 0,
                cache_read_input_tokens: 0,
                ephemeral_5m_input_tokens: 0,
                ephemeral_1h_input_tokens: 0,
            },

            ..Default::default()
        };
        let pricing = PricingConfig::default();

        let cost = stats.estimated_cost(&pricing, Some("sonnet"));

        // Sonnet: $3 (input) + $15 (output) = $18
        assert_eq!(cost, 18.0);
    }

    #[test]
    fn estimated_cost_uses_haiku_pricing() {
        let stats = SessionStats {
            total_usage: TokenUsage {
                input_tokens: 1_000_000,
                output_tokens: 1_000_000,
                cache_creation_input_tokens: 0,
                cache_read_input_tokens: 0,
                ephemeral_5m_input_tokens: 0,
                ephemeral_1h_input_tokens: 0,
            },

            ..Default::default()
        };
        let pricing = PricingConfig::default();

        let cost = stats.estimated_cost(&pricing, Some("haiku"));

        // Haiku: $0.8 (input) + $4 (output) = $4.8
        assert_eq!(cost, 4.8);
    }

    #[test]
    fn estimated_cost_defaults_to_opus_for_unknown_model() {
        let stats = SessionStats {
            total_usage: TokenUsage {
                input_tokens: 1_000_000,
                output_tokens: 1_000_000,
                cache_creation_input_tokens: 0,
                cache_read_input_tokens: 0,
                ephemeral_5m_input_tokens: 0,
                ephemeral_1h_input_tokens: 0,
            },

            ..Default::default()
        };
        let pricing = PricingConfig::default();

        let cost = stats.estimated_cost(&pricing, Some("unknown-model"));

        // Default (Opus): $15 (input) + $75 (output) = $90
        assert_eq!(cost, 90.0);
    }

    #[test]
    fn estimated_cost_handles_none_model_id() {
        let stats = SessionStats {
            total_usage: TokenUsage {
                input_tokens: 1_000_000,
                output_tokens: 1_000_000,
                cache_creation_input_tokens: 0,
                cache_read_input_tokens: 0,
                ephemeral_5m_input_tokens: 0,
                ephemeral_1h_input_tokens: 0,
            },

            ..Default::default()
        };
        let pricing = PricingConfig::default();

        let cost = stats.estimated_cost(&pricing, None);

        // Default (Opus): $15 (input) + $75 (output) = $90
        assert_eq!(cost, 90.0);
    }

    // ===== PricingConfig::get Tests =====

    #[test]
    fn pricing_config_get_exact_match_opus() {
        let pricing = PricingConfig::default();
        let model_pricing = pricing.get("opus");

        assert_eq!(model_pricing.input_cost_per_million, 15.0);
        assert_eq!(model_pricing.output_cost_per_million, 75.0);
        assert_eq!(model_pricing.cached_input_cost_per_million, Some(1.5));
    }

    #[test]
    fn pricing_config_get_exact_match_sonnet() {
        let pricing = PricingConfig::default();
        let model_pricing = pricing.get("sonnet");

        assert_eq!(model_pricing.input_cost_per_million, 3.0);
        assert_eq!(model_pricing.output_cost_per_million, 15.0);
        assert_eq!(model_pricing.cached_input_cost_per_million, Some(0.3));
    }

    #[test]
    fn pricing_config_get_exact_match_haiku() {
        let pricing = PricingConfig::default();
        let model_pricing = pricing.get("haiku");

        assert_eq!(model_pricing.input_cost_per_million, 0.8);
        assert_eq!(model_pricing.output_cost_per_million, 4.0);
        assert_eq!(model_pricing.cached_input_cost_per_million, Some(0.08));
    }

    #[test]
    fn pricing_config_get_family_match_opus() {
        let pricing = PricingConfig::default();
        let model_pricing = pricing.get("claude-opus-4-5-20251101");

        assert_eq!(model_pricing.input_cost_per_million, 15.0);
        assert_eq!(model_pricing.output_cost_per_million, 75.0);
    }

    #[test]
    fn pricing_config_get_family_match_sonnet() {
        let pricing = PricingConfig::default();
        let model_pricing = pricing.get("claude-sonnet-4-5-20250929");

        assert_eq!(model_pricing.input_cost_per_million, 3.0);
        assert_eq!(model_pricing.output_cost_per_million, 15.0);
    }

    #[test]
    fn pricing_config_get_family_match_haiku() {
        let pricing = PricingConfig::default();
        let model_pricing = pricing.get("claude-haiku-3-5-20241022");

        assert_eq!(model_pricing.input_cost_per_million, 0.8);
        assert_eq!(model_pricing.output_cost_per_million, 4.0);
    }

    #[test]
    fn pricing_config_get_defaults_for_unknown() {
        let pricing = PricingConfig::default();
        let model_pricing = pricing.get("gpt-4");

        assert_eq!(model_pricing.input_cost_per_million, 15.0);
        assert_eq!(model_pricing.output_cost_per_million, 75.0);
    }

    // ===== StatsFilter Tests =====

    #[test]
    fn stats_filter_all_sessions_combined_equality() {
        assert_eq!(
            StatsFilter::AllSessionsCombined,
            StatsFilter::AllSessionsCombined
        );
    }

    #[test]
    fn stats_filter_session_equality() {
        let session1 = make_session_id("session-1");
        let session2 = make_session_id("session-1");
        assert_eq!(StatsFilter::Session(session1), StatsFilter::Session(session2));
    }

    #[test]
    fn stats_filter_main_agent_equality() {
        let session1 = make_session_id("session-1");
        let session2 = make_session_id("session-1");
        assert_eq!(
            StatsFilter::MainAgent(session1),
            StatsFilter::MainAgent(session2)
        );
    }

    #[test]
    fn stats_filter_subagent_equality() {
        let agent1 = make_agent_id("agent-1");
        let agent2 = make_agent_id("agent-1");
        assert_eq!(StatsFilter::Subagent(agent1), StatsFilter::Subagent(agent2));
    }

    #[test]
    fn stats_filter_different_variants_not_equal() {
        let session_id = make_session_id("session-1");
        assert_ne!(
            StatsFilter::AllSessionsCombined,
            StatsFilter::MainAgent(session_id)
        );
    }

    #[test]
    fn stats_filter_different_subagents_not_equal() {
        let agent1 = make_agent_id("agent-1");
        let agent2 = make_agent_id("agent-2");
        assert_ne!(StatsFilter::Subagent(agent1), StatsFilter::Subagent(agent2));
    }

    #[test]
    fn stats_filter_label_all_sessions_combined() {
        let filter = StatsFilter::AllSessionsCombined;
        assert_eq!(filter.label(), "Statistics: All Sessions");
    }

    #[test]
    fn stats_filter_label_session() {
        let session_id = make_session_id("session-123");
        let filter = StatsFilter::Session(session_id);
        assert_eq!(filter.label(), "Statistics: Session session-123");
    }

    #[test]
    fn stats_filter_label_main_agent() {
        let session_id = make_session_id("session-456");
        let filter = StatsFilter::MainAgent(session_id);
        assert_eq!(filter.label(), "Statistics: Main Agent (Session session-456)");
    }

    #[test]
    fn stats_filter_label_subagent() {
        let agent_id = make_agent_id("agent-789");
        let filter = StatsFilter::Subagent(agent_id);
        assert_eq!(filter.label(), "Subagent agent-789");
    }

    #[test]
    fn stats_filter_short_label_all_sessions_combined() {
        let filter = StatsFilter::AllSessionsCombined;
        assert_eq!(filter.short_label(), "All");
    }

    #[test]
    fn stats_filter_short_label_session() {
        let session_id = make_session_id("session-123");
        let filter = StatsFilter::Session(session_id);
        assert_eq!(filter.short_label(), "Sess");
    }

    #[test]
    fn stats_filter_short_label_main_agent() {
        let session_id = make_session_id("session-456");
        let filter = StatsFilter::MainAgent(session_id);
        assert_eq!(filter.short_label(), "Main");
    }

    #[test]
    fn stats_filter_short_label_subagent() {
        let agent_id = make_agent_id("agent-789");
        let filter = StatsFilter::Subagent(agent_id);
        assert_eq!(filter.short_label(), "Sub");
    }

    #[test]
    fn stats_filter_default_is_all_sessions_combined() {
        let filter = StatsFilter::default();
        assert_eq!(filter, StatsFilter::AllSessionsCombined);
    }

    // ===== ModelPricing Tests =====

    #[test]
    fn model_pricing_new_creates_without_cache() {
        let pricing = ModelPricing::new(10.0, 50.0);

        assert_eq!(pricing.input_cost_per_million, 10.0);
        assert_eq!(pricing.output_cost_per_million, 50.0);
        assert_eq!(pricing.cached_input_cost_per_million, None);
    }

    #[test]
    fn model_pricing_with_cache_adds_cached_cost() {
        let pricing = ModelPricing::new(10.0, 50.0).with_cache(1.0);

        assert_eq!(pricing.input_cost_per_million, 10.0);
        assert_eq!(pricing.output_cost_per_million, 50.0);
        assert_eq!(pricing.cached_input_cost_per_million, Some(1.0));
    }

    // ===== SessionStats::filtered_usage Tests =====

    #[test]
    fn filtered_usage_global_returns_total_usage() {
        let stats = SessionStats {
            total_usage: TokenUsage {
                input_tokens: 1000,
                output_tokens: 500,
                cache_creation_input_tokens: 100,
                cache_read_input_tokens: 50,
                ephemeral_5m_input_tokens: 0,
                ephemeral_1h_input_tokens: 0,
            },
            main_agent_usage: TokenUsage {
                input_tokens: 600,
                output_tokens: 300,
                cache_creation_input_tokens: 0,
                cache_read_input_tokens: 0,
                ephemeral_5m_input_tokens: 0,
                ephemeral_1h_input_tokens: 0,
            },
            entry_count: 10,
            ..Default::default()
        };

        let filter = StatsFilter::AllSessionsCombined;
        let usage = stats.filtered_usage(&filter);

        assert_eq!(usage.input_tokens, 1000);
        assert_eq!(usage.output_tokens, 500);
        assert_eq!(usage.cache_creation_input_tokens, 100);
        assert_eq!(usage.cache_read_input_tokens, 50);
    }

    #[test]
    fn filtered_usage_main_agent_returns_main_agent_usage() {
        let session_id = make_session_id("test-session");
        let mut main_agent_usage_by_session = HashMap::new();
        main_agent_usage_by_session.insert(
            session_id.clone(),
            TokenUsage {
                input_tokens: 600,
                output_tokens: 300,
                cache_creation_input_tokens: 0,
                cache_read_input_tokens: 0,
                ephemeral_5m_input_tokens: 0,
                ephemeral_1h_input_tokens: 0,
            },
        );

        let stats = SessionStats {
            total_usage: TokenUsage {
                input_tokens: 1000,
                output_tokens: 500,
                cache_creation_input_tokens: 100,
                cache_read_input_tokens: 50,
                ephemeral_5m_input_tokens: 0,
                ephemeral_1h_input_tokens: 0,
            },
            main_agent_usage: TokenUsage {
                input_tokens: 600,
                output_tokens: 300,
                cache_creation_input_tokens: 0,
                cache_read_input_tokens: 0,
                ephemeral_5m_input_tokens: 0,
                ephemeral_1h_input_tokens: 0,
            },
            main_agent_usage_by_session,
            entry_count: 10,
            ..Default::default()
        };

        let filter = StatsFilter::MainAgent(session_id);
        let usage = stats.filtered_usage(&filter);

        assert_eq!(usage.input_tokens, 600);
        assert_eq!(usage.output_tokens, 300);
        assert_eq!(usage.cache_creation_input_tokens, 0);
        assert_eq!(usage.cache_read_input_tokens, 0);
    }

    #[test]
    fn filtered_usage_subagent_returns_specific_subagent_usage() {
        let agent1 = make_agent_id("agent-1");
        let agent2 = make_agent_id("agent-2");

        let mut subagent_usage = HashMap::new();
        subagent_usage.insert(
            agent1.clone(),
            TokenUsage {
                input_tokens: 400,
                output_tokens: 200,
                cache_creation_input_tokens: 0,
                cache_read_input_tokens: 0,
                ephemeral_5m_input_tokens: 0,
                ephemeral_1h_input_tokens: 0,
            },
        );
        subagent_usage.insert(
            agent2.clone(),
            TokenUsage {
                input_tokens: 300,
                output_tokens: 150,
                cache_creation_input_tokens: 0,
                cache_read_input_tokens: 0,
                ephemeral_5m_input_tokens: 0,
                ephemeral_1h_input_tokens: 0,
            },
        );

        let stats = SessionStats {
            total_usage: TokenUsage {
                input_tokens: 1000,
                output_tokens: 500,
                cache_creation_input_tokens: 0,
                cache_read_input_tokens: 0,
                ephemeral_5m_input_tokens: 0,
                ephemeral_1h_input_tokens: 0,
            },
            main_agent_usage: TokenUsage {
                input_tokens: 300,
                output_tokens: 150,
                cache_creation_input_tokens: 0,
                cache_read_input_tokens: 0,
                ephemeral_5m_input_tokens: 0,
                ephemeral_1h_input_tokens: 0,
            },
            subagent_usage,
            subagent_count: 2,
            entry_count: 20,
            ..Default::default()
        };

        let filter = StatsFilter::Subagent(agent1);
        let usage = stats.filtered_usage(&filter);

        assert_eq!(usage.input_tokens, 400);
        assert_eq!(usage.output_tokens, 200);
        assert_eq!(usage.cache_creation_input_tokens, 0);
        assert_eq!(usage.cache_read_input_tokens, 0);
    }

    #[test]
    fn filtered_usage_subagent_returns_default_when_agent_not_found() {
        let agent1 = make_agent_id("agent-1");
        let agent_missing = make_agent_id("agent-missing");

        let mut subagent_usage = HashMap::new();
        subagent_usage.insert(
            agent1.clone(),
            TokenUsage {
                input_tokens: 400,
                output_tokens: 200,
                cache_creation_input_tokens: 0,
                cache_read_input_tokens: 0,
                ephemeral_5m_input_tokens: 0,
                ephemeral_1h_input_tokens: 0,
            },
        );

        let stats = SessionStats {
            total_usage: TokenUsage {
                input_tokens: 700,
                output_tokens: 350,
                cache_creation_input_tokens: 0,
                cache_read_input_tokens: 0,
                ephemeral_5m_input_tokens: 0,
                ephemeral_1h_input_tokens: 0,
            },
            main_agent_usage: TokenUsage {
                input_tokens: 300,
                output_tokens: 150,
                cache_creation_input_tokens: 0,
                cache_read_input_tokens: 0,
                ephemeral_5m_input_tokens: 0,
                ephemeral_1h_input_tokens: 0,
            },
            subagent_usage,
            subagent_count: 1,
            entry_count: 15,
            ..Default::default()
        };

        let filter = StatsFilter::Subagent(agent_missing);
        let usage = stats.filtered_usage(&filter);

        // Should return default (zeros) for missing agent
        assert_eq!(usage.input_tokens, 0);
        assert_eq!(usage.output_tokens, 0);
        assert_eq!(usage.cache_creation_input_tokens, 0);
        assert_eq!(usage.cache_read_input_tokens, 0);
    }

    // ===== Per-Agent Tool Counts Tests =====

    #[test]
    fn record_entry_tracks_main_agent_tool_counts() {
        let mut stats = SessionStats::default();
        let message = make_message_with_tool_calls(vec![ToolName::Read, ToolName::Write]);
        let entry = make_log_entry("e1", "s1", None, message); // None = main agent

        stats.record_entry(&entry);

        // Should track in main_agent_tool_counts
        assert_eq!(stats.main_agent_tool_counts.get(&ToolName::Read), Some(&1));
        assert_eq!(stats.main_agent_tool_counts.get(&ToolName::Write), Some(&1));
        // Should NOT track in subagent_tool_counts
        assert!(stats.subagent_tool_counts.is_empty());
    }

    #[test]
    fn record_entry_tracks_subagent_tool_counts() {
        let mut stats = SessionStats::default();
        let message = make_message_with_tool_calls(vec![ToolName::Bash, ToolName::Grep]);
        let entry = make_log_entry("e1", "s1", Some("agent-123"), message);

        stats.record_entry(&entry);

        // Should track in subagent_tool_counts under the agent
        let agent_id = make_agent_id("agent-123");
        let agent_tools = stats
            .subagent_tool_counts
            .get(&agent_id)
            .expect("subagent tools");
        assert_eq!(agent_tools.get(&ToolName::Bash), Some(&1));
        assert_eq!(agent_tools.get(&ToolName::Grep), Some(&1));
        // Should NOT track in main_agent_tool_counts
        assert!(stats.main_agent_tool_counts.is_empty());
    }

    #[test]
    fn record_entry_accumulates_main_agent_tool_counts() {
        let mut stats = SessionStats::default();
        let message1 = make_message_with_tool_calls(vec![ToolName::Read]);
        let message2 = make_message_with_tool_calls(vec![ToolName::Read, ToolName::Bash]);
        let entry1 = make_log_entry("e1", "s1", None, message1);
        let entry2 = make_log_entry("e2", "s1", None, message2);

        stats.record_entry(&entry1);
        stats.record_entry(&entry2);

        assert_eq!(stats.main_agent_tool_counts.get(&ToolName::Read), Some(&2));
        assert_eq!(stats.main_agent_tool_counts.get(&ToolName::Bash), Some(&1));
    }

    #[test]
    fn record_entry_accumulates_subagent_tool_counts_for_same_agent() {
        let mut stats = SessionStats::default();
        let message1 = make_message_with_tool_calls(vec![ToolName::Edit]);
        let message2 = make_message_with_tool_calls(vec![ToolName::Edit, ToolName::Read]);
        let entry1 = make_log_entry("e1", "s1", Some("agent-abc"), message1);
        let entry2 = make_log_entry("e2", "s1", Some("agent-abc"), message2);

        stats.record_entry(&entry1);
        stats.record_entry(&entry2);

        let agent_id = make_agent_id("agent-abc");
        let agent_tools = stats
            .subagent_tool_counts
            .get(&agent_id)
            .expect("subagent tools");
        assert_eq!(agent_tools.get(&ToolName::Edit), Some(&2));
        assert_eq!(agent_tools.get(&ToolName::Read), Some(&1));
    }

    #[test]
    fn record_entry_keeps_subagent_tool_counts_separate() {
        let mut stats = SessionStats::default();
        let message1 = make_message_with_tool_calls(vec![ToolName::Bash]);
        let message2 = make_message_with_tool_calls(vec![ToolName::Read]);
        let entry1 = make_log_entry("e1", "s1", Some("agent-1"), message1);
        let entry2 = make_log_entry("e2", "s1", Some("agent-2"), message2);

        stats.record_entry(&entry1);
        stats.record_entry(&entry2);

        let agent1 = make_agent_id("agent-1");
        let agent2 = make_agent_id("agent-2");

        let agent1_tools = stats
            .subagent_tool_counts
            .get(&agent1)
            .expect("agent-1 tools");
        assert_eq!(agent1_tools.get(&ToolName::Bash), Some(&1));
        assert_eq!(agent1_tools.get(&ToolName::Read), None);

        let agent2_tools = stats
            .subagent_tool_counts
            .get(&agent2)
            .expect("agent-2 tools");
        assert_eq!(agent2_tools.get(&ToolName::Read), Some(&1));
        assert_eq!(agent2_tools.get(&ToolName::Bash), None);
    }

    #[test]
    fn filtered_tool_counts_returns_global_tools() {
        let mut tool_counts = HashMap::new();
        tool_counts.insert(ToolName::Read, 5);
        tool_counts.insert(ToolName::Write, 3);

        let stats = SessionStats {
            tool_counts: tool_counts.clone(),

            ..Default::default()
        };

        let filter = StatsFilter::AllSessionsCombined;
        let result = stats.filtered_tool_counts(&filter);

        assert_eq!(result.get(&ToolName::Read), Some(&5));
        assert_eq!(result.get(&ToolName::Write), Some(&3));
    }

    #[test]
    fn filtered_tool_counts_returns_main_agent_tools() {
        let mut global_counts = HashMap::new();
        global_counts.insert(ToolName::Read, 10);
        global_counts.insert(ToolName::Write, 8);

        let mut main_counts = HashMap::new();
        main_counts.insert(ToolName::Read, 6);
        main_counts.insert(ToolName::Write, 4);

        let stats = SessionStats {
            tool_counts: global_counts,
            main_agent_tool_counts: main_counts,

            ..Default::default()
        };

        let session_id = make_session_id("test-session");
        let filter = StatsFilter::MainAgent(session_id);
        let result = stats.filtered_tool_counts(&filter);

        // Should return only main agent counts
        assert_eq!(result.get(&ToolName::Read), Some(&6));
        assert_eq!(result.get(&ToolName::Write), Some(&4));
    }

    #[test]
    fn filtered_tool_counts_returns_specific_subagent_tools() {
        let agent1 = make_agent_id("agent-1");
        let agent2 = make_agent_id("agent-2");

        let mut agent1_counts = HashMap::new();
        agent1_counts.insert(ToolName::Bash, 7);
        agent1_counts.insert(ToolName::Read, 2);

        let mut agent2_counts = HashMap::new();
        agent2_counts.insert(ToolName::Edit, 3);

        let mut subagent_counts = HashMap::new();
        subagent_counts.insert(agent1.clone(), agent1_counts);
        subagent_counts.insert(agent2.clone(), agent2_counts);

        let stats = SessionStats {
            tool_counts: HashMap::new(),

            subagent_tool_counts: subagent_counts,
            ..Default::default()
        };

        let filter = StatsFilter::Subagent(agent1);
        let result = stats.filtered_tool_counts(&filter);

        // Should return only agent-1 counts
        assert_eq!(result.get(&ToolName::Bash), Some(&7));
        assert_eq!(result.get(&ToolName::Read), Some(&2));
        assert_eq!(result.get(&ToolName::Edit), None);
    }

    #[test]
    fn filtered_tool_counts_returns_empty_for_unknown_subagent() {
        let agent1 = make_agent_id("agent-1");
        let agent_missing = make_agent_id("agent-missing");

        let mut agent1_counts = HashMap::new();
        agent1_counts.insert(ToolName::Read, 5);

        let mut subagent_counts = HashMap::new();
        subagent_counts.insert(agent1.clone(), agent1_counts);

        let stats = SessionStats {
            tool_counts: HashMap::new(),

            subagent_tool_counts: subagent_counts,
            ..Default::default()
        };

        let filter = StatsFilter::Subagent(agent_missing);
        let result = stats.filtered_tool_counts(&filter);

        // Should return empty HashMap for missing agent
        assert!(result.is_empty());
    }

    // ===== FMT-010: Result Entry Cost Tracking Tests =====

    #[test]
    fn record_entry_extracts_cost_from_result_entry() {
        // RED TEST: SessionStats should extract total_cost_usd from Result entry's ResultMetadata
        use crate::model::{EntryType, ResultMetadata};

        let mut stats = SessionStats::default();

        // Create a Result entry with total_cost_usd = 1.5
        let result_metadata = ResultMetadata {
            is_error: false,
            duration_ms: 100000,
            num_turns: 10,
            total_cost_usd: 1.5,
            result_text: "Session complete".to_string(),
        };

        let message = Message::new(Role::Assistant, MessageContent::Text("Done".to_string()));
        let entry = LogEntry::new_with_result_metadata(
            make_uuid("result-1"),
            None,
            make_session_id("s1"),
            None,
            Utc::now(),
            EntryType::Result,
            message,
            EntryMetadata::default(),
            Some(result_metadata),
        );

        stats.record_entry(&entry);

        // Should extract total_cost_usd from result entry
        assert_eq!(
            stats.actual_cost_usd,
            Some(1.5),
            "Should extract total_cost_usd from Result entry"
        );
    }

    #[test]
    fn record_entry_does_not_extract_cost_from_non_result_entry() {
        // Non-result entries should not set actual_cost_usd
        let mut stats = SessionStats::default();

        let message = Message::new(Role::User, MessageContent::Text("Hello".to_string()));
        let entry = LogEntry::new(
            make_uuid("user-1"),
            None,
            make_session_id("s1"),
            None,
            Utc::now(),
            EntryType::User,
            message,
            EntryMetadata::default(),
        );

        stats.record_entry(&entry);

        // actual_cost_usd should remain None for non-result entries
        assert_eq!(
            stats.actual_cost_usd, None,
            "Non-result entries should not set actual_cost_usd"
        );
    }

    #[test]
    fn record_entry_overwrites_cost_with_latest_result_entry() {
        // If multiple result entries are seen (unusual but possible), use the latest
        use crate::model::{EntryType, ResultMetadata};

        let mut stats = SessionStats::default();

        // First result entry with cost = 1.0
        let result1 = ResultMetadata {
            is_error: false,
            duration_ms: 50000,
            num_turns: 5,
            total_cost_usd: 1.0,
            result_text: "First".to_string(),
        };
        let entry1 = LogEntry::new_with_result_metadata(
            make_uuid("result-1"),
            None,
            make_session_id("s1"),
            None,
            Utc::now(),
            EntryType::Result,
            Message::new(Role::Assistant, MessageContent::Text("".to_string())),
            EntryMetadata::default(),
            Some(result1),
        );

        stats.record_entry(&entry1);
        assert_eq!(stats.actual_cost_usd, Some(1.0));

        // Second result entry with cost = 2.5
        let result2 = ResultMetadata {
            is_error: false,
            duration_ms: 100000,
            num_turns: 10,
            total_cost_usd: 2.5,
            result_text: "Second".to_string(),
        };
        let entry2 = LogEntry::new_with_result_metadata(
            make_uuid("result-2"),
            None,
            make_session_id("s1"),
            None,
            Utc::now(),
            EntryType::Result,
            Message::new(Role::Assistant, MessageContent::Text("".to_string())),
            EntryMetadata::default(),
            Some(result2),
        );

        stats.record_entry(&entry2);

        // Should update to latest cost
        assert_eq!(
            stats.actual_cost_usd,
            Some(2.5),
            "Should update to latest result entry cost"
        );
    }

    // ===== Session-Scoped Statistics Tests =====

    #[test]
    fn record_entry_populates_session_usage_for_main_agent() {
        let mut stats = SessionStats::default();
        let usage = TokenUsage {
            input_tokens: 100,
            output_tokens: 50,
            cache_creation_input_tokens: 10,
            cache_read_input_tokens: 5,
            ephemeral_5m_input_tokens: 0,
            ephemeral_1h_input_tokens: 0,
        };
        let message = make_message_with_usage(usage);
        let entry = make_log_entry("e1", "session-1", None, message); // Main agent

        stats.record_entry(&entry);

        let session_id = make_session_id("session-1");
        let session_usage = stats
            .session_usage
            .get(&session_id)
            .expect("session usage should be tracked");
        assert_eq!(session_usage.input_tokens, 100);
        assert_eq!(session_usage.output_tokens, 50);
        assert_eq!(session_usage.cache_creation_input_tokens, 10);
        assert_eq!(session_usage.cache_read_input_tokens, 5);
    }

    #[test]
    fn record_entry_populates_session_usage_for_subagent() {
        let mut stats = SessionStats::default();
        let usage = TokenUsage {
            input_tokens: 200,
            output_tokens: 100,
            cache_creation_input_tokens: 0,
            cache_read_input_tokens: 0,
            ephemeral_5m_input_tokens: 0,
            ephemeral_1h_input_tokens: 0,
        };
        let message = make_message_with_usage(usage);
        let entry = make_log_entry("e1", "session-2", Some("agent-123"), message); // Subagent

        stats.record_entry(&entry);

        let session_id = make_session_id("session-2");
        let session_usage = stats
            .session_usage
            .get(&session_id)
            .expect("session usage should be tracked");
        assert_eq!(session_usage.input_tokens, 200);
        assert_eq!(session_usage.output_tokens, 100);
    }

    #[test]
    fn record_entry_accumulates_session_usage_across_agents() {
        let mut stats = SessionStats::default();

        // Main agent entry for session-1
        let usage1 = TokenUsage {
            input_tokens: 100,
            output_tokens: 50,
            cache_creation_input_tokens: 0,
            cache_read_input_tokens: 0,
            ephemeral_5m_input_tokens: 0,
            ephemeral_1h_input_tokens: 0,
        };
        let entry1 = make_log_entry("e1", "session-1", None, make_message_with_usage(usage1));

        // Subagent entry for session-1
        let usage2 = TokenUsage {
            input_tokens: 200,
            output_tokens: 100,
            cache_creation_input_tokens: 0,
            cache_read_input_tokens: 0,
            ephemeral_5m_input_tokens: 0,
            ephemeral_1h_input_tokens: 0,
        };
        let entry2 = make_log_entry(
            "e2",
            "session-1",
            Some("agent-123"),
            make_message_with_usage(usage2),
        );

        stats.record_entry(&entry1);
        stats.record_entry(&entry2);

        let session_id = make_session_id("session-1");
        let session_usage = stats
            .session_usage
            .get(&session_id)
            .expect("session usage should be tracked");

        // Should accumulate both main and subagent
        assert_eq!(session_usage.input_tokens, 300);
        assert_eq!(session_usage.output_tokens, 150);
    }

    #[test]
    fn record_entry_tracks_separate_sessions() {
        let mut stats = SessionStats::default();

        let usage1 = TokenUsage {
            input_tokens: 100,
            output_tokens: 50,
            cache_creation_input_tokens: 0,
            cache_read_input_tokens: 0,
            ephemeral_5m_input_tokens: 0,
            ephemeral_1h_input_tokens: 0,
        };
        let entry1 = make_log_entry("e1", "session-1", None, make_message_with_usage(usage1));

        let usage2 = TokenUsage {
            input_tokens: 200,
            output_tokens: 100,
            cache_creation_input_tokens: 0,
            cache_read_input_tokens: 0,
            ephemeral_5m_input_tokens: 0,
            ephemeral_1h_input_tokens: 0,
        };
        let entry2 = make_log_entry("e2", "session-2", None, make_message_with_usage(usage2));

        stats.record_entry(&entry1);
        stats.record_entry(&entry2);

        let session1_id = make_session_id("session-1");
        let session2_id = make_session_id("session-2");

        let session1_usage = stats.session_usage.get(&session1_id).expect("session-1");
        assert_eq!(session1_usage.input_tokens, 100);

        let session2_usage = stats.session_usage.get(&session2_id).expect("session-2");
        assert_eq!(session2_usage.input_tokens, 200);
    }

    #[test]
    fn record_entry_populates_main_agent_usage_by_session() {
        let mut stats = SessionStats::default();
        let usage = TokenUsage {
            input_tokens: 150,
            output_tokens: 75,
            cache_creation_input_tokens: 0,
            cache_read_input_tokens: 0,
            ephemeral_5m_input_tokens: 0,
            ephemeral_1h_input_tokens: 0,
        };
        let message = make_message_with_usage(usage);
        let entry = make_log_entry("e1", "session-3", None, message); // Main agent

        stats.record_entry(&entry);

        let session_id = make_session_id("session-3");
        let main_usage = stats
            .main_agent_usage_by_session
            .get(&session_id)
            .expect("main agent usage by session should be tracked");
        assert_eq!(main_usage.input_tokens, 150);
        assert_eq!(main_usage.output_tokens, 75);
    }

    #[test]
    fn record_entry_does_not_populate_main_agent_usage_by_session_for_subagent() {
        let mut stats = SessionStats::default();
        let usage = TokenUsage {
            input_tokens: 250,
            output_tokens: 125,
            cache_creation_input_tokens: 0,
            cache_read_input_tokens: 0,
            ephemeral_5m_input_tokens: 0,
            ephemeral_1h_input_tokens: 0,
        };
        let message = make_message_with_usage(usage);
        let entry = make_log_entry("e1", "session-4", Some("agent-456"), message); // Subagent

        stats.record_entry(&entry);

        let session_id = make_session_id("session-4");
        // Should NOT populate main_agent_usage_by_session for subagent
        assert!(!stats.main_agent_usage_by_session.contains_key(&session_id));
    }

    #[test]
    fn record_entry_accumulates_main_agent_usage_by_session() {
        let mut stats = SessionStats::default();

        let usage1 = TokenUsage {
            input_tokens: 100,
            output_tokens: 50,
            cache_creation_input_tokens: 0,
            cache_read_input_tokens: 0,
            ephemeral_5m_input_tokens: 0,
            ephemeral_1h_input_tokens: 0,
        };
        let entry1 = make_log_entry("e1", "session-5", None, make_message_with_usage(usage1));

        let usage2 = TokenUsage {
            input_tokens: 200,
            output_tokens: 100,
            cache_creation_input_tokens: 0,
            cache_read_input_tokens: 0,
            ephemeral_5m_input_tokens: 0,
            ephemeral_1h_input_tokens: 0,
        };
        let entry2 = make_log_entry("e2", "session-5", None, make_message_with_usage(usage2));

        stats.record_entry(&entry1);
        stats.record_entry(&entry2);

        let session_id = make_session_id("session-5");
        let main_usage = stats
            .main_agent_usage_by_session
            .get(&session_id)
            .expect("main agent usage should accumulate");

        assert_eq!(main_usage.input_tokens, 300);
        assert_eq!(main_usage.output_tokens, 150);
    }

    #[test]
    fn filtered_usage_session_returns_session_scoped_usage() {
        let mut stats = SessionStats::default();

        // Session 1: main (100) + subagent (200) = 300 input
        let entry1 = make_log_entry(
            "e1",
            "session-1",
            None,
            make_message_with_usage(TokenUsage {
                input_tokens: 100,
                output_tokens: 50,
                cache_creation_input_tokens: 0,
                cache_read_input_tokens: 0,
                ephemeral_5m_input_tokens: 0,
                ephemeral_1h_input_tokens: 0,
            }),
        );
        let entry2 = make_log_entry(
            "e2",
            "session-1",
            Some("agent-123"),
            make_message_with_usage(TokenUsage {
                input_tokens: 200,
                output_tokens: 100,
                cache_creation_input_tokens: 0,
                cache_read_input_tokens: 0,
                ephemeral_5m_input_tokens: 0,
                ephemeral_1h_input_tokens: 0,
            }),
        );

        stats.record_entry(&entry1);
        stats.record_entry(&entry2);

        let session_id = make_session_id("session-1");
        let filter = StatsFilter::Session(session_id);
        let usage = stats.filtered_usage(&filter);

        assert_eq!(usage.input_tokens, 300);
        assert_eq!(usage.output_tokens, 150);
    }

    #[test]
    fn filtered_usage_main_agent_returns_session_scoped_main_usage() {
        let mut stats = SessionStats::default();

        // Session 1: main (150) + subagent (ignored for MainAgent filter)
        let entry1 = make_log_entry(
            "e1",
            "session-1",
            None,
            make_message_with_usage(TokenUsage {
                input_tokens: 150,
                output_tokens: 75,
                cache_creation_input_tokens: 0,
                cache_read_input_tokens: 0,
                ephemeral_5m_input_tokens: 0,
                ephemeral_1h_input_tokens: 0,
            }),
        );
        let entry2 = make_log_entry(
            "e2",
            "session-1",
            Some("agent-456"),
            make_message_with_usage(TokenUsage {
                input_tokens: 300,
                output_tokens: 150,
                cache_creation_input_tokens: 0,
                cache_read_input_tokens: 0,
                ephemeral_5m_input_tokens: 0,
                ephemeral_1h_input_tokens: 0,
            }),
        );

        stats.record_entry(&entry1);
        stats.record_entry(&entry2);

        let session_id = make_session_id("session-1");
        let filter = StatsFilter::MainAgent(session_id);
        let usage = stats.filtered_usage(&filter);

        // Should only include main agent usage (150), not subagent (300)
        assert_eq!(usage.input_tokens, 150);
        assert_eq!(usage.output_tokens, 75);
    }

    #[test]
    fn filtered_usage_session_returns_zero_for_unknown_session() {
        let stats = SessionStats::default();
        let session_id = make_session_id("unknown-session");
        let filter = StatsFilter::Session(session_id);
        let usage = stats.filtered_usage(&filter);

        assert_eq!(usage.input_tokens, 0);
        assert_eq!(usage.output_tokens, 0);
    }

    #[test]
    fn filtered_usage_main_agent_returns_zero_for_unknown_session() {
        let stats = SessionStats::default();
        let session_id = make_session_id("unknown-session");
        let filter = StatsFilter::MainAgent(session_id);
        let usage = stats.filtered_usage(&filter);

        assert_eq!(usage.input_tokens, 0);
        assert_eq!(usage.output_tokens, 0);
    }

    // ===== Invariant Tests (from contract) =====

    #[test]
    fn invariant_all_sessions_equals_sum_of_sessions() {
        let mut stats = SessionStats::default();

        // Session 1: 100 input
        stats.record_entry(&make_log_entry(
            "e1",
            "session-1",
            None,
            make_message_with_usage(TokenUsage {
                input_tokens: 100,
                output_tokens: 50,
                cache_creation_input_tokens: 0,
                cache_read_input_tokens: 0,
                ephemeral_5m_input_tokens: 0,
                ephemeral_1h_input_tokens: 0,
            }),
        ));

        // Session 2: 200 input
        stats.record_entry(&make_log_entry(
            "e2",
            "session-2",
            None,
            make_message_with_usage(TokenUsage {
                input_tokens: 200,
                output_tokens: 100,
                cache_creation_input_tokens: 0,
                cache_read_input_tokens: 0,
                ephemeral_5m_input_tokens: 0,
                ephemeral_1h_input_tokens: 0,
            }),
        ));

        // AllSessionsCombined should equal sum of individual sessions
        let all_combined = stats.filtered_usage(&StatsFilter::AllSessionsCombined);

        let session1_usage =
            stats.filtered_usage(&StatsFilter::Session(make_session_id("session-1")));
        let session2_usage =
            stats.filtered_usage(&StatsFilter::Session(make_session_id("session-2")));

        let sum_input = session1_usage.input_tokens + session2_usage.input_tokens;
        let sum_output = session1_usage.output_tokens + session2_usage.output_tokens;

        assert_eq!(all_combined.input_tokens, sum_input);
        assert_eq!(all_combined.output_tokens, sum_output);
    }

    #[test]
    fn invariant_session_equals_main_plus_subagents() {
        let mut stats = SessionStats::default();

        // Main agent: 100 input
        stats.record_entry(&make_log_entry(
            "e1",
            "session-1",
            None,
            make_message_with_usage(TokenUsage {
                input_tokens: 100,
                output_tokens: 50,
                cache_creation_input_tokens: 0,
                cache_read_input_tokens: 0,
                ephemeral_5m_input_tokens: 0,
                ephemeral_1h_input_tokens: 0,
            }),
        ));

        // Subagent 1: 200 input
        stats.record_entry(&make_log_entry(
            "e2",
            "session-1",
            Some("agent-1"),
            make_message_with_usage(TokenUsage {
                input_tokens: 200,
                output_tokens: 100,
                cache_creation_input_tokens: 0,
                cache_read_input_tokens: 0,
                ephemeral_5m_input_tokens: 0,
                ephemeral_1h_input_tokens: 0,
            }),
        ));

        // Subagent 2: 300 input
        stats.record_entry(&make_log_entry(
            "e3",
            "session-1",
            Some("agent-2"),
            make_message_with_usage(TokenUsage {
                input_tokens: 300,
                output_tokens: 150,
                cache_creation_input_tokens: 0,
                cache_read_input_tokens: 0,
                ephemeral_5m_input_tokens: 0,
                ephemeral_1h_input_tokens: 0,
            }),
        ));

        let session_id = make_session_id("session-1");
        let session_usage = stats.filtered_usage(&StatsFilter::Session(session_id.clone()));
        let main_usage = stats.filtered_usage(&StatsFilter::MainAgent(session_id));
        let sub1_usage = stats.filtered_usage(&StatsFilter::Subagent(make_agent_id("agent-1")));
        let sub2_usage = stats.filtered_usage(&StatsFilter::Subagent(make_agent_id("agent-2")));

        let sum_input = main_usage.input_tokens + sub1_usage.input_tokens + sub2_usage.input_tokens;
        let sum_output =
            main_usage.output_tokens + sub1_usage.output_tokens + sub2_usage.output_tokens;

        assert_eq!(session_usage.input_tokens, sum_input);
        assert_eq!(session_usage.output_tokens, sum_output);
    }
}
