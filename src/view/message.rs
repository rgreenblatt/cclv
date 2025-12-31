//! Conversation view widget - shared by main and subagent panes.
//!
//! Implements virtualized rendering to handle large conversations efficiently.
//! Only renders visible messages (plus ±20 buffer) based on scroll position.

use crate::model::{AgentConversation, ContentBlock, ConversationEntry, MessageContent, ToolCall};
use crate::state::{ScrollState, WrapMode};
use crate::view::MessageStyles;
use crate::view_state::conversation::ConversationViewState;
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

/// Apply search highlighting to rendered sections.
///
/// Takes sections with markdown styling already applied and overlays search match
/// highlighting while preserving section type tags for wrap behavior.
///
/// # Arguments
/// * `sections` - Rendered sections with styling applied
/// * `entry_uuid` - UUID of the entry being highlighted (to filter matches)
/// * `search` - Search state containing matches
///
/// # Returns
/// Sections with search highlighting applied to matching text
fn apply_search_to_sections(
    sections: Vec<RenderedSection>,
    entry_uuid: &crate::model::EntryUuid,
    search: &crate::state::SearchState,
) -> Vec<RenderedSection> {
    // Extract match information if search is active
    let match_info = match search {
        crate::state::SearchState::Active {
            matches,
            current_match,
            ..
        } => {
            // Filter matches for this entry (block_index 0 for text content)
            let entry_matches: Vec<_> = matches
                .iter()
                .enumerate()
                .filter_map(|(idx, m)| {
                    if m.entry_uuid == *entry_uuid && m.block_index == 0 {
                        Some((m.char_offset, m.length, idx == *current_match))
                    } else {
                        None
                    }
                })
                .collect();

            if entry_matches.is_empty() {
                None
            } else {
                Some(entry_matches)
            }
        }
        _ => None,
    };

    // If no matches, return sections unchanged
    let Some(entry_matches) = match_info else {
        return sections;
    };

    // Apply highlighting to each section while tracking cumulative char offset
    let mut cumulative_offset = 0_usize;
    let mut result_sections = Vec::new();

    for section in sections {
        // Calculate section text to determine its character range
        let section_text: String = section
            .lines
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");

        let section_start = cumulative_offset;
        let section_end = section_start + section_text.len();

        // Find matches that overlap this section
        let section_matches: Vec<(usize, usize, bool)> = entry_matches
            .iter()
            .filter_map(|(offset, length, is_current)| {
                let match_start = *offset;
                let match_end = match_start + *length;

                // Check if match overlaps this section
                if match_start < section_end && match_end > section_start {
                    // Convert to section-relative offset
                    let section_relative_start = match_start.saturating_sub(section_start);
                    let section_relative_end = (match_end - section_start).min(section_text.len());
                    let section_relative_length =
                        section_relative_end.saturating_sub(section_relative_start);

                    if section_relative_length > 0 {
                        Some((section_relative_start, section_relative_length, *is_current))
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect();

        // If no matches in this section, keep it unchanged
        if section_matches.is_empty() {
            result_sections.push(section);
            cumulative_offset = section_end + 1; // +1 for newline between sections
            continue;
        }

        // Apply highlighting to each line in the section
        let mut highlighted_lines = Vec::new();
        let mut line_offset = 0_usize;

        for line in &section.lines {
            // Extract line text and base style
            let line_text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
            let base_style = line.spans.first().map(|s| s.style).unwrap_or_default();

            let line_start = line_offset;
            let line_end = line_start + line_text.len();

            // Find matches that overlap this line
            let line_matches: Vec<(usize, usize, bool)> = section_matches
                .iter()
                .filter_map(|(offset, length, is_current)| {
                    let match_start = *offset;
                    let match_end = match_start + *length;

                    // Check if match overlaps this line
                    if match_start < line_end && match_end > line_start {
                        // Convert to line-relative offset
                        let line_relative_start = match_start.saturating_sub(line_start);
                        let line_relative_end = (match_end - line_start).min(line_text.len());
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

            // Apply highlighting to the line
            let highlighted_line = if line_matches.is_empty() {
                // No matches in this line - preserve original
                line.clone()
            } else {
                // Apply highlighting
                apply_highlights_to_text(&line_text, &line_matches, base_style)
            };

            highlighted_lines.push(highlighted_line);
            line_offset = line_end + 1; // +1 for newline
        }

        result_sections.push(RenderedSection {
            section_type: section.section_type,
            lines: highlighted_lines,
        });

        cumulative_offset = section_end + 1; // +1 for newline between sections
    }

    result_sections
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
    conversation: &'a AgentConversation,
    view_state: &'a ConversationViewState,
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
    /// * `view_state` - View state (for expansion tracking and entry iteration)
    /// * `scroll_state` - Scroll state (for scrolling offsets)
    /// * `styles` - Message styling configuration
    /// * `focused` - Whether this pane currently has focus (affects border color)
    pub fn new(
        conversation: &'a AgentConversation,
        view_state: &'a ConversationViewState,
        scroll_state: &'a ScrollState,
        styles: &'a MessageStyles,
        focused: bool,
    ) -> Self {
        Self {
            conversation,
            view_state,
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
        entry_index: usize,
        is_expanded: bool,
        viewport_width: usize,
        global_wrap: WrapMode,
        is_subagent_view: bool,
    ) -> usize {
        match entry {
            ConversationEntry::Valid(log_entry) => {
                // Get effective wrap mode from view-state (per-entry override)
                let effective_wrap = self
                    .view_state
                    .get(crate::view_state::types::EntryIndex::new(entry_index))
                    .map(|e| e.effective_wrap(global_wrap))
                    .unwrap_or(global_wrap);

                match log_entry.message().content() {
                    MessageContent::Text(text) => {
                        let mut total_lines = match effective_wrap {
                            WrapMode::Wrap => {
                                // Calculate wrapped line count
                                Self::calculate_wrapped_lines(text, viewport_width)
                            }
                            WrapMode::NoWrap => {
                                // Count newlines (original behavior)
                                text.lines().count().max(1) // At least 1 line for empty text
                            }
                        };

                        // Add "Initial Prompt" label line for first message in subagent view
                        if is_subagent_view && entry_index == 0 {
                            total_lines += 1;
                        }

                        if total_lines > self.collapse_threshold && !is_expanded {
                            // Collapsed: summary_lines + 1 indicator line + spacing
                            self.summary_lines + 1 + 1
                        } else {
                            // Expanded or not collapsible: content lines + spacing
                            total_lines + 1
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
                                is_expanded,
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
    ///
    /// # Visibility
    /// Public for property testing in integration tests.
    pub fn calculate_wrapped_lines(text: &str, viewport_width: usize) -> usize {
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
    #[allow(clippy::too_many_arguments)]
    fn calculate_entry_layouts(
        &self,
        visible_entries: &[ConversationEntry],
        start_idx: usize,
        scroll_offset: usize,
        viewport_width: usize,
        viewport_height: usize,
        global_wrap: WrapMode,
        is_subagent_view: bool,
    ) -> Vec<EntryLayout> {
        let mut layouts = Vec::new();
        let mut cumulative_y = 0_usize;

        for (idx, entry) in visible_entries.iter().enumerate() {
            let actual_entry_index = start_idx + idx;

            // Calculate height for this entry
            let is_expanded = entry
                .uuid()
                .map(|uuid| self.view_state.is_expanded_by_uuid(uuid))
                .unwrap_or(false);
            let height = self.calculate_entry_height(
                entry,
                actual_entry_index,
                is_expanded,
                viewport_width,
                global_wrap,
                is_subagent_view,
            );

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
        let is_subagent_view = self.conversation.agent_id().is_some();

        // Calculate which entry the scroll offset corresponds to
        let mut cumulative_height = 0;
        let mut start_entry_index = 0;

        // Find the first entry that should be visible (accounting for buffer)
        // Start rendering from buffer_size lines above viewport, or 0 if scroll < buffer
        let render_start_line = scroll_offset.saturating_sub(self.buffer_size);

        for (i, entry) in entries.iter().enumerate() {
            let is_expanded = entry
                .uuid()
                .map(|uuid| self.view_state.is_expanded_by_uuid(uuid))
                .unwrap_or(false);
            let entry_height = self.calculate_entry_height(
                entry,
                i,
                is_expanded,
                viewport_width,
                global_wrap,
                is_subagent_view,
            );

            // If this entry's bottom edge is past render_start_line, include it
            if cumulative_height + entry_height > render_start_line {
                start_entry_index = i;
                break;
            }
            cumulative_height = cumulative_height.saturating_add(entry_height);
        }

        // Find the last entry that should be visible (accounting for buffer)
        let mut end_entry_index = start_entry_index;
        cumulative_height = 0;

        for (i, entry) in entries.iter().enumerate().skip(start_entry_index) {
            let is_expanded = entry
                .uuid()
                .map(|uuid| self.view_state.is_expanded_by_uuid(uuid))
                .unwrap_or(false);
            let entry_height = self.calculate_entry_height(
                entry,
                i,
                is_expanded,
                viewport_width,
                global_wrap,
                is_subagent_view,
            );
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

            // Calculate absolute Y position of first visible entry
            let mut first_entry_absolute_y = 0_usize;
            for (idx, entry) in self.conversation.entries()[..start_index]
                .iter()
                .enumerate()
            {
                let is_expanded = entry
                    .uuid()
                    .map(|uuid| self.view_state.is_expanded_by_uuid(uuid))
                    .unwrap_or(false);
                first_entry_absolute_y += self.calculate_entry_height(
                    entry,
                    idx,
                    is_expanded,
                    viewport_width,
                    self.global_wrap,
                    self.is_subagent_view,
                );
            }

            let mut cumulative_y = first_entry_absolute_y;
            let scroll_offset = self.scroll_state.vertical_offset;

            // Render only the visible range
            for (visible_index, entry) in self.conversation.entries()[start_index..end_index]
                .iter()
                .enumerate()
            {
                // Calculate actual index in full entry list
                let actual_index = start_index + visible_index;

                // Calculate how many lines to skip from this entry if it's partially scrolled off
                let lines_to_skip = if cumulative_y < scroll_offset {
                    scroll_offset.saturating_sub(cumulative_y)
                } else {
                    0
                };

                let is_expanded = entry
                    .uuid()
                    .map(|uuid| self.view_state.is_expanded_by_uuid(uuid))
                    .unwrap_or(false);
                let entry_height = self.calculate_entry_height(
                    entry,
                    actual_index,
                    is_expanded,
                    viewport_width,
                    self.global_wrap,
                    self.is_subagent_view,
                );

                // Collect entry lines into temporary vector
                let mut entry_lines = Vec::new();

                match entry {
                    ConversationEntry::Valid(log_entry) => {
                        let role = log_entry.message().role();
                        let role_style = self.styles.style_for_role(role);

                        // Add "Initial Prompt" label for first message in subagent view
                        if self.is_subagent_view && actual_index == 0 {
                            entry_lines.push(Line::from(vec![ratatui::text::Span::styled(
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
                                    entry_lines.push(Line::from(vec![
                                        ratatui::text::Span::styled(line.to_string(), role_style),
                                    ]));
                                }
                            }
                            MessageContent::Blocks(blocks) => {
                                // Structured content - render each block
                                for block in blocks {
                                    let is_expanded =
                                        self.view_state.is_expanded_by_uuid(log_entry.uuid());
                                    let block_lines = render_content_block(
                                        block,
                                        log_entry.uuid(),
                                        is_expanded,
                                        self.scroll_state,
                                        self.styles,
                                        role_style,
                                        self.collapse_threshold,
                                        self.summary_lines,
                                    );
                                    entry_lines.extend(block_lines);
                                }
                            }
                        }
                    }
                    ConversationEntry::Malformed(malformed) => {
                        // Render malformed entry with error styling
                        // Header: "⚠ Parse Error (line N)"
                        entry_lines.push(Line::from(vec![ratatui::text::Span::styled(
                            "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━",
                            Style::default().fg(Color::Red),
                        )]));
                        entry_lines.push(Line::from(vec![ratatui::text::Span::styled(
                            format!("⚠ Parse Error (line {})", malformed.line_number()),
                            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                        )]));
                        // Error message
                        for error_line in malformed.error_message().lines() {
                            entry_lines.push(Line::from(vec![ratatui::text::Span::styled(
                                error_line.to_string(),
                                Style::default().fg(Color::Red),
                            )]));
                        }
                    }
                }

                // Add spacing between entries
                entry_lines.push(Line::from(""));

                // Skip lines that are scrolled off the top, then add to final lines
                lines.extend(entry_lines.into_iter().skip(lines_to_skip));

                // Update cumulative_y for next entry
                cumulative_y += entry_height;
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
    scroll: &ScrollState,
    styles: &MessageStyles,
    collapse_threshold: usize,
    summary_lines: usize,
) -> Vec<Line<'static>> {
    render_entry_lines_with_search(
        entry,
        entry_index,
        is_subagent_view,
        is_expanded,
        scroll,
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
    scroll: &ScrollState,
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
/// - Code blocks to never wrap (always horizontal scroll)
///
/// # Arguments
/// * `entry` - The conversation entry to render
/// * `entry_index` - Index of this entry (for initial prompt label)
/// * `is_subagent_view` - Whether in subagent pane (affects first entry labeling)
/// * `scroll` - Scroll state (for expansion tracking)
/// * `styles` - Message styling configuration
/// * `collapse_threshold` - Number of lines before collapsing
/// * `summary_lines` - Number of lines shown when collapsed
///
/// # Returns
/// Vector of RenderedSection with type tags and styled lines
#[allow(clippy::too_many_arguments)]
fn render_entry_as_sections(
    entry: &ConversationEntry,
    entry_index: usize,
    is_subagent_view: bool,
    is_expanded: bool,
    scroll: &ScrollState,
    styles: &MessageStyles,
    collapse_threshold: usize,
    summary_lines: usize,
) -> Vec<RenderedSection> {
    use ratatui::text::Span;

    let mut sections = Vec::new();

    match entry {
        ConversationEntry::Valid(log_entry) => {
            let role = log_entry.message().role();
            let role_style = styles.style_for_role(role);

            // Add "Initial Prompt" label for first message in subagent view as separate section
            if is_subagent_view && entry_index == 0 {
                sections.push(RenderedSection {
                    section_type: SectionType::Prose,
                    lines: vec![Line::from(vec![Span::styled(
                        "🔷 Initial Prompt",
                        Style::default()
                            .fg(Color::Magenta)
                            .add_modifier(Modifier::BOLD),
                    )])],
                });
            }

            match log_entry.message().content() {
                MessageContent::Text(text) => {
                    // Simple text - no section parsing needed, treat as single prose section
                    let text_lines: Vec<_> = text.lines().collect();
                    let total_lines = text_lines.len();

                    let should_collapse = total_lines > collapse_threshold && !is_expanded;

                    let mut lines = Vec::new();
                    if should_collapse {
                        for line in text_lines.iter().take(summary_lines) {
                            lines
                                .push(Line::from(vec![Span::styled(line.to_string(), role_style)]));
                        }
                        let remaining = total_lines.saturating_sub(summary_lines);
                        lines.push(Line::from(vec![Span::styled(
                            format!("(+{} more lines)", remaining),
                            Style::default()
                                .fg(Color::DarkGray)
                                .add_modifier(Modifier::DIM),
                        )]));
                    } else {
                        for line in text_lines {
                            lines
                                .push(Line::from(vec![Span::styled(line.to_string(), role_style)]));
                        }
                    }

                    sections.push(RenderedSection {
                        section_type: SectionType::Prose,
                        lines,
                    });
                }
                MessageContent::Blocks(blocks) => {
                    // Structured content - render each block as sections
                    for block in blocks {
                        let block_sections = render_content_block_as_sections(
                            block,
                            log_entry.uuid(),
                            is_expanded,
                            scroll,
                            styles,
                            role_style,
                            collapse_threshold,
                            summary_lines,
                        );
                        sections.extend(block_sections);
                    }
                }
            }
        }
        ConversationEntry::Malformed(malformed) => {
            // Malformed entries are always prose (error messages)
            let mut lines = Vec::new();
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

            sections.push(RenderedSection {
                section_type: SectionType::Prose,
                lines,
            });
        }
    }

    // Add spacing section at end
    sections.push(RenderedSection {
        section_type: SectionType::Prose,
        lines: vec![Line::from("")],
    });

    sections
}

/// Flatten sections to lines (discards section type information).
///
/// Used when rendering with the per-entry Paragraph approach.
/// For section-aware rendering, use sections directly.
fn flatten_sections_to_lines(sections: Vec<RenderedSection>) -> Vec<Line<'static>> {
    sections.into_iter().flat_map(|s| s.lines).collect()
}

/// Render entry as sections with search highlighting applied.
///
/// Combines section-based rendering (for independent wrap control) with search highlighting.
/// This is the primary entry rendering function when search is active.
///
/// # Arguments
/// * `entry` - The conversation entry to render
/// * `entry_index` - Index of this entry (for initial prompt label)
/// * `is_subagent_view` - Whether in subagent pane (affects first entry labeling)
/// * `scroll` - Scroll state (for expansion tracking)
/// * `search` - Search state (for match highlighting)
/// * `styles` - Message styling configuration
/// * `collapse_threshold` - Number of lines before collapsing
/// * `summary_lines` - Number of lines shown when collapsed
///
/// # Returns
/// Vector of RenderedSection with type tags, styled lines, and search highlighting
#[allow(clippy::too_many_arguments)]
fn render_entry_as_sections_with_search(
    entry: &ConversationEntry,
    entry_index: usize,
    is_subagent_view: bool,
    scroll: &ScrollState,
    view_state: &ConversationViewState,
    search: &crate::state::SearchState,
    styles: &MessageStyles,
    collapse_threshold: usize,
    summary_lines: usize,
) -> Vec<RenderedSection> {
    // Determine if entry is expanded
    let is_expanded = if let ConversationEntry::Valid(log_entry) = entry {
        view_state.is_expanded_by_uuid(log_entry.uuid())
    } else {
        false
    };

    // First render as sections (without search highlighting)
    let sections = render_entry_as_sections(
        entry,
        entry_index,
        is_subagent_view,
        is_expanded,
        scroll,
        styles,
        collapse_threshold,
        summary_lines,
    );

    // Then apply search highlighting if this is a valid entry
    match entry {
        ConversationEntry::Valid(log_entry) => {
            apply_search_to_sections(sections, log_entry.uuid(), search)
        }
        ConversationEntry::Malformed(_) => {
            // Malformed entries don't have search highlighting
            sections
        }
    }
}

/// Render a content block as sections (FR-053).
///
/// For ContentBlock::Text with markdown, parses into prose/code sections.
/// Other block types render as single prose sections.
///
/// # Returns
/// Vector of RenderedSection preserving section types
#[allow(clippy::too_many_arguments)]
fn render_content_block_as_sections(
    block: &ContentBlock,
    entry_uuid: &crate::model::EntryUuid,
    is_expanded: bool,
    scroll_state: &ScrollState,
    styles: &MessageStyles,
    role_style: Style,
    collapse_threshold: usize,
    summary_lines: usize,
) -> Vec<RenderedSection> {
    use ratatui::text::Span;

    match block {
        ContentBlock::Text { text } => {
            // Parse and render as sections for independent wrap control
            let rendered_sections = render_markdown_as_sections(text, role_style);

            // Apply collapse logic if needed
            let total_lines: usize = rendered_sections.iter().map(|s| s.lines.len()).sum();
            let should_collapse = total_lines > collapse_threshold && !is_expanded;

            if should_collapse {
                // Take first `summary_lines` worth of content
                let mut collapsed_lines = Vec::new();
                let mut lines_taken = 0;

                for section in &rendered_sections {
                    for line in &section.lines {
                        if lines_taken < summary_lines {
                            collapsed_lines.push(line.clone());
                            lines_taken += 1;
                        } else {
                            break;
                        }
                    }
                    if lines_taken >= summary_lines {
                        break;
                    }
                }

                // Add collapse indicator
                collapsed_lines.push(Line::from(vec![Span::styled(
                    format!(
                        "(+{} more lines)",
                        total_lines.saturating_sub(summary_lines)
                    ),
                    Style::default()
                        .fg(Color::DarkGray)
                        .add_modifier(Modifier::DIM),
                )]));

                // Return as single prose section when collapsed
                vec![RenderedSection {
                    section_type: SectionType::Prose,
                    lines: collapsed_lines,
                }]
            } else {
                // Return all sections when expanded
                rendered_sections
            }
        }
        ContentBlock::ToolUse(tool_call) => {
            // Tool calls are always prose (no code blocks in tool definitions)
            let block_style = styles.style_for_content_block(block);
            let tool_style = block_style.unwrap_or(role_style);
            let lines = render_tool_use(
                tool_call,
                entry_uuid,
                is_expanded,
                scroll_state,
                tool_style,
                collapse_threshold,
                summary_lines,
            );

            vec![RenderedSection {
                section_type: SectionType::Prose,
                lines,
            }]
        }
        ContentBlock::ToolResult {
            tool_use_id: _,
            content,
            is_error,
        } => {
            // Tool results might contain code, parse as sections
            let block_style = styles.style_for_content_block(block);
            let result_style = block_style.unwrap_or(role_style);

            // For error results, render as single prose section
            // For normal results, parse markdown for sections
            if *is_error {
                let mut lines = Vec::new();
                lines.push(Line::from(vec![Span::styled(
                    "⚠ Tool Error",
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                )]));
                for line in content.lines() {
                    lines.push(Line::from(vec![Span::styled(
                        line.to_string(),
                        result_style,
                    )]));
                }

                vec![RenderedSection {
                    section_type: SectionType::Prose,
                    lines,
                }]
            } else {
                // Normal tool result - might contain code blocks
                render_markdown_as_sections(content, result_style)
            }
        }
        ContentBlock::Thinking { thinking } => {
            // Thinking blocks might contain code, parse as sections
            render_markdown_as_sections(thinking, role_style)
        }
    }
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
    scroll: &ScrollState,
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
    // Dead code: use empty view-state (all entries collapsed)
    let empty_view_state = crate::view_state::conversation::ConversationViewState::empty();
    let temp_view = ConversationView::new(conversation, &empty_view_state, scroll, styles, focused)
        .global_wrap(global_wrap);

    // Calculate visible entry range
    let (start_idx, end_idx) =
        temp_view.calculate_visible_range(viewport_height, viewport_width, global_wrap);

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
                false, // Dead code: collapsed by default
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
        start_idx,
        scroll.vertical_offset,
        viewport_width,
        viewport_height,
        global_wrap,
        is_subagent_view,
    );

    // Calculate absolute cumulative_y for first visible entry
    // (sum of heights of all entries before start_idx)
    let mut first_entry_absolute_y = 0_usize;
    for (idx, entry) in all_entries[..start_idx].iter().enumerate() {
        first_entry_absolute_y += temp_view.calculate_entry_height(
            entry,
            idx,
            false, // Dead code: collapsed by default
            viewport_width,
            global_wrap,
            is_subagent_view,
        );
    }

    // Render each visible entry as a separate Paragraph
    // Track cumulative_y to detect entries partially scrolled off top
    let mut cumulative_y = first_entry_absolute_y;
    for (layout_idx, layout) in layouts.iter().enumerate() {
        let entry = &visible_entries[layout_idx];
        let actual_entry_index = start_idx + layout_idx;

        // Get per-entry effective wrap mode
        // FR-053: Per-entry wrap setting influences section-level rendering
        // NOTE: Dead code - using global_wrap only (no per-entry overrides)
        let effective_wrap = global_wrap;

        // Calculate lines to skip for clipping
        let lines_to_skip = if layout.y_offset == 0 {
            calculate_lines_to_skip(cumulative_y, scroll.vertical_offset)
        } else {
            0
        };
        let visible_height = (layout.height as usize).saturating_sub(lines_to_skip) as u16;

        // FR-053: Section-aware rendering when wrap enabled
        // Render each section (prose/code) as separate Paragraph with independent wrap
        if effective_wrap == WrapMode::Wrap {
            // Get entry as sections
            let entry_sections = render_entry_as_sections(
                entry,
                actual_entry_index,
                is_subagent_view,
                false, // Dead code: collapsed by default
                scroll,
                styles,
                10, // Default collapse threshold
                3,  // Default summary lines
            );

            // Stack sections vertically, applying wrap per section type
            let mut section_y_offset = 0_u16;
            let mut lines_skipped_so_far = 0_usize;

            for section in entry_sections {
                let mut section_lines = section.lines;

                // Apply section-specific transformations
                match section.section_type {
                    SectionType::Prose => {
                        // Prose: add wrap continuation indicators (FR-052)
                        section_lines = add_wrap_continuation_indicators(
                            section_lines,
                            inner_area.width as usize,
                        );
                    }
                    SectionType::Code => {
                        // Code: apply horizontal offset for scrolling (never wrap)
                        if horizontal_offset > 0 {
                            section_lines = section_lines
                                .into_iter()
                                .map(|line| apply_horizontal_offset(line, horizontal_offset))
                                .collect();
                        }
                    }
                }

                // Handle clipping for sections scrolled off top
                if lines_skipped_so_far < lines_to_skip {
                    let to_skip_in_section =
                        (lines_to_skip - lines_skipped_so_far).min(section_lines.len());
                    lines_skipped_so_far += to_skip_in_section;
                    section_lines = section_lines.into_iter().skip(to_skip_in_section).collect();
                }

                if section_lines.is_empty() {
                    continue; // Section fully clipped
                }

                // Create Paragraph with section-specific wrap setting
                let section_paragraph = match section.section_type {
                    SectionType::Prose => {
                        Paragraph::new(section_lines.clone()).wrap(Wrap { trim: false })
                    }
                    SectionType::Code => {
                        // Code blocks: render without wrapping
                        // Each line is pre-formatted, shown as-is
                        Paragraph::new(section_lines.clone())
                    }
                };

                // Calculate section height (will be adjusted by Paragraph wrapping)
                let section_height = section_lines.len() as u16;

                // Calculate section area within entry
                let section_area = Rect {
                    x: inner_area.x,
                    y: inner_area.y + layout.y_offset + section_y_offset,
                    width: inner_area.width,
                    height: section_height.min(visible_height.saturating_sub(section_y_offset)),
                };

                if section_area.height > 0 {
                    frame.render_widget(section_paragraph, section_area);
                }

                section_y_offset += section_height;

                // Stop if we've filled the entry area
                if section_y_offset >= visible_height {
                    break;
                }
            }
        } else {
            // NoWrap mode: use existing line-based rendering (FR-040)
            let mut entry_lines = render_entry_lines(
                entry,
                actual_entry_index,
                is_subagent_view,
                false, // Dead code: collapsed by default
                scroll,
                styles,
                10,
                3,
            );

            // Apply horizontal offset
            if horizontal_offset > 0 {
                entry_lines = entry_lines
                    .into_iter()
                    .map(|line| apply_horizontal_offset(line, horizontal_offset))
                    .collect();
            }

            // Clip lines
            if lines_to_skip > 0 {
                entry_lines = entry_lines.into_iter().skip(lines_to_skip).collect();
            }

            // Create single Paragraph (no wrap)
            let entry_paragraph = Paragraph::new(entry_lines);

            let entry_area = Rect {
                x: inner_area.x,
                y: inner_area.y + layout.y_offset,
                width: inner_area.width,
                height: visible_height,
            };

            frame.render_widget(entry_paragraph, entry_area);
        }

        // Update cumulative_y for next iteration
        cumulative_y += layout.height as usize;
    }
}

/// Render a conversation view with search match highlighting.
///
/// Uses per-entry Paragraph rendering architecture (matching render_conversation_view)
/// with search highlighting applied via render_entry_lines_with_search.
///
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
    view_state: &ConversationViewState,
    scroll: &ScrollState,
    search: &crate::state::SearchState,
    styles: &MessageStyles,
    focused: bool,
    global_wrap: WrapMode,
) {
    let entry_count = view_state.len();

    // Build title with model info
    let model_info = view_state
        .model_name()
        .map(|m| format!(" [{}]", m))
        .unwrap_or_default();
    let title = format!("Conversation{} ({} entries)", model_info, entry_count);

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

    // Calculate viewport dimensions
    let viewport_width = area.width.saturating_sub(2) as usize;
    let viewport_height = area.height.saturating_sub(2) as usize;

    // Get all entry views for rendering
    let all_entries = view_state.entries();

    // TODO: Determine subagent vs main from context (need agent_id in view-state)
    let is_subagent_view = false;

    // Create temporary ConversationView to use helper methods
    // TODO: Remove ConversationView entirely, use view_state methods directly
    let empty_conv = crate::model::AgentConversation::new(None);
    let temp_view = ConversationView::new(&empty_conv, view_state, scroll, styles, focused)
        .global_wrap(global_wrap);

    // Extract domain entries from entry views for backward-compat with old rendering code
    let domain_entries: Vec<_> = all_entries.iter().map(|ev| ev.entry().clone()).collect();

    // Calculate visible entry range
    // Note: temp_view uses an empty conversation for backward-compat, so calculate_visible_range
    // returns (0,0). Instead, render from view_state which has the actual data.
    // Limit to a reasonable number of entries to avoid rendering too much content.
    let (start_idx, end_idx) = if domain_entries.is_empty() {
        (0, 0)
    } else {
        // Estimate ~3 lines per entry, render enough to fill viewport + buffer
        let estimated_entries = (viewport_height / 3).max(10);
        (0, domain_entries.len().min(estimated_entries))
    };

    let visible_entries = &domain_entries[start_idx..end_idx];

    // Determine scroll indicators and horizontal offset (FR-040)
    let horizontal_offset = scroll.horizontal_offset;
    let title_with_indicators = if global_wrap == WrapMode::NoWrap {
        // Collect all lines temporarily to check for scroll indicators
        let mut all_lines = Vec::new();
        for (idx, entry) in visible_entries.iter().enumerate() {
            let actual_entry_index = start_idx + idx;
            let sections = render_entry_as_sections_with_search(
                entry,
                actual_entry_index,
                is_subagent_view,
                scroll,
                view_state,
                search,
                styles,
                10,
                3,
            );
            let entry_lines = flatten_sections_to_lines(sections);
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
        start_idx,
        scroll.vertical_offset,
        viewport_width,
        viewport_height,
        global_wrap,
        is_subagent_view,
    );

    // Calculate absolute cumulative_y for first visible entry
    let mut first_entry_absolute_y = 0_usize;
    for (idx, entry) in domain_entries[..start_idx].iter().enumerate() {
        first_entry_absolute_y += temp_view.calculate_entry_height(
            entry,
            idx,
            false, // Dead code: collapsed by default
            viewport_width,
            global_wrap,
            is_subagent_view,
        );
    }

    // Render each visible entry as a separate Paragraph
    let mut cumulative_y = first_entry_absolute_y;
    for (layout_idx, layout) in layouts.iter().enumerate() {
        let entry = &visible_entries[layout_idx];
        let actual_entry_index = start_idx + layout_idx;

        // Get per-entry effective wrap mode from view-state
        // FR-053: Code blocks never wrap, always use horizontal scroll
        let effective_wrap = if has_code_blocks(&extract_entry_text(entry)) {
            WrapMode::NoWrap
        } else {
            view_state
                .get(crate::view_state::types::EntryIndex::new(
                    actual_entry_index,
                ))
                .map(|e| e.effective_wrap(global_wrap))
                .unwrap_or(global_wrap)
        };

        // Get entry lines with search highlighting (section-based rendering)
        let sections = render_entry_as_sections_with_search(
            entry,
            actual_entry_index,
            is_subagent_view,
            scroll,
            view_state,
            search,
            styles,
            10, // Default collapse threshold
            3,  // Default summary lines
        );
        let mut entry_lines = flatten_sections_to_lines(sections);

        // Apply horizontal offset if NoWrap mode and offset > 0 (FR-040)
        if effective_wrap == WrapMode::NoWrap && horizontal_offset > 0 {
            entry_lines = entry_lines
                .into_iter()
                .map(|line| apply_horizontal_offset(line, horizontal_offset))
                .collect();
        }

        // Add wrap continuation indicators if Wrap mode (FR-052)
        if effective_wrap == WrapMode::Wrap {
            entry_lines = add_wrap_continuation_indicators(entry_lines, inner_area.width as usize);
        }

        // Clip lines that are scrolled off the top of the viewport
        let lines_to_skip = if layout.y_offset == 0 {
            calculate_lines_to_skip(cumulative_y, scroll.vertical_offset)
        } else {
            0
        };

        // Skip the clipped lines and adjust height
        if lines_to_skip > 0 {
            entry_lines = entry_lines.into_iter().skip(lines_to_skip).collect();
        }
        let visible_height = (layout.height as usize).saturating_sub(lines_to_skip) as u16;

        // Create Paragraph with appropriate wrap setting
        let entry_paragraph = match effective_wrap {
            WrapMode::Wrap => Paragraph::new(entry_lines).wrap(Wrap { trim: false }),
            WrapMode::NoWrap => Paragraph::new(entry_lines),
        };

        // Calculate entry area within viewport
        // CRITICAL: Clamp y coordinate to prevent buffer bounds violations
        // layout.y_offset can equal viewport_height when entry starts at bottom edge,
        // which would write to y coordinate beyond buffer bounds (height-1)
        let entry_y = inner_area.y + layout.y_offset;
        let entry_y_clamped = entry_y.min(inner_area.y + inner_area.height.saturating_sub(1));

        // Also clamp height to ensure entry doesn't extend beyond inner_area
        let max_height = inner_area.height.saturating_sub(layout.y_offset);
        let entry_height = visible_height.min(max_height);

        let entry_area = Rect {
            x: inner_area.x,
            y: entry_y_clamped,
            width: inner_area.width,
            height: entry_height,
        };

        frame.render_widget(entry_paragraph, entry_area);

        // Update cumulative_y for next iteration
        cumulative_y += layout.height as usize;
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
    _scroll_state: &ScrollState,
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
    _scroll_state: &ScrollState,
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
                false, // Dead code: collapsed by default
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
