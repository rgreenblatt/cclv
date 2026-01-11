//! Search input widget for rendering the search bar.

use crate::state::SearchState;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Widget},
};

/// Search input widget.
/// Renders the search bar when in Typing state.
pub struct SearchInput<'a> {
    search_state: &'a SearchState,
}

impl<'a> SearchInput<'a> {
    /// Create new SearchInput widget.
    pub fn new(search_state: &'a SearchState) -> Self {
        Self { search_state }
    }
}

impl Widget for SearchInput<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        match self.search_state {
            SearchState::Typing { query, cursor } => {
                // Split query into before/after cursor for rendering cursor
                let before = query.chars().take(*cursor).collect::<String>();
                let after_chars: Vec<char> = query.chars().skip(*cursor).collect();

                // Create spans with cursor indicator
                let (cursor_char, after_text) = if *cursor == query.len() {
                    (" ".to_string(), String::new())
                } else {
                    let cursor_ch = after_chars
                        .first()
                        .map(|c| c.to_string())
                        .unwrap_or_default();
                    let remaining: String = after_chars.iter().skip(1).collect();
                    (cursor_ch, remaining)
                };

                let spans = vec![
                    Span::raw(before),
                    Span::styled(
                        cursor_char,
                        Style::default()
                            .bg(Color::White)
                            .fg(Color::Black)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(after_text),
                ];

                let line = Line::from(spans);
                let paragraph = Paragraph::new(line).block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title("Search")
                        .style(Style::default().bg(Color::DarkGray)),
                );

                paragraph.render(area, buf);
            }
            SearchState::Active { query, .. } => {
                // Show active search (read-only)
                let paragraph = Paragraph::new(Line::from(query.as_str())).block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title("Search (active)")
                        .style(Style::default().bg(Color::Blue)),
                );

                paragraph.render(area, buf);
            }
            SearchState::Inactive => {
                // No search input to show
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    #[test]
    fn search_input_renders_typing_state() {
        let mut terminal = Terminal::new(TestBackend::new(40, 3)).unwrap();

        let state = SearchState::Typing {
            query: "test".to_string(),
            cursor: 2,
        };

        terminal
            .draw(|frame| {
                let widget = SearchInput::new(&state);
                frame.render_widget(widget, frame.area());
            })
            .unwrap();

        // Just verify it doesn't panic - visual verification is manual
    }

    #[test]
    fn search_input_renders_active_state() {
        let mut terminal = Terminal::new(TestBackend::new(40, 3)).unwrap();

        let query = crate::state::SearchQuery::new("active query").unwrap();
        let state = SearchState::Active {
            query,
            matches: vec![],
            current_match: 0,
        };

        terminal
            .draw(|frame| {
                let widget = SearchInput::new(&state);
                frame.render_widget(widget, frame.area());
            })
            .unwrap();

        // Just verify it doesn't panic
    }

    #[test]
    fn search_input_inactive_renders_nothing() {
        let mut terminal = Terminal::new(TestBackend::new(40, 3)).unwrap();

        let state = SearchState::Inactive;

        terminal
            .draw(|frame| {
                let widget = SearchInput::new(&state);
                frame.render_widget(widget, frame.area());
            })
            .unwrap();

        // Should render without panic even when Inactive
    }
}
