//! Conversation view widget - shared by main and subagent panes.
//!
//! Implements virtualized rendering to handle large conversations efficiently.
//! Only renders visible messages (plus ±20 buffer) based on scroll position.

use crate::model::{AgentConversation, ContentBlock, MessageContent, ToolCall};
use crate::state::ScrollState;
use crate::view::MessageStyles;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, Borders, Paragraph, Widget},
    Frame,
};
use tui_markdown::from_str;

// ===== ConversationView Widget =====

/// Virtualized conversation view widget.
///
/// Renders only visible messages (plus ±20 buffer) for performance.
/// Shared by both main agent and subagent panes.
pub struct ConversationView<'a> {
    conversation: &'a AgentConversation,
    scroll_state: &'a ScrollState,
    styles: &'a MessageStyles,
    focused: bool,
    is_subagent_view: bool,
    collapse_threshold: usize,
    summary_lines: usize,
    buffer_size: usize,
}

impl<'a> ConversationView<'a> {
    /// Create a new ConversationView widget.
    ///
    /// # Arguments
    /// * `conversation` - The agent conversation to display
    /// * `scroll_state` - Scroll state (for expansion tracking and scrolling)
    /// * `styles` - Message styling configuration
    /// * `focused` - Whether this pane currently has focus (affects border color)
    pub fn new(
        conversation: &'a AgentConversation,
        scroll_state: &'a ScrollState,
        styles: &'a MessageStyles,
        focused: bool,
    ) -> Self {
        Self {
            conversation,
            scroll_state,
            styles,
            focused,
            is_subagent_view: false, // Default to false (main agent view)
            collapse_threshold: 10,
            summary_lines: 3,
            buffer_size: 20,
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

    /// Calculate the height in lines for a single log entry.
    ///
    /// Accounts for collapsed state based on scroll_state expansion tracking.
    fn calculate_entry_height(&self, entry: &crate::model::LogEntry) -> usize {
        let is_expanded = self.scroll_state.is_expanded(entry.uuid());

        match entry.message().content() {
            MessageContent::Text(text) => {
                let total_lines = text.lines().count();
                if total_lines > self.collapse_threshold && !is_expanded {
                    // Collapsed: summary_lines + 1 indicator line
                    self.summary_lines + 1
                } else {
                    total_lines
                }
            }
            MessageContent::Blocks(blocks) => {
                let mut total_height = 0;
                let role = entry.message().role();
                let role_style = self.styles.style_for_role(role);

                for block in blocks {
                    let block_lines = render_content_block(
                        block,
                        entry.uuid(),
                        self.scroll_state,
                        self.styles,
                        role_style,
                        self.collapse_threshold,
                        self.summary_lines,
                    );
                    total_height += block_lines.len();
                }
                // Add spacing between entries
                total_height + 1
            }
        }
    }

    /// Determine the range of entries that should be rendered based on viewport.
    ///
    /// Returns (start_index, end_index) for the visible range including buffer.
    fn calculate_visible_range(&self, viewport_height: usize) -> (usize, usize) {
        let entries = self.conversation.entries();
        let total_entries = entries.len();

        if total_entries == 0 {
            return (0, 0);
        }

        let scroll_offset = self.scroll_state.vertical_offset;

        // Calculate which entry the scroll offset corresponds to
        let mut cumulative_height = 0;
        let mut start_entry_index = 0;

        // Find the first entry that should be visible (accounting for buffer)
        for (i, entry) in entries.iter().enumerate() {
            let entry_height = self.calculate_entry_height(entry);
            if cumulative_height + entry_height > scroll_offset.saturating_sub(self.buffer_size) {
                start_entry_index = i;
                break;
            }
            cumulative_height += entry_height;
        }

        // Find the last entry that should be visible (accounting for buffer)
        let mut end_entry_index = start_entry_index;
        cumulative_height = 0;

        for (i, entry) in entries.iter().enumerate().skip(start_entry_index) {
            let entry_height = self.calculate_entry_height(entry);
            cumulative_height += entry_height;

            if cumulative_height > viewport_height + (self.buffer_size * 2) {
                end_entry_index = i;
                break;
            }
            end_entry_index = i + 1;
        }

        // Ensure we don't exceed bounds
        end_entry_index = end_entry_index.min(total_entries);

        (start_entry_index, end_entry_index)
    }
}

impl<'a> Widget for ConversationView<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let entry_count = self.conversation.entries().len();

        // Build title with agent info
        let title = if let Some(agent_id) = self.conversation.agent_id() {
            // Subagent conversation
            let model_info = self
                .conversation
                .model()
                .map(|m| format!(" [{}]", m.display_name()))
                .unwrap_or_default();
            format!("Subagent {}{} ({} entries)", agent_id, model_info, entry_count)
        } else {
            // Main agent conversation
            let model_info = self
                .conversation
                .model()
                .map(|m| format!(" [{}]", m.display_name()))
                .unwrap_or_default();
            format!("Main Agent{} ({} entries)", model_info, entry_count)
        };

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

        // Calculate viewport height (area height minus borders)
        let viewport_height = area.height.saturating_sub(2) as usize;

        // Render content: only render visible entries
        let mut lines = Vec::new();

        if entry_count == 0 {
            lines.push(Line::from("No messages yet..."));
        } else {
            // Calculate which entries are visible
            let (start_index, end_index) = self.calculate_visible_range(viewport_height);

            // Render only the visible range
            for (visible_index, entry) in self.conversation.entries()[start_index..end_index].iter().enumerate() {
                let role = entry.message().role();
                let role_style = self.styles.style_for_role(role);

                // Calculate actual index in full entry list
                let actual_index = start_index + visible_index;

                // Add "Initial Prompt" label for first message in subagent view
                if self.is_subagent_view && actual_index == 0 {
                    lines.push(Line::from(vec![
                        ratatui::text::Span::styled(
                            "🔷 Initial Prompt",
                            Style::default()
                                .fg(Color::Magenta)
                                .add_modifier(Modifier::BOLD),
                        ),
                    ]));
                }

                match entry.message().content() {
                    MessageContent::Text(text) => {
                        // Simple text message - render each line with role-based styling
                        for line in text.lines() {
                            lines.push(Line::from(vec![ratatui::text::Span::styled(
                                line.to_string(),
                                role_style,
                            )]));
                        }
                    }
                    MessageContent::Blocks(blocks) => {
                        // Structured content - render each block
                        for block in blocks {
                            let block_lines = render_content_block(
                                block,
                                entry.uuid(),
                                self.scroll_state,
                                self.styles,
                                role_style,
                                self.collapse_threshold,
                                self.summary_lines,
                            );
                            lines.extend(block_lines);
                        }
                    }
                }
                // Add spacing between entries
                lines.push(Line::from(""));
            }
        }

        let paragraph = Paragraph::new(lines).block(block);
        paragraph.render(area, buf);
    }
}

/// Render a conversation view for either main agent or subagent.
///
/// This is the shared widget used by both panes. It takes an AgentConversation
/// reference and renders it consistently regardless of which pane it's in.
///
/// # Arguments
/// * `frame` - The ratatui frame to render into
/// * `area` - The area to render within
/// * `conversation` - The agent conversation to display
/// * `scroll` - Scroll state (for expansion tracking and scrolling)
/// * `styles` - Message styling configuration
/// * `focused` - Whether this pane currently has focus (affects border color)
pub fn render_conversation_view(
    frame: &mut Frame,
    area: Rect,
    conversation: &AgentConversation,
    scroll: &ScrollState,
    styles: &MessageStyles,
    focused: bool,
) {
    let entry_count = conversation.entries().len();

    // Build title with agent info
    let title = if let Some(agent_id) = conversation.agent_id() {
        // Subagent conversation
        let model_info = conversation
            .model()
            .map(|m| format!(" [{}]", m.display_name()))
            .unwrap_or_default();
        format!("Subagent {}{} ({} entries)", agent_id, model_info, entry_count)
    } else {
        // Main agent conversation
        let model_info = conversation
            .model()
            .map(|m| format!(" [{}]", m.display_name()))
            .unwrap_or_default();
        format!("Main Agent{} ({} entries)", model_info, entry_count)
    };

    // Style based on focus
    let border_color = if focused { Color::Cyan } else { Color::Gray };

    // Calculate viewport width (subtract borders)
    let viewport_width = area.width.saturating_sub(2) as usize;

    // Render content: collect all lines from all entries
    let mut lines = Vec::new();

    if entry_count == 0 {
        lines.push(Line::from("No messages yet..."));
    } else {
        // Determine if this is a subagent conversation
        let is_subagent_view = conversation.agent_id().is_some();

        // Iterate through all entries and render their content blocks
        for (entry_index, entry) in conversation.entries().iter().enumerate() {
            let role = entry.message().role();
            let role_style = styles.style_for_role(role);

            // Add "Initial Prompt" label for first message in subagent view
            if is_subagent_view && entry_index == 0 {
                lines.push(Line::from(vec![ratatui::text::Span::styled(
                    "🔷 Initial Prompt",
                    Style::default()
                        .fg(Color::Magenta)
                        .add_modifier(Modifier::BOLD),
                )]));
            }

            match entry.message().content() {
                MessageContent::Text(text) => {
                    // Simple text message - apply collapse/expand logic with role-based styling
                    let text_lines: Vec<_> = text.lines().collect();
                    let total_lines = text_lines.len();
                    let collapse_threshold = 10;
                    let summary_lines = 3;

                    let is_expanded = scroll.is_expanded(entry.uuid());
                    let should_collapse = total_lines > collapse_threshold && !is_expanded;

                    if should_collapse {
                        // Show summary lines with role styling
                        for line in text_lines.iter().take(summary_lines) {
                            lines.push(Line::from(vec![ratatui::text::Span::styled(
                                line.to_string(),
                                role_style,
                            )]));
                        }
                        // Add collapse indicator
                        let remaining = total_lines - summary_lines;
                        lines.push(Line::from(vec![ratatui::text::Span::styled(
                            format!("(+{} more lines)", remaining),
                            Style::default()
                                .fg(Color::DarkGray)
                                .add_modifier(Modifier::DIM),
                        )]));
                    } else {
                        // Show all lines with role styling
                        for line in text_lines {
                            lines.push(Line::from(vec![ratatui::text::Span::styled(
                                line.to_string(),
                                role_style,
                            )]));
                        }
                    }
                }
                MessageContent::Blocks(blocks) => {
                    // Structured content - render each block
                    for block in blocks {
                        let block_lines = render_content_block(
                            block,
                            entry.uuid(),
                            scroll,
                            styles,
                            role_style,
                            10, // Default collapse threshold
                            3,  // Default summary lines
                        );
                        lines.extend(block_lines);
                    }
                }
            }
            // Add spacing between entries
            lines.push(Line::from(""));
        }
    }

    // Determine scroll indicators (FR-040) - check BEFORE applying offset
    let horizontal_offset = scroll.horizontal_offset;
    let has_left_indicator = horizontal_offset > 0;
    let has_right_indicator = has_long_lines(&lines, viewport_width + horizontal_offset);

    // Apply horizontal scrolling offset to all lines (FR-039)
    if horizontal_offset > 0 {
        lines = lines.into_iter()
            .map(|line| apply_horizontal_offset(line, horizontal_offset))
            .collect();
    }

    // Update title with scroll indicators
    let title_with_indicators = add_scroll_indicators_to_title(
        title,
        has_left_indicator,
        has_right_indicator,
    );

    // Rebuild block with updated title
    let block_with_indicators = Block::default()
        .borders(Borders::ALL)
        .title(title_with_indicators)
        .style(Style::default().fg(border_color));

    let paragraph = Paragraph::new(lines).block(block_with_indicators);
    frame.render_widget(paragraph, area);
}

// ===== Horizontal Scrolling Helpers =====

/// Apply horizontal offset to a line, skipping the first `offset` characters.
///
/// Returns a new Line with characters starting from `offset` position.
/// If offset exceeds line length, returns empty line.
fn apply_horizontal_offset(line: Line<'static>, offset: usize) -> Line<'static> {
    if offset == 0 {
        return line;
    }

    // Calculate total visible width of the line
    let total_width: usize = line.spans.iter().map(|span| span.content.len()).sum();

    if offset >= total_width {
        // Offset exceeds line length, return empty
        return Line::from(vec![]);
    }

    // Skip characters across spans
    let mut chars_to_skip = offset;
    let mut new_spans = Vec::new();

    for span in line.spans {
        let span_len = span.content.len();

        if chars_to_skip >= span_len {
            // Skip entire span
            chars_to_skip -= span_len;
        } else if chars_to_skip > 0 {
            // Skip partial span
            let remaining = span.content[chars_to_skip..].to_string();
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
fn has_long_lines(lines: &[Line], viewport_width: usize) -> bool {
    lines.iter().any(|line| {
        let width: usize = line.spans.iter().map(|s| s.content.len()).sum();
        width > viewport_width
    })
}

/// Add horizontal scroll indicators to lines if needed.
///
/// Prepends ◀ if offset > 0 (can scroll left).
/// Appends ▶ if content extends beyond viewport (can scroll right).
///
/// Returns modified title string with indicators.
fn add_scroll_indicators_to_title(
    base_title: String,
    has_left: bool,
    has_right: bool,
) -> String {
    let mut title = base_title;

    if has_left {
        title = format!("◀ {}", title);
    }

    if has_right {
        title = format!("{} ▶", title);
    }

    title
}

// ===== Markdown Rendering =====

/// Render markdown text with role-based styling applied to unstyled spans.
///
/// Markdown styling (bold, italic, code highlighting) takes precedence,
/// but plain text inherits the role's color.
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
        .map(|line| {
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
            Line::from(owned_spans)
        })
        .collect()
}

// ===== Content Block Rendering =====

/// Render a ContentBlock::ToolUse as formatted lines with collapse/expand support.
///
/// Displays:
/// - Tool name as header (always visible, styled with tool_style)
/// - Tool input/parameters (collapsible if exceeds threshold)
/// - When collapsed: Shows first `summary_lines` + "(+N more lines)" indicator
/// - When expanded: Shows all parameter lines
///
/// # Arguments
/// * `tool_call` - The tool call to render
/// * `entry_uuid` - UUID of the entry (for expansion state lookup)
/// * `scroll_state` - Scroll state containing expansion tracking
/// * `tool_style` - Style to apply to tool call content
/// * `collapse_threshold` - Number of lines before collapsing (default: 10)
/// * `summary_lines` - Number of lines to show when collapsed (default: 3)
///
/// # Returns
/// Vector of ratatui `Line` objects representing the rendered tool use block
pub fn render_tool_use(
    tool_call: &ToolCall,
    entry_uuid: &crate::model::EntryUuid,
    scroll_state: &ScrollState,
    tool_style: Style,
    collapse_threshold: usize,
    summary_lines: usize,
) -> Vec<Line<'static>> {
    use ratatui::text::Span;

    let mut lines = Vec::new();

    // Tool name header (always visible, with tool_style + bold)
    let tool_name = tool_call.name().as_str();
    let header = format!("🔧 Tool: {}", tool_name);
    lines.push(Line::from(vec![Span::styled(
        header,
        tool_style.add_modifier(Modifier::BOLD),
    )]));

    // Tool input/parameters - collapsible
    let input_json = serde_json::to_string_pretty(tool_call.input()).unwrap_or_default();
    let input_lines: Vec<_> = input_json.lines().collect();
    let total_lines = input_lines.len();

    let is_expanded = scroll_state.is_expanded(entry_uuid);
    let should_collapse = total_lines > collapse_threshold && !is_expanded;

    if should_collapse {
        // Show summary lines
        for line in input_lines.iter().take(summary_lines) {
            lines.push(Line::from(format!("  {}", line)));
        }
        // Add collapse indicator
        let remaining = total_lines - summary_lines;
        lines.push(Line::from(vec![Span::styled(
            format!("  (+{} more lines)", remaining),
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::DIM),
        )]));
    } else {
        // Show all lines
        for line in input_lines {
            lines.push(Line::from(format!("  {}", line)));
        }
    }

    lines
}

/// Render a ContentBlock::ToolResult as formatted lines with collapse/expand support.
///
/// Displays:
/// - Output content (collapsible if exceeds threshold)
/// - Styled with result_style (which may be error red or default)
/// - When collapsed: Shows first `summary_lines` + "(+N more lines)" indicator
/// - When expanded: Shows all output lines
///
/// # Arguments
/// * `content` - The tool result content string
/// * `is_error` - Whether this result represents an error
/// * `entry_uuid` - UUID of the entry (for expansion state lookup)
/// * `scroll_state` - Scroll state containing expansion tracking
/// * `result_style` - Style to apply to result content (red for errors)
/// * `collapse_threshold` - Number of lines before collapsing (default: 10)
/// * `summary_lines` - Number of lines to show when collapsed (default: 3)
///
/// # Returns
/// Vector of ratatui `Line` objects representing the rendered tool result
pub fn render_tool_result(
    content: &str,
    is_error: bool,
    entry_uuid: &crate::model::EntryUuid,
    scroll_state: &ScrollState,
    result_style: Style,
    collapse_threshold: usize,
    summary_lines: usize,
) -> Vec<Line<'static>> {
    use ratatui::text::Span;

    let mut lines = Vec::new();
    let content_lines: Vec<_> = content.lines().collect();
    let total_lines = content_lines.len();

    let is_expanded = scroll_state.is_expanded(entry_uuid);
    let should_collapse = total_lines > collapse_threshold && !is_expanded;

    // Determine which lines to show
    let lines_to_show = if should_collapse {
        summary_lines
    } else {
        total_lines
    };

    // Render the visible lines with styling
    for line in content_lines.iter().take(lines_to_show) {
        let rendered_line = if is_error {
            Line::from(vec![Span::styled(
                line.to_string(),
                result_style,
            )])
        } else {
            Line::from(line.to_string())
        };
        lines.push(rendered_line);
    }

    // Add collapse indicator if collapsed
    if should_collapse {
        let remaining = total_lines - summary_lines;
        lines.push(Line::from(vec![Span::styled(
            format!("(+{} more lines)", remaining),
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::DIM),
        )]));
    }

    lines
}

/// Render any ContentBlock variant as formatted lines with collapse/expand support.
///
/// Delegates to specific rendering functions based on block type.
///
/// # Arguments
/// * `block` - The content block to render
/// * `entry_uuid` - UUID of the entry (for expansion state lookup)
/// * `scroll_state` - Scroll state containing expansion tracking
/// * `styles` - Message styling configuration
/// * `role_style` - Default style for this message's role
/// * `collapse_threshold` - Number of lines before collapsing (default: 10)
/// * `summary_lines` - Number of lines to show when collapsed (default: 3)
///
/// # Returns
/// Vector of ratatui `Line` objects representing the rendered block
pub fn render_content_block(
    block: &ContentBlock,
    entry_uuid: &crate::model::EntryUuid,
    scroll_state: &ScrollState,
    styles: &MessageStyles,
    role_style: Style,
    collapse_threshold: usize,
    summary_lines: usize,
) -> Vec<Line<'static>> {
    use ratatui::text::Span;

    // Check if this block has specific styling (tool calls, errors)
    let block_style = styles.style_for_content_block(block);

    match block {
        ContentBlock::Text { text } => {
            // Render markdown text with role-based styling
            let markdown_lines = render_markdown_with_style(text, role_style);
            let total_lines = markdown_lines.len();

            let is_expanded = scroll_state.is_expanded(entry_uuid);
            let should_collapse = total_lines > collapse_threshold && !is_expanded;

            let mut lines = Vec::new();

            if should_collapse {
                // Show summary lines
                lines.extend(markdown_lines.into_iter().take(summary_lines));
                // Add collapse indicator
                let remaining = total_lines - summary_lines;
                lines.push(Line::from(vec![Span::styled(
                    format!("(+{} more lines)", remaining),
                    Style::default()
                        .fg(Color::DarkGray)
                        .add_modifier(Modifier::DIM),
                )]));
            } else {
                // Show all lines
                lines.extend(markdown_lines);
            }

            lines
        }
        ContentBlock::ToolUse(tool_call) => {
            let tool_style = block_style.unwrap_or(role_style);
            render_tool_use(
                tool_call,
                entry_uuid,
                scroll_state,
                tool_style,
                collapse_threshold,
                summary_lines,
            )
        }
        ContentBlock::ToolResult {
            tool_use_id: _,
            content,
            is_error,
        } => {
            let result_style = block_style.unwrap_or(role_style);
            render_tool_result(
                content,
                *is_error,
                entry_uuid,
                scroll_state,
                result_style,
                collapse_threshold,
                summary_lines,
            )
        }
        ContentBlock::Thinking { thinking } => thinking
            .lines()
            .map(|line| {
                Line::from(vec![Span::styled(
                    line.to_string(),
                    Style::default()
                        .add_modifier(Modifier::ITALIC)
                        .add_modifier(Modifier::DIM),
                )])
            })
            .collect(),
    }
}

// ===== Tests =====

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ToolName, ToolUseId};
    use std::collections::HashSet;

    // ===== Helper: Create test scroll state =====

    fn create_test_scroll_state() -> ScrollState {
        ScrollState::default()
    }

    fn create_expanded_scroll_state(uuid: &crate::model::EntryUuid) -> ScrollState {
        let mut state = ScrollState::default();
        state.toggle_expand(uuid);
        state
    }

    // ===== Helper: Create test message styles =====

    fn create_test_styles() -> MessageStyles {
        MessageStyles::new()
    }

    fn get_test_role_style() -> Style {
        create_test_styles().style_for_role(crate::model::Role::Assistant)
    }

    // ===== render_tool_use Tests =====

    #[test]
    fn render_tool_use_with_short_input_shows_all_lines() {
        let id = ToolUseId::new("tool-123").expect("valid id");
        let tool_call = ToolCall::new(
            id,
            ToolName::Read,
            serde_json::json!({"file_path": "/test.txt"}),
        );
        let uuid = crate::model::EntryUuid::new("entry-1").expect("valid uuid");
        let scroll_state = create_test_scroll_state();

        let lines = render_tool_use(&tool_call, &uuid, &scroll_state, get_test_role_style(), 10, 3);

        // Tool name should be visible in the output
        let text: String = lines.iter().map(|l| l.to_string()).collect();
        assert!(
            text.contains("Read"),
            "Tool name 'Read' should be visible in rendered output"
        );
        // Should not show collapse indicator for short content
        assert!(
            !text.contains("more lines"),
            "Should not show collapse indicator for short content"
        );
    }

    #[test]
    fn render_tool_use_with_long_input_collapsed_shows_summary() {
        let id = ToolUseId::new("tool-456").expect("valid id");
        // Create long JSON input with many fields (12+ to ensure >10 lines when pretty-printed)
        let long_input = serde_json::json!({
            "field1": "value1",
            "field2": "value2",
            "field3": "value3",
            "field4": "value4",
            "field5": "value5",
            "field6": "value6",
            "field7": "value7",
            "field8": "value8",
            "field9": "value9",
            "field10": "value10",
            "field11": "value11",
            "field12": "value12",
        });
        let tool_call = ToolCall::new(id, ToolName::Bash, long_input);
        let uuid = crate::model::EntryUuid::new("entry-2").expect("valid uuid");
        let scroll_state = create_test_scroll_state(); // NOT expanded

        let lines = render_tool_use(&tool_call, &uuid, &scroll_state, get_test_role_style(), 10, 3);

        // Should show collapse indicator
        let text: String = lines.iter().map(|l| l.to_string()).collect();
        assert!(
            text.contains("more lines") || text.contains("+"),
            "Should show collapse indicator for long content: {}",
            text
        );
        // Should have limited lines (header + 3 summary + 1 indicator)
        assert!(
            lines.len() <= 5,
            "Collapsed content should have at most 5 lines, got {}",
            lines.len()
        );
    }

    #[test]
    fn render_tool_use_with_long_input_expanded_shows_all() {
        let id = ToolUseId::new("tool-789").expect("valid id");
        // Create long JSON input with many fields (12+ to ensure >10 lines when pretty-printed)
        let long_input = serde_json::json!({
            "field1": "value1",
            "field2": "value2",
            "field3": "value3",
            "field4": "value4",
            "field5": "value5",
            "field6": "value6",
            "field7": "value7",
            "field8": "value8",
            "field9": "value9",
            "field10": "value10",
            "field11": "value11",
            "field12": "value12",
        });
        let tool_call = ToolCall::new(id, ToolName::Bash, long_input);
        let uuid = crate::model::EntryUuid::new("entry-3").expect("valid uuid");
        let scroll_state = create_expanded_scroll_state(&uuid); // IS expanded

        let lines = render_tool_use(&tool_call, &uuid, &scroll_state, get_test_role_style(), 10, 3);

        // Should NOT show collapse indicator when expanded
        let text: String = lines.iter().map(|l| l.to_string()).collect();
        assert!(
            !text.contains("more lines"),
            "Should not show collapse indicator when expanded"
        );
        // Should have more than 5 lines (header + all JSON lines)
        assert!(
            lines.len() > 5,
            "Expanded content should have more than 5 lines, got {}",
            lines.len()
        );
    }

    // ===== render_tool_result Tests =====

    #[test]
    fn render_tool_result_with_short_content_shows_all_lines() {
        let content = "File contents:\nLine 1\nLine 2";
        let uuid = crate::model::EntryUuid::new("entry-4").expect("valid uuid");
        let scroll_state = create_test_scroll_state();

        let lines = render_tool_result(content, false, &uuid, &scroll_state, get_test_role_style(), 10, 3);

        // Output content should be visible
        let text: String = lines.iter().map(|l| l.to_string()).collect();
        assert!(
            text.contains("Line 1") || text.contains("File contents"),
            "Tool result content should be visible"
        );
        // Should not show collapse indicator for short content
        assert!(
            !text.contains("more lines"),
            "Should not show collapse indicator for short content"
        );
    }

    #[test]
    fn render_tool_result_with_long_content_collapsed_shows_summary() {
        // Create content with more than 10 lines
        let content = "Line 1\nLine 2\nLine 3\nLine 4\nLine 5\n\
                       Line 6\nLine 7\nLine 8\nLine 9\nLine 10\n\
                       Line 11\nLine 12\nLine 13";
        let uuid = crate::model::EntryUuid::new("entry-5").expect("valid uuid");
        let scroll_state = create_test_scroll_state(); // NOT expanded

        let lines = render_tool_result(content, false, &uuid, &scroll_state, get_test_role_style(), 10, 3);

        // Should show first 3 lines + collapse indicator
        let text: String = lines.iter().map(|l| l.to_string()).collect();
        assert!(
            text.contains("Line 1"),
            "Should show first line of content"
        );
        assert!(
            text.contains("more lines") || text.contains("+"),
            "Should show collapse indicator for long content"
        );
        // Should have exactly 4 lines (3 summary + 1 indicator)
        assert_eq!(
            lines.len(),
            4,
            "Collapsed content should have 4 lines (3 summary + indicator), got {}",
            lines.len()
        );
    }

    #[test]
    fn render_tool_result_with_long_content_expanded_shows_all() {
        // Create content with more than 10 lines
        let content = "Line 1\nLine 2\nLine 3\nLine 4\nLine 5\n\
                       Line 6\nLine 7\nLine 8\nLine 9\nLine 10\n\
                       Line 11\nLine 12\nLine 13";
        let uuid = crate::model::EntryUuid::new("entry-6").expect("valid uuid");
        let scroll_state = create_expanded_scroll_state(&uuid); // IS expanded

        let lines = render_tool_result(content, false, &uuid, &scroll_state, get_test_role_style(), 10, 3);

        // Should NOT show collapse indicator when expanded
        let text: String = lines.iter().map(|l| l.to_string()).collect();
        assert!(
            !text.contains("more lines"),
            "Should not show collapse indicator when expanded"
        );
        // Should have all 13 lines
        assert_eq!(
            lines.len(),
            13,
            "Expanded content should have all 13 lines, got {}",
            lines.len()
        );
    }

    #[test]
    fn render_tool_result_applies_error_style_when_is_error_true() {
        let content = "Error: file not found";
        let uuid = crate::model::EntryUuid::new("entry-7").expect("valid uuid");
        let scroll_state = create_test_scroll_state();

        // For errors, pass red style
        let error_style = Style::default().fg(Color::Red);
        let lines = render_tool_result(content, true, &uuid, &scroll_state, error_style, 10, 3);

        // Error results should have red styling
        let has_red_style = lines.iter().any(|line| {
            line.spans
                .iter()
                .any(|span| span.style.fg == Some(Color::Red))
        });

        assert!(
            has_red_style,
            "Error tool results should be styled with red color"
        );
    }

    #[test]
    fn render_tool_result_does_not_apply_error_style_when_is_error_false() {
        let content = "Success output";
        let uuid = crate::model::EntryUuid::new("entry-8").expect("valid uuid");
        let scroll_state = create_test_scroll_state();

        let lines = render_tool_result(content, false, &uuid, &scroll_state, get_test_role_style(), 10, 3);

        // Non-error results should not have red styling
        let has_red_style = lines.iter().any(|line| {
            line.spans
                .iter()
                .any(|span| span.style.fg == Some(Color::Red))
        });

        assert!(
            !has_red_style,
            "Non-error tool results should not be styled with red color"
        );
    }

    // ===== render_content_block Tests =====

    #[test]
    fn render_content_block_handles_tool_use() {
        let id = ToolUseId::new("test-tool").expect("valid id");
        let tool_call = ToolCall::new(id, ToolName::Read, serde_json::json!({"file": "x.txt"}));
        let block = ContentBlock::ToolUse(tool_call);
        let uuid = crate::model::EntryUuid::new("entry-9").expect("valid uuid");
        let scroll_state = create_test_scroll_state();

        let lines = render_content_block(&block, &uuid, &scroll_state, &create_test_styles(), get_test_role_style(), 10, 3);

        // Should render tool name
        let text: String = lines.iter().map(|l| l.to_string()).collect();
        assert!(
            text.contains("Read"),
            "Should render ToolUse block with tool name visible"
        );
    }

    #[test]
    fn render_content_block_handles_tool_result() {
        let id = ToolUseId::new("result-123").expect("valid id");
        let block = ContentBlock::ToolResult {
            tool_use_id: id,
            content: "Output data".to_string(),
            is_error: false,
        };
        let uuid = crate::model::EntryUuid::new("entry-10").expect("valid uuid");
        let scroll_state = create_test_scroll_state();

        let lines = render_content_block(&block, &uuid, &scroll_state, &create_test_styles(), get_test_role_style(), 10, 3);

        // Should render result content
        let text: String = lines.iter().map(|l| l.to_string()).collect();
        assert!(
            text.contains("Output data"),
            "Should render ToolResult block with content visible"
        );
    }

    #[test]
    fn render_content_block_handles_text() {
        let block = ContentBlock::Text {
            text: "Plain text message".to_string(),
        };
        let uuid = crate::model::EntryUuid::new("entry-11").expect("valid uuid");
        let scroll_state = create_test_scroll_state();

        let lines = render_content_block(&block, &uuid, &scroll_state, &create_test_styles(), get_test_role_style(), 10, 3);

        // Should render text content
        let text: String = lines.iter().map(|l| l.to_string()).collect();
        assert!(
            text.contains("Plain text message"),
            "Should render Text block with text visible"
        );
    }

    #[test]
    fn render_content_block_handles_thinking() {
        let block = ContentBlock::Thinking {
            thinking: "Analyzing the problem...".to_string(),
        };
        let uuid = crate::model::EntryUuid::new("entry-12").expect("valid uuid");
        let scroll_state = create_test_scroll_state();

        let lines = render_content_block(&block, &uuid, &scroll_state, &create_test_styles(), get_test_role_style(), 10, 3);

        // Should render thinking content
        let text: String = lines.iter().map(|l| l.to_string()).collect();
        assert!(
            text.contains("Analyzing"),
            "Should render Thinking block with content visible"
        );
    }

    // ===== ContentBlock::Text collapse/expand tests =====

    #[test]
    fn render_content_block_text_with_short_content_shows_all_lines() {
        let short_text = "Line 1\nLine 2\nLine 3";
        let block = ContentBlock::Text {
            text: short_text.to_string(),
        };
        let uuid = crate::model::EntryUuid::new("entry-13").expect("valid uuid");
        let scroll_state = create_test_scroll_state();

        let lines = render_content_block(&block, &uuid, &scroll_state, &create_test_styles(), get_test_role_style(), 10, 3);

        // Should show all lines for short content
        assert_eq!(
            lines.len(),
            3,
            "Short text (3 lines) should show all lines, got {} lines",
            lines.len()
        );
        let text: String = lines.iter().map(|l| l.to_string()).collect();
        assert!(text.contains("Line 1"), "Should contain Line 1");
        assert!(text.contains("Line 2"), "Should contain Line 2");
        assert!(text.contains("Line 3"), "Should contain Line 3");
    }

    #[test]
    fn render_content_block_text_with_long_content_collapsed_shows_summary() {
        // Create text with 15 lines (exceeds threshold of 10)
        let long_text = "Line 1\nLine 2\nLine 3\nLine 4\nLine 5\n\
                        Line 6\nLine 7\nLine 8\nLine 9\nLine 10\n\
                        Line 11\nLine 12\nLine 13\nLine 14\nLine 15";
        let block = ContentBlock::Text {
            text: long_text.to_string(),
        };
        let uuid = crate::model::EntryUuid::new("entry-14").expect("valid uuid");
        let scroll_state = create_test_scroll_state(); // NOT expanded

        let lines = render_content_block(&block, &uuid, &scroll_state, &create_test_styles(), get_test_role_style(), 10, 3);

        // Should show first 3 lines + collapse indicator
        assert_eq!(
            lines.len(),
            4,
            "Collapsed text should show 3 summary lines + 1 indicator, got {} lines",
            lines.len()
        );

        let text: String = lines.iter().map(|l| l.to_string()).collect();
        assert!(text.contains("Line 1"), "Should show Line 1");
        assert!(text.contains("Line 2"), "Should show Line 2");
        assert!(text.contains("Line 3"), "Should show Line 3");
        assert!(
            text.contains("more lines") || text.contains("+12"),
            "Should show collapse indicator with remaining line count"
        );
        assert!(
            !text.contains("Line 15"),
            "Should NOT show last line when collapsed"
        );
    }

    #[test]
    fn render_content_block_text_with_long_content_expanded_shows_all() {
        // Create text with 15 lines (exceeds threshold of 10)
        let long_text = "Line 1\nLine 2\nLine 3\nLine 4\nLine 5\n\
                        Line 6\nLine 7\nLine 8\nLine 9\nLine 10\n\
                        Line 11\nLine 12\nLine 13\nLine 14\nLine 15";
        let block = ContentBlock::Text {
            text: long_text.to_string(),
        };
        let uuid = crate::model::EntryUuid::new("entry-15").expect("valid uuid");
        let scroll_state = create_expanded_scroll_state(&uuid); // IS expanded

        let lines = render_content_block(&block, &uuid, &scroll_state, &create_test_styles(), get_test_role_style(), 10, 3);

        // Should show all 15 lines when expanded
        assert_eq!(
            lines.len(),
            15,
            "Expanded text should show all 15 lines, got {} lines",
            lines.len()
        );

        let text: String = lines.iter().map(|l| l.to_string()).collect();
        assert!(text.contains("Line 1"), "Should show Line 1");
        assert!(text.contains("Line 15"), "Should show Line 15 when expanded");
        assert!(
            !text.contains("more lines"),
            "Should NOT show collapse indicator when expanded"
        );
    }

    #[test]
    fn render_content_block_text_exactly_at_threshold_does_not_collapse() {
        // Create text with exactly 10 lines (threshold)
        let text = "Line 1\nLine 2\nLine 3\nLine 4\nLine 5\n\
                   Line 6\nLine 7\nLine 8\nLine 9\nLine 10";
        let block = ContentBlock::Text {
            text: text.to_string(),
        };
        let uuid = crate::model::EntryUuid::new("entry-16").expect("valid uuid");
        let scroll_state = create_test_scroll_state();

        let lines = render_content_block(&block, &uuid, &scroll_state, &create_test_styles(), get_test_role_style(), 10, 3);

        // Exactly at threshold should NOT collapse (must exceed threshold)
        assert_eq!(
            lines.len(),
            10,
            "Text at threshold (10 lines) should show all lines without collapsing, got {} lines",
            lines.len()
        );

        let text_output: String = lines.iter().map(|l| l.to_string()).collect();
        assert!(
            !text_output.contains("more lines"),
            "Text at threshold should NOT show collapse indicator"
        );
    }

    // ===== render_conversation_view Integration Tests =====

    #[test]
    fn render_conversation_view_renders_text_content_blocks() {
        use crate::model::{
            AgentConversation, EntryMetadata, EntryType, EntryUuid, LogEntry, Message,
            MessageContent, Role, SessionId,
        };
        use chrono::Utc;
        use ratatui::backend::TestBackend;
        use ratatui::Terminal;

        // Create a conversation with a message containing text blocks
        let mut conversation = AgentConversation::new(None);

        let message = Message::new(
            Role::Assistant,
            MessageContent::Blocks(vec![ContentBlock::Text {
                text: "Test message content".to_string(),
            }]),
        );

        let entry = LogEntry::new(
            EntryUuid::new("entry-1").expect("valid uuid"),
            None,
            SessionId::new("session-1").expect("valid session id"),
            None,
            Utc::now(),
            EntryType::Assistant,
            message,
            EntryMetadata::default(),
        );

        conversation.add_entry(entry);

        // Create a test terminal and render
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).expect("Failed to create terminal");

        let scroll_state = ScrollState::default();

        terminal
            .draw(|frame| {
                let area = frame.area();
                render_conversation_view(frame, area, &conversation, &scroll_state, &create_test_styles(), false);
            })
            .expect("Failed to draw");

        // Get the rendered buffer and check it contains our text
        let buffer = terminal.backend().buffer().clone();
        let content: String = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();

        // Should render the text content
        assert!(
            content.contains("Test message content"),
            "Should render text content from message blocks"
        );
    }

    #[test]
    fn render_conversation_view_renders_tool_use_blocks() {
        use crate::model::{
            AgentConversation, EntryMetadata, EntryType, EntryUuid, LogEntry, Message,
            MessageContent, Role, SessionId,
        };
        use chrono::Utc;
        use ratatui::backend::TestBackend;
        use ratatui::Terminal;

        // Create a conversation with a message containing a tool use block
        let mut conversation = AgentConversation::new(None);

        let tool_id = ToolUseId::new("tool-123").expect("valid id");
        let tool_call = ToolCall::new(
            tool_id,
            ToolName::Read,
            serde_json::json!({"file_path": "/test.txt"}),
        );

        let message = Message::new(
            Role::Assistant,
            MessageContent::Blocks(vec![ContentBlock::ToolUse(tool_call)]),
        );

        let entry = LogEntry::new(
            EntryUuid::new("entry-2").expect("valid uuid"),
            None,
            SessionId::new("session-2").expect("valid session id"),
            None,
            Utc::now(),
            EntryType::Assistant,
            message,
            EntryMetadata::default(),
        );

        conversation.add_entry(entry);

        // Create a test terminal and render
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).expect("Failed to create terminal");

        let scroll_state = ScrollState::default();

        terminal
            .draw(|frame| {
                let area = frame.area();
                render_conversation_view(frame, area, &conversation, &scroll_state, &create_test_styles(), false);
            })
            .expect("Failed to draw");

        // Get the rendered buffer and check it contains tool name
        let buffer = terminal.backend().buffer().clone();
        let content: String = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();

        // Should render the tool name
        assert!(
            content.contains("Read"),
            "Should render tool name from ToolUse blocks"
        );
    }

    // ===== ConversationView Widget Tests =====

    #[test]
    fn conversation_view_widget_renders_empty_conversation() {
        use crate::model::AgentConversation;
        use ratatui::backend::TestBackend;
        use ratatui::Terminal;

        let conversation = AgentConversation::new(None);
        let scroll_state = ScrollState::default();

        let styles = create_test_styles();
        let widget = ConversationView::new(&conversation, &scroll_state, &styles, false);

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).expect("Failed to create terminal");

        terminal
            .draw(|frame| {
                let area = frame.area();
                frame.render_widget(widget, area);
            })
            .expect("Failed to draw");

        let buffer = terminal.backend().buffer().clone();
        let content: String = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();

        // Should show "No messages yet..." for empty conversation
        assert!(
            content.contains("No messages yet") || content.contains("messages"),
            "Empty conversation should show placeholder message"
        );
    }

    #[test]
    fn conversation_view_calculate_entry_height_counts_lines_in_collapsed_message() {
        use crate::model::{
            AgentConversation, EntryMetadata, EntryType, EntryUuid, LogEntry, Message,
            MessageContent, Role, SessionId,
        };
        use chrono::Utc;

        let conversation = AgentConversation::new(None);
        let scroll_state = ScrollState::default();
        let styles = create_test_styles();
        let widget = ConversationView::new(&conversation, &scroll_state, &styles, false);

        // Create entry with multi-line text content (15 lines)
        let text = "Line 1\nLine 2\nLine 3\nLine 4\nLine 5\n\
                    Line 6\nLine 7\nLine 8\nLine 9\nLine 10\n\
                    Line 11\nLine 12\nLine 13\nLine 14\nLine 15";
        let message = Message::new(Role::Assistant, MessageContent::Text(text.to_string()));

        let entry = LogEntry::new(
            EntryUuid::new("entry-1").expect("valid uuid"),
            None,
            SessionId::new("session-1").expect("valid session id"),
            None,
            Utc::now(),
            EntryType::Assistant,
            message,
            EntryMetadata::default(),
        );

        let height = widget.calculate_entry_height(&entry);

        // With collapse_threshold=10, summary_lines=3:
        // Should collapse to 3 lines + 1 indicator line = 4 lines
        assert_eq!(
            height, 4,
            "Collapsed entry should show 3 summary lines + 1 indicator"
        );
    }

    #[test]
    fn conversation_view_calculate_entry_height_counts_all_lines_when_expanded() {
        use crate::model::{
            AgentConversation, EntryMetadata, EntryType, EntryUuid, LogEntry, Message,
            MessageContent, Role, SessionId,
        };
        use chrono::Utc;

        let conversation = AgentConversation::new(None);
        let mut scroll_state = ScrollState::default();

        let entry_uuid = EntryUuid::new("entry-1").expect("valid uuid");

        // Expand this entry
        scroll_state.toggle_expand(&entry_uuid);

        let styles = create_test_styles();
        let widget = ConversationView::new(&conversation, &scroll_state, &styles, false);

        // Create entry with multi-line text content (15 lines)
        let text = "Line 1\nLine 2\nLine 3\nLine 4\nLine 5\n\
                    Line 6\nLine 7\nLine 8\nLine 9\nLine 10\n\
                    Line 11\nLine 12\nLine 13\nLine 14\nLine 15";
        let message = Message::new(Role::Assistant, MessageContent::Text(text.to_string()));

        let entry = LogEntry::new(
            entry_uuid,
            None,
            SessionId::new("session-1").expect("valid session id"),
            None,
            Utc::now(),
            EntryType::Assistant,
            message,
            EntryMetadata::default(),
        );

        let height = widget.calculate_entry_height(&entry);

        // When expanded, should show all 15 lines
        assert_eq!(height, 15, "Expanded entry should show all 15 lines");
    }

    #[test]
    fn conversation_view_calculate_visible_range_with_small_viewport() {
        use crate::model::AgentConversation;

        let conversation = AgentConversation::new(None);
        let scroll_state = ScrollState::default();
        let styles = create_test_styles();
        let widget = ConversationView::new(&conversation, &scroll_state, &styles, false);

        // Viewport shows 10 lines, buffer_size=20
        let (start, end) = widget.calculate_visible_range(10);

        // With scroll_offset=0, should render from 0 to min(20 buffer, entry_count)
        assert_eq!(start, 0, "Should start at beginning");
        assert!(end <= 20, "Should not exceed buffer size");
    }

    #[test]
    fn conversation_view_calculate_visible_range_respects_scroll_offset() {
        use crate::model::{
            AgentConversation, EntryMetadata, EntryType, EntryUuid, LogEntry, Message,
            MessageContent, Role, SessionId,
        };
        use chrono::Utc;

        let mut conversation = AgentConversation::new(None);

        // Add 100 single-line entries
        for i in 0..100 {
            let message = Message::new(Role::Assistant, MessageContent::Text(format!("M{}", i)));

            let entry = LogEntry::new(
                EntryUuid::new(format!("entry-{}", i)).expect("valid uuid"),
                None,
                SessionId::new("session-1").expect("valid session id"),
                None,
                Utc::now(),
                EntryType::Assistant,
                message,
                EntryMetadata::default(),
            );

            conversation.add_entry(entry);
        }

        let scroll_state = ScrollState {
            vertical_offset: 50, // Scrolled down by 50 lines
            ..Default::default()
        };

        let styles = create_test_styles();
        let widget = ConversationView::new(&conversation, &scroll_state, &styles, false);

        let (start, end) = widget.calculate_visible_range(10);

        // With scroll_offset=50, buffer=20:
        // Should start rendering before line 50 (accounting for buffer)
        // With single-line entries, should skip some entries before visible range
        assert!(
            start > 0,
            "Should skip entries before visible range when scrolled down"
        );
        assert!(end > start, "End should be after start");
        assert!(
            end <= 100,
            "End should not exceed total entry count"
        );
    }

    #[test]
    fn conversation_view_widget_renders_only_visible_entries() {
        use crate::model::{
            AgentConversation, EntryMetadata, EntryType, EntryUuid, LogEntry, Message,
            MessageContent, Role, SessionId,
        };
        use chrono::Utc;
        use ratatui::backend::TestBackend;
        use ratatui::Terminal;

        let mut conversation = AgentConversation::new(None);

        // Add 100 entries
        for i in 0..100 {
            let message = Message::new(
                Role::Assistant,
                MessageContent::Text(format!("Message {}", i)),
            );

            let entry = LogEntry::new(
                EntryUuid::new(format!("entry-{}", i)).expect("valid uuid"),
                None,
                SessionId::new("session-1").expect("valid session id"),
                None,
                Utc::now(),
                EntryType::Assistant,
                message,
                EntryMetadata::default(),
            );

            conversation.add_entry(entry);
        }

        let scroll_state = ScrollState::default();
        let styles = create_test_styles();
        let widget = ConversationView::new(&conversation, &scroll_state, &styles, false);

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).expect("Failed to create terminal");

        terminal
            .draw(|frame| {
                let area = frame.area();
                frame.render_widget(widget, area);
            })
            .expect("Failed to draw");

        let buffer = terminal.backend().buffer().clone();
        let content: String = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();

        // With virtualization, should render early messages (buffer=20)
        assert!(
            content.contains("Message 0") || content.contains("Message 1"),
            "Should render messages at start of visible range"
        );

        // Should NOT render messages far beyond visible range
        assert!(
            !content.contains("Message 99"),
            "Should NOT render messages far beyond viewport (virtualization working)"
        );
    }

    #[test]
    fn conversation_view_widget_builder_pattern_works() {
        use crate::model::AgentConversation;

        let conversation = AgentConversation::new(None);
        let scroll_state = ScrollState::default();

        let styles = create_test_styles();
        let widget = ConversationView::new(&conversation, &scroll_state, &styles, false)
            .collapse_threshold(15)
            .summary_lines(5)
            .buffer_size(30);

        assert_eq!(
            widget.collapse_threshold, 15,
            "Builder pattern should set collapse_threshold"
        );
        assert_eq!(
            widget.summary_lines, 5,
            "Builder pattern should set summary_lines"
        );
        assert_eq!(
            widget.buffer_size, 30,
            "Builder pattern should set buffer_size"
        );
    }

    // ===== Subagent Initial Prompt Visual Distinction Tests (FR-006, cclv-07v.4.8) =====

    #[test]
    fn conversation_view_subagent_first_message_has_initial_prompt_label() {
        use crate::model::{
            AgentConversation, AgentId, EntryMetadata, EntryType, EntryUuid, LogEntry, Message,
            MessageContent, Role, SessionId,
        };
        use chrono::Utc;
        use ratatui::backend::TestBackend;
        use ratatui::Terminal;

        // Create subagent conversation with 2 messages
        let agent_id = AgentId::new("subagent-123").expect("valid agent id");
        let mut conversation = AgentConversation::new(Some(agent_id));

        // First message (initial prompt from main agent)
        let msg1 = Message::new(
            Role::User,
            MessageContent::Text("Please analyze this file.".to_string()),
        );
        let entry1 = LogEntry::new(
            EntryUuid::new("entry-1").expect("valid uuid"),
            None,
            SessionId::new("session-1").expect("valid session id"),
            None,
            Utc::now(),
            EntryType::User,
            msg1,
            EntryMetadata::default(),
        );
        conversation.add_entry(entry1);

        // Second message (subagent response)
        let msg2 = Message::new(
            Role::Assistant,
            MessageContent::Text("Analyzing file...".to_string()),
        );
        let entry2 = LogEntry::new(
            EntryUuid::new("entry-2").expect("valid uuid"),
            None,
            SessionId::new("session-1").expect("valid session id"),
            None,
            Utc::now(),
            EntryType::Assistant,
            msg2,
            EntryMetadata::default(),
        );
        conversation.add_entry(entry2);

        let scroll_state = ScrollState::default();
        let styles = create_test_styles();

        // Create widget with is_subagent_view=true (will fail until we add this field)
        let widget = ConversationView::new(&conversation, &scroll_state, &styles, false)
            .is_subagent_view(true);

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).expect("Failed to create terminal");

        terminal
            .draw(|frame| {
                let area = frame.area();
                frame.render_widget(widget, area);
            })
            .expect("Failed to draw");

        let buffer = terminal.backend().buffer().clone();
        let content: String = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();

        // FR-006: First message should have "Initial Prompt" label or visual marker
        assert!(
            content.contains("Initial Prompt") || content.contains("🔷"),
            "First message in subagent conversation should have 'Initial Prompt' label or marker"
        );
    }

    #[test]
    fn conversation_view_subagent_second_message_has_no_initial_prompt_label() {
        use crate::model::{
            AgentConversation, AgentId, EntryMetadata, EntryType, EntryUuid, LogEntry, Message,
            MessageContent, Role, SessionId,
        };
        use chrono::Utc;
        use ratatui::backend::TestBackend;
        use ratatui::Terminal;

        // Create subagent conversation with 2 messages
        let agent_id = AgentId::new("subagent-456").expect("valid agent id");
        let mut conversation = AgentConversation::new(Some(agent_id));

        // First message
        let msg1 = Message::new(
            Role::User,
            MessageContent::Text("First message.".to_string()),
        );
        let entry1 = LogEntry::new(
            EntryUuid::new("entry-1").expect("valid uuid"),
            None,
            SessionId::new("session-1").expect("valid session id"),
            None,
            Utc::now(),
            EntryType::User,
            msg1,
            EntryMetadata::default(),
        );
        conversation.add_entry(entry1);

        // Second message (should NOT have initial prompt marker)
        let msg2 = Message::new(
            Role::Assistant,
            MessageContent::Text("Second message.".to_string()),
        );
        let entry2 = LogEntry::new(
            EntryUuid::new("entry-2").expect("valid uuid"),
            None,
            SessionId::new("session-1").expect("valid session id"),
            None,
            Utc::now(),
            EntryType::Assistant,
            msg2,
            EntryMetadata::default(),
        );
        conversation.add_entry(entry2);

        let scroll_state = ScrollState::default();
        let styles = create_test_styles();
        let widget = ConversationView::new(&conversation, &scroll_state, &styles, false)
            .is_subagent_view(true);

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).expect("Failed to create terminal");

        terminal
            .draw(|frame| {
                let area = frame.area();
                frame.render_widget(widget, area);
            })
            .expect("Failed to draw");

        let buffer = terminal.backend().buffer().clone();
        let content: String = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();

        // Should contain second message content
        assert!(
            content.contains("Second message"),
            "Should render second message content"
        );

        // Count occurrences of "Initial Prompt" text (the emoji might not render in TestBackend)
        let initial_prompt_count = content.matches("Initial Prompt").count();

        // Should have exactly ONE initial prompt marker (for first message only)
        assert_eq!(
            initial_prompt_count, 1,
            "Should have exactly one 'Initial Prompt' marker (first message only), found {}",
            initial_prompt_count
        );
    }

    #[test]
    fn conversation_view_main_agent_does_not_show_initial_prompt_label() {
        use crate::model::{
            AgentConversation, EntryMetadata, EntryType, EntryUuid, LogEntry, Message,
            MessageContent, Role, SessionId,
        };
        use chrono::Utc;
        use ratatui::backend::TestBackend;
        use ratatui::Terminal;

        // Create MAIN agent conversation (None agent_id)
        let mut conversation = AgentConversation::new(None);

        // First message in main agent
        let msg = Message::new(
            Role::User,
            MessageContent::Text("User request to main agent.".to_string()),
        );
        let entry = LogEntry::new(
            EntryUuid::new("entry-1").expect("valid uuid"),
            None,
            SessionId::new("session-1").expect("valid session id"),
            None,
            Utc::now(),
            EntryType::User,
            msg,
            EntryMetadata::default(),
        );
        conversation.add_entry(entry);

        let scroll_state = ScrollState::default();
        let styles = create_test_styles();

        // Create widget with is_subagent_view=false (main agent)
        let widget = ConversationView::new(&conversation, &scroll_state, &styles, false)
            .is_subagent_view(false);

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).expect("Failed to create terminal");

        terminal
            .draw(|frame| {
                let area = frame.area();
                frame.render_widget(widget, area);
            })
            .expect("Failed to draw");

        let buffer = terminal.backend().buffer().clone();
        let content: String = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();

        // Main agent should NOT have "Initial Prompt" label
        assert!(
            !content.contains("Initial Prompt") && !content.contains("🔷"),
            "Main agent conversation should NOT have 'Initial Prompt' label"
        );

        // Should still render the message content
        assert!(
            content.contains("User request"),
            "Should render main agent message content"
        );
    }

    #[test]
    fn conversation_view_subagent_initial_prompt_has_distinct_color() {
        use crate::model::{
            AgentConversation, AgentId, EntryMetadata, EntryType, EntryUuid, LogEntry, Message,
            MessageContent, Role, SessionId,
        };
        use chrono::Utc;
        use ratatui::backend::TestBackend;
        use ratatui::style::Color;
        use ratatui::Terminal;

        // Create subagent conversation
        let agent_id = AgentId::new("subagent-789").expect("valid agent id");
        let mut conversation = AgentConversation::new(Some(agent_id));

        // First message (initial prompt)
        let msg = Message::new(
            Role::User,
            MessageContent::Text("Initial prompt.".to_string()),
        );
        let entry = LogEntry::new(
            EntryUuid::new("entry-1").expect("valid uuid"),
            None,
            SessionId::new("session-1").expect("valid session id"),
            None,
            Utc::now(),
            EntryType::User,
            msg,
            EntryMetadata::default(),
        );
        conversation.add_entry(entry);

        let scroll_state = ScrollState::default();
        let styles = create_test_styles();
        let widget = ConversationView::new(&conversation, &scroll_state, &styles, false)
            .is_subagent_view(true);

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).expect("Failed to create terminal");

        terminal
            .draw(|frame| {
                let area = frame.area();
                frame.render_widget(widget, area);
            })
            .expect("Failed to draw");

        let buffer = terminal.backend().buffer().clone();

        // Check for distinct color in the initial prompt marker/label
        // Should have Magenta color for the marker (distinct from User's Cyan)
        let has_magenta = buffer
            .content()
            .iter()
            .any(|cell| cell.fg == Color::Magenta);

        assert!(
            has_magenta,
            "Initial prompt label should have distinct color (Magenta) for visual distinction"
        );
    }

    // ===== render_markdown Tests =====

    #[test]
    fn render_markdown_with_plain_text_returns_unchanged() {
        let text = "This is plain text\nAnother line";
        let lines = render_markdown_with_style(text, Style::default());

        // Plain text should be rendered as-is
        let rendered: String = lines.iter().map(|l| l.to_string()).collect();
        assert!(
            rendered.contains("This is plain text"),
            "Plain text should be preserved in markdown rendering"
        );
        assert!(
            rendered.contains("Another line"),
            "All plain text lines should be rendered"
        );
    }

    #[test]
    fn render_markdown_with_heading_preserves_structure() {
        let markdown = "# Heading 1\n## Heading 2\nPlain text";
        let lines = render_markdown_with_style(markdown, Style::default());

        // Should have lines for headings and plain text
        assert!(
            lines.len() >= 3,
            "Should render at least 3 lines (2 headings + text), got {}",
            lines.len()
        );

        // Verify heading markers are present (tui-markdown includes # prefix)
        let rendered: String = lines.iter().map(|l| l.to_string()).collect();
        assert!(
            rendered.contains("# ") || rendered.contains("Heading 1"),
            "H1 heading content should be visible"
        );
        assert!(
            rendered.contains("## ") || rendered.contains("Heading 2"),
            "H2 heading content should be visible"
        );
        assert!(
            rendered.contains("Plain text"),
            "Plain text should be visible"
        );
    }

    #[test]
    fn render_markdown_with_code_block_preserves_content() {
        let markdown = "Some text\n```rust\nfn main() {}\n```\nMore text";
        let lines = render_markdown_with_style(markdown, Style::default());

        // Code content should be visible
        let rendered: String = lines.iter().map(|l| l.to_string()).collect();
        assert!(
            rendered.contains("fn main") || rendered.contains("main"),
            "Code block content should be preserved and visible"
        );
    }

    #[test]
    fn render_markdown_with_bold_applies_styling() {
        let markdown = "Normal text **bold text** more normal";
        let lines = render_markdown_with_style(markdown, Style::default());

            // Should have bold styling somewhere
        let has_bold = lines.iter().any(|line| {
            line.spans
                .iter()
                .any(|span| span.style.add_modifier.contains(Modifier::BOLD))
        });
        assert!(
            has_bold,
            "Bold markdown (**text**) should apply bold styling"
        );

        // Content should be present
        let rendered: String = lines.iter().map(|l| l.to_string()).collect();
        assert!(
            rendered.contains("bold text") || rendered.contains("text"),
            "Bold text content should be visible"
        );
    }

    #[test]
    fn render_markdown_with_italic_applies_styling() {
        let markdown = "Normal text *italic text* more normal";
        let lines = render_markdown_with_style(markdown, Style::default());

        // Should have italic styling somewhere
        let has_italic = lines.iter().any(|line| {
            line.spans
                .iter()
                .any(|span| span.style.add_modifier.contains(Modifier::ITALIC))
        });
        assert!(
            has_italic,
            "Italic markdown (*text*) should apply italic styling"
        );

        // Content should be present
        let rendered: String = lines.iter().map(|l| l.to_string()).collect();
        assert!(
            rendered.contains("italic text") || rendered.contains("text"),
            "Italic text content should be visible"
        );
    }

    #[test]
    fn render_markdown_with_list_shows_items() {
        let markdown = "List:\n- Item 1\n- Item 2\n- Item 3";
        let lines = render_markdown_with_style(markdown, Style::default());

        // List items should be visible
        let rendered: String = lines.iter().map(|l| l.to_string()).collect();
        assert!(
            rendered.contains("Item 1"),
            "First list item should be visible"
        );
        assert!(
            rendered.contains("Item 2"),
            "Second list item should be visible"
        );
        assert!(
            rendered.contains("Item 3"),
            "Third list item should be visible"
        );
    }

    #[test]
    fn render_markdown_with_link_shows_url() {
        let markdown = "Check [this link](https://example.com) out";
        let lines = render_markdown_with_style(markdown, Style::default());

        // Link text or URL should be visible
        let rendered: String = lines.iter().map(|l| l.to_string()).collect();
        assert!(
            rendered.contains("this link") || rendered.contains("example.com"),
            "Link text or URL should be visible in rendered output"
        );
    }

    #[test]
    fn render_markdown_with_code_block_applies_syntax_highlighting() {
        // FR-022: System MUST apply syntax highlighting to code blocks
        let markdown = "```rust\nfn main() {\n    println!(\"Hello\");\n}\n```";
        let lines = render_markdown_with_style(markdown, Style::default());

        // Should have syntax highlighting (foreground colors) on code content
        let has_syntax_colors = lines.iter().any(|line| {
            line.spans
                .iter()
                .any(|span| span.style.fg.is_some())
        });
        assert!(
            has_syntax_colors,
            "Rust code blocks should have syntax highlighting (foreground colors applied)"
        );

        // Content should be preserved
        let rendered: String = lines.iter().map(|l| l.to_string()).collect();
        assert!(
            rendered.contains("fn") || rendered.contains("main") || rendered.contains("println"),
            "Code block content should be preserved"
        );
    }

    // ===== render_conversation_view Initial Prompt Production Path Test (cclv-07v.4.8) =====

    #[test]
    fn render_conversation_view_function_shows_initial_prompt_for_subagent() {
        use crate::model::{
            AgentConversation, AgentId, EntryMetadata, EntryType, EntryUuid, LogEntry, Message,
            MessageContent, Role, SessionId,
        };
        use chrono::Utc;
        use ratatui::backend::TestBackend;
        use ratatui::Terminal;

        // Create subagent conversation (has agent_id)
        let agent_id = AgentId::new("subagent-prod-test").expect("valid agent id");
        let mut conversation = AgentConversation::new(Some(agent_id));

        // Add first message (initial prompt)
        let msg1 = Message::new(
            Role::User,
            MessageContent::Text("Please analyze this.".to_string()),
        );
        let entry1 = LogEntry::new(
            EntryUuid::new("entry-1").expect("valid uuid"),
            None,
            SessionId::new("session-1").expect("valid session id"),
            None,
            Utc::now(),
            EntryType::User,
            msg1,
            EntryMetadata::default(),
        );
        conversation.add_entry(entry1);

        // Add second message
        let msg2 = Message::new(
            Role::Assistant,
            MessageContent::Text("Analyzing...".to_string()),
        );
        let entry2 = LogEntry::new(
            EntryUuid::new("entry-2").expect("valid uuid"),
            None,
            SessionId::new("session-1").expect("valid session id"),
            None,
            Utc::now(),
            EntryType::Assistant,
            msg2,
            EntryMetadata::default(),
        );
        conversation.add_entry(entry2);

        let scroll_state = ScrollState::default();

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).expect("Failed to create terminal");

        terminal
            .draw(|frame| {
                let area = frame.area();
                // THIS is the production code path used by layout.rs
                render_conversation_view(
                    frame,
                    area,
                    &conversation,
                    &scroll_state,
                    &create_test_styles(),
                    false,
                );
            })
            .expect("Failed to draw");

        let buffer = terminal.backend().buffer().clone();
        let content: String = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();

        // BUG: This test will FAIL because render_conversation_view() doesn't have the logic
        assert!(
            content.contains("Initial Prompt") || content.contains("🔷"),
            "render_conversation_view() MUST show initial prompt label for subagent first message"
        );
    }

    #[test]
    fn render_conversation_view_function_does_not_show_initial_prompt_for_main_agent() {
        use crate::model::{
            AgentConversation, EntryMetadata, EntryType, EntryUuid, LogEntry, Message,
            MessageContent, Role, SessionId,
        };
        use chrono::Utc;
        use ratatui::backend::TestBackend;
        use ratatui::Terminal;

        // Create MAIN agent conversation (no agent_id)
        let mut conversation = AgentConversation::new(None);

        // Add first message
        let msg = Message::new(
            Role::User,
            MessageContent::Text("User message to main agent.".to_string()),
        );
        let entry = LogEntry::new(
            EntryUuid::new("entry-1").expect("valid uuid"),
            None,
            SessionId::new("session-1").expect("valid session id"),
            None,
            Utc::now(),
            EntryType::User,
            msg,
            EntryMetadata::default(),
        );
        conversation.add_entry(entry);

        let scroll_state = ScrollState::default();

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).expect("Failed to create terminal");

        terminal
            .draw(|frame| {
                let area = frame.area();
                render_conversation_view(
                    frame,
                    area,
                    &conversation,
                    &scroll_state,
                    &create_test_styles(),
                    false,
                );
            })
            .expect("Failed to draw");

        let buffer = terminal.backend().buffer().clone();
        let content: String = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();

        // Main agent should NOT have initial prompt label
        assert!(
            !content.contains("Initial Prompt"),
            "Main agent should NOT show initial prompt label"
        );
    }

    // ===== Horizontal Scrolling Tests (FR-039/040, cclv-07v.4.7) =====

    #[test]
    fn conversation_view_applies_horizontal_offset_to_long_lines() {
        use crate::model::{
            AgentConversation, EntryMetadata, EntryType, EntryUuid, LogEntry, Message,
            MessageContent, Role, SessionId,
        };
        use chrono::Utc;
        use ratatui::backend::TestBackend;
        use ratatui::Terminal;

        let mut conversation = AgentConversation::new(None);

        // Create entry with a very long line with non-repeating content
        // Line: "AAAAAAAAAA0123456789012345..." (10 A's, then incrementing digits)
        let long_line = format!("{}{}",
            "A".repeat(10),
            (0..90).map(|i| char::from_digit(i % 10, 10).unwrap()).collect::<String>()
        );
        let message = Message::new(Role::Assistant, MessageContent::Text(long_line));

        let entry = LogEntry::new(
            EntryUuid::new("entry-hscroll-1").expect("valid uuid"),
            None,
            SessionId::new("session-1").expect("valid session id"),
            None,
            Utc::now(),
            EntryType::Assistant,
            message,
            EntryMetadata::default(),
        );

        conversation.add_entry(entry);

        // Create scroll state with horizontal_offset = 10
        let scroll_state = ScrollState {
            vertical_offset: 0,
            horizontal_offset: 10,
            expanded_messages: HashSet::new(),
        };

        let backend = TestBackend::new(80, 10);
        let mut terminal = Terminal::new(backend).expect("Failed to create terminal");

        terminal
            .draw(|frame| {
                let area = frame.area();
                render_conversation_view(
                    frame,
                    area,
                    &conversation,
                    &scroll_state,
                    &create_test_styles(),
                    false,
                );
            })
            .expect("Failed to draw");

        let buffer = terminal.backend().buffer().clone();
        let content: String = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();

        // FR-039: With offset=10, first 10 chars should be skipped
        // Original line: "AAAAAAAAAA012345678901234..." (10 A's at start)
        // After offset=10: "012345678901234..." (A's should be gone)

        // Should NOT show the "AAAA" that would only appear at the start
        assert!(
            !content.contains("AAA"),
            "Should NOT show first 10 chars (AAAA...) after horizontal scroll. Content: {:?}",
            &content[..content.len().min(300)]
        );

        // Should show content starting from position 10 (digits)
        assert!(
            content.contains("01234567"),
            "Should show content starting from position 10 (digits). Content: {:?}",
            &content[..content.len().min(300)]
        );
    }

    #[test]
    fn conversation_view_shows_left_scroll_indicator_when_offset_greater_than_zero() {
        use crate::model::{
            AgentConversation, EntryMetadata, EntryType, EntryUuid, LogEntry, Message,
            MessageContent, Role, SessionId,
        };
        use chrono::Utc;
        use ratatui::backend::TestBackend;
        use ratatui::Terminal;

        let mut conversation = AgentConversation::new(None);

        let long_line = "x".repeat(200);
        let message = Message::new(Role::Assistant, MessageContent::Text(long_line));

        let entry = LogEntry::new(
            EntryUuid::new("entry-hscroll-2").expect("valid uuid"),
            None,
            SessionId::new("session-1").expect("valid session id"),
            None,
            Utc::now(),
            EntryType::Assistant,
            message,
            EntryMetadata::default(),
        );

        conversation.add_entry(entry);

        // Scroll right by 20 chars
        let scroll_state = ScrollState {
            vertical_offset: 0,
            horizontal_offset: 20,
            expanded_messages: HashSet::new(),
        };

        let backend = TestBackend::new(80, 10);
        let mut terminal = Terminal::new(backend).expect("Failed to create terminal");

        terminal
            .draw(|frame| {
                let area = frame.area();
                render_conversation_view(
                    frame,
                    area,
                    &conversation,
                    &scroll_state,
                    &create_test_styles(),
                    false,
                );
            })
            .expect("Failed to draw");

        let buffer = terminal.backend().buffer().clone();
        let content: String = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();

        // FR-040: Should show left arrow indicator (◀) when offset > 0
        assert!(
            content.contains("◀") || content.contains("<"),
            "Should show left scroll indicator when horizontally scrolled right (offset > 0)"
        );
    }

    #[test]
    fn conversation_view_shows_right_scroll_indicator_when_content_extends_beyond_viewport() {
        use crate::model::{
            AgentConversation, EntryMetadata, EntryType, EntryUuid, LogEntry, Message,
            MessageContent, Role, SessionId,
        };
        use chrono::Utc;
        use ratatui::backend::TestBackend;
        use ratatui::Terminal;

        let mut conversation = AgentConversation::new(None);

        // Line much longer than viewport width (80 cols)
        let long_line = "x".repeat(200);
        let message = Message::new(Role::Assistant, MessageContent::Text(long_line));

        let entry = LogEntry::new(
            EntryUuid::new("entry-hscroll-3").expect("valid uuid"),
            None,
            SessionId::new("session-1").expect("valid session id"),
            None,
            Utc::now(),
            EntryType::Assistant,
            message,
            EntryMetadata::default(),
        );

        conversation.add_entry(entry);

        // No horizontal scroll (offset = 0), but line extends beyond viewport
        let scroll_state = ScrollState::default();

        let backend = TestBackend::new(80, 10);
        let mut terminal = Terminal::new(backend).expect("Failed to create terminal");

        terminal
            .draw(|frame| {
                let area = frame.area();
                render_conversation_view(
                    frame,
                    area,
                    &conversation,
                    &scroll_state,
                    &create_test_styles(),
                    false,
                );
            })
            .expect("Failed to draw");

        let buffer = terminal.backend().buffer().clone();
        let content: String = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();

        // FR-040: Should show right arrow indicator (▶) when content extends beyond visible area
        assert!(
            content.contains("▶") || content.contains(">"),
            "Should show right scroll indicator when long line extends beyond viewport"
        );
    }

    #[test]
    fn conversation_view_no_scroll_indicators_for_short_lines() {
        use crate::model::{
            AgentConversation, EntryMetadata, EntryType, EntryUuid, LogEntry, Message,
            MessageContent, Role, SessionId,
        };
        use chrono::Utc;
        use ratatui::backend::TestBackend;
        use ratatui::Terminal;

        let mut conversation = AgentConversation::new(None);

        // Short line that fits in viewport
        let short_line = "Short message";
        let message = Message::new(Role::Assistant, MessageContent::Text(short_line.to_string()));

        let entry = LogEntry::new(
            EntryUuid::new("entry-hscroll-4").expect("valid uuid"),
            None,
            SessionId::new("session-1").expect("valid session id"),
            None,
            Utc::now(),
            EntryType::Assistant,
            message,
            EntryMetadata::default(),
        );

        conversation.add_entry(entry);

        let scroll_state = ScrollState::default();

        let backend = TestBackend::new(80, 10);
        let mut terminal = Terminal::new(backend).expect("Failed to create terminal");

        terminal
            .draw(|frame| {
                let area = frame.area();
                render_conversation_view(
                    frame,
                    area,
                    &conversation,
                    &scroll_state,
                    &create_test_styles(),
                    false,
                );
            })
            .expect("Failed to draw");

        let buffer = terminal.backend().buffer().clone();
        let content: String = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();

        // Should NOT show scroll indicators for short lines
        assert!(
            !content.contains("◀") && !content.contains("▶"),
            "Should NOT show scroll indicators for short lines that fit in viewport"
        );
    }

    // ===== render_conversation_view MessageContent::Text collapse tests (FR-031/032/033) =====

    #[test]
    fn render_conversation_view_collapses_long_messagecontent_text() {
        use crate::model::{
            AgentConversation, EntryMetadata, EntryType, EntryUuid, LogEntry, Message,
            MessageContent, Role, SessionId,
        };
        use chrono::Utc;
        use ratatui::backend::TestBackend;
        use ratatui::Terminal;

        let mut conversation = AgentConversation::new(None);

        // Create entry with long MessageContent::Text (15 lines, exceeds threshold of 10)
        let long_text = "Line 1\nLine 2\nLine 3\nLine 4\nLine 5\n\
                        Line 6\nLine 7\nLine 8\nLine 9\nLine 10\n\
                        Line 11\nLine 12\nLine 13\nLine 14\nLine 15";
        let message = Message::new(Role::Assistant, MessageContent::Text(long_text.to_string()));

        let entry = LogEntry::new(
            EntryUuid::new("entry-collapse-test").expect("valid uuid"),
            None,
            SessionId::new("session-1").expect("valid session id"),
            None,
            Utc::now(),
            EntryType::Assistant,
            message,
            EntryMetadata::default(),
        );

        conversation.add_entry(entry);

        let scroll_state = ScrollState::default(); // NOT expanded

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).expect("Failed to create terminal");

        terminal
            .draw(|frame| {
                let area = frame.area();
                render_conversation_view(frame, area, &conversation, &scroll_state, &create_test_styles(), false);
            })
            .expect("Failed to draw");

        let buffer = terminal.backend().buffer().clone();
        let content: String = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();

        // FR-031: Should show first 3 lines + collapse indicator
        assert!(
            content.contains("Line 1"),
            "Should show first line when collapsed"
        );
        assert!(
            content.contains("Line 2"),
            "Should show second line when collapsed"
        );
        assert!(
            content.contains("Line 3"),
            "Should show third line when collapsed"
        );
        assert!(
            content.contains("more lines") || content.contains("+"),
            "FR-031: Should show collapse indicator for long MessageContent::Text"
        );
        assert!(
            !content.contains("Line 15"),
            "Should NOT show last line when collapsed"
        );
    }

    #[test]
    fn render_conversation_view_expands_long_messagecontent_text_when_toggled() {
        use crate::model::{
            AgentConversation, EntryMetadata, EntryType, EntryUuid, LogEntry, Message,
            MessageContent, Role, SessionId,
        };
        use chrono::Utc;
        use ratatui::backend::TestBackend;
        use ratatui::Terminal;

        let mut conversation = AgentConversation::new(None);

        // Create entry with long MessageContent::Text (15 lines)
        let long_text = "Line 1\nLine 2\nLine 3\nLine 4\nLine 5\n\
                        Line 6\nLine 7\nLine 8\nLine 9\nLine 10\n\
                        Line 11\nLine 12\nLine 13\nLine 14\nLine 15";
        let message = Message::new(Role::Assistant, MessageContent::Text(long_text.to_string()));

        let entry_uuid = EntryUuid::new("entry-expand-test").expect("valid uuid");

        let entry = LogEntry::new(
            entry_uuid.clone(),
            None,
            SessionId::new("session-1").expect("valid session id"),
            None,
            Utc::now(),
            EntryType::Assistant,
            message,
            EntryMetadata::default(),
        );

        conversation.add_entry(entry);

        // Toggle expansion for this entry
        let mut scroll_state = ScrollState::default();
        scroll_state.toggle_expand(&entry_uuid);

        let backend = TestBackend::new(80, 30);
        let mut terminal = Terminal::new(backend).expect("Failed to create terminal");

        terminal
            .draw(|frame| {
                let area = frame.area();
                render_conversation_view(frame, area, &conversation, &scroll_state, &create_test_styles(), false);
            })
            .expect("Failed to draw");

        let buffer = terminal.backend().buffer().clone();
        let content: String = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();

        // FR-032: Should show all lines when expanded
        assert!(
            content.contains("Line 1"),
            "Should show first line when expanded"
        );
        assert!(
            content.contains("Line 15"),
            "FR-032: Should show last line when expanded"
        );
        assert!(
            !content.contains("more lines"),
            "Should NOT show collapse indicator when expanded"
        );
    }

    #[test]
    fn render_conversation_view_does_not_collapse_short_messagecontent_text() {
        use crate::model::{
            AgentConversation, EntryMetadata, EntryType, EntryUuid, LogEntry, Message,
            MessageContent, Role, SessionId,
        };
        use chrono::Utc;
        use ratatui::backend::TestBackend;
        use ratatui::Terminal;

        let mut conversation = AgentConversation::new(None);

        // Create entry with short MessageContent::Text (5 lines, below threshold)
        let short_text = "Line 1\nLine 2\nLine 3\nLine 4\nLine 5";
        let message = Message::new(Role::Assistant, MessageContent::Text(short_text.to_string()));

        let entry = LogEntry::new(
            EntryUuid::new("entry-short-test").expect("valid uuid"),
            None,
            SessionId::new("session-1").expect("valid session id"),
            None,
            Utc::now(),
            EntryType::Assistant,
            message,
            EntryMetadata::default(),
        );

        conversation.add_entry(entry);

        let scroll_state = ScrollState::default();

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).expect("Failed to create terminal");

        terminal
            .draw(|frame| {
                let area = frame.area();
                render_conversation_view(frame, area, &conversation, &scroll_state, &create_test_styles(), false);
            })
            .expect("Failed to draw");

        let buffer = terminal.backend().buffer().clone();
        let content: String = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();

        // Should show all lines for short content (no collapse)
        assert!(content.contains("Line 1"), "Should show Line 1");
        assert!(content.contains("Line 5"), "Should show Line 5");
        assert!(
            !content.contains("more lines"),
            "Should NOT show collapse indicator for short content"
        );
    }
}
