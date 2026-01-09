//! Session list modal rendering.

use ratatui::prelude::*;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState};

use crate::state::{AppState, ViewedSession};
use crate::view_state::session_summary::SessionSummary;

/// Render the session list modal overlay.
///
/// Displays a centered modal with:
/// - Session list with current session marked
/// - Selected row highlighted
/// - Footer with keybinding hints
///
/// Only renders when `state.session_modal.is_visible()` is true.
///
/// # Layout
/// - 60 columns wide, centered horizontally
/// - Height adapts to session count
/// - Clears background before rendering for overlay effect
///
/// # FR-002: Session list modal accessible via keyboard
/// # FR-003: Select session from modal using keyboard navigation
/// # FR-005: Visually indicate currently active session
pub fn render_session_modal(frame: &mut Frame, state: &AppState) {
    // Early return if modal is not visible
    if !state.session_modal.is_visible() {
        return;
    }

    let area = frame.area();
    let session_count = state.log_view().session_count();
    let modal_area = centered_rect(60, session_count, area);

    // Clear the background for overlay effect
    frame.render_widget(Clear, modal_area);

    // Collect session summaries using from_session factory
    let sessions: Vec<SessionSummary> = state
        .log_view()
        .sessions()
        .enumerate()
        .map(|(i, session_view)| {
            let index =
                crate::view_state::types::SessionIndex::new(i, state.log_view().session_count())
                    .expect("Index should be valid");

            SessionSummary::from_session(index, session_view)
        })
        .collect();
    let session_count = sessions.len();

    // Determine which session is current
    let current_index = match state.viewed_session {
        ViewedSession::Latest => session_count.saturating_sub(1),
        ViewedSession::Pinned(idx) => idx.get(),
    };

    // Build list items
    let items: Vec<ListItem> = sessions
        .iter()
        .enumerate()
        .map(|(i, summary)| {
            let mut spans = Vec::new();

            // Add selection prefix
            if i == state.session_modal.selected_index() {
                spans.push(Span::raw("> "));
            } else {
                spans.push(Span::raw("  "));
            }

            // Add session info
            spans.push(Span::raw(summary.display_line()));

            // Add [CURRENT] marker if this is the current session
            // Per contract line 110: Current marker | Yellow, italic
            if i == current_index {
                spans.push(Span::raw("  "));
                spans.push(Span::styled(
                    "[CURRENT]",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::ITALIC),
                ));
            }

            ListItem::new(Line::from(spans))
        })
        .collect();

    // Calculate scroll indicators
    // Per contract lines 116-133: Show ▲/▼ when content extends beyond view
    let visible_rows = modal_area
        .height
        .saturating_sub(4)
        .max(1);
    let needs_scrolling = session_count > visible_rows as usize;
    let selected = state.session_modal.selected_index();

    // Determine if we're scrolled (not showing first item)
    let scroll_offset = if needs_scrolling {
        // ratatui List widget centers selection, so estimate scroll offset
        let half_visible = visible_rows / 2;
        if selected < half_visible as usize {
            0
        } else if selected >= session_count.saturating_sub(half_visible as usize) {
            session_count.saturating_sub(visible_rows as usize)
        } else {
            selected.saturating_sub(half_visible as usize)
        }
    } else {
        0
    };

    let show_up_arrow = needs_scrolling && scroll_offset > 0;
    let show_down_arrow = needs_scrolling && (scroll_offset + visible_rows as usize) < session_count;

    // Build title with optional scroll indicator
    let title_text = if show_up_arrow {
        " Session List                        ▲   "
    } else {
        " Session List "
    };

    // Create the list widget
    let list = List::new(items)
        .block(
            Block::default()
                .title(
                    ratatui::text::Line::from(vec![ratatui::text::Span::styled(
                        title_text,
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    )])
                    .alignment(ratatui::layout::Alignment::Center),
                )
                .borders(Borders::ALL)
                .border_type(ratatui::widgets::BorderType::Rounded)
                .border_style(Style::default().fg(Color::White))
                .style(Style::default().bg(Color::DarkGray)),
        )
        .highlight_style(
            Style::default()
                .bg(Color::Cyan)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD),
        );

    // Create list state for selection
    let mut list_state =
        ListState::default().with_selected(Some(state.session_modal.selected_index()));

    // Render the list
    frame.render_stateful_widget(list, modal_area, &mut list_state);

    // Render footer with keybinding hints and optional scroll indicator
    let footer_area = Rect {
        x: modal_area.x + 1,
        y: modal_area.y + modal_area.height.saturating_sub(2),
        width: modal_area.width.saturating_sub(2),
        height: 1,
    };

    let footer_text = if show_down_arrow {
        "↑/↓: Navigate  Enter: Select  Esc: Cancel               ▼   "
    } else {
        "↑/↓: Navigate  Enter: Select  Esc: Cancel  S: Close"
    };

    let footer = ratatui::widgets::Paragraph::new(footer_text)
        .style(Style::default().fg(Color::Gray).add_modifier(Modifier::DIM))
        .alignment(ratatui::layout::Alignment::Center);

    frame.render_widget(footer, footer_area);
}

/// Calculate centered rect with fixed width.
///
/// Returns a Rect that is centered horizontally with the specified width.
/// Height is calculated per contract: min(session_count + 4, terminal_height - 4)
fn centered_rect(width_cols: u16, session_count: usize, area: Rect) -> Rect {
    // Fixed width of 60 columns
    let popup_width = width_cols.min(area.width);

    // Contract line 30: Height = min(session_count + 4, terminal_height - 4)
    let popup_height = (session_count as u16 + 4).min(area.height.saturating_sub(4));

    let popup_x = (area.width.saturating_sub(popup_width)) / 2;
    let popup_y = (area.height.saturating_sub(popup_height)) / 2;

    Rect {
        x: area.x + popup_x,
        y: area.y + popup_y,
        width: popup_width,
        height: popup_height,
    }
}

#[cfg(test)]
#[path = "session_modal_tests.rs"]
mod tests;
