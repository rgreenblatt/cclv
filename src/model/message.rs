//! Message types for Claude Code log entries.
//!
//! Types represent the structure of messages exchanged during sessions.
//! Raw constructors are never exported - use smart constructors only.

use crate::model::{ModelInfo, TokenUsage, ToolUseId};

// ===== Role =====

/// Message role in a Claude Code conversation.
///
/// Identifies who authored a message in the JSONL log.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// Message authored by the user or system
    User,
    /// Message authored by Claude assistant
    Assistant,
}

// ===== MessageContent =====

/// Content of a message in the Claude Code log format.
///
/// Messages can be either plain text (simple user messages) or structured
/// blocks (assistant messages containing text, tool calls, results, and thinking).
/// Sum type ensures exactly one representation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MessageContent {
    /// Plain text content (typically user messages)
    Text(String),
    /// Structured content blocks (assistant messages with tool use)
    Blocks(Vec<ContentBlock>),
}

// ===== ContentBlock =====

/// Individual content block within a structured message.
///
/// Assistant messages in Claude Code logs consist of heterogeneous blocks:
/// text (visible output), tool_use (tool invocations), tool_result (tool outputs),
/// and thinking (extended reasoning). Each block type has distinct semantics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContentBlock {
    /// Text block containing markdown-formatted assistant output
    Text {
        /// Markdown content visible to the user
        text: String,
    },
    /// Tool invocation by the assistant
    ToolUse(ToolCall),
    /// Result returned from a tool execution
    ToolResult {
        /// ID linking this result to the originating tool_use
        tool_use_id: ToolUseId,
        /// Tool output (stdout, file contents, etc.)
        content: String,
        /// Whether the tool execution failed
        is_error: bool,
    },
    /// Extended thinking block (Claude's internal reasoning)
    Thinking {
        /// Reasoning content, not shown to user by default
        thinking: String,
    },
}

// ===== ToolCall =====

/// Tool invocation recorded in a Claude Code log.
///
/// Represents the assistant calling a tool (Read, Write, Bash, etc.) with
/// structured parameters. The id links to a corresponding ToolResult block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolCall {
    /// Unique identifier for this tool invocation
    id: ToolUseId,
    /// Tool being invoked (Read, Bash, Grep, etc.)
    name: ToolName,
    /// Tool-specific parameters as JSON
    input: serde_json::Value,
}

impl ToolCall {
    /// Create a new tool call.
    ///
    /// Smart constructor for building tool calls during parsing or testing.
    pub fn new(id: ToolUseId, name: ToolName, input: serde_json::Value) -> Self {
        Self { id, name, input }
    }

    /// Unique identifier linking this call to its result
    pub fn id(&self) -> &ToolUseId {
        &self.id
    }

    /// Tool name (Read, Write, Bash, etc.)
    pub fn name(&self) -> &ToolName {
        &self.name
    }

    /// Tool-specific input parameters
    pub fn input(&self) -> &serde_json::Value {
        &self.input
    }
}

// ===== ToolName =====

/// Tool names recognized in Claude Code logs.
///
/// Enumerates known tools provided by Claude Code with a fallback variant
/// for custom or future tools. Used for statistics aggregation and special
/// rendering (e.g., syntax highlighting for Bash commands).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ToolName {
    /// Read files from filesystem
    Read,
    /// Write files to filesystem
    Write,
    /// Edit existing files (string replacement)
    Edit,
    /// Apply multiple edits atomically
    MultiEdit,
    /// Execute bash commands
    Bash,
    /// Search file contents with regex
    Grep,
    /// Find files by glob pattern
    Glob,
    /// Create or manage subagent tasks
    Task,
    /// Search the web
    WebSearch,
    /// Fetch web resources
    WebFetch,
    /// Unknown or custom tool
    Other(String),
}

impl ToolName {
    /// Parse a tool name from the JSONL log.
    ///
    /// Recognizes standard Claude Code tools, wrapping unknown names in `Other`.
    pub fn parse(name: &str) -> Self {
        match name {
            "Read" => Self::Read,
            "Write" => Self::Write,
            "Edit" => Self::Edit,
            "MultiEdit" => Self::MultiEdit,
            "Bash" => Self::Bash,
            "Grep" => Self::Grep,
            "Glob" => Self::Glob,
            "Task" => Self::Task,
            "WebSearch" => Self::WebSearch,
            "WebFetch" => Self::WebFetch,
            other => Self::Other(other.to_string()),
        }
    }

    /// Get the canonical string representation.
    pub fn as_str(&self) -> &str {
        match self {
            Self::Read => "Read",
            Self::Write => "Write",
            Self::Edit => "Edit",
            Self::MultiEdit => "MultiEdit",
            Self::Bash => "Bash",
            Self::Grep => "Grep",
            Self::Glob => "Glob",
            Self::Task => "Task",
            Self::WebSearch => "WebSearch",
            Self::WebFetch => "WebFetch",
            Self::Other(s) => s,
        }
    }
}

// ===== Message =====

/// Complete message in a Claude Code conversation.
///
/// Represents a single turn in the conversation log with role, content,
/// and optional metadata (model info, token usage). Messages are the primary
/// unit of interaction displayed in the TUI.
#[derive(Debug, Clone)]
pub struct Message {
    /// Who authored this message (User or Assistant)
    role: Role,
    /// Message content (text or structured blocks)
    content: MessageContent,
    /// Model information (e.g., "claude-opus-4-5-20251101")
    model: Option<ModelInfo>,
    /// Token usage statistics for this turn
    usage: Option<TokenUsage>,
}

impl Message {
    /// Create a new message with role and content.
    ///
    /// Smart constructor for building messages during parsing or testing.
    /// Use `with_model()` and `with_usage()` to add optional metadata.
    pub fn new(role: Role, content: MessageContent) -> Self {
        Self {
            role,
            content,
            model: None,
            usage: None,
        }
    }

    /// Message author role
    pub fn role(&self) -> Role {
        self.role
    }

    /// Message content (text or blocks)
    pub fn content(&self) -> &MessageContent {
        &self.content
    }

    /// Model that generated this message (if assistant)
    pub fn model(&self) -> Option<&ModelInfo> {
        self.model.as_ref()
    }

    /// Token usage for this message turn
    pub fn usage(&self) -> Option<&TokenUsage> {
        self.usage.as_ref()
    }

    /// Attach token usage to this message (builder pattern).
    pub fn with_usage(mut self, usage: TokenUsage) -> Self {
        self.usage = Some(usage);
        self
    }

    /// Attach model info to this message (builder pattern).
    pub fn with_model(mut self, model: ModelInfo) -> Self {
        self.model = Some(model);
        self
    }

    /// Extract all tool calls from this message.
    ///
    /// Returns empty vector for text-only messages. Used for statistics
    /// aggregation and tool usage display.
    pub fn tool_calls(&self) -> Vec<&ToolCall> {
        match &self.content {
            MessageContent::Text(_) => vec![],
            MessageContent::Blocks(blocks) => blocks
                .iter()
                .filter_map(|block| match block {
                    ContentBlock::ToolUse(call) => Some(call),
                    _ => None,
                })
                .collect(),
        }
    }

    /// Get text content, joining all text blocks.
    ///
    /// Extracts only Text blocks, ignoring tool use, results, and thinking.
    /// Returns empty string if no text blocks present.
    pub fn text(&self) -> String {
        match &self.content {
            MessageContent::Text(text) => text.clone(),
            MessageContent::Blocks(blocks) => blocks
                .iter()
                .filter_map(|block| match block {
                    ContentBlock::Text { text } => Some(text.as_str()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("\n"),
        }
    }
}

// ===== Tests =====

#[cfg(test)]
mod tests {
    use super::*;

    // ===== ToolName Tests =====

    #[test]
    fn tool_name_parse_recognizes_read() {
        assert_eq!(ToolName::parse("Read"), ToolName::Read);
    }

    #[test]
    fn tool_name_parse_recognizes_write() {
        assert_eq!(ToolName::parse("Write"), ToolName::Write);
    }

    #[test]
    fn tool_name_parse_recognizes_edit() {
        assert_eq!(ToolName::parse("Edit"), ToolName::Edit);
    }

    #[test]
    fn tool_name_parse_recognizes_multi_edit() {
        assert_eq!(ToolName::parse("MultiEdit"), ToolName::MultiEdit);
    }

    #[test]
    fn tool_name_parse_recognizes_bash() {
        assert_eq!(ToolName::parse("Bash"), ToolName::Bash);
    }

    #[test]
    fn tool_name_parse_recognizes_grep() {
        assert_eq!(ToolName::parse("Grep"), ToolName::Grep);
    }

    #[test]
    fn tool_name_parse_recognizes_glob() {
        assert_eq!(ToolName::parse("Glob"), ToolName::Glob);
    }

    #[test]
    fn tool_name_parse_recognizes_task() {
        assert_eq!(ToolName::parse("Task"), ToolName::Task);
    }

    #[test]
    fn tool_name_parse_recognizes_web_search() {
        assert_eq!(ToolName::parse("WebSearch"), ToolName::WebSearch);
    }

    #[test]
    fn tool_name_parse_recognizes_web_fetch() {
        assert_eq!(ToolName::parse("WebFetch"), ToolName::WebFetch);
    }

    #[test]
    fn tool_name_parse_wraps_unknown_in_other() {
        assert_eq!(
            ToolName::parse("CustomTool"),
            ToolName::Other("CustomTool".to_string())
        );
    }

    #[test]
    fn tool_name_as_str_returns_read() {
        assert_eq!(ToolName::Read.as_str(), "Read");
    }

    #[test]
    fn tool_name_as_str_returns_write() {
        assert_eq!(ToolName::Write.as_str(), "Write");
    }

    #[test]
    fn tool_name_as_str_returns_edit() {
        assert_eq!(ToolName::Edit.as_str(), "Edit");
    }

    #[test]
    fn tool_name_as_str_returns_multi_edit() {
        assert_eq!(ToolName::MultiEdit.as_str(), "MultiEdit");
    }

    #[test]
    fn tool_name_as_str_returns_bash() {
        assert_eq!(ToolName::Bash.as_str(), "Bash");
    }

    #[test]
    fn tool_name_as_str_returns_grep() {
        assert_eq!(ToolName::Grep.as_str(), "Grep");
    }

    #[test]
    fn tool_name_as_str_returns_glob() {
        assert_eq!(ToolName::Glob.as_str(), "Glob");
    }

    #[test]
    fn tool_name_as_str_returns_task() {
        assert_eq!(ToolName::Task.as_str(), "Task");
    }

    #[test]
    fn tool_name_as_str_returns_web_search() {
        assert_eq!(ToolName::WebSearch.as_str(), "WebSearch");
    }

    #[test]
    fn tool_name_as_str_returns_web_fetch() {
        assert_eq!(ToolName::WebFetch.as_str(), "WebFetch");
    }

    #[test]
    fn tool_name_as_str_returns_other_value() {
        let tool = ToolName::Other("CustomTool".to_string());
        assert_eq!(tool.as_str(), "CustomTool");
    }

    // ===== ToolCall Tests =====

    #[test]
    fn tool_call_new_creates_instance() {
        let id = ToolUseId::new("tool-123").expect("valid id");
        let name = ToolName::Read;
        let input = serde_json::json!({"file": "test.txt"});

        let call = ToolCall::new(id.clone(), name.clone(), input.clone());

        assert_eq!(call.id(), &id);
        assert_eq!(call.name(), &name);
        assert_eq!(call.input(), &input);
    }

    #[test]
    fn tool_call_accessors_return_correct_values() {
        let id = ToolUseId::new("tool-456").expect("valid id");
        let name = ToolName::Bash;
        let input = serde_json::json!({"command": "ls -la"});

        let call = ToolCall::new(id.clone(), name.clone(), input.clone());

        assert_eq!(call.id().as_str(), "tool-456");
        assert_eq!(call.name(), &ToolName::Bash);
        assert_eq!(call.input()["command"], "ls -la");
    }

    // ===== Message Tests =====

    #[test]
    fn message_new_creates_text_message() {
        let msg = Message::new(Role::User, MessageContent::Text("Hello".to_string()));

        assert_eq!(msg.role(), Role::User);
        match msg.content() {
            MessageContent::Text(text) => assert_eq!(text, "Hello"),
            _ => panic!("Expected Text content"),
        }
    }

    #[test]
    fn message_new_creates_blocks_message() {
        let blocks = vec![ContentBlock::Text {
            text: "Test".to_string(),
        }];
        let msg = Message::new(Role::Assistant, MessageContent::Blocks(blocks));

        assert_eq!(msg.role(), Role::Assistant);
        match msg.content() {
            MessageContent::Blocks(b) => assert_eq!(b.len(), 1),
            _ => panic!("Expected Blocks content"),
        }
    }

    // ===== Message::tool_calls Tests =====

    #[test]
    fn message_tool_calls_extracts_tool_use_blocks() {
        let id1 = ToolUseId::new("tool-1").expect("valid id");
        let id2 = ToolUseId::new("tool-2").expect("valid id");

        let blocks = vec![
            ContentBlock::Text {
                text: "Checking files".to_string(),
            },
            ContentBlock::ToolUse(ToolCall::new(
                id1,
                ToolName::Read,
                serde_json::json!({"file": "a.txt"}),
            )),
            ContentBlock::Thinking {
                thinking: "I should grep".to_string(),
            },
            ContentBlock::ToolUse(ToolCall::new(
                id2,
                ToolName::Grep,
                serde_json::json!({"pattern": "TODO"}),
            )),
        ];

        let msg = Message::new(Role::Assistant, MessageContent::Blocks(blocks));
        let calls = msg.tool_calls();

        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].name(), &ToolName::Read);
        assert_eq!(calls[1].name(), &ToolName::Grep);
    }

    #[test]
    fn message_tool_calls_returns_empty_for_text_content() {
        let msg = Message::new(Role::User, MessageContent::Text("Hello".to_string()));
        let calls = msg.tool_calls();

        assert_eq!(calls.len(), 0);
    }

    #[test]
    fn message_tool_calls_returns_empty_for_no_tool_blocks() {
        let blocks = vec![
            ContentBlock::Text {
                text: "Just text".to_string(),
            },
            ContentBlock::Thinking {
                thinking: "Thinking...".to_string(),
            },
        ];

        let msg = Message::new(Role::Assistant, MessageContent::Blocks(blocks));
        let calls = msg.tool_calls();

        assert_eq!(calls.len(), 0);
    }

    // ===== Message::text Tests =====

    #[test]
    fn message_text_returns_simple_text_content() {
        let msg = Message::new(Role::User, MessageContent::Text("Hello world".to_string()));

        assert_eq!(msg.text(), "Hello world");
    }

    #[test]
    fn message_text_joins_multiple_text_blocks() {
        let blocks = vec![
            ContentBlock::Text {
                text: "First ".to_string(),
            },
            ContentBlock::Text {
                text: "Second ".to_string(),
            },
            ContentBlock::Text {
                text: "Third".to_string(),
            },
        ];

        let msg = Message::new(Role::Assistant, MessageContent::Blocks(blocks));

        assert_eq!(msg.text(), "First \nSecond \nThird");
    }

    #[test]
    fn message_text_ignores_non_text_blocks() {
        let id = ToolUseId::new("tool-1").expect("valid id");
        let blocks = vec![
            ContentBlock::Text {
                text: "Before ".to_string(),
            },
            ContentBlock::ToolUse(ToolCall::new(
                id.clone(),
                ToolName::Read,
                serde_json::json!({}),
            )),
            ContentBlock::Text {
                text: "After ".to_string(),
            },
            ContentBlock::Thinking {
                thinking: "Hmm".to_string(),
            },
            ContentBlock::Text {
                text: "End".to_string(),
            },
        ];

        let msg = Message::new(Role::Assistant, MessageContent::Blocks(blocks));

        assert_eq!(msg.text(), "Before \nAfter \nEnd");
    }

    #[test]
    fn message_text_returns_empty_for_no_text_blocks() {
        let id = ToolUseId::new("tool-1").expect("valid id");
        let blocks = vec![
            ContentBlock::ToolUse(ToolCall::new(id, ToolName::Bash, serde_json::json!({}))),
            ContentBlock::Thinking {
                thinking: "Thinking".to_string(),
            },
        ];

        let msg = Message::new(Role::Assistant, MessageContent::Blocks(blocks));

        assert_eq!(msg.text(), "");
    }

    #[test]
    fn message_text_returns_empty_for_empty_blocks() {
        let msg = Message::new(Role::Assistant, MessageContent::Blocks(vec![]));

        assert_eq!(msg.text(), "");
    }

    // ===== Role Tests =====

    #[test]
    fn role_user_equals_user() {
        assert_eq!(Role::User, Role::User);
    }

    #[test]
    fn role_assistant_equals_assistant() {
        assert_eq!(Role::Assistant, Role::Assistant);
    }

    #[test]
    fn role_user_not_equals_assistant() {
        assert_ne!(Role::User, Role::Assistant);
    }
}
