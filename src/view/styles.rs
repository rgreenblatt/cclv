//! Message type styling configuration.
//!
//! Provides distinct colors for different message types (User, Assistant, Tool calls, Errors).

use crate::model::{ContentBlock, Role};
use ratatui::style::{Color, Style};

// ===== MessageStyles =====

/// Configuration for message type styling.
///
/// Provides distinct colors for:
/// - User messages (Cyan)
/// - Assistant messages (Green)
/// - Tool calls (Yellow)
/// - Errors (Red)
pub struct MessageStyles {
    user_style: Style,
    assistant_style: Style,
    tool_call_style: Style,
    error_style: Style,
}

impl MessageStyles {
    /// Create a new MessageStyles with default color scheme.
    pub fn new() -> Self {
        todo!("MessageStyles::new")
    }

    /// Get the style for a message role.
    pub fn style_for_role(&self, role: Role) -> Style {
        todo!("MessageStyles::style_for_role")
    }

    /// Get the style for a content block.
    ///
    /// Returns appropriate style based on block type:
    /// - ToolUse: tool_call_style
    /// - ToolResult with is_error=true: error_style
    /// - Others: No specific styling (return default)
    pub fn style_for_content_block(&self, block: &ContentBlock) -> Option<Style> {
        todo!("MessageStyles::style_for_content_block")
    }
}

impl Default for MessageStyles {
    fn default() -> Self {
        Self::new()
    }
}

// ===== Tests =====

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ToolCall, ToolName, ToolUseId};

    // ===== MessageStyles Construction Tests =====

    #[test]
    fn message_styles_new_creates_instance() {
        let styles = MessageStyles::new();
        // Type-level test: instance exists
        let _verify: MessageStyles = styles;
    }

    #[test]
    fn message_styles_default_creates_instance() {
        let styles = MessageStyles::default();
        // Type-level test: instance exists
        let _verify: MessageStyles = styles;
    }

    // ===== style_for_role Tests (FR-023/024: Distinct colors for message types) =====

    #[test]
    fn style_for_role_user_returns_distinct_color() {
        let styles = MessageStyles::new();
        let style = styles.style_for_role(Role::User);

        // User messages should have a foreground color (cyan)
        assert!(
            style.fg.is_some(),
            "User role should have a foreground color"
        );
    }

    #[test]
    fn style_for_role_assistant_returns_distinct_color() {
        let styles = MessageStyles::new();
        let style = styles.style_for_role(Role::Assistant);

        // Assistant messages should have a foreground color (green)
        assert!(
            style.fg.is_some(),
            "Assistant role should have a foreground color"
        );
    }

    #[test]
    fn style_for_role_user_and_assistant_are_different() {
        let styles = MessageStyles::new();
        let user_style = styles.style_for_role(Role::User);
        let assistant_style = styles.style_for_role(Role::Assistant);

        // FR-023/024: User and Assistant must have distinct colors
        assert_ne!(
            user_style.fg, assistant_style.fg,
            "User and Assistant roles must have different foreground colors"
        );
    }

    // ===== style_for_content_block Tests =====

    #[test]
    fn style_for_content_block_tool_use_returns_style() {
        let styles = MessageStyles::new();
        let id = ToolUseId::new("tool-1").expect("valid id");
        let tool_call = ToolCall::new(id, ToolName::Read, serde_json::json!({"file": "test.txt"}));
        let block = ContentBlock::ToolUse(tool_call);

        let style = styles.style_for_content_block(&block);

        // Tool calls should have a distinct style
        assert!(
            style.is_some(),
            "ToolUse blocks should return a style"
        );
        assert!(
            style.unwrap().fg.is_some(),
            "ToolUse style should have a foreground color"
        );
    }

    #[test]
    fn style_for_content_block_tool_result_error_returns_red() {
        let styles = MessageStyles::new();
        let id = ToolUseId::new("result-1").expect("valid id");
        let block = ContentBlock::ToolResult {
            tool_use_id: id,
            content: "Error: file not found".to_string(),
            is_error: true,
        };

        let style = styles.style_for_content_block(&block);

        // Error results should have red styling
        assert!(
            style.is_some(),
            "ToolResult with is_error=true should return a style"
        );
        assert_eq!(
            style.unwrap().fg,
            Some(Color::Red),
            "Error ToolResult should have red foreground color"
        );
    }

    #[test]
    fn style_for_content_block_tool_result_success_returns_none() {
        let styles = MessageStyles::new();
        let id = ToolUseId::new("result-2").expect("valid id");
        let block = ContentBlock::ToolResult {
            tool_use_id: id,
            content: "Success output".to_string(),
            is_error: false,
        };

        let style = styles.style_for_content_block(&block);

        // Non-error results should not have special styling
        assert!(
            style.is_none(),
            "ToolResult with is_error=false should return None (no special styling)"
        );
    }

    #[test]
    fn style_for_content_block_text_returns_none() {
        let styles = MessageStyles::new();
        let block = ContentBlock::Text {
            text: "Plain text".to_string(),
        };

        let style = styles.style_for_content_block(&block);

        // Text blocks don't need special styling (handled by role)
        assert!(
            style.is_none(),
            "Text blocks should return None (no special styling)"
        );
    }

    #[test]
    fn style_for_content_block_thinking_returns_none() {
        let styles = MessageStyles::new();
        let block = ContentBlock::Thinking {
            thinking: "Analyzing...".to_string(),
        };

        let style = styles.style_for_content_block(&block);

        // Thinking blocks already have their own styling (italic/dim)
        assert!(
            style.is_none(),
            "Thinking blocks should return None (already have specific styling)"
        );
    }

    #[test]
    fn style_for_role_user_and_tool_call_are_different() {
        let styles = MessageStyles::new();
        let user_style = styles.style_for_role(Role::User);

        let id = ToolUseId::new("tool-1").expect("valid id");
        let tool_call = ToolCall::new(id, ToolName::Read, serde_json::json!({}));
        let block = ContentBlock::ToolUse(tool_call);
        let tool_style = styles.style_for_content_block(&block).expect("ToolUse should have style");

        // FR-024: Tool calls should be distinct from user messages
        assert_ne!(
            user_style.fg, tool_style.fg,
            "User and Tool call styling must be different"
        );
    }
}
