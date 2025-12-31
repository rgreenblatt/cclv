//! Domain-level keyboard actions independent of key bindings.

/// Domain-level actions that can be mapped to configurable key bindings.
///
/// These represent user intent, not specific keys. The mapping from
/// crossterm::event::KeyEvent to KeyAction is handled by KeyBindings.
///
/// See FR-043, FR-044, FR-045 for keyboard configuration requirements.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeyAction {
    // Scrolling
    /// Scroll conversation view up by one line (FR-027, FR-034). Default: j/↓
    ScrollUp,
    /// Scroll conversation view down by one line (FR-027, FR-034). Default: k/↑
    ScrollDown,
    /// Scroll horizontally left when line wrapping disabled (FR-040). Default: h/←
    ScrollLeft,
    /// Scroll horizontally right when line wrapping disabled (FR-040). Default: l/→
    ScrollRight,
    /// Scroll up by one page height (FR-027). Default: Ctrl+u/Page Up
    PageUp,
    /// Scroll down by one page height (FR-027). Default: Ctrl+d/Page Down
    PageDown,
    /// Jump to top of conversation (FR-027). Default: g/Home
    ScrollToTop,
    /// Jump to bottom of conversation (FR-027). Default: G/End
    ScrollToBottom,

    // Focus navigation
    /// Focus main agent conversation pane (FR-001, FR-025). Default: 1
    FocusMain,
    /// Focus subagent tabbed pane (FR-003, FR-025). Default: 2
    FocusSubagent,
    /// Focus statistics panel (FR-015-020, FR-025). Default: 3
    FocusStats,
    /// Cycle focus between panes: Main → Subagent → Stats (FR-025). Default: Tab
    CycleFocus,

    // Tab navigation
    /// Switch to next subagent tab (FR-003, FR-004). Default: ]
    NextTab,
    /// Switch to previous subagent tab (FR-003, FR-004). Default: [/Shift+Tab
    PrevTab,
    /// Select specific subagent tab by number (FR-003). Field: tab index (1-9)
    SelectTab(usize),

    // Message interaction
    /// Expand collapsed message to show full content (FR-032). Default: Enter/Space
    ExpandMessage,
    /// Collapse expanded message to summary form (FR-033). Default: Enter/Space
    CollapseMessage,
    /// Toggle current message between expanded and collapsed (FR-032, FR-033). Default: Enter/Space
    ToggleExpand,

    // Entry navigation (keyboard focus)
    /// Move focus to next entry in conversation. Default: Ctrl+j
    NextEntry,
    /// Move focus to previous entry in conversation. Default: Ctrl+k
    PrevEntry,

    // Search
    /// Activate search input (FR-011). Default: //Ctrl+f
    StartSearch,
    /// Submit search query and highlight matches (FR-012). Default: Enter
    SubmitSearch,
    /// Cancel search and clear highlighting (FR-014). Default: Esc
    CancelSearch,
    /// Navigate to next search match (FR-013). Default: n
    NextMatch,
    /// Navigate to previous search match (FR-013). Default: N/Shift+n
    PrevMatch,

    // Stats
    /// Toggle visibility of statistics panel (FR-015-020). Default: s
    ToggleStats,
    /// Filter stats to show all agents globally (FR-020). Default: !
    FilterGlobal,
    /// Filter stats to show main agent only (FR-020). Default: @
    FilterMainAgent,
    /// Filter stats to show current subagent only (FR-020). Default: #
    FilterSubagent,

    // Auto-scroll (live mode)
    /// Toggle auto-scroll behavior when following live logs (FR-036, FR-038). Default: a
    ToggleAutoScroll,
    /// Resume auto-scroll and jump to latest content (FR-038). Default: scroll to bottom
    ScrollToLatest,

    // Line wrapping
    /// Toggle line wrapping for current conversation item (FR-048, FR-049, FR-050). Default: w
    ToggleWrap,
    /// Toggle global line wrapping for all items (FR-039, FR-050, FR-051). Default: W/Shift+w
    ToggleGlobalWrap,

    // Application
    /// Exit the application (FR-025). Default: q/Ctrl+c
    Quit,
    /// Show help overlay with keyboard shortcuts (FR-026). Default: ?
    Help,
    /// Refresh display and reload data (FR-025). Default: r
    Refresh,
}

#[cfg(test)]
mod tests {
    use super::*;

    // ===== ToggleWrap Tests =====

    #[test]
    fn toggle_wrap_pattern_match_discriminates_correctly() {
        let action = KeyAction::ToggleWrap;
        match action {
            KeyAction::ToggleWrap => {
                // Correct variant matched
            }
            _ => panic!("ToggleWrap should match ToggleWrap variant"),
        }
    }

    #[test]
    fn toggle_wrap_equals_itself() {
        let action1 = KeyAction::ToggleWrap;
        let action2 = KeyAction::ToggleWrap;
        assert_eq!(
            action1, action2,
            "ToggleWrap should equal another ToggleWrap"
        );
    }

    #[test]
    fn toggle_wrap_not_equals_other_variant() {
        let toggle_wrap = KeyAction::ToggleWrap;
        let scroll_up = KeyAction::ScrollUp;
        assert_ne!(
            toggle_wrap, scroll_up,
            "ToggleWrap should not equal ScrollUp"
        );
    }

    #[test]
    fn toggle_wrap_clone_equals_original() {
        let action = KeyAction::ToggleWrap;
        let cloned = action;
        assert_eq!(action, cloned, "Cloned ToggleWrap should equal original");
    }

    // ===== ToggleGlobalWrap Tests =====

    #[test]
    fn toggle_global_wrap_pattern_match_discriminates_correctly() {
        let action = KeyAction::ToggleGlobalWrap;
        match action {
            KeyAction::ToggleGlobalWrap => {
                // Correct variant matched
            }
            _ => panic!("ToggleGlobalWrap should match ToggleGlobalWrap variant"),
        }
    }

    #[test]
    fn toggle_global_wrap_equals_itself() {
        let action1 = KeyAction::ToggleGlobalWrap;
        let action2 = KeyAction::ToggleGlobalWrap;
        assert_eq!(
            action1, action2,
            "ToggleGlobalWrap should equal another ToggleGlobalWrap"
        );
    }

    #[test]
    fn toggle_global_wrap_not_equals_other_variant() {
        let toggle_global = KeyAction::ToggleGlobalWrap;
        let scroll_down = KeyAction::ScrollDown;
        assert_ne!(
            toggle_global, scroll_down,
            "ToggleGlobalWrap should not equal ScrollDown"
        );
    }

    #[test]
    fn toggle_global_wrap_clone_equals_original() {
        let action = KeyAction::ToggleGlobalWrap;
        let cloned = action;
        assert_eq!(
            action, cloned,
            "Cloned ToggleGlobalWrap should equal original"
        );
    }

    // ===== Discriminate Between ToggleWrap and ToggleGlobalWrap =====

    #[test]
    fn toggle_wrap_not_equals_toggle_global_wrap() {
        let toggle_wrap = KeyAction::ToggleWrap;
        let toggle_global = KeyAction::ToggleGlobalWrap;
        assert_ne!(
            toggle_wrap, toggle_global,
            "ToggleWrap should not equal ToggleGlobalWrap"
        );
    }
}
