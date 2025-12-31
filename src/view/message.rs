//! Conversation view widget - shared by main and subagent panes.
//!
//! PLACEHOLDER: This is a minimal implementation showing agent info.
//! Full conversation rendering (messages, markdown, syntax highlighting)
//! will be implemented in bead cclv-07v.4.2.

use crate::model::{AgentConversation, ContentBlock, ToolCall};
use crate::state::ScrollState;
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, Borders, Paragraph},
    Frame,
};

/// Render a conversation view for either main agent or subagent.
///
/// This is the shared widget used by both panes. It takes an AgentConversation
/// reference and renders it consistently regardless of which pane it's in.
///
/// # Arguments
/// * `frame` - The ratatui frame to render into
/// * `area` - The area to render within
/// * `conversation` - The agent conversation to display
/// * `_scroll` - Scroll state (unused in placeholder, prefix with _ to avoid warning)
/// * `focused` - Whether this pane currently has focus (affects border color)
pub fn render_conversation_view(
    frame: &mut Frame,
    area: Rect,
    conversation: &AgentConversation,
    _scroll: &ScrollState,
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

    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .style(Style::default().fg(border_color));

    // Placeholder content
    let placeholder_text = if entry_count == 0 {
        "No messages yet...".to_string()
    } else {
        format!("Conversation with {} messages", entry_count)
    };

    let paragraph = Paragraph::new(placeholder_text).block(block);
    frame.render_widget(paragraph, area);
}

// ===== Content Block Rendering =====

/// Render a ContentBlock::ToolUse as formatted lines with collapse/expand support.
///
/// Displays:
/// - Tool name as header (always visible)
/// - Tool input/parameters (collapsible if exceeds threshold)
/// - When collapsed: Shows first `summary_lines` + "(+N more lines)" indicator
/// - When expanded: Shows all parameter lines
///
/// # Arguments
/// * `tool_call` - The tool call to render
/// * `entry_uuid` - UUID of the entry (for expansion state lookup)
/// * `scroll_state` - Scroll state containing expansion tracking
/// * `collapse_threshold` - Number of lines before collapsing (default: 10)
/// * `summary_lines` - Number of lines to show when collapsed (default: 3)
///
/// # Returns
/// Vector of ratatui `Line` objects representing the rendered tool use block
pub fn render_tool_use(
    tool_call: &ToolCall,
    entry_uuid: &crate::model::EntryUuid,
    scroll_state: &ScrollState,
    collapse_threshold: usize,
    summary_lines: usize,
) -> Vec<Line<'static>> {
    use ratatui::text::Span;

    let mut lines = Vec::new();

    // Tool name header (always visible)
    let tool_name = tool_call.name().as_str();
    let header = format!("🔧 Tool: {}", tool_name);
    lines.push(Line::from(vec![Span::styled(
        header,
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
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
/// - Error styling (red) when is_error=true
/// - When collapsed: Shows first `summary_lines` + "(+N more lines)" indicator
/// - When expanded: Shows all output lines
///
/// # Arguments
/// * `content` - The tool result content string
/// * `is_error` - Whether this result represents an error
/// * `entry_uuid` - UUID of the entry (for expansion state lookup)
/// * `scroll_state` - Scroll state containing expansion tracking
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

    // Render the visible lines with error styling if needed
    for line in content_lines.iter().take(lines_to_show) {
        let rendered_line = if is_error {
            Line::from(vec![Span::styled(
                line.to_string(),
                Style::default().fg(Color::Red),
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
/// * `collapse_threshold` - Number of lines before collapsing (default: 10)
/// * `summary_lines` - Number of lines to show when collapsed (default: 3)
///
/// # Returns
/// Vector of ratatui `Line` objects representing the rendered block
pub fn render_content_block(
    block: &ContentBlock,
    entry_uuid: &crate::model::EntryUuid,
    scroll_state: &ScrollState,
    collapse_threshold: usize,
    summary_lines: usize,
) -> Vec<Line<'static>> {
    use ratatui::text::Span;

    match block {
        ContentBlock::Text { text } => text
            .lines()
            .map(|line| Line::from(line.to_string()))
            .collect(),
        ContentBlock::ToolUse(tool_call) => {
            render_tool_use(tool_call, entry_uuid, scroll_state, collapse_threshold, summary_lines)
        }
        ContentBlock::ToolResult {
            tool_use_id: _,
            content,
            is_error,
        } => render_tool_result(
            content,
            *is_error,
            entry_uuid,
            scroll_state,
            collapse_threshold,
            summary_lines,
        ),
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

    // ===== Helper: Create test scroll state =====

    fn create_test_scroll_state() -> ScrollState {
        ScrollState::default()
    }

    fn create_expanded_scroll_state(uuid: &crate::model::EntryUuid) -> ScrollState {
        let mut state = ScrollState::default();
        state.toggle_expand(uuid);
        state
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

        let lines = render_tool_use(&tool_call, &uuid, &scroll_state, 10, 3);

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

        let lines = render_tool_use(&tool_call, &uuid, &scroll_state, 10, 3);

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

        let lines = render_tool_use(&tool_call, &uuid, &scroll_state, 10, 3);

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

        let lines = render_tool_result(content, false, &uuid, &scroll_state, 10, 3);

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

        let lines = render_tool_result(content, false, &uuid, &scroll_state, 10, 3);

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

        let lines = render_tool_result(content, false, &uuid, &scroll_state, 10, 3);

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

        let lines = render_tool_result(content, true, &uuid, &scroll_state, 10, 3);

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

        let lines = render_tool_result(content, false, &uuid, &scroll_state, 10, 3);

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

        let lines = render_content_block(&block, &uuid, &scroll_state, 10, 3);

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

        let lines = render_content_block(&block, &uuid, &scroll_state, 10, 3);

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

        let lines = render_content_block(&block, &uuid, &scroll_state, 10, 3);

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

        let lines = render_content_block(&block, &uuid, &scroll_state, 10, 3);

        // Should render thinking content
        let text: String = lines.iter().map(|l| l.to_string()).collect();
        assert!(
            text.contains("Analyzing"),
            "Should render Thinking block with content visible"
        );
    }
}
