//! Conversation view widget - shared by main and subagent panes.
//!
//! Implements virtualized rendering to handle large conversations efficiently.
//! Only renders visible messages (plus ±20 buffer) based on scroll position.

use crate::model::{ContentBlock, ConversationEntry, MessageContent, ToolCall};
use crate::state::WrapMode;
use crate::view::MessageStyles;
use crate::view_state::conversation::ConversationViewState;
use crate::view_state::types::ViewportDimensions;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, Borders, Paragraph, Widget, Wrap},
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

// ===== Content Section =====

/// A section of content within a conversation entry.
///
/// Markdown content is split into sections to enable independent wrap behavior:
/// - Prose sections follow the configured wrap setting
/// - Code blocks never wrap (always horizontal scroll)
///
/// This enables FR-053: code blocks never wrap while prose wraps within the same entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContentSection {
    /// Prose text (paragraphs, headings, lists, etc.)
    ///
    /// Follows the effective wrap mode for the entry.
    Prose(Vec<Line<'static>>),

    /// Code block (fenced or indented)
    ///
    /// Never wraps regardless of wrap settings; always uses horizontal scrolling.
    CodeBlock(Vec<Line<'static>>),
}

/// A rendered section with type information and styled lines.
///
/// Used to preserve section type (prose vs code) after markdown rendering.
#[derive(Debug, Clone)]
#[allow(dead_code)] // Used in render_conversation_view refactoring
pub struct RenderedSection {
    /// Type of section (determines wrap behavior)
    pub section_type: SectionType,
    /// Rendered lines with markdown styling applied
    pub lines: Vec<Line<'static>>,
}

/// Section type classification for wrap behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)] // Used in render_conversation_view refactoring
pub enum SectionType {
    /// Prose content - respects wrap settings
    Prose,
    /// Code block - never wraps
    Code,
}

/// Render markdown text as sections with styling applied.
///
/// Splits markdown into prose and code sections, renders each through the markdown
/// renderer, and returns sections with type information preserved.
///
/// This enables FR-053: code blocks rendered as separate widgets with independent wrap settings.
///
/// # Arguments
/// * `markdown_text` - Raw markdown to render
/// * `base_style` - Base style to apply (typically role color)
///
/// # Returns
/// Vector of rendered sections preserving section type and order
#[allow(dead_code)] // Used in render_conversation_view refactoring
pub fn render_markdown_as_sections(markdown_text: &str, base_style: Style) -> Vec<RenderedSection> {
    use ratatui::text::{Line, Span};

    // Identify section boundaries in raw text
    let raw_sections = parse_raw_sections(markdown_text);

    // Render each section and tag with type
    raw_sections
        .into_iter()
        .map(|(section_type, text)| {
            let lines = match section_type {
                SectionType::Prose => {
                    // Prose: use markdown rendering with wrapping
                    render_markdown_with_style(&text, base_style)
                }
                SectionType::Code => {
                    // Code: render as plain text, never wrap
                    // Each line becomes a styled Line
                    text.lines()
                        .map(|line| Line::from(Span::styled(line.to_string(), base_style)))
                        .collect()
                }
            };
            RenderedSection {
                section_type,
                lines,
            }
        })
        .collect()
}

/// Parse raw markdown into (section_type, text) pairs.
///
/// Identifies code block boundaries without rendering.
///
/// # Arguments
/// * `content` - Raw markdown text
///
/// # Returns
/// Vector of (SectionType, String) pairs in original order
#[allow(dead_code)] // Used by render_markdown_as_sections
fn parse_raw_sections(content: &str) -> Vec<(SectionType, String)> {
    let mut sections = Vec::new();
    let mut current_prose = String::new();
    let mut current_code = String::new();

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum State {
        Prose,
        FencedCode,
        IndentedCode,
    }

    let mut state = State::Prose;
    let mut prev_line_blank = false;

    for line in content.lines() {
        let is_blank = line.trim().is_empty();
        let is_fence = line.trim_start().starts_with("```");
        let is_indented = line.starts_with("    ") || line.starts_with('\t');

        match state {
            State::Prose => {
                if is_fence {
                    // Flush current prose section
                    if !current_prose.is_empty() {
                        sections.push((SectionType::Prose, current_prose.clone()));
                        current_prose.clear();
                    }
                    // Include fence line in code block
                    current_code.push_str(line);
                    current_code.push('\n');
                    state = State::FencedCode;
                } else if is_indented && prev_line_blank && !is_blank {
                    // Start indented code block
                    if !current_prose.is_empty() {
                        sections.push((SectionType::Prose, current_prose.clone()));
                        current_prose.clear();
                    }
                    current_code.push_str(line);
                    current_code.push('\n');
                    state = State::IndentedCode;
                } else {
                    // Regular prose line
                    current_prose.push_str(line);
                    current_prose.push('\n');
                }
            }
            State::FencedCode => {
                current_code.push_str(line);
                current_code.push('\n');
                if is_fence {
                    // End of fenced code block
                    sections.push((SectionType::Code, current_code.clone()));
                    current_code.clear();
                    state = State::Prose;
                }
            }
            State::IndentedCode => {
                if is_indented && !is_blank {
                    // Continue indented code block
                    current_code.push_str(line);
                    current_code.push('\n');
                } else {
                    // End of indented code block
                    sections.push((SectionType::Code, current_code.clone()));
                    current_code.clear();
                    state = State::Prose;

                    // Process current line as prose
                    current_prose.push_str(line);
                    current_prose.push('\n');
                }
            }
        }

        prev_line_blank = is_blank;
    }

    // Flush remaining content
    if !current_prose.is_empty() {
        sections.push((SectionType::Prose, current_prose));
    }
    if !current_code.is_empty() {
        sections.push((SectionType::Code, current_code));
    }

    sections
}

/// Parse markdown content into sections (prose and code blocks).
///
/// Splits entry content to enable independent wrap behavior:
/// - Fenced code blocks (```) become CodeBlock sections
/// - Indented code blocks (4 spaces/tab after blank line) become CodeBlock sections
/// - All other content becomes Prose sections
/// - Adjacent prose lines are grouped into single Prose section
///
/// # Arguments
/// * `content` - Raw markdown text to parse
///
/// # Returns
/// Vector of content sections maintaining original order
///
/// NOTE: This function is deprecated in favor of `render_markdown_as_sections()`
/// which includes markdown rendering. Kept for backward compatibility with tests.
#[allow(dead_code)]
pub fn parse_entry_sections(content: &str) -> Vec<ContentSection> {
    let mut sections = Vec::new();
    let mut current_prose: Vec<Line<'static>> = Vec::new();
    let mut current_code: Vec<Line<'static>> = Vec::new();

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum State {
        Prose,
        FencedCode,
        IndentedCode,
    }

    let mut state = State::Prose;
    let mut prev_line_blank = false;

    for line in content.lines() {
        let is_blank = line.trim().is_empty();
        let is_fence = line.trim_start().starts_with("```");
        let is_indented = line.starts_with("    ") || line.starts_with('\t');

        match state {
            State::Prose => {
                if is_fence {
                    // Flush current prose section
                    if !current_prose.is_empty() {
                        sections.push(ContentSection::Prose(current_prose.clone()));
                        current_prose.clear();
                    }
                    state = State::FencedCode;
                } else if is_indented && prev_line_blank && !is_blank {
                    // Start indented code block (requires previous blank line)
                    if !current_prose.is_empty() {
                        sections.push(ContentSection::Prose(current_prose.clone()));
                        current_prose.clear();
                    }
                    // Strip leading indentation (4 spaces or 1 tab)
                    let stripped = line
                        .strip_prefix('\t')
                        .or_else(|| line.strip_prefix("    "))
                        .unwrap_or(line);
                    current_code.push(Line::from(stripped.to_string()));
                    state = State::IndentedCode;
                } else {
                    // Regular prose line
                    current_prose.push(Line::from(line.to_string()));
                }
            }
            State::FencedCode => {
                if is_fence {
                    // End of fenced code block
                    sections.push(ContentSection::CodeBlock(current_code.clone()));
                    current_code.clear();
                    state = State::Prose;
                } else {
                    // Inside fenced code block
                    current_code.push(Line::from(line.to_string()));
                }
            }
            State::IndentedCode => {
                if is_indented && !is_blank {
                    // Continue indented code block
                    let stripped = line
                        .strip_prefix('\t')
                        .or_else(|| line.strip_prefix("    "))
                        .unwrap_or(line);
                    current_code.push(Line::from(stripped.to_string()));
                } else {
                    // End of indented code block
                    sections.push(ContentSection::CodeBlock(current_code.clone()));
                    current_code.clear();
                    state = State::Prose;

                    // Process current line as prose
                    current_prose.push(Line::from(line.to_string()));
                }
            }
        }

        prev_line_blank = is_blank;
    }

    // Flush remaining content
    if !current_prose.is_empty() {
        sections.push(ContentSection::Prose(current_prose));
    }
    if !current_code.is_empty() {
        sections.push(ContentSection::CodeBlock(current_code));
    }

    sections
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
        let title = if let Some(agent_id) = self.view_state.agent_id() {
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
                let cumulative_y = entry_view.layout().cumulative_y().get();

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
#[allow(clippy::too_many_arguments)]
fn render_entry_lines(
    entry: &ConversationEntry,
    entry_index: usize,
    is_subagent_view: bool,
    is_expanded: bool,
    styles: &MessageStyles,
    collapse_threshold: usize,
    summary_lines: usize,
) -> Vec<Line<'static>> {
    render_entry_lines_with_search(
        entry,
        entry_index,
        is_subagent_view,
        is_expanded,
        &crate::state::SearchState::Inactive,
        styles,
        collapse_threshold,
        summary_lines,
    )
}

/// Format the entry index as a right-aligned prefix with separator.
///
/// Formats a 0-based entry index as a 1-based display number, right-aligned
/// in a 4-character column with a vertical separator.
///
/// # Arguments
/// * `entry_index` - 0-based index of the entry in the conversation
///
/// # Returns
/// Formatted string like "   1│", "  42│", " 999│" (right-aligned in 4 chars + separator)
///
/// # Examples
/// - `format_entry_index(0)` returns `"   1│"` (index 0 → display as 1)
/// - `format_entry_index(41)` returns `"  42│"` (index 41 → display as 42)
/// - `format_entry_index(998)` returns `" 999│"` (index 998 → display as 999)
fn format_entry_index(entry_index: usize) -> String {
    let display_num = entry_index + 1; // Convert 0-based to 1-based
    format!("{:>4}│", display_num)
}

/// Prepend the entry index to a line as a styled prefix.
///
/// Takes an existing Line and prepends the entry index with dim gray styling.
/// The index is formatted as a right-aligned number with separator (e.g., "   1│").
///
/// # Arguments
/// * `line` - The line to prepend the index to
/// * `entry_index` - 0-based index of the entry in the conversation
///
/// # Returns
/// A new Line with the index prepended as the first span
fn prepend_index_to_line(line: Line<'static>, entry_index: usize) -> Line<'static> {
    use ratatui::text::Span;

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

/// Render entry lines with search match highlighting applied.
///
/// Wraps the existing entry rendering logic and applies search highlighting
/// based on SearchState matches for the given entry.
///
/// # Arguments
/// * `entry` - The conversation entry to render
/// * `entry_index` - Index of this entry in the conversation (for initial prompt label)
/// * `is_subagent_view` - Whether this is being rendered in a subagent pane
/// * `scroll` - Scroll state (for expansion tracking)
/// * `search` - Search state (for match highlighting)
/// * `styles` - Message styling configuration
/// * `collapse_threshold` - Number of lines before collapsing
/// * `summary_lines` - Number of lines shown when collapsed
///
/// # Returns
/// Vector of Lines representing this entry with search highlighting, including spacing line at end
#[allow(clippy::too_many_arguments)]
fn render_entry_lines_with_search(
    entry: &ConversationEntry,
    entry_index: usize,
    is_subagent_view: bool,
    is_expanded: bool,
    search: &crate::state::SearchState,
    styles: &MessageStyles,
    collapse_threshold: usize,
    summary_lines: usize,
) -> Vec<Line<'static>> {
    use ratatui::text::Span;

    // Extract match information if search is active
    let match_info = match search {
        crate::state::SearchState::Active {
            matches,
            current_match,
            ..
        } => Some((matches, *current_match)),
        _ => None,
    };

    let mut lines = Vec::new();
    let mut index_added = false; // Track whether we've added the index to the first content line

    match entry {
        ConversationEntry::Valid(log_entry) => {
            let role = log_entry.message().role();
            let role_style = styles.style_for_role(role);

            // Add "Initial Prompt" label for first message in subagent view
            if is_subagent_view && entry_index == 0 {
                lines.push(Line::from(vec![Span::styled(
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

                    let should_collapse = total_lines > collapse_threshold && !is_expanded;

                    if should_collapse {
                        // Show summary lines with role styling (no search highlighting in collapsed view)
                        for (line_idx, line) in text_lines.iter().take(summary_lines).enumerate() {
                            let mut rendered_line =
                                Line::from(vec![Span::styled(line.to_string(), role_style)]);

                            // Add index to first content line
                            if line_idx == 0 && !index_added {
                                rendered_line = prepend_index_to_line(rendered_line, entry_index);
                                index_added = true;
                            }

                            lines.push(rendered_line);
                        }
                        // Add collapse indicator
                        let remaining = total_lines.saturating_sub(summary_lines);
                        lines.push(Line::from(vec![Span::styled(
                            format!("(+{} more lines)", remaining),
                            Style::default()
                                .fg(Color::DarkGray)
                                .add_modifier(Modifier::DIM),
                        )]));
                    } else {
                        // Show all lines - apply search highlighting if active
                        if let Some((matches, current_idx)) = &match_info {
                            // Get matches for this entry (block_index 0 for Text content)
                            let entry_matches: Vec<_> = matches
                                .iter()
                                .enumerate()
                                .filter_map(|(idx, m)| {
                                    if m.entry_uuid == *log_entry.uuid() && m.block_index == 0 {
                                        Some((m.char_offset, m.length, idx == *current_idx))
                                    } else {
                                        None
                                    }
                                })
                                .collect();

                            if entry_matches.is_empty() {
                                // No matches in this entry - render normally
                                for (line_idx, line) in text_lines.iter().enumerate() {
                                    let mut rendered_line = Line::from(vec![Span::styled(
                                        line.to_string(),
                                        role_style,
                                    )]);

                                    // Add index to first content line
                                    if line_idx == 0 && !index_added {
                                        rendered_line =
                                            prepend_index_to_line(rendered_line, entry_index);
                                        index_added = true;
                                    }

                                    lines.push(rendered_line);
                                }
                            } else {
                                // Apply highlighting - track cumulative offset for multi-line text
                                let mut cumulative_offset: usize = 0;
                                for (line_idx, line_text) in text_lines.iter().enumerate() {
                                    let line_start = cumulative_offset;
                                    let line_end = line_start.saturating_add(line_text.len());

                                    // Filter matches that overlap this line and convert to line-relative offsets
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
                                    let mut highlighted_line = apply_highlights_to_text(
                                        line_text,
                                        &line_matches,
                                        role_style,
                                    );

                                    // Add index to first content line
                                    if line_idx == 0 && !index_added {
                                        highlighted_line =
                                            prepend_index_to_line(highlighted_line, entry_index);
                                        index_added = true;
                                    }

                                    lines.push(highlighted_line);

                                    // Update cumulative offset (add line length + newline char)
                                    cumulative_offset = line_end.saturating_add(1);
                                }
                            }
                        } else {
                            // No search active - render normally
                            for (line_idx, line) in text_lines.iter().enumerate() {
                                let mut rendered_line =
                                    Line::from(vec![Span::styled(line.to_string(), role_style)]);

                                // Add index to first content line
                                if line_idx == 0 && !index_added {
                                    rendered_line =
                                        prepend_index_to_line(rendered_line, entry_index);
                                    index_added = true;
                                }

                                lines.push(rendered_line);
                            }
                        }
                    }
                }
                MessageContent::Blocks(blocks) => {
                    // Structured content - render each block
                    // TODO: Add search highlighting support for blocks (similar to Text handling)
                    for block in blocks {
                        let block_lines = render_content_block(
                            block,
                            log_entry.uuid(),
                            is_expanded,
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

    // Add spacing between entries
    lines.push(Line::from(""));

    lines
}

/// Render entry as sections for independent wrap control (FR-053).
///
/// Returns sections with type information preserved, enabling:
/// - Prose sections to respect wrap settings
///
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
    styles: &MessageStyles,
    collapse_threshold: usize,
    summary_lines: usize,
    wrap_mode: WrapMode,
) -> Paragraph<'static> {
    // Get lines from existing helper function
    // Dead code: use false for is_expanded (collapsed by default)
    let lines = render_entry_lines(
        entry,
        entry_index,
        is_subagent_view,
        false, // Dead code: collapsed by default
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

/// Apply horizontal offset to a line, preserving entry index prefix if present.
///
/// Skips the first `offset` characters from line content, but preserves the entry index
/// prefix (first span containing "│") if present. This ensures the index remains visible
/// during horizontal scrolling.
///
/// Returns a new Line with characters starting from `offset` position.
/// If offset exceeds line length, returns empty line (or just index if present).
///
/// Uses character-based indexing (not byte-based) for UTF-8 safety.
#[allow(dead_code)]
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
    _entry_uuid: &crate::model::EntryUuid,
    is_expanded: bool,
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
#[allow(clippy::too_many_arguments)]
pub fn render_tool_result(
    content: &str,
    is_error: bool,
    _entry_uuid: &crate::model::EntryUuid,
    is_expanded: bool,
    result_style: Style,
    collapse_threshold: usize,
    summary_lines: usize,
) -> Vec<Line<'static>> {
    use ratatui::text::Span;

    let mut lines = Vec::new();
    let content_lines: Vec<_> = content.lines().collect();
    let total_lines = content_lines.len();

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
#[allow(clippy::too_many_arguments)]
pub fn render_content_block(
    block: &ContentBlock,
    entry_uuid: &crate::model::EntryUuid,
    is_expanded: bool,
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
                false, // Dead code: collapsed by default
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
                false, // Dead code: collapsed by default
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
