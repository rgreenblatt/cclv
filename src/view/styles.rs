//! Message type styling configuration.
//!
//! Provides distinct colors for different message types (User, Assistant, Tool calls, Errors).

use crate::model::{ContentBlock, Role};
use ratatui::style::{Color, Style};

// ===== ColorConfig =====

/// Configuration for color output.
///
/// Determines whether colors should be enabled or disabled based on:
/// - `--no-color` CLI flag
/// - `NO_COLOR` environment variable
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ColorConfig {
    enabled: bool,
}

impl ColorConfig {
    /// Create a ColorConfig from CLI args and environment.
    ///
    /// Priority (first match wins):
    /// 1. `--no-color` flag (disables colors)
    /// 2. `NO_COLOR` env var (any value disables colors)
    /// 3. Default: colors enabled
    pub fn from_env_and_args(no_color_flag: bool) -> Self {
        let enabled = !no_color_flag && std::env::var("NO_COLOR").is_err();
        Self { enabled }
    }

    /// Check if colors are enabled.
    pub fn colors_enabled(self) -> bool {
        self.enabled
    }
}

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
        Self::with_color_config(ColorConfig::from_env_and_args(false))
    }

    /// Create a new MessageStyles with specified color configuration.
    ///
    /// If colors are disabled, all styles will use default (no color) styling.
    pub fn with_color_config(config: ColorConfig) -> Self {
        if config.colors_enabled() {
            Self {
                user_style: Style::default().fg(Color::Cyan),
                assistant_style: Style::default().fg(Color::Green),
                tool_call_style: Style::default().fg(Color::Yellow),
                error_style: Style::default().fg(Color::Red),
            }
        } else {
            Self {
                user_style: Style::default(),
                assistant_style: Style::default(),
                tool_call_style: Style::default(),
                error_style: Style::default(),
            }
        }
    }

    /// Get the style for a message role.
    pub fn style_for_role(&self, role: Role) -> Style {
        match role {
            Role::User => self.user_style,
            Role::Assistant => self.assistant_style,
        }
    }

    /// Get the style for a content block.
    ///
    /// Returns appropriate style based on block type:
    /// - ToolUse: tool_call_style
    /// - ToolResult with is_error=true: error_style
    /// - Others: No specific styling (return default)
    pub fn style_for_content_block(&self, block: &ContentBlock) -> Option<Style> {
        match block {
            ContentBlock::ToolUse(_) => Some(self.tool_call_style),
            ContentBlock::ToolResult { is_error, .. } => {
                if *is_error {
                    Some(self.error_style)
                } else {
                    None
                }
            }
            ContentBlock::Text { .. } | ContentBlock::Thinking { .. } => None,
        }
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

    // ===== ColorConfig Tests =====

    #[test]
    fn color_config_respects_no_color_flag() {
        let config = ColorConfig::from_env_and_args(true);
        assert!(
            !config.colors_enabled(),
            "--no-color flag should disable colors"
        );
    }

    #[test]
    fn color_config_respects_no_color_env_var() {
        // Set NO_COLOR env var
        std::env::set_var("NO_COLOR", "1");
        let config = ColorConfig::from_env_and_args(false);
        assert!(
            !config.colors_enabled(),
            "NO_COLOR env var should disable colors"
        );
        std::env::remove_var("NO_COLOR");
    }

    #[test]
    fn color_config_flag_overrides_env_var() {
        // Set NO_COLOR env var but also pass --no-color flag
        std::env::set_var("NO_COLOR", "1");
        let config = ColorConfig::from_env_and_args(true);
        assert!(
            !config.colors_enabled(),
            "Flag should still disable when env var also present"
        );
        std::env::remove_var("NO_COLOR");
    }

    #[test]
    fn color_config_enables_colors_by_default() {
        // Ensure NO_COLOR is not set
        std::env::remove_var("NO_COLOR");
        let config = ColorConfig::from_env_and_args(false);
        assert!(
            config.colors_enabled(),
            "Colors should be enabled by default"
        );
    }

    #[test]
    fn color_config_no_color_env_any_value_disables() {
        // NO_COLOR can be any value (even empty string)
        std::env::set_var("NO_COLOR", "");
        let config = ColorConfig::from_env_and_args(false);
        assert!(
            !config.colors_enabled(),
            "NO_COLOR with empty string should disable colors"
        );
        std::env::remove_var("NO_COLOR");
    }

    // ===== MessageStyles with ColorConfig Tests =====

    #[test]
    fn message_styles_with_color_config_enabled_has_colors() {
        std::env::remove_var("NO_COLOR");
        let config = ColorConfig::from_env_and_args(false);
        let styles = MessageStyles::with_color_config(config);

        let user_style = styles.style_for_role(Role::User);
        let assistant_style = styles.style_for_role(Role::Assistant);

        assert!(
            user_style.fg.is_some(),
            "User style should have color when colors enabled"
        );
        assert!(
            assistant_style.fg.is_some(),
            "Assistant style should have color when colors enabled"
        );
    }

    #[test]
    fn message_styles_with_color_config_disabled_has_no_colors() {
        let config = ColorConfig::from_env_and_args(true); // --no-color
        let styles = MessageStyles::with_color_config(config);

        let user_style = styles.style_for_role(Role::User);
        let assistant_style = styles.style_for_role(Role::Assistant);

        assert!(
            user_style.fg.is_none(),
            "User style should have no color when colors disabled"
        );
        assert!(
            assistant_style.fg.is_none(),
            "Assistant style should have no color when colors disabled"
        );
    }

    #[test]
    fn message_styles_no_color_disables_tool_call_colors() {
        let config = ColorConfig::from_env_and_args(true);
        let styles = MessageStyles::with_color_config(config);

        let id = ToolUseId::new("tool-1").expect("valid id");
        let tool_call = ToolCall::new(id, ToolName::Read, serde_json::json!({"file": "test.txt"}));
        let block = ContentBlock::ToolUse(tool_call);

        let style = styles.style_for_content_block(&block);

        // Tool calls should return a style, but with no foreground color
        assert!(
            style.is_some(),
            "ToolUse should still return a style struct"
        );
        assert!(
            style.unwrap().fg.is_none(),
            "ToolUse style should have no color when colors disabled"
        );
    }

    #[test]
    fn message_styles_no_color_disables_error_colors() {
        let config = ColorConfig::from_env_and_args(true);
        let styles = MessageStyles::with_color_config(config);

        let id = ToolUseId::new("result-1").expect("valid id");
        let block = ContentBlock::ToolResult {
            tool_use_id: id,
            content: "Error: file not found".to_string(),
            is_error: true,
        };

        let style = styles.style_for_content_block(&block);

        // Errors should return a style, but with no color
        assert!(
            style.is_some(),
            "Error ToolResult should still return a style struct"
        );
        assert!(
            style.unwrap().fg.is_none(),
            "Error style should have no color when colors disabled"
        );
    }

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
        assert!(style.is_some(), "ToolUse blocks should return a style");
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
        let tool_style = styles
            .style_for_content_block(&block)
            .expect("ToolUse should have style");

        // FR-024: Tool calls should be distinct from user messages
        assert_ne!(
            user_style.fg, tool_style.fg,
            "User and Tool call styling must be different"
        );
    }
}
