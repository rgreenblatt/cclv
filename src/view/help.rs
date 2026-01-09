//! Help overlay widget displaying keyboard shortcuts.
//!
//! Shows a centered modal overlay with all keyboard shortcuts grouped by category.
//! Triggered by '?' key, dismissed by 'Esc' or '?'.

use super::constants::{HELP_POPUP_HEIGHT_PERCENT, HELP_POPUP_WIDTH_PERCENT};
use super::helpers::empty_line;
use ratatui::{
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};

/// Render the help overlay centered on the screen.
///
/// The overlay displays all keyboard shortcuts grouped by category:
/// - Navigation
/// - Pane Focus
/// - Tabs (Subagent Pane)
/// - Message Interaction
/// - Search
/// - Stats
/// - Live Mode
/// - Application
///
/// The overlay is centered on the screen with a border and dismissal hint.
/// The scroll_offset parameter controls which line is shown at the top (cclv-5ur.76).
pub fn render_help_overlay(frame: &mut Frame, scroll_offset: u16) {
    let area = frame.area();
    let popup_area = centered_rect(HELP_POPUP_WIDTH_PERCENT, HELP_POPUP_HEIGHT_PERCENT, area);

    // Clear the background for the overlay
    frame.render_widget(Clear, popup_area);

    // Build help content
    let help_content = build_help_content();

    // Create the help paragraph
    let help_paragraph = Paragraph::new(help_content)
        .block(
            Block::default()
                .title(" Keyboard Shortcuts ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        )
        .wrap(Wrap { trim: false })
        .alignment(Alignment::Left)
        .scroll((scroll_offset, 0)); // Scroll vertically (cclv-5ur.76)

    frame.render_widget(help_paragraph, popup_area);

    // Render dismissal hint at the bottom
    let hint_area = Rect {
        x: popup_area.x,
        y: popup_area.y + popup_area.height.saturating_sub(1),
        width: popup_area.width,
        height: 1,
    };

    let hint = Paragraph::new(Line::from(vec![Span::styled(
        " Press Esc or ? to close ",
        super::styles::MUTED_TEXT.add_modifier(Modifier::DIM),
    )]))
    .alignment(Alignment::Center);

    frame.render_widget(hint, hint_area);
}

/// Calculate the centered rect for the help overlay.
///
/// Returns a Rect that is centered on the screen with the specified
/// percentage of width and height.
fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let popup_width = area.width * percent_x / 100;
    let popup_height = area.height * percent_y / 100;
    let popup_x = (area.width.saturating_sub(popup_width)) / 2;
    let popup_y = (area.height.saturating_sub(popup_height)) / 2;

    Rect {
        x: area.x + popup_x,
        y: area.y + popup_y,
        width: popup_width,
        height: popup_height,
    }
}

/// Build the help content lines grouped by category.
///
/// Returns a Vec of Line representing all shortcuts with category headers.
fn build_help_content() -> Vec<Line<'static>> {
    let category_style = super::styles::SECTION_HEADER;
    let key_style = Style::default()
        .fg(Color::Yellow)
        .add_modifier(Modifier::BOLD);
    let desc_style = Style::default().fg(Color::White);

    vec![
        // Navigation (cli.md lines 120-131)
        Line::from(vec![Span::styled("Navigation", category_style)]),
        Line::from(vec![
            Span::styled("  j/↓         ", key_style),
            Span::styled("Scroll down", desc_style),
        ]),
        Line::from(vec![
            Span::styled("  k/↑         ", key_style),
            Span::styled("Scroll up", desc_style),
        ]),
        Line::from(vec![
            Span::styled("  h/←         ", key_style),
            Span::styled("Scroll left (for long lines)", desc_style),
        ]),
        Line::from(vec![
            Span::styled("  l/→         ", key_style),
            Span::styled("Scroll right", desc_style),
        ]),
        Line::from(vec![
            Span::styled("  Ctrl+d/Page Down ", key_style),
            Span::styled("Page down", desc_style),
        ]),
        Line::from(vec![
            Span::styled("  Ctrl+u/Page Up   ", key_style),
            Span::styled("Page up", desc_style),
        ]),
        Line::from(vec![
            Span::styled("  g/Home      ", key_style),
            Span::styled("Go to top", desc_style),
        ]),
        Line::from(vec![
            Span::styled("  G/End       ", key_style),
            Span::styled("Go to bottom", desc_style),
        ]),
        empty_line(),
        // Pane Focus (cli.md lines 133-140)
        Line::from(vec![Span::styled("Pane Focus", category_style)]),
        Line::from(vec![
            Span::styled("  Tab         ", key_style),
            Span::styled("Cycle focus between panes", desc_style),
        ]),
        Line::from(vec![
            Span::styled("  1           ", key_style),
            Span::styled("Focus main agent pane", desc_style),
        ]),
        Line::from(vec![
            Span::styled("  2           ", key_style),
            Span::styled("Focus subagent pane", desc_style),
        ]),
        Line::from(vec![
            Span::styled("  3           ", key_style),
            Span::styled("Focus stats panel", desc_style),
        ]),
        empty_line(),
        // Tabs (Subagent Pane) (cli.md lines 142-150)
        Line::from(vec![Span::styled("Tabs (Subagent Pane)", category_style)]),
        Line::from(vec![
            Span::styled("  [/Shift+Tab ", key_style),
            Span::styled("Previous tab", desc_style),
        ]),
        Line::from(vec![
            Span::styled("  ]           ", key_style),
            Span::styled("Next tab", desc_style),
        ]),
        Line::from(vec![
            Span::styled("  1-9         ", key_style),
            Span::styled("Select tab by number", desc_style),
        ]),
        empty_line(),
        // Message Interaction (cli.md lines 152-158)
        Line::from(vec![Span::styled("Message Interaction", category_style)]),
        Line::from(vec![
            Span::styled("  Enter/Space ", key_style),
            Span::styled("Toggle expand/collapse message", desc_style),
        ]),
        Line::from(vec![
            Span::styled("  e           ", key_style),
            Span::styled("Expand all messages", desc_style),
        ]),
        Line::from(vec![
            Span::styled("  c           ", key_style),
            Span::styled("Collapse all messages", desc_style),
        ]),
        empty_line(),
        // Search (cli.md lines 160-168)
        Line::from(vec![Span::styled("Search", category_style)]),
        Line::from(vec![
            Span::styled("  //Ctrl+f    ", key_style),
            Span::styled("Start search", desc_style),
        ]),
        Line::from(vec![
            Span::styled("  Ctrl+s      ", key_style),
            Span::styled("Submit search", desc_style),
        ]),
        Line::from(vec![
            Span::styled("  Esc         ", key_style),
            Span::styled("Cancel search", desc_style),
        ]),
        Line::from(vec![
            Span::styled("  n           ", key_style),
            Span::styled("Next match", desc_style),
        ]),
        Line::from(vec![
            Span::styled("  N/Shift+n   ", key_style),
            Span::styled("Previous match", desc_style),
        ]),
        empty_line(),
        // Stats (cli.md lines 170-177)
        Line::from(vec![Span::styled("Stats", category_style)]),
        Line::from(vec![
            Span::styled("  s           ", key_style),
            Span::styled("Toggle stats panel", desc_style),
        ]),
        Line::from(vec![
            Span::styled("  f           ", key_style),
            Span::styled("Filter: Global", desc_style),
        ]),
        Line::from(vec![
            Span::styled("  m           ", key_style),
            Span::styled("Filter: Main agent only", desc_style),
        ]),
        Line::from(vec![
            Span::styled("  S           ", key_style),
            Span::styled("Filter: Current subagent", desc_style),
        ]),
        empty_line(),
        // Live Mode
        Line::from(vec![Span::styled("Live Mode", category_style)]),
        Line::from(vec![
            Span::styled("  a           ", key_style),
            Span::styled("Toggle auto-scroll", desc_style),
        ]),
        empty_line(),
        // Application (cli.md lines 186-192)
        Line::from(vec![Span::styled("Application", category_style)]),
        Line::from(vec![
            Span::styled("  q/Ctrl+c    ", key_style),
            Span::styled("Quit", desc_style),
        ]),
        Line::from(vec![
            Span::styled("  ?           ", key_style),
            Span::styled("Show help overlay", desc_style),
        ]),
        Line::from(vec![
            Span::styled("  r           ", key_style),
            Span::styled("Refresh display", desc_style),
        ]),
    ]
}

// ===== Tests =====

#[cfg(test)]
#[path = "help_tests.rs"]
mod tests;
