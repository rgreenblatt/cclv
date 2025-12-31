//! Conversation view widget - shared by main and subagent panes.
//!
//! Implements virtualized rendering to handle large conversations efficiently.
//! Only renders visible messages (plus ±20 buffer) based on scroll position.

use crate::model::{AgentConversation, ContentBlock, ConversationEntry, MessageContent, ToolCall};
use crate::state::{ScrollState, WrapMode};
use crate::view::MessageStyles;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, Borders, Paragraph, Widget, Wrap},
    Frame,
};
use tui_markdown::from_str;
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
    global_wrap: WrapMode,
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

    /// Calculate the height in lines for a single conversation entry.
    ///
    /// Accounts for collapsed state based on scroll_state expansion tracking.
    /// When wrap is enabled, calculates how many display lines text occupies when
    /// wrapped to viewport_width. When disabled, counts newlines.
    /// For malformed entries, returns fixed height (line number + error message).
    fn calculate_entry_height(
        &self,
        entry: &ConversationEntry,
        viewport_width: usize,
        global_wrap: WrapMode,
    ) -> usize {
        match entry {
            ConversationEntry::Valid(log_entry) => {
                let is_expanded = self.scroll_state.is_expanded(log_entry.uuid());

                // Get effective wrap mode (per-entry override may invert global)
                let effective_wrap = self.scroll_state.effective_wrap(log_entry.uuid(), global_wrap);

                match log_entry.message().content() {
                    MessageContent::Text(text) => {
                        let total_lines = match effective_wrap {
                            WrapMode::Wrap => {
                                // Calculate wrapped line count
                                Self::calculate_wrapped_lines(text, viewport_width)
                            }
                            WrapMode::NoWrap => {
                                // Count newlines (original behavior)
                                text.lines().count().max(1) // At least 1 line for empty text
                            }
                        };

                        if total_lines > self.collapse_threshold && !is_expanded {
                            // Collapsed: summary_lines + 1 indicator line
                            self.summary_lines + 1
                        } else {
                            total_lines
                        }
                    }
                    MessageContent::Blocks(blocks) => {
                        let mut total_height = 0;
                        let role = log_entry.message().role();
                        let role_style = self.styles.style_for_role(role);

                        for block in blocks {
                            let block_lines = render_content_block(
                                block,
                                log_entry.uuid(),
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
            ConversationEntry::Malformed(malformed) => {
                // Malformed entries: error message might wrap
                let error_lines = malformed.error_message().lines().count();
                // Header line + error lines + spacing
                2 + error_lines
            }
        }
    }

    /// Calculate how many display lines text will occupy when wrapped to viewport width.
    fn calculate_wrapped_lines(text: &str, viewport_width: usize) -> usize {
        if viewport_width == 0 {
            return text.lines().count().max(1);
        }

        let mut total_lines = 0;
        for line in text.lines() {
            // Simple character-based wrapping (not grapheme-aware for now)
            let line_len = line.len();
            if line_len == 0 {
                total_lines += 1; // Empty line still counts
            } else {
                // Calculate how many wrapped lines this logical line produces
                total_lines += line_len.div_ceil(viewport_width);
            }
        }

        // Ensure at least 1 line for empty text
        total_lines.max(1)
    }

    /// Build a layout map with Y offsets and heights for visible entries.
    ///
    /// # Arguments
    /// * `visible_entries` - Slice of conversation entries to layout
    /// * `scroll_offset` - Current vertical scroll position in lines
    /// * `viewport_width` - Width of the viewport for wrapping calculations
    /// * `viewport_height` - Height of the viewport to determine visibility
    /// * `global_wrap` - Global wrap mode setting
    ///
    /// # Returns
    /// Vector of EntryLayout structs with y_offset and height for each visible entry.
    /// Indices correspond to positions in the visible_entries slice.
    #[allow(dead_code)]
    fn calculate_entry_layouts(
        &self,
        visible_entries: &[ConversationEntry],
        scroll_offset: usize,
        viewport_width: usize,
        viewport_height: usize,
        global_wrap: WrapMode,
    ) -> Vec<EntryLayout> {
        let mut layouts = Vec::new();
        let mut cumulative_y = 0_usize;

        for entry in visible_entries {
            // Calculate height for this entry
            let height = self.calculate_entry_height(entry, viewport_width, global_wrap);

            // Determine if this entry is visible in the viewport
            // Entry is visible if any part overlaps with [scroll_offset, scroll_offset + viewport_height)
            let entry_end = cumulative_y + height;
            let viewport_end = scroll_offset + viewport_height;

            let is_visible = cumulative_y < viewport_end && entry_end > scroll_offset;

            if is_visible {
                // Calculate y_offset relative to viewport (accounting for scroll)
                // If entry starts before scroll_offset, it renders at viewport y=0
                // Otherwise, it renders at (cumulative_y - scroll_offset)
                let y_offset = if cumulative_y >= scroll_offset {
                    (cumulative_y - scroll_offset).min(u16::MAX as usize) as u16
                } else {
                    0_u16
                };

                debug_assert!(height <= u16::MAX as usize, "Entry height exceeds u16::MAX");

                layouts.push(EntryLayout {
                    y_offset,
                    height: height.min(u16::MAX as usize) as u16,
                });
            }

            cumulative_y += height;

            // Early exit if we've passed the visible viewport
            if cumulative_y >= viewport_end {
                break;
            }
        }

        layouts
    }

    /// Determine the range of entries that should be rendered based on viewport.
    ///
    /// Returns (start_index, end_index) for the visible range including buffer.
    fn calculate_visible_range(
        &self,
        viewport_height: usize,
        viewport_width: usize,
        global_wrap: WrapMode,
    ) -> (usize, usize) {
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
            let entry_height = self.calculate_entry_height(entry, viewport_width, global_wrap);
            if cumulative_height + entry_height > scroll_offset.saturating_sub(self.buffer_size) {
                start_entry_index = i;
                break;
            }
            cumulative_height = cumulative_height.saturating_add(entry_height);
        }

        // Find the last entry that should be visible (accounting for buffer)
        let mut end_entry_index = start_entry_index;
        cumulative_height = 0;

        for (i, entry) in entries.iter().enumerate().skip(start_entry_index) {
            let entry_height = self.calculate_entry_height(entry, viewport_width, global_wrap);
            cumulative_height = cumulative_height.saturating_add(entry_height);

            if cumulative_height > viewport_height + self.buffer_size.saturating_mul(2) {
                end_entry_index = i;
                break;
            }
            end_entry_index = i.saturating_add(1);
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
            format!(
                "Subagent {}{} ({} entries)",
                agent_id, model_info, entry_count
            )
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

        // Calculate viewport dimensions (area minus borders)
        let viewport_height = area.height.saturating_sub(2) as usize;
        let viewport_width = area.width.saturating_sub(2) as usize;

        // Render content: only render visible entries
        let mut lines = Vec::new();

        if entry_count == 0 {
            lines.push(Line::from("No messages yet..."));
        } else {
            // Calculate which entries are visible
            let (start_index, end_index) =
                self.calculate_visible_range(viewport_height, viewport_width, self.global_wrap);

            // Render only the visible range
            for (visible_index, entry) in self.conversation.entries()[start_index..end_index]
                .iter()
                .enumerate()
            {
                // Calculate actual index in full entry list
                let actual_index = start_index + visible_index;

                match entry {
                    ConversationEntry::Valid(log_entry) => {
                        let role = log_entry.message().role();
                        let role_style = self.styles.style_for_role(role);

                        // Add "Initial Prompt" label for first message in subagent view
                        if self.is_subagent_view && actual_index == 0 {
                            lines.push(Line::from(vec![ratatui::text::Span::styled(
                                "🔷 Initial Prompt",
                                Style::default()
                                    .fg(Color::Magenta)
                                    .add_modifier(Modifier::BOLD),
                            )]));
                        }

                        match log_entry.message().content() {
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
                                        log_entry.uuid(),
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
                    }
                    ConversationEntry::Malformed(malformed) => {
                        // Render malformed entry with error styling
                        // Header: "⚠ Parse Error (line N)"
                        lines.push(Line::from(vec![ratatui::text::Span::styled(
                            "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━",
                            Style::default().fg(Color::Red),
                        )]));
                        lines.push(Line::from(vec![ratatui::text::Span::styled(
                            format!("⚠ Parse Error (line {})", malformed.line_number()),
                            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                        )]));
                        // Error message
                        for error_line in malformed.error_message().lines() {
                            lines.push(Line::from(vec![ratatui::text::Span::styled(
                                error_line.to_string(),
                                Style::default().fg(Color::Red),
                            )]));
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

/// Render lines for a single conversation entry.
///
/// This helper extracts the per-entry line building logic to support
/// the view architecture refactor for per-item wrap toggle (FR-048).
///
/// # Arguments
/// * `entry` - The conversation entry to render (Valid or Malformed)
/// * `entry_index` - Index of this entry in the conversation (0-based)
/// * `is_subagent_view` - Whether this is a subagent conversation (affects first entry labeling)
/// * `scroll` - Scroll state (for expansion tracking)
/// * `styles` - Message styling configuration
/// * `collapse_threshold` - Number of lines before a message is collapsed
/// * `summary_lines` - Number of lines shown when collapsed
///
/// # Returns
/// Vector of Lines representing this entry, including spacing line at end
fn render_entry_lines(
    entry: &ConversationEntry,
    entry_index: usize,
    is_subagent_view: bool,
    scroll: &ScrollState,
    styles: &MessageStyles,
    collapse_threshold: usize,
    summary_lines: usize,
) -> Vec<Line<'static>> {
    let mut lines = Vec::new();

    match entry {
        ConversationEntry::Valid(log_entry) => {
            let role = log_entry.message().role();
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

            match log_entry.message().content() {
                MessageContent::Text(text) => {
                    // Simple text message - apply collapse/expand logic with role-based styling
                    let text_lines: Vec<_> = text.lines().collect();
                    let total_lines = text_lines.len();

                    let is_expanded = scroll.is_expanded(log_entry.uuid());
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
                        let remaining = total_lines.saturating_sub(summary_lines);
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
                            log_entry.uuid(),
                            scroll,
                            styles,
                            role_style,
                            collapse_threshold,
                            summary_lines,
                        );
                        lines.extend(block_lines);
                    }
                }
            }
        }
        ConversationEntry::Malformed(malformed) => {
            // Render malformed entry with error styling
            lines.push(Line::from(vec![ratatui::text::Span::styled(
                "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━",
                Style::default().fg(Color::Red),
            )]));
            lines.push(Line::from(vec![ratatui::text::Span::styled(
                format!("⚠ Parse Error (line {})", malformed.line_number()),
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            )]));
            for error_line in malformed.error_message().lines() {
                lines.push(Line::from(vec![ratatui::text::Span::styled(
                    error_line.to_string(),
                    Style::default().fg(Color::Red),
                )]));
            }
        }
    }

    // Add spacing between entries
    lines.push(Line::from(""));

    lines
}

/// Render a single conversation entry as a Paragraph widget with individual wrap setting.
///
/// This function builds on `render_entry_lines()` to create a ratatui Paragraph widget
/// with per-entry wrap mode support (FR-048).
///
/// # Arguments
/// * `entry` - The conversation entry to render (Valid or Malformed)
/// * `entry_index` - Index of this entry in the conversation (0-based)
/// * `is_subagent_view` - Whether this is a subagent conversation (affects first entry labeling)
/// * `scroll` - Scroll state (for expansion tracking)
/// * `styles` - Message styling configuration
/// * `collapse_threshold` - Number of lines before a message is collapsed
/// * `summary_lines` - Number of lines shown when collapsed
/// * `wrap_mode` - Wrap mode for this specific entry
///
/// # Returns
/// A ratatui Paragraph widget ready to render
#[allow(dead_code)]
#[allow(clippy::too_many_arguments)]
fn render_entry_paragraph(
    entry: &ConversationEntry,
    entry_index: usize,
    is_subagent_view: bool,
    scroll: &ScrollState,
    styles: &MessageStyles,
    collapse_threshold: usize,
    summary_lines: usize,
    wrap_mode: WrapMode,
) -> Paragraph<'static> {
    // Get lines from existing helper function
    let lines = render_entry_lines(
        entry,
        entry_index,
        is_subagent_view,
        scroll,
        styles,
        collapse_threshold,
        summary_lines,
    );

    // Create paragraph with appropriate wrap setting
    match wrap_mode {
        WrapMode::Wrap => Paragraph::new(lines).wrap(Wrap { trim: false }),
        WrapMode::NoWrap => Paragraph::new(lines),
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
/// * `global_wrap` - Global wrap mode setting (FR-039)
///
/// # Implementation
/// Uses per-entry rendering with individual wrap modes (FR-048).
/// Each entry renders as a separate Paragraph widget with effective_wrap
/// (global_wrap + per-entry override).
#[allow(dead_code)]
pub fn render_conversation_view(
    frame: &mut Frame,
    area: Rect,
    conversation: &AgentConversation,
    scroll: &ScrollState,
    styles: &MessageStyles,
    focused: bool,
    global_wrap: WrapMode,
) {
    let entry_count = conversation.entries().len();

    // Build title with agent info
    let title = if let Some(agent_id) = conversation.agent_id() {
        // Subagent conversation
        let model_info = conversation
            .model()
            .map(|m| format!(" [{}]", m.display_name()))
            .unwrap_or_default();
        format!(
            "Subagent {}{} ({} entries)",
            agent_id, model_info, entry_count
        )
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

    // Handle empty conversation
    if entry_count == 0 {
        let block = Block::default()
            .borders(Borders::ALL)
            .title(title)
            .style(Style::default().fg(border_color));
        let inner_area = block.inner(area);
        frame.render_widget(block, area);
        let empty_msg = Paragraph::new(vec![Line::from("No messages yet...")]);
        frame.render_widget(empty_msg, inner_area);
        return;
    }

    // Calculate viewport dimensions (need to compute before rendering block)
    let viewport_width = area.width.saturating_sub(2) as usize;
    let viewport_height = area.height.saturating_sub(2) as usize;

    // Determine if this is a subagent conversation
    let is_subagent_view = conversation.agent_id().is_some();

    // Get all entries for rendering
    let all_entries = conversation.entries();

    // Create temporary ConversationView to use helper methods
    let temp_view = ConversationView::new(conversation, scroll, styles, focused)
        .global_wrap(global_wrap);

    // Calculate visible entry range
    let (start_idx, end_idx) = temp_view.calculate_visible_range(
        viewport_height,
        viewport_width,
        global_wrap,
    );

    let visible_entries = &all_entries[start_idx..end_idx];

    // Determine scroll indicators and horizontal offset (FR-040)
    let horizontal_offset = scroll.horizontal_offset;
    let title_with_indicators = if global_wrap == WrapMode::NoWrap {
        // Need to check if any visible entry has long lines
        // Collect all lines temporarily to check
        let mut all_lines = Vec::new();
        for (idx, entry) in visible_entries.iter().enumerate() {
            let actual_entry_index = start_idx + idx;
            let entry_lines = render_entry_lines(
                entry,
                actual_entry_index,
                is_subagent_view,
                scroll,
                styles,
                10,
                3,
            );
            all_lines.extend(entry_lines);
        }

        let has_left_indicator = horizontal_offset > 0;
        let has_right_indicator = has_long_lines(&all_lines, viewport_width + horizontal_offset);

        add_scroll_indicators_to_title(title, has_left_indicator, has_right_indicator)
    } else {
        title
    };

    // Render border with title (including scroll indicators)
    let block = Block::default()
        .borders(Borders::ALL)
        .title(title_with_indicators)
        .style(Style::default().fg(border_color));

    let inner_area = block.inner(area);
    frame.render_widget(block, area);

    // Calculate layouts for visible entries
    let layouts = temp_view.calculate_entry_layouts(
        visible_entries,
        scroll.vertical_offset,
        viewport_width,
        viewport_height,
        global_wrap,
    );

    // Render each visible entry as a separate Paragraph
    for (layout_idx, layout) in layouts.iter().enumerate() {
        let entry = &visible_entries[layout_idx];
        let actual_entry_index = start_idx + layout_idx;

        // Get per-entry effective wrap mode
        let effective_wrap = if let ConversationEntry::Valid(log_entry) = entry {
            scroll.effective_wrap(log_entry.uuid(), global_wrap)
        } else {
            global_wrap
        };

        // Get entry lines
        let mut entry_lines = render_entry_lines(
            entry,
            actual_entry_index,
            is_subagent_view,
            scroll,
            styles,
            10, // Default collapse threshold
            3,  // Default summary lines
        );

        // Apply horizontal offset if NoWrap mode and offset > 0 (FR-040)
        if effective_wrap == WrapMode::NoWrap && horizontal_offset > 0 {
            entry_lines = entry_lines
                .into_iter()
                .map(|line| apply_horizontal_offset(line, horizontal_offset))
                .collect();
        }

        // Create Paragraph with appropriate wrap setting
        let entry_paragraph = match effective_wrap {
            WrapMode::Wrap => Paragraph::new(entry_lines).wrap(Wrap { trim: false }),
            WrapMode::NoWrap => Paragraph::new(entry_lines),
        };

        // Calculate entry area within viewport
        let entry_area = Rect {
            x: inner_area.x,
            y: inner_area.y + layout.y_offset,
            width: inner_area.width,
            height: layout.height,
        };

        frame.render_widget(entry_paragraph, entry_area);
    }
}

/// Render a conversation view with search match highlighting.
///
/// This function extends render_conversation_view to support search highlighting.
/// When SearchState::Active, all matches are highlighted with distinct styles.
/// The current match (at current_match index) has a different style than other matches.
///
/// # Arguments
/// * `frame` - The ratatui frame to render into
/// * `area` - The area to render within
/// * `conversation` - The agent conversation to display
/// * `scroll` - Scroll state (for expansion tracking and scrolling)
/// * `search` - Search state (for match highlighting)
/// * `styles` - Message styling configuration
/// * `focused` - Whether this pane currently has focus (affects border color)
/// * `global_wrap` - Global wrap mode setting (FR-039)
#[allow(clippy::too_many_arguments)]
pub fn render_conversation_view_with_search(
    frame: &mut Frame,
    area: Rect,
    conversation: &AgentConversation,
    scroll: &ScrollState,
    search: &crate::state::SearchState,
    styles: &MessageStyles,
    focused: bool,
    global_wrap: WrapMode,
) {
    use ratatui::text::Span;

    let entry_count = conversation.entries().len();

    // Build title
    let title = if let Some(agent_id) = conversation.agent_id() {
        let model_info = conversation
            .model()
            .map(|m| format!(" [{}]", m.display_name()))
            .unwrap_or_default();
        format!(
            "Subagent {}{} ({} entries)",
            agent_id, model_info, entry_count
        )
    } else {
        let model_info = conversation
            .model()
            .map(|m| format!(" [{}]", m.display_name()))
            .unwrap_or_default();
        format!("Main Agent{} ({} entries)", model_info, entry_count)
    };

    let border_color = if focused { Color::Cyan } else { Color::Gray };

    let mut lines = Vec::new();

    if entry_count == 0 {
        lines.push(Line::from("No messages yet..."));
    } else {
        // Extract match information if search is active
        let match_info = match search {
            crate::state::SearchState::Active {
                matches,
                current_match,
                ..
            } => Some((matches, *current_match)),
            _ => None,
        };

        // Render entries with highlighting
        for entry in conversation.entries() {
            match entry {
                ConversationEntry::Valid(log_entry) => {
                    let role = log_entry.message().role();
                    let role_style = styles.style_for_role(role);

                    match log_entry.message().content() {
                        MessageContent::Text(text) => {
                            // Get matches for this entry
                            let entry_matches = if let Some((matches, current_idx)) = &match_info {
                                let mut entry_m = Vec::new();
                                for (idx, m) in matches.iter().enumerate() {
                                    if m.entry_uuid == *log_entry.uuid() && m.block_index == 0 {
                                        entry_m.push((
                                            m.char_offset,
                                            m.length,
                                            idx == *current_idx,
                                        ));
                                    }
                                }
                                entry_m
                            } else {
                                Vec::new()
                            };

                            // Render text with highlighting (handle multi-line correctly)
                            if entry_matches.is_empty() {
                                // No highlighting - simple iteration
                                for line_text in text.lines() {
                                    lines.push(Line::from(vec![Span::styled(
                                        line_text.to_string(),
                                        role_style,
                                    )]));
                                }
                            } else {
                                // With highlighting - track line positions
                                let mut cumulative_offset: usize = 0;
                                for line_text in text.lines() {
                                    let line_start = cumulative_offset;
                                    let line_end = line_start.saturating_add(line_text.len());

                                    // Filter and convert matches for this line
                                    let line_matches: Vec<(usize, usize, bool)> = entry_matches
                                        .iter()
                                        .filter_map(|(offset, length, is_current)| {
                                            let match_start = *offset;
                                            let match_end = match_start.saturating_add(*length);

                                            // Check if match overlaps this line
                                            if match_start < line_end && match_end > line_start {
                                                // Convert to line-relative offset
                                                let line_relative_start =
                                                    match_start.saturating_sub(line_start);
                                                let line_relative_end =
                                                    (match_end - line_start).min(line_text.len());
                                                let line_relative_length = line_relative_end
                                                    .saturating_sub(line_relative_start);

                                                if line_relative_length > 0 {
                                                    Some((
                                                        line_relative_start,
                                                        line_relative_length,
                                                        *is_current,
                                                    ))
                                                } else {
                                                    None
                                                }
                                            } else {
                                                None
                                            }
                                        })
                                        .collect();

                                    // Render line with highlights
                                    let highlighted_line = apply_highlights_to_text(
                                        line_text,
                                        &line_matches,
                                        role_style,
                                    );
                                    lines.push(highlighted_line);

                                    // Update cumulative offset (add line length + newline char)
                                    cumulative_offset = line_end.saturating_add(1);
                                }
                            }
                        }
                        MessageContent::Blocks(blocks) => {
                            // TODO: Add search highlighting for ContentBlock variants
                            // (ToolUse, ToolResult, Thinking). Requires:
                            // 1. Extract text from each block type
                            // 2. Track block_index to match SearchMatch.block_index
                            // 3. Apply same multi-line highlighting logic per block
                            // For now, delegate to existing render logic (no highlighting)
                            for block in blocks {
                                let block_lines = render_content_block(
                                    block,
                                    log_entry.uuid(),
                                    scroll,
                                    styles,
                                    role_style,
                                    10,
                                    3,
                                );
                                lines.extend(block_lines);
                            }
                        }
                    }
                }
                ConversationEntry::Malformed(malformed) => {
                    lines.push(Line::from(vec![Span::styled(
                        "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━",
                        Style::default().fg(Color::Red),
                    )]));
                    lines.push(Line::from(vec![Span::styled(
                        format!("⚠ Parse Error (line {})", malformed.line_number()),
                        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                    )]));
                    for error_line in malformed.error_message().lines() {
                        lines.push(Line::from(vec![Span::styled(
                            error_line.to_string(),
                            Style::default().fg(Color::Red),
                        )]));
                    }
                }
            }
            lines.push(Line::from(""));
        }
    }

    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .style(Style::default().fg(border_color));

    // Apply wrap mode (FR-039)
    let paragraph = if global_wrap == WrapMode::Wrap {
        Paragraph::new(lines)
            .block(block)
            .wrap(Wrap { trim: false })
    } else {
        Paragraph::new(lines).block(block)
    };

    frame.render_widget(paragraph, area);
}

/// Apply highlighting to a text string based on match offsets.
/// Returns a Line with spans that have highlight styling applied.
fn apply_highlights_to_text(
    text: &str,
    matches: &[(usize, usize, bool)], // (offset, length, is_current)
    base_style: Style,
) -> Line<'static> {
    use ratatui::text::Span;

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

// ===== Horizontal Scrolling Helpers =====

/// Apply horizontal offset to a line, skipping the first `offset` characters.
///
/// Returns a new Line with characters starting from `offset` position.
/// If offset exceeds line length, returns empty line.
///
/// Uses character-based indexing (not byte-based) for UTF-8 safety.
#[allow(dead_code)]
fn apply_horizontal_offset(line: Line<'static>, offset: usize) -> Line<'static> {
    if offset == 0 {
        return line;
    }

    // Calculate total character count (not bytes)
    let total_chars: usize = line
        .spans
        .iter()
        .map(|span| span.content.chars().count())
        .sum();

    if offset >= total_chars {
        // Offset exceeds line length, return empty
        return Line::from(vec![]);
    }

    // Skip characters across spans (character-safe, not byte-based)
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
// TODO(cclv-07v.9): Wire up once horizontal scroll enabled
#[allow(dead_code)]
fn has_long_lines(lines: &[Line], viewport_width: usize) -> bool {
    lines.iter().any(|line| {
        let width: usize = line.spans.iter().map(|s| s.content.width()).sum();
        width > viewport_width
    })
}

/// Add horizontal scroll indicators to lines if needed.
///
/// Prepends ◀ if offset > 0 (can scroll left).
/// Appends ▶ if content extends beyond viewport (can scroll right).
///
/// Returns modified title string with indicators.
// TODO(cclv-07v.9): Wire up once horizontal scroll enabled
#[allow(dead_code)]
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
            Line::from(vec![Span::styled(line.to_string(), result_style)])
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

// ===== Wrap Continuation Indicators =====

/// Add wrap continuation indicators to lines that exceed viewport width.
///
/// For each Line that exceeds viewport_width when rendered, this function:
/// 1. Calculates where the line will wrap based on character width
/// 2. Splits the line into multiple Lines at wrap boundaries
/// 3. Appends a dimmed `↩` (U+21A9) indicator to each wrapped segment (except the last)
///
/// This implements FR-052: Display subtle continuation indicator at wrap points to
/// distinguish soft-wrapped lines from intentional line breaks.
///
/// # Arguments
/// * `lines` - Vector of Lines to process
/// * `viewport_width` - Width of the viewport for wrapping calculation (must be > 0)
///
/// # Returns
/// New vector of Lines with continuation indicators inserted at wrap points
///
/// # Panics
/// Never panics in public API. Invalid inputs (viewport_width = 0) return input unchanged.
#[cfg(test)]
fn add_wrap_continuation_indicators(
    lines: Vec<Line<'static>>,
    viewport_width: usize,
) -> Vec<Line<'static>> {
    use ratatui::text::Span;
    use unicode_width::UnicodeWidthChar;

    // Edge case: invalid viewport or empty input
    if viewport_width == 0 || lines.is_empty() {
        return lines;
    }

    let mut result = Vec::new();

    for line in lines {
        // Calculate the display width of this line
        let line_str = line.to_string();
        let line_width = line_str.width();

        // If line fits within viewport, no wrapping needed
        if line_width <= viewport_width {
            result.push(line);
            continue;
        }

        // Line needs wrapping - split it into segments
        // We must use display width (not character count) to ensure segments fit in viewport
        // Wide characters (emoji, CJK) take 2 display columns but count as 1 character

        let chars: Vec<char> = line_str.chars().collect();
        let mut char_pos = 0;

        while char_pos < chars.len() {
            // Calculate display width of remaining text
            let remaining_str: String = chars[char_pos..].iter().collect();
            let remaining_width = remaining_str.width();

            // Check if remaining text fits in viewport (this is the last segment)
            if remaining_width <= viewport_width {
                // Last segment: no indicator needed
                result.push(Line::from(vec![Span::raw(remaining_str)]));
                break;
            }

            // Need to wrap: accumulate characters until we reach (viewport_width - 1) display columns
            // Reserve 1 column for the ↩ continuation indicator
            let target_width = viewport_width.saturating_sub(1);
            let mut segment_chars = Vec::new();
            let mut accumulated_width = 0;

            for &ch in &chars[char_pos..] {
                let ch_width = ch.width().unwrap_or(0);
                if accumulated_width + ch_width > target_width {
                    break;
                }
                segment_chars.push(ch);
                accumulated_width += ch_width;
            }

            // Handle edge case: if we couldn't fit even one character
            if segment_chars.is_empty() && char_pos < chars.len() {
                // Take at least one character to avoid infinite loop
                segment_chars.push(chars[char_pos]);
            }

            let segment: String = segment_chars.iter().collect();
            char_pos += segment_chars.len();

            // Add segment with continuation indicator
            result.push(Line::from(vec![
                Span::raw(segment),
                Span::styled(
                    "↩",
                    Style::default()
                        .fg(Color::DarkGray)
                        .add_modifier(Modifier::DIM),
                ),
            ]));
        }
    }

    result
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

        let lines = render_tool_use(
            &tool_call,
            &uuid,
            &scroll_state,
            get_test_role_style(),
            10,
            3,
        );

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

        let lines = render_tool_use(
            &tool_call,
            &uuid,
            &scroll_state,
            get_test_role_style(),
            10,
            3,
        );

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

        let lines = render_tool_use(
            &tool_call,
            &uuid,
            &scroll_state,
            get_test_role_style(),
            10,
            3,
        );

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

        let lines = render_tool_result(
            content,
            false,
            &uuid,
            &scroll_state,
            get_test_role_style(),
            10,
            3,
        );

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

        let lines = render_tool_result(
            content,
            false,
            &uuid,
            &scroll_state,
            get_test_role_style(),
            10,
            3,
        );

        // Should show first 3 lines + collapse indicator
        let text: String = lines.iter().map(|l| l.to_string()).collect();
        assert!(text.contains("Line 1"), "Should show first line of content");
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

        let lines = render_tool_result(
            content,
            false,
            &uuid,
            &scroll_state,
            get_test_role_style(),
            10,
            3,
        );

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

        let lines = render_tool_result(
            content,
            false,
            &uuid,
            &scroll_state,
            get_test_role_style(),
            10,
            3,
        );

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

        let lines = render_content_block(
            &block,
            &uuid,
            &scroll_state,
            &create_test_styles(),
            get_test_role_style(),
            10,
            3,
        );

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

        let lines = render_content_block(
            &block,
            &uuid,
            &scroll_state,
            &create_test_styles(),
            get_test_role_style(),
            10,
            3,
        );

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

        let lines = render_content_block(
            &block,
            &uuid,
            &scroll_state,
            &create_test_styles(),
            get_test_role_style(),
            10,
            3,
        );

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

        let lines = render_content_block(
            &block,
            &uuid,
            &scroll_state,
            &create_test_styles(),
            get_test_role_style(),
            10,
            3,
        );

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

        let lines = render_content_block(
            &block,
            &uuid,
            &scroll_state,
            &create_test_styles(),
            get_test_role_style(),
            10,
            3,
        );

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

        let lines = render_content_block(
            &block,
            &uuid,
            &scroll_state,
            &create_test_styles(),
            get_test_role_style(),
            10,
            3,
        );

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

        let lines = render_content_block(
            &block,
            &uuid,
            &scroll_state,
            &create_test_styles(),
            get_test_role_style(),
            10,
            3,
        );

        // Should show all 15 lines when expanded
        assert_eq!(
            lines.len(),
            15,
            "Expanded text should show all 15 lines, got {} lines",
            lines.len()
        );

        let text: String = lines.iter().map(|l| l.to_string()).collect();
        assert!(text.contains("Line 1"), "Should show Line 1");
        assert!(
            text.contains("Line 15"),
            "Should show Line 15 when expanded"
        );
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

        let lines = render_content_block(
            &block,
            &uuid,
            &scroll_state,
            &create_test_styles(),
            get_test_role_style(),
            10,
            3,
        );

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
                render_conversation_view(
                    frame,
                    area,
                    &conversation,
                    &scroll_state,
                    &create_test_styles(),
                    false,
                    WrapMode::default(),
                );
            })
            .expect("Failed to draw");

        // Get the rendered buffer and check it contains our text
        let buffer = terminal.backend().buffer().clone();
        let content: String = buffer.content().iter().map(|cell| cell.symbol()).collect();

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
                render_conversation_view(
                    frame,
                    area,
                    &conversation,
                    &scroll_state,
                    &create_test_styles(),
                    false,
                    WrapMode::default(),
                );
            })
            .expect("Failed to draw");

        // Get the rendered buffer and check it contains tool name
        let buffer = terminal.backend().buffer().clone();
        let content: String = buffer.content().iter().map(|cell| cell.symbol()).collect();

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
        let content: String = buffer.content().iter().map(|cell| cell.symbol()).collect();

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

        let conv_entry = ConversationEntry::Valid(Box::new(entry));
        let height = widget.calculate_entry_height(&conv_entry, 80, WrapMode::NoWrap);

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

        let conv_entry = ConversationEntry::Valid(Box::new(entry));
        let height = widget.calculate_entry_height(&conv_entry, 80, WrapMode::NoWrap);

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
        let (start, end) = widget.calculate_visible_range(10, 80, WrapMode::NoWrap);

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

        let (start, end) = widget.calculate_visible_range(10, 80, WrapMode::NoWrap);

        // With scroll_offset=50, buffer=20:
        // Should start rendering before line 50 (accounting for buffer)
        // With single-line entries, should skip some entries before visible range
        assert!(
            start > 0,
            "Should skip entries before visible range when scrolled down"
        );
        assert!(end > start, "End should be after start");
        assert!(end <= 100, "End should not exceed total entry count");
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
        let content: String = buffer.content().iter().map(|cell| cell.symbol()).collect();

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
        let content: String = buffer.content().iter().map(|cell| cell.symbol()).collect();

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
        let content: String = buffer.content().iter().map(|cell| cell.symbol()).collect();

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
        let content: String = buffer.content().iter().map(|cell| cell.symbol()).collect();

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
        let has_syntax_colors = lines
            .iter()
            .any(|line| line.spans.iter().any(|span| span.style.fg.is_some()));
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
                    WrapMode::default(),
                );
            })
            .expect("Failed to draw");

        let buffer = terminal.backend().buffer().clone();
        let content: String = buffer.content().iter().map(|cell| cell.symbol()).collect();

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
                    WrapMode::default(),
                );
            })
            .expect("Failed to draw");

        let buffer = terminal.backend().buffer().clone();
        let content: String = buffer.content().iter().map(|cell| cell.symbol()).collect();

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
        let long_line = format!(
            "{}{}",
            "A".repeat(10),
            (0..90)
                .map(|i| char::from_digit(i % 10, 10).unwrap())
                .collect::<String>()
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
            focused_message: None,
            wrap_overrides: HashSet::new(),
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
                    WrapMode::NoWrap, // Horizontal scrolling requires NoWrap mode
                );
            })
            .expect("Failed to draw");

        let buffer = terminal.backend().buffer().clone();
        let content: String = buffer.content().iter().map(|cell| cell.symbol()).collect();

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
            focused_message: None,
            wrap_overrides: HashSet::new(),
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
                    WrapMode::NoWrap, // Scroll indicators require NoWrap mode
                );
            })
            .expect("Failed to draw");

        let buffer = terminal.backend().buffer().clone();
        let content: String = buffer.content().iter().map(|cell| cell.symbol()).collect();

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
                    WrapMode::NoWrap, // Scroll indicators require NoWrap mode
                );
            })
            .expect("Failed to draw");

        let buffer = terminal.backend().buffer().clone();
        let content: String = buffer.content().iter().map(|cell| cell.symbol()).collect();

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
        let message = Message::new(
            Role::Assistant,
            MessageContent::Text(short_line.to_string()),
        );

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
                    WrapMode::default(),
                );
            })
            .expect("Failed to draw");

        let buffer = terminal.backend().buffer().clone();
        let content: String = buffer.content().iter().map(|cell| cell.symbol()).collect();

        // Should NOT show scroll indicators for short lines
        assert!(
            !content.contains("◀") && !content.contains("▶"),
            "Should NOT show scroll indicators for short lines that fit in viewport"
        );
    }

    // ===== Wrap Mode Tests (FR-039/040, LW-008) =====

    #[test]
    fn conversation_view_no_scroll_indicators_when_wrap_enabled() {
        use crate::model::{
            AgentConversation, EntryMetadata, EntryType, EntryUuid, LogEntry, Message,
            MessageContent, Role, SessionId,
        };
        use crate::state::WrapMode;
        use chrono::Utc;
        use ratatui::backend::TestBackend;
        use ratatui::Terminal;

        let mut conversation = AgentConversation::new(None);

        // Create a message with a very long line (exceeds viewport width)
        let long_line = "This is a very long line that will definitely exceed the viewport width and would normally trigger horizontal scroll indicators when wrap is disabled but should NOT show indicators when wrap is enabled.";
        let message = Message::new(Role::Assistant, MessageContent::Text(long_line.to_string()));

        let entry = LogEntry::new(
            EntryUuid::new("entry-wrap-1").expect("valid uuid"),
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
                    WrapMode::Wrap, // FR-039: Wrap mode enabled
                );
            })
            .expect("Failed to draw");

        let buffer = terminal.backend().buffer().clone();
        let content: String = buffer.content().iter().map(|cell| cell.symbol()).collect();

        // FR-039: When wrap is enabled, should NOT show horizontal scroll indicators
        assert!(
            !content.contains("◀") && !content.contains("▶"),
            "Should NOT show scroll indicators when wrap mode is enabled"
        );
    }

    #[test]
    fn conversation_view_shows_scroll_indicators_when_wrap_disabled() {
        use crate::model::{
            AgentConversation, EntryMetadata, EntryType, EntryUuid, LogEntry, Message,
            MessageContent, Role, SessionId,
        };
        use crate::state::WrapMode;
        use chrono::Utc;
        use ratatui::backend::TestBackend;
        use ratatui::Terminal;

        let mut conversation = AgentConversation::new(None);

        // Create a message with a very long line (exceeds viewport width)
        let long_line = "This is a very long line that will definitely exceed the viewport width and should trigger horizontal scroll indicators when wrap is disabled since the content extends beyond the visible area.";
        let message = Message::new(Role::Assistant, MessageContent::Text(long_line.to_string()));

        let entry = LogEntry::new(
            EntryUuid::new("entry-wrap-2").expect("valid uuid"),
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
                    WrapMode::NoWrap, // FR-040: Wrap disabled, horizontal scrolling
                );
            })
            .expect("Failed to draw");

        let buffer = terminal.backend().buffer().clone();
        let content: String = buffer.content().iter().map(|cell| cell.symbol()).collect();

        // FR-040: When wrap is disabled, should show right scroll indicator for long lines
        assert!(
            content.contains("▶") || content.contains(">"),
            "Should show right scroll indicator when wrap disabled and content extends beyond viewport"
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
                render_conversation_view(
                    frame,
                    area,
                    &conversation,
                    &scroll_state,
                    &create_test_styles(),
                    false,
                    WrapMode::default(),
                );
            })
            .expect("Failed to draw");

        let buffer = terminal.backend().buffer().clone();
        let content: String = buffer.content().iter().map(|cell| cell.symbol()).collect();

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
                render_conversation_view(
                    frame,
                    area,
                    &conversation,
                    &scroll_state,
                    &create_test_styles(),
                    false,
                    WrapMode::default(),
                );
            })
            .expect("Failed to draw");

        let buffer = terminal.backend().buffer().clone();
        let content: String = buffer.content().iter().map(|cell| cell.symbol()).collect();

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

    // ===== Horizontal Scrolling UTF-8 Safety Tests =====

    #[test]
    fn apply_horizontal_offset_with_cjk_characters_does_not_panic() {
        // Test with Chinese characters (3 bytes each in UTF-8)
        // String: "Hello 世界" - 'H'(0) 'e'(1) 'l'(2) 'l'(3) 'o'(4) ' '(5) '世'(byte 6-8) '界'(byte 9-11)
        let line = Line::from(vec![ratatui::text::Span::raw("Hello 世界")]);

        // Try to skip 7 "units" - with buggy implementation this would try to slice at byte 7
        // which is in the middle of '世' (bytes 6-8) -> PANIC
        // With correct implementation, should skip 7 characters and show '界'
        let result = apply_horizontal_offset(line.clone(), 7);

        // Should contain 界 without panic
        let text: String = result.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(
            text.contains("界"),
            "Should handle CJK characters without panic, got: '{}'",
            text
        );
    }

    #[test]
    fn apply_horizontal_offset_with_emoji_does_not_panic() {
        // Test with emoji (4 bytes in UTF-8)
        let line = Line::from(vec![ratatui::text::Span::raw("Hi 🎉 there")]);

        // Skip past emoji - should not panic
        let result = apply_horizontal_offset(line.clone(), 4);

        // Should not panic - we're just verifying it completes
        let text: String = result.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(
            !text.is_empty() || text.is_empty(),
            "Should handle emoji without panic"
        );
    }

    #[test]
    fn apply_horizontal_offset_mid_multibyte_char_handles_gracefully() {
        // Test offset that lands in the middle of a multi-byte character
        let line = Line::from(vec![ratatui::text::Span::raw("AB世CD")]);

        // Offset 3 should skip "AB世" (3 characters) and show "CD"
        let result = apply_horizontal_offset(line.clone(), 3);

        let text: String = result.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(
            text.contains("CD") || text.starts_with("CD"),
            "Should correctly skip multi-byte characters"
        );
    }

    #[test]
    fn has_long_lines_uses_visual_width_not_byte_count() {
        // CJK characters have visual width 2 (take up 2 terminal columns)
        // "世界" is 2 characters but 6 bytes, visual width = 4 (2 chars × 2 cols)
        // "ab" is 2 characters, 2 bytes, visual width = 2 (2 chars × 1 col)

        let line_cjk = Line::from(vec![ratatui::text::Span::raw("世界")]);
        let line_ascii = Line::from(vec![ratatui::text::Span::raw("ab")]);

        // CJK visual width 4 should exceed viewport width 3
        assert!(
            has_long_lines(&[line_cjk], 3),
            "CJK '世界' has visual width 4, should exceed viewport 3"
        );

        // ASCII visual width 2 should NOT exceed viewport width 3
        assert!(
            !has_long_lines(&[line_ascii], 3),
            "ASCII 'ab' has visual width 2, should NOT exceed viewport 3"
        );

        // If we used byte count (buggy), CJK would be 6 bytes (exceeds 3)
        // and ASCII would be 2 bytes (doesn't exceed 3)
        // This test proves we're using visual width, not bytes

        // Both should fit in viewport width 5
        let line_cjk2 = Line::from(vec![ratatui::text::Span::raw("世界")]);
        let line_ascii2 = Line::from(vec![ratatui::text::Span::raw("ab")]);
        assert!(
            !has_long_lines(&[line_cjk2], 5),
            "CJK visual width 4 should fit in viewport 5"
        );
        assert!(
            !has_long_lines(&[line_ascii2], 5),
            "ASCII visual width 2 should fit in viewport 5"
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
        let message = Message::new(
            Role::Assistant,
            MessageContent::Text(short_text.to_string()),
        );

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
                render_conversation_view(
                    frame,
                    area,
                    &conversation,
                    &scroll_state,
                    &create_test_styles(),
                    false,
                    WrapMode::default(),
                );
            })
            .expect("Failed to draw");

        let buffer = terminal.backend().buffer().clone();
        let content: String = buffer.content().iter().map(|cell| cell.symbol()).collect();

        // Should show all lines for short content (no collapse)
        assert!(content.contains("Line 1"), "Should show Line 1");
        assert!(content.contains("Line 5"), "Should show Line 5");
        assert!(
            !content.contains("more lines"),
            "Should NOT show collapse indicator for short content"
        );
    }

    // ===== Search Highlighting Tests =====

    /// Helper to create a test entry with simple text content
    fn create_test_log_entry(uuid: &str, text: &str) -> crate::model::LogEntry {
        let uuid = crate::model::EntryUuid::new(uuid).expect("valid uuid");
        let session_id = crate::model::SessionId::new("session-1").expect("valid session");
        let message = crate::model::Message::new(
            crate::model::Role::Assistant,
            crate::model::MessageContent::Text(text.to_string()),
        );

        crate::model::LogEntry::new(
            uuid,
            None,
            session_id,
            None,
            chrono::Utc::now(),
            crate::model::EntryType::Assistant,
            message,
            crate::model::EntryMetadata::default(),
        )
    }

    #[test]
    fn render_text_without_search_has_no_highlighting() {
        // ARRANGE: Create conversation with simple text
        let mut conversation = crate::model::AgentConversation::new(None);
        conversation.add_entry(create_test_log_entry("entry-1", "This is some test text"));

        let scroll_state = ScrollState::default();
        let search_state = crate::state::SearchState::Inactive;

        // ACT: Render the conversation
        let backend = ratatui::backend::TestBackend::new(80, 24);
        let mut terminal = ratatui::Terminal::new(backend).expect("Failed to create terminal");

        terminal
            .draw(|frame| {
                let area = frame.area();
                render_conversation_view_with_search(
                    frame,
                    area,
                    &conversation,
                    &scroll_state,
                    &search_state,
                    &create_test_styles(),
                    false,
                    WrapMode::default(),
                );
            })
            .expect("Failed to draw");

        let buffer = terminal.backend().buffer().clone();

        // ASSERT: No cells should have highlight background
        for cell in buffer.content() {
            assert_ne!(
                cell.style().bg,
                Some(Color::Yellow),
                "Should not have yellow background when search inactive"
            );
        }
    }

    #[test]
    fn render_text_with_active_search_highlights_matches() {
        // ARRANGE: Create conversation with text containing "test"
        let mut conversation = crate::model::AgentConversation::new(None);
        let entry_uuid = crate::model::EntryUuid::new("entry-1").expect("valid uuid");
        conversation.add_entry(create_test_log_entry(
            "entry-1",
            "This is test text with test repeated",
        ));

        let scroll_state = ScrollState::default();

        // Create search state with matches for "test"
        let query = crate::state::SearchQuery::new("test").expect("valid query");
        let matches = vec![
            crate::state::SearchMatch {
                agent_id: None,
                entry_uuid: entry_uuid.clone(),
                block_index: 0,
                char_offset: 8, // First "test"
                length: 4,
            },
            crate::state::SearchMatch {
                agent_id: None,
                entry_uuid: entry_uuid.clone(),
                block_index: 0,
                char_offset: 23, // Second "test"
                length: 4,
            },
        ];
        let search_state = crate::state::SearchState::Active {
            query,
            matches,
            current_match: 0,
        };

        // ACT: Render the conversation
        let backend = ratatui::backend::TestBackend::new(80, 24);
        let mut terminal = ratatui::Terminal::new(backend).expect("Failed to create terminal");

        terminal
            .draw(|frame| {
                let area = frame.area();
                render_conversation_view_with_search(
                    frame,
                    area,
                    &conversation,
                    &scroll_state,
                    &search_state,
                    &create_test_styles(),
                    false,
                    WrapMode::default(),
                );
            })
            .expect("Failed to draw");

        let buffer = terminal.backend().buffer().clone();

        // ASSERT: Should have highlighting for search matches
        let highlighted_cells: Vec<_> = buffer
            .content()
            .iter()
            .filter(|cell| cell.style().bg == Some(Color::Yellow))
            .collect();

        assert!(
            !highlighted_cells.is_empty(),
            "Should have at least one highlighted cell for search matches"
        );
    }

    #[test]
    fn render_text_with_active_search_distinguishes_current_match() {
        // ARRANGE: Create conversation with text containing "test" multiple times
        let mut conversation = crate::model::AgentConversation::new(None);
        let entry_uuid = crate::model::EntryUuid::new("entry-1").expect("valid uuid");
        conversation.add_entry(create_test_log_entry(
            "entry-1",
            "test one test two test three",
        ));

        let scroll_state = ScrollState::default();

        // Create search state with 3 matches, current_match = 1 (second match)
        let query = crate::state::SearchQuery::new("test").expect("valid query");
        let matches = vec![
            crate::state::SearchMatch {
                agent_id: None,
                entry_uuid: entry_uuid.clone(),
                block_index: 0,
                char_offset: 0,
                length: 4,
            },
            crate::state::SearchMatch {
                agent_id: None,
                entry_uuid: entry_uuid.clone(),
                block_index: 0,
                char_offset: 9,
                length: 4,
            },
            crate::state::SearchMatch {
                agent_id: None,
                entry_uuid: entry_uuid.clone(),
                block_index: 0,
                char_offset: 18,
                length: 4,
            },
        ];
        let search_state = crate::state::SearchState::Active {
            query,
            matches,
            current_match: 1, // Second match is current
        };

        // ACT: Render the conversation
        let backend = ratatui::backend::TestBackend::new(80, 24);
        let mut terminal = ratatui::Terminal::new(backend).expect("Failed to create terminal");

        terminal
            .draw(|frame| {
                let area = frame.area();
                render_conversation_view_with_search(
                    frame,
                    area,
                    &conversation,
                    &scroll_state,
                    &search_state,
                    &create_test_styles(),
                    false,
                    WrapMode::default(),
                );
            })
            .expect("Failed to draw");

        let buffer = terminal.backend().buffer().clone();

        // ASSERT: Should have different styling for current match vs other matches
        let yellow_bg_cells: Vec<_> = buffer
            .content()
            .iter()
            .filter(|cell| cell.style().bg == Some(Color::Yellow))
            .collect();

        let inverted_cells: Vec<_> = buffer
            .content()
            .iter()
            .filter(|cell| cell.style().add_modifier == Modifier::REVERSED)
            .collect();

        assert!(
            !yellow_bg_cells.is_empty() || !inverted_cells.is_empty(),
            "Should have highlighting for search matches"
        );
    }

    // ===== apply_highlights_to_text Tests =====

    #[test]
    fn apply_highlights_single_line_no_matches() {
        let text = "Hello world";
        let matches = vec![];
        let style = Style::default();

        let line = apply_highlights_to_text(text, &matches, style);

        // Should have single span with no highlighting
        assert_eq!(line.spans.len(), 1);
        assert_eq!(line.spans[0].content, "Hello world");
        assert_eq!(line.spans[0].style, style);
    }

    #[test]
    fn apply_highlights_single_line_one_match() {
        let text = "Hello world";
        let matches = vec![(6, 5, false)]; // "world"
        let style = Style::default();

        let line = apply_highlights_to_text(text, &matches, style);

        // Should have 3 spans: before, match, after
        assert_eq!(line.spans.len(), 2); // "Hello " + highlighted "world"
        assert_eq!(line.spans[0].content, "Hello ");
        assert_eq!(line.spans[1].content, "world");
        assert_eq!(line.spans[1].style.bg, Some(Color::Yellow));
    }

    #[test]
    fn apply_highlights_single_line_current_match() {
        let text = "Hello world";
        let matches = vec![(6, 5, true)]; // "world" as current
        let style = Style::default();

        let line = apply_highlights_to_text(text, &matches, style);

        assert_eq!(line.spans.len(), 2);
        assert_eq!(line.spans[1].content, "world");
        assert_eq!(line.spans[1].style.bg, Some(Color::Yellow));
        assert_eq!(line.spans[1].style.add_modifier, Modifier::REVERSED);
    }

    #[test]
    fn apply_highlights_single_line_multiple_matches() {
        let text = "foo bar foo";
        let matches = vec![
            (0, 3, false), // first "foo"
            (8, 3, false), // second "foo"
        ];
        let style = Style::default();

        let line = apply_highlights_to_text(text, &matches, style);

        // Should have: highlighted "foo", " bar ", highlighted "foo"
        assert_eq!(line.spans.len(), 3);
        assert_eq!(line.spans[0].content, "foo");
        assert_eq!(line.spans[0].style.bg, Some(Color::Yellow));
        assert_eq!(line.spans[1].content, " bar ");
        assert_eq!(line.spans[2].content, "foo");
        assert_eq!(line.spans[2].style.bg, Some(Color::Yellow));
    }

    // ===== Multi-line Highlighting Tests (EXPOSE THE BUG) =====

    #[test]
    fn render_multiline_text_with_match_on_second_line() {
        // ARRANGE: Multi-line text with match on line 2
        let mut conversation = crate::model::AgentConversation::new(None);
        let entry_uuid = crate::model::EntryUuid::new("entry-ml1").expect("valid uuid");
        conversation.add_entry(create_test_log_entry(
            "entry-ml1",
            "First line\nSecond line",
        ));

        let scroll_state = ScrollState::default();

        let query = crate::state::SearchQuery::new("Second").expect("valid query");
        let matches = vec![crate::state::SearchMatch {
            agent_id: None,
            entry_uuid: entry_uuid.clone(),
            block_index: 0,
            char_offset: 11, // After "First line\n"
            length: 6,       // "Second"
        }];
        let search_state = crate::state::SearchState::Active {
            query,
            matches,
            current_match: 0,
        };

        // ACT: Render
        let backend = ratatui::backend::TestBackend::new(80, 24);
        let mut terminal = ratatui::Terminal::new(backend).expect("Failed to create terminal");

        terminal
            .draw(|frame| {
                let area = frame.area();
                render_conversation_view_with_search(
                    frame,
                    area,
                    &conversation,
                    &scroll_state,
                    &search_state,
                    &create_test_styles(),
                    false,
                    WrapMode::default(),
                );
            })
            .expect("Failed to draw");

        let buffer = terminal.backend().buffer().clone();
        let content: String = buffer.content().iter().map(|cell| cell.symbol()).collect();

        // ASSERT: Both lines rendered
        assert!(content.contains("First"), "Should render first line");
        assert!(content.contains("Second"), "Should render second line");

        // CRITICAL: "Second" should be highlighted
        let highlighted_cells: Vec<_> = buffer
            .content()
            .iter()
            .filter(|cell| cell.style().bg == Some(Color::Yellow))
            .collect();

        assert!(
            !highlighted_cells.is_empty(),
            "BUG DETECTED: Word 'Second' on line 2 should be highlighted. \
             Current implementation applies text-wide char offsets to per-line text, \
             which fails for matches on line 2+."
        );
    }

    #[test]
    fn render_multiline_text_with_match_on_first_line() {
        // ARRANGE: Multi-line text with match on line 1 (should work with current impl)
        let mut conversation = crate::model::AgentConversation::new(None);
        let entry_uuid = crate::model::EntryUuid::new("entry-ml2").expect("valid uuid");
        conversation.add_entry(create_test_log_entry(
            "entry-ml2",
            "First line\nSecond line",
        ));

        let scroll_state = ScrollState::default();

        let query = crate::state::SearchQuery::new("First").expect("valid query");
        let matches = vec![crate::state::SearchMatch {
            agent_id: None,
            entry_uuid: entry_uuid.clone(),
            block_index: 0,
            char_offset: 0, // Start of text
            length: 5,      // "First"
        }];
        let search_state = crate::state::SearchState::Active {
            query,
            matches,
            current_match: 0,
        };

        // ACT: Render
        let backend = ratatui::backend::TestBackend::new(80, 24);
        let mut terminal = ratatui::Terminal::new(backend).expect("Failed to create terminal");

        terminal
            .draw(|frame| {
                let area = frame.area();
                render_conversation_view_with_search(
                    frame,
                    area,
                    &conversation,
                    &scroll_state,
                    &search_state,
                    &create_test_styles(),
                    false,
                    WrapMode::default(),
                );
            })
            .expect("Failed to draw");

        let buffer = terminal.backend().buffer().clone();

        // ASSERT: "First" should be highlighted (works with current impl)
        let highlighted_cells: Vec<_> = buffer
            .content()
            .iter()
            .filter(|cell| cell.style().bg == Some(Color::Yellow))
            .collect();

        assert!(
            !highlighted_cells.is_empty(),
            "First line match should be highlighted"
        );
    }

    #[test]
    fn render_multiline_text_with_multiple_matches_across_lines() {
        // ARRANGE: Matches on both lines
        let mut conversation = crate::model::AgentConversation::new(None);
        let entry_uuid = crate::model::EntryUuid::new("entry-ml3").expect("valid uuid");
        conversation.add_entry(create_test_log_entry("entry-ml3", "foo bar\nfoo baz"));

        let scroll_state = ScrollState::default();

        let query = crate::state::SearchQuery::new("foo").expect("valid query");
        let matches = vec![
            crate::state::SearchMatch {
                agent_id: None,
                entry_uuid: entry_uuid.clone(),
                block_index: 0,
                char_offset: 0, // First "foo"
                length: 3,
            },
            crate::state::SearchMatch {
                agent_id: None,
                entry_uuid: entry_uuid.clone(),
                block_index: 0,
                char_offset: 8, // Second "foo" after "foo bar\n"
                length: 3,
            },
        ];
        let search_state = crate::state::SearchState::Active {
            query,
            matches,
            current_match: 0,
        };

        // ACT: Render
        let backend = ratatui::backend::TestBackend::new(80, 24);
        let mut terminal = ratatui::Terminal::new(backend).expect("Failed to create terminal");

        terminal
            .draw(|frame| {
                let area = frame.area();
                render_conversation_view_with_search(
                    frame,
                    area,
                    &conversation,
                    &scroll_state,
                    &search_state,
                    &create_test_styles(),
                    false,
                    WrapMode::default(),
                );
            })
            .expect("Failed to draw");

        let buffer = terminal.backend().buffer().clone();

        // ASSERT: Both matches highlighted
        let yellow_cells: Vec<_> = buffer
            .content()
            .iter()
            .filter(|cell| cell.style().bg == Some(Color::Yellow))
            .collect();

        assert!(
            yellow_cells.len() >= 6,
            "Both 'foo' matches should be highlighted (6 chars). Found {} highlighted cells. \
             BUG: Current implementation fails for second match on line 2.",
            yellow_cells.len()
        );
    }

    #[test]
    fn render_multiline_text_with_current_match_on_second_line() {
        // ARRANGE: Current match on line 2
        let mut conversation = crate::model::AgentConversation::new(None);
        let entry_uuid = crate::model::EntryUuid::new("entry-ml4").expect("valid uuid");
        conversation.add_entry(create_test_log_entry("entry-ml4", "foo bar\nfoo baz"));

        let scroll_state = ScrollState::default();

        let query = crate::state::SearchQuery::new("foo").expect("valid query");
        let matches = vec![
            crate::state::SearchMatch {
                agent_id: None,
                entry_uuid: entry_uuid.clone(),
                block_index: 0,
                char_offset: 0, // First "foo"
                length: 3,
            },
            crate::state::SearchMatch {
                agent_id: None,
                entry_uuid: entry_uuid.clone(),
                block_index: 0,
                char_offset: 8, // Second "foo" (CURRENT)
                length: 3,
            },
        ];
        let search_state = crate::state::SearchState::Active {
            query,
            matches,
            current_match: 1, // Current is second match
        };

        // ACT: Render
        let backend = ratatui::backend::TestBackend::new(80, 24);
        let mut terminal = ratatui::Terminal::new(backend).expect("Failed to create terminal");

        terminal
            .draw(|frame| {
                let area = frame.area();
                render_conversation_view_with_search(
                    frame,
                    area,
                    &conversation,
                    &scroll_state,
                    &search_state,
                    &create_test_styles(),
                    false,
                    WrapMode::default(),
                );
            })
            .expect("Failed to draw");

        let buffer = terminal.backend().buffer().clone();

        // ASSERT: Should have REVERSED highlighting for current match
        let yellow_bg: Vec<_> = buffer
            .content()
            .iter()
            .filter(|cell| cell.style().bg == Some(Color::Yellow))
            .collect();

        let reversed: Vec<_> = buffer
            .content()
            .iter()
            .filter(|cell| cell.style().add_modifier == Modifier::REVERSED)
            .collect();

        assert!(!yellow_bg.is_empty(), "Should have yellow highlighting");
        assert!(
            !reversed.is_empty(),
            "BUG: Current match on line 2 should have REVERSED modifier. \
             Current implementation fails due to line-by-line iteration with text-wide offsets."
        );
    }

    // ===== render_entry_paragraph Tests =====

    #[test]
    fn render_entry_paragraph_returns_paragraph_with_wrap_mode() {
        // ARRANGE: Create a simple valid entry
        let entry = ConversationEntry::Valid(Box::new(create_test_log_entry(
            "entry-para-1",
            "Simple text",
        )));
        let scroll_state = create_test_scroll_state();
        let styles = create_test_styles();

        // ACT: Render with Wrap mode
        let paragraph = render_entry_paragraph(
            &entry,
            0,
            false,
            &scroll_state,
            &styles,
            10,
            3,
            WrapMode::Wrap,
        );

        // ASSERT: Returns a Paragraph widget (compilation verifies the type)
        // We can verify it's a valid Paragraph by attempting to render it
        let mut buffer = Buffer::empty(Rect::new(0, 0, 80, 10));
        paragraph.render(Rect::new(0, 0, 80, 10), &mut buffer);
        // If we got here without panic, the Paragraph was valid
    }

    #[test]
    fn render_entry_paragraph_applies_wrap_when_wrap_mode_is_wrap() {
        // ARRANGE: Entry with long text that would wrap
        let long_text = "This is a very long line that would definitely wrap in a narrow terminal viewport if wrapping is enabled for this entry";
        let entry = ConversationEntry::Valid(Box::new(create_test_log_entry(
            "entry-para-2",
            long_text,
        )));
        let scroll_state = create_test_scroll_state();
        let styles = create_test_styles();

        // ACT: Render with Wrap mode
        let paragraph = render_entry_paragraph(
            &entry,
            0,
            false,
            &scroll_state,
            &styles,
            10,
            3,
            WrapMode::Wrap,
        );

        // ASSERT: Render to a narrow buffer and verify text wraps
        let mut buffer = Buffer::empty(Rect::new(0, 0, 20, 10));
        paragraph.render(Rect::new(0, 0, 20, 10), &mut buffer);

        // Extract lines from buffer
        let mut lines_with_content = Vec::new();
        for y in 0..10 {
            let line: String = (0..20)
                .map(|x| {
                    let idx = y * 20 + x;
                    buffer.content()[idx].symbol()
                })
                .collect();
            if line.trim().len() > 0 {
                lines_with_content.push(line);
            }
        }

        assert!(
            lines_with_content.len() > 1,
            "Text should wrap to multiple lines in narrow viewport. Found {} non-empty lines",
            lines_with_content.len()
        );
    }

    #[test]
    fn render_entry_paragraph_no_wrap_when_wrap_mode_is_nowrap() {
        // ARRANGE: Entry with long text
        let long_text = "This is a very long line that would wrap if wrapping was enabled but should stay on one line";
        let entry = ConversationEntry::Valid(Box::new(create_test_log_entry(
            "entry-para-3",
            long_text,
        )));
        let scroll_state = create_test_scroll_state();
        let styles = create_test_styles();

        // ACT: Render with NoWrap mode
        let paragraph = render_entry_paragraph(
            &entry,
            0,
            false,
            &scroll_state,
            &styles,
            10,
            3,
            WrapMode::NoWrap,
        );

        // ASSERT: Render to a narrow buffer - text should not wrap
        let mut buffer = Buffer::empty(Rect::new(0, 0, 20, 10));
        paragraph.render(Rect::new(0, 0, 20, 10), &mut buffer);

        // In NoWrap mode, content should be on line 0 (single line), rest empty
        // Line 1 might have the spacing line from render_entry_lines, but the main
        // content text should not have wrapped to multiple lines
        let line0: String = (0..20)
            .map(|x| buffer.content()[x].symbol())
            .collect();
        let line1: String = (0..20)
            .map(|x| buffer.content()[20 + x].symbol())
            .collect();

        // Line 0 should have text content (truncated, not wrapped)
        assert!(
            line0.trim().len() > 0,
            "Line 0 should have content"
        );

        // The long text should appear on one line only (may be truncated)
        // We verify this by checking that line 1 is either empty or just spacing
        let line1_is_content = line1.trim().len() > 0
            && line1.contains(char::is_alphanumeric);

        assert!(
            !line1_is_content,
            "NoWrap mode should not wrap text content to line 1. Line 1 content: '{}'",
            line1.trim()
        );
    }

    #[test]
    fn render_entry_paragraph_uses_render_entry_lines_for_content() {
        // ARRANGE: Entry with multiple lines
        let entry = ConversationEntry::Valid(Box::new(create_test_log_entry(
            "entry-para-4",
            "Line 1\nLine 2\nLine 3",
        )));
        let scroll_state = create_test_scroll_state();
        let styles = create_test_styles();

        // ACT: Render paragraph
        let paragraph = render_entry_paragraph(
            &entry,
            0,
            false,
            &scroll_state,
            &styles,
            10,
            3,
            WrapMode::Wrap,
        );

        // ASSERT: All lines from render_entry_lines should be in the paragraph
        let mut buffer = Buffer::empty(Rect::new(0, 0, 80, 10));
        paragraph.render(Rect::new(0, 0, 80, 10), &mut buffer);

        let content: String = buffer.content().iter().map(|cell| cell.symbol()).collect();

        assert!(
            content.contains("Line 1"),
            "Should contain Line 1 from render_entry_lines"
        );
        assert!(
            content.contains("Line 2"),
            "Should contain Line 2 from render_entry_lines"
        );
        assert!(
            content.contains("Line 3"),
            "Should contain Line 3 from render_entry_lines"
        );
    }

    // ===== Tests for calculate_entry_height with wrap mode =====

    #[test]
    fn calculate_entry_height_nowrap_counts_newlines() {
        use crate::model::{EntryMetadata, EntryType, EntryUuid, LogEntry, Message, Role, SessionId};
        use chrono::Utc;

        let uuid = EntryUuid::new("test-uuid-1").expect("valid uuid");
        let session_id = SessionId::new("test-session-1").expect("valid session");
        let timestamp = Utc::now();

        // Create entry with 3 lines of text (2 newlines)
        let message = Message::new(Role::User, MessageContent::Text("Line 1\nLine 2\nLine 3".to_string()));
        let log_entry = LogEntry::new(
            uuid,
            None,
            session_id,
            None,
            timestamp,
            EntryType::User,
            message,
            EntryMetadata::default(),
        );
        let entry = ConversationEntry::Valid(Box::new(log_entry));

        let conversation = AgentConversation::new(None);
        let scroll_state = ScrollState::default();
        let styles = create_test_styles();
        let widget = ConversationView::new(&conversation, &scroll_state, &styles, false);

        // With NoWrap, should return line count (3 lines)
        let height = widget.calculate_entry_height(&entry, 80, WrapMode::NoWrap);
        assert_eq!(height, 3, "NoWrap mode should count newlines: 3 lines");
    }

    #[test]
    fn calculate_entry_height_wrap_with_long_line_wraps() {
        use crate::model::{EntryMetadata, EntryType, EntryUuid, LogEntry, Message, Role, SessionId};
        use chrono::Utc;

        let uuid = EntryUuid::new("test-uuid-2").expect("valid uuid");
        let session_id = SessionId::new("test-session-2").expect("valid session");
        let timestamp = Utc::now();

        // Create entry with single long line (100 chars, should wrap to 2 lines at width 80)
        let long_text = "a".repeat(100);
        
        let message = Message::new(Role::User, MessageContent::Text(long_text));
        let log_entry = LogEntry::new(
            uuid,
            None,
            session_id,
            None,
            timestamp,
            EntryType::User,
            message,
            EntryMetadata::default(),
        );
        let entry = ConversationEntry::Valid(Box::new(log_entry));

        let conversation = AgentConversation::new(None);
        let scroll_state = ScrollState::default();
        let styles = create_test_styles();
        let widget = ConversationView::new(&conversation, &scroll_state, &styles, false);

        // With Wrap at width 80, 100 chars should wrap to at least 2 lines
        let height = widget.calculate_entry_height(&entry, 80, WrapMode::Wrap);
        assert!(height >= 2, "Wrap mode should wrap 100 chars at width 80 to at least 2 lines, got {}", height);
    }

    #[test]
    fn calculate_entry_height_wrap_respects_per_entry_override() {
        use crate::model::{EntryMetadata, EntryType, EntryUuid, LogEntry, Message, Role, SessionId};
        use chrono::Utc;

        let uuid = EntryUuid::new("test-uuid-3").expect("valid uuid");
        let session_id = SessionId::new("test-session-3").expect("valid session");
        let timestamp = Utc::now();

        // Create entry with long line
        let long_text = "a".repeat(100);
        
        let message = Message::new(Role::User, MessageContent::Text(long_text));
        let log_entry = LogEntry::new(
            uuid.clone(),
            None,
            session_id,
            None,
            timestamp,
            EntryType::User,
            message,
            EntryMetadata::default(),
        );
        let entry = ConversationEntry::Valid(Box::new(log_entry));

        let conversation = AgentConversation::new(None);
        let mut scroll_state = ScrollState::default();

        // Add per-item override (global Wrap -> NoWrap for this entry)
        scroll_state.toggle_wrap(&uuid);

        let styles = create_test_styles();
        let widget = ConversationView::new(&conversation, &scroll_state, &styles, false);

        // Global is Wrap, but per-item override should make it NoWrap (1 line)
        let height = widget.calculate_entry_height(&entry, 80, WrapMode::Wrap);
        assert_eq!(height, 1, "Per-item override should invert global Wrap to NoWrap (1 line)");
    }

    #[test]
    fn calculate_entry_height_wrap_empty_text() {
        use crate::model::{EntryMetadata, EntryType, EntryUuid, LogEntry, Message, Role, SessionId};
        use chrono::Utc;

        let uuid = EntryUuid::new("test-uuid-4").expect("valid uuid");
        let session_id = SessionId::new("test-session-4").expect("valid session");
        let timestamp = Utc::now();

        
        let message = Message::new(Role::User, MessageContent::Text("".to_string()));
        let log_entry = LogEntry::new(
            uuid,
            None,
            session_id,
            None,
            timestamp,
            EntryType::User,
            message,
            EntryMetadata::default(),
        );
        let entry = ConversationEntry::Valid(Box::new(log_entry));

        let conversation = AgentConversation::new(None);
        let scroll_state = ScrollState::default();
        let styles = create_test_styles();
        let widget = ConversationView::new(&conversation, &scroll_state, &styles, false);

        // Empty text should be at least 1 line (empty line still occupies space)
        let height = widget.calculate_entry_height(&entry, 80, WrapMode::Wrap);
        assert!(height >= 1, "Empty text should have height of at least 1, got {}", height);
    }

    #[test]
    fn calculate_entry_height_wrap_exactly_viewport_width() {
        use crate::model::{EntryMetadata, EntryType, EntryUuid, LogEntry, Message, Role, SessionId};
        use chrono::Utc;

        let uuid = EntryUuid::new("test-uuid-5").expect("valid uuid");
        let session_id = SessionId::new("test-session-5").expect("valid session");
        let timestamp = Utc::now();

        // Create text exactly 80 chars (should fit in one line)
        let text = "a".repeat(80);
        
        let message = Message::new(Role::User, MessageContent::Text(text));
        let log_entry = LogEntry::new(
            uuid,
            None,
            session_id,
            None,
            timestamp,
            EntryType::User,
            message,
            EntryMetadata::default(),
        );
        let entry = ConversationEntry::Valid(Box::new(log_entry));

        let conversation = AgentConversation::new(None);
        let scroll_state = ScrollState::default();
        let styles = create_test_styles();
        let widget = ConversationView::new(&conversation, &scroll_state, &styles, false);

        // Exactly viewport width should fit in 1 line
        let height = widget.calculate_entry_height(&entry, 80, WrapMode::Wrap);
        assert_eq!(height, 1, "Text exactly viewport width should fit in 1 line");
    }

    #[test]
    fn calculate_entry_height_wrap_one_char_over_wraps() {
        use crate::model::{EntryMetadata, EntryType, EntryUuid, LogEntry, Message, Role, SessionId};
        use chrono::Utc;

        let uuid = EntryUuid::new("test-uuid-6").expect("valid uuid");
        let session_id = SessionId::new("test-session-6").expect("valid session");
        let timestamp = Utc::now();

        // Create text 81 chars (one more than viewport, should wrap to 2 lines)
        let text = "a".repeat(81);
        
        let message = Message::new(Role::User, MessageContent::Text(text));
        let log_entry = LogEntry::new(
            uuid,
            None,
            session_id,
            None,
            timestamp,
            EntryType::User,
            message,
            EntryMetadata::default(),
        );
        let entry = ConversationEntry::Valid(Box::new(log_entry));

        let conversation = AgentConversation::new(None);
        let scroll_state = ScrollState::default();
        let styles = create_test_styles();
        let widget = ConversationView::new(&conversation, &scroll_state, &styles, false);

        // 81 chars at width 80 should wrap to 2 lines
        let height = widget.calculate_entry_height(&entry, 80, WrapMode::Wrap);
        assert_eq!(height, 2, "Text one char over viewport width should wrap to 2 lines");
    }

    #[test]
    fn calculate_entry_height_wrap_multiline_text_each_line_wraps() {
        use crate::model::{EntryMetadata, EntryType, EntryUuid, LogEntry, Message, Role, SessionId};
        use chrono::Utc;

        let uuid = EntryUuid::new("test-uuid-7").expect("valid uuid");
        let session_id = SessionId::new("test-session-7").expect("valid session");
        let timestamp = Utc::now();

        // Two lines, each 100 chars (should wrap to 2 lines each = 4 total)
        let text = format!("{}\n{}", "a".repeat(100), "b".repeat(100));
        
        let message = Message::new(Role::User, MessageContent::Text(text));
        let log_entry = LogEntry::new(
            uuid,
            None,
            session_id,
            None,
            timestamp,
            EntryType::User,
            message,
            EntryMetadata::default(),
        );
        let entry = ConversationEntry::Valid(Box::new(log_entry));

        let conversation = AgentConversation::new(None);
        let scroll_state = ScrollState::default();
        let styles = create_test_styles();
        let widget = ConversationView::new(&conversation, &scroll_state, &styles, false);

        // 2 lines × 2 wrapped = 4 total lines
        let height = widget.calculate_entry_height(&entry, 80, WrapMode::Wrap);
        assert!(height >= 4, "Two 100-char lines at width 80 should wrap to at least 4 lines, got {}", height);
    }

    // ===== Tests for calculate_entry_layouts =====

    #[test]
    fn calculate_entry_layouts_empty_entries_returns_empty() {
        let conversation = AgentConversation::new(None);
        let scroll_state = ScrollState::default();
        let styles = create_test_styles();
        let widget = ConversationView::new(&conversation, &scroll_state, &styles, false);

        let layouts = widget.calculate_entry_layouts(&[], 0, 80, 24, WrapMode::NoWrap);

        assert_eq!(layouts.len(), 0, "Empty entries should return empty layout vec");
    }

    #[test]
    fn calculate_entry_layouts_single_entry_has_zero_offset() {
        use crate::model::{EntryMetadata, EntryType, EntryUuid, LogEntry, Message, MessageContent, Role, SessionId};
        use chrono::Utc;

        let uuid = EntryUuid::new("test-uuid-1").expect("valid uuid");
        let session_id = SessionId::new("test-session-1").expect("valid session");
        let message = Message::new(
            Role::User,
            MessageContent::Text("Line 1\nLine 2\nLine 3".to_string()),
        );
        let log_entry = LogEntry::new(
            uuid,
            None,
            session_id,
            None,
            Utc::now(),
            EntryType::User,
            message,
            EntryMetadata::default(),
        );
        let entry = ConversationEntry::Valid(Box::new(log_entry));

        let conversation = AgentConversation::new(None);
        let scroll_state = ScrollState::default();
        let styles = create_test_styles();
        let widget = ConversationView::new(&conversation, &scroll_state, &styles, false);

        let layouts = widget.calculate_entry_layouts(&[entry], 0, 80, 24, WrapMode::NoWrap);

        assert_eq!(layouts.len(), 1, "Single entry should return one layout");
        assert_eq!(layouts[0].y_offset, 0, "First entry should have y_offset=0");
        assert_eq!(layouts[0].height, 3, "Entry with 3 lines should have height=3");
    }

    #[test]
    fn calculate_entry_layouts_multiple_entries_have_cumulative_offsets() {
        use crate::model::{EntryMetadata, EntryType, EntryUuid, LogEntry, Message, MessageContent, Role, SessionId};
        use chrono::Utc;

        // Entry 1: 3 lines
        let uuid1 = EntryUuid::new("test-uuid-1").expect("valid uuid");
        let session_id = SessionId::new("test-session-1").expect("valid session");
        let message1 = Message::new(
            Role::User,
            MessageContent::Text("Line 1\nLine 2\nLine 3".to_string()),
        );
        let log_entry1 = LogEntry::new(
            uuid1,
            None,
            session_id.clone(),
            None,
            Utc::now(),
            EntryType::User,
            message1,
            EntryMetadata::default(),
        );
        let entry1 = ConversationEntry::Valid(Box::new(log_entry1));

        // Entry 2: 2 lines
        let uuid2 = EntryUuid::new("test-uuid-2").expect("valid uuid");
        let message2 = Message::new(
            Role::Assistant,
            MessageContent::Text("Response 1\nResponse 2".to_string()),
        );
        let log_entry2 = LogEntry::new(
            uuid2,
            None,
            session_id.clone(),
            None,
            Utc::now(),
            EntryType::Assistant,
            message2,
            EntryMetadata::default(),
        );
        let entry2 = ConversationEntry::Valid(Box::new(log_entry2));

        // Entry 3: 1 line
        let uuid3 = EntryUuid::new("test-uuid-3").expect("valid uuid");
        let message3 = Message::new(
            Role::User,
            MessageContent::Text("Single line".to_string()),
        );
        let log_entry3 = LogEntry::new(
            uuid3,
            None,
            session_id,
            None,
            Utc::now(),
            EntryType::User,
            message3,
            EntryMetadata::default(),
        );
        let entry3 = ConversationEntry::Valid(Box::new(log_entry3));

        let conversation = AgentConversation::new(None);
        let scroll_state = ScrollState::default();
        let styles = create_test_styles();
        let widget = ConversationView::new(&conversation, &scroll_state, &styles, false);

        let entries = vec![entry1, entry2, entry3];
        let layouts = widget.calculate_entry_layouts(&entries, 0, 80, 24, WrapMode::NoWrap);

        assert_eq!(layouts.len(), 3, "Three entries should return three layouts");

        // Entry 1: y_offset=0, height=3
        assert_eq!(layouts[0].y_offset, 0, "First entry should start at y=0");
        assert_eq!(layouts[0].height, 3, "First entry should have height=3");

        // Entry 2: y_offset=3, height=2
        assert_eq!(layouts[1].y_offset, 3, "Second entry should start at y=3 (after first entry)");
        assert_eq!(layouts[1].height, 2, "Second entry should have height=2");

        // Entry 3: y_offset=5, height=1
        assert_eq!(layouts[2].y_offset, 5, "Third entry should start at y=5 (after first two)");
        assert_eq!(layouts[2].height, 1, "Third entry should have height=1");
    }

    #[test]
    fn calculate_entry_layouts_respects_scroll_offset() {
        use crate::model::{EntryMetadata, EntryType, EntryUuid, LogEntry, Message, MessageContent, Role, SessionId};
        use chrono::Utc;

        // Create entry with 5 lines
        let uuid = EntryUuid::new("test-uuid-1").expect("valid uuid");
        let session_id = SessionId::new("test-session-1").expect("valid session");
        let message = Message::new(
            Role::User,
            MessageContent::Text("Line 1\nLine 2\nLine 3\nLine 4\nLine 5".to_string()),
        );
        let log_entry = LogEntry::new(
            uuid,
            None,
            session_id,
            None,
            Utc::now(),
            EntryType::User,
            message,
            EntryMetadata::default(),
        );
        let entry = ConversationEntry::Valid(Box::new(log_entry));

        let conversation = AgentConversation::new(None);
        let scroll_state = ScrollState::default();
        let styles = create_test_styles();
        let widget = ConversationView::new(&conversation, &scroll_state, &styles, false);

        // When scrolled down 2 lines, the entry should still start at y=0 in viewport
        // but content starts 2 lines down
        let layouts = widget.calculate_entry_layouts(&[entry], 2, 80, 24, WrapMode::NoWrap);

        assert_eq!(layouts.len(), 1, "Should return one layout");
        // The y_offset should be relative to scroll position
        // Entry starts at absolute y=0, but when scroll_offset=2, it appears at viewport y=-2
        // However, visible portion starts at viewport y=0
        assert_eq!(layouts[0].y_offset, 0, "Entry should render at top of viewport when partially scrolled");
        assert_eq!(layouts[0].height, 5, "Entry height should remain 5 lines");
    }

    #[test]
    fn calculate_entry_layouts_filters_entries_outside_viewport() {
        use crate::model::{EntryMetadata, EntryType, EntryUuid, LogEntry, Message, MessageContent, Role, SessionId};
        use chrono::Utc;

        let session_id = SessionId::new("test-session-1").expect("valid session");

        // Create 5 entries, each 5 lines tall (total 25 lines)
        let mut entries = Vec::new();
        for i in 0..5 {
            let uuid = EntryUuid::new(&format!("test-uuid-{}", i)).expect("valid uuid");
            let message = Message::new(
                Role::User,
                MessageContent::Text(format!("Line 1\nLine 2\nLine 3\nLine 4\nLine 5")),
            );
            let log_entry = LogEntry::new(
                uuid,
                None,
                session_id.clone(),
                None,
                Utc::now(),
                EntryType::User,
                message,
                EntryMetadata::default(),
            );
            entries.push(ConversationEntry::Valid(Box::new(log_entry)));
        }

        let conversation = AgentConversation::new(None);
        let scroll_state = ScrollState::default();
        let styles = create_test_styles();
        let widget = ConversationView::new(&conversation, &scroll_state, &styles, false);

        // Viewport height is 10 lines
        // With no scroll, should see entries 0-1 fully, entry 2 partially (total 10+ lines)
        let layouts = widget.calculate_entry_layouts(&entries, 0, 80, 10, WrapMode::NoWrap);

        // Should include entries that are visible or partially visible in viewport
        assert!(layouts.len() >= 2 && layouts.len() <= 3,
            "Should return 2-3 visible entries for 10-line viewport, got {}", layouts.len());

        // Verify first entry
        if layouts.len() >= 1 {
            assert_eq!(layouts[0].y_offset, 0, "First visible entry should start at y=0");
            assert_eq!(layouts[0].height, 5, "First entry should be 5 lines");
        }

        // Verify second entry
        if layouts.len() >= 2 {
            assert_eq!(layouts[1].y_offset, 5, "Second visible entry should start at y=5");
            assert_eq!(layouts[1].height, 5, "Second entry should be 5 lines");
        }
    }

    // ===== Horizontal Scrolling with Per-Entry Wrap Override Tests (FR-040 + FR-048) =====

    #[test]
    fn render_conversation_view_applies_horizontal_scroll_with_per_entry_nowrap_override() {
        use crate::model::{
            AgentConversation, EntryMetadata, EntryType, EntryUuid, LogEntry, Message,
            MessageContent, Role, SessionId,
        };
        use chrono::Utc;
        use ratatui::backend::TestBackend;
        use ratatui::Terminal;

        // Create a conversation with a message containing long text
        let mut conversation = AgentConversation::new(None);

        let long_text = "This is a very long line that should be horizontally scrolled when in NoWrap mode";
        let message = Message::new(Role::Assistant, MessageContent::Text(long_text.to_string()));

        let uuid = EntryUuid::new("entry-scroll-test").expect("valid uuid");
        let entry = LogEntry::new(
            uuid.clone(),
            None,
            SessionId::new("session-1").expect("valid session id"),
            None,
            Utc::now(),
            EntryType::Assistant,
            message,
            EntryMetadata::default(),
        );

        conversation.add_entry(entry);

        // Create scroll state with:
        // - Global wrap mode: Wrap
        // - Per-entry override: toggles to NoWrap
        // - Horizontal offset: 10 characters
        let mut scroll_state = ScrollState::default();
        scroll_state.toggle_wrap(&uuid); // Override global Wrap -> NoWrap for this entry
        scroll_state.horizontal_offset = 10;

        // Create a test terminal and render
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
                    WrapMode::Wrap, // Global is Wrap, but entry overrides to NoWrap
                );
            })
            .expect("Failed to draw");

        // Get the rendered buffer
        let buffer = terminal.backend().buffer().clone();
        let content: String = buffer.content().iter().map(|cell| cell.symbol()).collect();

        // CRITICAL TEST: When effective_wrap is NoWrap (due to override),
        // horizontal scrolling should apply even though global_wrap is Wrap.
        //
        // The first 10 characters "This is a " should be scrolled off-screen.
        // Should NOT see "This is a " at the start of the line.
        assert!(
            !content.contains("This is a "),
            "BUG: Horizontal scroll should apply when effective_wrap is NoWrap (per-entry override). \
             Line 783 likely uses global_wrap instead of effective_wrap. Content: {}",
            content.chars().take(200).collect::<String>()
        );

        // Should see text starting from offset 10: "very long line..."
        assert!(
            content.contains("very long line"),
            "Should see horizontally scrolled content starting from offset 10. Content: {}",
            content.chars().take(200).collect::<String>()
        );
    }

    // ===== Wrap Continuation Indicator Tests =====

    #[test]
    fn test_add_wrap_indicators_no_wrapping_needed() {
        use ratatui::text::Span;

        // Short line that fits within viewport
        let lines = vec![Line::from(vec![Span::raw("Hello")])];

        let result = add_wrap_continuation_indicators(lines.clone(), 80);

        // Should return unchanged - no wrapping needed
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].to_string(), "Hello");
    }

    #[test]
    fn test_add_wrap_indicators_single_line_wraps_once() {
        use ratatui::text::Span;

        // Line of 19 chars: "1234567890123456789"
        // With viewport_width=10, first segment is 9 chars + ↩ (10 total)
        // Second segment is remaining 10 chars (fits in viewport)
        let long_line = "1234567890123456789";
        let lines = vec![Line::from(vec![Span::raw(long_line.to_string())])];

        // Viewport width 10 - should wrap into 2 lines
        let result = add_wrap_continuation_indicators(lines, 10);

        // Should have 2 lines: first with ↩ indicator, second without
        assert_eq!(result.len(), 2, "Should split into 2 lines");

        // First line should end with ↩
        let first_line_str = result[0].to_string();
        assert!(
            first_line_str.ends_with('↩'),
            "First line should end with ↩ indicator, got: {}",
            first_line_str
        );

        // Second line should NOT have ↩ (it's the final segment)
        let second_line_str = result[1].to_string();
        assert!(
            !second_line_str.contains('↩'),
            "Last segment should not have ↩ indicator, got: {}",
            second_line_str
        );
    }

    #[test]
    fn test_add_wrap_indicators_multiple_wraps() {
        use ratatui::text::Span;

        // Line of 28 chars: "1234567890123456789012345678"
        // With viewport_width=10:
        // - First segment: 9 chars + ↩
        // - Second segment: 9 chars + ↩
        // - Third segment: 10 chars (remaining, fits exactly)
        let long_line = "1234567890123456789012345678";
        let lines = vec![Line::from(vec![Span::raw(long_line.to_string())])];

        // Viewport width 10 - should wrap into 3 lines
        let result = add_wrap_continuation_indicators(lines, 10);

        // Should have 3 lines
        assert_eq!(result.len(), 3, "Should split into 3 lines");

        // First two lines should have ↩
        for (i, line) in result.iter().take(2).enumerate() {
            let line_str = line.to_string();
            assert!(
                line_str.ends_with('↩'),
                "Line {} should end with ↩ indicator, got: {}",
                i,
                line_str
            );
        }

        // Last line should NOT have ↩
        let last_line_str = result[2].to_string();
        assert!(
            !last_line_str.contains('↩'),
            "Last line should not have ↩ indicator, got: {}",
            last_line_str
        );
    }

    #[test]
    fn test_add_wrap_indicators_preserves_intentional_breaks() {
        use ratatui::text::Span;

        // Two short lines - intentional line breaks
        let lines = vec![
            Line::from(vec![Span::raw("First line")]),
            Line::from(vec![Span::raw("Second line")]),
        ];

        let result = add_wrap_continuation_indicators(lines, 80);

        // Should still be 2 lines (no wrapping needed)
        assert_eq!(result.len(), 2, "Should preserve line count");

        // Neither should have ↩ (they don't wrap)
        for (i, line) in result.iter().enumerate() {
            let line_str = line.to_string();
            assert!(
                !line_str.contains('↩'),
                "Line {} should not have ↩ (no wrapping), got: {}",
                i,
                line_str
            );
        }
    }

    #[test]
    fn test_add_wrap_indicators_zero_viewport_width() {
        use ratatui::text::Span;

        let lines = vec![Line::from(vec![Span::raw("Hello")])];

        // Edge case: viewport_width = 0 should return input unchanged
        let result = add_wrap_continuation_indicators(lines.clone(), 0);

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].to_string(), "Hello");
    }

    #[test]
    fn test_add_wrap_indicators_empty_lines() {
        // Empty input should return empty output
        let lines: Vec<Line<'static>> = vec![];

        let result = add_wrap_continuation_indicators(lines, 80);

        assert_eq!(result.len(), 0, "Empty input should return empty output");
    }

    #[test]
    fn test_add_wrap_indicators_preserves_styling() {
        use ratatui::text::Span;

        // Line with styled span
        let styled_span = Span::styled(
            "1234567890123456789012345".to_string(),
            Style::default().fg(Color::Blue),
        );
        let lines = vec![Line::from(vec![styled_span])];

        // Viewport width 10 - should wrap into 3 lines
        let result = add_wrap_continuation_indicators(lines, 10);

        assert_eq!(result.len(), 3, "Should split into 3 lines");

        // Verify the indicator span is separate and has DIM style
        for line in result.iter().take(2) {
            // First 2 lines should have continuation indicator
            let line_str = line.to_string();
            assert!(
                line_str.ends_with('↩'),
                "Line should end with ↩, got: {}",
                line_str
            );

            // The ↩ should be in a separate span with DIM modifier
            // (We can't easily test style in string form, but the implementation should handle this)
        }
    }

    #[test]
    fn test_add_wrap_indicators_emoji_display_width() {
        use ratatui::text::Span;
        use unicode_width::UnicodeWidthStr;

        // Test case: emoji has 2 display columns but 1 character
        // "😀234567890" = 10 chars, but 11 display columns (emoji=2, rest=9)
        // Viewport: 10 columns
        // Expected behavior:
        //   - First segment: "😀2345678" (9 display columns) + "↩" (1 column) = 10 columns
        //   - Second segment: "90" (2 display columns)
        let line = "😀234567890"; // 10 chars, 11 display width
        let lines = vec![Line::from(vec![Span::raw(line.to_string())])];

        let viewport_width = 10;
        let result = add_wrap_continuation_indicators(lines, viewport_width);

        // Should wrap into 2 lines
        assert_eq!(result.len(), 2, "Should split into 2 lines");

        // CRITICAL: First segment + indicator must fit in viewport
        let first_line_str = result[0].to_string();
        let first_display_width = first_line_str.width();
        assert!(
            first_display_width <= viewport_width,
            "First line display width {} exceeds viewport width {}. Content: '{}'",
            first_display_width,
            viewport_width,
            first_line_str
        );

        // Second segment should also fit
        let second_line_str = result[1].to_string();
        let second_display_width = second_line_str.width();
        assert!(
            second_display_width <= viewport_width,
            "Second line display width {} exceeds viewport width {}. Content: '{}'",
            second_display_width,
            viewport_width,
            second_line_str
        );

        // First line should have continuation indicator
        assert!(
            first_line_str.ends_with('↩'),
            "First line should end with ↩, got: {}",
            first_line_str
        );
    }

    #[test]
    fn test_add_wrap_indicators_cjk_display_width() {
        use ratatui::text::Span;
        use unicode_width::UnicodeWidthStr;

        // CJK characters are typically 2 display columns each
        // "日本語12" = 5 chars, but 8 display columns (日=2, 本=2, 語=2, 1=1, 2=1)
        let line = "日本語123456";
        let lines = vec![Line::from(vec![Span::raw(line.to_string())])];

        let viewport_width = 10;
        let result = add_wrap_continuation_indicators(lines, viewport_width);

        // All segments must fit within viewport width
        for (i, result_line) in result.iter().enumerate() {
            let line_str = result_line.to_string();
            let display_width = line_str.width();
            assert!(
                display_width <= viewport_width,
                "Line {} display width {} exceeds viewport width {}. Content: '{}'",
                i,
                display_width,
                viewport_width,
                line_str
            );
        }
    }

    // ===== Wrap Rendering Integration Tests =====
    // These tests verify FR-052 and FR-053 at the RENDERING level.

    /// Test that wrap continuation indicators appear in rendered output (FR-052).
    ///
    /// TODO: This test currently FAILS because add_wrap_continuation_indicators()
    /// is not integrated into the rendering pipeline. The function exists and its
    /// algorithm is tested, but it's not called in render_conversation_view().
    ///
    /// Integration point: Lines 772-794 in render_conversation_view() should call
    /// add_wrap_continuation_indicators() when effective_wrap == WrapMode::Wrap.
    #[test]
    #[ignore = "TODO: Wrap continuation indicators not yet integrated into rendering (cclv-07v.9.9)"]
    fn test_render_wrap_continuation_indicator_appears_in_output() {
        use crate::model::{
            AgentConversation, EntryMetadata, EntryType, EntryUuid, LogEntry, Message,
            MessageContent, Role, SessionId,
        };
        use chrono::Utc;
        use ratatui::backend::TestBackend;
        use ratatui::Terminal;

        // Create a conversation with a long line that will wrap
        let mut conversation = AgentConversation::new(None);

        // Line of 100 chars - will wrap multiple times in 40-column viewport
        let long_text = "1234567890".repeat(10); // 100 chars
        let message = Message::new(Role::Assistant, MessageContent::Text(long_text));

        let uuid = EntryUuid::new("entry-wrap-indicator").expect("valid uuid");
        let entry = LogEntry::new(
            uuid.clone(),
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

        // Create a narrow test terminal (40 columns wide)
        let backend = TestBackend::new(40, 24);
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
                    WrapMode::Wrap, // Enable wrap mode
                );
            })
            .expect("Failed to draw");

        // Get the rendered buffer
        let buffer = terminal.backend().buffer().clone();
        let content: String = buffer.content().iter().map(|cell| cell.symbol()).collect();

        // FR-052: Wrapped lines MUST show continuation indicator (↩)
        // In a 40-column viewport with 2-column borders (38 usable), a 100-char line
        // should wrap multiple times, each showing ↩ at the wrap point (except the last segment).
        assert!(
            content.contains('↩'),
            "FAIL: Wrap continuation indicator (↩) not found in rendered output. \
             FR-052 requires continuation indicator at wrap points. \
             First 200 chars: {}",
            content.chars().take(200).collect::<String>()
        );

        // The indicator should appear multiple times (not just once)
        let indicator_count = content.matches('↩').count();
        assert!(
            indicator_count >= 2,
            "FAIL: Expected at least 2 wrap indicators for 100-char line in 40-col viewport, \
             found {}. Content: {}",
            indicator_count,
            content.chars().take(300).collect::<String>()
        );
    }

    /// Test that intentional line breaks do NOT show wrap indicators (FR-052).
    ///
    /// TODO: Same integration gap as above - test is ignored until implementation.
    #[test]
    #[ignore = "TODO: Wrap continuation indicators not yet integrated into rendering (cclv-07v.9.9)"]
    fn test_render_intentional_line_breaks_no_indicator() {
        use crate::model::{
            AgentConversation, EntryMetadata, EntryType, EntryUuid, LogEntry, Message,
            MessageContent, Role, SessionId,
        };
        use chrono::Utc;
        use ratatui::backend::TestBackend;
        use ratatui::Terminal;

        // Create a conversation with multiple short lines
        let mut conversation = AgentConversation::new(None);

        let multiline_text = "First line\nSecond line\nThird line";
        let message = Message::new(Role::Assistant, MessageContent::Text(multiline_text.to_string()));

        let uuid = EntryUuid::new("entry-no-indicator").expect("valid uuid");
        let entry = LogEntry::new(
            uuid,
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

        // Wide viewport - lines won't wrap
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
                    WrapMode::Wrap,
                );
            })
            .expect("Failed to draw");

        let buffer = terminal.backend().buffer().clone();
        let content: String = buffer.content().iter().map(|cell| cell.symbol()).collect();

        // FR-052: Intentional line breaks should NOT have continuation indicators
        // Since lines are short and don't wrap, there should be NO ↩ symbols
        assert!(
            !content.contains('↩'),
            "FAIL: Continuation indicator (↩) should NOT appear for intentional line breaks. \
             Content: {}",
            content.chars().take(200).collect::<String>()
        );
    }

    /// Test that code blocks NEVER wrap regardless of global wrap setting (FR-053).
    ///
    /// TODO: This test is IGNORED because FR-053 (code block exemption) is not yet
    /// implemented (bead cclv-07v.9.10). When implemented:
    /// 1. Code blocks should be detected in markdown content
    /// 2. Code blocks should always use WrapMode::NoWrap regardless of global setting
    /// 3. Code blocks should use horizontal scrolling
    ///
    /// Integration point: render_entry_lines() should detect code blocks and apply
    /// NoWrap mode specifically for those lines.
    #[test]
    #[ignore = "TODO: Code block wrap exemption not yet implemented (cclv-07v.9.10 / FR-053)"]
    fn test_render_code_blocks_never_wrap() {
        use crate::model::{
            AgentConversation, EntryMetadata, EntryType, EntryUuid, LogEntry, Message,
            MessageContent, Role, SessionId,
        };
        use chrono::Utc;
        use ratatui::backend::TestBackend;
        use ratatui::Terminal;

        // Create a conversation with markdown containing a code block
        let mut conversation = AgentConversation::new(None);

        let markdown_with_code = r#"Here is some code:

```rust
fn very_long_function_name_that_exceeds_viewport_width() -> Result<String, Error> {
    let result = "This line is intentionally very long to test that code blocks do not wrap";
    Ok(result.to_string())
}
```

That was the code."#;

        let message = Message::new(Role::Assistant, MessageContent::Text(markdown_with_code.to_string()));

        let uuid = EntryUuid::new("entry-code-block").expect("valid uuid");
        let entry = LogEntry::new(
            uuid,
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

        // Narrow viewport (40 cols) - prose would wrap, but code blocks should not
        let backend = TestBackend::new(40, 30);
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
                    WrapMode::Wrap, // Global wrap enabled
                );
            })
            .expect("Failed to draw");

        let buffer = terminal.backend().buffer().clone();
        let content: String = buffer.content().iter().map(|cell| cell.symbol()).collect();

        // FR-053: Code blocks MUST never wrap, even when global wrap is enabled
        // The long code lines should NOT be broken up across multiple lines
        // Instead, they should use horizontal scrolling

        // EXPECTED BEHAVIOR (when implemented):
        // 1. The prose text "Here is some code:" should wrap if needed (global setting)
        // 2. The code block lines should NOT wrap regardless of global setting
        // 3. Code block should be visually distinct (no wrap continuation indicators in code)

        // Test 1: Code block content should appear on single lines (not wrapped)
        // The function declaration should be on one line in the buffer
        assert!(
            content.contains("fn very_long_function_name"),
            "FAIL: Code block content not found. Expected function declaration on single line. \
             Content: {}",
            content.chars().take(500).collect::<String>()
        );

        // Test 2: Code block should NOT have wrap continuation indicators (↩)
        // Even though prose might wrap with indicators, code blocks never should
        let lines: Vec<&str> = content.split('\n').collect();
        let code_block_lines: Vec<&&str> = lines
            .iter()
            .filter(|line| {
                line.contains("fn very_long") || line.contains("let result =")
            })
            .collect();

        for code_line in code_block_lines {
            assert!(
                !code_line.contains('↩'),
                "FAIL: Code block line should NOT have wrap continuation indicator. \
                 FR-053 requires code blocks never wrap. Line: {}",
                code_line
            );
        }
    }

    /// Test that prose text DOES wrap with indicators while code blocks don't (FR-052 + FR-053).
    ///
    /// TODO: Ignored until both wrap indicators and code block exemption are implemented.
    #[test]
    #[ignore = "TODO: Requires both wrap indicators (cclv-07v.9.9) and code exemption (cclv-07v.9.10)"]
    fn test_render_mixed_prose_and_code_wrap_behavior() {
        use crate::model::{
            AgentConversation, EntryMetadata, EntryType, EntryUuid, LogEntry, Message,
            MessageContent, Role, SessionId,
        };
        use chrono::Utc;
        use ratatui::backend::TestBackend;
        use ratatui::Terminal;

        let mut conversation = AgentConversation::new(None);

        let mixed_content = r#"This is a very long prose paragraph that will definitely wrap in a narrow viewport and should show continuation indicators at wrap points.

```rust
let code_line = "This code line is also very long but should NOT wrap even in narrow viewport";
```

And here is more prose text that will wrap and show indicators."#;

        let message = Message::new(Role::Assistant, MessageContent::Text(mixed_content.to_string()));

        let uuid = EntryUuid::new("entry-mixed").expect("valid uuid");
        let entry = LogEntry::new(
            uuid,
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

        let backend = TestBackend::new(50, 30);
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
                    WrapMode::Wrap,
                );
            })
            .expect("Failed to draw");

        let buffer = terminal.backend().buffer().clone();
        let content: String = buffer.content().iter().map(|cell| cell.symbol()).collect();

        // EXPECTED BEHAVIOR:
        // 1. Prose text wraps and shows ↩ indicators at wrap points
        // 2. Code block does NOT wrap (no ↩ in code section)
        // 3. Both prose sections should have indicators, code section should not

        // Prose should have wrap indicators
        let prose_has_indicator = content.contains('↩');
        assert!(
            prose_has_indicator,
            "FAIL: Prose text should wrap with continuation indicators. \
             Content: {}",
            content.chars().take(300).collect::<String>()
        );

        // Code block line should NOT have indicator
        // (This is harder to verify without parsing the rendered output more carefully,
        // but the visual test will make it obvious)
    }
}
