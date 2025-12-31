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
        // Create the block with title and borders
        let title = match self.filter {
            StatsFilter::Global => " Statistics ",
            StatsFilter::MainAgent => " Statistics (Main Agent) ",
            StatsFilter::Subagent(_) => " Statistics (Subagent) ",
        };

        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .style(Style::default().fg(Color::White));

        let inner = block.inner(area);
        block.render(area, buf);

        // Build content lines
        let mut lines = Vec::new();

        // Token section
        lines.push(Line::from("Tokens:").style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)));
        lines.push(Line::from(format!(
            "  Input:  {}",
            format_tokens(self.stats.total_usage.total_input())
        )));
        lines.push(Line::from(format!(
            "  Output: {}",
            format_tokens(self.stats.total_usage.output_tokens)
        )));
        lines.push(Line::from(format!(
            "  Total:  {}",
            format_tokens(self.stats.total_usage.total())
        )));
        lines.push(Line::from(""));

        // Cost section
        let cost = self.stats.estimated_cost(self.pricing, self.model_id);
        lines.push(Line::from("Estimated Cost:").style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)));
        lines.push(Line::from(format!("  {}", format_cost(cost))));
        lines.push(Line::from(""));

        // Tool usage section
        if !self.stats.tool_counts.is_empty() {
            lines.push(Line::from("Tool Usage:").style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)));
            let mut tool_vec: Vec<_> = self.stats.tool_counts.iter().collect();
            tool_vec.sort_by(|a, b| b.1.cmp(a.1)); // Sort by count descending
            for (tool_name, count) in tool_vec {
                lines.push(Line::from(format!("  {}: {}", tool_name.as_str(), count)));
            }
            lines.push(Line::from(""));
        }

        // Subagents section
        lines.push(Line::from("Subagents:").style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)));
        lines.push(Line::from(format!("  Count: {}", self.stats.subagent_count)));

        // Render the paragraph
        let paragraph = Paragraph::new(lines);
        paragraph.render(inner, buf);
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
    let s = tokens.to_string();
    let mut result = String::new();
    let chars: Vec<char> = s.chars().collect();

    for (i, c) in chars.iter().enumerate() {
        if i > 0 && (chars.len() - i) % 3 == 0 {
            result.push(',');
        }
        result.push(*c);
    }

    result
}

/// Format a cost value in USD.
///
/// Examples:
/// - `format_cost(0.0)` → "$0.00"
/// - `format_cost(2.45)` → "$2.45"
/// - `format_cost(123.456)` → "$123.46" (rounds to 2 decimals)
fn format_cost(cost: f64) -> String {
    // Round to 2 decimal places
    let rounded = (cost * 100.0).round() / 100.0;

    // Format the integer and fractional parts
    let dollars = rounded.floor() as u64;
    let cents = ((rounded - dollars as f64) * 100.0).round() as u64;

    // Format dollars with commas
    let dollars_str = format_tokens(dollars);

    format!("${}.{:02}", dollars_str, cents)
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

    #[test]
    fn stats_panel_displays_cache_tokens_when_nonzero() {
        use crate::model::TokenUsage;
        use ratatui::buffer::Buffer;
        use ratatui::layout::Rect;
        use std::collections::HashMap;

        let stats = SessionStats {
            total_usage: TokenUsage {
                input_tokens: 1000,
                output_tokens: 500,
                cache_creation_input_tokens: 200,
                cache_read_input_tokens: 150,
            },
            main_agent_usage: TokenUsage::default(),
            subagent_usage: HashMap::new(),
            tool_counts: HashMap::new(),
            subagent_count: 0,
            entry_count: 5,
        };

        let filter = StatsFilter::Global;
        let pricing = PricingConfig::default();
        let panel = StatsPanel::new(&stats, &filter, &pricing, Some("opus"));

        let mut buffer = Buffer::empty(Rect::new(0, 0, 50, 25));
        panel.render(Rect::new(0, 0, 50, 25), &mut buffer);

        // Extract rendered text from buffer
        let content = buffer_to_string(&buffer);

        // Verify cache tokens are displayed
        assert!(
            content.contains("Cache:"),
            "Expected 'Cache:' label in output, got:\n{}",
            content
        );
        assert!(
            content.contains("350"),  // 200 + 150 = 350 total cache tokens
            "Expected total cache tokens '350' in output, got:\n{}",
            content
        );
    }

    #[test]
    fn stats_panel_hides_cache_tokens_when_zero() {
        use crate::model::TokenUsage;
        use ratatui::buffer::Buffer;
        use ratatui::layout::Rect;
        use std::collections::HashMap;

        let stats = SessionStats {
            total_usage: TokenUsage {
                input_tokens: 1000,
                output_tokens: 500,
                cache_creation_input_tokens: 0,
                cache_read_input_tokens: 0,
            },
            main_agent_usage: TokenUsage::default(),
            subagent_usage: HashMap::new(),
            tool_counts: HashMap::new(),
            subagent_count: 0,
            entry_count: 5,
        };

        let filter = StatsFilter::Global;
        let pricing = PricingConfig::default();
        let panel = StatsPanel::new(&stats, &filter, &pricing, Some("opus"));

        let mut buffer = Buffer::empty(Rect::new(0, 0, 50, 25));
        panel.render(Rect::new(0, 0, 50, 25), &mut buffer);

        // Extract rendered text from buffer
        let content = buffer_to_string(&buffer);

        // Verify cache tokens are NOT displayed when zero
        assert!(
            !content.contains("Cache:"),
            "Expected NO 'Cache:' label when cache tokens are zero, got:\n{}",
            content
        );
    }

    /// Helper to extract text content from a ratatui Buffer
    fn buffer_to_string(buffer: &Buffer) -> String {
        let area = buffer.area();
        let mut lines = Vec::new();

        for y in area.top()..area.bottom() {
            let mut line = String::new();
            for x in area.left()..area.right() {
                let cell = &buffer[(x, y)];
                line.push_str(cell.symbol());
            }
            let trimmed = line.trim_end();
            if !trimmed.is_empty() {
                lines.push(trimmed.to_string());
            }
        }

        lines.join("\n")
    }
}
