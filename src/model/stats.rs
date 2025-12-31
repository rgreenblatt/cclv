//! Session statistics and cost estimation.
//!
//! This module provides aggregated statistics for sessions, including token usage,
//! tool counts, and estimated costs based on pricing configuration.

use crate::model::{AgentId, LogEntry, TokenUsage, ToolName};
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
    /// Corresponds to StatsFilter::Global (FR-020). This is the sum of
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

    /// Total tool invocation counts across all agents, grouped by tool name (FR-018).
    ///
    /// Tracks how many times each tool (Read, Write, Bash, etc.) was invoked across
    /// the entire session. Corresponds to StatsFilter::Global.
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
}

impl SessionStats {
    /// Record statistics from a log entry.
    ///
    /// This method:
    /// - Increments entry_count
    /// - Accumulates token usage to total_usage
    /// - Routes usage to main_agent_usage or subagent_usage based on agent_id
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

            // Route to main agent or subagent
            if let Some(agent_id) = entry.agent_id() {
                // Subagent usage
                let agent_usage = self.subagent_usage.entry(agent_id.clone()).or_default();
                agent_usage.input_tokens += usage.input_tokens;
                agent_usage.output_tokens += usage.output_tokens;
            } else {
                // Main agent usage
                self.main_agent_usage.input_tokens += usage.input_tokens;
                self.main_agent_usage.output_tokens += usage.output_tokens;
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
    /// - `StatsFilter::Global`: total_usage (all agents)
    /// - `StatsFilter::MainAgent`: main_agent_usage only
    /// - `StatsFilter::Subagent(id)`: usage for specific subagent, or default if not found
    pub fn filtered_usage(&self, filter: &StatsFilter) -> TokenUsage {
        match filter {
            StatsFilter::Global => self.total_usage,
            StatsFilter::MainAgent => self.main_agent_usage,
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
    /// - `StatsFilter::Global`: tool_counts (all agents)
    /// - `StatsFilter::MainAgent`: main_agent_tool_counts only
    /// - `StatsFilter::Subagent(id)`: tool_counts for specific subagent, or empty if not found
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

// ===== StatsFilter =====

/// Filter for statistics display.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StatsFilter {
    /// Show statistics for the entire session (all agents)
    Global,
    /// Show statistics for the main agent only
    MainAgent,
    /// Show statistics for a specific subagent
    Subagent(AgentId),
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
        };
        let usage2 = TokenUsage {
            input_tokens: 200,
            output_tokens: 75,
            cache_creation_input_tokens: 0,
            cache_read_input_tokens: 0,
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
            },
            main_agent_tool_counts: HashMap::new(),
            subagent_tool_counts: HashMap::new(),
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
            },
            main_agent_tool_counts: HashMap::new(),
            subagent_tool_counts: HashMap::new(),
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
            },
            main_agent_tool_counts: HashMap::new(),
            subagent_tool_counts: HashMap::new(),
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
            },
            main_agent_tool_counts: HashMap::new(),
            subagent_tool_counts: HashMap::new(),
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
            },
            main_agent_tool_counts: HashMap::new(),
            subagent_tool_counts: HashMap::new(),
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
            },
            main_agent_tool_counts: HashMap::new(),
            subagent_tool_counts: HashMap::new(),
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
            },
            main_agent_tool_counts: HashMap::new(),
            subagent_tool_counts: HashMap::new(),
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
            },
            main_agent_tool_counts: HashMap::new(),
            subagent_tool_counts: HashMap::new(),
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
    fn stats_filter_global_equality() {
        assert_eq!(StatsFilter::Global, StatsFilter::Global);
    }

    #[test]
    fn stats_filter_main_agent_equality() {
        assert_eq!(StatsFilter::MainAgent, StatsFilter::MainAgent);
    }

    #[test]
    fn stats_filter_subagent_equality() {
        let agent1 = make_agent_id("agent-1");
        let agent2 = make_agent_id("agent-1");
        assert_eq!(StatsFilter::Subagent(agent1), StatsFilter::Subagent(agent2));
    }

    #[test]
    fn stats_filter_different_variants_not_equal() {
        assert_ne!(StatsFilter::Global, StatsFilter::MainAgent);
    }

    #[test]
    fn stats_filter_different_subagents_not_equal() {
        let agent1 = make_agent_id("agent-1");
        let agent2 = make_agent_id("agent-2");
        assert_ne!(StatsFilter::Subagent(agent1), StatsFilter::Subagent(agent2));
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
            },
            main_agent_usage: TokenUsage {
                input_tokens: 600,
                output_tokens: 300,
                cache_creation_input_tokens: 0,
                cache_read_input_tokens: 0,
            },
            subagent_usage: HashMap::new(),
            tool_counts: HashMap::new(),
            main_agent_tool_counts: HashMap::new(),
            subagent_tool_counts: HashMap::new(),
            subagent_count: 0,
            entry_count: 10,
        };

        let filter = StatsFilter::Global;
        let usage = stats.filtered_usage(&filter);

        assert_eq!(usage.input_tokens, 1000);
        assert_eq!(usage.output_tokens, 500);
        assert_eq!(usage.cache_creation_input_tokens, 100);
        assert_eq!(usage.cache_read_input_tokens, 50);
    }

    #[test]
    fn filtered_usage_main_agent_returns_main_agent_usage() {
        let stats = SessionStats {
            total_usage: TokenUsage {
                input_tokens: 1000,
                output_tokens: 500,
                cache_creation_input_tokens: 100,
                cache_read_input_tokens: 50,
            },
            main_agent_usage: TokenUsage {
                input_tokens: 600,
                output_tokens: 300,
                cache_creation_input_tokens: 0,
                cache_read_input_tokens: 0,
            },
            subagent_usage: HashMap::new(),
            tool_counts: HashMap::new(),
            main_agent_tool_counts: HashMap::new(),
            subagent_tool_counts: HashMap::new(),
            subagent_count: 0,
            entry_count: 10,
        };

        let filter = StatsFilter::MainAgent;
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
            },
        );
        subagent_usage.insert(
            agent2.clone(),
            TokenUsage {
                input_tokens: 300,
                output_tokens: 150,
                cache_creation_input_tokens: 0,
                cache_read_input_tokens: 0,
            },
        );

        let stats = SessionStats {
            total_usage: TokenUsage {
                input_tokens: 1000,
                output_tokens: 500,
                cache_creation_input_tokens: 0,
                cache_read_input_tokens: 0,
            },
            main_agent_usage: TokenUsage {
                input_tokens: 300,
                output_tokens: 150,
                cache_creation_input_tokens: 0,
                cache_read_input_tokens: 0,
            },
            subagent_usage,
            tool_counts: HashMap::new(),
            main_agent_tool_counts: HashMap::new(),
            subagent_tool_counts: HashMap::new(),
            subagent_count: 2,
            entry_count: 20,
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
            },
        );

        let stats = SessionStats {
            total_usage: TokenUsage {
                input_tokens: 700,
                output_tokens: 350,
                cache_creation_input_tokens: 0,
                cache_read_input_tokens: 0,
            },
            main_agent_usage: TokenUsage {
                input_tokens: 300,
                output_tokens: 150,
                cache_creation_input_tokens: 0,
                cache_read_input_tokens: 0,
            },
            subagent_usage,
            tool_counts: HashMap::new(),
            main_agent_tool_counts: HashMap::new(),
            subagent_tool_counts: HashMap::new(),
            subagent_count: 1,
            entry_count: 15,
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
            main_agent_tool_counts: HashMap::new(),
            subagent_tool_counts: HashMap::new(),
            ..Default::default()
        };

        let filter = StatsFilter::Global;
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
            subagent_tool_counts: HashMap::new(),
            ..Default::default()
        };

        let filter = StatsFilter::MainAgent;
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
            main_agent_tool_counts: HashMap::new(),
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
            main_agent_tool_counts: HashMap::new(),
            subagent_tool_counts: subagent_counts,
            ..Default::default()
        };

        let filter = StatsFilter::Subagent(agent_missing);
        let result = stats.filtered_tool_counts(&filter);

        // Should return empty HashMap for missing agent
        assert!(result.is_empty());
    }
}
