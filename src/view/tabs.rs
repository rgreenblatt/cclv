//! Subagent tab bar widget.
//!
//! Displays tabs for each subagent using ratatui's Tabs widget.
//! Selection state is managed by AppState.selected_tab.

use crate::model::AgentId;
use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::Line,
    widgets::{Block, Borders, Tabs},
    Frame,
};
use std::collections::HashSet;

/// Render the subagent tab bar.
///
/// # Arguments
/// * `frame` - The ratatui frame to render into
/// * `area` - The area to render the tab bar within
/// * `agent_ids` - Ordered list of subagent IDs (from Session::subagent_ids_ordered)
/// * `selected_tab` - Index of selected tab (None means no selection)
/// * `tabs_with_matches` - Set of agent IDs that contain search matches (empty set if no search active)
///
/// # Behavior
/// - Shows one tab per subagent with agent ID as label
/// - Highlights the selected tab if Some(index) and index is in bounds
/// - Supports deselection via None (no highlight)
/// - Out-of-bounds indices are treated as None
/// - Agent IDs may be truncated if they exceed available space
/// - Tabs with search matches display a visual indicator (•)
pub fn render_tab_bar(
    frame: &mut Frame,
    area: Rect,
    agent_ids: &[&AgentId],
    selected_tab: Option<usize>,
    tabs_with_matches: &HashSet<AgentId>,
) {
    // Convert agent IDs to tab titles with match indicators
    let titles: Vec<Line> = agent_ids
        .iter()
        .map(|id| {
            let label = if tabs_with_matches.contains(*id) {
                format!("{} •", id.as_str())
            } else {
                id.as_str().to_string()
            };
            Line::from(label)
        })
        .collect();

    // Validate bounds: treat out-of-bounds as None
    let validated_selection = selected_tab.filter(|&idx| idx < agent_ids.len());

    // Create Tabs widget with block
    let mut tabs = Tabs::new(titles)
        .block(Block::default().borders(Borders::ALL).title("Subagents"))
        .style(Style::default().fg(Color::White));

    // Apply highlight only if we have a valid selection
    // ratatui's Tabs widget doesn't support "no selection", so we work around it:
    // - With selection: set highlight_style and select
    // - Without selection: omit highlight_style (tabs render without highlight)
    if let Some(idx) = validated_selection {
        tabs = tabs
            .highlight_style(Style::default().fg(Color::Yellow))
            .select(idx);
    }

    // Render the tabs widget
    frame.render_widget(tabs, area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::AgentId;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
    use std::collections::HashSet;

    // Test helper: Create test terminal
    fn create_test_terminal() -> Terminal<TestBackend> {
        let backend = TestBackend::new(80, 24);
        Terminal::new(backend).unwrap()
    }

    // Test helper: Create agent ID
    fn agent_id(s: &str) -> AgentId {
        AgentId::new(s).unwrap()
    }

    // Test helper: Empty match set (no search active)
    fn no_matches() -> HashSet<AgentId> {
        HashSet::new()
    }

    #[test]
    fn render_tab_bar_displays_single_tab() {
        let mut terminal = create_test_terminal();
        let agent1 = agent_id("agent-abc");
        let agent_ids = vec![&agent1];

        terminal
            .draw(|frame| {
                render_tab_bar(frame, frame.area(), &agent_ids, Some(0), &no_matches());
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let buffer_str = buffer
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect::<String>();

        // Should contain the agent ID somewhere in the buffer
        assert!(
            buffer_str.contains("agent-abc"),
            "Tab bar should display agent ID 'agent-abc'"
        );
    }

    #[test]
    fn render_tab_bar_displays_multiple_tabs() {
        let mut terminal = create_test_terminal();
        let agent1 = agent_id("agent-1");
        let agent2 = agent_id("agent-2");
        let agent3 = agent_id("agent-3");
        let agent_ids = vec![&agent1, &agent2, &agent3];

        terminal
            .draw(|frame| {
                render_tab_bar(frame, frame.area(), &agent_ids, Some(1), &no_matches());
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let buffer_str = buffer
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect::<String>();

        // All three agent IDs should be present
        assert!(buffer_str.contains("agent-1"), "Should contain agent-1");
        assert!(buffer_str.contains("agent-2"), "Should contain agent-2");
        assert!(buffer_str.contains("agent-3"), "Should contain agent-3");
    }

    #[test]
    fn render_tab_bar_handles_no_selection() {
        let mut terminal = create_test_terminal();
        let agent1 = agent_id("agent-xyz");
        let agent_ids = vec![&agent1];

        // Should not panic with None selection
        let result = terminal.draw(|frame| {
            render_tab_bar(frame, frame.area(), &agent_ids, None, &no_matches());
        });

        assert!(
            result.is_ok(),
            "Should render without error when selection is None"
        );
    }

    #[test]
    fn render_tab_bar_handles_empty_agent_list() {
        let mut terminal = create_test_terminal();
        let agent_ids: Vec<&AgentId> = vec![];

        // Should not panic with empty list
        let result = terminal.draw(|frame| {
            render_tab_bar(frame, frame.area(), &agent_ids, None, &no_matches());
        });

        assert!(
            result.is_ok(),
            "Should render without error when agent list is empty"
        );
    }

    #[test]
    fn render_tab_bar_selection_within_bounds() {
        let mut terminal = create_test_terminal();
        let agent1 = agent_id("agent-1");
        let agent2 = agent_id("agent-2");
        let agent_ids = vec![&agent1, &agent2];

        // Selecting index 1 (agent-2) should work
        let result = terminal.draw(|frame| {
            render_tab_bar(frame, frame.area(), &agent_ids, Some(1), &no_matches());
        });

        assert!(result.is_ok(), "Should render with valid selection index");
    }

    #[test]
    fn render_tab_bar_uses_agent_id_as_label() {
        let mut terminal = create_test_terminal();
        let agent1 = agent_id("subagent-12345");
        let agent_ids = vec![&agent1];

        terminal
            .draw(|frame| {
                render_tab_bar(frame, frame.area(), &agent_ids, Some(0), &no_matches());
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let buffer_str = buffer
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect::<String>();

        // The full agent ID should be used as the tab label
        assert!(
            buffer_str.contains("subagent-12345"),
            "Tab should use agent ID as label"
        );
    }

    #[test]
    fn render_tab_bar_handles_out_of_bounds_selection() {
        let mut terminal = create_test_terminal();
        let agent1 = agent_id("agent-1");
        let agent2 = agent_id("agent-2");
        let agent_ids = vec![&agent1, &agent2];

        // Selecting index 5 when only 2 agents exist should be treated as None
        let result = terminal.draw(|frame| {
            render_tab_bar(frame, frame.area(), &agent_ids, Some(5), &no_matches());
        });

        assert!(
            result.is_ok(),
            "Should handle out-of-bounds selection gracefully"
        );
    }

    #[test]
    fn render_tab_bar_none_differs_visually_from_some_zero() {
        use ratatui::style::Color;

        let agent1 = agent_id("agent-1");
        let agent2 = agent_id("agent-2");
        let agent_ids = vec![&agent1, &agent2];

        // Render with None selection
        let mut terminal_none = create_test_terminal();
        terminal_none
            .draw(|frame| {
                render_tab_bar(frame, frame.area(), &agent_ids, None, &no_matches());
            })
            .unwrap();
        let buffer_none = terminal_none.backend().buffer().clone();

        // Render with Some(0) selection
        let mut terminal_some = create_test_terminal();
        terminal_some
            .draw(|frame| {
                render_tab_bar(frame, frame.area(), &agent_ids, Some(0), &no_matches());
            })
            .unwrap();
        let buffer_some = terminal_some.backend().buffer().clone();

        // The two buffers should differ - None should not highlight any tab,
        // while Some(0) should highlight the first tab
        let none_has_yellow = buffer_none
            .content()
            .iter()
            .any(|cell| cell.fg == Color::Yellow);
        let some_has_yellow = buffer_some
            .content()
            .iter()
            .any(|cell| cell.fg == Color::Yellow);

        assert!(
            !none_has_yellow,
            "None selection should not highlight any tab (no yellow)"
        );
        assert!(
            some_has_yellow,
            "Some(0) selection should highlight first tab (has yellow)"
        );
    }

    // ===== Match Indicator Tests =====

    #[test]
    fn render_tab_bar_shows_indicator_on_tab_with_matches() {
        let mut terminal = create_test_terminal();
        let agent1 = agent_id("agent-1");
        let agent2 = agent_id("agent-2");
        let agent_ids = vec![&agent1, &agent2];

        // Create match set with agent-1 having matches
        let mut matches = HashSet::new();
        matches.insert(agent_id("agent-1"));

        terminal
            .draw(|frame| {
                render_tab_bar(frame, frame.area(), &agent_ids, None, &matches);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let buffer_str = buffer
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect::<String>();

        // Tab with matches should show indicator (•)
        assert!(
            buffer_str.contains("•"),
            "Tab with matches should display indicator (•)"
        );
    }

    #[test]
    fn render_tab_bar_no_indicator_without_matches() {
        let mut terminal = create_test_terminal();
        let agent1 = agent_id("agent-1");
        let agent2 = agent_id("agent-2");
        let agent_ids = vec![&agent1, &agent2];

        // Empty match set - no matches
        let matches = HashSet::new();

        terminal
            .draw(|frame| {
                render_tab_bar(frame, frame.area(), &agent_ids, None, &matches);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let buffer_str = buffer
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect::<String>();

        // No indicator should be shown
        assert!(
            !buffer_str.contains("•"),
            "Tabs without matches should not display indicator"
        );
    }

    #[test]
    fn render_tab_bar_multiple_tabs_with_matches() {
        let mut terminal = create_test_terminal();
        let agent1 = agent_id("agent-1");
        let agent2 = agent_id("agent-2");
        let agent3 = agent_id("agent-3");
        let agent_ids = vec![&agent1, &agent2, &agent3];

        // Both agent-1 and agent-3 have matches
        let mut matches = HashSet::new();
        matches.insert(agent_id("agent-1"));
        matches.insert(agent_id("agent-3"));

        terminal
            .draw(|frame| {
                render_tab_bar(frame, frame.area(), &agent_ids, None, &matches);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let buffer_str = buffer
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect::<String>();

        // Should have multiple indicators (at least 2 • symbols)
        let indicator_count = buffer_str.matches('•').count();
        assert!(
            indicator_count >= 2,
            "Should have at least 2 indicators for 2 tabs with matches, found: {}",
            indicator_count
        );
    }

    #[test]
    fn render_tab_bar_indicator_only_on_matching_tab() {
        let mut terminal = create_test_terminal();
        let agent1 = agent_id("agent-xxx");
        let agent2 = agent_id("agent-yyy");
        let agent_ids = vec![&agent1, &agent2];

        // Only agent-yyy has matches
        let mut matches = HashSet::new();
        matches.insert(agent_id("agent-yyy"));

        terminal
            .draw(|frame| {
                render_tab_bar(frame, frame.area(), &agent_ids, None, &matches);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let buffer_str = buffer
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect::<String>();

        // Should have exactly one indicator
        let indicator_count = buffer_str.matches('•').count();
        assert_eq!(
            indicator_count, 1,
            "Should have exactly 1 indicator for 1 tab with matches"
        );
    }

    #[test]
    fn render_tab_bar_match_indicator_with_selection() {
        let mut terminal = create_test_terminal();
        let agent1 = agent_id("agent-aaa");
        let agent2 = agent_id("agent-bbb");
        let agent_ids = vec![&agent1, &agent2];

        // agent-aaa has matches and is selected
        let mut matches = HashSet::new();
        matches.insert(agent_id("agent-aaa"));

        terminal
            .draw(|frame| {
                render_tab_bar(frame, frame.area(), &agent_ids, Some(0), &matches);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let buffer_str = buffer
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect::<String>();

        // Should show both indicator and selection highlight
        assert!(
            buffer_str.contains("•"),
            "Selected tab with matches should still show indicator"
        );

        // Should have yellow color for selection
        let has_yellow = buffer
            .content()
            .iter()
            .any(|cell| cell.fg == ratatui::style::Color::Yellow);
        assert!(
            has_yellow,
            "Selected tab should be highlighted in yellow"
        );
    }
}
