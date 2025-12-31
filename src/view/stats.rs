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

        // Get filtered usage based on current filter
        let usage = self.stats.filtered_usage(self.filter);

        // Token section
        lines.push(
            Line::from("Tokens:").style(
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
        );
        lines.push(Line::from(format!(
            "  Input:  {}",
            format_tokens(usage.total_input())
        )));
        lines.push(Line::from(format!(
            "  Output: {}",
            format_tokens(usage.output_tokens)
        )));
        lines.push(Line::from(format!(
            "  Total:  {}",
            format_tokens(usage.total())
        )));

        // Cache tokens (only if non-zero)
        let total_cache = usage.cache_creation_input_tokens + usage.cache_read_input_tokens;
        if total_cache > 0 {
            lines.push(Line::from(format!(
                "  Cache:  {}",
                format_tokens(total_cache)
            )));
        }

        lines.push(Line::from(""));

        // Cost section
        let cost = self.stats.estimated_cost(self.pricing, self.model_id);
        lines.push(
            Line::from("Estimated Cost:").style(
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
        );
        lines.push(Line::from(format!("  {}", format_cost(cost))));
        lines.push(Line::from(""));

        // Tool usage section
        if !self.stats.tool_counts.is_empty() {
            lines.push(
                Line::from("Tool Usage:").style(
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
            );
            let tool_lines = format_tool_breakdown(&self.stats.tool_counts, 10);
            lines.extend(tool_lines);
            lines.push(Line::from(""));
        }

        // Subagents section
        lines.push(
            Line::from("Subagents:").style(
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
        );
        lines.push(Line::from(format!(
            "  Count: {}",
            self.stats.subagent_count
        )));

        // Render the paragraph
        let paragraph = Paragraph::new(lines);
        paragraph.render(inner, buf);
    }
}

// ===== Formatting Helpers =====

/// Format tool usage breakdown with top N limiting.
///
/// Returns lines displaying tool names with counts, sorted by count descending.
/// If tools exceed max_display, shows "... and X more" line.
///
/// # Arguments
/// * `tool_counts` - Map of tool names to invocation counts
/// * `max_display` - Maximum number of tools to display before truncating
///
/// # Examples
/// ```
/// // With 3 tools, max 10: shows all 3
/// // With 12 tools, max 10: shows top 10 + "... and 2 more"
/// ```
fn format_tool_breakdown(
    tool_counts: &std::collections::HashMap<crate::model::ToolName, u32>,
    max_display: usize,
) -> Vec<Line<'static>> {
    // Return empty if no tools
    if tool_counts.is_empty() {
        return vec![];
    }

    // Sort by count descending
    let mut tools: Vec<_> = tool_counts.iter().collect();
    tools.sort_by(|a, b| b.1.cmp(a.1));

    let mut lines = Vec::new();
    let total_tools = tools.len();

    // Take top N tools
    for (tool_name, count) in tools.iter().take(max_display) {
        lines.push(Line::from(format!("  {}: {}", tool_name.as_str(), count)));
    }

    // Add overflow indicator if needed
    if total_tools > max_display {
        let remaining = total_tools - max_display;
        lines.push(Line::from(format!("  ... and {} more", remaining)));
    }

    lines
}

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
            content.contains("350"), // 200 + 150 = 350 total cache tokens
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

    #[test]
    fn stats_panel_displays_cost_for_opus_model() {
        use crate::model::TokenUsage;
        use ratatui::buffer::Buffer;
        use ratatui::layout::Rect;
        use std::collections::HashMap;

        // Setup: 1M input tokens, 1M output tokens using Opus pricing
        // Expected: $15 (input) + $75 (output) = $90.00
        let stats = SessionStats {
            total_usage: TokenUsage {
                input_tokens: 1_000_000,
                output_tokens: 1_000_000,
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

        let content = buffer_to_string(&buffer);

        // Verify "Estimated Cost:" label is present
        assert!(
            content.contains("Estimated Cost:"),
            "Expected 'Estimated Cost:' label in output, got:\n{}",
            content
        );

        // Verify the actual cost is displayed correctly
        assert!(
            content.contains("$90.00"),
            "Expected cost '$90.00' for Opus model (1M input + 1M output), got:\n{}",
            content
        );
    }

    #[test]
    fn stats_panel_displays_cost_for_sonnet_model() {
        use crate::model::TokenUsage;
        use ratatui::buffer::Buffer;
        use ratatui::layout::Rect;
        use std::collections::HashMap;

        // Setup: 2M input tokens, 1M output tokens using Sonnet pricing
        // Expected: $6 (2M * $3) + $15 (1M * $15) = $21.00
        let stats = SessionStats {
            total_usage: TokenUsage {
                input_tokens: 2_000_000,
                output_tokens: 1_000_000,
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
        let panel = StatsPanel::new(&stats, &filter, &pricing, Some("sonnet"));

        let mut buffer = Buffer::empty(Rect::new(0, 0, 50, 25));
        panel.render(Rect::new(0, 0, 50, 25), &mut buffer);

        let content = buffer_to_string(&buffer);

        assert!(
            content.contains("$21.00"),
            "Expected cost '$21.00' for Sonnet model (2M input + 1M output), got:\n{}",
            content
        );
    }

    #[test]
    fn stats_panel_displays_cost_for_haiku_model() {
        use crate::model::TokenUsage;
        use ratatui::buffer::Buffer;
        use ratatui::layout::Rect;
        use std::collections::HashMap;

        // Setup: 5M input tokens, 2M output tokens using Haiku pricing
        // Expected: $4 (5M * $0.8) + $8 (2M * $4) = $12.00
        let stats = SessionStats {
            total_usage: TokenUsage {
                input_tokens: 5_000_000,
                output_tokens: 2_000_000,
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
        let panel = StatsPanel::new(&stats, &filter, &pricing, Some("haiku"));

        let mut buffer = Buffer::empty(Rect::new(0, 0, 50, 25));
        panel.render(Rect::new(0, 0, 50, 25), &mut buffer);

        let content = buffer_to_string(&buffer);

        assert!(
            content.contains("$12.00"),
            "Expected cost '$12.00' for Haiku model (5M input + 2M output), got:\n{}",
            content
        );
    }

    #[test]
    fn stats_panel_displays_cost_with_cached_tokens() {
        use crate::model::TokenUsage;
        use ratatui::buffer::Buffer;
        use ratatui::layout::Rect;
        use std::collections::HashMap;

        // Setup: 1M input, 1M output, 1M cache using Opus pricing
        // Expected: $15 (input) + $75 (output) + $1.50 (cache) = $91.50
        let stats = SessionStats {
            total_usage: TokenUsage {
                input_tokens: 1_000_000,
                output_tokens: 1_000_000,
                cache_creation_input_tokens: 500_000,
                cache_read_input_tokens: 500_000,
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

        let content = buffer_to_string(&buffer);

        assert!(
            content.contains("$91.50"),
            "Expected cost '$91.50' for Opus with cache (1M input + 1M output + 1M cache), got:\n{}",
            content
        );
    }

    #[test]
    fn stats_panel_matches_model_family_from_full_model_id() {
        use crate::model::TokenUsage;
        use ratatui::buffer::Buffer;
        use ratatui::layout::Rect;
        use std::collections::HashMap;

        // Setup: Use full model ID "claude-sonnet-4-5-20250929"
        // Should match "sonnet" family and use $3/$15 pricing
        let stats = SessionStats {
            total_usage: TokenUsage {
                input_tokens: 1_000_000,
                output_tokens: 1_000_000,
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
        let panel = StatsPanel::new(
            &stats,
            &filter,
            &pricing,
            Some("claude-sonnet-4-5-20250929"),
        );

        let mut buffer = Buffer::empty(Rect::new(0, 0, 50, 25));
        panel.render(Rect::new(0, 0, 50, 25), &mut buffer);

        let content = buffer_to_string(&buffer);

        // Should use Sonnet pricing: $3 + $15 = $18
        assert!(
            content.contains("$18.00"),
            "Expected cost '$18.00' for full Sonnet model ID, got:\n{}",
            content
        );
    }

    #[test]
    fn stats_panel_displays_fractional_cost_correctly() {
        use crate::model::TokenUsage;
        use ratatui::buffer::Buffer;
        use ratatui::layout::Rect;
        use std::collections::HashMap;

        // Setup: 100k input tokens, 50k output tokens using Opus pricing
        // Expected: $1.50 (0.1M * $15) + $3.75 (0.05M * $75) = $5.25
        let stats = SessionStats {
            total_usage: TokenUsage {
                input_tokens: 100_000,
                output_tokens: 50_000,
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

        let content = buffer_to_string(&buffer);

        assert!(
            content.contains("$5.25"),
            "Expected fractional cost '$5.25', got:\n{}",
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

    // ===== format_tool_breakdown tests =====

    #[test]
    fn format_tool_breakdown_sorts_by_count_descending() {
        use crate::model::ToolName;
        use std::collections::HashMap;

        let mut tool_counts = HashMap::new();
        tool_counts.insert(ToolName::Read, 5);
        tool_counts.insert(ToolName::Write, 15);
        tool_counts.insert(ToolName::Bash, 10);

        let lines = format_tool_breakdown(&tool_counts, 10);

        // Extract just the text content from lines
        let text: Vec<String> = lines
            .iter()
            .map(|l| {
                l.spans
                    .iter()
                    .map(|s| s.content.as_ref())
                    .collect::<String>()
            })
            .collect();

        // Should be sorted: Write (15), Bash (10), Read (5)
        assert_eq!(text.len(), 3);
        assert!(text[0].contains("Write"));
        assert!(text[0].contains("15"));
        assert!(text[1].contains("Bash"));
        assert!(text[1].contains("10"));
        assert!(text[2].contains("Read"));
        assert!(text[2].contains("5"));
    }

    #[test]
    fn format_tool_breakdown_shows_overflow_indicator_when_exceeds_limit() {
        use crate::model::ToolName;
        use std::collections::HashMap;

        let mut tool_counts = HashMap::new();
        // Create 12 tools with different counts
        tool_counts.insert(ToolName::Read, 100);
        tool_counts.insert(ToolName::Write, 90);
        tool_counts.insert(ToolName::Bash, 80);
        tool_counts.insert(ToolName::Edit, 70);
        tool_counts.insert(ToolName::Grep, 60);
        tool_counts.insert(ToolName::Glob, 50);
        tool_counts.insert(ToolName::Task, 40);
        tool_counts.insert(ToolName::WebSearch, 30);
        tool_counts.insert(ToolName::WebFetch, 20);
        tool_counts.insert(ToolName::MultiEdit, 10);
        tool_counts.insert(ToolName::Other("Custom1".to_string()), 5);
        tool_counts.insert(ToolName::Other("Custom2".to_string()), 1);

        let lines = format_tool_breakdown(&tool_counts, 10);

        // Should show top 10 + overflow line = 11 lines total
        assert_eq!(lines.len(), 11);

        // Last line should be overflow indicator
        let last_line_text: String = lines[10].spans.iter().map(|s| s.content.as_ref()).collect();

        assert!(
            last_line_text.contains("... and 2 more"),
            "Expected overflow indicator '... and 2 more', got: {}",
            last_line_text
        );
    }

    #[test]
    fn format_tool_breakdown_shows_all_tools_when_within_limit() {
        use crate::model::ToolName;
        use std::collections::HashMap;

        let mut tool_counts = HashMap::new();
        tool_counts.insert(ToolName::Read, 10);
        tool_counts.insert(ToolName::Write, 5);
        tool_counts.insert(ToolName::Bash, 3);

        let lines = format_tool_breakdown(&tool_counts, 10);

        // Should show all 3, no overflow
        assert_eq!(lines.len(), 3);

        // No line should contain "... and"
        for line in &lines {
            let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
            assert!(
                !text.contains("... and"),
                "Should not show overflow when within limit"
            );
        }
    }

    #[test]
    fn format_tool_breakdown_handles_empty_tool_counts() {
        use std::collections::HashMap;

        let tool_counts = HashMap::new();
        let lines = format_tool_breakdown(&tool_counts, 10);

        // Should return empty vec
        assert_eq!(lines.len(), 0);
    }

    #[test]
    fn format_tool_breakdown_formats_each_line_correctly() {
        use crate::model::ToolName;
        use std::collections::HashMap;

        let mut tool_counts = HashMap::new();
        tool_counts.insert(ToolName::Read, 42);

        let lines = format_tool_breakdown(&tool_counts, 10);

        assert_eq!(lines.len(), 1);

        let text: String = lines[0].spans.iter().map(|s| s.content.as_ref()).collect();

        // Should be formatted as "  Read: 42"
        assert!(text.contains("Read"));
        assert!(text.contains("42"));
        assert!(text.starts_with("  "), "Should have 2-space indent");
    }

    #[test]
    fn format_tool_breakdown_uses_tool_name_as_str() {
        use crate::model::ToolName;
        use std::collections::HashMap;

        let mut tool_counts = HashMap::new();
        tool_counts.insert(ToolName::Other("CustomTool".to_string()), 7);

        let lines = format_tool_breakdown(&tool_counts, 10);

        let text: String = lines[0].spans.iter().map(|s| s.content.as_ref()).collect();

        // Should use ToolName::as_str() which returns "CustomTool"
        assert!(text.contains("CustomTool"));
        assert!(text.contains("7"));
    }

    // ===== StatsPanel filtered display tests =====

    #[test]
    fn stats_panel_displays_global_filter_total_tokens() {
        use crate::model::{AgentId, TokenUsage};
        use ratatui::buffer::Buffer;
        use ratatui::layout::Rect;
        use std::collections::HashMap;

        let mut subagent_usage = HashMap::new();
        subagent_usage.insert(
            AgentId::new("agent-1").unwrap(),
            TokenUsage {
                input_tokens: 400,
                output_tokens: 200,
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
                input_tokens: 600,
                output_tokens: 300,
                cache_creation_input_tokens: 0,
                cache_read_input_tokens: 0,
            },
            subagent_usage,
            tool_counts: HashMap::new(),
            subagent_count: 1,
            entry_count: 10,
        };

        let filter = StatsFilter::Global;
        let pricing = PricingConfig::default();
        let panel = StatsPanel::new(&stats, &filter, &pricing, Some("opus"));

        let mut buffer = Buffer::empty(Rect::new(0, 0, 50, 25));
        panel.render(Rect::new(0, 0, 50, 25), &mut buffer);

        let content = buffer_to_string(&buffer);

        // Should display total usage (1000 + 500 = 1500)
        assert!(
            content.contains("1,000"),
            "Expected input tokens '1,000' for Global filter, got:\n{}",
            content
        );
        assert!(
            content.contains("500"),
            "Expected output tokens '500' for Global filter, got:\n{}",
            content
        );
        assert!(
            content.contains("1,500"),
            "Expected total tokens '1,500' for Global filter, got:\n{}",
            content
        );
    }

    #[test]
    fn stats_panel_displays_main_agent_filter_main_tokens() {
        use crate::model::{AgentId, TokenUsage};
        use ratatui::buffer::Buffer;
        use ratatui::layout::Rect;
        use std::collections::HashMap;

        let mut subagent_usage = HashMap::new();
        subagent_usage.insert(
            AgentId::new("agent-1").unwrap(),
            TokenUsage {
                input_tokens: 400,
                output_tokens: 200,
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
                input_tokens: 600,
                output_tokens: 300,
                cache_creation_input_tokens: 0,
                cache_read_input_tokens: 0,
            },
            subagent_usage,
            tool_counts: HashMap::new(),
            subagent_count: 1,
            entry_count: 10,
        };

        let filter = StatsFilter::MainAgent;
        let pricing = PricingConfig::default();
        let panel = StatsPanel::new(&stats, &filter, &pricing, Some("opus"));

        let mut buffer = Buffer::empty(Rect::new(0, 0, 50, 25));
        panel.render(Rect::new(0, 0, 50, 25), &mut buffer);

        let content = buffer_to_string(&buffer);

        // Should display main agent usage only (600 + 300 = 900)
        assert!(
            content.contains("600"),
            "Expected input tokens '600' for MainAgent filter, got:\n{}",
            content
        );
        assert!(
            content.contains("300"),
            "Expected output tokens '300' for MainAgent filter, got:\n{}",
            content
        );
        assert!(
            content.contains("900"),
            "Expected total tokens '900' for MainAgent filter, got:\n{}",
            content
        );
    }

    #[test]
    fn stats_panel_displays_subagent_filter_subagent_tokens() {
        use crate::model::{AgentId, TokenUsage};
        use ratatui::buffer::Buffer;
        use ratatui::layout::Rect;
        use std::collections::HashMap;

        let agent1 = AgentId::new("agent-1").unwrap();
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
                input_tokens: 1000,
                output_tokens: 500,
                cache_creation_input_tokens: 0,
                cache_read_input_tokens: 0,
            },
            main_agent_usage: TokenUsage {
                input_tokens: 600,
                output_tokens: 300,
                cache_creation_input_tokens: 0,
                cache_read_input_tokens: 0,
            },
            subagent_usage,
            tool_counts: HashMap::new(),
            subagent_count: 1,
            entry_count: 10,
        };

        let filter = StatsFilter::Subagent(agent1);
        let pricing = PricingConfig::default();
        let panel = StatsPanel::new(&stats, &filter, &pricing, Some("opus"));

        let mut buffer = Buffer::empty(Rect::new(0, 0, 50, 25));
        panel.render(Rect::new(0, 0, 50, 25), &mut buffer);

        let content = buffer_to_string(&buffer);

        // Should display subagent usage only (400 + 200 = 600)
        assert!(
            content.contains("400"),
            "Expected input tokens '400' for Subagent filter, got:\n{}",
            content
        );
        assert!(
            content.contains("200"),
            "Expected output tokens '200' for Subagent filter, got:\n{}",
            content
        );
        assert!(
            content.contains("600"),
            "Expected total tokens '600' for Subagent filter, got:\n{}",
            content
        );
    }

    // ===== Filter indicator tests =====

    #[test]
    fn stats_panel_shows_global_filter_in_title() {
        use ratatui::buffer::Buffer;
        use ratatui::layout::Rect;

        let stats = SessionStats::default();
        let filter = StatsFilter::Global;
        let pricing = PricingConfig::default();
        let panel = StatsPanel::new(&stats, &filter, &pricing, Some("opus"));

        let mut buffer = Buffer::empty(Rect::new(0, 0, 50, 25));
        panel.render(Rect::new(0, 0, 50, 25), &mut buffer);

        let content = buffer_to_string(&buffer);

        // Title should indicate Global filter
        assert!(
            content.contains("Statistics"),
            "Expected 'Statistics' title for Global filter, got:\n{}",
            content
        );
        // Global has no suffix in title (just "Statistics")
        assert!(
            !content.contains("(Main Agent)") && !content.contains("(Subagent)"),
            "Global filter should not have agent suffix in title, got:\n{}",
            content
        );
    }

    #[test]
    fn stats_panel_shows_main_agent_filter_in_title() {
        use ratatui::buffer::Buffer;
        use ratatui::layout::Rect;

        let stats = SessionStats::default();
        let filter = StatsFilter::MainAgent;
        let pricing = PricingConfig::default();
        let panel = StatsPanel::new(&stats, &filter, &pricing, Some("opus"));

        let mut buffer = Buffer::empty(Rect::new(0, 0, 50, 25));
        panel.render(Rect::new(0, 0, 50, 25), &mut buffer);

        let content = buffer_to_string(&buffer);

        // Title should indicate Main Agent filter
        assert!(
            content.contains("(Main Agent)"),
            "Expected '(Main Agent)' in title for MainAgent filter, got:\n{}",
            content
        );
    }

    #[test]
    fn stats_panel_shows_subagent_filter_in_title() {
        use crate::model::AgentId;
        use ratatui::buffer::Buffer;
        use ratatui::layout::Rect;

        let stats = SessionStats::default();
        let agent_id = AgentId::new("agent-123").unwrap();
        let filter = StatsFilter::Subagent(agent_id);
        let pricing = PricingConfig::default();
        let panel = StatsPanel::new(&stats, &filter, &pricing, Some("opus"));

        let mut buffer = Buffer::empty(Rect::new(0, 0, 50, 25));
        panel.render(Rect::new(0, 0, 50, 25), &mut buffer);

        let content = buffer_to_string(&buffer);

        // Title should indicate Subagent filter
        assert!(
            content.contains("(Subagent)"),
            "Expected '(Subagent)' in title for Subagent filter, got:\n{}",
            content
        );
    }

    // ===== Cost filtering tests =====

    #[test]
    fn stats_panel_displays_filtered_cost_for_main_agent() {
        use crate::model::{AgentId, TokenUsage};
        use ratatui::buffer::Buffer;
        use ratatui::layout::Rect;
        use std::collections::HashMap;

        // Setup: Main agent has 1M input/output, subagent has 1M input/output
        // When filtering to MainAgent, should only show cost for main agent
        let mut subagent_usage = HashMap::new();
        subagent_usage.insert(
            AgentId::new("agent-1").unwrap(),
            TokenUsage {
                input_tokens: 1_000_000,
                output_tokens: 1_000_000,
                cache_creation_input_tokens: 0,
                cache_read_input_tokens: 0,
            },
        );

        let stats = SessionStats {
            total_usage: TokenUsage {
                input_tokens: 2_000_000, // Main + subagent
                output_tokens: 2_000_000,
                cache_creation_input_tokens: 0,
                cache_read_input_tokens: 0,
            },
            main_agent_usage: TokenUsage {
                input_tokens: 1_000_000,
                output_tokens: 1_000_000,
                cache_creation_input_tokens: 0,
                cache_read_input_tokens: 0,
            },
            subagent_usage,
            tool_counts: HashMap::new(),
            subagent_count: 1,
            entry_count: 10,
        };

        let filter = StatsFilter::MainAgent;
        let pricing = PricingConfig::default();
        let panel = StatsPanel::new(&stats, &filter, &pricing, Some("opus"));

        let mut buffer = Buffer::empty(Rect::new(0, 0, 50, 25));
        panel.render(Rect::new(0, 0, 50, 25), &mut buffer);

        let content = buffer_to_string(&buffer);

        // Should show cost for main agent only: $15 + $75 = $90.00
        // NOT total cost of $180.00
        assert!(
            content.contains("$90.00"),
            "Expected filtered cost '$90.00' for MainAgent (1M input + 1M output), got:\n{}",
            content
        );
        assert!(
            !content.contains("$180.00"),
            "Should NOT show total cost '$180.00' when filtering to MainAgent, got:\n{}",
            content
        );
    }

    #[test]
    fn stats_panel_displays_filtered_cost_for_subagent() {
        use crate::model::{AgentId, TokenUsage};
        use ratatui::buffer::Buffer;
        use ratatui::layout::Rect;
        use std::collections::HashMap;

        let agent1 = AgentId::new("agent-1").unwrap();

        // Setup: Main agent has 1M input/output, subagent has 500k input/output
        // When filtering to Subagent, should only show cost for that subagent
        let mut subagent_usage = HashMap::new();
        subagent_usage.insert(
            agent1.clone(),
            TokenUsage {
                input_tokens: 500_000,
                output_tokens: 500_000,
                cache_creation_input_tokens: 0,
                cache_read_input_tokens: 0,
            },
        );

        let stats = SessionStats {
            total_usage: TokenUsage {
                input_tokens: 1_500_000, // Main + subagent
                output_tokens: 1_500_000,
                cache_creation_input_tokens: 0,
                cache_read_input_tokens: 0,
            },
            main_agent_usage: TokenUsage {
                input_tokens: 1_000_000,
                output_tokens: 1_000_000,
                cache_creation_input_tokens: 0,
                cache_read_input_tokens: 0,
            },
            subagent_usage,
            tool_counts: HashMap::new(),
            subagent_count: 1,
            entry_count: 10,
        };

        let filter = StatsFilter::Subagent(agent1);
        let pricing = PricingConfig::default();
        let panel = StatsPanel::new(&stats, &filter, &pricing, Some("opus"));

        let mut buffer = Buffer::empty(Rect::new(0, 0, 50, 25));
        panel.render(Rect::new(0, 0, 50, 25), &mut buffer);

        let content = buffer_to_string(&buffer);

        // Should show cost for subagent only: $7.50 + $37.50 = $45.00
        // NOT total cost of $135.00 or main agent cost of $90.00
        assert!(
            content.contains("$45.00"),
            "Expected filtered cost '$45.00' for Subagent (500k input + 500k output), got:\n{}",
            content
        );
        assert!(
            !content.contains("$135.00"),
            "Should NOT show total cost '$135.00' when filtering to Subagent, got:\n{}",
            content
        );
        assert!(
            !content.contains("$90.00"),
            "Should NOT show main agent cost '$90.00' when filtering to Subagent, got:\n{}",
            content
        );
    }
}
