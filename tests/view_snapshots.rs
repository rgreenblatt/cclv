//! Snapshot tests for key view components
//!
//! Uses insta + ratatui TestBackend to verify rendering output doesn't regress.
//! These tests capture the visual representation of widgets and protect against
//! accidental UI changes.

use cclv::model::{
    AgentConversation, AgentId, ContentBlock, ConversationEntry, EntryMetadata, EntryType,
    EntryUuid, LogEntry, Message, MessageContent, PricingConfig, Role, Session, SessionId,
    SessionStats, StatsFilter, TokenUsage, ToolCall, ToolName, ToolUseId,
};
use cclv::state::{ScrollState, WrapMode};
use cclv::view::{tabs, ConversationView, MessageStyles, StatsPanel};
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use std::collections::{HashMap, HashSet};

// ===== Test Helpers =====

/// Convert a ratatui buffer to a string representation for snapshot testing.
///
/// Captures the visual output character by character, preserving layout.
/// Empty trailing lines are removed to keep snapshots clean.
fn buffer_to_string(buffer: &ratatui::buffer::Buffer) -> String {
    let area = buffer.area();
    let mut lines = Vec::new();

    for y in area.top()..area.bottom() {
        let mut line = String::new();
        for x in area.left()..area.right() {
            let cell = &buffer[(x, y)];
            line.push_str(cell.symbol());
        }
        let trimmed = line.trim_end();
        if !trimmed.is_empty() {
            lines.push(trimmed.to_string());
        }
    }

    lines.join("\n")
}

/// Create a test terminal with the given dimensions.
fn create_terminal(width: u16, height: u16) -> Terminal<TestBackend> {
    let backend = TestBackend::new(width, height);
    Terminal::new(backend).unwrap()
}

/// Create sample SessionStats for testing.
fn create_sample_stats() -> SessionStats {
    let mut tool_counts = HashMap::new();
    tool_counts.insert(ToolName::Read, 5);
    tool_counts.insert(ToolName::Write, 3);
    tool_counts.insert(ToolName::Bash, 2);

    SessionStats {
        total_usage: TokenUsage {
            input_tokens: 1_500,
            output_tokens: 750,
            cache_creation_input_tokens: 200,
            cache_read_input_tokens: 100,
            ephemeral_5m_input_tokens: 0,
            ephemeral_1h_input_tokens: 0,
        },
        main_agent_usage: TokenUsage {
            input_tokens: 1_000,
            output_tokens: 500,
            cache_creation_input_tokens: 100,
            cache_read_input_tokens: 50,
            ephemeral_5m_input_tokens: 0,
            ephemeral_1h_input_tokens: 0,
        },
        subagent_usage: HashMap::new(),
        tool_counts,
        main_agent_tool_counts: HashMap::new(),
        subagent_tool_counts: HashMap::new(),
        subagent_count: 2,
        entry_count: 10,
        ..Default::default()
    }
}

// ===== StatsPanel Snapshot Tests =====

#[test]
fn snapshot_stats_panel_empty() {
    let stats = SessionStats::default();
    let filter = StatsFilter::Global;
    let pricing = PricingConfig::default();

    let mut terminal = create_terminal(50, 20);
    terminal
        .draw(|frame| {
            let panel = StatsPanel::new(&stats, &filter, &pricing, Some("opus"), false);
            frame.render_widget(panel, frame.area());
        })
        .unwrap();

    let output = buffer_to_string(terminal.backend().buffer());
    insta::assert_snapshot!("stats_panel_empty", output);
}

#[test]
fn snapshot_stats_panel_with_data() {
    let stats = create_sample_stats();
    let filter = StatsFilter::Global;
    let pricing = PricingConfig::default();

    let mut terminal = create_terminal(50, 25);
    terminal
        .draw(|frame| {
            let panel = StatsPanel::new(&stats, &filter, &pricing, Some("opus"), false);
            frame.render_widget(panel, frame.area());
        })
        .unwrap();

    let output = buffer_to_string(terminal.backend().buffer());
    insta::assert_snapshot!("stats_panel_with_data", output);
}

#[test]
fn snapshot_stats_panel_focused() {
    let stats = create_sample_stats();
    let filter = StatsFilter::Global;
    let pricing = PricingConfig::default();

    let mut terminal = create_terminal(50, 25);
    terminal
        .draw(|frame| {
            let panel = StatsPanel::new(&stats, &filter, &pricing, Some("opus"), true);
            frame.render_widget(panel, frame.area());
        })
        .unwrap();

    let output = buffer_to_string(terminal.backend().buffer());
    insta::assert_snapshot!("stats_panel_focused", output);
}

#[test]
fn snapshot_stats_panel_main_agent_filter() {
    let stats = create_sample_stats();
    let filter = StatsFilter::MainAgent;
    let pricing = PricingConfig::default();

    let mut terminal = create_terminal(55, 25);
    terminal
        .draw(|frame| {
            let panel = StatsPanel::new(&stats, &filter, &pricing, Some("sonnet"), false);
            frame.render_widget(panel, frame.area());
        })
        .unwrap();

    let output = buffer_to_string(terminal.backend().buffer());
    insta::assert_snapshot!("stats_panel_main_agent_filter", output);
}

#[test]
fn snapshot_stats_panel_with_cache_tokens() {
    let stats = SessionStats {
        total_usage: TokenUsage {
            input_tokens: 10_000,
            output_tokens: 5_000,
            cache_creation_input_tokens: 2_000,
            cache_read_input_tokens: 1_500,
            ephemeral_5m_input_tokens: 0,
            ephemeral_1h_input_tokens: 0,
        },
        main_agent_usage: TokenUsage::default(),
        subagent_usage: HashMap::new(),
        tool_counts: HashMap::new(),
        main_agent_tool_counts: HashMap::new(),
        subagent_tool_counts: HashMap::new(),
        subagent_count: 0,
        entry_count: 5,
        ..Default::default()
    };
    let filter = StatsFilter::Global;
    let pricing = PricingConfig::default();

    let mut terminal = create_terminal(50, 25);
    terminal
        .draw(|frame| {
            let panel = StatsPanel::new(&stats, &filter, &pricing, Some("haiku"), false);
            frame.render_widget(panel, frame.area());
        })
        .unwrap();

    let output = buffer_to_string(terminal.backend().buffer());
    insta::assert_snapshot!("stats_panel_with_cache_tokens", output);
}

// ===== Tab Bar Snapshot Tests =====

#[test]
fn snapshot_tab_bar_single_tab() {
    let agent1 = AgentId::new("agent-abc123").unwrap();
    let agent_ids = vec![&agent1];
    let matches = HashSet::new();

    let mut terminal = create_terminal(40, 5);
    terminal
        .draw(|frame| {
            tabs::render_tab_bar(frame, frame.area(), &agent_ids, Some(0), &matches);
        })
        .unwrap();

    let output = buffer_to_string(terminal.backend().buffer());
    insta::assert_snapshot!("tab_bar_single_tab", output);
}

#[test]
fn snapshot_tab_bar_multiple_tabs() {
    let agent1 = AgentId::new("agent-1").unwrap();
    let agent2 = AgentId::new("agent-2").unwrap();
    let agent3 = AgentId::new("agent-3").unwrap();
    let agent_ids = vec![&agent1, &agent2, &agent3];
    let matches = HashSet::new();

    let mut terminal = create_terminal(60, 5);
    terminal
        .draw(|frame| {
            tabs::render_tab_bar(frame, frame.area(), &agent_ids, Some(1), &matches);
        })
        .unwrap();

    let output = buffer_to_string(terminal.backend().buffer());
    insta::assert_snapshot!("tab_bar_multiple_tabs", output);
}

#[test]
fn snapshot_tab_bar_with_search_matches() {
    let agent1 = AgentId::new("agent-1").unwrap();
    let agent2 = AgentId::new("agent-2").unwrap();
    let agent3 = AgentId::new("agent-3").unwrap();
    let agent_ids = vec![&agent1, &agent2, &agent3];

    let mut matches = HashSet::new();
    matches.insert(agent1.clone());
    matches.insert(agent3.clone());

    let mut terminal = create_terminal(60, 5);
    terminal
        .draw(|frame| {
            tabs::render_tab_bar(frame, frame.area(), &agent_ids, Some(0), &matches);
        })
        .unwrap();

    let output = buffer_to_string(terminal.backend().buffer());
    insta::assert_snapshot!("tab_bar_with_search_matches", output);
}

#[test]
fn snapshot_tab_bar_no_selection() {
    let agent1 = AgentId::new("agent-1").unwrap();
    let agent2 = AgentId::new("agent-2").unwrap();
    let agent_ids = vec![&agent1, &agent2];
    let matches = HashSet::new();

    let mut terminal = create_terminal(50, 5);
    terminal
        .draw(|frame| {
            tabs::render_tab_bar(frame, frame.area(), &agent_ids, None, &matches);
        })
        .unwrap();

    let output = buffer_to_string(terminal.backend().buffer());
    insta::assert_snapshot!("tab_bar_no_selection", output);
}

// ===== Message Rendering Snapshot Tests =====

/// Create a test LogEntry with given content and type.
fn create_test_log_entry(
    uuid: &str,
    role: Role,
    content: MessageContent,
    entry_type: EntryType,
) -> LogEntry {
    use chrono::Utc;

    let message = Message::new(role, content);
    LogEntry::new(
        EntryUuid::new(uuid).unwrap(),
        None,
        SessionId::new("test-session").unwrap(),
        None,
        Utc::now(),
        entry_type,
        message,
        EntryMetadata::default(),
    )
}

/// Create a test conversation with given entries.
fn create_test_conversation(entries: Vec<LogEntry>) -> AgentConversation {
    let session_id = SessionId::new("test-session").unwrap();
    let mut session = Session::new(session_id);

    for entry in entries {
        session.add_conversation_entry(ConversationEntry::Valid(Box::new(entry)));
    }

    // Return the main agent conversation
    session.main_agent().clone()
}

#[test]
fn snapshot_message_collapsed_multiline() {
    // Create a message with multiple lines that should be collapsed
    let long_text = "Line 1\nLine 2\nLine 3\nLine 4\nLine 5\nLine 6\nLine 7\nLine 8";
    let entry = create_test_log_entry(
        "msg-1",
        Role::User,
        MessageContent::Text(long_text.to_string()),
        EntryType::User,
    );

    let conversation = create_test_conversation(vec![entry]);
    let scroll_state = ScrollState::default();
    let styles = MessageStyles::new();

    let mut terminal = create_terminal(60, 15);
    terminal
        .draw(|frame| {
            let widget = ConversationView::new(&conversation, &scroll_state, &styles, false)
                .global_wrap(WrapMode::Wrap);
            frame.render_widget(widget, frame.area());
        })
        .unwrap();

    let output = buffer_to_string(terminal.backend().buffer());
    insta::assert_snapshot!("message_collapsed_multiline", output);
}

#[test]
fn snapshot_message_expanded_multiline() {
    // Create a message with multiple lines and expand it
    let long_text = "Line 1\nLine 2\nLine 3\nLine 4\nLine 5\nLine 6\nLine 7\nLine 8";
    let entry = create_test_log_entry(
        "msg-expanded",
        Role::User,
        MessageContent::Text(long_text.to_string()),
        EntryType::User,
    );

    let conversation = create_test_conversation(vec![entry.clone()]);
    let mut scroll_state = ScrollState::default();
    // Expand the message
    scroll_state.toggle_expand(entry.uuid());
    let styles = MessageStyles::new();

    let mut terminal = create_terminal(60, 20);
    terminal
        .draw(|frame| {
            let widget = ConversationView::new(&conversation, &scroll_state, &styles, false)
                .global_wrap(WrapMode::Wrap);
            frame.render_widget(widget, frame.area());
        })
        .unwrap();

    let output = buffer_to_string(terminal.backend().buffer());
    insta::assert_snapshot!("message_expanded_multiline", output);
}

#[test]
fn snapshot_message_with_code_block() {
    // Create a message with markdown code block
    let markdown_text = r#"Here's some code:

```rust
fn main() {
    println!("Hello, world!");
}
```

That's the code."#;

    let entry = create_test_log_entry(
        "msg-code",
        Role::Assistant,
        MessageContent::Text(markdown_text.to_string()),
        EntryType::Assistant,
    );

    let conversation = create_test_conversation(vec![entry.clone()]);
    let mut scroll_state = ScrollState::default();
    // Expand to see full code block
    scroll_state.toggle_expand(entry.uuid());
    let styles = MessageStyles::new();

    let mut terminal = create_terminal(70, 25);
    terminal
        .draw(|frame| {
            let widget = ConversationView::new(&conversation, &scroll_state, &styles, false)
                .global_wrap(WrapMode::Wrap);
            frame.render_widget(widget, frame.area());
        })
        .unwrap();

    let output = buffer_to_string(terminal.backend().buffer());
    insta::assert_snapshot!("message_with_code_block", output);
}

#[test]
fn snapshot_message_with_tool_use() {
    // Create a message with ToolUse content block
    let tool_call = ToolCall::new(
        ToolUseId::new("tool-read-1").unwrap(),
        ToolName::Read,
        serde_json::json!({
            "file_path": "/home/user/test.rs",
            "limit": 100
        }),
    );

    let blocks = vec![
        ContentBlock::Text {
            text: "Let me read that file for you.".to_string(),
        },
        ContentBlock::ToolUse(tool_call),
    ];

    let entry = create_test_log_entry(
        "msg-tool-use",
        Role::Assistant,
        MessageContent::Blocks(blocks),
        EntryType::Assistant,
    );

    let conversation = create_test_conversation(vec![entry.clone()]);
    let mut scroll_state = ScrollState::default();
    scroll_state.toggle_expand(entry.uuid());
    let styles = MessageStyles::new();

    let mut terminal = create_terminal(80, 20);
    terminal
        .draw(|frame| {
            let widget = ConversationView::new(&conversation, &scroll_state, &styles, false)
                .global_wrap(WrapMode::Wrap);
            frame.render_widget(widget, frame.area());
        })
        .unwrap();

    let output = buffer_to_string(terminal.backend().buffer());
    insta::assert_snapshot!("message_with_tool_use", output);
}

#[test]
fn snapshot_message_with_tool_result() {
    // Create a message with ToolResult content block
    let tool_result_content = r#"File contents:
fn main() {
    println!("Hello, world!");
}

Total lines: 3"#;

    let blocks = vec![ContentBlock::ToolResult {
        tool_use_id: ToolUseId::new("tool-123").unwrap(),
        content: tool_result_content.to_string(),
        is_error: false,
    }];

    let entry = create_test_log_entry(
        "msg-tool-result",
        Role::User,
        MessageContent::Blocks(blocks),
        EntryType::User, // Tool results are typically User role
    );

    let conversation = create_test_conversation(vec![entry.clone()]);
    let mut scroll_state = ScrollState::default();
    scroll_state.toggle_expand(entry.uuid());
    let styles = MessageStyles::new();

    let mut terminal = create_terminal(80, 20);
    terminal
        .draw(|frame| {
            let widget = ConversationView::new(&conversation, &scroll_state, &styles, false)
                .global_wrap(WrapMode::Wrap);
            frame.render_widget(widget, frame.area());
        })
        .unwrap();

    let output = buffer_to_string(terminal.backend().buffer());
    insta::assert_snapshot!("message_with_tool_result", output);
}

#[test]
fn snapshot_message_with_thinking_block() {
    // Create a message with Thinking content block
    let thinking_text = "Let me analyze this problem step by step:\n1. First, I need to understand the requirements\n2. Then identify the key components\n3. Finally, propose a solution";

    let blocks = vec![
        ContentBlock::Thinking {
            thinking: thinking_text.to_string(),
        },
        ContentBlock::Text {
            text: "Based on my analysis, here's what I recommend...".to_string(),
        },
    ];

    let entry = create_test_log_entry(
        "msg-thinking",
        Role::Assistant,
        MessageContent::Blocks(blocks),
        EntryType::Assistant,
    );

    let conversation = create_test_conversation(vec![entry.clone()]);
    let mut scroll_state = ScrollState::default();
    scroll_state.toggle_expand(entry.uuid());
    let styles = MessageStyles::new();

    let mut terminal = create_terminal(80, 20);
    terminal
        .draw(|frame| {
            let widget = ConversationView::new(&conversation, &scroll_state, &styles, false)
                .global_wrap(WrapMode::Wrap);
            frame.render_widget(widget, frame.area());
        })
        .unwrap();

    let output = buffer_to_string(terminal.backend().buffer());
    insta::assert_snapshot!("message_with_thinking_block", output);
}

// ===== Search Highlighting Snapshot Tests =====

#[test]
fn snapshot_message_with_search_highlighting() {
    use cclv::state::{SearchQuery, SearchState};
    use cclv::view::render_conversation_view_with_search;

    // Create a message with searchable text
    let text =
        "This is a test message with some searchable content.\nAnother line with test keyword.";
    let entry = create_test_log_entry(
        "msg-search",
        Role::Assistant,
        MessageContent::Text(text.to_string()),
        EntryType::Assistant,
    );

    let conversation = create_test_conversation(vec![entry.clone()]);
    let mut scroll_state = ScrollState::default();
    scroll_state.toggle_expand(entry.uuid());
    let styles = MessageStyles::new();

    // Create active search state with query "test"
    let query = SearchQuery::new("test").expect("Valid search query");
    let search = SearchState::Active {
        query,
        matches: vec![], // Matches populated by execute_search in real app
        current_match: 0,
    };

    let mut terminal = create_terminal(80, 20);
    terminal
        .draw(|frame| {
            render_conversation_view_with_search(
                frame,
                frame.area(),
                &conversation,
                &scroll_state,
                &search,
                &styles,
                false,
                WrapMode::Wrap,
            );
        })
        .unwrap();

    let output = buffer_to_string(terminal.backend().buffer());
    insta::assert_snapshot!("message_with_search_highlighting", output);
}

// ===== Bug Reproduction Tests =====

/// Bug reproduction: cclv-07v.12.21.1
/// Scroll offset calculation is incorrect - offsets 0-20 all show same content.
/// Scrolling doesn't smoothly move through content; it ignores small offsets
/// then jumps between entries at larger offsets.
///
/// EXPECTED: Each scroll offset should show progressively different content.
///           Offset 5 should show different first visible line than offset 0.
/// ACTUAL: Offsets 0, 5, 10, 15, 20 all show identical Entry 1-4 content.
#[test]
#[ignore = "cclv-07v.12.21.1: scroll offset ignored for small values, viewport doesn't move"]
fn bug_scroll_offset_adds_blank_lines_instead_of_moving_viewport() {
    // Create 20 entries - each entry is 2 lines (content + blank separator)
    let entries: Vec<LogEntry> = (1..=20)
        .map(|i| {
            create_test_log_entry(
                &format!("msg-{}", i),
                if i % 2 == 0 { Role::Assistant } else { Role::User },
                MessageContent::Text(format!("Entry {} content here.", i)),
                if i % 2 == 0 {
                    EntryType::Assistant
                } else {
                    EntryType::User
                },
            )
        })
        .collect();

    let conversation = create_test_conversation(entries);
    let styles = MessageStyles::new();

    // Render at offset 0 and offset 10 - they should show DIFFERENT content
    let render_at_offset = |offset: usize| -> String {
        let mut scroll_state = ScrollState::default();
        scroll_state.vertical_offset = offset;

        let mut terminal = create_terminal(60, 10);
        terminal
            .draw(|frame| {
                let widget = ConversationView::new(&conversation, &scroll_state, &styles, false)
                    .global_wrap(WrapMode::Wrap);
                frame.render_widget(widget, frame.area());
            })
            .unwrap();

        buffer_to_string(terminal.backend().buffer())
    };

    let output_at_0 = render_at_offset(0);
    let output_at_10 = render_at_offset(10);

    // BUG: These outputs are IDENTICAL when they should be different.
    // At offset 10, we should NOT see "Entry 1" - it should be scrolled off.
    assert_ne!(
        output_at_0, output_at_10,
        "Scroll offset 10 should show different content than offset 0.\n\
         Both show identical content, proving scroll offset is ignored.\n\
         Output at 0:\n{}\n\nOutput at 10:\n{}",
        output_at_0, output_at_10
    );
}

/// Diagnostic test: Print exact rendering to understand scroll behavior
#[test]
#[ignore] // Run with --ignored to see output
fn diagnostic_scroll_rendering_with_many_entries() {
    // Create 20 entries to ensure scrolling is needed
    let entries: Vec<LogEntry> = (1..=20)
        .map(|i| {
            create_test_log_entry(
                &format!("msg-{}", i),
                if i % 2 == 0 { Role::Assistant } else { Role::User },
                MessageContent::Text(format!("Entry {} content here.", i)),
                if i % 2 == 0 {
                    EntryType::Assistant
                } else {
                    EntryType::User
                },
            )
        })
        .collect();

    let conversation = create_test_conversation(entries);
    let styles = MessageStyles::new();

    // Test at different scroll offsets
    let test_offsets = vec![0, 5, 10, 15, 20, 30, 50];
    let viewport_height = 10;

    for offset in test_offsets {
        let mut scroll_state = ScrollState::default();
        scroll_state.vertical_offset = offset;

        let mut terminal = create_terminal(60, viewport_height);
        terminal
            .draw(|frame| {
                let widget = ConversationView::new(&conversation, &scroll_state, &styles, false)
                    .global_wrap(WrapMode::Wrap);
                frame.render_widget(widget, frame.area());
            })
            .unwrap();

        let output = buffer_to_string(terminal.backend().buffer());
        let lines: Vec<&str> = output.lines().collect();
        let blank_lines = lines.iter().filter(|l| l.trim().is_empty()).count();
        let content_lines = lines.iter().filter(|l| !l.trim().is_empty()).count();

        println!(
            "\n=== Offset: {} ===\nTotal lines: {}, Content: {}, Blank: {}\nOutput:\n{}",
            offset,
            lines.len(),
            content_lines,
            blank_lines,
            output
        );
    }
}
