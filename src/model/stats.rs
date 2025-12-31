//! Session statistics and cost estimation.
//!
//! This module provides aggregated statistics for sessions, including token usage,
//! tool counts, and estimated costs based on pricing configuration.

use crate::model::{AgentId, LogEntry, ToolName, TokenUsage};
use std::collections::HashMap;

// ===== SessionStats =====

/// Aggregated session statistics.
///
/// Invariant: Statistics are incrementally recorded as entries are processed.
/// `total_usage` equals the sum of `main_agent_usage` and all `subagent_usage` values.
#[derive(Debug, Clone, Default)]
pub struct SessionStats {
    pub total_usage: TokenUsage,
    pub main_agent_usage: TokenUsage,
    pub subagent_usage: HashMap<AgentId, TokenUsage>,
    pub tool_counts: HashMap<ToolName, u32>,
    pub subagent_count: usize,
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
                agent_usage.cache_creation_input_tokens += usage.cache_creation_input_tokens;
                agent_usage.cache_read_input_tokens += usage.cache_read_input_tokens;
            } else {
                // Main agent usage
                self.main_agent_usage.input_tokens += usage.input_tokens;
                self.main_agent_usage.output_tokens += usage.output_tokens;
                self.main_agent_usage.cache_creation_input_tokens += usage.cache_creation_input_tokens;
                self.main_agent_usage.cache_read_input_tokens += usage.cache_read_input_tokens;
            }
        }

        // Count tool calls
        for tool in entry.message().tool_calls() {
            *self.tool_counts.entry(tool.name().clone()).or_default() += 1;
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

    /// Merge pricing from a file configuration section.
    ///
    /// This is used when loading pricing from config files.
    /// Not part of the public API - used by config module.
    #[allow(dead_code)]
    fn merge(&mut self, _other: HashMap<String, ModelPricing>) {
        todo!("PricingConfig::merge")
    }
}

// ===== ModelPricing =====

/// Pricing for a specific model (per million tokens, in USD).
#[derive(Debug, Clone, Copy)]
pub struct ModelPricing {
    pub input_cost_per_million: f64,
    pub output_cost_per_million: f64,
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
        let entry1 = make_log_entry("e1", "s1", Some("agent-abc"), make_message_with_usage(usage1));
        let entry2 = make_log_entry("e2", "s1", Some("agent-abc"), make_message_with_usage(usage2));

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
        let entry1 = make_log_entry("e1", "s1", Some("agent-1"), make_message_with_usage(TokenUsage::default()));
        let entry2 = make_log_entry("e2", "s1", Some("agent-2"), make_message_with_usage(TokenUsage::default()));
        let entry3 = make_log_entry("e3", "s1", Some("agent-1"), make_message_with_usage(TokenUsage::default()));

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
        assert_eq!(
            StatsFilter::Subagent(agent1),
            StatsFilter::Subagent(agent2)
        );
    }

    #[test]
    fn stats_filter_different_variants_not_equal() {
        assert_ne!(StatsFilter::Global, StatsFilter::MainAgent);
    }

    #[test]
    fn stats_filter_different_subagents_not_equal() {
        let agent1 = make_agent_id("agent-1");
        let agent2 = make_agent_id("agent-2");
        assert_ne!(
            StatsFilter::Subagent(agent1),
            StatsFilter::Subagent(agent2)
        );
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
}
