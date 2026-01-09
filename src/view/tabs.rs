//! Conversation tab bar widget.
//!
//! Displays tabs for all conversations (Main Agent + Subagents).
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

/// Represents a conversation tab in the tab bar.
///
/// FR-083/084/086: Tab bar always includes Main Agent at position 0,
/// followed by subagents in spawn order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConversationTab<'a> {
    /// Main agent conversation (always at position 0)
    Main,
    /// Subagent conversation identified by AgentId
    Subagent(&'a AgentId),
}

/// Render the conversation tab bar.
///
/// FR-083/084/086/088: Tab bar always includes Main Agent at position 0,
/// followed by subagents. Tab bar always visible, even with no subagents.
///
/// # Arguments
/// * `frame` - The ratatui frame to render into
/// * `area` - The area to render the tab bar within
/// * `tabs` - Ordered list of conversation tabs (Main Agent at index 0, subagents follow)
/// * `selected_tab` - Index of selected tab (None means no selection)
/// * `tabs_with_matches` - Set of agent IDs that contain search matches (empty set if no search active)
///
/// # Behavior
/// - Tab 0 is always "Main Agent"
/// - Tabs 1..N show subagent IDs as labels
/// - Highlights the selected tab if Some(index) and index is in bounds
/// - Supports deselection via None (no highlight)
/// - Out-of-bounds indices are treated as None
/// - Labels may be truncated if they exceed available space
/// - Tabs with search matches display a visual indicator (•)
pub fn render_tab_bar(
    frame: &mut Frame,
    area: Rect,
    tabs: &[ConversationTab],
    selected_tab: Option<usize>,
    tabs_with_matches: &HashSet<AgentId>,
) {
    // Convert conversation tabs to titles with match indicators
    let titles: Vec<Line> = tabs
        .iter()
        .map(|tab| {
            let label = match tab {
                ConversationTab::Main => {
                    // Main agent - no match indicator (matches apply to subagents only)
                    "Main".to_string()
                }
                ConversationTab::Subagent(agent_id) => {
                    // Subagent - show match indicator if matches exist
                    if tabs_with_matches.contains(*agent_id) {
                        format!("{} •", agent_id.as_str())
                    } else {
                        agent_id.as_str().to_string()
                    }
                }
            };
            Line::from(label)
        })
        .collect();

    // Validate bounds: treat out-of-bounds as None
    let validated_selection = selected_tab.filter(|&idx| idx < tabs.len());

    // Create Tabs widget with block
    let mut tabs_widget = Tabs::new(titles)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Conversations"),
        )
        .style(Style::default().fg(Color::White));

    // Apply highlight only if we have a valid selection
    // ratatui's Tabs widget doesn't support "no selection", so we work around it:
    // - With selection: set highlight_style and select
    // - Without selection: omit highlight_style (tabs render without highlight)
    if let Some(idx) = validated_selection {
        tabs_widget = tabs_widget
            .highlight_style(Style::default().fg(Color::Yellow))
            .select(idx);
    }

    // Render the tabs widget
    frame.render_widget(tabs_widget, area);
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
    fn render_tab_bar_displays_single_subagent_tab() {
        let mut terminal = create_test_terminal();
        let agent1 = agent_id("agent-abc");
        let tabs = vec![ConversationTab::Main, ConversationTab::Subagent(&agent1)];

        terminal
            .draw(|frame| {
                render_tab_bar(frame, frame.area(), &tabs, Some(1), &no_matches());
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let buffer_str = buffer
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect::<String>();

        // Should contain both Main and the subagent ID
        assert!(buffer_str.contains("Main"), "Tab bar should display 'Main'");
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
        let tabs = vec![
            ConversationTab::Main,
            ConversationTab::Subagent(&agent1),
            ConversationTab::Subagent(&agent2),
            ConversationTab::Subagent(&agent3),
        ];

        terminal
            .draw(|frame| {
                render_tab_bar(frame, frame.area(), &tabs, Some(1), &no_matches());
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let buffer_str = buffer
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect::<String>();

        // Main and all three subagent IDs should be present
        assert!(buffer_str.contains("Main"), "Should contain Main");
        assert!(buffer_str.contains("agent-1"), "Should contain agent-1");
        assert!(buffer_str.contains("agent-2"), "Should contain agent-2");
        assert!(buffer_str.contains("agent-3"), "Should contain agent-3");
    }

    #[test]
    fn render_tab_bar_handles_no_selection() {
        let mut terminal = create_test_terminal();
        let agent1 = agent_id("agent-xyz");
        let tabs = vec![ConversationTab::Main, ConversationTab::Subagent(&agent1)];

        // Should not panic with None selection
        let result = terminal.draw(|frame| {
            render_tab_bar(frame, frame.area(), &tabs, None, &no_matches());
        });

        assert!(
            result.is_ok(),
            "Should render without error when selection is None"
        );
    }

    #[test]
    fn render_tab_bar_handles_main_agent_only() {
        let mut terminal = create_test_terminal();
        let tabs = vec![ConversationTab::Main];

        // Should not panic with only Main Agent (no subagents)
        let result = terminal.draw(|frame| {
            render_tab_bar(frame, frame.area(), &tabs, None, &no_matches());
        });

        assert!(
            result.is_ok(),
            "Should render without error when only Main Agent exists"
        );
    }

    #[test]
    fn render_tab_bar_selection_within_bounds() {
        let mut terminal = create_test_terminal();
        let agent1 = agent_id("agent-1");
        let agent2 = agent_id("agent-2");
        let tabs = vec![
            ConversationTab::Main,
            ConversationTab::Subagent(&agent1),
            ConversationTab::Subagent(&agent2),
        ];

        // Selecting index 1 (first subagent) should work
        let result = terminal.draw(|frame| {
            render_tab_bar(frame, frame.area(), &tabs, Some(1), &no_matches());
        });

        assert!(result.is_ok(), "Should render with valid selection index");
    }

    #[test]
    fn render_tab_bar_uses_agent_id_as_label() {
        let mut terminal = create_test_terminal();
        let agent1 = agent_id("subagent-12345");
        let tabs = vec![ConversationTab::Main, ConversationTab::Subagent(&agent1)];

        terminal
            .draw(|frame| {
                render_tab_bar(frame, frame.area(), &tabs, Some(1), &no_matches());
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
        let tabs = vec![
            ConversationTab::Main,
            ConversationTab::Subagent(&agent1),
            ConversationTab::Subagent(&agent2),
        ];

        // Selecting index 5 when only 3 tabs exist should be treated as None
        let result = terminal.draw(|frame| {
            render_tab_bar(frame, frame.area(), &tabs, Some(5), &no_matches());
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
        let tabs = vec![
            ConversationTab::Main,
            ConversationTab::Subagent(&agent1),
            ConversationTab::Subagent(&agent2),
        ];

        // Render with None selection
        let mut terminal_none = create_test_terminal();
        terminal_none
            .draw(|frame| {
                render_tab_bar(frame, frame.area(), &tabs, None, &no_matches());
            })
            .unwrap();
        let buffer_none = terminal_none.backend().buffer().clone();

        // Render with Some(0) selection (Main Agent)
        let mut terminal_some = create_test_terminal();
        terminal_some
            .draw(|frame| {
                render_tab_bar(frame, frame.area(), &tabs, Some(0), &no_matches());
            })
            .unwrap();
        let buffer_some = terminal_some.backend().buffer().clone();

        // The two buffers should differ - None should not highlight any tab,
        // while Some(0) should highlight Main Agent tab
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
            "Some(0) selection should highlight Main Agent tab (has yellow)"
        );
    }

    // ===== Match Indicator Tests =====

    #[test]
    fn render_tab_bar_shows_indicator_on_tab_with_matches() {
        let mut terminal = create_test_terminal();
        let agent1 = agent_id("agent-1");
        let agent2 = agent_id("agent-2");
        let tabs = vec![
            ConversationTab::Main,
            ConversationTab::Subagent(&agent1),
            ConversationTab::Subagent(&agent2),
        ];

        // Create match set with agent-1 having matches
        let mut matches = HashSet::new();
        matches.insert(agent_id("agent-1"));

        terminal
            .draw(|frame| {
                render_tab_bar(frame, frame.area(), &tabs, None, &matches);
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
        let tabs = vec![
            ConversationTab::Main,
            ConversationTab::Subagent(&agent1),
            ConversationTab::Subagent(&agent2),
        ];

        // Empty match set - no matches
        let matches = HashSet::new();

        terminal
            .draw(|frame| {
                render_tab_bar(frame, frame.area(), &tabs, None, &matches);
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
        let tabs = vec![
            ConversationTab::Main,
            ConversationTab::Subagent(&agent1),
            ConversationTab::Subagent(&agent2),
            ConversationTab::Subagent(&agent3),
        ];

        // Both agent-1 and agent-3 have matches
        let mut matches = HashSet::new();
        matches.insert(agent_id("agent-1"));
        matches.insert(agent_id("agent-3"));

        terminal
            .draw(|frame| {
                render_tab_bar(frame, frame.area(), &tabs, None, &matches);
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
        let tabs = vec![
            ConversationTab::Main,
            ConversationTab::Subagent(&agent1),
            ConversationTab::Subagent(&agent2),
        ];

        // Only agent-yyy has matches
        let mut matches = HashSet::new();
        matches.insert(agent_id("agent-yyy"));

        terminal
            .draw(|frame| {
                render_tab_bar(frame, frame.area(), &tabs, None, &matches);
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
        let tabs = vec![
            ConversationTab::Main,
            ConversationTab::Subagent(&agent1),
            ConversationTab::Subagent(&agent2),
        ];

        // agent-aaa (tab index 1) has matches and is selected
        let mut matches = HashSet::new();
        matches.insert(agent_id("agent-aaa"));

        terminal
            .draw(|frame| {
                render_tab_bar(frame, frame.area(), &tabs, Some(1), &matches);
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
        assert!(has_yellow, "Selected tab should be highlighted in yellow");
    }

    // ===== FR-083/084/088: Main Agent Tab Tests =====

    #[test]
    fn render_tab_bar_shows_main_agent_at_position_zero() {
        let mut terminal = create_test_terminal();
        let agent1 = agent_id("subagent-1");
        let agent2 = agent_id("subagent-2");

        // Build tab list: Main Agent + subagents
        let tabs = vec![
            ConversationTab::Main,
            ConversationTab::Subagent(&agent1),
            ConversationTab::Subagent(&agent2),
        ];

        terminal
            .draw(|frame| {
                render_tab_bar(frame, frame.area(), &tabs, Some(0), &no_matches());
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let buffer_str = buffer
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect::<String>();

        // FR-086: Main should appear in tab bar
        assert!(
            buffer_str.contains("Main"),
            "Tab bar should display 'Main' at position 0"
        );
    }

    #[test]
    fn render_tab_bar_shows_main_agent_even_without_subagents() {
        let mut terminal = create_test_terminal();

        // Only main agent, no subagents (FR-084/088)
        let tabs = vec![ConversationTab::Main];

        terminal
            .draw(|frame| {
                render_tab_bar(frame, frame.area(), &tabs, Some(0), &no_matches());
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let buffer_str = buffer
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect::<String>();

        // FR-084/088: Tab bar should NOT be empty when only main agent exists
        assert!(
            buffer_str.contains("Main"),
            "Tab bar should show 'Main' even with no subagents"
        );
    }

    #[test]
    fn render_tab_bar_title_says_conversations_not_subagents() {
        let mut terminal = create_test_terminal();
        let tabs = vec![ConversationTab::Main];

        terminal
            .draw(|frame| {
                render_tab_bar(frame, frame.area(), &tabs, Some(0), &no_matches());
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let buffer_str = buffer
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect::<String>();

        // Title should say "Conversations" not "Subagents"
        assert!(
            buffer_str.contains("Conversations"),
            "Tab bar title should say 'Conversations' not 'Subagents'"
        );
        assert!(
            !buffer_str.contains("Subagents"),
            "Tab bar title should NOT say 'Subagents'"
        );
    }

    #[test]
    fn render_tab_bar_main_agent_and_subagents_in_correct_order() {
        let mut terminal = create_test_terminal();
        let agent1 = agent_id("subagent-alpha");
        let agent2 = agent_id("subagent-beta");

        // FR-086: Main at position 0, subagents follow
        let tabs = vec![
            ConversationTab::Main,
            ConversationTab::Subagent(&agent1),
            ConversationTab::Subagent(&agent2),
        ];

        terminal
            .draw(|frame| {
                render_tab_bar(frame, frame.area(), &tabs, None, &no_matches());
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let buffer_str = buffer
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect::<String>();

        // All tabs should be present
        assert!(buffer_str.contains("Main"), "Should contain Main");
        assert!(
            buffer_str.contains("subagent-alpha"),
            "Should contain subagent-alpha"
        );
        assert!(
            buffer_str.contains("subagent-beta"),
            "Should contain subagent-beta"
        );

        // Main should appear before subagents in buffer
        let main_pos = buffer_str.find("Main").unwrap();
        let alpha_pos = buffer_str.find("subagent-alpha").unwrap();
        assert!(
            main_pos < alpha_pos,
            "Main should appear before subagents in tab order"
        );
    }
}
