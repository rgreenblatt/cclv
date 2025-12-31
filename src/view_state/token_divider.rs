//! Token statistics divider rendering for conversation entries.
//!
//! Displays per-entry token usage, cost, and accumulated context window usage
//! as a subtle divider line between entries (FR-XXX).

use crate::model::{stats::PricingConfig, TokenUsage};
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

// ===== ContextWindowTokens =====

/// Maximum context window size in tokens.
///
/// Represents the model's context window limit (e.g., 200,000 tokens for Claude Opus 4.5).
/// Used to calculate percentage of context window consumed by accumulated tokens.
///
/// # Configuration
///
/// Default: 200,000 tokens (FR-XXX)
/// Configurable via `config.toml`:
/// ```toml
/// max_context_tokens = 200000
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContextWindowTokens(u64);

impl ContextWindowTokens {
    /// Create a new ContextWindowTokens.
    ///
    /// # Arguments
    /// * `tokens` - Maximum context window size in tokens
    ///
    /// # Returns
    /// ContextWindowTokens instance
    ///
    /// # Examples
    /// ```
    /// # use cclv::view_state::token_divider::ContextWindowTokens;
    /// let max = ContextWindowTokens::new(200_000);
    /// assert_eq!(max.get(), 200_000);
    /// ```
    pub const fn new(tokens: u64) -> Self {
        Self(tokens)
    }

    /// Get the maximum context window size in tokens.
    pub const fn get(&self) -> u64 {
        self.0
    }
}

impl Default for ContextWindowTokens {
    fn default() -> Self {
        Self(200_000)
    }
}

// ===== Token Divider Rendering =====

/// Render a token statistics divider line for a conversation entry.
///
/// Displays:
/// - Entry token usage (input/output)
/// - Entry cost (calculated from usage + pricing)
/// - Accumulated context so far (sum of all prior entries' input tokens)
/// - Percentage of context window used (accumulated / max_context * 100)
///
/// Format: `── 1.2k in / 340 out / $0.02 | Context: 45.2k (23%) ──`
///
/// # Arguments
/// * `entry_usage` - Token usage for this specific entry
/// * `accumulated_input_tokens` - Total input tokens accumulated up to (and including) this entry
/// * `max_context` - Maximum context window size
/// * `pricing` - Pricing configuration for cost calculation
/// * `model_id` - Model ID for pricing lookup (e.g., "opus", "sonnet")
///
/// # Returns
/// A single Line with the divider text and dim gray styling
///
/// # Examples
/// ```ignore
/// let usage = TokenUsage {
///     input_tokens: 1200,
///     output_tokens: 340,
///     cache_creation_input_tokens: 0,
///     cache_read_input_tokens: 0,
///     ephemeral_5m_input_tokens: 0,
///     ephemeral_1h_input_tokens: 0,
/// };
/// let accumulated = 45_200;
/// let max_context = ContextWindowTokens::new(200_000);
/// let pricing = PricingConfig::default();
///
/// let divider = render_token_divider(&usage, accumulated, max_context, &pricing, Some("opus"));
/// // Returns line: "── 1.2k in / 340 out / $0.02 | Context: 45.2k (23%) ──"
/// ```
pub fn render_token_divider(
    entry_usage: &TokenUsage,
    accumulated_input_tokens: u64,
    max_context: ContextWindowTokens,
    pricing: &PricingConfig,
    model_id: Option<&str>,
) -> Line<'static> {
    // Calculate entry cost using pricing config
    let model_pricing = pricing.get(model_id.unwrap_or("opus"));

    let input_cost =
        (entry_usage.input_tokens as f64 / 1_000_000.0) * model_pricing.input_cost_per_million;

    let output_cost =
        (entry_usage.output_tokens as f64 / 1_000_000.0) * model_pricing.output_cost_per_million;

    // Use cached rate if available, otherwise use standard input rate
    let cache_rate = model_pricing
        .cached_input_cost_per_million
        .unwrap_or(model_pricing.input_cost_per_million);

    let cache_cost = ((entry_usage.cache_creation_input_tokens
        + entry_usage.cache_read_input_tokens) as f64
        / 1_000_000.0)
        * cache_rate;

    let total_cost = input_cost + output_cost + cache_cost;

    // Format token counts
    let input_str = format_token_count(entry_usage.input_tokens);
    let output_str = format_token_count(entry_usage.output_tokens);
    let cost_str = format_cost(total_cost);
    let context_str = format_token_count(accumulated_input_tokens);
    let percentage = calculate_context_percentage(accumulated_input_tokens, max_context);

    // Build divider text
    let divider_text = format!(
        "── {} in / {} out / {} | Context: {} ({}%) ──",
        input_str, output_str, cost_str, context_str, percentage
    );

    // Return line with dim gray styling
    Line::from(vec![Span::styled(
        divider_text,
        Style::default()
            .fg(Color::DarkGray)
            .add_modifier(Modifier::DIM),
    )])
}

/// Format token count in human-readable form (e.g., 1200 -> "1.2k", 45200 -> "45.2k").
///
/// Rules:
/// - < 1000: show exact count (e.g., "340")
/// - >= 1000: show with k suffix, 1 decimal place (e.g., "1.2k", "45.2k")
/// - >= 1_000_000: show with M suffix, 1 decimal place (e.g., "1.5M")
///
/// # Arguments
/// * `tokens` - Token count to format
///
/// # Returns
/// Formatted string
///
/// # Examples
/// ```ignore
/// assert_eq!(format_token_count(340), "340");
/// assert_eq!(format_token_count(1200), "1.2k");
/// assert_eq!(format_token_count(45200), "45.2k");
/// assert_eq!(format_token_count(1_500_000), "1.5M");
/// ```
#[allow(dead_code)]
fn format_token_count(tokens: u64) -> String {
    if tokens < 1_000 {
        // Less than 1k: show exact count
        tokens.to_string()
    } else if tokens < 1_000_000 {
        // 1k to 1M: show with k suffix, 1 decimal place
        let k = tokens as f64 / 1000.0;
        format!("{:.1}k", k)
    } else {
        // 1M and above: show with M suffix, 1 decimal place
        let m = tokens as f64 / 1_000_000.0;
        format!("{:.1}M", m)
    }
}

/// Format cost in USD (e.g., 0.0234 -> "$0.02", 1.234 -> "$1.23").
///
/// Rules:
/// - Always show 2 decimal places
/// - Always include $ prefix
///
/// # Arguments
/// * `cost_usd` - Cost in USD
///
/// # Returns
/// Formatted string
///
/// # Examples
/// ```ignore
/// assert_eq!(format_cost(0.0234), "$0.02");
/// assert_eq!(format_cost(1.234), "$1.23");
/// assert_eq!(format_cost(0.001), "$0.00");
/// ```
#[allow(dead_code)]
fn format_cost(cost_usd: f64) -> String {
    format!("${:.2}", cost_usd)
}

/// Calculate percentage of context window used.
///
/// Formula: (accumulated_input_tokens / max_context_tokens) * 100
///
/// # Arguments
/// * `accumulated_input_tokens` - Total input tokens accumulated
/// * `max_context` - Maximum context window size
///
/// # Returns
/// Percentage as integer (0-100), clamped to 100 if exceeds
///
/// # Examples
/// ```ignore
/// let max = ContextWindowTokens::new(200_000);
/// assert_eq!(calculate_context_percentage(45_200, max), 23);
/// assert_eq!(calculate_context_percentage(100_000, max), 50);
/// assert_eq!(calculate_context_percentage(250_000, max), 100); // Clamped
/// ```
#[allow(dead_code)]
fn calculate_context_percentage(
    accumulated_input_tokens: u64,
    max_context: ContextWindowTokens,
) -> u8 {
    let percentage = (accumulated_input_tokens as f64 / max_context.get() as f64) * 100.0;
    percentage.min(100.0) as u8
}

#[cfg(test)]
#[path = "token_divider_tests.rs"]
mod tests;
