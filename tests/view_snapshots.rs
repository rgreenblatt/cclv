//! Snapshot tests for key view components
//!
//! Uses insta + ratatui TestBackend to verify rendering output doesn't regress.
//! These tests capture the visual representation of widgets and protect against
//! accidental UI changes.

use cclv::model::{
    AgentId, ContentBlock, ConversationEntry, EntryMetadata, EntryType, EntryUuid, LogEntry,
    Message, MessageContent, PricingConfig, Role, SessionId, SessionStats, StatsFilter, TokenUsage,
    ToolCall, ToolName, ToolUseId,
};
use cclv::state::WrapMode;
use cclv::view::{tabs, ConversationView, MessageStyles, StatsPanel};
use cclv::view_state::conversation::ConversationViewState;
use cclv::view_state::layout_params::LayoutParams;
use cclv::view_state::types::{EntryIndex, ViewportDimensions};
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
    let mut view_state = ConversationViewState::new(None, None, conversation.clone());
    view_state.relayout(60, WrapMode::Wrap);
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
    let mut view_state = ConversationViewState::new(None, None, conversation.clone());
    let params = LayoutParams::new(80, WrapMode::Wrap);
    view_state
        .toggle_expand(
            EntryIndex::new(0),
            params,
            ViewportDimensions::new(80, 24),
        )
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
    let mut view_state = ConversationViewState::new(None, None, conversation.clone());
    let params = LayoutParams::new(80, WrapMode::Wrap);
    view_state
        .toggle_expand(
            EntryIndex::new(0),
            params,
            ViewportDimensions::new(80, 24),
        )
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
    let mut view_state = ConversationViewState::new(None, None, conversation.clone());
    let params = LayoutParams::new(80, WrapMode::Wrap);
    view_state
        .toggle_expand(
            EntryIndex::new(0),
            params,
            ViewportDimensions::new(80, 24),
        )
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
    let mut view_state = ConversationViewState::new(None, None, conversation.clone());
    let params = LayoutParams::new(80, WrapMode::Wrap);
    view_state
        .toggle_expand(
            EntryIndex::new(0),
            params,
            ViewportDimensions::new(80, 24),
        )
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
    let mut color_counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();

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
    let mut view_state = ConversationViewState::new(None, None, conversation.clone());
    let params = LayoutParams::new(80, WrapMode::Wrap);
    view_state
        .toggle_expand(
            EntryIndex::new(0),
            params,
            ViewportDimensions::new(80, 24),
        )
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
    let mut view_state = ConversationViewState::new(None, None, conversation.clone());
    let params = LayoutParams::new(80, WrapMode::Wrap);
    view_state
        .toggle_expand(
            EntryIndex::new(0),
            params,
            ViewportDimensions::new(80, 24),
        )
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
    let mut view_state = ConversationViewState::new(None, None, conversation.clone());
    let params = LayoutParams::new(80, WrapMode::Wrap);
    view_state
        .toggle_expand(
            EntryIndex::new(0),
            params,
            ViewportDimensions::new(80, 24),
        )
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
#[ignore = "Search highlighting not yet implemented with view-state"]
fn snapshot_message_with_search_highlighting() {
    // TODO: Reimplement once search highlighting is integrated with view-state
}

// ===== Bug Reproduction Tests =====

/// Bug reproduction: cclv-07v.12.21.2
/// Entry indices (FR-061) are not visible in rendered output.
/// Each entry should show its 1-based index in a dim column before content.
///
/// EXPECTED: "  1 │ Entry content..." with visible index column
/// ACTUAL: "Entry content..." with no index visible
#[test]
#[ignore = "cclv-07v.12.21.2: entry indices (FR-061) not rendered in conversation view"]
fn bug_entry_indices_not_visible_in_rendered_output() {
    // Create a simple entry
    let entry = create_test_log_entry(
        "msg-1",
        Role::User,
        MessageContent::Text("First message content".to_string()),
        EntryType::User,
    );

    let conversation = create_test_conversation(vec![entry]);
    let mut view_state = ConversationViewState::new(None, None, conversation.clone());
    view_state.relayout(60, WrapMode::Wrap);
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
    let has_index_column = output.contains("1│") || output.contains("1 │");

    // BUG: This assertion will FAIL because no index column is rendered.
    assert!(
        has_index_column,
        "Entry index should be visible (FR-061).\n\
         Expected format like '  1 │ First message content'\n\
         But no index column found in output:\n{}",
        output
    );
}

/// Bug reproduction: cclv-07v.12.21.4
/// Initial screen is blank until user presses a key.
///
/// EXPECTED: Content visible immediately after app creation.
/// ACTUAL: Terminal buffer is empty until first event triggers render.
#[test]
fn bug_initial_screen_blank_until_keypress() {
    use cclv::source::FileSource;
    use cclv::state::AppState;
    use cclv::view::TuiApp;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
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
    let key_bindings = cclv::config::keybindings::KeyBindings::default();
    let input_source =
        cclv::source::InputSource::Stdin(cclv::source::StdinSource::from_reader(&b""[..]));

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
#[ignore = "cclv-07v.12.21.3: excessive blank lines before/between entries"]
fn bug_excessive_blank_lines_in_entry_rendering() {
    use cclv::source::FileSource;
    use cclv::state::AppState;
    use cclv::view::TuiApp;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
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

    let key_bindings = cclv::config::keybindings::KeyBindings::default();
    let input_source =
        cclv::source::InputSource::Stdin(cclv::source::StdinSource::from_reader(&b""[..]));

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
    use cclv::source::FileSource;
    use cclv::state::AppState;
    use cclv::view::TuiApp;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
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

    let key_bindings = cclv::config::keybindings::KeyBindings::default();
    let input_source =
        cclv::source::InputSource::Stdin(cclv::source::StdinSource::from_reader(&b""[..]));

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
    use cclv::source::FileSource;
    use cclv::state::AppState;
    use cclv::view::TuiApp;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
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
        entry_count >= 30_000,
        "Fixture should have 30,000+ entries for this test"
    );

    // Build session

    // Create TuiApp
    let backend = TestBackend::new(100, 46);
    let terminal = Terminal::new(backend).unwrap();
    let mut app_state = AppState::new();

    // CRITICAL: Populate log_view from session entries (dual-write pattern)
    app_state.add_entries(entries);

    let key_bindings = cclv::config::keybindings::KeyBindings::default();
    let input_source =
        cclv::source::InputSource::Stdin(cclv::source::StdinSource::from_reader(&b""[..]));

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
    use cclv::source::FileSource;
    use cclv::state::AppState;
    use cclv::view::TuiApp;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
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

    let key_bindings = cclv::config::keybindings::KeyBindings::default();
    let input_source =
        cclv::source::InputSource::Stdin(cclv::source::StdinSource::from_reader(&b""[..]));

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
    use cclv::source::FileSource;
    use cclv::state::AppState;
    use cclv::view::TuiApp;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
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
    let key_bindings = cclv::config::keybindings::KeyBindings::default();
    let input_source =
        cclv::source::InputSource::Stdin(cclv::source::StdinSource::from_reader(&b""[..]));

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
    use cclv::source::FileSource;
    use cclv::state::AppState;
    use cclv::view::TuiApp;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
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

    let key_bindings = cclv::config::keybindings::KeyBindings::default();
    let input_source =
        cclv::source::InputSource::Stdin(cclv::source::StdinSource::from_reader(&b""[..]));

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
#[ignore = "cclv-5ur.9: thinking blocks truncated instead of wrapped"]
fn bug_thinking_blocks_not_wrapped_like_prose() {
    use cclv::source::FileSource;
    use cclv::state::AppState;
    use cclv::view::TuiApp;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
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
    let key_bindings = cclv::config::keybindings::KeyBindings::default();
    let input_source =
        cclv::source::InputSource::Stdin(cclv::source::StdinSource::from_reader(&b""[..]));

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
    use cclv::source::FileSource;
    use cclv::state::AppState;
    use cclv::view::TuiApp;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
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
    let key_bindings = cclv::config::keybindings::KeyBindings::default();
    let input_source =
        cclv::source::InputSource::Stdin(cclv::source::StdinSource::from_reader(&b""[..]));

    let mut app =
        TuiApp::new_for_test(terminal, app_state, input_source, entry_count, key_bindings);

    // Initial render
    app.render_test().expect("Initial render should succeed");

    // Toggle to NoWrap mode (default is Wrap)
    // Press 'W' (Shift+w) to toggle GLOBAL wrap mode
    let toggle_global_wrap = KeyEvent::new(KeyCode::Char('W'), KeyModifiers::SHIFT);
    app.handle_key_test(toggle_global_wrap);
    app.render_test().expect("Render after wrap toggle should succeed");

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
#[ignore = "cclv-5ur.14: scroll behavior bug - tracked by cclv-5ur.14.8"]
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
    let mut state = ConversationViewState::new(None, None, entries);
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

    use cclv::view_state::scroll::ScrollPosition;
    use cclv::view_state::types::LineOffset;

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
#[ignore = "cclv-5ur.14: height mismatch bug - tracked by cclv-5ur.14.8"]
fn bug_collapsed_entry_height_mismatch() {
    use cclv::source::FileSource;
    // calculate_height is now used internally by ConversationViewState
    use cclv::view_state::scroll::ScrollPosition;
    use cclv::view_state::types::LineOffset;
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
    let mut state = ConversationViewState::new(None, None, entries);
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
    use cclv::source::FileSource;
    // calculate_height is now used internally by ConversationViewState
    use cclv::view_state::scroll::ScrollPosition;
    use cclv::view_state::types::LineOffset;
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
    let mut state = ConversationViewState::new(None, None, entries.clone());
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
    let mut view_state = ConversationViewState::new(None, None, conversation.clone());

    // Use narrow viewport (60 chars) to force wrapping
    view_state.relayout(60, WrapMode::Wrap);
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
    let mut view_state = ConversationViewState::new(None, None, conversation.clone());

    // Use narrow viewport (60 chars) but disable wrapping
    view_state.relayout(60, WrapMode::NoWrap);
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
    use cclv::view_state::layout_params::LayoutParams;
    use cclv::view_state::types::EntryIndex;

    // Create entry with long line
    let long_line = "This is a very long line that definitely exceeds the viewport width and should wrap to multiple lines when wrap mode is enabled. The content should continue on the next line without truncation.";
    let entry = create_test_log_entry(
        "override-test-1",
        Role::User,
        MessageContent::Text(long_line.to_string()),
        EntryType::User,
    );

    let conversation = create_test_conversation(vec![entry]);
    let mut view_state = ConversationViewState::new(None, None, conversation.clone());

    // Set global wrap to Wrap
    let params = LayoutParams::new(60, WrapMode::Wrap);
    view_state.relayout(60, WrapMode::Wrap);

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

/// Bug reproduction: Subagent entries not routed to separate tabs
///
/// EXPECTED: Entries with parent_tool_use_id should appear in separate subagent tabs.
///           Per FR-003: "System MUST display subagent conversations in a tabbed pane"
///           Per FR-004: "System MUST create a new tab when a subagent spawn event is detected"
///
/// ACTUAL: All entries appear in the main agent conversation. No subagent tabs are created.
///
/// ROOT CAUSE: Parser looks for 'agentId' field (which doesn't exist in Claude Code JSONL).
///             Subagent entries are identified by 'parent_tool_use_id' field, not 'agentId'.
///
/// Steps to reproduce manually:
/// 1. cargo run --release -- tests/fixtures/subagent_tab_repro.jsonl
/// 2. Observe: All entries are in "Main Agent" - no subagent tabs exist
/// 3. Entries with parent_tool_use_id should be in a separate tab
#[test]
#[ignore = "cclv-5ur.34: Subagent entries not routed to separate tabs"]
fn bug_subagent_entries_not_in_separate_tabs() {
    use cclv::config::keybindings::KeyBindings;
    use cclv::source::{FileSource, InputSource, StdinSource};
    use cclv::state::AppState;
    use cclv::view::TuiApp;
    use std::path::PathBuf;

    // Load minimal fixture with subagent entries
    let mut file_source = FileSource::new(PathBuf::from("tests/fixtures/subagent_tab_repro.jsonl"))
        .expect("Should load fixture");
    let log_entries = file_source
        .drain_entries()
        .expect("Should parse entries");
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

    let mut app = TuiApp::new_for_test(terminal, app_state, input_source, entry_count, key_bindings);

    app.render_test().expect("Render should succeed");

    let buffer = app.terminal().backend().buffer();
    let output = buffer_to_string(buffer);

    // Snapshot captures buggy state (no subagent tabs)
    insta::assert_snapshot!("bug_subagent_entries_not_in_tabs", output);

    // This assertion FAILS due to the bug:
    // The fixture contains entries with parent_tool_use_id which should create subagent tabs,
    // but has_subagents() returns false because parser doesn't recognize them.
    let has_subagents = app.app_state().session_view().has_subagents();
    assert!(
        has_subagents,
        "BUG: Entries with parent_tool_use_id should create subagent tabs.\n\
         Expected: has_subagents() == true (fixture contains 3 entries with parent_tool_use_id)\n\
         Actual: has_subagents() == false (all entries routed to main conversation)\n\
         Root cause: Parser looks for 'agentId' field, not 'parent_tool_use_id'\n\
         Output:\n{output}"
    );
}
