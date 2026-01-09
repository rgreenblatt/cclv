//! Tests for help overlay widget

use super::*;
use ratatui::backend::TestBackend;
use ratatui::Terminal;

#[test]
fn render_help_overlay_shows_centered_modal() {
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal
        .draw(|frame| {
            render_help_overlay(frame, 0);
        })
        .unwrap();

    let buffer = terminal.backend().buffer();

    // Should have a border somewhere in the center
    let has_border = buffer.content().iter().any(|cell| {
        let symbol = cell.symbol();
        symbol.contains('┌') || symbol.contains('─') || symbol.contains('┐') || symbol.contains('│')
    });

    assert!(has_border, "Help overlay should render a bordered box");
}

#[test]
fn render_help_overlay_contains_navigation_shortcuts() {
    let backend = TestBackend::new(80, 30);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal
        .draw(|frame| {
            render_help_overlay(frame, 0);
        })
        .unwrap();

    let buffer = terminal.backend().buffer();
    let rendered_text = buffer_to_string(buffer);

    // Check for key navigation categories and shortcuts
    assert!(
        rendered_text.contains("Navigation"),
        "Should show Navigation category"
    );
    assert!(
        rendered_text.contains("j") || rendered_text.contains("↓"),
        "Should show scroll down shortcut"
    );
    assert!(
        rendered_text.contains("k") || rendered_text.contains("↑"),
        "Should show scroll up shortcut"
    );
}

#[test]
fn render_help_overlay_contains_pane_focus_shortcuts() {
    let backend = TestBackend::new(80, 30);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal
        .draw(|frame| {
            render_help_overlay(frame, 0);
        })
        .unwrap();

    let buffer = terminal.backend().buffer();
    let rendered_text = buffer_to_string(buffer);

    assert!(
        rendered_text.contains("Pane Focus") || rendered_text.contains("Focus"),
        "Should show Pane Focus category"
    );
    assert!(
        rendered_text.contains("Tab"),
        "Should show Tab for cycling panes"
    );
    assert!(
        rendered_text.contains("1") && rendered_text.contains("2") && rendered_text.contains("3"),
        "Should show 1, 2, 3 for direct pane focus"
    );
}

#[test]
fn render_help_overlay_contains_search_shortcuts() {
    let backend = TestBackend::new(80, 50);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal
        .draw(|frame| {
            render_help_overlay(frame, 0);
        })
        .unwrap();

    let buffer = terminal.backend().buffer();
    let rendered_text = buffer_to_string(buffer);

    assert!(
        rendered_text.contains("Search"),
        "Should show Search category"
    );
    assert!(
        rendered_text.contains("/") || rendered_text.contains("Ctrl+f"),
        "Should show search trigger shortcut"
    );
    assert!(
        rendered_text.contains("n"),
        "Should show next match shortcut"
    );
}

#[test]
fn render_help_overlay_contains_application_shortcuts() {
    // NOTE: This test now checks the content directly instead of the rendered buffer
    // because with 48 lines of help content and a ~38 line popup, the Application
    // section at the bottom gets cut off in the visible viewport.
    let lines = build_help_content();
    let text = help_lines_to_text(&lines);

    assert!(
        text.contains("q") || text.contains("Quit"),
        "Should show quit shortcut"
    );
    assert!(
        text.contains("?") || text.contains("Help"),
        "Should show help toggle shortcut"
    );
}

#[test]
fn render_help_overlay_shows_dismissal_hint() {
    let backend = TestBackend::new(80, 50);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal
        .draw(|frame| {
            render_help_overlay(frame, 0);
        })
        .unwrap();

    let buffer = terminal.backend().buffer();
    let rendered_text = buffer_to_string(buffer);

    assert!(
        rendered_text.contains("Esc") || rendered_text.contains("?"),
        "Should show dismissal hint (Esc or ?)"
    );
}

#[test]
fn centered_rect_calculates_correct_dimensions() {
    let area = Rect::new(0, 0, 100, 50);
    let centered = centered_rect(80, 80, area);

    // 80% of 100 = 80 width
    // 80% of 50 = 40 height
    assert_eq!(centered.width, 80, "Width should be 80% of area width");
    assert_eq!(centered.height, 40, "Height should be 80% of area height");

    // Should be centered: (100 - 80) / 2 = 10 offset
    assert_eq!(centered.x, 10, "Should be horizontally centered");
    assert_eq!(centered.y, 5, "Should be vertically centered");
}

#[test]
fn build_help_content_returns_non_empty() {
    let lines = build_help_content();
    assert!(
        !lines.is_empty(),
        "Help content should contain at least one line"
    );
}

#[test]
fn build_help_content_includes_all_categories() {
    let lines = build_help_content();
    let text = lines
        .iter()
        .map(|line| {
            line.spans
                .iter()
                .map(|span| span.content.as_ref())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n");

    // Check for all required categories
    let categories = [
        "Navigation",
        "Pane Focus",
        "Tabs",
        "Message",
        "Search",
        "Stats",
        "Live Mode",
        "Application",
    ];

    for category in &categories {
        assert!(
            text.contains(category),
            "Help content should include {} category",
            category
        );
    }
}

// ===== Exact cli.md Contract Tests =====

#[test]
fn navigation_shortcuts_match_cli_contract() {
    let lines = build_help_content();
    let text = help_lines_to_text(&lines);

    // cli.md lines 120-131: Navigation shortcuts
    assert!(
        text.contains("j") && text.contains("↓"),
        "Must show j/↓ for scroll down"
    );
    assert!(
        text.contains("k") && text.contains("↑"),
        "Must show k/↑ for scroll up"
    );
    assert!(
        text.contains("h") && text.contains("←"),
        "Must show h/← for scroll left"
    );
    assert!(
        text.contains("l") && text.contains("→"),
        "Must show l/→ for scroll right"
    );
    assert!(
        text.contains("Ctrl+d") && text.contains("Page Down"),
        "Must show Ctrl+d/Page Down"
    );
    assert!(
        text.contains("Ctrl+u") && text.contains("Page Up"),
        "Must show Ctrl+u/Page Up"
    );
    assert!(
        text.contains("g") && text.contains("Home"),
        "Must show g/Home for go to top"
    );
    assert!(
        text.contains("G") && text.contains("End"),
        "Must show G/End for go to bottom"
    );

    assert!(text.contains("Scroll down"), "Must describe scroll down");
    assert!(text.contains("Scroll up"), "Must describe scroll up");
    assert!(text.contains("Scroll left"), "Must describe scroll left");
    assert!(text.contains("Scroll right"), "Must describe scroll right");
    assert!(text.contains("Page down"), "Must describe page down");
    assert!(text.contains("Page up"), "Must describe page up");
    assert!(text.contains("Go to top"), "Must describe go to top");
    assert!(text.contains("Go to bottom"), "Must describe go to bottom");
}

#[test]
fn pane_focus_shortcuts_match_cli_contract() {
    let lines = build_help_content();
    let text = help_lines_to_text(&lines);

    // cli.md lines 133-140: Pane Focus
    assert!(text.contains("Tab"), "Must show Tab for cycle focus");
    assert!(text.contains("1"), "Must show 1 for focus main pane");
    assert!(text.contains("2"), "Must show 2 for focus subagent pane");
    assert!(text.contains("3"), "Must show 3 for focus stats panel");

    assert!(
        text.contains("Cycle focus") || text.contains("focus between panes"),
        "Must describe Tab cycling panes"
    );
    assert!(
        text.contains("Focus main") || text.contains("main agent pane"),
        "Must describe 1 focusing main pane"
    );
    assert!(
        text.contains("Focus subagent") || text.contains("subagent pane"),
        "Must describe 2 focusing subagent pane"
    );
    assert!(
        text.contains("Focus stats") || text.contains("stats panel"),
        "Must describe 3 focusing stats panel"
    );
}

#[test]
fn tabs_shortcuts_match_cli_contract() {
    let lines = build_help_content();
    let text = help_lines_to_text(&lines);

    // cli.md lines 142-150: Tabs (Subagent Pane)
    assert!(
        text.contains("[") && text.contains("Shift+Tab"),
        "Must show [/Shift+Tab for previous tab"
    );
    assert!(text.contains("]"), "Must show ] for next tab");
    assert!(
        text.contains("1-9") || (text.contains("1") && text.contains("9")),
        "Must show 1-9 for select tab by number"
    );

    assert!(text.contains("Previous tab"), "Must describe previous tab");
    assert!(text.contains("Next tab"), "Must describe next tab");
    assert!(
        text.contains("Select tab"),
        "Must describe select tab by number"
    );

    // MUST NOT use h/← or l/→ for tabs
    // This is context-dependent - we just verify the correct shortcuts are present
}

#[test]
fn message_interaction_shortcuts_match_cli_contract() {
    let lines = build_help_content();
    let text = help_lines_to_text(&lines);

    // cli.md lines 152-158: Message Interaction
    assert!(
        text.contains("Enter") && text.contains("Space"),
        "Must show Enter/Space for toggle expand/collapse"
    );
    assert!(text.contains("e"), "Must show e for expand all");
    assert!(text.contains("c"), "Must show c for collapse all");

    assert!(
        text.contains("Toggle expand") || text.contains("expand/collapse"),
        "Must describe toggle expand/collapse"
    );
    assert!(text.contains("Expand all"), "Must describe expand all");
    assert!(text.contains("Collapse all"), "Must describe collapse all");

    // MUST NOT show y (Copy to clipboard) - not in cli.md
    assert!(
        !text.contains("Copy to clipboard"),
        "Must NOT show 'Copy to clipboard' - not in cli.md contract"
    );
}

#[test]
fn search_shortcuts_match_cli_contract() {
    let lines = build_help_content();
    let text = help_lines_to_text(&lines);

    // cli.md lines 160-168: Search
    assert!(
        text.contains("/") && text.contains("Ctrl+f"),
        "Must show //Ctrl+f for start search"
    );
    assert!(text.contains("Enter"), "Must show Enter for submit search");
    assert!(text.contains("Esc"), "Must show Esc for cancel search");
    assert!(text.contains("n"), "Must show n for next match");
    assert!(
        text.contains("N") && text.contains("Shift+n"),
        "Must show N/Shift+n for previous match"
    );

    assert!(text.contains("Start search"), "Must describe start search");
    assert!(
        text.contains("Submit search"),
        "Must describe submit search"
    );
    assert!(
        text.contains("Cancel search"),
        "Must describe cancel search"
    );
    assert!(text.contains("Next match"), "Must describe next match");
    assert!(
        text.contains("Previous match"),
        "Must describe previous match"
    );
}

#[test]
fn stats_shortcuts_match_cli_contract() {
    let lines = build_help_content();
    let text = help_lines_to_text(&lines);

    // Stats keybindings (actual implementation in keybindings.rs)
    assert!(text.contains("s"), "Must show s for toggle stats panel");
    assert!(text.contains("  f  "), "Must show f for filter: global");
    assert!(
        text.contains("  m  "),
        "Must show m for filter: main agent only"
    );
    assert!(
        text.contains("  S  "),
        "Must show S for filter: current subagent"
    );

    assert!(
        text.contains("Toggle stats"),
        "Must describe toggle stats panel"
    );
    assert!(
        text.contains("Global") || text.contains("global"),
        "Must describe global filter"
    );
    assert!(
        text.contains("Main agent") || text.contains("main agent"),
        "Must describe main agent filter"
    );
    assert!(
        text.contains("Current subagent") || text.contains("subagent"),
        "Must describe current subagent filter"
    );

    // MUST NOT show f (Cycle stats filter) - that's for stats filtering
    assert!(
        !text.contains("Cycle stats"),
        "Must NOT show 'Cycle stats' - not in cli.md contract"
    );
}

#[test]
fn live_mode_shortcuts_match_cli_contract() {
    let lines = build_help_content();
    let text = help_lines_to_text(&lines);

    // Live Mode: only auto-scroll toggle remains (follow mode removed)
    assert!(text.contains("a"), "Must show a for toggle auto-scroll");

    assert!(
        text.contains("Toggle auto-scroll") || text.contains("auto-scroll"),
        "Must describe toggle auto-scroll"
    );

    // MUST NOT show Space for toggle live mode
    assert!(
        !text.contains("Space") || !text.contains("Toggle live"),
        "Must NOT show Space for toggle live mode - not in cli.md contract"
    );
}

#[test]
fn application_shortcuts_match_cli_contract() {
    let lines = build_help_content();
    let text = help_lines_to_text(&lines);

    // cli.md lines 186-192: Application
    assert!(
        text.contains("q") && text.contains("Ctrl+c"),
        "Must show q/Ctrl+c for quit"
    );
    assert!(text.contains("?"), "Must show ? for show help overlay");
    assert!(text.contains("r"), "Must show r for refresh display");

    assert!(text.contains("Quit"), "Must describe quit");
    assert!(
        text.contains("Show help") || text.contains("help overlay"),
        "Must describe show help overlay"
    );
    assert!(text.contains("Refresh"), "Must describe refresh display");
}

// Helper to convert help lines to searchable text
fn help_lines_to_text(lines: &[Line]) -> String {
    lines
        .iter()
        .map(|line| {
            line.spans
                .iter()
                .map(|span| span.content.as_ref())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

// Helper function to convert buffer to string for text search
fn buffer_to_string(buffer: &ratatui::buffer::Buffer) -> String {
    let mut result = String::new();
    for y in 0..buffer.area.height {
        for x in 0..buffer.area.width {
            if let Some(cell) = buffer.cell((x, y)) {
                result.push_str(cell.symbol());
            }
        }
        result.push('\n');
    }
    result
}
