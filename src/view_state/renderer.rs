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
use crate::view::MessageStyles;
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};
use tui_markdown::from_str;

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
/// * `wrap_mode` - Effective wrap mode for this entry (applied to Thinking blocks)
/// * `width` - Viewport width for text wrapping calculations (applied to Thinking blocks)
/// * `collapse_threshold` - Number of lines before collapsing (typically 10)
/// * `summary_lines` - Number of lines to show when collapsed (typically 3)
/// * `styles` - MessageStyles for role-based coloring (User=Cyan, Assistant=Green, etc.)
///
/// # Returns
///
/// Vector of owned Lines with 'static lifetime, including:
/// - Entry content (respecting collapse state)
/// - Separator line at end (blank line between entries)
///
/// # Note on Wrapping
///
/// The `wrap_mode` and `width` parameters control text wrapping behavior for all block types.
/// All content blocks (Text, ToolUse, ToolResult, Thinking) apply wrapping consistently to match
/// the height calculation in layout.rs, ensuring rendered line count equals calculated height.
///
/// # Example
///
/// ```ignore
/// let entry = /* ConversationEntry with 100-line Thinking block */;
/// let styles = MessageStyles::new();
/// let collapsed_lines = compute_entry_lines(&entry, false, WrapMode::Wrap, 80, 10, 3, &styles, Some(0));
/// // Should return ~4 lines (3 summary + 1 collapse indicator), each prefixed with "   1│"
///
/// let expanded_lines = compute_entry_lines(&entry, true, WrapMode::Wrap, 80, 10, 3, &styles, None);
/// // Should return ~100 lines (all content), without index prefixes
/// ```
#[allow(clippy::too_many_arguments)]
pub fn compute_entry_lines(
    entry: &ConversationEntry,
    expanded: bool,
    wrap_mode: WrapMode,
    width: u16,
    collapse_threshold: usize,
    summary_lines: usize,
    styles: &MessageStyles,
    entry_index: Option<usize>,
    is_subagent_view: bool,
    search_state: &crate::state::SearchState,
) -> Vec<Line<'static>> {
    // Extract match information if search is active
    let match_info = match search_state {
        crate::state::SearchState::Active {
            matches,
            current_match,
            ..
        } => Some((matches, *current_match)),
        _ => None,
    };

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

    // Get role-based style for this entry
    let role_style = styles.style_for_role(message.role());

    // Add "Initial Prompt" label for first message in subagent view (FR-XXX)
    // This label appears BEFORE the entry content and gets the entry index prefix
    if is_subagent_view && entry_index == Some(0) {
        lines.push(Line::from(vec![Span::styled(
            "🔷 Initial Prompt",
            Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
        )]));
    }

    // Handle message content
    match message.content() {
        MessageContent::Text(text) => {
            // Check if we have search matches for this entry
            let entry_matches: Vec<_> = match &match_info {
                Some((matches, current_idx)) => matches
                    .iter()
                    .enumerate()
                    .filter_map(|(idx, m)| {
                        if m.entry_uuid == *valid_entry.uuid() && m.block_index == 0 {
                            Some((m.char_offset, m.length, idx == *current_idx))
                        } else {
                            None
                        }
                    })
                    .collect(),
                None => vec![],
            };

            // Split into lines and wrap BEFORE markdown rendering
            // This ensures rendered line count matches height calculation
            let text_lines: Vec<_> = text.lines().collect();
            let wrapped_lines = wrap_lines(&text_lines, wrap_mode, width);

            // If we have search matches AND are expanded, apply highlighting
            // (Don't highlight collapsed view for simplicity)
            if !entry_matches.is_empty() && expanded {
                // Apply highlighting to wrapped text (skip markdown for now)
                // Track cumulative offset for multi-line text
                let mut cumulative_offset: usize = 0;
                for line_text in wrapped_lines {
                    let line_start = cumulative_offset;
                    let line_end = line_start.saturating_add(line_text.len());

                    // Filter matches that overlap this line
                    let line_matches: Vec<(usize, usize, bool)> = entry_matches
                        .iter()
                        .filter_map(|(offset, length, is_current)| {
                            let match_start = *offset;
                            let match_end = match_start.saturating_add(*length);

                            // Check if match overlaps this line
                            if match_start < line_end && match_end > line_start {
                                // Convert to line-relative offset
                                let line_relative_start = match_start.saturating_sub(line_start);
                                let line_relative_end =
                                    (match_end - line_start).min(line_text.len());
                                let line_relative_length =
                                    line_relative_end.saturating_sub(line_relative_start);

                                if line_relative_length > 0 {
                                    Some((line_relative_start, line_relative_length, *is_current))
                                } else {
                                    None
                                }
                            } else {
                                None
                            }
                        })
                        .collect();

                    // Render line with highlights
                    let highlighted_line =
                        apply_highlights_to_text(&line_text, &line_matches, role_style);
                    lines.push(highlighted_line);

                    // Update cumulative offset (add line length + newline char)
                    cumulative_offset = line_end.saturating_add(1);
                }
            } else {
                // No search matches or collapsed - render as markdown normally
                let wrapped_text = wrapped_lines.join("\n");
                let markdown_lines = render_markdown_with_style(&wrapped_text, role_style);

                // Apply collapse logic to markdown-rendered lines
                let total_lines = markdown_lines.len();
                let should_collapse = total_lines > collapse_threshold && !expanded;

                if should_collapse {
                    // Show summary lines (already markdown-rendered)
                    for line in markdown_lines.iter().take(summary_lines) {
                        lines.push(line.clone());
                    }
                    // Add collapse indicator
                    let remaining = total_lines - summary_lines;
                    lines.push(Line::from(Span::styled(
                        format!("(+{} more lines)", remaining),
                        Style::default().add_modifier(Modifier::DIM),
                    )));
                } else {
                    // Show all lines (already markdown-rendered)
                    lines.extend(markdown_lines);
                }
            }

            // Add separator line at end
            lines.push(Line::from(""));
        }
        MessageContent::Blocks(blocks) => {
            // Render each content block with role-based styling
            for block in blocks {
                let block_lines = render_block(
                    block,
                    expanded,
                    wrap_mode,
                    width,
                    collapse_threshold,
                    summary_lines,
                    role_style,
                    styles,
                );
                lines.extend(block_lines);
            }

            // Add separator line at end
            lines.push(Line::from(""));
        }
    }

    // Apply entry index prefix if requested
    if let Some(index) = entry_index {
        // Prepend index to all lines EXCEPT the separator (last line)
        let separator = lines.pop(); // Remove separator temporarily
        lines = lines
            .into_iter()
            .map(|line| prepend_index_to_line(line, index))
            .collect();
        if let Some(sep) = separator {
            lines.push(sep); // Re-add separator without prefix
        }
    }

    lines
}

/// Render markdown text with role-based styling applied to unstyled spans.
///
/// This function parses markdown with tui-markdown (which applies syntax highlighting)
/// and then post-processes to remove fence markers that tui-markdown adds by design.
///
/// # Fence Marker Handling
///
/// tui-markdown intentionally adds fence marker lines (```lang) to code blocks.
/// We filter these out because they're redundant in a TUI - syntax highlighting
/// already indicates code blocks visually.
///
/// # Arguments
/// * `markdown_text` - The markdown content to render
/// * `base_style` - Base style to apply to unstyled text (typically role-based color)
///
/// # Returns
/// Vector of ratatui `Line` objects representing the rendered markdown
fn render_markdown_with_style(markdown_text: &str, base_style: Style) -> Vec<Line<'static>> {
    let text = from_str(markdown_text);

    text.lines
        .into_iter()
        .filter_map(|line| {
            // Filter out fence marker lines that tui-markdown adds
            // Fence markers start with ``` and contain only that marker (possibly with language)
            let line_text: String = line
                .spans
                .iter()
                .map(|span| span.content.as_ref())
                .collect();
            let trimmed = line_text.trim();

            // Skip lines that are fence markers: ``` or ```lang
            if trimmed.starts_with("```") {
                None
            } else {
                // Apply base_style to non-fence-marker lines
                let owned_spans: Vec<_> = line
                    .spans
                    .into_iter()
                    .map(|span| {
                        // Apply base_style as default, then overlay markdown styling
                        let combined_style = base_style.patch(span.style);
                        ratatui::text::Span {
                            content: span.content.into_owned().into(),
                            style: combined_style,
                        }
                    })
                    .collect();
                Some(Line::from(owned_spans))
            }
        })
        .collect()
}

/// Wrap lines to match height calculation behavior.
///
/// Takes source lines and wraps them at the viewport width boundary
/// ensuring rendered line count matches calculated height.
///
/// # Arguments
/// * `source_lines` - Original lines from content (split on '\n')
/// * `wrap_mode` - Whether to wrap or not
/// * `width` - Viewport width for wrapping (terminal width, will adjust for borders)
///
/// # Returns
/// Vector of wrapped lines (String, not Line<'static>) ready for styling
fn wrap_lines(source_lines: &[&str], wrap_mode: WrapMode, width: u16) -> Vec<String> {
    match wrap_mode {
        WrapMode::NoWrap => {
            // No wrapping: one output line per source line
            source_lines.iter().map(|&s| s.to_string()).collect()
        }
        WrapMode::Wrap => {
            // Adjust width for borders (ConversationView uses `area.width.saturating_sub(2)`)
            // The width parameter is terminal width, but content area is 2 chars narrower
            let content_width = width.saturating_sub(2).max(1) as usize;
            let mut wrapped = Vec::new();

            for &line in source_lines {
                if line.is_empty() {
                    // Empty lines stay empty
                    wrapped.push(String::new());
                } else {
                    // Split line into chunks of content_width characters
                    let chars: Vec<char> = line.chars().collect();
                    let mut offset = 0;
                    while offset < chars.len() {
                        let end = (offset + content_width).min(chars.len());
                        let chunk: String = chars[offset..end].iter().collect();
                        wrapped.push(chunk);
                        offset = end;
                    }
                }
            }

            wrapped
        }
    }
}

/// Render a single content block with collapse support and styling.
///
/// Applies both role-based styling (from parent message) and block-specific
/// styling (ToolUse=Yellow, Error=Red).
#[allow(clippy::too_many_arguments)]
fn render_block(
    block: &ContentBlock,
    expanded: bool,
    wrap_mode: WrapMode,
    width: u16,
    collapse_threshold: usize,
    summary_lines: usize,
    role_style: Style,
    styles: &MessageStyles,
) -> Vec<Line<'static>> {
    // Get block-specific style if applicable, otherwise use role style
    let base_style = styles.style_for_content_block(block).unwrap_or(role_style);

    match block {
        ContentBlock::Text { text } => {
            // Split into lines and wrap BEFORE markdown rendering
            // This ensures rendered line count matches height calculation
            let text_lines: Vec<_> = text.lines().collect();
            let wrapped_lines = wrap_lines(&text_lines, wrap_mode, width);

            // Rejoin wrapped lines for markdown parsing
            // Each wrapped line becomes a separate paragraph in markdown
            let wrapped_text = wrapped_lines.join("\n");

            // Parse markdown and render with role-based styling
            let markdown_lines = render_markdown_with_style(&wrapped_text, base_style);

            // Apply collapse logic to markdown-rendered lines
            let total_lines = markdown_lines.len();
            let should_collapse = total_lines > collapse_threshold && !expanded;

            let mut lines = Vec::new();

            if should_collapse {
                // Show summary lines (already markdown-rendered)
                for line in markdown_lines.iter().take(summary_lines) {
                    lines.push(line.clone());
                }
                // Add collapse indicator
                let remaining = total_lines - summary_lines;
                lines.push(Line::from(Span::styled(
                    format!("(+{} more lines)", remaining),
                    Style::default().add_modifier(Modifier::DIM),
                )));
            } else {
                // Show all lines (already markdown-rendered)
                lines.extend(markdown_lines);
            }

            lines
        }
        ContentBlock::ToolUse(tool_call) => {
            let mut lines = Vec::new();

            // Tool name header (always visible) with ToolUse color (Yellow)
            let tool_name = tool_call.name().as_str();
            let header = format!("🔧 Tool: {}", tool_name);
            lines.push(Line::from(Span::styled(
                header,
                base_style.add_modifier(Modifier::BOLD),
            )));

            // Tool input/parameters - collapsible
            let input_json = serde_json::to_string_pretty(tool_call.input()).unwrap_or_default();
            let input_lines: Vec<_> = input_json.lines().collect();

            // Wrap lines to match height calculation
            let wrapped_lines = wrap_lines(&input_lines, wrap_mode, width);
            let total_lines = wrapped_lines.len();
            let should_collapse = total_lines > collapse_threshold && !expanded;

            if should_collapse {
                // Show summary lines with ToolUse styling
                for line in wrapped_lines.iter().take(summary_lines) {
                    lines.push(Line::from(Span::styled(format!("  {}", line), base_style)));
                }
                // Add collapse indicator
                let remaining = total_lines - summary_lines;
                lines.push(Line::from(Span::styled(
                    format!("  (+{} more lines)", remaining),
                    Style::default().add_modifier(Modifier::DIM),
                )));
            } else {
                // Show all lines with ToolUse styling
                for line in wrapped_lines {
                    lines.push(Line::from(Span::styled(format!("  {}", line), base_style)));
                }
            }

            lines
        }
        ContentBlock::ToolResult { content, .. } => {
            let mut lines = Vec::new();
            let content_lines: Vec<_> = content.lines().collect();

            // Wrap lines to match height calculation
            let wrapped_lines = wrap_lines(&content_lines, wrap_mode, width);
            let total_lines = wrapped_lines.len();
            let should_collapse = total_lines > collapse_threshold && !expanded;

            // Determine which lines to show
            let lines_to_show = if should_collapse {
                summary_lines
            } else {
                total_lines
            };

            // Render the visible lines with styling (base_style is Red if is_error=true)
            for line in wrapped_lines.iter().take(lines_to_show) {
                lines.push(Line::from(Span::styled(line.clone(), base_style)));
            }

            // Add collapse indicator if collapsed
            if should_collapse {
                let remaining = total_lines - summary_lines;
                lines.push(Line::from(Span::styled(
                    format!("(+{} more lines)", remaining),
                    Style::default().add_modifier(Modifier::DIM),
                )));
            }

            lines
        }
        ContentBlock::Thinking { thinking } => {
            // THIS IS THE KEY FIX: Thinking blocks now respect collapse state
            // AND wrap long lines to match height calculation
            let thinking_lines: Vec<_> = thinking.lines().collect();

            // Wrap lines to match height calculation
            let wrapped_lines = wrap_lines(&thinking_lines, wrap_mode, width);
            let total_lines = wrapped_lines.len();
            let should_collapse = total_lines > collapse_threshold && !expanded;

            let mut lines = Vec::new();

            if should_collapse {
                // Show summary lines with thinking style (role color + italic/dim)
                for line in wrapped_lines.iter().take(summary_lines) {
                    lines.push(Line::from(Span::styled(
                        line.clone(),
                        base_style
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
                // Show all lines with thinking style (role color + italic/dim)
                for line in wrapped_lines {
                    lines.push(Line::from(Span::styled(
                        line,
                        base_style
                            .add_modifier(Modifier::ITALIC)
                            .add_modifier(Modifier::DIM),
                    )));
                }
            }

            lines
        }
    }
}

/// Format entry index as right-aligned 4-character string with separator.
///
/// Converts 0-based index to 1-based display number, right-aligns in 4 characters,
/// and appends the "│" separator.
///
/// # Arguments
/// * `entry_index` - 0-based index of the entry in the conversation
///
/// # Returns
/// Formatted string like "   1│", "  42│", "1000│"
///
/// # Examples
/// ```ignore
/// format_entry_index(0)   => "   1│"
/// format_entry_index(41)  => "  42│"
/// format_entry_index(999) => "1000│"
/// ```
fn format_entry_index(entry_index: usize) -> String {
    let display_num = entry_index + 1; // Convert 0-based to 1-based
    format!("{:>4}│", display_num)
}

/// Prepend the entry index to a line as a styled prefix.
///
/// Takes an existing Line and prepends the entry index with DarkGray + DIM styling.
/// The index is formatted as a right-aligned number with separator (e.g., "   1│").
///
/// # Arguments
/// * `line` - The line to prepend the index to
/// * `entry_index` - 0-based index of the entry in the conversation
///
/// # Returns
/// A new Line with the index prepended as the first span
fn prepend_index_to_line(line: Line<'static>, entry_index: usize) -> Line<'static> {
    let index_text = format_entry_index(entry_index);
    let index_span = Span::styled(
        index_text,
        Style::default()
            .fg(Color::DarkGray)
            .add_modifier(Modifier::DIM),
    );

    // Create new line with index span prepended
    let mut new_spans = vec![index_span];
    new_spans.extend(line.spans);

    Line::from(new_spans)
}

/// Apply search highlighting to plain text.
///
/// Takes plain text and a list of matches (offset, length, is_current) and returns
/// a Line with spans that have yellow background for matches and REVERSED modifier
/// for the current match.
///
/// This function splits the text into spans:
/// - Unhighlighted text: base_style
/// - Other matches: base_style + yellow background
/// - Current match: base_style + yellow background + REVERSED
fn apply_highlights_to_text(
    text: &str,
    matches: &[(usize, usize, bool)], // (offset, length, is_current)
    base_style: Style,
) -> Line<'static> {
    if matches.is_empty() {
        return Line::from(vec![Span::styled(text.to_string(), base_style)]);
    }

    let mut spans = Vec::new();
    let mut last_pos = 0;

    // Sort matches by offset
    let mut sorted_matches = matches.to_vec();
    sorted_matches.sort_by_key(|(offset, _, _)| *offset);

    for (offset, length, is_current) in sorted_matches {
        // Add text before match
        if offset > last_pos {
            spans.push(Span::styled(text[last_pos..offset].to_string(), base_style));
        }

        // Add highlighted match
        let end = offset + length;
        if end <= text.len() {
            let match_style = if is_current {
                // Current match: reversed/inverted
                base_style
                    .bg(Color::Yellow)
                    .add_modifier(Modifier::REVERSED)
            } else {
                // Other matches: yellow background
                base_style.bg(Color::Yellow)
            };

            spans.push(Span::styled(text[offset..end].to_string(), match_style));
            last_pos = end;
        }
    }

    // Add remaining text after last match
    if last_pos < text.len() {
        spans.push(Span::styled(text[last_pos..].to_string(), base_style));
    }

    Line::from(spans)
}

#[cfg(test)]
#[path = "renderer_tests.rs"]
mod renderer_tests;
