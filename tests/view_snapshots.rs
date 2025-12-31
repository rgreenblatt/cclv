//! Snapshot tests for key view components
//!
//! Uses insta + ratatui TestBackend to verify rendering output doesn't regress.
//! These tests capture the visual representation of widgets and protect against
//! accidental UI changes.

use cclv::model::{
    AgentId, ContentBlock, ConversationEntry, EntryMetadata, EntryType,
    EntryUuid, LogEntry, Message, MessageContent, PricingConfig, Role, SessionId,
    SessionStats, StatsFilter, TokenUsage, ToolCall, ToolName, ToolUseId,
};
use cclv::state::WrapMode;
use cclv::view::{tabs, ConversationView, MessageStyles, StatsPanel};
use cclv::view_state::conversation::ConversationViewState;
use cclv::view_state::layout_params::LayoutParams;
use cclv::view_state::types::{EntryIndex, LineHeight, ViewportDimensions};
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
    let view_state = ConversationViewState::new(None, None, conversation.clone());
    let styles = MessageStyles::new();

    let mut terminal = create_terminal(60, 15);
    terminal
        .draw(|frame| {
            let widget = ConversationView::new(&view_state, &styles, false)
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
    // Create view state and expand the message
    let mut view_state = ConversationViewState::new(None, None, conversation.clone());
    let params = LayoutParams::new(80, WrapMode::Wrap);
    view_state
        .toggle_expand(
            EntryIndex::new(0),
            params,
            ViewportDimensions::new(80, 24),
            |_, _, _| LineHeight::new(100).unwrap(), // Mock height calculation
        )
        .expect("Should be able to toggle expand");
    let styles = MessageStyles::new();

    let mut terminal = create_terminal(60, 20);
    terminal
        .draw(|frame| {
            let widget =
                ConversationView::new(&view_state, &styles, false)
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
    // Expand to see full code block
    let mut view_state = ConversationViewState::new(None, None, conversation.clone());
    let params = LayoutParams::new(80, WrapMode::Wrap);
    view_state.toggle_expand(
        EntryIndex::new(0),
        params,
        ViewportDimensions::new(80, 24),
        |_, _, _| LineHeight::new(100).unwrap(),
    );
    let styles = MessageStyles::new();

    let mut terminal = create_terminal(70, 25);
    terminal
        .draw(|frame| {
            let widget =
                ConversationView::new(&view_state, &styles, false)
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
    let mut view_state = ConversationViewState::new(None, None, conversation.clone());
    let params = LayoutParams::new(80, WrapMode::Wrap);
    view_state.toggle_expand(
        EntryIndex::new(0),
        params,
        ViewportDimensions::new(80, 24),
        |_, _, _| LineHeight::new(100).unwrap(),
    );
    let styles = MessageStyles::new();

    let mut terminal = create_terminal(80, 20);
    terminal
        .draw(|frame| {
            let widget =
                ConversationView::new(&view_state, &styles, false)
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
    let mut view_state = ConversationViewState::new(None, None, conversation.clone());
    let params = LayoutParams::new(80, WrapMode::Wrap);
    view_state.toggle_expand(
        EntryIndex::new(0),
        params,
        ViewportDimensions::new(80, 24),
        |_, _, _| LineHeight::new(100).unwrap(),
    );
    let styles = MessageStyles::new();

    let mut terminal = create_terminal(80, 20);
    terminal
        .draw(|frame| {
            let widget =
                ConversationView::new(&view_state, &styles, false)
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
    let mut view_state = ConversationViewState::new(None, None, conversation.clone());
    let params = LayoutParams::new(80, WrapMode::Wrap);
    view_state.toggle_expand(
        EntryIndex::new(0),
        params,
        ViewportDimensions::new(80, 24),
        |_, _, _| LineHeight::new(100).unwrap(),
    );
    let styles = MessageStyles::new();

    let mut terminal = create_terminal(80, 20);
    terminal
        .draw(|frame| {
            let widget =
                ConversationView::new(&view_state, &styles, false)
                    .global_wrap(WrapMode::Wrap);
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
    let view_state = ConversationViewState::new(None, None, conversation.clone());
    let styles = MessageStyles::new();

    let mut terminal = create_terminal(60, 10);
    terminal
        .draw(|frame| {
            let widget =
                ConversationView::new(&view_state, &styles, false)
                    .global_wrap(WrapMode::Wrap);
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
#[ignore = "Enabled by cclv-5ur.2.12, will pass after Integration phase (cclv-5ur.6) wires view-state layer"]
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
