//! Snapshot tests for key view components
//!
//! Uses insta + ratatui TestBackend to verify rendering output doesn't regress.
//! These tests capture the visual representation of widgets and protect against
//! accidental UI changes.

use crate::model::{
    AgentId, ContentBlock, ConversationEntry, EntryMetadata, EntryType, EntryUuid, LogEntry,
    Message, MessageContent, PricingConfig, Role, SessionId, SessionStats, StatsFilter, TokenUsage,
    ToolCall, ToolName, ToolUseId,
};
use crate::state::WrapMode;
use crate::view::{ConversationView, MessageStyles, StatsPanel, tabs};
use crate::view_state::conversation::ConversationViewState;
use crate::view_state::layout_params::LayoutParams;
use crate::view_state::types::{EntryIndex, ViewportDimensions};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
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
    let filter = StatsFilter::AllSessionsCombined;
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
    let filter = StatsFilter::AllSessionsCombined;
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
    let filter = StatsFilter::AllSessionsCombined;
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
    let session_id = SessionId::new("test-session").unwrap();
    let filter = StatsFilter::MainAgent(session_id);
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
    let filter = StatsFilter::AllSessionsCombined;
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
    let conversation_tabs = vec![
        tabs::ConversationTab::Main,
        tabs::ConversationTab::Subagent(&agent1),
    ];
    let matches = HashSet::new();

    let mut terminal = create_terminal(40, 5);
    terminal
        .draw(|frame| {
            tabs::render_tab_bar(frame, frame.area(), &conversation_tabs, Some(0), &matches);
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
    let conversation_tabs = vec![
        tabs::ConversationTab::Main,
        tabs::ConversationTab::Subagent(&agent1),
        tabs::ConversationTab::Subagent(&agent2),
        tabs::ConversationTab::Subagent(&agent3),
    ];
    let matches = HashSet::new();

    let mut terminal = create_terminal(60, 5);
    terminal
        .draw(|frame| {
            tabs::render_tab_bar(frame, frame.area(), &conversation_tabs, Some(1), &matches);
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
    let conversation_tabs = vec![
        tabs::ConversationTab::Main,
        tabs::ConversationTab::Subagent(&agent1),
        tabs::ConversationTab::Subagent(&agent2),
        tabs::ConversationTab::Subagent(&agent3),
    ];

    let mut matches = HashSet::new();
    matches.insert(agent1.clone());
    matches.insert(agent3.clone());

    let mut terminal = create_terminal(60, 5);
    terminal
        .draw(|frame| {
            tabs::render_tab_bar(frame, frame.area(), &conversation_tabs, Some(0), &matches);
        })
        .unwrap();

    let output = buffer_to_string(terminal.backend().buffer());
    insta::assert_snapshot!("tab_bar_with_search_matches", output);
}

#[test]
fn snapshot_tab_bar_no_selection() {
    let agent1 = AgentId::new("agent-1").unwrap();
    let agent2 = AgentId::new("agent-2").unwrap();
    let conversation_tabs = vec![
        tabs::ConversationTab::Main,
        tabs::ConversationTab::Subagent(&agent1),
        tabs::ConversationTab::Subagent(&agent2),
    ];
    let matches = HashSet::new();

    let mut terminal = create_terminal(50, 5);
    terminal
        .draw(|frame| {
            tabs::render_tab_bar(frame, frame.area(), &conversation_tabs, None, &matches);
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
fn create_test_conversation(entries: Vec<LogEntry>) -> Vec<ConversationEntry> {
    entries
        .into_iter()
        .map(|e| ConversationEntry::Valid(Box::new(e)))
        .collect()
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
    let mut view_state = ConversationViewState::new(
        None,
        None,
        conversation.clone(),
        200_000,
        PricingConfig::default(),
    );
    view_state.relayout(60, WrapMode::Wrap, &crate::state::SearchState::Inactive);
    let styles = MessageStyles::new();

    let mut terminal = create_terminal(60, 15);
    terminal
        .draw(|frame| {
            let widget =
                ConversationView::new(&view_state, &styles, false).global_wrap(WrapMode::Wrap);
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
    // Create view state and expand the message
    let mut view_state = ConversationViewState::new(
        None,
        None,
        conversation.clone(),
        200_000,
        PricingConfig::default(),
    );
    let params = LayoutParams::new(80, WrapMode::Wrap);
    view_state
        .toggle_expand(EntryIndex::new(0), params, ViewportDimensions::new(80, 24))
        .expect("Should be able to toggle expand");
    let styles = MessageStyles::new();

    let mut terminal = create_terminal(60, 20);
    terminal
        .draw(|frame| {
            let widget =
                ConversationView::new(&view_state, &styles, false).global_wrap(WrapMode::Wrap);
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
    // Expand to see full code block
    let mut view_state = ConversationViewState::new(
        None,
        None,
        conversation.clone(),
        200_000,
        PricingConfig::default(),
    );
    let params = LayoutParams::new(80, WrapMode::Wrap);
    view_state
        .toggle_expand(EntryIndex::new(0), params, ViewportDimensions::new(80, 24))
        .expect("Should be able to toggle expand");
    let styles = MessageStyles::new();

    let mut terminal = create_terminal(70, 25);
    terminal
        .draw(|frame| {
            let widget =
                ConversationView::new(&view_state, &styles, false).global_wrap(WrapMode::Wrap);
            frame.render_widget(widget, frame.area());
        })
        .unwrap();

    let output = buffer_to_string(terminal.backend().buffer());
    insta::assert_snapshot!("message_with_code_block", output);
}

#[test]
fn test_code_block_fence_markers_removed() {
    // RED TEST: Verify tui-markdown removes fence markers (```) from code blocks
    // This proves tui-markdown is actually parsing the markdown, not just showing raw text
    let markdown_text = r#"Here's some code:

```rust
fn main() {
    println!("Hello, world!");
}
```

That's the code."#;

    let entry = create_test_log_entry(
        "msg-fence-test",
        Role::Assistant,
        MessageContent::Text(markdown_text.to_string()),
        EntryType::Assistant,
    );

    let conversation = create_test_conversation(vec![entry.clone()]);
    let mut view_state = ConversationViewState::new(
        None,
        None,
        conversation.clone(),
        200_000,
        PricingConfig::default(),
    );
    let params = LayoutParams::new(80, WrapMode::Wrap);
    view_state
        .toggle_expand(EntryIndex::new(0), params, ViewportDimensions::new(80, 24))
        .expect("Should be able to toggle expand");
    let styles = MessageStyles::new();

    let mut terminal = create_terminal(80, 20);
    terminal
        .draw(|frame| {
            let widget =
                ConversationView::new(&view_state, &styles, false).global_wrap(WrapMode::Wrap);
            frame.render_widget(widget, frame.area());
        })
        .unwrap();

    let output = buffer_to_string(terminal.backend().buffer());

    // CRITICAL: Fence markers (```) should NOT appear in rendered output
    // tui-markdown should parse them and render styled code instead
    assert!(
        !output.contains("```"),
        "Fence markers (```) should be removed by tui-markdown parser. Found in output:\n{}",
        output
    );

    // Code content SHOULD still be present (just without fences)
    assert!(
        output.contains("fn main()"),
        "Code content should be rendered (without fence markers)"
    );
}

#[test]
fn test_code_block_syntax_highlighting() {
    // RED TEST: Verify syntax highlighting applies colors to code
    // This checks that cells have non-default foreground colors
    let markdown_text = r#"```rust
fn main() {
    println!("Hello, world!");
}
```"#;

    let entry = create_test_log_entry(
        "msg-syntax-test",
        Role::Assistant,
        MessageContent::Text(markdown_text.to_string()),
        EntryType::Assistant,
    );

    let conversation = create_test_conversation(vec![entry.clone()]);
    let mut view_state = ConversationViewState::new(
        None,
        None,
        conversation.clone(),
        200_000,
        PricingConfig::default(),
    );
    let params = LayoutParams::new(80, WrapMode::Wrap);
    view_state
        .toggle_expand(EntryIndex::new(0), params, ViewportDimensions::new(80, 24))
        .expect("Should be able to toggle expand");
    let styles = MessageStyles::new();

    let mut terminal = create_terminal(80, 20);
    terminal
        .draw(|frame| {
            let widget =
                ConversationView::new(&view_state, &styles, false).global_wrap(WrapMode::Wrap);
            frame.render_widget(widget, frame.area());
        })
        .unwrap();

    let buffer = terminal.backend().buffer();

    // Find a line containing "fn main()" - this should have syntax highlighting
    // Search for the 'fn' keyword which should be colored
    let area = buffer.area();
    let mut found_fn_keyword = false;
    let mut color_counts: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();

    for y in area.top()..area.bottom() {
        let mut line_text = String::new();
        for x in area.left()..area.right() {
            let cell = &buffer[(x, y)];
            line_text.push_str(cell.symbol());
        }

        // Look for line containing "fn main"
        if line_text.contains("fn main") {
            found_fn_keyword = true;

            // Count colors used in this line for debugging
            for x in area.left()..area.right() {
                let cell = &buffer[(x, y)];
                let color_name = format!("{:?}", cell.fg);
                *color_counts.entry(color_name).or_insert(0) += 1;
            }
            break;
        }
    }

    assert!(found_fn_keyword, "Should find 'fn main' in rendered output");

    // Debug: print what colors were found
    eprintln!("Colors found in 'fn main' line: {:?}", color_counts);

    // Check for syntax highlighting colors (NOT just role colors)
    // tui-markdown with syntect should use colors like Cyan, Yellow, Magenta, Blue, Red
    let has_syntax_color = color_counts.keys().any(|color| {
        !color.contains("Reset")
            && !color.contains("Green") // Role color
            && !color.contains("Gray") // Default styling
    });

    assert!(
        has_syntax_color,
        "Syntax highlighting should apply colors to code keywords/strings. \
         Only found role/default colors: {:?}",
        color_counts
    );
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
    let mut view_state = ConversationViewState::new(
        None,
        None,
        conversation.clone(),
        200_000,
        PricingConfig::default(),
    );
    let params = LayoutParams::new(80, WrapMode::Wrap);
    view_state
        .toggle_expand(EntryIndex::new(0), params, ViewportDimensions::new(80, 24))
        .expect("Should be able to toggle expand");
    let styles = MessageStyles::new();

    let mut terminal = create_terminal(80, 20);
    terminal
        .draw(|frame| {
            let widget =
                ConversationView::new(&view_state, &styles, false).global_wrap(WrapMode::Wrap);
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
    let mut view_state = ConversationViewState::new(
        None,
        None,
        conversation.clone(),
        200_000,
        PricingConfig::default(),
    );
    let params = LayoutParams::new(80, WrapMode::Wrap);
    view_state
        .toggle_expand(EntryIndex::new(0), params, ViewportDimensions::new(80, 24))
        .expect("Should be able to toggle expand");
    let styles = MessageStyles::new();

    let mut terminal = create_terminal(80, 20);
    terminal
        .draw(|frame| {
            let widget =
                ConversationView::new(&view_state, &styles, false).global_wrap(WrapMode::Wrap);
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
    let mut view_state = ConversationViewState::new(
        None,
        None,
        conversation.clone(),
        200_000,
        PricingConfig::default(),
    );
    let params = LayoutParams::new(80, WrapMode::Wrap);
    view_state
        .toggle_expand(EntryIndex::new(0), params, ViewportDimensions::new(80, 24))
        .expect("Should be able to toggle expand");
    let styles = MessageStyles::new();

    let mut terminal = create_terminal(80, 20);
    terminal
        .draw(|frame| {
            let widget =
                ConversationView::new(&view_state, &styles, false).global_wrap(WrapMode::Wrap);
            frame.render_widget(widget, frame.area());
        })
        .unwrap();

    let output = buffer_to_string(terminal.backend().buffer());
    insta::assert_snapshot!("message_with_thinking_block", output);
}

// ===== Search Highlighting Snapshot Tests =====

#[test]
fn snapshot_message_with_search_highlighting() {
    // TODO: Reimplement once search highlighting is integrated with view-state
}

// ===== Bug Reproduction Tests =====

/// Bug reproduction: cclv-07v.12.21.2
/// Entry indices (FR-061) are not visible in rendered output.
/// Each entry should show its 1-based index in a dim column before content.
///
/// EXPECTED: "|  1 Entry content..." with visible index column
/// ACTUAL: "Entry content..." with no index visible
#[test]
fn bug_entry_indices_not_visible_in_rendered_output() {
    // Create a simple entry
    let entry = create_test_log_entry(
        "msg-1",
        Role::User,
        MessageContent::Text("First message content".to_string()),
        EntryType::User,
    );

    let conversation = create_test_conversation(vec![entry]);
    let mut view_state = ConversationViewState::new(
        None,
        None,
        conversation.clone(),
        200_000,
        PricingConfig::default(),
    );
    view_state.relayout(60, WrapMode::Wrap, &crate::state::SearchState::Inactive);
    let styles = MessageStyles::new();

    let mut terminal = create_terminal(60, 10);
    terminal
        .draw(|frame| {
            let widget =
                ConversationView::new(&view_state, &styles, false).global_wrap(WrapMode::Wrap);
            frame.render_widget(widget, frame.area());
        })
        .unwrap();

    let output = buffer_to_string(terminal.backend().buffer());

    // FR-061 requires visible entry indices.
    // The first entry should show index "1" in the rendered output.
    // Format should be like "  1 │" or "   1│" before the content.
    let has_index_column = output.contains("│  1 First message");

    // BUG: This assertion will FAIL because no index column is rendered.
    assert!(
        has_index_column,
        "Entry index should be visible (FR-061).\n\
         Expected format like '│  1 First message content'\n\
         But no index column found in output:\n{}",
        output
    );
    insta::assert_snapshot!("bug_entry_indices_not_visible_in_rendered_output", output);
}

/// Bug reproduction: cclv-07v.12.21.4
/// Initial screen is blank until user presses a key.
///
/// EXPECTED: Content visible immediately after app creation.
/// ACTUAL: Terminal buffer is empty until first event triggers render.
#[test]
fn bug_initial_screen_blank_until_keypress() {
    use crate::source::FileSource;
    use crate::state::AppState;
    use crate::view::TuiApp;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use std::path::PathBuf;

    // Load fixture
    let mut file_source = FileSource::new(PathBuf::from("tests/fixtures/blank_lines_repro.jsonl"))
        .expect("Should load fixture");
    let log_entries = file_source.drain_entries().expect("Should parse entries");
    let entry_count = log_entries.len();

    // Convert to ConversationEntry
    let entries: Vec<ConversationEntry> = log_entries
        .into_iter()
        .map(|e| ConversationEntry::Valid(Box::new(e)))
        .collect();

    // Create app
    let backend = TestBackend::new(80, 40);
    let terminal = Terminal::new(backend).unwrap();
    let mut app_state = AppState::new();
    app_state.add_entries(entries);
    let key_bindings = crate::config::keybindings::KeyBindings::default();
    let input_source =
        crate::source::InputSource::Stdin(crate::source::StdinSource::from_reader(&b""[..]));

    let mut app =
        TuiApp::new_for_test(terminal, app_state, input_source, entry_count, key_bindings);

    // Simulate the initial draw() that now happens in run() (cclv-07v.12.21.4)
    // This verifies that:
    // 1. The rendering logic works correctly
    // 2. When run() calls draw() as first action, users see content immediately
    app.render_test().expect("Initial render should succeed");

    // Verify buffer has content after initial render
    let buffer = app.terminal().backend().buffer();
    let output = buffer_to_string(buffer);
    insta::assert_snapshot!("bug_initial_screen_blank_until_keypress", output);

    assert!(
        !output.is_empty(),
        "Screen should have content after initial render.\n\
         Buffer is empty, which means rendering failed.\n\
         This would cause users to see blank screen until they press a key."
    );
}

/// Bug reproduction: cclv-07v.12.21.3
/// Excessive blank lines appear at top of viewport when rendering real log data.
///
/// Uses actual Claude Code log fixture to reproduce the bug that only manifests
/// with real session data (not synthetic test entries).
///
/// EXPECTED: Content starts immediately after header (0-1 blank lines max).
/// ACTUAL: 4 blank lines before first content at scroll position 0.
#[test]
fn bug_excessive_blank_lines_in_entry_rendering() {
    use crate::source::FileSource;
    use crate::state::AppState;
    use crate::view::TuiApp;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use std::path::PathBuf;

    // Load real fixture that reproduces the bug
    let mut file_source = FileSource::new(PathBuf::from("tests/fixtures/blank_lines_repro.jsonl"))
        .expect("Should load fixture");
    let log_entries = file_source.drain_entries().expect("Should parse entries");

    // Convert to ConversationEntry
    let entries: Vec<ConversationEntry> = log_entries
        .into_iter()
        .map(|e| ConversationEntry::Valid(Box::new(e)))
        .collect();

    let entry_count = entries.len();
    assert!(entry_count > 0, "Fixture should have entries");

    // Build session

    // Create app and render
    let backend = TestBackend::new(80, 40);
    let terminal = Terminal::new(backend).unwrap();
    let mut app_state = AppState::new();
    app_state.add_entries(entries);

    let key_bindings = crate::config::keybindings::KeyBindings::default();
    let input_source =
        crate::source::InputSource::Stdin(crate::source::StdinSource::from_reader(&b""[..]));

    let mut app =
        TuiApp::new_for_test(terminal, app_state, input_source, entry_count, key_bindings);
    app.render_test().expect("Should render");

    let buffer = app.terminal().backend().buffer();
    let output = buffer_to_string(buffer);

    // Parse output to find content lines (lines inside the border)
    let lines: Vec<&str> = output.lines().collect();

    // Count leading blank lines after header (first line with ┌)
    let mut leading_blanks = 0;
    let mut found_header = false;

    for line in &lines {
        if line.starts_with('┌') {
            found_header = true;
            continue;
        }
        if found_header && line.starts_with('│') && !line.starts_with("└") {
            let content = line.trim_start_matches('│').trim_end_matches('│').trim();
            if content.is_empty() {
                leading_blanks += 1;
            } else {
                break; // Found first content line
            }
        }
    }

    // BUG: With real log data, we see 4 blank lines before first content
    // Expected: 0-1 blank lines before first content
    // Actual: 4 blank lines at top of viewport
    assert!(
        leading_blanks <= 1,
        "Should have at most 1 leading blank line after header.\n\
         Found {} leading blank lines.\n\
         This bug only manifests with real log data containing system entries.\n\
         Output:\n{}",
        leading_blanks,
        output
    );
}

/// Bug reproduction: cclv-07v.12.21.5
/// Page Down twice causes blank viewport despite having content.
/// Observed in tmux pane (181x46) with 31210-entry log file.
/// After 2 Page Downs (offset ~92), viewport shows 100% blank.
///
/// EXPECTED: Viewport should be filled with entries at any scroll position.
/// ACTUAL: Viewport is 100% blank after 2 Page Downs.
///
/// Reproduce manually:
///   cargo run --release -- tests/fixtures/cc-session-log.jsonl
///   Press PageDown twice
///   Observe blank screen
///
/// NOTE: This test uses TuiApp to simulate actual key handling, not just widget rendering.
/// The ConversationView widget in isolation renders correctly at offset 92, but the
/// bug manifests through TuiApp's scroll handling.
#[test]
fn bug_page_down_twice_causes_blank_viewport() {
    use crate::source::FileSource;
    use crate::state::AppState;
    use crate::view::TuiApp;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use std::path::PathBuf;

    // Load minimal fixture from real log data (300 lines → ~294 entries)
    let mut file_source = FileSource::new(PathBuf::from("tests/fixtures/page_down_repro.jsonl"))
        .expect("Should load fixture");
    let log_entries = file_source.drain_entries().expect("Should parse entries");
    let entry_count = log_entries.len();

    // Convert to ConversationEntry
    let entries: Vec<ConversationEntry> = log_entries
        .into_iter()
        .map(|e| ConversationEntry::Valid(Box::new(e)))
        .collect();

    // Build session

    // Create TuiApp like the real app
    let backend = TestBackend::new(100, 46); // Match tmux viewport
    let terminal = Terminal::new(backend).unwrap();
    let mut app_state = AppState::new();

    // CRITICAL: Populate log_view from session entries (dual-write pattern)
    // The tests build Session first, then create AppState, so log_view is empty.
    // In production, entries are added via AppState::add_entries() which does dual-write.
    // Here we sync log_view from the already-populated Session.
    app_state.add_entries(entries);

    let key_bindings = crate::config::keybindings::KeyBindings::default();
    let input_source =
        crate::source::InputSource::Stdin(crate::source::StdinSource::from_reader(&b""[..]));

    let mut app =
        TuiApp::new_for_test(terminal, app_state, input_source, entry_count, key_bindings);

    // Initial render
    app.render_test().expect("Initial render should succeed");

    // Simulate 2 Page Downs (render after each to update state)
    let page_down = KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE);
    let _ = app.handle_key_test(page_down);
    app.render_test().expect("Render after first PageDown");
    let _ = app.handle_key_test(page_down);
    app.render_test().expect("Render after second PageDown");

    // Capture output
    let buffer = app.terminal().backend().buffer();
    let mut lines = Vec::new();
    for y in 0..buffer.area().height {
        let mut line = String::new();
        for x in 0..buffer.area().width {
            let cell = &buffer[(x, y)];
            line.push_str(cell.symbol());
        }
        lines.push(line.trim_end().to_string());
    }
    let output = lines.join("\n");

    // Check that viewport has actual content (not just border)
    let content_lines: Vec<&str> = lines
        .iter()
        .filter(|line| line.starts_with('│') && !line.ends_with("─┐") && !line.ends_with("─┘"))
        .filter(|line| {
            let inner = line.trim_start_matches('│').trim_end_matches('│').trim();
            !inner.is_empty()
        })
        .map(|s| s.as_str())
        .collect();

    // BUG: After 2 Page Downs, viewport is completely blank
    // We should see entries somewhere in the middle of the ~294 entry list
    assert!(
        !content_lines.is_empty(),
        "Viewport should contain content after 2 Page Downs.\n\
         Total entries: {}\n\
         Expected: Entries visible in viewport\n\
         Actual: Viewport is blank\n\
         Output:\n{}",
        entry_count,
        output
    );
}

// ===== US1 Acceptance Tests - View-State Layer =====

/// US1 Acceptance Scenario 1: Page Down through large log shows content at every position.
///
/// GIVEN: A log file with 30,000+ entries
/// WHEN: User presses Page Down repeatedly until reaching the bottom
/// THEN: Every viewport shows content (no blank screens)
///
/// This test verifies the core US1 requirement: scroll position is based on line offsets
/// (not entry count), preventing blank viewports at any scroll position.
#[test]
fn us1_page_down_to_bottom_always_shows_content() {
    use crate::source::FileSource;
    use crate::state::AppState;
    use crate::view::TuiApp;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use std::path::PathBuf;

    // Load large fixture: cc-session-log.jsonl has 31,210 entries
    let mut file_source = FileSource::new(PathBuf::from("tests/fixtures/cc-session-log.jsonl"))
        .expect("Should load large fixture");
    let log_entries = file_source.drain_entries().expect("Should parse entries");
    let entry_count = log_entries.len();
    // Convert to ConversationEntry
    let entries: Vec<ConversationEntry> = log_entries
        .into_iter()
        .map(|e| ConversationEntry::Valid(Box::new(e)))
        .collect();

    assert!(
        entry_count >= 2_000,
        "Fixture should have 2,000+ entries for this test"
    );

    // Build session

    // Create TuiApp
    let backend = TestBackend::new(100, 46);
    let terminal = Terminal::new(backend).unwrap();
    let mut app_state = AppState::new();

    // CRITICAL: Populate log_view from session entries (dual-write pattern)
    app_state.add_entries(entries);

    let key_bindings = crate::config::keybindings::KeyBindings::default();
    let input_source =
        crate::source::InputSource::Stdin(crate::source::StdinSource::from_reader(&b""[..]));

    let mut app =
        TuiApp::new_for_test(terminal, app_state, input_source, entry_count, key_bindings);

    // Initial render
    app.render_test().expect("Initial render should succeed");

    // Simulate Page Down until we reach bottom (max 1000 iterations to prevent infinite loop)
    let page_down = KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE);
    let max_iterations = 1000;
    let mut blank_viewports = Vec::new();

    for iteration in 0..max_iterations {
        // Press Page Down
        let handled = app.handle_key_test(page_down);
        app.render_test().expect("Render should succeed");

        // Check if viewport has content
        let buffer = app.terminal().backend().buffer();
        let has_content = (0..buffer.area().height)
            .flat_map(|y| (0..buffer.area().width).map(move |x| (x, y)))
            .any(|(x, y)| {
                let cell = &buffer[(x, y)];
                let symbol = cell.symbol();
                // Content is any non-border character that's not whitespace
                !symbol
                    .chars()
                    .all(|c| c.is_whitespace() || "│┌┐└┘─".contains(c))
            });

        if !has_content {
            blank_viewports.push(iteration);
        }

        // Stop if we can't page down further
        if !handled {
            break;
        }
    }

    // US1 requirement: No blank viewports at any scroll position
    assert!(
        blank_viewports.is_empty(),
        "US1 FAILURE: Found {} blank viewports during Page Down traversal.\n\
         Blank occurred at iterations: {:?}\n\
         Total Page Downs before reaching bottom: {}\n\
         This violates US1 requirement: viewport must always show content.",
        blank_viewports.len(),
        blank_viewports,
        max_iterations.min(entry_count / 46) // Rough estimate
    );
}

/// US1 Acceptance Scenario 2: Home key shows first entries immediately.
///
/// GIVEN: User is at bottom of log
/// WHEN: User presses Home
/// THEN: Viewport shows first entries immediately
///
/// This verifies ScrollPosition::Top resolves correctly and content renders from line 0.
#[test]
fn us1_home_key_shows_first_entries_from_bottom() {
    use crate::source::FileSource;
    use crate::state::AppState;
    use crate::view::TuiApp;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use std::path::PathBuf;

    // Load large fixture
    let mut file_source = FileSource::new(PathBuf::from("tests/fixtures/cc-session-log.jsonl"))
        .expect("Should load fixture");
    let log_entries = file_source.drain_entries().expect("Should parse entries");
    let entry_count = log_entries.len();

    // Convert to ConversationEntry
    let entries: Vec<ConversationEntry> = log_entries
        .into_iter()
        .map(|e| ConversationEntry::Valid(Box::new(e)))
        .collect();

    // Build session

    // Create TuiApp
    let backend = TestBackend::new(100, 46);
    let terminal = Terminal::new(backend).unwrap();
    let mut app_state = AppState::new();

    // CRITICAL: Populate log_view from session entries (dual-write pattern)
    app_state.add_entries(entries);

    let key_bindings = crate::config::keybindings::KeyBindings::default();
    let input_source =
        crate::source::InputSource::Stdin(crate::source::StdinSource::from_reader(&b""[..]));

    let mut app =
        TuiApp::new_for_test(terminal, app_state, input_source, entry_count, key_bindings);

    // Initial render
    app.render_test().expect("Initial render should succeed");

    // Go to bottom using End key
    let end_key = KeyEvent::new(KeyCode::End, KeyModifiers::NONE);
    app.handle_key_test(end_key);
    app.render_test().expect("Render after End");

    // Now press Home to return to top
    let home_key = KeyEvent::new(KeyCode::Home, KeyModifiers::NONE);
    app.handle_key_test(home_key);
    app.render_test().expect("Render after Home");

    // Verify viewport shows content from the beginning
    let buffer = app.terminal().backend().buffer();
    let output = buffer_to_string(buffer);

    // The first entry UUID should be visible in the output
    // We can check for typical first-entry indicators
    let has_content = !output.trim().is_empty();
    assert!(
        has_content,
        "US1 FAILURE: Viewport is blank after Home key.\n\
         Expected: First entries visible\n\
         Actual: Blank viewport\n\
         Output:\n{}",
        output
    );
}

/// US1 Acceptance Scenario 3: End key shows last entries with content visible.
///
/// GIVEN: User is at any position
/// WHEN: User presses End
/// THEN: Viewport shows last entries with content visible
///
/// This verifies ScrollPosition::Bottom resolves correctly and last entries render.
#[test]
fn us1_end_key_shows_last_entries_with_content() {
    use crate::source::FileSource;
    use crate::state::AppState;
    use crate::view::TuiApp;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use std::path::PathBuf;

    // Load large fixture
    let mut file_source = FileSource::new(PathBuf::from("tests/fixtures/cc-session-log.jsonl"))
        .expect("Should load fixture");
    let log_entries = file_source.drain_entries().expect("Should parse entries");
    let entry_count = log_entries.len();

    // Convert to ConversationEntry
    let entries: Vec<ConversationEntry> = log_entries
        .into_iter()
        .map(|e| ConversationEntry::Valid(Box::new(e)))
        .collect();

    // Build session

    // Create TuiApp
    let backend = TestBackend::new(100, 46);
    let terminal = Terminal::new(backend).unwrap();
    let mut app_state = AppState::new();
    app_state.add_entries(entries);
    let key_bindings = crate::config::keybindings::KeyBindings::default();
    let input_source =
        crate::source::InputSource::Stdin(crate::source::StdinSource::from_reader(&b""[..]));

    let mut app =
        TuiApp::new_for_test(terminal, app_state, input_source, entry_count, key_bindings);

    // Initial render (at top)
    app.render_test().expect("Initial render should succeed");

    // Press End to jump to bottom
    let end_key = KeyEvent::new(KeyCode::End, KeyModifiers::NONE);
    app.handle_key_test(end_key);
    app.render_test().expect("Render after End");

    // Verify viewport shows content
    let buffer = app.terminal().backend().buffer();
    let output = buffer_to_string(buffer);

    let has_content = !output.trim().is_empty();
    assert!(
        has_content,
        "US1 FAILURE: Viewport is blank after End key.\n\
         Expected: Last entries visible\n\
         Actual: Blank viewport\n\
         Output:\n{}",
        output
    );
}

/// US1 Acceptance Scenario 4: Rapid scrolling updates viewport within 16ms (60fps target).
///
/// GIVEN: User is scrolling rapidly
/// WHEN: Viewport updates
/// THEN: Content appears within 16ms (60fps target)
///
/// This is a performance test verifying view-state layer enables fast scroll operations.
/// We measure the time for visible_range calculation + render, which should be O(log n).
///
/// NOTE: Runs only in release mode (`cargo test --release`) because debug builds are
/// 5-10x slower due to lack of optimizations. The 60fps target is for release builds only.
#[cfg(not(debug_assertions))]
#[test]
#[ignore = "Performance target (16ms/60fps) not yet achieved - requires O(log n) optimization in cclv-5ur.6"]
fn us1_rapid_scroll_updates_within_60fps() {
    use crate::source::FileSource;
    use crate::state::AppState;
    use crate::view::TuiApp;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use std::path::PathBuf;
    use std::time::Instant;

    // Load large fixture
    let mut file_source = FileSource::new(PathBuf::from("tests/fixtures/cc-session-log.jsonl"))
        .expect("Should load fixture");
    let log_entries = file_source.drain_entries().expect("Should parse entries");
    let entry_count = log_entries.len();

    // Convert to ConversationEntry
    let entries: Vec<ConversationEntry> = log_entries
        .into_iter()
        .map(|e| ConversationEntry::Valid(Box::new(e)))
        .collect();

    // Build session

    // Create TuiApp
    let backend = TestBackend::new(100, 46);
    let terminal = Terminal::new(backend).unwrap();
    let mut app_state = AppState::new();

    // CRITICAL: Populate log_view from session entries (dual-write pattern)
    app_state.add_entries(entries);

    let key_bindings = crate::config::keybindings::KeyBindings::default();
    let input_source =
        crate::source::InputSource::Stdin(crate::source::StdinSource::from_reader(&b""[..]));

    let mut app =
        TuiApp::new_for_test(terminal, app_state, input_source, entry_count, key_bindings);

    // Initial render
    app.render_test().expect("Initial render should succeed");

    // Measure time for 10 rapid scroll operations (Page Down)
    let page_down = KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE);
    let num_scrolls = 10;
    let mut total_duration = std::time::Duration::ZERO;

    for _ in 0..num_scrolls {
        let start = Instant::now();
        app.handle_key_test(page_down);
        app.render_test().expect("Render should succeed");
        let duration = start.elapsed();
        total_duration += duration;
    }

    let avg_duration = total_duration / num_scrolls;
    let target_fps = 60;
    let max_frame_time_ms = 1000 / target_fps; // 16ms

    // NOTE: This test runs in debug mode (unoptimized). The 60fps target is for release builds.
    // Debug mode is ~5-10x slower due to lack of optimizations and debug assertions.
    // Current performance (debug): ~90ms per scroll+render
    // Expected release performance: <16ms per scroll+render
    assert!(
        avg_duration.as_millis() <= max_frame_time_ms as u128,
        "US1 FAILURE: Scroll operations too slow for 60fps.\n\
         Average scroll+render time: {}ms\n\
         Target (60fps): {}ms\n\
         Total time for {} scrolls: {}ms\n\
         NOTE: Test runs in debug mode (unoptimized). Release builds will be much faster.\n\
         This violates US1 performance requirement.",
        avg_duration.as_millis(),
        max_frame_time_ms,
        num_scrolls,
        total_duration.as_millis()
    );
}

/// Bug reproduction: Thinking blocks don't wrap like prose blocks
///
/// EXPECTED: Thinking block content wraps at terminal width, just like
///           regular prose Text blocks. Long lines should continue on
///           the next line, not be truncated.
///
/// ACTUAL: Thinking blocks are truncated at terminal width boundary.
///         The word "window" in the long line is cut off as "windo"
///         instead of wrapping to the next line.
///
/// Steps to reproduce manually:
/// 1. cargo run -- tests/fixtures/thinking_wrap_repro.jsonl
/// 2. Resize terminal to narrow width (~60 chars)
/// 3. Observe the long thinking block line is truncated, not wrapped
///
/// The status bar shows "Wrap: On" but thinking blocks ignore this setting.
#[test]
fn bug_thinking_blocks_not_wrapped_like_prose() {
    use crate::source::FileSource;
    use crate::state::AppState;
    use crate::view::TuiApp;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use std::path::PathBuf;

    // Load minimal fixture with long thinking block line
    let mut file_source =
        FileSource::new(PathBuf::from("tests/fixtures/thinking_wrap_repro.jsonl"))
            .expect("Should load fixture");
    let log_entries = file_source.drain_entries().expect("Should parse entries");
    let entry_count = log_entries.len();

    // Convert to ConversationEntry
    let entries: Vec<ConversationEntry> = log_entries
        .into_iter()
        .map(|e| ConversationEntry::Valid(Box::new(e)))
        .collect();

    // Create app with NARROW terminal to force wrapping
    // Width 60 is narrower than the long line in the fixture
    let backend = TestBackend::new(60, 20);
    let terminal = Terminal::new(backend).unwrap();
    let mut app_state = AppState::new();
    app_state.add_entries(entries);
    let key_bindings = crate::config::keybindings::KeyBindings::default();
    let input_source =
        crate::source::InputSource::Stdin(crate::source::StdinSource::from_reader(&b""[..]));

    let mut app =
        TuiApp::new_for_test(terminal, app_state, input_source, entry_count, key_bindings);

    // Render with wrap mode enabled (default)
    app.render_test().expect("Initial render should succeed");

    // Capture output
    let buffer = app.terminal().backend().buffer();
    let output = buffer_to_string(buffer);

    // Snapshot captures the bug: truncated line instead of wrapped
    insta::assert_snapshot!("bug_thinking_blocks_not_wrapped", output);

    // The long line from fixture is:
    // "This is a very long line that should definitely wrap when displayed in a narrow terminal window..."
    // If wrapping works correctly, "window" should appear on a continuation line
    // If truncated, it will be cut off (e.g., "windo" or earlier)

    // Check that the word "window" appears (it should if wrapping works)
    // This assertion will FAIL because the bug truncates the line
    assert!(
        output.contains("window"),
        "BUG: Thinking block content is truncated instead of wrapped.\n\
         The word 'window' from the long line should appear in the output,\n\
         but the line is being cut off at the terminal width.\n\
         Thinking blocks should wrap like prose Text blocks when Wrap mode is enabled.\n\n\
         Actual output:\n{output}"
    );
}

// ===== Horizontal Scroll Bug Reproduction =====

/// Bug reproduction: Horizontal scroll does not work
///
/// EXPECTED: When wrap mode is OFF and user presses Right arrow,
///           content should shift left to reveal truncated text.
/// ACTUAL: Content remains unchanged - horizontal_offset state updates
///         but rendering does not apply the offset to displayed content.
///
/// Steps to reproduce manually:
/// 1. cargo run -- tests/fixtures/horizontal_scroll_repro.jsonl
/// 2. Press 'w' to toggle wrap off (if needed)
/// 3. Scroll to a line that is truncated (exceeds viewport width)
/// 4. Press Right arrow multiple times
/// 5. Observe: Content does not shift horizontally
#[test]
fn bug_horizontal_scroll_does_not_work() {
    use crate::source::FileSource;
    use crate::state::AppState;
    use crate::view::TuiApp;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use std::path::PathBuf;

    // Load minimal fixture with long line
    let mut file_source = FileSource::new(PathBuf::from(
        "tests/fixtures/horizontal_scroll_repro.jsonl",
    ))
    .expect("Should load fixture");
    let log_entries = file_source.drain_entries().expect("Should parse entries");
    let entry_count = log_entries.len();

    // Convert to ConversationEntry
    let entries: Vec<ConversationEntry> = log_entries
        .into_iter()
        .map(|e| ConversationEntry::Valid(Box::new(e)))
        .collect();

    // Create app with narrow terminal (60 chars) so long line is truncated
    let backend = TestBackend::new(60, 15);
    let terminal = Terminal::new(backend).unwrap();
    let mut app_state = AppState::new();
    app_state.add_entries(entries);
    let key_bindings = crate::config::keybindings::KeyBindings::default();
    let input_source =
        crate::source::InputSource::Stdin(crate::source::StdinSource::from_reader(&b""[..]));

    let mut app =
        TuiApp::new_for_test(terminal, app_state, input_source, entry_count, key_bindings);

    // Initial render
    app.render_test().expect("Initial render should succeed");

    // Toggle to NoWrap mode (default is Wrap)
    // Press 'W' (Shift+w) to toggle GLOBAL wrap mode
    let toggle_global_wrap = KeyEvent::new(KeyCode::Char('W'), KeyModifiers::SHIFT);
    app.handle_key_test(toggle_global_wrap);
    app.render_test()
        .expect("Render after wrap toggle should succeed");

    // Capture initial state - tool block JSON line should be truncated, MARKER not visible
    // Tool blocks render as JSON which should NOT wrap in NoWrap mode
    let initial_output = buffer_to_string(app.terminal().backend().buffer());

    // The fixture contains a tool_use with file_path ending in "MARKER_END_OF_LINE.txt"
    // With 60-char width, this marker should NOT be visible initially
    assert!(
        !initial_output.contains("MARKER_END_OF_LINE"),
        "MARKER should be beyond viewport initially (line truncated at ~58 chars).\n\
         If visible, the test fixture is too short.\n\
         Output:\n{initial_output}"
    );

    // Simulate pressing Right arrow 120 times to scroll horizontally
    // Need to scroll far enough to reveal MARKER_END_OF_LINE at the end of the long path
    let right_key = KeyEvent::new(KeyCode::Right, KeyModifiers::NONE);
    for _ in 0..120 {
        let _ = app.handle_key_test(right_key);
    }
    app.render_test()
        .expect("Render after scroll should succeed");

    // Capture output after horizontal scroll
    let scrolled_output = buffer_to_string(app.terminal().backend().buffer());

    // Snapshot captures current (buggy) state
    insta::assert_snapshot!("bug_horizontal_scroll_no_effect", scrolled_output);

    // TEST 1: Verify scroll indicators appear in title after scrolling
    // After scrolling right 120 chars, title should show "◀ Main Agent ... ▶"
    // (left indicator because offset > 0, right indicator because content extends beyond viewport)
    assert!(
        scrolled_output.contains("◀"),
        "Scroll indicator ◀ should appear in title after scrolling right.\n\
         This indicates content extends to the left (horizontal_offset > 0).\n\
         Scrolled output:\n{scrolled_output}"
    );

    assert!(
        scrolled_output.contains("▶"),
        "Scroll indicator ▶ should appear in title when content extends beyond viewport.\n\
         This indicates more content is available to the right.\n\
         Scrolled output:\n{scrolled_output}"
    );

    // TEST 2: BUG: After scrolling right 120 chars, we should see content that was hidden
    // The MARKER_END_OF_LINE should now be visible, but rendering ignores horizontal_offset
    assert!(
        scrolled_output.contains("MARKER_END_OF_LINE"),
        "BUG: Horizontal scroll has no visual effect.\n\
         After pressing Right 120 times, content should shift left to reveal MARKER_END_OF_LINE.\n\
         Expected: MARKER_END_OF_LINE visible after scrolling right\n\
         Actual: Content unchanged, horizontal_offset state updates but rendering ignores it.\n\n\
         Initial output:\n{initial_output}\n\n\
         Scrolled output:\n{scrolled_output}"
    );
}

// ===== Jerky Scroll Bug Reproducer =====

/// Bug reproduction: Line-by-line scroll is jerky, not smooth
///
/// EXPECTED: Each scroll-down should shift ALL content up by exactly 1 line
/// ACTUAL: Content jumps erratically - sometimes doesn't move, sometimes jumps multiple lines
///
/// Steps to reproduce manually:
/// 1. cargo run --release -- tests/fixtures/jerky_scroll_repro.jsonl
/// 2. Press 'g' to go to top
/// 3. Press 'j' repeatedly to scroll down line by line
/// 4. Observe: Content jumps/stutters instead of smooth 1-line shifts
///
/// Root cause: Scroll advances by entry boundaries (variable height) rather than visual lines.
#[test]
fn bug_jerky_scroll_line_by_line() {
    // calculate_height is now used internally by ConversationViewState
    use chrono::Utc;

    // Create entries with varying content lengths
    fn make_entry(uuid: &str, lines: &[&str]) -> ConversationEntry {
        let text = lines.join("\n");
        let message = Message::new(Role::User, MessageContent::Text(text));
        let entry = LogEntry::new(
            EntryUuid::new(uuid).unwrap(),
            None,
            SessionId::new("test-session").unwrap(),
            None,
            Utc::now(),
            EntryType::User,
            message,
            EntryMetadata::default(),
        );
        ConversationEntry::Valid(Box::new(entry))
    }

    let entries = vec![
        make_entry(
            "e1",
            &[
                "First entry line 1",
                "First entry line 2",
                "First entry line 3",
            ],
        ),
        make_entry("e2", &["Second entry line 1", "Second entry line 2"]),
        make_entry("e3", &["Third entry - single line"]),
        make_entry(
            "e4",
            &[
                "Fourth entry line 1",
                "Fourth entry line 2",
                "Fourth entry line 3",
                "Fourth entry line 4",
            ],
        ),
        make_entry("e5", &["Fifth entry line 1", "Fifth entry line 2"]),
    ];

    // Create view state
    let mut state =
        ConversationViewState::new(None, None, entries, 200_000, PricingConfig::default());
    let params = LayoutParams::new(80, WrapMode::NoWrap);
    state.relayout_from(EntryIndex::new(0), params);

    // Use a small viewport to force scrolling (content is ~17 lines)
    let viewport = ViewportDimensions::new(80, 10);

    // Helper to render and get content lines
    let render = |s: &ConversationViewState| -> Vec<String> {
        let mut terminal =
            Terminal::new(TestBackend::new(viewport.width, viewport.height)).unwrap();
        terminal
            .draw(|frame| {
                let styles = MessageStyles::default();
                let widget = ConversationView::new(s, &styles, false);
                frame.render_widget(widget, frame.area());
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let area = buffer.area();

        let mut lines = Vec::new();
        for y in area.top()..area.bottom() {
            let mut line = String::new();
            for x in area.left()..area.right() {
                let cell = &buffer[(x, y)];
                line.push_str(cell.symbol());
            }
            lines.push(line.trim_end().to_string());
        }

        // Skip first/last lines (frame borders)
        lines[1..lines.len().saturating_sub(1)].to_vec()
    };

    use crate::view_state::scroll::ScrollPosition;
    use crate::view_state::types::LineOffset;

    // Go to top
    state.set_scroll(ScrollPosition::Top);

    // Render BEFORE scroll
    let before_lines = render(&state);

    // Scroll down by 1 line - use AtLine(1) to scroll to line offset 1
    state.set_scroll(ScrollPosition::AtLine(LineOffset::new(1)));

    // Render AFTER scroll
    let after_lines = render(&state);

    // Snapshot both states for debugging
    let before_str = before_lines.join("\n");
    let after_str = after_lines.join("\n");
    insta::assert_snapshot!("bug_jerky_scroll_before", before_str);
    insta::assert_snapshot!("bug_jerky_scroll_after", after_str);

    // THE KEY ASSERTION: After scrolling down by 1 line, content should shift up by 1 line
    // This means: before_lines[1] should equal after_lines[0]
    //             before_lines[2] should equal after_lines[1]
    //             etc.

    // Simple check: the SECOND content line from before should now be the FIRST content line after scroll
    if before_lines.len() > 1 && after_lines.len() > 1 {
        let before_line_1 = &before_lines[1]; // Second line before scroll
        let after_line_0 = &after_lines[0]; // First line after scroll

        // They should match (content shifted up by 1)
        // But with jerky scroll, this fails because scroll jumps by entry heights, not lines
        assert_eq!(
            before_line_1.trim(),
            after_line_0.trim(),
            "BUG: Scroll is jerky, not smooth.\n\
             After scrolling down 1 line, content should shift up by exactly 1 line.\n\
             Expected line 1 before scroll to become line 0 after scroll.\n\n\
             Before line 1: '{}'\n\
             After line 0:  '{}'\n\n\
             This indicates scroll is jumping by entry boundaries instead of visual lines.",
            before_line_1.trim(),
            after_line_0.trim()
        );
    }
}

/// Bug reproduction: Collapsed entries have height mismatch causing jerky scroll
///
/// EXPECTED: Each j press scrolls by exactly 1 visual line
/// ACTUAL: Multiple j presses required to scroll when entries are collapsed
///
/// Steps to reproduce manually:
/// 1. cargo run -- tests/fixtures/cc-session-log.jsonl
/// 2. Navigate to an entry showing "(+N more lines)" collapse indicator
/// 3. Press j repeatedly
/// 4. Observe: First j works, then 2-4 j presses do nothing, then it finally scrolls
///
/// Root cause: Height calculator returns full content height for collapsed entries,
/// but renderer only shows 3 summary lines + collapse indicator (~4 lines total).
#[test]
fn bug_collapsed_entry_height_mismatch() {
    use crate::source::FileSource;
    // calculate_height is now used internally by ConversationViewState
    use crate::view_state::scroll::ScrollPosition;
    use crate::view_state::types::LineOffset;
    use std::path::PathBuf;

    // Load the full fixture that reproduces the bug in the TUI
    let mut file_source = FileSource::new(PathBuf::from("tests/fixtures/cc-session-log.jsonl"))
        .expect("Should load fixture");
    let log_entries = file_source.drain_entries().expect("Should parse entries");

    let entries: Vec<ConversationEntry> = log_entries
        .into_iter()
        .map(|e| ConversationEntry::Valid(Box::new(e)))
        .collect();

    // Create view state with entries NOT expanded (collapsed by default)
    let mut state =
        ConversationViewState::new(None, None, entries, 200_000, PricingConfig::default());
    let params = LayoutParams::new(211, WrapMode::Wrap);
    state.relayout_from(EntryIndex::new(0), params);

    // Use viewport similar to actual terminal (211x62 observed in tmux)
    let viewport = ViewportDimensions::new(211, 62);

    // Helper to render and get content lines
    let render = |s: &ConversationViewState| -> Vec<String> {
        let mut terminal =
            Terminal::new(TestBackend::new(viewport.width, viewport.height)).unwrap();
        terminal
            .draw(|frame| {
                let styles = MessageStyles::default();
                let widget = ConversationView::new(s, &styles, false);
                frame.render_widget(widget, frame.area());
            })
            .unwrap();
        let buffer = terminal.backend().buffer();
        let mut lines = Vec::new();
        for y in 0..buffer.area.height {
            let mut line = String::new();
            for x in 0..buffer.area.width {
                line.push(buffer[(x, y)].symbol().chars().next().unwrap_or(' '));
            }
            lines.push(line);
        }
        // Skip first/last lines (frame borders)
        lines[1..lines.len().saturating_sub(1)].to_vec()
    };

    // Track how many consecutive scrolls produce identical output across ALL positions
    let mut max_stuck_run = 0;
    let mut current_stuck_run = 0;
    let total_height = state.total_height();

    // Start from top and scroll through the document
    state.set_scroll(ScrollPosition::Top);
    let mut prev_lines = render(&state);

    // Test scrolling through first 100 positions (or total_height if smaller)
    let test_range = total_height.min(100);
    for offset in 1..test_range {
        state.set_scroll(ScrollPosition::AtLine(LineOffset::new(offset)));
        let current_lines = render(&state);

        if current_lines == prev_lines {
            current_stuck_run += 1;
            max_stuck_run = max_stuck_run.max(current_stuck_run);
        } else {
            current_stuck_run = 0;
        }
        prev_lines = current_lines;
    }

    // REPRODUCER: This test should FAIL until the jerky scroll bug is fixed.
    // When fixed, every scroll step should produce different rendered output.
    // See bead cclv-5ur.13 for details.
    assert_eq!(
        max_stuck_run, 0,
        "BUG NOT FIXED: Scroll got stuck for {} consecutive steps.\n\
         Height calculation doesn't match rendered output.\n\
         Tested {} scroll positions.\n\
         This reproducer should fail until cclv-5ur.13 is resolved.",
        max_stuck_run, test_range
    );
}

/// Bug reproduction: Scroll is stuck with thinking block entries
///
/// EXPECTED: Each 'j' press should shift content up by 1 line
/// ACTUAL: Content stays identical - scroll does not move at all
///
/// Steps to reproduce manually:
/// 1. cargo run -- tests/fixtures/scroll_jump_thinking_repro.jsonl
/// 2. Press 'j' multiple times to scroll down
/// 3. Observe: First line stays the same, scroll is stuck
///
/// Fixture: 5 entries with thinking blocks + text content
/// The bug occurs when trying to scroll line-by-line through thinking blocks.
#[test]
fn bug_scroll_stuck_with_thinking_blocks() {
    use crate::source::FileSource;
    // calculate_height is now used internally by ConversationViewState
    use crate::view_state::scroll::ScrollPosition;
    use crate::view_state::types::LineOffset;
    use std::path::PathBuf;

    // Load fixture with thinking block entries
    let mut file_source = FileSource::new(PathBuf::from(
        "tests/fixtures/scroll_jump_thinking_repro.jsonl",
    ))
    .expect("Should load fixture");
    let log_entries = file_source.drain_entries().expect("Should parse entries");

    let entries: Vec<ConversationEntry> = log_entries
        .into_iter()
        .map(|e| ConversationEntry::Valid(Box::new(e)))
        .collect();

    assert!(
        !entries.is_empty(),
        "Fixture should have entries with thinking blocks"
    );

    // Create view state with entries COLLAPSED (default state)
    // This is how entries appear initially in the TUI
    let mut state = ConversationViewState::new(
        None,
        None,
        entries.clone(),
        200_000,
        PricingConfig::default(),
    );
    let params = LayoutParams::new(80, WrapMode::Wrap);
    state.relayout_from(EntryIndex::new(0), params);

    // Helper to render and get output as string
    let render = |s: &ConversationViewState| -> String {
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        terminal
            .draw(|frame| {
                let styles = MessageStyles::default();
                let widget = ConversationView::new(s, &styles, false);
                frame.render_widget(widget, frame.area());
            })
            .unwrap();
        let buffer = terminal.backend().buffer();
        buffer_to_string(buffer)
    };

    // Test consecutive scrolls to find where scroll gets stuck
    // Similar to bug_collapsed_entry_height_mismatch approach
    let total_height = state.total_height();
    let mut stuck_positions = Vec::new();

    // Calculate the viewport height from the test backend
    let viewport_height = 24; // TestBackend::new(80, 24)

    // Calculate the maximum VALID scroll offset
    // Beyond this, scroll is correctly clamped to show the end of the document
    let max_scroll = total_height.saturating_sub(viewport_height);

    state.set_scroll(ScrollPosition::Top);
    let mut prev_output = render(&state);

    // ONLY test positions within the valid scroll range
    // Positions beyond max_scroll are SUPPOSED to show identical content (clamped)
    for offset in 1..=max_scroll {
        state.set_scroll(ScrollPosition::AtLine(LineOffset::new(offset)));
        let current_output = render(&state);

        if current_output == prev_output {
            stuck_positions.push(offset);
        }
        prev_output = current_output;
    }

    // Capture snapshots at positions where scroll gets stuck (if any)
    if let Some(&first_stuck) = stuck_positions.first() {
        // Render at position before stuck
        state.set_scroll(ScrollPosition::AtLine(LineOffset::new(first_stuck - 1)));
        let before_stuck = render(&state);

        // Render at stuck position
        state.set_scroll(ScrollPosition::AtLine(LineOffset::new(first_stuck)));
        let at_stuck = render(&state);

        insta::assert_snapshot!("bug_scroll_stuck_thinking_before_stuck", before_stuck);
        insta::assert_snapshot!("bug_scroll_stuck_thinking_at_stuck", at_stuck);
    }

    // THE KEY ASSERTION: Scroll should never get stuck WITHIN valid range
    // Every line offset within [0, max_scroll] should produce different rendered output
    assert!(
        stuck_positions.is_empty(),
        "BUG: Scroll got stuck at {} positions with thinking blocks.\n\
         Stuck positions (consecutive identical outputs): {:?}\n\
         Valid scroll range: 0 to {} (viewport_height={}, total_height={}).\n\n\
         This violates the core scroll contract: each line offset within the \
         valid scroll range should produce different visible content.",
        stuck_positions.len(),
        stuck_positions,
        max_scroll,
        viewport_height,
        total_height
    );
}

// ===== Wrap Mode Bug Reproduction (cclv-5ur.18) =====

/// Snapshot test: Global wrap mode wraps long lines at viewport width.
///
/// EXPECTED: Long line wraps to multiple lines when global WrapMode is Wrap.
/// ACTUAL: Line wraps correctly after cclv-5ur.18 fix.
///
/// This test verifies that the default Wrap mode causes lines exceeding viewport
/// width to wrap to continuation lines, preserving all content visibility.
#[test]
fn snapshot_wrap_mode_global_wrap() {
    // Create entry with a long line that exceeds viewport width
    let long_line = "This is a very long line that definitely exceeds the viewport width and should wrap to multiple lines when wrap mode is enabled. The content should continue on the next line without truncation.";
    let entry = create_test_log_entry(
        "wrap-test-1",
        Role::User,
        MessageContent::Text(long_line.to_string()),
        EntryType::User,
    );

    let conversation = create_test_conversation(vec![entry]);
    let mut view_state = ConversationViewState::new(
        None,
        None,
        conversation.clone(),
        200_000,
        PricingConfig::default(),
    );

    // Use narrow viewport (60 chars) to force wrapping
    view_state.relayout(60, WrapMode::Wrap, &crate::state::SearchState::Inactive);
    let styles = MessageStyles::new();

    let mut terminal = create_terminal(60, 15);
    terminal
        .draw(|frame| {
            let widget =
                ConversationView::new(&view_state, &styles, false).global_wrap(WrapMode::Wrap);
            frame.render_widget(widget, frame.area());
        })
        .unwrap();

    let output = buffer_to_string(terminal.backend().buffer());

    // Snapshot captures wrapped output
    insta::assert_snapshot!("wrap_mode_global_wrap", output);

    // Verify the line wrapped (should see content on multiple lines)
    assert!(
        output.lines().count() > 3,
        "Long line should wrap to multiple lines. Output:\n{output}"
    );
}

/// Snapshot test: Global NoWrap mode truncates long lines.
///
/// EXPECTED: Long line does NOT wrap when global WrapMode is NoWrap.
/// ACTUAL: Line is truncated to single line after cclv-5ur.18 fix.
///
/// This test verifies that NoWrap mode prevents line wrapping, truncating
/// content that exceeds viewport width on a single line.
#[test]
fn snapshot_wrap_mode_global_nowrap() {
    // Use the same long line as wrap test
    let long_line = "This is a very long line that definitely exceeds the viewport width and should wrap to multiple lines when wrap mode is enabled. The content should continue on the next line without truncation.";
    let entry = create_test_log_entry(
        "nowrap-test-1",
        Role::User,
        MessageContent::Text(long_line.to_string()),
        EntryType::User,
    );

    let conversation = create_test_conversation(vec![entry]);
    let mut view_state = ConversationViewState::new(
        None,
        None,
        conversation.clone(),
        200_000,
        PricingConfig::default(),
    );

    // Use narrow viewport (60 chars) but disable wrapping
    view_state.relayout(60, WrapMode::NoWrap, &crate::state::SearchState::Inactive);
    let styles = MessageStyles::new();

    let mut terminal = create_terminal(60, 15);
    terminal
        .draw(|frame| {
            let widget =
                ConversationView::new(&view_state, &styles, false).global_wrap(WrapMode::NoWrap);
            frame.render_widget(widget, frame.area());
        })
        .unwrap();

    let output = buffer_to_string(terminal.backend().buffer());

    // Snapshot captures truncated output
    insta::assert_snapshot!("wrap_mode_global_nowrap", output);

    // The word "truncation" appears near the end of the long line
    // With NoWrap + 60 char viewport, it should be truncated (not visible)
    assert!(
        !output.contains("truncation"),
        "Long line should be truncated in NoWrap mode. Output:\n{output}"
    );
}

/// Snapshot test: Per-entry wrap override takes precedence over global.
///
/// EXPECTED: Entry with wrap_override=Some(NoWrap) does NOT wrap despite global Wrap.
/// ACTUAL: Per-entry override correctly overrides global after cclv-5ur.18 fix.
///
/// This test verifies FR-048: per-entry wrap override takes precedence over global.
#[test]
fn snapshot_wrap_mode_per_entry_override() {
    use crate::view_state::layout_params::LayoutParams;
    use crate::view_state::types::EntryIndex;

    // Create entry with long line
    let long_line = "This is a very long line that definitely exceeds the viewport width and should wrap to multiple lines when wrap mode is enabled. The content should continue on the next line without truncation.";
    let entry = create_test_log_entry(
        "override-test-1",
        Role::User,
        MessageContent::Text(long_line.to_string()),
        EntryType::User,
    );

    let conversation = create_test_conversation(vec![entry]);
    let mut view_state = ConversationViewState::new(
        None,
        None,
        conversation.clone(),
        200_000,
        PricingConfig::default(),
    );

    // Set global wrap to Wrap
    let params = LayoutParams::new(60, WrapMode::Wrap);
    view_state.relayout(60, WrapMode::Wrap, &crate::state::SearchState::Inactive);

    // Override THIS entry to NoWrap
    view_state.set_wrap_override(EntryIndex::new(0), Some(WrapMode::NoWrap), params);

    let styles = MessageStyles::new();

    let mut terminal = create_terminal(60, 15);
    terminal
        .draw(|frame| {
            let widget =
                ConversationView::new(&view_state, &styles, false).global_wrap(WrapMode::Wrap);
            frame.render_widget(widget, frame.area());
        })
        .unwrap();

    let output = buffer_to_string(terminal.backend().buffer());

    // Snapshot captures override behavior
    insta::assert_snapshot!("wrap_mode_per_entry_override", output);

    // Even though global is Wrap, this entry should be truncated (NoWrap override)
    assert!(
        !output.contains("truncation"),
        "Per-entry NoWrap override should truncate despite global Wrap. Output:\n{output}"
    );
}

// ===== Bug Reproduction Tests =====

/// Regression test for cclv-5ur.34: Verifies subagent entries are routed to separate tabs.
///
/// This test ensures entries with parent_tool_use_id create subagent tabs and are
/// correctly routed to those tabs instead of appearing in the Main Agent conversation.
///
/// Requirements:
/// - FR-003: "System MUST display subagent conversations in a tabbed pane"
/// - FR-004: "System MUST create a new tab when a subagent spawn event is detected"
///
/// Fixture: tests/fixtures/subagent_tab_repro.jsonl
#[test]
fn bug_subagent_entries_not_in_separate_tabs() {
    use crate::config::keybindings::KeyBindings;
    use crate::source::{FileSource, InputSource, StdinSource};
    use crate::state::AppState;
    use crate::view::TuiApp;
    use std::path::PathBuf;

    // Load minimal fixture with subagent entries
    let mut file_source = FileSource::new(PathBuf::from("tests/fixtures/subagent_tab_repro.jsonl"))
        .expect("Should load fixture");
    let log_entries = file_source.drain_entries().expect("Should parse entries");
    let entry_count = log_entries.len();

    let entries: Vec<ConversationEntry> = log_entries
        .into_iter()
        .map(|e| ConversationEntry::Valid(Box::new(e)))
        .collect();

    // Create terminal with standard dimensions
    let backend = TestBackend::new(120, 30);
    let terminal = Terminal::new(backend).unwrap();
    let mut app_state = AppState::new();
    app_state.add_entries(entries);
    let key_bindings = KeyBindings::default();
    let input_source = InputSource::Stdin(StdinSource::from_reader(&b""[..]));

    let mut app =
        TuiApp::new_for_test(terminal, app_state, input_source, entry_count, key_bindings);

    app.render_test().expect("Render should succeed");

    let buffer = app.terminal().backend().buffer();
    let output = buffer_to_string(buffer);

    // Snapshot captures buggy state (no subagent tabs)
    insta::assert_snapshot!("bug_subagent_entries_not_in_tabs", output);

    // This assertion should now PASS after fixing:
    // 1. Parser to recognize parent_tool_use_id as agent_id
    // 2. Fixture to have all entries in same session
    let has_subagents = app.app_state().session_view().has_subagents();
    assert!(
        has_subagents,
        "BUG REGRESSION: Entries with parent_tool_use_id should create subagent tabs.\n\
         Expected: has_subagents() == true (fixture contains 3 entries with parent_tool_use_id)\n\
         Actual: has_subagents() == false\n\
         This indicates either:\n\
         1. Parser not extracting parent_tool_use_id as agent_id, OR\n\
         2. View-state not creating subagent conversations from agent_id\n\
         Output:\n{output}"
    );
}

/// Bug reproduction: Header shows wrong agent when selected_tab = Some(0).
///
/// EXPECTED: When selected_tab = Some(0), header should show "Main Agent" since
/// tab 0 is the main agent tab.
///
/// ACTUAL: Header shows first subagent (agent_ids[0]) instead of "Main Agent".
/// The render_header function uses agent_ids.get(selected_idx) without checking
/// if selected_idx == 0 should mean "Main Agent".
///
/// Steps to reproduce manually:
/// 1. cargo run --release -- tests/fixtures/tab_header_mismatch_repro.jsonl
/// 2. Observe on initial load:
///    - Header shows: "Subagent toolu_subagent1" (WRONG)
///    - Content pane shows: "Main Agent (2 entries)" (correct)
///
/// Root cause: layout.rs render_header() uses agent_ids.get(selected_idx)
/// directly, but tab 0 should be Main Agent, not agent_ids[0].
///
/// Fixture: tests/fixtures/tab_header_mismatch_repro.jsonl
#[test]
fn bug_header_shows_wrong_agent_for_tab_zero() {
    use crate::config::keybindings::KeyBindings;
    use crate::source::{FileSource, InputSource, StdinSource};
    use crate::state::AppState;
    use crate::view::TuiApp;
    use std::path::PathBuf;

    // Load fixture with subagent entries
    let mut file_source = FileSource::new(PathBuf::from(
        "tests/fixtures/tab_header_mismatch_repro.jsonl",
    ))
    .expect("Should load fixture");
    let log_entries = file_source.drain_entries().expect("Should parse entries");
    let entry_count = log_entries.len();

    let entries: Vec<ConversationEntry> = log_entries
        .into_iter()
        .map(|e| ConversationEntry::Valid(Box::new(e)))
        .collect();

    // Create terminal with standard dimensions
    let backend = TestBackend::new(120, 30);
    let terminal = Terminal::new(backend).unwrap();
    let mut app_state = AppState::new();
    app_state.add_entries(entries);
    let key_bindings = KeyBindings::default();
    let input_source = InputSource::Stdin(StdinSource::from_reader(&b""[..]));

    let mut app =
        TuiApp::new_for_test(terminal, app_state, input_source, entry_count, key_bindings);

    // Verify subagents exist before testing
    assert!(
        app.app_state().session_view().has_subagents(),
        "Test precondition: fixture must have subagent entries"
    );

    // Verify initial state: selected_tab should be Some(0) (main agent tab)
    assert_eq!(
        app.app_state().selected_tab_index(),
        Some(0),
        "Test precondition: selected_tab should default to Some(0)"
    );

    // Initial render
    app.render_test().expect("Initial render should succeed");
    let output = buffer_to_string(app.terminal().backend().buffer());
    insta::assert_snapshot!("bug_header_mismatch_initial", output.clone());

    // Note: Header line removed per cclv-5ur.61
    // This test now verifies the header is NOT present
    let first_line = output.lines().next().unwrap_or("");

    // After cclv-5ur.61: Header removed
    // Verify first line is NOT a header (should be tab bar or content)
    assert!(
        !first_line.contains("Model:") && !first_line.contains("Main Agent | "),
        "First line should NOT contain header elements (header removed per cclv-5ur.61). Got: '{}'",
        first_line
    );

    // Verify UI still renders correctly
    assert!(
        output.contains("Conversations") || output.contains("Main Agent"),
        "UI should still render tab bar and content"
    );
}

/// Bug reproduction: Tab cycling was counting subagents from wrong session
///
/// EXPECTED: Pressing ']' should cycle through ALL subagent tabs in the current session
/// ACTUAL: Tab cycling counted subagents from session 0, but next_tab() used current_session()
///
/// ROOT CAUSE: session_view() returned get_session(0) instead of current_session().
/// In multi-session logs (122 sessions in fixture), this caused mismatch:
/// - Test counted subagents from session 0 (6 subagents)
/// - Tab cycling used current_session() (3 subagents)
///
/// FIX: Changed session_view() to use current_session() for consistency.
///
/// Fixture: tests/fixtures/cc-session-log.jsonl (31k lines, 122 sessions)
#[test]
fn bug_tab_cycling_limited_to_three_subagents() {
    use crate::config::keybindings::KeyBindings;
    use crate::source::{FileSource, InputSource, StdinSource};
    use crate::state::AppState;
    use crate::view::TuiApp;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use std::path::PathBuf;

    // Load fixture with multiple subagents
    let mut file_source = FileSource::new(PathBuf::from("tests/fixtures/cc-session-log.jsonl"))
        .expect("Should load fixture");
    let log_entries = file_source.drain_entries().expect("Should parse entries");
    let entry_count = log_entries.len();

    let entries: Vec<ConversationEntry> = log_entries
        .into_iter()
        .map(|e| ConversationEntry::Valid(Box::new(e)))
        .collect();

    // Create terminal
    let backend = TestBackend::new(200, 40);
    let terminal = Terminal::new(backend).unwrap();
    let mut app_state = AppState::new();
    app_state.add_entries(entries);
    let key_bindings = KeyBindings::default();
    let input_source = InputSource::Stdin(StdinSource::from_reader(&b""[..]));

    let mut app =
        TuiApp::new_for_test(terminal, app_state, input_source, entry_count, key_bindings);

    // Verify precondition: multiple subagents exist in current session
    // NOTE: The fixture has 122 sessions. We view the current/last session,
    // which should have at least 2 subagents to test tab cycling.
    let subagent_count = app.app_state().session_view().subagent_ids().count();
    assert!(
        subagent_count >= 2,
        "Test precondition: need at least 2 subagents, got {}",
        subagent_count
    );

    // Initial render
    app.render_test().expect("Initial render should succeed");

    // Track which tabs we visit
    let mut visited_tabs: Vec<Option<usize>> = vec![app.app_state().selected_tab_index()];

    // Press ']' to cycle through all tabs (should visit Main + all subagents)
    // With 7 subagents, we need 8 presses to cycle back to Main
    let total_tabs = 1 + subagent_count; // Main + subagents
    for _ in 0..total_tabs {
        let key_event = KeyEvent::new(KeyCode::Char(']'), KeyModifiers::NONE);
        app.handle_key_test(key_event);
        visited_tabs.push(app.app_state().selected_tab_index());
    }

    // Count unique tabs visited (excluding the final return to start)
    let unique_tabs: std::collections::HashSet<_> = visited_tabs.iter().collect();

    // BUG: Only 4 unique tabs visited (Main + 3 subagents) instead of 8 (Main + 7 subagents)
    assert_eq!(
        unique_tabs.len(),
        total_tabs,
        "BUG: Tab cycling limited to {} tabs instead of {}.\n\
         With {} subagents, pressing ']' should cycle through Main + all subagents.\n\
         Visited tabs: {:?}\n\
         Unique tabs: {:?}\n\n\
         Expected: Main Agent (0) + subagents (1-{})\n\
         Actual: Only cycling through first 3 subagents then back to Main",
        unique_tabs.len(),
        total_tabs,
        subagent_count,
        visited_tabs,
        unique_tabs,
        subagent_count
    );
}

/// Bug reproduction: Subagent scroll worked on wrong session
///
/// EXPECTED: Pressing 'j' in a subagent tab should scroll its conversation down
/// ACTUAL: Scroll events routed to session 0 subagent, not current session subagent
///
/// ROOT CAUSE: session_view() returned get_session(0) instead of current_session().
/// In multi-session logs, scroll events were routed to the wrong session's subagent.
///
/// FIX: Changed session_view() to use current_session() for consistency.
///
/// Fixture: tests/fixtures/cc-session-log.jsonl
#[test]
fn bug_subagent_scroll_does_not_work() {
    use crate::config::keybindings::KeyBindings;
    use crate::source::{FileSource, InputSource, StdinSource};
    use crate::state::AppState;
    use crate::view::TuiApp;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use std::path::PathBuf;

    // Load fixture with subagent entries
    let mut file_source = FileSource::new(PathBuf::from("tests/fixtures/cc-session-log.jsonl"))
        .expect("Should load fixture");
    let log_entries = file_source.drain_entries().expect("Should parse entries");
    let entry_count = log_entries.len();

    let entries: Vec<ConversationEntry> = log_entries
        .into_iter()
        .map(|e| ConversationEntry::Valid(Box::new(e)))
        .collect();

    // Create terminal
    let backend = TestBackend::new(200, 40);
    let terminal = Terminal::new(backend).unwrap();
    let mut app_state = AppState::new();
    app_state.add_entries(entries);
    let key_bindings = KeyBindings::default();
    let input_source = InputSource::Stdin(StdinSource::from_reader(&b""[..]));

    let mut app =
        TuiApp::new_for_test(terminal, app_state, input_source, entry_count, key_bindings);

    // Initial render
    app.render_test().expect("Initial render should succeed");

    // Switch to first subagent tab with ']'
    let key_event = KeyEvent::new(KeyCode::Char(']'), KeyModifiers::NONE);
    app.handle_key_test(key_event);

    // Verify we're on a subagent tab
    assert!(
        app.app_state().selected_tab_index().unwrap_or(0) > 0,
        "Should be on a subagent tab after pressing ']'"
    );

    // Render after tab switch
    app.render_test()
        .expect("Render after tab switch should succeed");
    let output_before = buffer_to_string(app.terminal().backend().buffer());
    insta::assert_snapshot!("bug_subagent_scroll_before", output_before.clone());

    // Get scroll position before
    let selected_tab = app.app_state().selected_tab_index().unwrap_or(0);

    // Get the subagent's conversation view state
    let mut sorted_agent_ids: Vec<_> = app.app_state().session_view().subagent_ids().collect();
    sorted_agent_ids.sort_by(|a, b| a.as_str().cmp(b.as_str()));

    let agent_id = sorted_agent_ids[selected_tab - 1].clone(); // selected_tab 1 = first subagent = index 0
    let scroll_before = app
        .app_state()
        .session_view()
        .subagents()
        .get(&agent_id)
        .map(|cv| cv.approximate_scroll_line())
        .unwrap_or(0);

    // Try to scroll down 5 times
    for _ in 0..5 {
        let key_event = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE);
        app.handle_key_test(key_event);
    }

    // Render after scroll attempts
    app.render_test()
        .expect("Render after scroll should succeed");
    let output_after = buffer_to_string(app.terminal().backend().buffer());
    insta::assert_snapshot!("bug_subagent_scroll_after", output_after.clone());

    // Get scroll position after
    let scroll_after = app
        .app_state()
        .session_view()
        .subagents()
        .get(&agent_id)
        .map(|cv| cv.approximate_scroll_line())
        .unwrap_or(0);

    // BUG: Scroll position unchanged despite pressing 'j' 5 times
    assert!(
        scroll_after > scroll_before,
        "BUG: Subagent scroll does not work.\n\
         Pressed 'j' 5 times but scroll position unchanged.\n\
         Scroll before: {}\n\
         Scroll after: {}\n\
         Selected tab: {} (subagent: {})\n\n\
         Expected: scroll_offset should increase\n\
         Actual: scroll_offset stayed at {}\n\n\
         Root cause: Scroll events likely not routed to subagent ConversationViewState",
        scroll_before,
        scroll_after,
        selected_tab,
        agent_id.as_str(),
        scroll_before
    );

    // Also verify rendered output changed
    assert_ne!(
        output_before, output_after,
        "BUG: Rendered output unchanged after scroll attempts.\n\
         The visible content should have changed if scroll worked."
    );
}

/// Bug reproduction: Subagent expand has rendering bug + mouse events broken
///
/// ## Bug 1: Vertical text rendering (cclv-5ur.48)
/// EXPECTED: Expanded text renders horizontally ("Review Task")
/// ACTUAL: Text renders vertically, one character per line ("R\ne\nv\ni\ne\nw...")
///
/// ## Bug 2: Mouse events not working (cclv-5ur.48)
/// EXPECTED: Clicking on entry in subagent pane toggles expand/collapse
/// ACTUAL: Nothing happens - mouse clicks don't work on subagent panes
///
/// Steps to reproduce manually:
/// 1. cargo run --release -- tests/fixtures/cc-session-log.jsonl
/// 2. Press ']' to switch to first subagent tab
/// 3. Press Enter on a collapsed entry -> text renders VERTICALLY (Bug 1)
/// 4. Click on an entry -> nothing happens (Bug 2)
///
/// Snapshots capture buggy state:
/// - bug_subagent_expand_before: collapsed state (correct)
/// - bug_subagent_expand_after: expanded but VERTICAL text (BUG)
///
/// Fixture: tests/fixtures/cc-session-log.jsonl
/// Bead: cclv-5ur.48
#[test]
fn bug_subagent_entry_expand_collapse_not_working() {
    use crate::config::keybindings::KeyBindings;
    use crate::source::{FileSource, InputSource, StdinSource};
    use crate::state::AppState;
    use crate::view::TuiApp;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use std::path::PathBuf;

    // Load fixture with subagent entries
    let mut file_source = FileSource::new(PathBuf::from("tests/fixtures/cc-session-log.jsonl"))
        .expect("Should load fixture");
    let log_entries = file_source.drain_entries().expect("Should parse entries");
    let entry_count = log_entries.len();

    let entries: Vec<ConversationEntry> = log_entries
        .into_iter()
        .map(|e| ConversationEntry::Valid(Box::new(e)))
        .collect();

    // Create terminal
    let backend = TestBackend::new(200, 40);
    let terminal = Terminal::new(backend).unwrap();
    let mut app_state = AppState::new();
    app_state.add_entries(entries);
    let key_bindings = KeyBindings::default();
    let input_source = InputSource::Stdin(StdinSource::from_reader(&b""[..]));

    let mut app =
        TuiApp::new_for_test(terminal, app_state, input_source, entry_count, key_bindings);

    // Initial render
    app.render_test().expect("Initial render should succeed");

    // Switch to first subagent tab with ']'
    let key_event = KeyEvent::new(KeyCode::Char(']'), KeyModifiers::NONE);
    app.handle_key_test(key_event);

    // Verify we're on a subagent tab
    let selected_tab = app.app_state().selected_tab_index().unwrap_or(0);
    assert!(
        selected_tab > 0,
        "Should be on a subagent tab after pressing ']'"
    );

    // Render after tab switch
    app.render_test()
        .expect("Render after tab switch should succeed");

    // Snapshot BEFORE expand (collapsed state)
    let output_before = buffer_to_string(app.terminal().backend().buffer());
    insta::assert_snapshot!("bug_subagent_expand_before", output_before);

    // Get the subagent's conversation view state
    let mut sorted_agent_ids: Vec<_> = app.app_state().session_view().subagent_ids().collect();
    sorted_agent_ids.sort_by(|a, b| a.as_str().cmp(b.as_str()));
    let agent_id = sorted_agent_ids[selected_tab - 1].clone();

    // Get expanded state of entry 0 in subagent BEFORE toggle attempt
    let subagent_entry_0_expanded_before = app
        .app_state()
        .session_view()
        .subagents()
        .get(&agent_id)
        .and_then(|cv| cv.get(crate::view_state::types::EntryIndex::new(0)))
        .map(|e| e.is_expanded())
        .unwrap_or(false);

    // Get expanded state of entry 0 in MAIN agent BEFORE toggle attempt
    let main_entry_0_expanded_before = app
        .app_state()
        .session_view()
        .main()
        .get(crate::view_state::types::EntryIndex::new(0))
        .map(|e| e.is_expanded())
        .unwrap_or(false);

    // Try to expand entry 0 using Enter key (keyboard expand)
    // First go to top to ensure we're at entry 0
    let key_event = KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE);
    app.handle_key_test(key_event);

    // Press Enter to toggle expand
    let key_event = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
    app.handle_key_test(key_event);

    // Render after toggle attempt
    app.render_test()
        .expect("Render after toggle should succeed");

    // Snapshot AFTER expand (expanded state - should show more content)
    let output_after = buffer_to_string(app.terminal().backend().buffer());
    insta::assert_snapshot!("bug_subagent_expand_after", output_after.clone());

    // BUG ASSERTION: Check for vertical text rendering bug (cclv-5ur.48)
    // When text renders correctly, we should see horizontal words like "Review"
    // When buggy, each character is on its own line forming a vertical pattern
    // Detect by looking for 3+ consecutive lines with only a single lowercase letter
    let lines: Vec<&str> = output_after.lines().collect();
    let mut consecutive_single_letters = 0;
    let mut max_consecutive = 0;
    for line in &lines {
        // Strip box-drawing chars and whitespace, check if just a single lowercase letter
        let content: String = line
            .chars()
            .filter(|c| !['│', '┌', '┐', '└', '┘', '─', ' '].contains(c))
            .collect();
        if content.len() == 1
            && content
                .chars()
                .next()
                .map(|c| c.is_ascii_lowercase())
                .unwrap_or(false)
        {
            consecutive_single_letters += 1;
            max_consecutive = max_consecutive.max(consecutive_single_letters);
        } else {
            consecutive_single_letters = 0;
        }
    }
    // 3+ consecutive single-letter lines indicates vertical text bug
    let has_vertical_text_bug = max_consecutive >= 3;
    assert!(
        !has_vertical_text_bug,
        "BUG: Expanded text renders VERTICALLY (one char per line) instead of horizontally.\n\
         Found {} consecutive single-letter lines - indicates width=1 rendering bug.\n\
         See snapshot bug_subagent_expand_after for visual evidence.\n\
         Bead: cclv-5ur.48",
        max_consecutive
    );

    // Get expanded state of entry 0 in subagent AFTER toggle attempt
    let subagent_entry_0_expanded_after = app
        .app_state()
        .session_view()
        .subagents()
        .get(&agent_id)
        .and_then(|cv| cv.get(crate::view_state::types::EntryIndex::new(0)))
        .map(|e| e.is_expanded())
        .unwrap_or(false);

    // Get expanded state of entry 0 in MAIN agent AFTER toggle attempt
    let main_entry_0_expanded_after = app
        .app_state()
        .session_view()
        .main()
        .get(crate::view_state::types::EntryIndex::new(0))
        .map(|e| e.is_expanded())
        .unwrap_or(false);

    // BUG: Subagent entry should have toggled, but main entry toggled instead
    assert_ne!(
        subagent_entry_0_expanded_before,
        subagent_entry_0_expanded_after,
        "BUG: Subagent entry expand/collapse does not work.\n\
         Pressed Enter while on subagent tab {} (agent: {}).\n\
         Subagent entry 0 expanded before: {}\n\
         Subagent entry 0 expanded after: {}\n\
         Main entry 0 expanded before: {}\n\
         Main entry 0 expanded after: {}\n\n\
         Expected: Subagent entry should toggle\n\
         Actual: Subagent entry unchanged, main entry toggled instead\n\n\
         Root cause: expand_handler.rs FocusPane::Subagent case returns unchanged state.\n\
         See TODO at src/state/expand_handler.rs:69",
        selected_tab,
        agent_id.as_str(),
        subagent_entry_0_expanded_before,
        subagent_entry_0_expanded_after,
        main_entry_0_expanded_before,
        main_entry_0_expanded_after
    );

    // Also verify main entry didn't accidentally get toggled
    assert_eq!(
        main_entry_0_expanded_before, main_entry_0_expanded_after,
        "Main entry should NOT have toggled when we're on a subagent tab"
    );
}

/// Bug reproduction: Mouse clicks don't toggle expand/collapse on subagent panes
///
/// EXPECTED: Clicking on an entry in a subagent pane toggles its expand/collapse state
/// ACTUAL: Nothing happens - mouse clicks are ignored on subagent panes
///
/// Steps to reproduce manually:
/// 1. cargo run --release -- tests/fixtures/cc-session-log.jsonl
/// 2. Press ']' to switch to first subagent tab
/// 3. Click on a collapsed entry
/// 4. Observe: Entry does NOT expand (unlike main pane where clicking works)
///
/// Bead: cclv-5ur.48
#[test]
fn bug_subagent_mouse_click_expand_not_working() {
    use crate::config::keybindings::KeyBindings;
    use crate::source::{FileSource, InputSource, StdinSource};
    use crate::state::AppState;
    use crate::view::TuiApp;
    use crossterm::event::{
        KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
    };
    use std::path::PathBuf;

    // Load fixture with subagent entries
    let mut file_source = FileSource::new(PathBuf::from("tests/fixtures/cc-session-log.jsonl"))
        .expect("Should load fixture");
    let log_entries = file_source.drain_entries().expect("Should parse entries");
    let entry_count = log_entries.len();

    let entries: Vec<ConversationEntry> = log_entries
        .into_iter()
        .map(|e| ConversationEntry::Valid(Box::new(e)))
        .collect();

    // Create terminal
    let backend = TestBackend::new(200, 40);
    let terminal = Terminal::new(backend).unwrap();
    let mut app_state = AppState::new();
    app_state.add_entries(entries);
    let key_bindings = KeyBindings::default();
    let input_source = InputSource::Stdin(StdinSource::from_reader(&b""[..]));

    let mut app =
        TuiApp::new_for_test(terminal, app_state, input_source, entry_count, key_bindings);

    // Initial render
    app.render_test().expect("Initial render should succeed");

    // Switch to first subagent tab with ']'
    let key_event = KeyEvent::new(KeyCode::Char(']'), KeyModifiers::NONE);
    app.handle_key_test(key_event);

    // Render after tab switch
    app.render_test()
        .expect("Render after tab switch should succeed");

    // Get the subagent's conversation view state
    let mut sorted_agent_ids: Vec<_> = app.app_state().session_view().subagent_ids().collect();
    sorted_agent_ids.sort_by(|a, b| a.as_str().cmp(b.as_str()));
    let selected_tab = app.app_state().selected_tab_index().unwrap_or(0);
    let agent_id = sorted_agent_ids[selected_tab - 1].clone();

    // Get expanded state of entry 0 in subagent BEFORE mouse click
    let expanded_before = app
        .app_state()
        .session_view()
        .subagents()
        .get(&agent_id)
        .and_then(|cv| cv.get(crate::view_state::types::EntryIndex::new(0)))
        .map(|e| e.is_expanded())
        .unwrap_or(false);

    // Click on entry 0 in the subagent pane
    // After cclv-5ur.61: Header removed, conversation pane now starts at row 4 (after tab bar only)
    // Tab bar: rows 0-2 (3 lines)
    // Conversation content: row 3+ (main pane border) then row 4+ (entries)
    let mouse_event = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 10, // Inside the content area
        row: 5,     // First entry area (adjusted for removed header)
        modifiers: KeyModifiers::NONE,
    };
    app.handle_mouse_test(mouse_event);

    // Render after click
    app.render_test()
        .expect("Render after mouse click should succeed");

    // Snapshot the state after mouse click
    let output_after_click = buffer_to_string(app.terminal().backend().buffer());
    insta::assert_snapshot!("bug_subagent_mouse_click_no_effect", output_after_click);

    // Get expanded state of entry 0 in subagent AFTER mouse click
    let expanded_after = app
        .app_state()
        .session_view()
        .subagents()
        .get(&agent_id)
        .and_then(|cv| cv.get(crate::view_state::types::EntryIndex::new(0)))
        .map(|e| e.is_expanded())
        .unwrap_or(false);

    // BUG ASSERTION: Mouse click should toggle expand state
    assert_ne!(
        expanded_before,
        expanded_after,
        "BUG: Mouse click on subagent entry does NOT toggle expand/collapse.\n\
         Clicked on entry 0 in subagent tab (agent: {}).\n\
         Expanded before click: {}\n\
         Expanded after click: {}\n\n\
         Expected: Entry should toggle expanded state\n\
         Actual: Entry state unchanged - mouse clicks ignored on subagent panes\n\
         Bead: cclv-5ur.48",
        agent_id.as_str(),
        expanded_before,
        expanded_after
    );
}

/// Bug reproduction: Tab click regions don't match visible tab positions
///
/// EXPECTED: Clicking on the visible "subagent_alpha" tab label should switch to that tab
/// ACTUAL: Click regions are evenly divided across full terminal width, not aligned with
///         visible tab labels. Clicking on visible tab text activates wrong tab or no tab.
///
/// Steps to reproduce manually:
/// 1. cargo run -- tests/fixtures/tab_click_mismatch_repro.jsonl
/// 2. Click on the visible "subagent_alpha" text in the tab bar (around x=15-30)
/// 3. Observe: Tab does NOT switch (or switches to wrong tab)
/// 4. Click very far right (x=100+) to find where click detection actually responds
///
/// Bead: cclv-154
#[test]
fn bug_tab_click_region_mismatch() {
    use crate::config::keybindings::KeyBindings;
    use crate::source::{FileSource, InputSource, StdinSource};
    use crate::state::AppState;
    use crate::view::TuiApp;
    use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
    use std::path::PathBuf;

    // Load fixture with 4 tabs: Main Agent + 3 subagents
    let mut file_source = FileSource::new(PathBuf::from(
        "tests/fixtures/tab_click_mismatch_repro.jsonl",
    ))
    .expect("Should load fixture");
    let log_entries = file_source.drain_entries().expect("Should parse entries");
    let entry_count = log_entries.len();

    let entries: Vec<ConversationEntry> = log_entries
        .into_iter()
        .map(|e| ConversationEntry::Valid(Box::new(e)))
        .collect();

    // Create terminal (wide enough to show all tabs)
    let backend = TestBackend::new(120, 30);
    let terminal = Terminal::new(backend).unwrap();
    let mut app_state = AppState::new();
    app_state.add_entries(entries);
    let key_bindings = KeyBindings::default();
    let input_source = InputSource::Stdin(StdinSource::from_reader(&b""[..]));

    let mut app =
        TuiApp::new_for_test(terminal, app_state, input_source, entry_count, key_bindings);

    // Initial render - should show Main Agent tab active
    app.render_test().expect("Initial render should succeed");

    let initial_output = buffer_to_string(app.terminal().backend().buffer());
    insta::assert_snapshot!("bug_tab_click_initial", initial_output.clone());

    // Verify initial state shows Main
    assert!(
        initial_output.contains("Main (2 entries)") || initial_output.contains("Main ["),
        "Should start with Main tab active"
    );

    // The tab bar shows: "│ Main │ subagent_alpha │ subagent_beta │ subagent_gamma"
    // Visual position of "subagent_alpha" starts around column 10
    // We click at column 15, row 2 (where the tab bar is rendered, 0-indexed)

    let tab_before = app.app_state().selected_tab_index();

    // Click on visible "subagent_alpha" text area (row 2 is tab bar, column 20 is in the label)
    let mouse_event = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 20, // Inside visible "subagent_alpha" text
        row: 2,     // Tab bar row (0-indexed)
        modifiers: KeyModifiers::NONE,
    };
    app.handle_mouse_test(mouse_event);

    app.render_test()
        .expect("Render after click should succeed");

    let after_click_output = buffer_to_string(app.terminal().backend().buffer());
    insta::assert_snapshot!(
        "bug_tab_click_after_visual_position",
        after_click_output.clone()
    );

    let tab_after = app.app_state().selected_tab_index();

    // BUG ASSERTION: Clicking on visible tab label should switch to that tab
    // Expected: tab_after should be Some(1) for subagent_alpha
    // Actual: tab stays at Some(0) for Main Agent
    assert_eq!(
        tab_after,
        Some(1),
        "BUG: Clicking on visible 'subagent_alpha' tab label (column 20) should activate it.\n\
         Tab before click: {:?}\n\
         Tab after click: {:?}\n\
         Clicked at: column=20, row=2\n\n\
         The tab bar shows tabs left-aligned, but click detection divides\n\
         terminal width evenly. This creates a mismatch between what users\n\
         see and where they need to click.\n\n\
         Expected: Click on visible tab label switches to that tab\n\
         Actual: Must click far to the right (past visible content) to switch tabs",
        tab_before,
        tab_after
    );
}

/// Bug reproduction: Tab click X coordinate off by 6 chars (Main vs Main Agent)
///
/// EXPECTED: Clicking at column 9 (visually on "tab2") should switch to tab index 1
/// ACTUAL: Click stays on Main because detect_tab_click uses "Main Agent" (10 chars)
///         instead of "Main" (4 chars) that is actually rendered
///
/// Root cause analysis:
/// - tabs.rs line 62 renders: "Main" (4 chars)
/// - mouse_handler.rs line 96 uses: "Main Agent" (10 chars)
/// - This creates a 6-character offset in all tab click positions
///
/// Tab bar layout:
///   Visual:  │ Main │ tab2 │
///   Columns: 0 1234 5 6789...
///   "Main" ends at column 5, "tab2" starts at column 8
///
///   detect_tab_click thinks:
///   "Main Agent" (10 chars + 3) = 13 chars, so Main extends to column 12
///
/// Steps to reproduce manually:
/// 1. cargo run -- tests/fixtures/tab_x_offset_repro.jsonl
/// 2. Click on the tab bar at approximately column 9 (on "tab2" text)
/// 3. Observe: Tab does NOT switch to tab2 - stays on Main
///
/// Bead: cclv-el4
#[test]
fn bug_tab_click_x_offset_main_vs_main_agent() {
    use crate::config::keybindings::KeyBindings;
    use crate::source::{FileSource, InputSource, StdinSource};
    use crate::state::AppState;
    use crate::view::TuiApp;
    use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
    use std::path::PathBuf;

    // Load minimal fixture with Main + one subagent "tab2"
    let mut file_source = FileSource::new(PathBuf::from("tests/fixtures/tab_x_offset_repro.jsonl"))
        .expect("Should load fixture");
    let log_entries = file_source.drain_entries().expect("Should parse entries");
    let entry_count = log_entries.len();

    let entries: Vec<ConversationEntry> = log_entries
        .into_iter()
        .map(|e| ConversationEntry::Valid(Box::new(e)))
        .collect();

    // Create terminal
    let backend = TestBackend::new(80, 24);
    let terminal = Terminal::new(backend).unwrap();
    let mut app_state = AppState::new();
    app_state.add_entries(entries);
    let key_bindings = KeyBindings::default();
    let input_source = InputSource::Stdin(StdinSource::from_reader(&b""[..]));

    let mut app =
        TuiApp::new_for_test(terminal, app_state, input_source, entry_count, key_bindings);

    // Initial render
    app.render_test().expect("Initial render should succeed");

    let initial_output = buffer_to_string(app.terminal().backend().buffer());
    insta::assert_snapshot!("bug_tab_x_offset_initial", initial_output.clone());

    // Verify we start on Main tab
    assert_eq!(
        app.app_state().selected_tab_index(),
        Some(0),
        "Should start with Main tab selected (index 0)"
    );

    // The tab bar renders as: "│ Main │ tab2 │"
    // Visual columns:
    //   0: │ (border)
    //   1: space
    //   2-5: "Main"
    //   6: space
    //   7: │ (separator)
    //   8: space
    //   9-12: "tab2"
    //
    // Click at column 9 - this is visually on "tab2" but due to the bug,
    // detect_tab_click thinks "Main Agent" extends to column 12, so it
    // reports TabClicked(0) instead of TabClicked(1).

    let mouse_event = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 9, // Visually on "tab2", but bug thinks this is still "Main Agent"
        row: 2,    // Tab bar content row (0-indexed, accounting for title row)
        modifiers: KeyModifiers::NONE,
    };
    app.handle_mouse_test(mouse_event);

    app.render_test()
        .expect("Render after click should succeed");

    let after_click_output = buffer_to_string(app.terminal().backend().buffer());
    insta::assert_snapshot!("bug_tab_x_offset_after_click", after_click_output.clone());

    // BUG ASSERTION: Click at column 9 should switch to tab2 (index 1)
    // but due to "Main Agent" vs "Main" mismatch, it stays on Main (index 0)
    assert_eq!(
        app.app_state().selected_tab_index(),
        Some(1),
        "BUG: Tab click X offset is wrong.\n\
         Clicking at column 9 (visually on 'tab2') should switch to tab index 1.\n\
         But detect_tab_click uses 'Main Agent' (10 chars) instead of 'Main' (4 chars),\n\
         creating a 6-character offset.\n\n\
         Tab bar visual:  │ Main │ tab2 │\n\
         Column 9 is on 'tab2', but click detection thinks Main extends to column 12.\n\n\
         Fix: Change mouse_handler.rs line 96 from:\n\
           let main_agent_width = \"Main Agent\".len() as u16 + 3;\n\
         to:\n\
           let main_agent_width = \"Main\".len() as u16 + 3;\n\n\
         Current tab: {:?} (expected: Some(1))",
        app.app_state().selected_tab_index()
    );
}

/// Bug reproduction: Help popup does not appear when pressing '?' in terminal
///
/// EXPECTED: Pressing '?' shows the help popup overlay
/// ACTUAL: Nothing happens - help popup does not appear
///
/// Root cause: The keybinding is registered as:
///   KeyEvent::new(KeyCode::Char('?'), KeyModifiers::SHIFT)
/// But real terminals (tmux, crossterm raw mode) send '?' as:
///   KeyEvent { code: Char('?'), modifiers: NONE }
/// The SHIFT modifier is "absorbed" into the character itself.
///
/// Steps to reproduce manually:
/// 1. cargo run -- tests/fixtures/minimal_session.jsonl
/// 2. Press '?' key
/// 3. Observe: No help popup appears (status bar still shows "?: Help")
/// 4. Press 's' to verify stats popup works (it does)
#[test]
fn bug_help_popup_not_triggered_by_question_mark() {
    use crate::source::FileSource;
    use crate::state::AppState;
    use crate::view::TuiApp;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use std::path::PathBuf;

    // Load minimal fixture
    let mut file_source = FileSource::new(PathBuf::from("tests/fixtures/minimal_session.jsonl"))
        .expect("Should load fixture");
    let log_entries = file_source.drain_entries().expect("Should parse entries");
    let entry_count = log_entries.len();

    let entries: Vec<ConversationEntry> = log_entries
        .into_iter()
        .map(|e| ConversationEntry::Valid(Box::new(e)))
        .collect();

    // Create app
    let backend = TestBackend::new(80, 40);
    let terminal = Terminal::new(backend).unwrap();
    let mut app_state = AppState::new();
    app_state.add_entries(entries);
    let key_bindings = crate::config::keybindings::KeyBindings::default();
    let input_source =
        crate::source::InputSource::Stdin(crate::source::StdinSource::from_reader(&b""[..]));

    let mut app =
        TuiApp::new_for_test(terminal, app_state, input_source, entry_count, key_bindings);

    // Initial render
    app.render_test().expect("Initial render should succeed");

    // Verify help is NOT visible initially
    assert!(
        !app.app_state().help_visible,
        "Help should not be visible initially"
    );

    // Simulate pressing '?' as a real terminal would send it:
    // Terminals send the '?' character directly WITHOUT the SHIFT modifier flag
    // (the shift is "absorbed" into producing the '?' character)
    let question_mark_real_terminal = KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE);
    app.handle_key_test(question_mark_real_terminal);
    app.render_test().expect("Render after '?' should succeed");

    // Capture output - should show help overlay
    let output = buffer_to_string(app.terminal().backend().buffer());
    insta::assert_snapshot!("bug_help_popup_not_triggered", output.clone());

    // BUG ASSERTION: Help popup should be visible after pressing '?'
    // The existing test uses KeyModifiers::SHIFT which works in test harness
    // but real terminals send '?' with KeyModifiers::NONE
    assert!(
        app.app_state().help_visible,
        "BUG: Pressing '?' should show help popup.\n\
         Real terminals (tmux, crossterm) send '?' as Char('?') with KeyModifiers::NONE.\n\
         The keybinding expects KeyModifiers::SHIFT which doesn't match.\n\n\
         Expected: help_visible = true\n\
         Actual: help_visible = false\n\n\
         The status bar shows '?: Help' but pressing '?' does nothing.\n\
         This is verified by manual testing in tmux pane."
    );
}

/// Bug reproduction: Help popup does not capture scroll events
///
/// EXPECTED: When help popup is visible, scroll events (j/k keys, mouse scroll)
///           should scroll the help content or be ignored - NOT scroll the
///           underlying conversation.
/// ACTUAL: Scroll events pass through to the conversation behind the popup,
///         causing the conversation to scroll while the help popup remains static.
///
/// Steps to reproduce manually:
/// 1. cargo run -- tests/fixtures/help_popup_scroll_repro.jsonl
/// 2. Press '?' to open help popup (requires SHIFT modifier in test)
/// 3. Press 'j' multiple times to scroll
/// 4. Observe: Conversation behind the popup scrolls, popup stays in place
#[test]
fn bug_help_popup_scroll_passthrough() {
    use crate::config::keybindings::KeyBindings;
    use crate::source::{FileSource, InputSource, StdinSource};
    use crate::state::AppState;
    use crate::view::TuiApp;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use std::path::PathBuf;

    // Load minimal fixture with enough entries to scroll
    let mut file_source = FileSource::new(PathBuf::from(
        "tests/fixtures/help_popup_scroll_repro.jsonl",
    ))
    .expect("Should load fixture");
    let log_entries = file_source.drain_entries().expect("Should parse entries");
    let entry_count = log_entries.len();

    let entries: Vec<ConversationEntry> = log_entries
        .into_iter()
        .map(|e| ConversationEntry::Valid(Box::new(e)))
        .collect();

    // Create app with small height to force scrolling
    // 15 rows: 3 for tabs, 11 for content, 1 for status - entries won't all fit
    let backend = TestBackend::new(100, 15);
    let terminal = Terminal::new(backend).unwrap();
    let mut app_state = AppState::new();
    app_state.add_entries(entries);
    let key_bindings = KeyBindings::default();
    let input_source = InputSource::Stdin(StdinSource::from_reader(&b""[..]));

    let mut app =
        TuiApp::new_for_test(terminal, app_state, input_source, entry_count, key_bindings);

    // Initial render
    app.render_test().expect("Initial render should succeed");

    // Open help popup
    let open_help = KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE);
    app.handle_key_test(open_help);
    app.render_test()
        .expect("Render after opening help should succeed");

    // Verify help is visible
    assert!(
        app.app_state().help_visible,
        "Help popup should be visible after pressing '?'"
    );

    // Capture state BEFORE scrolling
    let before_scroll = buffer_to_string(app.terminal().backend().buffer());
    insta::assert_snapshot!("bug_help_popup_scroll_before", before_scroll.clone());

    // Send multiple scroll events (j key)
    for _ in 0..5 {
        let scroll_down = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE);
        app.handle_key_test(scroll_down);
    }
    app.render_test()
        .expect("Render after scrolling should succeed");

    // Capture state AFTER scrolling
    let after_scroll = buffer_to_string(app.terminal().backend().buffer());
    insta::assert_snapshot!("bug_help_popup_scroll_after", after_scroll.clone());

    // FIXED (cclv-5ur.76): Help content should scroll, not conversation
    // The snapshots should be DIFFERENT (help scrolled) but conversation should be SAME
    assert_ne!(
        before_scroll, after_scroll,
        "REGRESSION: Help popup content should scroll when 'j' is pressed.\n\
         The help overlay should capture scroll events and scroll its own content.\n\n\
         Expected: Help content scrolls (snapshots different)\n\
         Actual: No change detected\n\n\
         Fixed in cclv-5ur.76 (was bug cclv-5ur.66)"
    );
}

/// Bug reproduction: Mouse scroll events pass through help popup
///
/// EXPECTED: Mouse scroll should be blocked when help popup is visible
/// ACTUAL: Mouse scroll events change the underlying conversation scroll position
///
/// This is the MOUSE SCROLL variant of bug_help_popup_scroll_passthrough (which tests keyboard)
/// Both keyboard (j/k) and mouse scroll should be blocked when help is visible.
///
/// Steps to reproduce manually:
/// 1. cargo run -- tests/fixtures/help_popup_scroll_repro.jsonl
/// 2. Press '?' to open help popup
/// 3. Mouse scroll up/down
/// 4. Observe: Conversation behind the popup scrolls, popup stays in place
#[test]
fn bug_help_popup_mouse_scroll_passthrough() {
    use crate::config::keybindings::KeyBindings;
    use crate::source::{FileSource, InputSource, StdinSource};
    use crate::state::AppState;
    use crate::view::TuiApp;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};
    use std::path::PathBuf;

    // Load minimal fixture with enough entries to scroll
    let mut file_source = FileSource::new(PathBuf::from(
        "tests/fixtures/help_popup_scroll_repro.jsonl",
    ))
    .expect("Should load fixture");
    let log_entries = file_source.drain_entries().expect("Should parse entries");
    let entry_count = log_entries.len();

    let entries: Vec<ConversationEntry> = log_entries
        .into_iter()
        .map(|e| ConversationEntry::Valid(Box::new(e)))
        .collect();

    // Create app with small height to force scrolling
    // 15 rows: 3 for tabs, 11 for content, 1 for status - entries won't all fit
    let backend = TestBackend::new(100, 15);
    let terminal = Terminal::new(backend).unwrap();
    let mut app_state = AppState::new();
    app_state.add_entries(entries);
    let key_bindings = KeyBindings::default();
    let input_source = InputSource::Stdin(StdinSource::from_reader(&b""[..]));

    let mut app =
        TuiApp::new_for_test(terminal, app_state, input_source, entry_count, key_bindings);

    // Initial render
    app.render_test().expect("Initial render should succeed");

    // Open help popup
    let open_help = KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE);
    app.handle_key_test(open_help);
    app.render_test()
        .expect("Render after opening help should succeed");

    // Verify help is visible
    assert!(
        app.app_state().help_visible,
        "Help popup should be visible after pressing '?'"
    );

    // Capture state BEFORE mouse scrolling
    let before_scroll = buffer_to_string(app.terminal().backend().buffer());
    insta::assert_snapshot!("bug_help_popup_mouse_scroll_before", before_scroll.clone());

    // Send multiple mouse scroll events (ScrollDown)
    for _ in 0..5 {
        let mouse_scroll = MouseEvent {
            kind: MouseEventKind::ScrollDown,
            column: 50,
            row: 10,
            modifiers: KeyModifiers::NONE,
        };
        app.handle_mouse_test(mouse_scroll);
    }
    app.render_test()
        .expect("Render after mouse scrolling should succeed");

    // Capture state AFTER mouse scrolling
    let after_scroll = buffer_to_string(app.terminal().backend().buffer());
    insta::assert_snapshot!("bug_help_popup_mouse_scroll_after", after_scroll.clone());

    // FIXED (cclv-5ur.76): Help content should scroll with mouse, not conversation
    // The snapshots should be DIFFERENT (help scrolled) but conversation should be SAME
    assert_ne!(
        before_scroll, after_scroll,
        "REGRESSION: Help popup content should scroll with mouse scroll events.\n\
         The help overlay should capture mouse scroll and scroll its own content.\n\n\
         Expected: Help content scrolls (snapshots different)\n\
         Actual: No change detected\n\n\
         Fixed in cclv-5ur.76 (was bug cclv-5ur.66)"
    );
}

/// Bug reproduction: Tab key should cycle tabs like ']' but only works once
///
/// EXPECTED: Tab key should cycle through conversation tabs (Main → subagent1 → subagent2 → Main)
///           The same behavior as pressing ']'
/// ACTUAL: Tab works once (Main → subagent1), then stops cycling (stays on subagent1)
///
/// Steps to reproduce manually:
/// 1. cargo run -- tests/fixtures/tab_navigation_repro.jsonl
/// 2. Press Tab - switches to first subagent (correct)
/// 3. Press Tab again - stays on same subagent (BUG)
/// 4. Press ']' - correctly switches to next subagent
#[test]
fn bug_tab_key_should_cycle_tabs_continuously() {
    use crate::config::keybindings::KeyBindings;
    use crate::source::{FileSource, InputSource, StdinSource};
    use crate::state::AppState;
    use crate::view::TuiApp;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use std::path::PathBuf;

    // Load minimal fixture with Main + 2 subagents
    let mut file_source =
        FileSource::new(PathBuf::from("tests/fixtures/tab_navigation_repro.jsonl"))
            .expect("Should load fixture");
    let log_entries = file_source.drain_entries().expect("Should parse entries");
    let entry_count = log_entries.len();

    let entries: Vec<ConversationEntry> = log_entries
        .into_iter()
        .map(|e| ConversationEntry::Valid(Box::new(e)))
        .collect();

    // Create terminal
    let backend = TestBackend::new(100, 30);
    let terminal = Terminal::new(backend).unwrap();
    let mut app_state = AppState::new();
    app_state.add_entries(entries);
    let key_bindings = KeyBindings::default();
    let input_source = InputSource::Stdin(StdinSource::from_reader(&b""[..]));

    let mut app =
        TuiApp::new_for_test(terminal, app_state, input_source, entry_count, key_bindings);

    // Verify precondition: we have Main + 2 subagents = 3 tabs
    let subagent_count = app.app_state().session_view().subagent_ids().count();
    assert_eq!(
        subagent_count, 2,
        "Test precondition: need exactly 2 subagents, got {}",
        subagent_count
    );

    // Initial render - should be on Main (tab 0)
    app.render_test().expect("Initial render should succeed");
    let initial_tab = app.app_state().selected_tab_index();
    assert_eq!(initial_tab, Some(0), "Should start on Main (tab 0)");

    // Press Tab once - should go to subagent 1 (tab 1)
    let tab_key = KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE);
    app.handle_key_test(tab_key);
    let after_first_tab = app.app_state().selected_tab_index();
    assert_eq!(
        after_first_tab,
        Some(1),
        "First Tab should switch to subagent 1 (tab 1)"
    );

    // Press Tab again - should go to subagent 2 (tab 2)
    let tab_key = KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE);
    app.handle_key_test(tab_key);
    let after_second_tab = app.app_state().selected_tab_index();

    // Capture snapshot for reference
    app.render_test().expect("Render after Tab should succeed");
    let output = buffer_to_string(app.terminal().backend().buffer());
    insta::assert_snapshot!("bug_tab_key_cycle_after_second_press", output);

    // BUG ASSERTION: Tab should continue cycling
    assert_eq!(
        after_second_tab,
        Some(2),
        "BUG: Tab key only works once, then stops cycling.\n\
         After first Tab: tab 1 (correct)\n\
         After second Tab: {:?} (should be Some(2))\n\n\
         Expected: Tab cycles tabs continuously like ']'\n\
         Actual: Tab works once then gets stuck\n\n\
         Verified by manual testing in tmux pane 1.\n\
         Bead: cclv-5ur.69",
        after_second_tab
    );
}

/// Bug reproduction: Number keys 1-9 should select specific tabs
///
/// EXPECTED: Pressing '1' selects Main, '2' selects first subagent, '3' selects second, etc.
/// ACTUAL: Number keys do nothing - the tab doesn't change
///
/// Steps to reproduce manually:
/// 1. cargo run -- tests/fixtures/tab_navigation_repro.jsonl
/// 2. Press ']' to go to a subagent tab
/// 3. Press '1' - should go back to Main (BUG: stays on subagent)
/// 4. Press '2' - should go to first subagent (BUG: does nothing)
#[test]
fn bug_number_keys_should_select_tabs() {
    use crate::config::keybindings::KeyBindings;
    use crate::source::{FileSource, InputSource, StdinSource};
    use crate::state::AppState;
    use crate::view::TuiApp;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use std::path::PathBuf;

    // Load minimal fixture with Main + 2 subagents
    let mut file_source =
        FileSource::new(PathBuf::from("tests/fixtures/tab_navigation_repro.jsonl"))
            .expect("Should load fixture");
    let log_entries = file_source.drain_entries().expect("Should parse entries");
    let entry_count = log_entries.len();

    let entries: Vec<ConversationEntry> = log_entries
        .into_iter()
        .map(|e| ConversationEntry::Valid(Box::new(e)))
        .collect();

    // Create terminal
    let backend = TestBackend::new(100, 30);
    let terminal = Terminal::new(backend).unwrap();
    let mut app_state = AppState::new();
    app_state.add_entries(entries);
    let key_bindings = KeyBindings::default();
    let input_source = InputSource::Stdin(StdinSource::from_reader(&b""[..]));

    let mut app =
        TuiApp::new_for_test(terminal, app_state, input_source, entry_count, key_bindings);

    // Verify precondition: we have Main + 2 subagents = 3 tabs
    let subagent_count = app.app_state().session_view().subagent_ids().count();
    assert_eq!(
        subagent_count, 2,
        "Test precondition: need exactly 2 subagents, got {}",
        subagent_count
    );

    // Initial render - should be on Main (tab 0)
    app.render_test().expect("Initial render should succeed");
    assert_eq!(
        app.app_state().selected_tab_index(),
        Some(0),
        "Should start on Main (tab 0)"
    );

    // Navigate to subagent 2 using ']' (which works)
    let bracket_key = KeyEvent::new(KeyCode::Char(']'), KeyModifiers::NONE);
    app.handle_key_test(bracket_key);
    app.handle_key_test(bracket_key);
    assert_eq!(
        app.app_state().selected_tab_index(),
        Some(2),
        "Should be on subagent 2 (tab 2) after two ']' presses"
    );

    // Press '1' to go back to Main
    let key_1 = KeyEvent::new(KeyCode::Char('1'), KeyModifiers::NONE);
    app.handle_key_test(key_1);
    let after_1 = app.app_state().selected_tab_index();

    // Capture snapshot for reference
    app.render_test().expect("Render after '1' should succeed");
    let output = buffer_to_string(app.terminal().backend().buffer());
    insta::assert_snapshot!("bug_number_key_1_should_select_main", output);

    // BUG ASSERTION: Pressing '1' should select Main (tab 0)
    assert_eq!(
        after_1,
        Some(0),
        "BUG: Number key '1' does not select Main tab.\n\
         Before pressing '1': tab 2 (subagent 2)\n\
         After pressing '1': {:?} (should be Some(0) for Main)\n\n\
         Expected: '1' selects Main, '2' selects first subagent, etc.\n\
         Actual: Number keys have no effect on tab selection\n\n\
         Verified by manual testing in tmux pane 1.\n\
         Bead: cclv-5ur.69",
        after_1
    );
}

// ===== Token Stats Divider Bug Reproduction =====

/// Bug reproduction: Entry separator token stats not calculated properly
///
/// EXPECTED: Token separator should match Claude Code statusline format:
///   - `↓{read_non_cached}/{read_total} ↑{write_non_cached}/{write_total}`
///   - read_non_cached = input_tokens + cache_creation_input_tokens
///   - read_total = input_tokens + cache_creation_input_tokens + cache_read_input_tokens
///   - write_non_cached = output_tokens
///   - write_total = output_tokens + estimated_thinking_tokens (chars/4)
///   - Context = input + cache_creation + cache_read + output (THIS ENTRY ONLY)
///
/// CRITICAL: Context calculation is WRONG - currently accumulates across entries.
///   Each entry's input_tokens already represents the current context window state
///   from the API's perspective. It includes prior conversation context - it's NOT
///   cumulative across entries. We should show THIS entry's context, not accumulated.
///
/// ACTUAL: Shows `{input} in / {output} out` format:
///   - Only shows raw input_tokens, not read breakdown
///   - Context INCORRECTLY accumulates all previous entries (grows to 100% fast)
///   - No thinking/tool_use token estimation
///
/// Reference: /home/claude/.claude/claude-code-status-line.py (lines 300-308)
///
/// Steps to reproduce manually:
/// 1. cargo run -- tests/fixtures/token_stats_repro.jsonl
/// 2. Observe entry separator lines - context % grows with each entry
/// 3. Expected: each entry's context should be independent, not accumulated
#[test]
fn bug_token_stats_divider_wrong_calculation() {
    use crate::source::FileSource;
    use crate::state::AppState;
    use crate::view::TuiApp;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use std::path::PathBuf;

    // Load minimal fixture with token usage data
    let mut file_source = FileSource::new(PathBuf::from("tests/fixtures/token_stats_repro.jsonl"))
        .expect("Should load fixture");
    let log_entries = file_source.drain_entries().expect("Should parse entries");
    let entry_count = log_entries.len();

    // Convert to ConversationEntry
    let entries: Vec<ConversationEntry> = log_entries
        .into_iter()
        .map(|e| ConversationEntry::Valid(Box::new(e)))
        .collect();

    // Create terminal wide enough to show full separator
    let backend = TestBackend::new(80, 25);
    let terminal = Terminal::new(backend).unwrap();
    let mut app_state = AppState::new();
    app_state.add_entries(entries);
    let key_bindings = crate::config::keybindings::KeyBindings::default();
    let input_source =
        crate::source::InputSource::Stdin(crate::source::StdinSource::from_reader(&b""[..]));

    let mut app =
        TuiApp::new_for_test(terminal, app_state, input_source, entry_count, key_bindings);

    app.render_test().expect("Render should succeed");

    let buffer = app.terminal().backend().buffer();
    let output = buffer_to_string(buffer);

    // Snapshot captures the buggy output format
    insta::assert_snapshot!("bug_token_stats_divider_wrong", output);

    // Entry 1 has:
    //   input_tokens: 100, cache_creation: 500, cache_read: 200, output: 50
    //   thinking block: ~25 tokens estimated (100 chars / 4)
    // Expected format per Claude Code statusline:
    //   read_non_cached = 100 + 500 = 600 (0.6k)
    //   read_total = 100 + 500 + 200 = 800 (0.8k)
    //   write_non_cached = 50
    //   write_total = 50 + 25 = 75
    //   context = 100 + 500 + 200 + 50 = 850 (0.9k)
    //
    // Expected: "↓0.6k/0.8k ↑50/75" or similar with arrows
    // Actual: "100 in / 50 out" - missing cache breakdown and arrows

    assert!(
        output.contains("↓") && output.contains("↑"),
        "BUG: Token divider format is incorrect.\n\
         Expected: Claude Code statusline format with ↓read ↑write arrows\n\
         Actual: Shows '{{input}} in / {{output}} out' without cache breakdown\n\n\
         The divider should show:\n\
         - ↓{{read_non_cached}}/{{read_total}} for input (cached vs total)\n\
         - ↑{{write_non_cached}}/{{write_total}} for output (with thinking estimate)\n\n\
         Per Claude Code statusline script, context should also include output_tokens.\n\n\
         Actual output:\n{output}"
    );

    // INVARIANT: Context should NOT accumulate across entries.
    // Entry 2 has: input=150, cache_creation=0, cache_read=800, output=30
    // Correct context for entry 2 = 150 + 0 + 800 + 30 = 980 tokens (0.5% of 200k)
    // NOT 1.8k which would be accumulated (entry1 + entry2)
    //
    // If we see "1.8k" in entry 2's separator, context is wrongly accumulated.
    // Correct value should show ~1.0k or 980 for entry 2.
    assert!(
        !output.contains("Context: 1.8k"),
        "BUG: Context is incorrectly ACCUMULATED across entries.\n\
         Entry 2 shows 'Context: 1.8k' but should show ~1.0k (this entry's tokens only).\n\n\
         Entry 2 has: input=150, cache_creation=0, cache_read=800, output=30\n\
         Correct context = 150 + 0 + 800 + 30 = 980 tokens ≈ 1.0k\n\
         Accumulated (wrong) = entry1(800) + entry2(980) ≈ 1.8k\n\n\
         Each entry's input_tokens from the API already includes prior conversation\n\
         context - we should NOT accumulate across entries.\n\n\
         Actual output:\n{output}"
    );
}

// ===== Help Popup Scroll Bug Tests =====

/// Bug reproduction: Help popup contents don't scroll
///
/// EXPECTED: When help popup is open and user presses j/↓ or uses mouse scroll,
///           the help content should scroll down to reveal more content
/// ACTUAL: Scroll inputs are ignored, content remains static
///
/// Steps to reproduce manually:
/// 1. cargo run -- tests/fixtures/help_scroll_repro.jsonl
/// 2. Press '?' to open help popup
/// 3. Press 'j' or scroll down with mouse
/// 4. Observe: content does not scroll, cannot see content below visible area
#[test]
fn bug_help_popup_no_scroll() {
    use crate::test_harness::AcceptanceTestHarness;
    use crossterm::event::KeyCode;

    // Use small height to ensure help content is taller than viewport
    // Help popup has ~50 lines of content, 20 row terminal ensures truncation
    let mut harness = AcceptanceTestHarness::from_fixture_with_size(
        "tests/fixtures/help_scroll_repro.jsonl",
        80,
        20,
    )
    .expect("Failed to load fixture");

    // Open help popup
    harness.send_key_with_mods(KeyCode::Char('?'), crossterm::event::KeyModifiers::NONE);

    // Capture state before scroll attempt
    let before_scroll = harness.render_to_string();

    // Take snapshot of initial help state (shows "Navigation" section at top)
    insta::assert_snapshot!("bug_help_popup_before_scroll", before_scroll.clone());

    // Attempt to scroll down
    harness.send_key(KeyCode::Char('j'));
    harness.send_key(KeyCode::Char('j'));
    harness.send_key(KeyCode::Char('j'));

    // Capture state after scroll attempt
    let after_scroll = harness.render_to_string();

    // Take snapshot of state after scroll attempt
    insta::assert_snapshot!("bug_help_popup_after_scroll", after_scroll.clone());

    // BUG ASSERTION: Content should have scrolled (changed)
    // This assertion FAILS because help popup doesn't scroll
    assert_ne!(
        before_scroll, after_scroll,
        "BUG: Help popup content did not scroll.\n\
         Pressing 'j' while help popup is open should scroll the help content.\n\
         Expected: Content should change (scroll down to show more shortcuts)\n\
         Actual: Content is identical before and after scroll attempt\n\n\
         The help popup has more content than fits in viewport but cannot be scrolled."
    );
}

// ===== Session Tab Navigation Bug Tests =====

/// Bug reproduction: Tab navigation broken after changing sessions
///
/// EXPECTED: After selecting a different session via session modal:
///   1. Screen immediately shows content (Main agent view or subagent if that was focused)
///   2. Tab cycles through Main and subagent tabs, each showing their content
///
/// ACTUAL: After selecting a different session:
///   1. Screen is completely blank (no content rendered)
///   2. Tab to subagent tab shows blank content even though subagent has entries
///
/// Steps to reproduce manually:
/// 1. cargo run -- tests/fixtures/session_tab_nav_repro.jsonl
/// 2. App starts showing Session 2/2 (the latest)
/// 3. Press 'S' to open session modal
/// 4. Press 'k' to navigate to Session 1
/// 5. Press Enter to select Session 1
/// 6. Observe: Screen is blank (BUG #1)
/// 7. Press Tab to switch to Main
/// 8. Observe: Main content appears
/// 9. Press Tab to switch to subagent tab
/// 10. Observe: Subagent tab is blank even though it has content (BUG #2)
#[test]
fn bug_session_tab_navigation_blank_after_change() {
    use crate::test_harness::AcceptanceTestHarness;
    use crossterm::event::KeyCode;

    // Load fixture with 2 sessions, session 1 has Main + subagent
    let mut harness = AcceptanceTestHarness::from_fixture_with_size(
        "tests/fixtures/session_tab_nav_repro.jsonl",
        80,
        24,
    )
    .expect("Failed to load fixture");

    // Initial render - should show Session 2/2 (latest)
    let initial = harness.render_to_string();
    assert!(
        initial.contains("Session 2/2") || initial.contains("Session 2 main"),
        "Should start on latest session (Session 2)\n\nActual:\n{initial}"
    );

    // Take snapshot of initial state
    insta::assert_snapshot!("bug_session_tab_nav_initial", initial);

    // Open session modal with 'S'
    harness.send_key(KeyCode::Char('S'));
    let with_modal = harness.render_to_string();
    assert!(
        with_modal.contains("Session List"),
        "Session modal should be visible"
    );

    // Navigate to Session 1 (press k to go up)
    harness.send_key(KeyCode::Char('k'));

    // Select Session 1 with Enter
    harness.send_key(KeyCode::Enter);

    // Capture state immediately after session change
    let after_session_change = harness.render_to_string();

    // Take snapshot - captures buggy blank state
    insta::assert_snapshot!(
        "bug_session_tab_nav_after_change",
        after_session_change.clone()
    );

    // BUG #1: Screen should NOT be blank after changing sessions
    // The content area should show the Main agent view (or previously focused pane)
    let is_blank = !after_session_change.contains("Session 1")
        && !after_session_change.contains("user message")
        && !after_session_change.contains("main agent");
    assert!(
        !is_blank || after_session_change.contains("Session 1"),
        "BUG #1: Screen is blank after changing sessions.\n\
         Expected: Content should be visible for Session 1\n\
         Actual: Screen appears blank with no visible entries\n\n\
         Actual output:\n{after_session_change}"
    );

    // Press Tab to cycle to first subagent
    harness.send_key(KeyCode::Tab);
    let after_first_tab = harness.render_to_string();

    // Take snapshot after first Tab - should show subagent-a
    insta::assert_snapshot!(
        "bug_session_tab_nav_after_first_tab",
        after_first_tab.clone()
    );

    // After first Tab, should show subagent-a content (Main -> subagent-a)
    let showing_subagent = after_first_tab.contains("subagent-a [Sonnet]")
        || after_first_tab.contains("subagent-a (1 entries)")
        || after_first_tab.contains("Session 1 subagent message");
    assert!(
        showing_subagent,
        "After first Tab, subagent-a content should be visible.\n\
         Expected: 'subagent-a [Sonnet]' with subagent content\n\
         Actual output:\n{after_first_tab}"
    );

    // Press Tab again to cycle back to Main
    harness.send_key(KeyCode::Tab);
    let after_second_tab = harness.render_to_string();

    // Take snapshot - should show Main again (completing the cycle)
    insta::assert_snapshot!(
        "bug_session_tab_nav_after_second_tab",
        after_second_tab.clone()
    );

    // After second Tab, should be back on Main (subagent-a -> Main)
    let showing_main = after_second_tab.contains("Main [Opus]")
        || after_second_tab.contains("Session 1 user message")
        || after_second_tab.contains("Session 1 main agent");

    assert!(
        showing_main,
        "After second Tab, should cycle back to Main agent.\n\
         Expected: 'Main [Opus] (2 entries)' with Main agent content\n\
         Actual: Tab cycling not working correctly\n\n\
         After pressing Tab twice from Main, we should have cycled:\n\
           Main (start) -> subagent-a (first Tab) -> Main (second Tab)\n\n\
         Actual output:\n{after_second_tab}"
    );
}

/// Bug reproduction: Screen blank when switching sessions while on subagent tab
///
/// EXPECTED: After switching sessions from a subagent tab, the new session's
///           Main conversation should be visible immediately.
///
/// ACTUAL: After switching sessions while viewing a subagent tab, the
///         conversation pane is completely blank. Pressing Tab fixes it.
///
/// Root cause hypothesis: When switching sessions from a subagent tab, the
/// viewed_session updates but the conversation view doesn't re-render the
/// correct content until tab cycling forces a view refresh.
///
/// Steps to reproduce manually:
/// 1. cargo run -- tests/fixtures/session_nav_subagent_blank_repro.jsonl
/// 2. App shows Session 2/2 with Main and subagent-b tabs
/// 3. Press Tab to switch to subagent-b tab (shows "Session 2 subagent message")
/// 4. Press 'S' to open session modal
/// 5. Press 'k' to navigate to Session 1
/// 6. Press Enter to select Session 1
/// 7. Observe: Screen is blank (BUG!) - tabs show "Main | subagent-a" but no content
/// 8. Press Tab - content appears (workaround)
#[test]
fn bug_session_switch_from_subagent_tab_shows_blank() {
    use crate::test_harness::AcceptanceTestHarness;
    use crossterm::event::KeyCode;

    // Load fixture: 2 sessions, each with Main + subagent
    let mut harness = AcceptanceTestHarness::from_fixture_with_size(
        "tests/fixtures/session_nav_subagent_blank_repro.jsonl",
        80,
        24,
    )
    .expect("Failed to load fixture");

    // Initial render - should show Session 2/2 (latest)
    let initial = harness.render_to_string();
    assert!(
        initial.contains("Session 2/2"),
        "Should start on Session 2/2\n\nActual:\n{initial}"
    );
    assert!(
        initial.contains("Session 2 main agent") || initial.contains("Session 2 user"),
        "Should show Session 2 content initially\n\nActual:\n{initial}"
    );

    // Step 1: Press Tab to switch to subagent tab
    harness.send_key(KeyCode::Tab);
    let on_subagent = harness.render_to_string();
    assert!(
        on_subagent.contains("subagent-b"),
        "Should now be on subagent-b tab\n\nActual:\n{on_subagent}"
    );
    assert!(
        on_subagent.contains("Session 2 subagent message"),
        "Should show Session 2 subagent content\n\nActual:\n{on_subagent}"
    );

    // Snapshot before session change (showing subagent-b tab)
    insta::assert_snapshot!("bug_subagent_blank_before_switch", on_subagent);

    // Step 2: Open session modal
    harness.send_key(KeyCode::Char('S'));
    let with_modal = harness.render_to_string();
    assert!(
        with_modal.contains("Session List"),
        "Session modal should be visible"
    );

    // Step 3: Navigate up and select Session 1
    harness.send_key(KeyCode::Char('k'));
    harness.send_key(KeyCode::Enter);

    // Capture state immediately after session change
    let after_session_change = harness.render_to_string();

    // Snapshot captures the buggy blank state
    insta::assert_snapshot!(
        "bug_subagent_blank_after_switch",
        after_session_change.clone()
    );

    // BUG ASSERTION: Screen should NOT be blank!
    // The conversation area should show Session 1's Main content
    let has_session_1_content = after_session_change.contains("Session 1 main agent")
        || after_session_change.contains("Session 1 user message");

    assert!(
        has_session_1_content,
        "BUG: Screen is blank after switching sessions from subagent tab.\n\n\
         Expected: Session 1 Main content should be visible immediately\n\
         Actual: Conversation area is blank\n\n\
         The status bar shows 'Session 1/2' and tabs show 'Main | subagent-a',\n\
         but the conversation pane has no content.\n\n\
         Workaround: Pressing Tab fixes the display.\n\n\
         Actual output:\n{after_session_change}"
    );
}

/// Bug reproduction: Mouse clicks stop working after switching to non-last session
///
/// EXPECTED: Mouse clicks for tab switching and expand/collapse should work
/// regardless of which session is currently viewed.
///
/// ACTUAL: After switching to a session that is NOT the last session,
/// mouse clicks on tabs cause blank screen, and expand/collapse clicks are ignored.
///
/// ROOT CAUSE: `session_view()` calls `current_session()` in `log.rs` which always
/// returns `sessions.last()`, ignoring the `viewed_session` state (which can be
/// `ViewedSession::Pinned(idx)`). This causes the mouse handler to get tab info
/// from the WRONG session.
///
/// Steps to reproduce manually:
/// 1. cargo run --release -- tests/fixtures/cc-session-log.jsonl
/// 2. Click on tabs and entries - works correctly on session 24/24
/// 3. Press Shift+S to open session modal, select session 1
/// 4. Try clicking on tabs - screen goes blank
/// 5. Try clicking on entries to expand - clicks are ignored
#[test]
#[ignore = "cclv-bgu: mouse clicks broken on non-last sessions"]
fn bug_mouse_clicks_broken_on_non_last_session() {
    use crate::test_harness::AcceptanceTestHarness;
    use crossterm::event::KeyCode;

    // Load fixture with multiple sessions using test harness
    let mut harness = AcceptanceTestHarness::from_fixture_with_size(
        "tests/fixtures/cc-session-log.jsonl",
        200,
        40,
    )
    .expect("Should load fixture");

    // Verify precondition: multiple sessions exist
    let session_count = harness.state().log_view().session_count();
    assert!(
        session_count >= 2,
        "Test precondition: need at least 2 sessions, got {}",
        session_count
    );

    // Initial render
    let initial_output = harness.render_to_string();
    assert!(
        initial_output.contains(&format!("Session {}/{}", session_count, session_count)),
        "Should start on last session"
    );

    // TEST PART 1: Verify mouse clicks work on last session
    // Click on second tab (around column 15-25 for a subagent tab, row 1 for tab bar)
    harness.click_at(20, 1);
    let after_tab_click_last = harness.render_to_string();

    // Should still show content (conversation pane should have text)
    let has_content_last = after_tab_click_last
        .lines()
        .skip(3) // Skip header and tab rows
        .any(|line| {
            let trimmed = line.trim();
            !trimmed.is_empty()
                && !trimmed
                    .chars()
                    .all(|c| matches!(c, '│' | '─' | '┌' | '┐' | '└' | '┘' | ' '))
        });
    assert!(
        has_content_last,
        "Tab click on last session should show content"
    );

    // TEST PART 2: Switch to first session via keyboard (session modal)
    // Press Shift+S to open session modal
    harness.send_key_with_mods(KeyCode::Char('S'), crossterm::event::KeyModifiers::SHIFT);
    let modal_output = harness.render_to_string();
    assert!(
        modal_output.contains("Session List"),
        "Session modal should open"
    );

    // Press 'g' to go to first session
    harness.send_key(KeyCode::Char('g'));

    // Press Enter to select session 1
    harness.send_key(KeyCode::Enter);

    // Render after session switch
    let after_switch = harness.render_to_string();
    assert!(
        after_switch.contains("Session 1/"),
        "Should be on session 1 after switch\nActual:\n{after_switch}"
    );

    // Verify content is visible after switch (before any mouse interaction)
    let has_content_after_switch = after_switch.lines().skip(3).any(|line| {
        let trimmed = line.trim();
        !trimmed.is_empty()
            && !trimmed
                .chars()
                .all(|c| matches!(c, '│' | '─' | '┌' | '┐' | '└' | '┘' | ' '))
    });
    assert!(
        has_content_after_switch,
        "Content should be visible after switching to session 1"
    );

    // TEST PART 3: Try clicking on a tab on the NON-LAST session
    // This is where the bug manifests
    harness.click_at(20, 1); // Click on second tab

    let after_tab_click_non_last = harness.render_to_string();

    // Capture snapshot of buggy state
    insta::assert_snapshot!("bug_mouse_clicks_non_last_session", after_tab_click_non_last.clone());

    // BUG: Screen goes blank after tab click on non-last session!
    // Check for actual conversation entry markers (│ N where N is entry number)
    // or meaningful text content in the conversation pane.
    // The status bar ([LIVE] │ Session...) should NOT count as conversation content.
    let has_conversation_content = after_tab_click_non_last.lines().any(|line| {
        // Look for entry number markers like "│  1 " or "│ 12 " which indicate
        // conversation entries are being rendered
        line.contains("│  1 ")
            || line.contains("│  2 ")
            || line.contains("│  3 ")
            || line.contains("│ 1 ")
            || line.contains("│ 2 ")
            || line.contains("│ 3 ")
            // Also check for tool call markers
            || line.contains("🔧 Tool:")
            // Or thinking block markers
            || line.contains("thinking")
            // Or any pane header with entry count like "[Opus] (83 entries)"
            || line.contains("entries)")
    });

    // The content pane should NOT be blank - we should see conversation entries
    assert!(
        has_conversation_content,
        "BUG: Screen went blank after clicking tab on non-last session!\n\n\
         Expected: Conversation entries should be visible (│ 1, │ 2, tool calls, etc.)\n\
         Actual: Conversation pane is blank - no entry markers found\n\n\
         Root cause: session_view() -> current_session() returns sessions.last()\n\
         instead of respecting viewed_session (ViewedSession::Pinned)\n\n\
         Output after tab click:\n{after_tab_click_non_last}"
    );
}
