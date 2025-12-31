//! Tests for cclv-5ur.22: ToolUse and ToolResult blocks default to NoWrap
//!
//! Requirement: ToolUse and ToolResult blocks MUST NOT wrap unless per-entry override is explicitly set.
//!
//! Test Coverage:
//! 1. ToolUse block with global Wrap + no override → NoWrap (JSON on one line each)
//! 2. ToolUse block with global Wrap + explicit override Wrap → Wrap (JSON breaks across lines)
//! 3. ToolResult block with global Wrap + no override → NoWrap
//! 4. ToolResult block with explicit override Wrap → Wrap
//! 5. Text and Thinking blocks still respect global wrap mode

use crate::model::{
    ContentBlock, ConversationEntry, EntryMetadata, EntryType, EntryUuid, LogEntry, Message,
    MessageContent, PricingConfig, Role, SessionId, ToolCall, ToolName, ToolUseId,
};
use crate::state::WrapMode;
use crate::view::{ConversationView, MessageStyles};
use crate::view_state::conversation::ConversationViewState;
use crate::view_state::layout_params::LayoutParams;
use crate::view_state::types::{EntryIndex, ViewportDimensions};
use ratatui::backend::TestBackend;
use ratatui::Terminal;

// ===== Test Helpers =====

fn create_test_entry_with_tooluse(uuid: &str, long_json_value: &str) -> LogEntry {
    let tool_call = ToolCall::new(
        ToolUseId::new("tool-1").unwrap(),
        ToolName::Read,
        serde_json::json!({
            "file_path": long_json_value,  // Long value that would wrap at narrow width
            "limit": 100
        }),
    );

    let blocks = vec![
        ContentBlock::Text {
            text: "Let me read that file.".to_string(),
        },
        ContentBlock::ToolUse(tool_call),
    ];

    let message = Message::new(Role::Assistant, MessageContent::Blocks(blocks));
    LogEntry::new(
        EntryUuid::new(uuid).unwrap(),
        None,
        SessionId::new("test-session").unwrap(),
        None,
        chrono::Utc::now(),
        EntryType::Assistant,
        message,
        EntryMetadata::default(),
    )
}

fn create_test_entry_with_toolresult(uuid: &str, long_content: &str) -> LogEntry {
    let blocks = vec![ContentBlock::ToolResult {
        tool_use_id: ToolUseId::new("tool-1").unwrap(),
        content: long_content.to_string(),
        is_error: false,
    }];

    let message = Message::new(Role::User, MessageContent::Blocks(blocks));
    LogEntry::new(
        EntryUuid::new(uuid).unwrap(),
        None,
        SessionId::new("test-session").unwrap(),
        None,
        chrono::Utc::now(),
        EntryType::User,
        message,
        EntryMetadata::default(),
    )
}

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

// ===== RED Tests (should FAIL until implementation) =====

#[test]
fn test_tooluse_defaults_to_nowrap_despite_global_wrap() {
    // Create entry with ToolUse containing long JSON field that would wrap at 60 chars
    let long_path =
        "/some/very/long/path/to/a/file/that/exceeds/sixty/characters/in/total/length/marker.txt";
    let entry = create_test_entry_with_tooluse("tooluse-1", long_path);

    let conversation = vec![ConversationEntry::Valid(Box::new(entry))];
    let mut view_state =
        ConversationViewState::new(None, None, conversation, 200_000, PricingConfig::default());

    // Set global wrap to Wrap and use NARROW viewport (60 chars)
    view_state.relayout(60, WrapMode::Wrap, &crate::state::SearchState::Inactive);

    // Expand the entry to see full ToolUse block
    let params = LayoutParams::new(60, WrapMode::Wrap);
    view_state
        .toggle_expand(EntryIndex::new(0), params, ViewportDimensions::new(60, 24))
        .expect("Should toggle expand");

    let styles = MessageStyles::new();
    let mut terminal = Terminal::new(TestBackend::new(60, 24)).unwrap();
    terminal
        .draw(|frame| {
            let widget =
                ConversationView::new(&view_state, &styles, false).global_wrap(WrapMode::Wrap);
            frame.render_widget(widget, frame.area());
        })
        .unwrap();

    let output = buffer_to_string(terminal.backend().buffer());

    // CRITICAL ASSERTION: With NoWrap, the long path should be TRUNCATED, not wrapped
    // We verify that:
    // 1. The path appears on a SINGLE line (starts with "/some/very/long")
    // 2. The line is TRUNCATED (does NOT contain "marker.txt" which is beyond 60 chars)
    // If it wrapped, we'd see "marker.txt" on a subsequent line

    let path_line = output.lines().find(|line| line.contains("/some/very/long"));
    let marker_on_different_line = output
        .lines()
        .any(|line| line.contains("marker.txt") && !line.contains("/some/very/long"));

    assert!(
        path_line.is_some() && !marker_on_different_line,
        "BUG: ToolUse block wrapped despite having NO per-entry override.\n\
         Expected: ToolUse defaults to NoWrap (line truncated at width, NOT wrapped)\n\
         Actual: ToolUse respects global Wrap mode, breaking JSON across lines\n\n\
         With NoWrap, the path should be truncated on ONE line.\n\
         With Wrap, 'marker.txt' would appear on a continuation line.\n\
         Output:\n{output}"
    );
}

#[test]
fn test_tooluse_respects_explicit_wrap_override() {
    // Create entry with ToolUse containing long JSON field
    let long_path =
        "/some/very/long/path/to/a/file/that/exceeds/sixty/characters/in/total/length/marker.txt";
    let entry = create_test_entry_with_tooluse("tooluse-2", long_path);

    let conversation = vec![ConversationEntry::Valid(Box::new(entry))];
    let mut view_state =
        ConversationViewState::new(None, None, conversation, 200_000, PricingConfig::default());

    // Set global wrap to Wrap
    let params = LayoutParams::new(60, WrapMode::Wrap);
    view_state.relayout(60, WrapMode::Wrap, &crate::state::SearchState::Inactive);

    // EXPLICITLY override THIS entry to Wrap
    view_state.set_wrap_override(EntryIndex::new(0), Some(WrapMode::Wrap), params);

    // Expand to see full ToolUse block
    view_state
        .toggle_expand(EntryIndex::new(0), params, ViewportDimensions::new(60, 24))
        .expect("Should toggle expand");

    let styles = MessageStyles::new();
    let mut terminal = Terminal::new(TestBackend::new(60, 24)).unwrap();
    terminal
        .draw(|frame| {
            let widget =
                ConversationView::new(&view_state, &styles, false).global_wrap(WrapMode::Wrap);
            frame.render_widget(widget, frame.area());
        })
        .unwrap();

    let output = buffer_to_string(terminal.backend().buffer());

    // When explicitly overridden to Wrap, the long path SHOULD be split across lines
    // After the wrap_lines fix (cclv-5ur.45), content width accounts for entry index prefix,
    // causing tighter wrapping. We verify that the path is split by checking that
    // the start appears on a different line than the end of the path.
    let path_start_line = output
        .lines()
        .position(|line| line.contains("/some/very/long"));
    let path_continuation_line = output.lines().position(|line| line.contains("marker.txt"));

    assert!(
        path_start_line.is_some()
            && path_continuation_line.is_some()
            && path_start_line != path_continuation_line,
        "Per-entry Wrap override should cause ToolUse JSON to wrap.\n\
         Expected: Long path split across multiple lines\n\
         Actual: Path on single line despite explicit Wrap override\n\n\
         Output:\n{output}"
    );
}

#[test]
fn test_toolresult_defaults_to_nowrap_despite_global_wrap() {
    // Create entry with ToolResult containing long single-line content
    let long_line = "This is a very long line of tool output that definitely exceeds sixty characters in width and should NOT wrap unless explicitly overridden.";
    let entry = create_test_entry_with_toolresult("toolresult-1", long_line);

    let conversation = vec![ConversationEntry::Valid(Box::new(entry))];
    let mut view_state =
        ConversationViewState::new(None, None, conversation, 200_000, PricingConfig::default());

    // Set global wrap to Wrap and use NARROW viewport (60 chars)
    view_state.relayout(60, WrapMode::Wrap, &crate::state::SearchState::Inactive);

    // Expand to see full ToolResult block
    let params = LayoutParams::new(60, WrapMode::Wrap);
    view_state
        .toggle_expand(EntryIndex::new(0), params, ViewportDimensions::new(60, 24))
        .expect("Should toggle expand");

    let styles = MessageStyles::new();
    let mut terminal = Terminal::new(TestBackend::new(60, 24)).unwrap();
    terminal
        .draw(|frame| {
            let widget =
                ConversationView::new(&view_state, &styles, false).global_wrap(WrapMode::Wrap);
            frame.render_widget(widget, frame.area());
        })
        .unwrap();

    let output = buffer_to_string(terminal.backend().buffer());

    // CRITICAL ASSERTION: With NoWrap, the long line should be TRUNCATED, not wrapped
    // We verify that:
    // 1. The line starts on ONE line (contains "This is a very long")
    // 2. The line is TRUNCATED (does NOT contain "overridden" which is beyond 60 chars)
    // If it wrapped, we'd see "overridden" on a subsequent line

    let content_line = output
        .lines()
        .find(|line| line.contains("This is a very long"));
    let continuation_line = output
        .lines()
        .any(|line| line.contains("overridden") && !line.contains("This is a very long"));

    assert!(
        content_line.is_some() && !continuation_line,
        "BUG: ToolResult block wrapped despite having NO per-entry override.\n\
         Expected: ToolResult defaults to NoWrap (line truncated at width, NOT wrapped)\n\
         Actual: ToolResult respects global Wrap mode, breaking lines\n\n\
         With NoWrap, the content should be truncated on ONE line.\n\
         With Wrap, 'overridden' would appear on a continuation line.\n\
         Output:\n{output}"
    );
}

#[test]
fn test_toolresult_respects_explicit_wrap_override() {
    // Create entry with ToolResult containing long single-line content
    let long_line = "This is a very long line of tool output that definitely exceeds sixty characters in width and should wrap when explicitly overridden.";
    let entry = create_test_entry_with_toolresult("toolresult-2", long_line);

    let conversation = vec![ConversationEntry::Valid(Box::new(entry))];
    let mut view_state =
        ConversationViewState::new(None, None, conversation, 200_000, PricingConfig::default());

    // Set global wrap to Wrap
    let params = LayoutParams::new(60, WrapMode::Wrap);
    view_state.relayout(60, WrapMode::Wrap, &crate::state::SearchState::Inactive);

    // EXPLICITLY override THIS entry to Wrap
    view_state.set_wrap_override(EntryIndex::new(0), Some(WrapMode::Wrap), params);

    // Expand to see full ToolResult block
    view_state
        .toggle_expand(EntryIndex::new(0), params, ViewportDimensions::new(60, 24))
        .expect("Should toggle expand");

    let styles = MessageStyles::new();
    let mut terminal = Terminal::new(TestBackend::new(60, 24)).unwrap();
    terminal
        .draw(|frame| {
            let widget =
                ConversationView::new(&view_state, &styles, false).global_wrap(WrapMode::Wrap);
            frame.render_widget(widget, frame.area());
        })
        .unwrap();

    let output = buffer_to_string(terminal.backend().buffer());

    // When explicitly overridden to Wrap, the long line SHOULD be split across lines
    let line_start = output
        .lines()
        .position(|line| line.contains("This is a very long"));
    let line_end = output.lines().position(|line| line.contains("overridden"));

    assert!(
        line_start.is_some() && line_end.is_some() && line_start != line_end,
        "Per-entry Wrap override should cause ToolResult content to wrap.\n\
         Expected: Long line split across multiple lines\n\
         Actual: Line on single line despite explicit Wrap override\n\n\
         Output:\n{output}"
    );
}

#[test]
fn test_text_block_still_respects_global_wrap() {
    // Verify that Text blocks (prose) still wrap normally with global Wrap mode
    let long_text = "This is a very long line of regular text content that definitely exceeds sixty characters and should wrap normally with global Wrap mode enabled.";

    let message = Message::new(Role::User, MessageContent::Text(long_text.to_string()));
    let entry = LogEntry::new(
        EntryUuid::new("text-1").unwrap(),
        None,
        SessionId::new("test-session").unwrap(),
        None,
        chrono::Utc::now(),
        EntryType::User,
        message,
        EntryMetadata::default(),
    );

    let conversation = vec![ConversationEntry::Valid(Box::new(entry))];
    let mut view_state =
        ConversationViewState::new(None, None, conversation, 200_000, PricingConfig::default());

    // Set global wrap to Wrap and use NARROW viewport
    view_state.relayout(60, WrapMode::Wrap, &crate::state::SearchState::Inactive);

    let styles = MessageStyles::new();
    let mut terminal = Terminal::new(TestBackend::new(60, 24)).unwrap();
    terminal
        .draw(|frame| {
            let widget =
                ConversationView::new(&view_state, &styles, false).global_wrap(WrapMode::Wrap);
            frame.render_widget(widget, frame.area());
        })
        .unwrap();

    let output = buffer_to_string(terminal.backend().buffer());

    // Text blocks should WRAP - the word "enabled" at the end should be on a different line
    let text_start = output
        .lines()
        .position(|line| line.contains("This is a very long"));
    let text_end = output.lines().position(|line| line.contains("enabled"));

    assert!(
        text_start.is_some() && text_end.is_some() && text_start != text_end,
        "Text blocks should still wrap with global Wrap mode.\n\
         Expected: Long text wraps across lines\n\
         Actual: Text on single line (broke normal wrap behavior)\n\n\
         Output:\n{output}"
    );
}
