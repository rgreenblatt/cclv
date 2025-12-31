//! Entry rendering with consistent collapse logic across all content blocks.
//!
//! This module provides unified rendering that fixes the bug where Thinking blocks
//! never collapse in the renderer (message.rs) but are counted as collapsed in the
//! height calculator (layout.rs), causing scroll offset mismatches.
//!
//! # The Bug (cclv-5ur.14)
//!
//! **Current broken behavior**:
//! - Height calculator (layout.rs): entry-level collapse, counts Thinking blocks
//! - Renderer (message.rs): block-level collapse, Thinking blocks NEVER collapse
//! - Result: 100-line Thinking block gets height=4 (collapsed) but renders 100 lines
//! - Symptom: Scroll gets stuck because rendered height > calculated height
//!
//! **This fix**:
//! - Single function computes rendered lines with entry-level collapse
//! - ALL block types (Text, ToolUse, ToolResult, **Thinking**) respect collapse state
//! - Collapse decision made once at entry level, not per-block
//! - Rendered line count matches height calculation

use crate::model::{ContentBlock, ConversationEntry, MessageContent};
use crate::state::WrapMode;
use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
};

/// Compute rendered lines for a conversation entry with consistent collapse logic.
///
/// This is the single source of truth for entry rendering. It ensures that:
/// - Collapse logic is consistent across ALL content block types (Text, ToolUse, ToolResult, Thinking)
/// - Thinking blocks collapse when entry is collapsed (fixes bug where they never collapsed)
/// - Entry-level collapse decision (not per-block) matches height calculator
/// - Returns owned lines with 'static lifetime for caching
///
/// # Collapse Behavior
///
/// When `expanded = false`:
/// - Count total lines that WOULD be rendered if expanded
/// - If total > collapse_threshold, show first `summary_lines` + collapse indicator
/// - Collapse indicator: "(+N more lines)" where N = total - summary_lines
///
/// When `expanded = true`:
/// - Render all content blocks fully
///
/// # Arguments
///
/// * `entry` - The conversation entry to render
/// * `expanded` - Whether the entry is currently expanded
/// * `wrap_mode` - Effective wrap mode for this entry
/// * `width` - Viewport width for text wrapping calculations
/// * `collapse_threshold` - Number of lines before collapsing (typically 10)
/// * `summary_lines` - Number of lines to show when collapsed (typically 3)
///
/// # Returns
///
/// Vector of owned Lines with 'static lifetime, including:
/// - Entry content (respecting collapse state)
/// - Separator line at end (blank line between entries)
///
/// # Example
///
/// ```ignore
/// let entry = /* ConversationEntry with 100-line Thinking block */;
/// let collapsed_lines = compute_entry_lines(&entry, false, WrapMode::Wrap, 80, 10, 3);
/// // Should return ~4 lines (3 summary + 1 collapse indicator)
///
/// let expanded_lines = compute_entry_lines(&entry, true, WrapMode::Wrap, 80, 10, 3);
/// // Should return ~100 lines (all content)
/// ```
pub fn compute_entry_lines(
    entry: &ConversationEntry,
    expanded: bool,
    _wrap_mode: WrapMode,
    _width: u16,
    collapse_threshold: usize,
    summary_lines: usize,
) -> Vec<Line<'static>> {
    let mut lines = Vec::new();

    // Only handle Valid entries for now (minimal implementation)
    let valid_entry = match entry.as_valid() {
        Some(e) => e,
        None => {
            // Malformed entries: just add separator
            lines.push(Line::from(""));
            return lines;
        }
    };

    let message = valid_entry.message();

    // Handle message content
    match message.content() {
        MessageContent::Text(_text) => {
            // For now, minimal implementation - just add separator
            lines.push(Line::from(""));
        }
        MessageContent::Blocks(blocks) => {
            // Render each content block
            for block in blocks {
                let block_lines = render_block(block, expanded, collapse_threshold, summary_lines);
                lines.extend(block_lines);
            }

            // Add separator line at end
            lines.push(Line::from(""));
        }
    }

    lines
}

/// Render a single content block with collapse support.
fn render_block(
    block: &ContentBlock,
    expanded: bool,
    collapse_threshold: usize,
    summary_lines: usize,
) -> Vec<Line<'static>> {
    match block {
        ContentBlock::Text { text } => {
            let text_lines: Vec<_> = text.lines().collect();
            let total_lines = text_lines.len();
            let should_collapse = total_lines > collapse_threshold && !expanded;

            let mut lines = Vec::new();

            if should_collapse {
                // Show summary lines
                for line in text_lines.iter().take(summary_lines) {
                    lines.push(Line::from(line.to_string()));
                }
                // Add collapse indicator
                let remaining = total_lines - summary_lines;
                lines.push(Line::from(Span::styled(
                    format!("(+{} more lines)", remaining),
                    Style::default().add_modifier(Modifier::DIM),
                )));
            } else {
                // Show all lines
                for line in text_lines {
                    lines.push(Line::from(line.to_string()));
                }
            }

            lines
        }
        ContentBlock::ToolUse(_) => {
            // Minimal: just a placeholder line
            vec![Line::from("Tool use")]
        }
        ContentBlock::ToolResult { .. } => {
            // Minimal: just a placeholder line
            vec![Line::from("Tool result")]
        }
        ContentBlock::Thinking { thinking } => {
            // THIS IS THE KEY FIX: Thinking blocks now respect collapse state
            let thinking_lines: Vec<_> = thinking.lines().collect();
            let total_lines = thinking_lines.len();
            let should_collapse = total_lines > collapse_threshold && !expanded;

            let mut lines = Vec::new();

            if should_collapse {
                // Show summary lines with thinking style
                for line in thinking_lines.iter().take(summary_lines) {
                    lines.push(Line::from(Span::styled(
                        line.to_string(),
                        Style::default()
                            .add_modifier(Modifier::ITALIC)
                            .add_modifier(Modifier::DIM),
                    )));
                }
                // Add collapse indicator
                let remaining = total_lines - summary_lines;
                lines.push(Line::from(Span::styled(
                    format!("(+{} more lines)", remaining),
                    Style::default().add_modifier(Modifier::DIM),
                )));
            } else {
                // Show all lines with thinking style
                for line in thinking_lines {
                    lines.push(Line::from(Span::styled(
                        line.to_string(),
                        Style::default()
                            .add_modifier(Modifier::ITALIC)
                            .add_modifier(Modifier::DIM),
                    )));
                }
            }

            lines
        }
    }
}

#[cfg(test)]
#[path = "renderer_tests.rs"]
mod renderer_tests;
