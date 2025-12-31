//! Conversation view widget - shared by main and subagent panes.
//!
//! Implements virtualized rendering to handle large conversations efficiently.
//! Only renders visible messages (plus ±20 buffer) based on scroll position.

use crate::model::{ContentBlock, ConversationEntry, MessageContent};
use crate::state::WrapMode;
use crate::view::MessageStyles;
use crate::view_state::conversation::ConversationViewState;
use crate::view_state::types::ViewportDimensions;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, Borders, Paragraph, Widget},
};
use unicode_width::UnicodeWidthStr;

// ===== Entry Layout =====

/// Layout information for a single conversation entry.
///
/// Maps an entry to its vertical position (y_offset) and height in the rendered view.
/// Used for virtualized rendering to determine which entries are visible and where to place them.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct EntryLayout {
    /// Vertical offset from the top of the conversation (in lines).
    y_offset: u16,
    /// Height of the entry in lines (accounting for wrapping and collapse state).
    height: u16,
}

/// Calculate how many lines to skip from the top of an entry when it's partially scrolled off.
///
/// When an entry starts above the viewport (cumulative_y < scroll_offset) but is partially
/// visible, we need to skip the lines that are scrolled off the top.
///
/// # Arguments
/// * `cumulative_y` - Absolute position where the entry starts in the document
/// * `scroll_offset` - Current vertical scroll position (top of viewport)
///
/// # Returns
/// Number of lines to skip from the top of the entry (0 if fully visible)
#[allow(dead_code)]
fn calculate_lines_to_skip(cumulative_y: usize, scroll_offset: usize) -> usize {
    // If entry starts before scroll position, skip the lines above viewport
    // saturating_sub returns 0 if cumulative_y >= scroll_offset (fully visible)
    scroll_offset.saturating_sub(cumulative_y)
}

// ===== ConversationView Widget =====

/// Virtualized conversation view widget.
///
/// Renders only visible messages (plus ±20 buffer) for performance.
/// Shared by both main agent and subagent panes.
pub struct ConversationView<'a> {
    view_state: &'a ConversationViewState,
    _styles: &'a MessageStyles,
    focused: bool,
    is_subagent_view: bool,
    collapse_threshold: usize,
    summary_lines: usize,
    buffer_size: usize,
    global_wrap: WrapMode,
}

impl<'a> ConversationView<'a> {
    /// Create a new ConversationView widget.
    ///
    /// # Arguments
    /// * `view_state` - View state (contains entries, scroll, expansion state, agent/model metadata)
    /// * `styles` - Message styling configuration
    /// * `focused` - Whether this pane currently has focus (affects border color)
    pub fn new(
        view_state: &'a ConversationViewState,
        styles: &'a MessageStyles,
        focused: bool,
    ) -> Self {
        Self {
            view_state,
            _styles: styles,
            focused,
            is_subagent_view: false, // Default to false (main agent view)
            collapse_threshold: 10,
            summary_lines: 3,
            buffer_size: 20,
            global_wrap: WrapMode::default(), // Default to Wrap
        }
    }

    /// Set the collapse threshold (number of lines before collapsing).
    pub fn collapse_threshold(mut self, threshold: usize) -> Self {
        self.collapse_threshold = threshold;
        self
    }

    /// Set the summary lines (number of lines shown when collapsed).
    pub fn summary_lines(mut self, lines: usize) -> Self {
        self.summary_lines = lines;
        self
    }

    /// Set the buffer size (number of entries above/below viewport to render).
    pub fn buffer_size(mut self, size: usize) -> Self {
        self.buffer_size = size;
        self
    }

    /// Set whether this is a subagent view (enables initial prompt labeling).
    ///
    /// # Arguments
    /// * `is_subagent` - true for subagent conversations, false for main agent
    ///
    /// # Returns
    /// Updated ConversationView with is_subagent_view set
    pub fn is_subagent_view(mut self, is_subagent: bool) -> Self {
        self.is_subagent_view = is_subagent;
        self
    }

    /// Set the global wrap mode.
    pub fn global_wrap(mut self, wrap: WrapMode) -> Self {
        self.global_wrap = wrap;
        self
    }
}

/// Widget implementation for ConversationView
impl<'a> Widget for ConversationView<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let entry_count = self.view_state.len();

        // Build title with agent info
        let base_title = if let Some(agent_id) = self.view_state.agent_id() {
            // Subagent conversation
            let model_info = self
                .view_state
                .model()
                .map(|m| format!(" [{}]", m.display_name()))
                .unwrap_or_default();
            format!(
                "Subagent {}{} ({} entries)",
                agent_id, model_info, entry_count
            )
        } else {
            // Main agent conversation
            let model_info = self
                .view_state
                .model()
                .map(|m| format!(" [{}]", m.display_name()))
                .unwrap_or_default();
            format!("Main Agent{} ({} entries)", model_info, entry_count)
        };

        // Calculate viewport dimensions (area minus borders)
        let viewport_height = area.height.saturating_sub(2);
        let viewport_width = area.width.saturating_sub(2);

        // Render content: only render visible entries
        let mut lines = Vec::new();

        if entry_count == 0 {
            lines.push(Line::from("No messages yet..."));
        } else {
            // Calculate which entries are visible using view-state layer
            let viewport = ViewportDimensions::new(viewport_width, viewport_height);
            let visible_range = self.view_state.visible_range(viewport);
            let scroll_offset = visible_range.scroll_offset.get();

            // Track previous session ID to detect boundaries (FR-074)
            let mut prev_session_id: Option<&crate::model::SessionId> = None;

            // Render only the visible range
            for entry_index in visible_range.indices() {
                let entry_view = match self.view_state.get(entry_index) {
                    Some(ev) => ev,
                    None => continue, // Skip if entry not found (shouldn't happen)
                };

                let entry = entry_view.entry();
                let cumulative_y = self
                    .view_state
                    .entry_cumulative_y(entry_index)
                    .map(|offset| offset.get())
                    .unwrap_or(0);

                // FR-074: Detect session boundary and render separator
                // Only render separator if session changed from previous entry
                if let Some(current_session_id) = entry.session_id() {
                    let session_changed =
                        prev_session_id.is_some_and(|prev| prev != current_session_id);

                    if session_changed {
                        // Render session separator line
                        lines.push(render_session_separator(current_session_id));
                    }

                    // Update tracking for next iteration
                    prev_session_id = Some(current_session_id);
                }

                // Calculate how many lines to skip from this entry if it's partially scrolled off
                let lines_to_skip = if cumulative_y < scroll_offset {
                    scroll_offset.saturating_sub(cumulative_y)
                } else {
                    0
                };

                // Use pre-computed rendered lines from EntryView
                let entry_lines = entry_view.rendered_lines().to_vec();

                // Skip lines that are scrolled off the top, then add to final lines
                lines.extend(entry_lines.into_iter().skip(lines_to_skip));
            }
        }

        // Check if content extends beyond viewport BEFORE applying horizontal offset
        // (because offset trims the lines, hiding the fact they were long)
        let horizontal_offset = self.view_state.horizontal_offset();
        let has_long_lines_flag = has_long_lines(&lines, viewport_width as usize);

        // Apply horizontal scroll offset if in NoWrap mode
        let final_lines: Vec<Line<'static>> =
            if self.global_wrap == WrapMode::NoWrap && horizontal_offset > 0 {
                lines
                    .into_iter()
                    .map(|line| apply_horizontal_offset(line, horizontal_offset as usize))
                    .collect()
            } else {
                lines
            };

        // Add scroll indicators to title if content extends beyond viewport
        let has_left = horizontal_offset > 0;
        let has_right = has_long_lines_flag;
        let title = add_scroll_indicators_to_title(base_title, has_left, has_right);

        // Style based on focus
        let border_color = if self.focused {
            Color::Cyan
        } else {
            Color::Gray
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .title(title)
            .style(Style::default().fg(border_color));

        // Render paragraph without additional wrapping
        // Lines are already pre-wrapped by compute_entry_lines based on wrap mode
        let paragraph = Paragraph::new(final_lines).block(block);
        paragraph.render(area, buf);
    }
}

/// Detect if markdown content contains code blocks.
///
/// Code blocks are detected as:
/// - Fenced code blocks: lines starting with ``` or ~~~
/// - Indented code blocks: lines with 4+ leading spaces (after list markers)
///
/// FR-053: Code blocks must never wrap, always using horizontal scroll.
///
/// # Arguments
/// * `content` - Markdown text to scan for code blocks
///
/// # Returns
/// `true` if any code block (fenced or indented) is present, `false` otherwise
///
/// # Visibility
/// Public for property testing in integration tests (FR-053).
pub fn has_code_blocks(content: &str) -> bool {
    for line in content.lines() {
        // Check for fenced code blocks (``` or ~~~)
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            return true;
        }

        // Check for indented code blocks (4+ leading spaces)
        // Count leading spaces
        let leading_spaces = line.len() - line.trim_start().len();
        if leading_spaces >= 4 && !line.trim().is_empty() {
            return true;
        }
    }

    false
}

/// Extract text content from a ConversationEntry to scan for code blocks.
///
/// Concatenates all text blocks from the entry's message content.
/// Used by `should_entry_wrap` to determine if code blocks are present.
///
/// Extracts text from:
/// - `ContentBlock::Text { text }` - User-visible markdown content
/// - `ContentBlock::Thinking { thinking }` - Internal reasoning (can contain code blocks)
/// - `ContentBlock::ToolResult { content, .. }` - Tool output (frequently contains code/commands)
///
/// FR-053: Code blocks must never wrap, so we need to detect them in ALL content types.
///
/// # Arguments
/// * `entry` - The conversation entry to extract text from
///
/// # Returns
/// Concatenated text content from all text blocks in the entry
///
/// # Visibility
/// Public for property testing in integration tests (FR-053).
pub fn extract_entry_text(entry: &ConversationEntry) -> String {
    match entry {
        ConversationEntry::Valid(log_entry) => {
            let message = log_entry.message();
            match message.content() {
                MessageContent::Text(text) => text.clone(),
                MessageContent::Blocks(blocks) => {
                    let mut result = String::new();
                    for block in blocks {
                        match block {
                            ContentBlock::Text { text } => {
                                result.push_str(text);
                                result.push('\n');
                            }
                            ContentBlock::Thinking { thinking } => {
                                result.push_str(thinking);
                                result.push('\n');
                            }
                            ContentBlock::ToolResult { content, .. } => {
                                result.push_str(content);
                                result.push('\n');
                            }
                            ContentBlock::ToolUse(_) => {
                                // ToolUse blocks don't contain text content to scan
                            }
                        }
                    }
                    result
                }
            }
        }
        ConversationEntry::Malformed(_) => String::new(),
    }
}

fn apply_horizontal_offset(line: Line<'static>, offset: usize) -> Line<'static> {
    if offset == 0 {
        return line;
    }

    // Check if first span is the index prefix (contains "│")
    let has_index = line
        .spans
        .first()
        .map(|span| span.content.contains('│'))
        .unwrap_or(false);

    if has_index {
        // Preserve index span, apply offset to remaining content
        let index_span = line.spans[0].clone();
        let content_spans = &line.spans[1..];

        // Calculate total chars in content (excluding index)
        let total_chars: usize = content_spans
            .iter()
            .map(|span| span.content.chars().count())
            .sum();

        if offset >= total_chars {
            // Offset exceeds content, return just index
            return Line::from(vec![index_span]);
        }

        // Skip characters in content spans
        let mut chars_to_skip = offset;
        let mut new_spans = vec![index_span];

        for span in content_spans {
            let span_char_count = span.content.chars().count();

            if chars_to_skip >= span_char_count {
                chars_to_skip -= span_char_count;
            } else if chars_to_skip > 0 {
                let remaining =
                    if let Some((byte_idx, _)) = span.content.char_indices().nth(chars_to_skip) {
                        span.content[byte_idx..].to_string()
                    } else {
                        String::new()
                    };
                chars_to_skip = 0;
                new_spans.push(ratatui::text::Span::styled(remaining, span.style));
            } else {
                new_spans.push(span.clone());
            }
        }

        return Line::from(new_spans);
    }

    // No index - apply offset to entire line
    let total_chars: usize = line
        .spans
        .iter()
        .map(|span| span.content.chars().count())
        .sum();

    if offset >= total_chars {
        return Line::from(vec![]);
    }

    let mut chars_to_skip = offset;
    let mut new_spans = Vec::new();

    for span in line.spans {
        let span_char_count = span.content.chars().count();

        if chars_to_skip >= span_char_count {
            // Skip entire span
            chars_to_skip -= span_char_count;
        } else if chars_to_skip > 0 {
            // Skip partial span - use char_indices for UTF-8 safety
            let remaining =
                if let Some((byte_idx, _)) = span.content.char_indices().nth(chars_to_skip) {
                    span.content[byte_idx..].to_string()
                } else {
                    // Shouldn't happen, but safe fallback
                    String::new()
                };
            chars_to_skip = 0;
            new_spans.push(ratatui::text::Span {
                content: remaining.into(),
                style: span.style,
            });
        } else {
            // No more skipping, keep span as-is
            new_spans.push(span);
        }
    }

    Line::from(new_spans)
}

/// Check if any line in the collection exceeds the viewport width.
///
/// Uses visual width (not byte count) for correct Unicode handling.
fn has_long_lines(lines: &[Line], viewport_width: usize) -> bool {
    lines.iter().any(|line| {
        let width: usize = line.spans.iter().map(|s| s.content.width()).sum();
        width > viewport_width
    })
}

/// Add horizontal scroll indicators to title if needed.
///
/// Prepends ◀ if offset > 0 (can scroll left).
/// Appends ▶ if content extends beyond viewport (can scroll right).
///
/// Returns modified title string with indicators.
fn add_scroll_indicators_to_title(base_title: String, has_left: bool, has_right: bool) -> String {
    let mut title = base_title;

    if has_left {
        title = format!("◀ {}", title);
    }

    if has_right {
        title = format!("{} ▶", title);
    }

    title
}

// ===== Session Separator Rendering (FR-074) =====

/// Render a session separator line.
///
/// Format: "─────────── Session: <session_id> ───────────"
/// Styling: Dim gray to distinguish from content
///
/// # Arguments
/// * `session_id` - The ID of the new session starting after this separator
///
/// # Returns
/// A single Line with the separator text and dim gray styling
fn render_session_separator(session_id: &crate::model::SessionId) -> Line<'static> {
    use ratatui::text::Span;

    let separator_text = format!("─────────── Session: {} ───────────", session_id);

    Line::from(vec![Span::styled(
        separator_text,
        Style::default()
            .fg(Color::DarkGray)
            .add_modifier(Modifier::DIM),
    )])
}
