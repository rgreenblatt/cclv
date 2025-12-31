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
            render_help_overlay(frame);
        })
        .unwrap();

    let buffer = terminal.backend().buffer();

    // Should have a border somewhere in the center
    let has_border = buffer
        .content()
        .iter()
        .any(|cell| {
            let symbol = cell.symbol();
            symbol.contains('┌')
                || symbol.contains('─')
                || symbol.contains('┐')
                || symbol.contains('│')
        });

    assert!(
        has_border,
        "Help overlay should render a bordered box"
    );
}

#[test]
fn render_help_overlay_contains_navigation_shortcuts() {
    let backend = TestBackend::new(80, 30);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal
        .draw(|frame| {
            render_help_overlay(frame);
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
            render_help_overlay(frame);
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
            render_help_overlay(frame);
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
    let backend = TestBackend::new(80, 50);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal
        .draw(|frame| {
            render_help_overlay(frame);
        })
        .unwrap();

    let buffer = terminal.backend().buffer();
    let rendered_text = buffer_to_string(buffer);

    assert!(
        rendered_text.contains("q") || rendered_text.contains("Quit"),
        "Should show quit shortcut"
    );
    assert!(
        rendered_text.contains("?") || rendered_text.contains("Help"),
        "Should show help toggle shortcut"
    );
}

#[test]
fn render_help_overlay_shows_dismissal_hint() {
    let backend = TestBackend::new(80, 50);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal
        .draw(|frame| {
            render_help_overlay(frame);
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
