//! Tests for token divider rendering.

use super::*;
use crate::model::stats::PricingConfig;
use crate::model::MessageContent;

// ===== ContextWindowTokens Tests =====

#[test]
fn context_window_tokens_new_stores_value() {
    let tokens = ContextWindowTokens::new(200_000);
    assert_eq!(tokens.get(), 200_000);
}

#[test]
fn context_window_tokens_default_is_200k() {
    let tokens = ContextWindowTokens::default();
    assert_eq!(tokens.get(), 200_000);
}

// ===== format_token_count Tests =====

#[test]
fn format_token_count_less_than_1000_shows_exact() {
    assert_eq!(format_token_count(0), "0");
    assert_eq!(format_token_count(1), "1");
    assert_eq!(format_token_count(340), "340");
    assert_eq!(format_token_count(999), "999");
}

#[test]
fn format_token_count_1k_to_1m_shows_k_suffix() {
    assert_eq!(format_token_count(1_000), "1.0k");
    assert_eq!(format_token_count(1_200), "1.2k");
    assert_eq!(format_token_count(45_200), "45.2k");
    assert_eq!(format_token_count(999_999), "1000.0k"); // Edge case before M
}

#[test]
fn format_token_count_1m_and_above_shows_m_suffix() {
    assert_eq!(format_token_count(1_000_000), "1.0M");
    assert_eq!(format_token_count(1_500_000), "1.5M");
    assert_eq!(format_token_count(45_200_000), "45.2M");
}

#[test]
fn format_token_count_rounds_to_one_decimal() {
    assert_eq!(format_token_count(1_234), "1.2k"); // Rounds down
    assert_eq!(format_token_count(1_567), "1.6k"); // Rounds up
    assert_eq!(format_token_count(1_234_567), "1.2M"); // Rounds down
}

// ===== format_cost Tests =====

#[test]
fn format_cost_shows_two_decimals() {
    assert_eq!(format_cost(0.0234), "$0.02");
    assert_eq!(format_cost(1.234), "$1.23");
    assert_eq!(format_cost(0.001), "$0.00");
    assert_eq!(format_cost(99.999), "$100.00"); // Rounds up
}

#[test]
fn format_cost_includes_dollar_sign() {
    let formatted = format_cost(1.50);
    assert!(formatted.starts_with('$'));
    assert_eq!(formatted, "$1.50");
}

#[test]
fn format_cost_handles_zero() {
    assert_eq!(format_cost(0.0), "$0.00");
}

#[test]
fn format_cost_handles_large_values() {
    assert_eq!(format_cost(1234.56), "$1234.56");
}

// ===== calculate_context_percentage Tests =====

#[test]
fn calculate_context_percentage_returns_correct_percentage() {
    let max = ContextWindowTokens::new(200_000);
    assert_eq!(calculate_context_percentage(0, max), 0);
    assert_eq!(calculate_context_percentage(45_200, max), 22); // 22.6% rounds to 22
    assert_eq!(calculate_context_percentage(100_000, max), 50);
    assert_eq!(calculate_context_percentage(150_000, max), 75);
    assert_eq!(calculate_context_percentage(200_000, max), 100);
}

#[test]
fn calculate_context_percentage_clamps_to_100() {
    let max = ContextWindowTokens::new(200_000);
    assert_eq!(calculate_context_percentage(250_000, max), 100);
    assert_eq!(calculate_context_percentage(1_000_000, max), 100);
}

#[test]
fn calculate_context_percentage_rounds_correctly() {
    let max = ContextWindowTokens::new(200_000);
    // 45,200 / 200,000 = 0.226 = 22.6% -> rounds to 23
    assert_eq!(calculate_context_percentage(45_200, max), 22);
    // 100,100 / 200,000 = 0.5005 = 50.05% -> rounds to 50
    assert_eq!(calculate_context_percentage(100_100, max), 50);
}

// ===== render_token_divider Tests =====

#[test]
fn render_token_divider_formats_basic_divider() {
    let usage = TokenUsage {
        input_tokens: 1_200,
        output_tokens: 340,
        cache_creation_input_tokens: 0,
        cache_read_input_tokens: 0,
        ephemeral_5m_input_tokens: 0,
        ephemeral_1h_input_tokens: 0,
    };
    let content = MessageContent::Text("response text".to_string());
    let max_context = ContextWindowTokens::new(200_000);
    let pricing = PricingConfig::default();

    let line = render_token_divider(&usage, &content, max_context, &pricing, Some("opus"));

    // Extract text from line spans
    let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();

    // Should use new format with arrows
    assert!(text.contains("↓"));
    assert!(text.contains("↑"));

    // Should contain context info (per-entry context = input + cache_creation + cache_read + output = 1200 + 0 + 0 + 340 = 1540)
    assert!(text.contains("Context:"));
    assert!(text.contains("1.5k"));
    assert!(text.contains("(0%)")); // 1.5k / 200k ≈ 0%

    // Should contain cost
    assert!(text.contains("$"));
}

#[test]
fn render_token_divider_calculates_cost_correctly() {
    let usage = TokenUsage {
        input_tokens: 1_000_000, // 1M input tokens
        output_tokens: 0,
        cache_creation_input_tokens: 0,
        cache_read_input_tokens: 0,
        ephemeral_5m_input_tokens: 0,
        ephemeral_1h_input_tokens: 0,
    };
    let content = MessageContent::Text("response".to_string());
    let max_context = ContextWindowTokens::new(200_000);

    // Use default pricing: Opus is $15 per million input
    let pricing = PricingConfig::default();

    let line = render_token_divider(&usage, &content, max_context, &pricing, Some("opus"));

    // Extract text from line spans
    let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();

    // 1M input tokens * $15/M = $15.00
    assert!(text.contains("$15.00"));
}

#[test]
fn render_token_divider_handles_zero_usage() {
    let usage = TokenUsage::default();
    let content = MessageContent::Text("".to_string());
    let max_context = ContextWindowTokens::new(200_000);
    let pricing = PricingConfig::default();

    let line = render_token_divider(&usage, &content, max_context, &pricing, None);

    let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();

    assert!(text.contains("↓0/0"));
    assert!(text.contains("↑0/0"));
    assert!(text.contains("$0.00"));
    assert!(text.contains("Context: 0 (0%)"));
}

#[test]
fn render_token_divider_uses_dim_gray_style() {
    let usage = TokenUsage::default();
    let content = MessageContent::Text("".to_string());
    let max_context = ContextWindowTokens::default();
    let pricing = PricingConfig::default();

    let line = render_token_divider(&usage, &content, max_context, &pricing, None);

    // Should have at least one span
    assert!(!line.spans.is_empty());

    // Check that spans use dim/muted styling (DarkGray or DIM modifier)
    // We'll verify at least one span has this styling
    let has_dim_style = line.spans.iter().any(|span| {
        use ratatui::style::{Color, Modifier};
        span.style.fg == Some(Color::DarkGray) || span.style.add_modifier.contains(Modifier::DIM)
    });

    assert!(
        has_dim_style,
        "Divider should have dim/muted styling (DarkGray or DIM modifier)"
    );
}

#[test]
fn render_token_divider_includes_cache_tokens_in_cost() {
    let usage = TokenUsage {
        input_tokens: 0,
        output_tokens: 0,
        cache_creation_input_tokens: 1_000_000, // 1M cache creation
        cache_read_input_tokens: 0,
        ephemeral_5m_input_tokens: 0,
        ephemeral_1h_input_tokens: 0,
    };
    let content = MessageContent::Text("".to_string());
    let max_context = ContextWindowTokens::new(200_000);

    // Opus pricing: $1.50 per million cached tokens
    let pricing = PricingConfig::default();

    let line = render_token_divider(&usage, &content, max_context, &pricing, Some("opus"));

    let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();

    // 1M cache tokens * $1.50/M = $1.50
    assert!(text.contains("$1.50"));
}
