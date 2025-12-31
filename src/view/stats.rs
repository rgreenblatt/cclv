//! Statistics panel widget for displaying session metrics.

use crate::model::{PricingConfig, SessionStats, StatsFilter};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, Borders, Paragraph, Widget},
};

// ===== StatsPanel Widget =====

/// Statistics panel widget.
///
/// Displays:
/// - Token usage (input, output, total)
/// - Estimated cost
/// - Tool usage breakdown
/// - Subagent count
pub struct StatsPanel<'a> {
    stats: &'a SessionStats,
    filter: &'a StatsFilter,
    pricing: &'a PricingConfig,
    model_id: Option<&'a str>,
}

impl<'a> StatsPanel<'a> {
    /// Create a new StatsPanel widget.
    ///
    /// # Arguments
    /// * `stats` - Session statistics to display
    /// * `filter` - Current stats filter (Global, MainAgent, or Subagent)
    /// * `pricing` - Pricing configuration for cost estimation
    /// * `model_id` - Model ID for pricing lookup (defaults to "opus" if None)
    pub fn new(
        stats: &'a SessionStats,
        filter: &'a StatsFilter,
        pricing: &'a PricingConfig,
        model_id: Option<&'a str>,
    ) -> Self {
        Self {
            stats,
            filter,
            pricing,
            model_id,
        }
    }
}

impl<'a> Widget for StatsPanel<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        todo!("StatsPanel::render")
    }
}

// ===== Formatting Helpers =====

/// Format a token count with thousands separators.
///
/// Examples:
/// - `format_tokens(0)` → "0"
/// - `format_tokens(1234)` → "1,234"
/// - `format_tokens(1234567)` → "1,234,567"
fn format_tokens(tokens: u64) -> String {
    todo!("format_tokens")
}

/// Format a cost value in USD.
///
/// Examples:
/// - `format_cost(0.0)` → "$0.00"
/// - `format_cost(2.45)` → "$2.45"
/// - `format_cost(123.456)` → "$123.46" (rounds to 2 decimals)
fn format_cost(cost: f64) -> String {
    todo!("format_cost")
}

// ===== Tests =====

#[cfg(test)]
mod tests {
    use super::*;

    // ===== format_tokens tests =====

    #[test]
    fn format_tokens_zero() {
        assert_eq!(format_tokens(0), "0");
    }

    #[test]
    fn format_tokens_small_value() {
        assert_eq!(format_tokens(123), "123");
    }

    #[test]
    fn format_tokens_thousands() {
        assert_eq!(format_tokens(1234), "1,234");
    }

    #[test]
    fn format_tokens_millions() {
        assert_eq!(format_tokens(1234567), "1,234,567");
    }

    #[test]
    fn format_tokens_large_value() {
        assert_eq!(format_tokens(123456789), "123,456,789");
    }

    // ===== format_cost tests =====

    #[test]
    fn format_cost_zero() {
        assert_eq!(format_cost(0.0), "$0.00");
    }

    #[test]
    fn format_cost_small_value() {
        assert_eq!(format_cost(2.45), "$2.45");
    }

    #[test]
    fn format_cost_rounds_to_two_decimals() {
        assert_eq!(format_cost(123.456), "$123.46");
    }

    #[test]
    fn format_cost_large_value() {
        assert_eq!(format_cost(12345.67), "$12,345.67");
    }

    #[test]
    fn format_cost_rounds_down() {
        assert_eq!(format_cost(1.234), "$1.23");
    }

    // ===== Widget rendering tests =====

    #[test]
    fn stats_panel_renders_without_panic_empty_stats() {
        use ratatui::backend::TestBackend;
        use ratatui::buffer::Buffer;
        use ratatui::layout::Rect;

        let stats = SessionStats::default();
        let filter = StatsFilter::Global;
        let pricing = PricingConfig::default();

        let panel = StatsPanel::new(&stats, &filter, &pricing, Some("opus"));

        let mut buffer = Buffer::empty(Rect::new(0, 0, 40, 20));
        panel.render(Rect::new(0, 0, 40, 20), &mut buffer);

        // If we get here without panic, test passes
    }

    #[test]
    fn stats_panel_renders_without_panic_with_data() {
        use crate::model::{TokenUsage, ToolName};
        use ratatui::backend::TestBackend;
        use ratatui::buffer::Buffer;
        use ratatui::layout::Rect;
        use std::collections::HashMap;

        let mut tool_counts = HashMap::new();
        tool_counts.insert(ToolName::Read, 5);
        tool_counts.insert(ToolName::Write, 3);

        let stats = SessionStats {
            total_usage: TokenUsage {
                input_tokens: 1000,
                output_tokens: 500,
                cache_creation_input_tokens: 100,
                cache_read_input_tokens: 50,
            },
            main_agent_usage: TokenUsage::default(),
            subagent_usage: HashMap::new(),
            tool_counts,
            subagent_count: 2,
            entry_count: 10,
        };

        let filter = StatsFilter::Global;
        let pricing = PricingConfig::default();

        let panel = StatsPanel::new(&stats, &filter, &pricing, Some("sonnet"));

        let mut buffer = Buffer::empty(Rect::new(0, 0, 40, 20));
        panel.render(Rect::new(0, 0, 40, 20), &mut buffer);

        // If we get here without panic, test passes
    }
}
