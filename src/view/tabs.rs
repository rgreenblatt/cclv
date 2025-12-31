//! Subagent tab bar widget.
//!
//! Displays tabs for each subagent using ratatui's Tabs widget.
//! Selection state is managed by AppState.selected_tab.

use crate::model::AgentId;
use ratatui::{
    layout::Rect,
    Frame,
};

/// Render the subagent tab bar.
///
/// # Arguments
/// * `frame` - The ratatui frame to render into
/// * `area` - The area to render the tab bar within
/// * `agent_ids` - Ordered list of subagent IDs (from Session::subagent_ids_ordered)
/// * `selected_tab` - Index of selected tab (None means no selection)
///
/// # Behavior
/// - Shows one tab per subagent with agent ID as label
/// - Highlights the selected tab if Some(index)
/// - Supports deselection via None
/// - Agent IDs may be truncated if they exceed available space
pub fn render_tab_bar(
    _frame: &mut Frame,
    _area: Rect,
    _agent_ids: &[&AgentId],
    _selected_tab: Option<usize>,
) {
    todo!("render_tab_bar")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::AgentId;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    // Test helper: Create test terminal
    fn create_test_terminal() -> Terminal<TestBackend> {
        let backend = TestBackend::new(80, 24);
        Terminal::new(backend).unwrap()
    }

    // Test helper: Create agent ID
    fn agent_id(s: &str) -> AgentId {
        AgentId::new(s).unwrap()
    }

    #[test]
    fn render_tab_bar_displays_single_tab() {
        let mut terminal = create_test_terminal();
        let agent1 = agent_id("agent-abc");
        let agent_ids = vec![&agent1];

        terminal
            .draw(|frame| {
                render_tab_bar(frame, frame.area(), &agent_ids, Some(0));
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content = buffer.content();

        // Should contain the agent ID somewhere in the buffer
        let has_agent_id = content.iter().any(|cell| {
            cell.symbol().contains("agent-abc")
        });

        assert!(has_agent_id, "Tab bar should display agent ID 'agent-abc'");
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
                render_tab_bar(frame, frame.area(), &agent_ids, Some(1));
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let buffer_str = buffer.content()
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
            render_tab_bar(frame, frame.area(), &agent_ids, None);
        });

        assert!(result.is_ok(), "Should render without error when selection is None");
    }

    #[test]
    fn render_tab_bar_handles_empty_agent_list() {
        let mut terminal = create_test_terminal();
        let agent_ids: Vec<&AgentId> = vec![];

        // Should not panic with empty list
        let result = terminal.draw(|frame| {
            render_tab_bar(frame, frame.area(), &agent_ids, None);
        });

        assert!(result.is_ok(), "Should render without error when agent list is empty");
    }

    #[test]
    fn render_tab_bar_selection_within_bounds() {
        let mut terminal = create_test_terminal();
        let agent1 = agent_id("agent-1");
        let agent2 = agent_id("agent-2");
        let agent_ids = vec![&agent1, &agent2];

        // Selecting index 1 (agent-2) should work
        let result = terminal.draw(|frame| {
            render_tab_bar(frame, frame.area(), &agent_ids, Some(1));
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
                render_tab_bar(frame, frame.area(), &agent_ids, Some(0));
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let buffer_str = buffer.content()
            .iter()
            .map(|c| c.symbol())
            .collect::<String>();

        // The full agent ID should be used as the tab label
        assert!(
            buffer_str.contains("subagent-12345"),
            "Tab should use agent ID as label"
        );
    }
}
