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
