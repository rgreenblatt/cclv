//! Domain-level keyboard actions independent of key bindings.

/// Domain-level actions that can be mapped to configurable key bindings.
///
/// These represent user intent, not specific keys. The mapping from
/// crossterm::event::KeyEvent to KeyAction is handled by KeyBindings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeyAction {
    // Scrolling
    ScrollUp,
    ScrollDown,
    ScrollLeft,
    ScrollRight,
    PageUp,
    PageDown,
    ScrollToTop,
    ScrollToBottom,

    // Focus navigation
    FocusMain,
    FocusSubagent,
    FocusStats,
    CycleFocus,

    // Tab navigation
    NextTab,
    PrevTab,
    SelectTab(usize), // 1-9

    // Message interaction
    ExpandMessage,
    CollapseMessage,
    ToggleExpand,

    // Search
    StartSearch,
    SubmitSearch,
    CancelSearch,
    NextMatch,
    PrevMatch,

    // Stats
    ToggleStats,
    FilterGlobal,
    FilterMainAgent,
    FilterSubagent,

    // Auto-scroll (live mode)
    ToggleAutoScroll,
    ScrollToLatest,

    // Application
    Quit,
    Help,
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
        let cloned = action.clone();
        assert_eq!(
            action, cloned,
            "Cloned ToggleWrap should equal original"
        );
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
        let cloned = action.clone();
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
