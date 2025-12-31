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
                if i % 2 == 0 {
                    Role::Assistant
                } else {
                    Role::User
                },
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
        let scroll_state = ScrollState {
            vertical_offset: offset,
            ..Default::default()
        };

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
                if i % 2 == 0 {
                    Role::Assistant
                } else {
                    Role::User
                },
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
        let scroll_state = ScrollState {
            vertical_offset: offset,
            ..Default::default()
        };

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
    let scroll_state = ScrollState::default();
    let styles = MessageStyles::new();

    let mut terminal = create_terminal(60, 10);
    terminal
        .draw(|frame| {
            let widget = ConversationView::new(&conversation, &scroll_state, &styles, false)
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
    use cclv::model::Session;
    use cclv::source::FileSource;
    use cclv::state::AppState;
    use cclv::view::TuiApp;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
    use std::path::PathBuf;

    // Load fixture
    let mut file_source = FileSource::new(PathBuf::from("tests/fixtures/blank_lines_repro.jsonl"))
        .expect("Should load fixture");
    let entries = file_source.drain_entries().expect("Should parse entries");
    let entry_count = entries.len();

    let mut session = Session::new(entries[0].session_id().clone());
    for entry in entries {
        session.add_entry(entry);
    }

    // Create app
    let backend = TestBackend::new(80, 40);
    let terminal = Terminal::new(backend).unwrap();
    let app_state = AppState::new(session);
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
    use cclv::model::Session;
    use cclv::source::FileSource;
    use cclv::state::AppState;
    use cclv::view::TuiApp;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
    use std::path::PathBuf;

    // Load real fixture that reproduces the bug
    let mut file_source = FileSource::new(PathBuf::from("tests/fixtures/blank_lines_repro.jsonl"))
        .expect("Should load fixture");
    let entries = file_source.drain_entries().expect("Should parse entries");

    let entry_count = entries.len();
    assert!(entry_count > 0, "Fixture should have entries");

    // Build session
    let mut session = Session::new(entries[0].session_id().clone());
    for entry in entries {
        session.add_entry(entry);
    }

    // Create app and render
    let backend = TestBackend::new(80, 40);
    let terminal = Terminal::new(backend).unwrap();
    let app_state = AppState::new(session);

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
