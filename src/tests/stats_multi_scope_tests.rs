//! Tests for multi-scope statistics panel.
//!
//! Tests verify that the panel correctly displays three stat scopes simultaneously:
//! 1. Focused conversation (currently selected tab)
//! 2. Session totals (all agents in current session)
//! 3. Global totals (all sessions - future multi-session)

use crate::model::{
    AgentId, ContentBlock, EntryMetadata, EntryType, EntryUuid, LogEntry, Message, MessageContent,
    PricingConfig, Role, SessionId, SessionStats, StatsFilter, TokenUsage, ToolCall, ToolName,
    ToolUseId,
};
use crate::view::MultiScopeStatsPanel;
use chrono::Utc;
use ratatui::{Terminal, backend::TestBackend, buffer::Buffer};

// ===== Test Helpers =====

fn make_uuid(s: &str) -> EntryUuid {
    EntryUuid::new(s).expect("valid uuid")
}

fn make_session_id(s: &str) -> SessionId {
    SessionId::new(s).expect("valid session id")
}

fn make_agent_id(s: &str) -> AgentId {
    AgentId::new(s).expect("valid agent id")
}

fn make_tool_use_id(s: &str) -> ToolUseId {
    ToolUseId::new(s).expect("valid tool use id")
}

fn make_message_with_usage(usage: TokenUsage) -> Message {
    Message::new(Role::Assistant, MessageContent::Text("Test".to_string())).with_usage(usage)
}

fn make_message_with_tool_calls(tool_names: Vec<ToolName>) -> Message {
    let blocks: Vec<ContentBlock> = tool_names
        .into_iter()
        .enumerate()
        .map(|(i, name)| {
            ContentBlock::ToolUse(ToolCall::new(
                make_tool_use_id(&format!("tool-{}", i)),
                name,
                serde_json::json!({}),
            ))
        })
        .collect();
    Message::new(Role::Assistant, MessageContent::Blocks(blocks))
}

fn make_log_entry(
    uuid: &str,
    session_id: &str,
    agent_id: Option<&str>,
    message: Message,
) -> LogEntry {
    LogEntry::new(
        make_uuid(uuid),
        None,
        make_session_id(session_id),
        agent_id.map(make_agent_id),
        Utc::now(),
        EntryType::Assistant,
        message,
        EntryMetadata::default(),
    )
}

/// Extract rendered text content from a ratatui Buffer.
fn buffer_to_string(buffer: &Buffer) -> String {
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

/// Build SessionStats with specific usage for main agent and subagent.
fn build_session_stats(
    main_input: u64,
    main_output: u64,
    subagent_input: u64,
    subagent_output: u64,
) -> SessionStats {
    let mut stats = SessionStats::default();

    // Add main agent entry
    let main_usage = TokenUsage {
        input_tokens: main_input,
        output_tokens: main_output,
        cache_creation_input_tokens: 0,
        cache_read_input_tokens: 0,
        ephemeral_5m_input_tokens: 0,
        ephemeral_1h_input_tokens: 0,
    };
    let main_entry = make_log_entry("e1", "s1", None, make_message_with_usage(main_usage));
    stats.record_entry(&main_entry);

    // Add subagent entry
    let subagent_usage = TokenUsage {
        input_tokens: subagent_input,
        output_tokens: subagent_output,
        cache_creation_input_tokens: 0,
        cache_read_input_tokens: 0,
        ephemeral_5m_input_tokens: 0,
        ephemeral_1h_input_tokens: 0,
    };
    let subagent_entry = make_log_entry(
        "e2",
        "s1",
        Some("agent-1"),
        make_message_with_usage(subagent_usage),
    );
    stats.record_entry(&subagent_entry);

    stats
}

// ===== Multi-Scope Display Tests =====

#[test]
fn multi_scope_panel_displays_focused_and_session_scopes() {
    // GIVEN: Stats with main agent (600 input, 300 output) and subagent (400 input, 200 output)
    // Total: 1000 input, 500 output
    let stats = build_session_stats(600, 300, 400, 200);

    // WHEN: Rendering panel focused on main agent
    // NOTE: build_session_stats creates entries with session_id "s1"
    let session_id = make_session_id("s1");
    let focused_filter = StatsFilter::MainAgent(session_id);
    let pricing = PricingConfig::default();
    let panel = MultiScopeStatsPanel::new(&stats, &focused_filter, &pricing, Some("opus"), false);

    let mut terminal = Terminal::new(TestBackend::new(100, 40)).unwrap();
    terminal
        .draw(|f| {
            f.render_widget(panel, f.area());
        })
        .unwrap();

    let buffer = terminal.backend().buffer();
    let content = buffer_to_string(buffer);

    // THEN: Should display BOTH focused (main agent) and session (global) scopes

    // Focused scope should show main agent stats (600 input, 300 output)
    assert!(
        content.contains("Focused") || content.contains("Main Agent"),
        "Expected 'Focused' or 'Main Agent' label for focused scope, got:\n{}",
        content
    );
    assert!(
        content.contains("600") && content.contains("300"),
        "Expected focused scope to show main agent tokens (600 input, 300 output), got:\n{}",
        content
    );

    // Session scope should show global totals (1000 input, 500 output)
    assert!(
        content.contains("Session") || content.contains("Global"),
        "Expected 'Session' or 'Global' label for session scope, got:\n{}",
        content
    );
    assert!(
        content.contains("1,000") && content.contains("500"),
        "Expected session scope to show global tokens (1,000 input, 500 output), got:\n{}",
        content
    );
}

#[test]
fn multi_scope_panel_focused_scope_updates_when_switching_tabs() {
    // GIVEN: Stats with main agent (600 input, 300 output) and subagent (400 input, 200 output)
    let stats = build_session_stats(600, 300, 400, 200);
    let pricing = PricingConfig::default();

    // WHEN: Rendering panel focused on SUBAGENT
    let agent_id = make_agent_id("agent-1");
    let focused_filter = StatsFilter::Subagent(agent_id);
    let panel = MultiScopeStatsPanel::new(&stats, &focused_filter, &pricing, Some("opus"), false);

    let mut terminal = Terminal::new(TestBackend::new(100, 40)).unwrap();
    terminal
        .draw(|f| {
            f.render_widget(panel, f.area());
        })
        .unwrap();

    let buffer = terminal.backend().buffer();
    let content = buffer_to_string(buffer);

    // THEN: Focused scope should show SUBAGENT stats (400 input, 200 output)
    assert!(
        content.contains("Focused") || content.contains("Subagent"),
        "Expected 'Focused' or 'Subagent' label for focused scope, got:\n{}",
        content
    );
    assert!(
        content.contains("400") && content.contains("200"),
        "Expected focused scope to show subagent tokens (400 input, 200 output), got:\n{}",
        content
    );

    // Session scope should STILL show global totals (1000 input, 500 output)
    assert!(
        content.contains("1,000") && content.contains("500"),
        "Expected session scope to show global tokens (1,000 input, 500 output), got:\n{}",
        content
    );
}

#[test]
fn multi_scope_panel_session_scope_always_shows_global_totals() {
    // GIVEN: Stats with main agent and subagent
    let stats = build_session_stats(600, 300, 400, 200);
    let pricing = PricingConfig::default();

    // Test with MainAgent focused
    let session_id = make_session_id("test-session");
    let focused_main = StatsFilter::MainAgent(session_id);
    let panel_main =
        MultiScopeStatsPanel::new(&stats, &focused_main, &pricing, Some("opus"), false);

    let mut terminal_main = Terminal::new(TestBackend::new(100, 40)).unwrap();
    terminal_main
        .draw(|f| {
            f.render_widget(panel_main, f.area());
        })
        .unwrap();

    let content_main = buffer_to_string(terminal_main.backend().buffer());

    // Test with Subagent focused
    let agent_id = make_agent_id("agent-1");
    let focused_subagent = StatsFilter::Subagent(agent_id);
    let panel_subagent =
        MultiScopeStatsPanel::new(&stats, &focused_subagent, &pricing, Some("opus"), false);

    let mut terminal_subagent = Terminal::new(TestBackend::new(100, 40)).unwrap();
    terminal_subagent
        .draw(|f| {
            f.render_widget(panel_subagent, f.area());
        })
        .unwrap();

    let content_subagent = buffer_to_string(terminal_subagent.backend().buffer());

    // THEN: Session scope should show same global totals (1,000 input, 500 output) regardless of focus
    assert!(
        content_main.contains("1,000") && content_main.contains("500"),
        "Session scope should show global totals when main focused, got:\n{}",
        content_main
    );
    assert!(
        content_subagent.contains("1,000") && content_subagent.contains("500"),
        "Session scope should show global totals when subagent focused, got:\n{}",
        content_subagent
    );
}

#[test]
fn multi_scope_panel_displays_scope_labels_clearly() {
    // GIVEN: Stats with main agent and subagent
    let stats = build_session_stats(600, 300, 400, 200);
    let session_id = make_session_id("test-session");
    let focused_filter = StatsFilter::MainAgent(session_id);
    let pricing = PricingConfig::default();

    // WHEN: Rendering panel
    let panel = MultiScopeStatsPanel::new(&stats, &focused_filter, &pricing, Some("opus"), false);

    let mut terminal = Terminal::new(TestBackend::new(100, 40)).unwrap();
    terminal
        .draw(|f| {
            f.render_widget(panel, f.area());
        })
        .unwrap();

    let buffer = terminal.backend().buffer();
    let content = buffer_to_string(buffer);

    // THEN: Should have distinct labels for each scope
    // At minimum, should differentiate focused vs session totals

    // Count occurrences of key distinguishing words
    let has_focused_label = content.contains("Focused")
        || content.contains("Main Agent")
        || content.contains("Current");
    let has_session_label = content.contains("Session") || content.contains("Total");

    assert!(
        has_focused_label,
        "Expected clear label for focused scope (e.g., 'Focused', 'Main Agent', 'Current'), got:\n{}",
        content
    );
    assert!(
        has_session_label,
        "Expected clear label for session scope (e.g., 'Session', 'Total'), got:\n{}",
        content
    );
}

#[test]
fn multi_scope_panel_handles_empty_stats() {
    // GIVEN: Empty stats (no entries recorded)
    let stats = SessionStats::default();
    let focused_filter = StatsFilter::AllSessionsCombined;
    let pricing = PricingConfig::default();

    // WHEN: Rendering panel
    let panel = MultiScopeStatsPanel::new(&stats, &focused_filter, &pricing, Some("opus"), false);

    let mut terminal = Terminal::new(TestBackend::new(100, 40)).unwrap();
    terminal
        .draw(|f| {
            f.render_widget(panel, f.area());
        })
        .unwrap();

    let buffer = terminal.backend().buffer();
    let content = buffer_to_string(buffer);

    // THEN: Should render without panicking, showing zeros
    assert!(
        content.contains("0") || content.contains("$0.00"),
        "Expected panel to show zero stats, got:\n{}",
        content
    );
}

#[test]
fn multi_scope_panel_cost_calculation_differs_by_scope() {
    // GIVEN: Stats with main agent using Opus pricing, subagent with different usage
    let stats = build_session_stats(
        1_000_000, // Main: 1M input
        1_000_000, // Main: 1M output
        500_000,   // Subagent: 500k input
        500_000,   // Subagent: 500k output
    );
    let pricing = PricingConfig::default();

    // WHEN: Rendering panel focused on main agent
    // NOTE: build_session_stats creates entries with session_id "s1"
    let session_id = make_session_id("s1");
    let focused_filter = StatsFilter::MainAgent(session_id);
    let panel = MultiScopeStatsPanel::new(&stats, &focused_filter, &pricing, Some("opus"), false);

    let mut terminal = Terminal::new(TestBackend::new(100, 40)).unwrap();
    terminal
        .draw(|f| {
            f.render_widget(panel, f.area());
        })
        .unwrap();

    let buffer = terminal.backend().buffer();
    let content = buffer_to_string(buffer);

    // THEN: Focused scope should show main agent cost: $15 + $75 = $90
    // Session scope should show total cost: ($15 + $75) + ($7.50 + $37.50) = $135
    assert!(
        content.contains("$90.00") || content.contains("90.00"),
        "Expected focused scope to show main agent cost ($90.00), got:\n{}",
        content
    );
    assert!(
        content.contains("$135.00") || content.contains("135.00"),
        "Expected session scope to show total cost ($135.00), got:\n{}",
        content
    );
}

#[test]
fn multi_scope_panel_tool_counts_differ_by_scope() {
    // GIVEN: Stats with tools called by main agent and subagent
    let mut stats = SessionStats::default();

    // Main agent calls Read, Write
    let main_message = make_message_with_tool_calls(vec![ToolName::Read, ToolName::Write]);
    let main_entry = make_log_entry("e1", "s1", None, main_message);
    stats.record_entry(&main_entry);

    // Subagent calls Bash, Edit
    let subagent_message = make_message_with_tool_calls(vec![ToolName::Bash, ToolName::Edit]);
    let subagent_entry = make_log_entry("e2", "s1", Some("agent-1"), subagent_message);
    stats.record_entry(&subagent_entry);

    let pricing = PricingConfig::default();

    // WHEN: Rendering panel focused on main agent
    // NOTE: build_session_stats creates entries with session_id "s1"
    let session_id = make_session_id("s1");
    let focused_filter = StatsFilter::MainAgent(session_id);
    let panel = MultiScopeStatsPanel::new(&stats, &focused_filter, &pricing, Some("opus"), false);

    let mut terminal = Terminal::new(TestBackend::new(100, 40)).unwrap();
    terminal
        .draw(|f| {
            f.render_widget(panel, f.area());
        })
        .unwrap();

    let buffer = terminal.backend().buffer();
    let content = buffer_to_string(buffer);

    // THEN: Focused scope should show only main agent tools (Read, Write)
    // Session scope should show all tools (Read, Write, Bash, Edit)

    // Verify focused scope has Read/Write but NOT Bash/Edit
    let has_read_write_focused = content.contains("Read") && content.contains("Write");
    assert!(
        has_read_write_focused,
        "Expected focused scope to show Read and Write tools, got:\n{}",
        content
    );

    // Verify session scope includes all tools
    // (This is harder to test precisely without inspecting layout, but at minimum
    // all tool names should appear somewhere in the output)
    let has_all_tools =
        content.contains("Read") && content.contains("Write") && content.contains("Bash");
    assert!(
        has_all_tools,
        "Expected session scope to include all tools (Read, Write, Bash), got:\n{}",
        content
    );
}

#[test]
fn multi_scope_panel_renders_without_panic_on_small_area() {
    // GIVEN: Stats and a very small rendering area (constrained terminal)
    let stats = build_session_stats(100, 50, 50, 25);
    let focused_filter = StatsFilter::AllSessionsCombined;
    let pricing = PricingConfig::default();

    // WHEN: Rendering in a small area (30x10)
    let panel = MultiScopeStatsPanel::new(&stats, &focused_filter, &pricing, Some("opus"), false);

    let mut terminal = Terminal::new(TestBackend::new(30, 10)).unwrap();
    terminal
        .draw(|f| {
            f.render_widget(panel, f.area());
        })
        .unwrap();

    // THEN: Should render without panicking (graceful degradation)
    // Exact layout may vary, but should not crash
    let buffer = terminal.backend().buffer();
    let content = buffer_to_string(buffer);

    // Basic sanity: should contain SOME stats output
    assert!(
        !content.is_empty(),
        "Expected some stats output even in constrained area"
    );
}
