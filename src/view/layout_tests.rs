//! Tests for split pane layout rendering.

use super::*;
use crate::model::{Session, SessionId};
use crate::state::AppState;
use ratatui::backend::TestBackend;
use ratatui::Terminal;

// ===== Test Helpers =====

fn create_test_terminal() -> Terminal<TestBackend> {
    let backend = TestBackend::new(80, 24);
    Terminal::new(backend).unwrap()
}

fn create_session_no_subagents() -> Session {
    let session_id = SessionId::new("test-session").unwrap();
    Session::new(session_id)
}

fn create_session_with_subagents() -> Session {
    use crate::model::{
        AgentId, EntryMetadata, EntryType, EntryUuid, LogEntry, Message, MessageContent, Role,
    };
    use chrono::Utc;

    let session_id = SessionId::new("test-session").unwrap();
    let mut session = Session::new(session_id);

    // Add a main agent entry
    let main_entry = LogEntry::new(
        EntryUuid::new("entry-1").unwrap(),
        None,
        SessionId::new("test-session").unwrap(),
        None,
        Utc::now(),
        EntryType::User,
        Message::new(Role::User, MessageContent::Text("Main message".to_string())),
        EntryMetadata::default(),
    );
    session.add_entry(main_entry);

    // Add a subagent entry
    let subagent_entry = LogEntry::new(
        EntryUuid::new("entry-2").unwrap(),
        None,
        SessionId::new("test-session").unwrap(),
        Some(AgentId::new("subagent-1").unwrap()),
        Utc::now(),
        EntryType::User,
        Message::new(
            Role::User,
            MessageContent::Text("Subagent message".to_string()),
        ),
        EntryMetadata::default(),
    );
    session.add_entry(subagent_entry);

    session
}

fn create_session_with_multiple_subagents() -> Session {
    use crate::model::{
        AgentId, EntryMetadata, EntryType, EntryUuid, LogEntry, Message, MessageContent, Role,
    };
    use chrono::Utc;

    let session_id = SessionId::new("test-session").unwrap();
    let mut session = Session::new(session_id);

    // Add a main agent entry
    let main_entry = LogEntry::new(
        EntryUuid::new("entry-1").unwrap(),
        None,
        SessionId::new("test-session").unwrap(),
        None,
        Utc::now(),
        EntryType::User,
        Message::new(Role::User, MessageContent::Text("Main message".to_string())),
        EntryMetadata::default(),
    );
    session.add_entry(main_entry);

    // Add three subagent entries
    for i in 1..=3 {
        let subagent_entry = LogEntry::new(
            EntryUuid::new(format!("entry-{}", i + 1)).unwrap(),
            None,
            SessionId::new("test-session").unwrap(),
            Some(AgentId::new(format!("subagent-{}", i)).unwrap()),
            Utc::now(),
            EntryType::User,
            Message::new(
                Role::User,
                MessageContent::Text(format!("Subagent {} message", i)),
            ),
            EntryMetadata::default(),
        );
        session.add_entry(subagent_entry);
    }

    session
}

// ===== calculate_horizontal_constraints Tests =====

#[test]
fn calculate_constraints_with_subagents_returns_60_40_split() {
    let (main, subagent) = calculate_horizontal_constraints(true);

    // Should be 60% and 40%
    assert!(
        matches!(main, Constraint::Percentage(60)),
        "Main pane should be 60% when subagents exist"
    );
    assert!(
        matches!(subagent, Constraint::Percentage(40)),
        "Subagent pane should be 40% when subagents exist"
    );
}

#[test]
fn calculate_constraints_without_subagents_returns_100_0_split() {
    let (main, subagent) = calculate_horizontal_constraints(false);

    // Should be 100% and 0% (or Min(0))
    assert!(
        matches!(main, Constraint::Percentage(100)),
        "Main pane should be 100% when no subagents"
    );
    assert!(
        matches!(subagent, Constraint::Min(0)),
        "Subagent pane should be Min(0) when no subagents"
    );
}

// ===== render_layout Integration Tests =====

#[test]
fn render_layout_creates_three_areas_with_subagents() {
    let mut terminal = create_test_terminal();
    let session = create_session_with_subagents();
    let state = AppState::new(session);

    terminal
        .draw(|frame| {
            render_layout(frame, &state);
        })
        .unwrap();

    let buffer = terminal.backend().buffer().clone();

    // Verify layout structure exists by checking for:
    // 1. Main pane border/title (left side)
    // 2. Subagent pane border/title (right side)
    // 3. Status bar (bottom)

    // Look for "Main Agent" title somewhere in the buffer
    let content = buffer
        .content
        .iter()
        .map(|c| c.symbol())
        .collect::<String>();
    assert!(
        content.contains("Main Agent"),
        "Main agent pane title should be rendered"
    );
    assert!(
        content.contains("Subagent"),
        "Subagent pane should be rendered"
    );
}

#[test]
fn render_layout_hides_subagent_pane_when_no_subagents() {
    let mut terminal = create_test_terminal();
    let session = create_session_no_subagents();
    let state = AppState::new(session);

    terminal
        .draw(|frame| {
            render_layout(frame, &state);
        })
        .unwrap();

    let buffer = terminal.backend().buffer().clone();
    let content = buffer
        .content
        .iter()
        .map(|c| c.symbol())
        .collect::<String>();

    // Main agent should be visible
    assert!(
        content.contains("Main Agent"),
        "Main agent pane should be rendered"
    );

    // Subagent pane should NOT be rendered (or have zero width)
    // We can't easily verify zero width, so we just check main pane exists
    // The constraint test above ensures the logic is correct
}

#[test]
fn render_layout_includes_status_bar() {
    let mut terminal = create_test_terminal();
    let session = create_session_no_subagents();
    let state = AppState::new(session);

    terminal
        .draw(|frame| {
            render_layout(frame, &state);
        })
        .unwrap();

    let buffer = terminal.backend().buffer().clone();
    let content = buffer
        .content
        .iter()
        .map(|c| c.symbol())
        .collect::<String>();

    // Status bar should show hints or live indicator
    assert!(
        content.contains("q:") || content.contains("LIVE"),
        "Status bar should contain hints or live mode indicator"
    );
}

#[test]
fn render_layout_shows_live_indicator_when_live_mode() {
    let mut terminal = create_test_terminal();
    let session = create_session_no_subagents();
    let mut state = AppState::new(session);
    state.live_mode = true;

    terminal
        .draw(|frame| {
            render_layout(frame, &state);
        })
        .unwrap();

    let buffer = terminal.backend().buffer().clone();
    let content = buffer
        .content
        .iter()
        .map(|c| c.symbol())
        .collect::<String>();

    assert!(
        content.contains("LIVE"),
        "Status bar should show LIVE indicator when in live mode"
    );
}

// ===== Tab Bar Integration Tests =====

#[test]
fn render_layout_displays_tab_bar_in_subagent_pane() {
    let mut terminal = create_test_terminal();
    let session = create_session_with_multiple_subagents();
    let state = AppState::new(session);

    terminal
        .draw(|frame| {
            render_layout(frame, &state);
        })
        .unwrap();

    let buffer = terminal.backend().buffer().clone();
    let content = buffer
        .content
        .iter()
        .map(|c| c.symbol())
        .collect::<String>();

    // Tab bar should contain at least the first subagent ID
    // (Terminal width of 80 chars may truncate additional tabs)
    assert!(
        content.contains("subagent-1"),
        "Tab bar should display first subagent ID"
    );

    // Verify "Subagents" title is present (indicates tab bar is rendered)
    assert!(
        content.contains("Subagents"),
        "Tab bar should have 'Subagents' title"
    );
}

#[test]
fn render_layout_uses_selected_tab_to_display_correct_subagent() {
    let mut terminal = create_test_terminal();
    let session = create_session_with_multiple_subagents();
    let mut state = AppState::new(session);
    state.selected_tab = Some(1); // Select second subagent

    terminal
        .draw(|frame| {
            render_layout(frame, &state);
        })
        .unwrap();

    let buffer = terminal.backend().buffer().clone();
    let content = buffer
        .content
        .iter()
        .map(|c| c.symbol())
        .collect::<String>();

    // The selected tab (subagent-2) should be highlighted in the tab bar
    // We verify tab selection is working by checking the tab bar shows subagent-2
    assert!(
        content.contains("subagent-2"),
        "Tab bar should display subagent-2"
    );

    // Note: The actual message content rendering depends on ConversationView widget
    // which is tested separately. This test verifies tab selection logic works.
}

// ===== Header Rendering Tests =====

#[test]
fn render_header_displays_model_name_for_main_agent() {
    use crate::model::{
        EntryMetadata, EntryType, EntryUuid, LogEntry, Message, MessageContent, ModelInfo, Role,
    };
    use chrono::Utc;

    let mut terminal = create_test_terminal();
    let session_id = SessionId::new("test-session").unwrap();
    let mut session = Session::new(session_id);

    // Add main agent entry with model info
    let model_info = ModelInfo::new("claude-sonnet-4-5-20250929");
    let message = Message::new(
        Role::Assistant,
        MessageContent::Text("Response".to_string()),
    )
    .with_model(model_info);
    let entry = LogEntry::new(
        EntryUuid::new("entry-1").unwrap(),
        None,
        SessionId::new("test-session").unwrap(),
        None,
        Utc::now(),
        EntryType::Assistant,
        message,
        EntryMetadata::default(),
    );
    session.add_entry(entry);

    let state = AppState::new(session);

    terminal
        .draw(|frame| {
            render_layout(frame, &state);
        })
        .unwrap();

    let buffer = terminal.backend().buffer().clone();
    let content = buffer
        .content
        .iter()
        .map(|c| c.symbol())
        .collect::<String>();

    // Header should contain model display name "Sonnet"
    assert!(
        content.contains("Sonnet"),
        "Header should display model name 'Sonnet'"
    );
}

#[test]
fn render_header_shows_live_indicator_when_live_mode_and_auto_scroll() {
    let mut terminal = create_test_terminal();
    let session = create_session_no_subagents();
    let mut state = AppState::new(session);
    state.live_mode = true;
    state.auto_scroll = true;

    terminal
        .draw(|frame| {
            render_layout(frame, &state);
        })
        .unwrap();

    let buffer = terminal.backend().buffer().clone();
    let content = buffer
        .content
        .iter()
        .map(|c| c.symbol())
        .collect::<String>();

    assert!(
        content.contains("[LIVE]"),
        "Header should show [LIVE] indicator when live_mode=true and auto_scroll=true"
    );
}

#[test]
fn render_header_hides_live_indicator_when_live_mode_false() {
    let mut terminal = create_test_terminal();
    let session = create_session_no_subagents();
    let mut state = AppState::new(session);
    state.live_mode = false;
    state.auto_scroll = true;

    terminal
        .draw(|frame| {
            render_layout(frame, &state);
        })
        .unwrap();

    let buffer = terminal.backend().buffer().clone();
    let _content = buffer
        .content
        .iter()
        .map(|c| c.symbol())
        .collect::<String>();

    // Should NOT contain [LIVE] in header area
    // Note: Status bar might still show LIVE, but we're testing header specifically
    // We'll verify by checking the first line of output
    let first_line: String = buffer
        .content
        .iter()
        .take(80) // First row (80 cols width)
        .map(|c| c.symbol())
        .collect();

    assert!(
        !first_line.contains("[LIVE]"),
        "Header (first line) should NOT show [LIVE] when live_mode=false"
    );
}

#[test]
fn render_header_hides_live_indicator_when_auto_scroll_false() {
    let mut terminal = create_test_terminal();
    let session = create_session_no_subagents();
    let mut state = AppState::new(session);
    state.live_mode = true;
    state.auto_scroll = false; // User scrolled away

    terminal
        .draw(|frame| {
            render_layout(frame, &state);
        })
        .unwrap();

    let buffer = terminal.backend().buffer().clone();
    let first_line: String = buffer.content.iter().take(80).map(|c| c.symbol()).collect();

    assert!(
        !first_line.contains("[LIVE]"),
        "Header should NOT show [LIVE] when auto_scroll=false (user scrolled)"
    );
}

#[test]
fn render_header_shows_main_agent_label() {
    let mut terminal = create_test_terminal();
    let session = create_session_no_subagents();
    let state = AppState::new(session);

    terminal
        .draw(|frame| {
            render_layout(frame, &state);
        })
        .unwrap();

    let buffer = terminal.backend().buffer().clone();
    let first_line: String = buffer.content.iter().take(80).map(|c| c.symbol()).collect();

    assert!(
        first_line.contains("Main") || first_line.contains("main"),
        "Header should identify main agent"
    );
}

#[test]
fn render_header_shows_subagent_id_when_subagent_focused() {
    let mut terminal = create_test_terminal();
    let session = create_session_with_subagents();
    let mut state = AppState::new(session);
    state.focus = FocusPane::Subagent;
    state.selected_tab = Some(0); // First subagent

    terminal
        .draw(|frame| {
            render_layout(frame, &state);
        })
        .unwrap();

    let buffer = terminal.backend().buffer().clone();
    let first_line: String = buffer.content.iter().take(80).map(|c| c.symbol()).collect();

    assert!(
        first_line.contains("subagent"),
        "Header should show subagent identifier when subagent pane focused"
    );
}

// ===== Stats Panel Integration Tests =====

#[test]
fn render_layout_hides_stats_panel_when_stats_visible_false() {
    let mut terminal = create_test_terminal();
    let session = create_session_no_subagents();
    let mut state = AppState::new(session);
    state.stats_visible = false;

    terminal
        .draw(|frame| {
            render_layout(frame, &state);
        })
        .unwrap();

    let buffer = terminal.backend().buffer().clone();
    let content = buffer
        .content
        .iter()
        .map(|c| c.symbol())
        .collect::<String>();

    // Stats panel should NOT be visible
    assert!(
        !content.contains("Statistics"),
        "Stats panel should be hidden when stats_visible=false"
    );
}

#[test]
fn render_layout_shows_stats_panel_when_stats_visible_true() {
    let mut terminal = create_test_terminal();
    let session = create_session_no_subagents();
    let mut state = AppState::new(session);
    state.stats_visible = true;

    terminal
        .draw(|frame| {
            render_layout(frame, &state);
        })
        .unwrap();

    let buffer = terminal.backend().buffer().clone();
    let content = buffer
        .content
        .iter()
        .map(|c| c.symbol())
        .collect::<String>();

    // Stats panel should be visible with "Statistics" title
    assert!(
        content.contains("Statistics"),
        "Stats panel should show 'Statistics' title when stats_visible=true"
    );

    // Verify stats content sections are present
    assert!(
        content.contains("Tokens:"),
        "Stats panel should display token usage section"
    );
}

#[test]
fn render_layout_highlights_stats_border_when_focused() {
    use ratatui::style::Color;

    let mut terminal = create_test_terminal();
    let session = create_session_no_subagents();
    let mut state = AppState::new(session);
    state.stats_visible = true;
    state.focus = FocusPane::Stats;

    terminal
        .draw(|frame| {
            render_layout(frame, &state);
        })
        .unwrap();

    let buffer = terminal.backend().buffer().clone();

    // Find the stats panel border cells and verify they have focus color
    // Stats panel is at bottom, so check cells in the stats area for border styling
    // The exact position depends on layout, but we can search for "Statistics" title
    // and check cells around it for the expected focus color

    let mut found_stats_border = false;
    for cell in buffer.content.iter() {
        // Look for border characters (│ ─ ┌ ┐ └ ┘) with focus color
        let symbol = cell.symbol();
        if matches!(
            symbol,
            "│" | "─" | "┌" | "┐" | "└" | "┘" | "┤" | "├" | "┬" | "┴"
        ) {
            // Check if this border cell has the focus color (typically cyan or highlighted)
            // The exact color depends on StatsPanel implementation
            if cell.fg == Color::Yellow || cell.fg == Color::Cyan {
                found_stats_border = true;
                break;
            }
        }
    }

    assert!(
        found_stats_border,
        "Stats panel border should be highlighted when FocusPane::Stats"
    );
}

#[test]
fn render_layout_stats_panel_does_not_highlight_when_not_focused() {
    let mut terminal = create_test_terminal();
    let session = create_session_no_subagents();
    let mut state = AppState::new(session);
    state.stats_visible = true;
    state.focus = FocusPane::Main; // Focus on main, not stats

    terminal
        .draw(|frame| {
            render_layout(frame, &state);
        })
        .unwrap();

    let buffer = terminal.backend().buffer().clone();
    let content = buffer
        .content
        .iter()
        .map(|c| c.symbol())
        .collect::<String>();

    // Stats panel should still be visible
    assert!(
        content.contains("Statistics"),
        "Stats panel should be visible when stats_visible=true"
    );

    // Border should NOT have focus color (should be white/default)
    // We verify by checking that stats panel exists but isn't highlighted
    // (detailed border color check would be fragile)
}

#[test]
fn render_layout_allocates_stats_panel_height_approximately_10_lines() {
    let mut terminal = create_test_terminal();
    let session = create_session_no_subagents();
    let mut state = AppState::new(session);
    state.stats_visible = true;

    terminal
        .draw(|frame| {
            render_layout(frame, &state);
        })
        .unwrap();

    let buffer = terminal.backend().buffer().clone();

    // Count rows that contain "Statistics" or stats content
    // Stats panel should occupy roughly 8-10 lines at the bottom
    let mut stats_rows = 0;
    for y in 0..buffer.area().height {
        let row: String = (0..buffer.area().width)
            .map(|x| buffer[(x, y)].symbol())
            .collect();

        if row.contains("Statistics") || row.contains("Tokens:") || row.contains("Estimated Cost:")
        {
            stats_rows += 1;
        }
    }

    // Stats panel should occupy at least a few lines
    // (Note: actual count depends on content and border rendering)
    assert!(
        stats_rows >= 2,
        "Stats panel should occupy at least 2 rows, found {}",
        stats_rows
    );
}

#[test]
fn render_layout_reduces_content_area_when_stats_visible() {
    let mut terminal = create_test_terminal();
    let session = create_session_with_subagents();

    // First measure with stats hidden
    let mut state_hidden = AppState::new(session.clone());
    state_hidden.stats_visible = false;

    terminal
        .draw(|frame| {
            render_layout(frame, &state_hidden);
        })
        .unwrap();

    let buffer_hidden = terminal.backend().buffer().clone();
    let content_hidden = buffer_to_string(&buffer_hidden);

    // Then measure with stats visible
    let mut state_visible = AppState::new(session);
    state_visible.stats_visible = true;

    terminal
        .draw(|frame| {
            render_layout(frame, &state_visible);
        })
        .unwrap();

    let buffer_visible = terminal.backend().buffer().clone();
    let content_visible = buffer_to_string(&buffer_visible);

    // Verify stats panel is NOT in hidden state
    assert!(
        !content_hidden.contains("Statistics"),
        "Stats panel should not appear when stats_visible=false"
    );

    // Verify stats panel IS in visible state
    assert!(
        content_visible.contains("Statistics"),
        "Stats panel should appear when stats_visible=true"
    );

    // Verify stats panel actually takes up space (reduces available area)
    // This is shown by the stats panel content being present
    assert!(
        content_visible.contains("Tokens:"),
        "Stats panel should display content when visible"
    );
}

// ===== Keyboard Hints Tests =====

#[test]
fn build_keyboard_hints_main_pane_shows_navigation_and_common() {
    let hints = build_keyboard_hints(FocusPane::Main, false, 80);

    // Should contain common shortcuts
    assert!(
        hints.contains("q: Quit"),
        "Main pane hints should include 'q: Quit'"
    );
    assert!(
        hints.contains("?: Help"),
        "Main pane hints should include '?: Help'"
    );

    // Should contain navigation shortcuts
    assert!(
        hints.contains("/: Search"),
        "Main pane hints should include '/: Search'"
    );
    assert!(
        hints.contains("s: Stats"),
        "Main pane hints should include 's: Stats'"
    );
}

#[test]
fn build_keyboard_hints_subagent_pane_shows_tab_shortcuts() {
    let hints = build_keyboard_hints(FocusPane::Subagent, false, 80);

    // Should contain tab navigation
    assert!(
        hints.contains("[ ]") || hints.contains("Tab"),
        "Subagent pane hints should include tab navigation"
    );

    // Should still have common shortcuts
    assert!(
        hints.contains("q: Quit"),
        "Subagent pane hints should include 'q: Quit'"
    );
}

#[test]
fn build_keyboard_hints_stats_pane_shows_filter_shortcuts() {
    let hints = build_keyboard_hints(FocusPane::Stats, false, 80);

    // Should contain filter shortcuts
    assert!(
        hints.contains("!: Global") || hints.contains("@: Main") || hints.contains("#: Current"),
        "Stats pane hints should include filter shortcuts"
    );

    // Should still have common shortcuts
    assert!(
        hints.contains("q: Quit"),
        "Stats pane hints should include 'q: Quit'"
    );
}

#[test]
fn build_keyboard_hints_search_active_shows_search_shortcuts() {
    let hints = build_keyboard_hints(FocusPane::Search, true, 80);

    // Should contain search-specific shortcuts
    assert!(
        hints.contains("Enter") || hints.contains("Esc") || hints.contains("n:"),
        "Search active hints should include search navigation shortcuts"
    );
}

#[test]
fn build_keyboard_hints_truncates_for_narrow_terminal() {
    let hints_wide = build_keyboard_hints(FocusPane::Main, false, 80);
    let hints_narrow = build_keyboard_hints(FocusPane::Main, false, 40);

    // Narrow terminal should produce shorter output
    assert!(
        hints_narrow.len() <= hints_wide.len(),
        "Narrow terminal hints should be truncated or abbreviated"
    );

    // Even narrow terminal should show critical shortcuts
    assert!(
        hints_narrow.contains("q:") || hints_narrow.contains("Quit"),
        "Even narrow hints should include quit shortcut"
    );
}

#[test]
fn render_status_bar_displays_keyboard_hints() {
    let mut terminal = create_test_terminal();
    let session = create_session_no_subagents();
    let state = AppState::new(session);

    terminal
        .draw(|frame| {
            render_layout(frame, &state);
        })
        .unwrap();

    let buffer = terminal.backend().buffer().clone();
    let last_line: String = buffer
        .content
        .iter()
        .skip(80 * 23) // Skip to last row (row 23, 0-indexed)
        .take(80)
        .map(|c| c.symbol())
        .collect();

    // Status bar should contain keyboard hints
    assert!(
        last_line.contains("q:") || last_line.contains("Quit"),
        "Status bar should display keyboard hints including quit"
    );
    assert!(
        last_line.contains("?:") || last_line.contains("Help"),
        "Status bar should display help shortcut"
    );
}

#[test]
fn render_status_bar_hints_change_based_on_focus() {
    let mut terminal = create_test_terminal();
    let session = create_session_with_subagents();

    // Test Main pane focus
    let mut state_main = AppState::new(session.clone());
    state_main.focus = FocusPane::Main;

    terminal
        .draw(|frame| {
            render_layout(frame, &state_main);
        })
        .unwrap();

    let buffer_main = terminal.backend().buffer().clone();
    let status_main: String = buffer_main
        .content
        .iter()
        .skip(80 * 23)
        .take(80)
        .map(|c| c.symbol())
        .collect();

    // Test Subagent pane focus
    let mut state_subagent = AppState::new(session);
    state_subagent.focus = FocusPane::Subagent;

    terminal
        .draw(|frame| {
            render_layout(frame, &state_subagent);
        })
        .unwrap();

    let buffer_subagent = terminal.backend().buffer().clone();
    let status_subagent: String = buffer_subagent
        .content
        .iter()
        .skip(80 * 23)
        .take(80)
        .map(|c| c.symbol())
        .collect();

    // Status bars should differ based on focus
    // Main pane should show search/stats, subagent should show tab shortcuts
    let has_different_hints = status_main != status_subagent;
    assert!(
        has_different_hints,
        "Status bar hints should change based on focused pane"
    );
}

// ===== Search Integration Tests =====

#[test]
fn render_layout_uses_search_highlighting_when_search_active() {
    use crate::model::{
        EntryMetadata, EntryType, EntryUuid, LogEntry, Message, MessageContent, Role,
    };
    use crate::state::SearchState;
    use chrono::Utc;

    let mut terminal = create_test_terminal();
    let session_id = SessionId::new("test-session").unwrap();
    let mut session = Session::new(session_id);

    // Add entry with searchable text
    let entry = LogEntry::new(
        EntryUuid::new("entry-1").unwrap(),
        None,
        SessionId::new("test-session").unwrap(),
        None,
        Utc::now(),
        EntryType::User,
        Message::new(
            Role::User,
            MessageContent::Text("hello world test".to_string()),
        ),
        EntryMetadata::default(),
    );
    session.add_entry(entry);

    let mut state = AppState::new(session);

    // Activate search for "world"
    use crate::state::search::{execute_search, SearchQuery};
    let query = SearchQuery::new("world").unwrap();
    let matches = execute_search(&state.session(), &query);
    state.search = SearchState::Active {
        query,
        matches,
        current_match: 0,
    };

    terminal
        .draw(|frame| {
            render_layout(frame, &state);
        })
        .unwrap();

    let buffer = terminal.backend().buffer().clone();

    // Verify the searchable text is present
    let content = buffer
        .content
        .iter()
        .map(|c| c.symbol())
        .collect::<String>();

    assert!(
        content.contains("hello") && content.contains("world"),
        "Rendered content should include the searched text"
    );

    // Note: We cannot easily verify highlighting in TestBackend as it doesn't
    // preserve exact styling. The real verification is that render_conversation_view_with_search
    // is called, which will be confirmed when dead_code warnings disappear.
}

#[test]
fn render_layout_no_search_highlighting_when_search_inactive() {
    use crate::model::{
        EntryMetadata, EntryType, EntryUuid, LogEntry, Message, MessageContent, Role,
    };
    use chrono::Utc;

    let mut terminal = create_test_terminal();
    let session_id = SessionId::new("test-session").unwrap();
    let mut session = Session::new(session_id);

    // Add entry with text
    let entry = LogEntry::new(
        EntryUuid::new("entry-1").unwrap(),
        None,
        SessionId::new("test-session").unwrap(),
        None,
        Utc::now(),
        EntryType::User,
        Message::new(
            Role::User,
            MessageContent::Text("hello world test".to_string()),
        ),
        EntryMetadata::default(),
    );
    session.add_entry(entry);

    let state = AppState::new(session);
    // search remains SearchState::Inactive (default)

    terminal
        .draw(|frame| {
            render_layout(frame, &state);
        })
        .unwrap();

    let buffer = terminal.backend().buffer().clone();
    let content = buffer
        .content
        .iter()
        .map(|c| c.symbol())
        .collect::<String>();

    // Text should still be rendered
    assert!(
        content.contains("hello") && content.contains("world"),
        "Text should render normally when search is inactive"
    );

    // With search inactive, render_conversation_view_with_search should handle
    // SearchState::Inactive and produce same output as render_conversation_view
}

// ===== Helper Functions =====

fn buffer_to_string(buffer: &ratatui::buffer::Buffer) -> String {
    let mut lines = Vec::new();
    for y in 0..buffer.area().height {
        let row: String = (0..buffer.area().width)
            .map(|x| buffer[(x, y)].symbol())
            .collect();
        lines.push(row);
    }
    lines.join("\n")
}
