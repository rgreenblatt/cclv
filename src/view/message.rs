//! Conversation view widget - shared by main and subagent panes.
//!
//! PLACEHOLDER: This is a minimal implementation showing agent info.
//! Full conversation rendering (messages, markdown, syntax highlighting)
//! will be implemented in bead cclv-07v.4.2.

use crate::model::AgentConversation;
use crate::state::ScrollState;
use ratatui::{
    layout::Rect,
    style::{Color, Style},
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
