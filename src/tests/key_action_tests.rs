//! Tests for KeyAction enum and KeyBindings.

use crate::config::KeyBindings;
use crate::model::KeyAction;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// Test that KeyAction enum has all required variants.
#[test]
fn test_key_action_variants_exist() {
    // Scrolling
    let _: KeyAction = KeyAction::ScrollUp;
    let _: KeyAction = KeyAction::ScrollDown;
    let _: KeyAction = KeyAction::ScrollLeft;
    let _: KeyAction = KeyAction::ScrollRight;
    let _: KeyAction = KeyAction::PageUp;
    let _: KeyAction = KeyAction::PageDown;
    let _: KeyAction = KeyAction::ScrollToTop;
    let _: KeyAction = KeyAction::ScrollToBottom;

    // Focus navigation
    let _: KeyAction = KeyAction::FocusMain;
    let _: KeyAction = KeyAction::FocusSubagent;
    let _: KeyAction = KeyAction::FocusStats;
    let _: KeyAction = KeyAction::CycleFocus;

    // Tab navigation
    let _: KeyAction = KeyAction::NextTab;
    let _: KeyAction = KeyAction::PrevTab;
    let _: KeyAction = KeyAction::SelectTab(1);

    // Message interaction
    let _: KeyAction = KeyAction::ExpandMessage;
    let _: KeyAction = KeyAction::CollapseMessage;
    let _: KeyAction = KeyAction::ToggleExpand;

    // Search
    let _: KeyAction = KeyAction::StartSearch;
    let _: KeyAction = KeyAction::SubmitSearch;
    let _: KeyAction = KeyAction::CancelSearch;
    let _: KeyAction = KeyAction::NextMatch;
    let _: KeyAction = KeyAction::PrevMatch;

    // Stats
    let _: KeyAction = KeyAction::ToggleStats;
    let _: KeyAction = KeyAction::FilterGlobal;
    let _: KeyAction = KeyAction::FilterMainAgent;
    let _: KeyAction = KeyAction::FilterSubagent;

    // Auto-scroll
    let _: KeyAction = KeyAction::ToggleAutoScroll;
    let _: KeyAction = KeyAction::ScrollToLatest;

    // Application
    let _: KeyAction = KeyAction::Quit;
    let _: KeyAction = KeyAction::Help;
    let _: KeyAction = KeyAction::Refresh;
}

/// Test that default bindings include vim-style navigation.
#[test]
fn test_default_bindings_vim_style() {
    let kb = KeyBindings::default();

    // j/k for vertical scrolling
    assert_eq!(
        kb.get(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE)),
        Some(KeyAction::ScrollDown)
    );
    assert_eq!(
        kb.get(KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE)),
        Some(KeyAction::ScrollUp)
    );

    // h/l for horizontal scrolling
    assert_eq!(
        kb.get(KeyEvent::new(KeyCode::Char('h'), KeyModifiers::NONE)),
        Some(KeyAction::ScrollLeft)
    );
    assert_eq!(
        kb.get(KeyEvent::new(KeyCode::Char('l'), KeyModifiers::NONE)),
        Some(KeyAction::ScrollRight)
    );

    // g/G for top/bottom
    assert_eq!(
        kb.get(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE)),
        Some(KeyAction::ScrollToTop)
    );
    assert_eq!(
        kb.get(KeyEvent::new(KeyCode::Char('G'), KeyModifiers::SHIFT)),
        Some(KeyAction::ScrollToBottom)
    );
}

/// Test that default bindings include arrow key navigation.
#[test]
fn test_default_bindings_arrow_keys() {
    let kb = KeyBindings::default();

    assert_eq!(
        kb.get(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE)),
        Some(KeyAction::ScrollUp)
    );
    assert_eq!(
        kb.get(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)),
        Some(KeyAction::ScrollDown)
    );
    assert_eq!(
        kb.get(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE)),
        Some(KeyAction::ScrollLeft)
    );
    assert_eq!(
        kb.get(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE)),
        Some(KeyAction::ScrollRight)
    );
}

/// Test that default bindings include page navigation.
#[test]
fn test_default_bindings_page_navigation() {
    let kb = KeyBindings::default();

    // Ctrl+d/u for page down/up
    assert_eq!(
        kb.get(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL)),
        Some(KeyAction::PageDown)
    );
    assert_eq!(
        kb.get(KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL)),
        Some(KeyAction::PageUp)
    );

    // PageDown/PageUp keys
    assert_eq!(
        kb.get(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE)),
        Some(KeyAction::PageDown)
    );
    assert_eq!(
        kb.get(KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE)),
        Some(KeyAction::PageUp)
    );
}

/// Test that default bindings include focus switching.
#[test]
fn test_default_bindings_focus() {
    let kb = KeyBindings::default();

    // Tab for cycle focus
    assert_eq!(
        kb.get(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE)),
        Some(KeyAction::CycleFocus)
    );

    // Number keys for direct focus
    assert_eq!(
        kb.get(KeyEvent::new(KeyCode::Char('1'), KeyModifiers::NONE)),
        Some(KeyAction::FocusMain)
    );
    assert_eq!(
        kb.get(KeyEvent::new(KeyCode::Char('2'), KeyModifiers::NONE)),
        Some(KeyAction::FocusSubagent)
    );
    assert_eq!(
        kb.get(KeyEvent::new(KeyCode::Char('3'), KeyModifiers::NONE)),
        Some(KeyAction::FocusStats)
    );
}

/// Test that default bindings include tab navigation.
#[test]
fn test_default_bindings_tab_navigation() {
    let kb = KeyBindings::default();

    // ]/[ for next/prev tab
    assert_eq!(
        kb.get(KeyEvent::new(KeyCode::Char(']'), KeyModifiers::NONE)),
        Some(KeyAction::NextTab)
    );
    assert_eq!(
        kb.get(KeyEvent::new(KeyCode::Char('['), KeyModifiers::NONE)),
        Some(KeyAction::PrevTab)
    );

    // Shift+Tab for prev tab
    assert_eq!(
        kb.get(KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT)),
        Some(KeyAction::PrevTab)
    );
}

/// Test that default bindings include message interaction.
#[test]
fn test_default_bindings_message_interaction() {
    let kb = KeyBindings::default();

    // Enter/Space for toggle
    assert_eq!(
        kb.get(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
        Some(KeyAction::ToggleExpand)
    );
    assert_eq!(
        kb.get(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE)),
        Some(KeyAction::ToggleExpand)
    );

    // e/c for expand/collapse
    assert_eq!(
        kb.get(KeyEvent::new(KeyCode::Char('e'), KeyModifiers::NONE)),
        Some(KeyAction::ExpandMessage)
    );
    assert_eq!(
        kb.get(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE)),
        Some(KeyAction::CollapseMessage)
    );
}

/// Test that default bindings include search controls.
#[test]
fn test_default_bindings_search() {
    let kb = KeyBindings::default();

    // / to start search
    assert_eq!(
        kb.get(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE)),
        Some(KeyAction::StartSearch)
    );

    // n/N for next/prev match
    assert_eq!(
        kb.get(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE)),
        Some(KeyAction::NextMatch)
    );
    assert_eq!(
        kb.get(KeyEvent::new(KeyCode::Char('N'), KeyModifiers::SHIFT)),
        Some(KeyAction::PrevMatch)
    );

    // Esc to cancel search
    assert_eq!(
        kb.get(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
        Some(KeyAction::CancelSearch)
    );
}

/// Test that default bindings include stats controls.
#[test]
fn test_default_bindings_stats() {
    let kb = KeyBindings::default();

    assert_eq!(
        kb.get(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE)),
        Some(KeyAction::ToggleStats)
    );
}

/// Test that default bindings include live mode controls.
#[test]
fn test_default_bindings_live_mode() {
    let kb = KeyBindings::default();

    assert_eq!(
        kb.get(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE)),
        Some(KeyAction::ToggleAutoScroll)
    );
}

/// Test that default bindings include application controls.
#[test]
fn test_default_bindings_application() {
    let kb = KeyBindings::default();

    // q to quit
    assert_eq!(
        kb.get(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE)),
        Some(KeyAction::Quit)
    );

    // ? for help
    assert_eq!(
        kb.get(KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE)),
        Some(KeyAction::Help)
    );

    // r for refresh
    assert_eq!(
        kb.get(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE)),
        Some(KeyAction::Refresh)
    );
}

/// Test that unmapped keys return None.
#[test]
fn test_unmapped_keys_return_none() {
    let kb = KeyBindings::default();

    // Random unmapped key
    assert_eq!(
        kb.get(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE)),
        None
    );

    // Ctrl+Z (typically not mapped)
    assert_eq!(
        kb.get(KeyEvent::new(KeyCode::Char('z'), KeyModifiers::CONTROL)),
        None
    );
}

/// Test that SelectTab works for number keys 1-9.
#[test]
fn test_select_tab_number_keys() {
    // This tests that SelectTab(usize) can be constructed with different values.
    // The actual mapping of keys 4-9 to SelectTab actions would be in implementation
    // if we decide to use them for tab selection.

    let tab1 = KeyAction::SelectTab(1);
    let tab2 = KeyAction::SelectTab(2);
    let tab9 = KeyAction::SelectTab(9);

    // Verify they're different
    assert_ne!(tab1, tab2);
    assert_ne!(tab1, tab9);

    // Verify they're equal to themselves
    assert_eq!(tab1, KeyAction::SelectTab(1));
    assert_eq!(tab9, KeyAction::SelectTab(9));
}

/// Test that KeyAction implements required traits.
#[test]
fn test_key_action_traits() {
    let action = KeyAction::ScrollUp;

    // Debug
    let _ = format!("{:?}", action);

    // Clone (via Copy - since KeyAction is Copy, assignment is sufficient)
    let cloned = action;
    assert_eq!(action, cloned);

    // Copy (via assignment)
    let copied = action;
    assert_eq!(action, copied);

    // PartialEq
    assert_eq!(KeyAction::ScrollUp, KeyAction::ScrollUp);
    assert_ne!(KeyAction::ScrollUp, KeyAction::ScrollDown);

    // Hash (can be used in HashMap)
    use std::collections::HashSet;
    let mut set = HashSet::new();
    set.insert(action);
    assert!(set.contains(&KeyAction::ScrollUp));
}

/// Test that all user-facing KeyAction variants have at least one default binding.
///
/// This prevents regressions where new actions are added but not mapped to keys.
/// We test by attempting to find at least one key that maps to each action.
#[test]
fn test_all_actions_have_default_bindings() {
    let kb = KeyBindings::default();

    // Helper to check if an action has any binding by testing common key combinations
    let has_binding = |target_action: KeyAction| -> bool {
        use crossterm::event::{KeyCode, KeyModifiers};

        // Build a vector of all keys to test
        let mut test_keys = Vec::new();

        // Letters a-z (lowercase)
        for c in 'a'..='z' {
            test_keys.push(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE));
        }

        // Letters A-Z (uppercase/shift)
        for c in 'A'..='Z' {
            test_keys.push(KeyEvent::new(KeyCode::Char(c), KeyModifiers::SHIFT));
        }

        // Numbers 0-9
        for c in '0'..='9' {
            test_keys.push(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE));
        }

        // Special characters
        test_keys.extend_from_slice(&[
            KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Char('['), KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Char(']'), KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE),
        ]);

        // Special keys
        test_keys.extend_from_slice(&[
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE),
            KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT),
            KeyEvent::new(KeyCode::Up, KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Down, KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Left, KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Right, KeyModifiers::NONE),
            KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE),
            KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Home, KeyModifiers::NONE),
            KeyEvent::new(KeyCode::End, KeyModifiers::NONE),
        ]);

        // Ctrl combinations
        for c in 'a'..='z' {
            test_keys.push(KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL));
        }

        for key in test_keys {
            if let Some(action) = kb.get(key) {
                // For SelectTab, match any tab number
                let matches = match (target_action, action) {
                    (KeyAction::SelectTab(_), KeyAction::SelectTab(_)) => true,
                    _ => action == target_action,
                };
                if matches {
                    return true;
                }
            }
        }
        false
    };

    // Define all user-facing actions that should have bindings
    let required_actions = vec![
        // Scrolling
        ("ScrollUp", KeyAction::ScrollUp),
        ("ScrollDown", KeyAction::ScrollDown),
        ("ScrollLeft", KeyAction::ScrollLeft),
        ("ScrollRight", KeyAction::ScrollRight),
        ("PageUp", KeyAction::PageUp),
        ("PageDown", KeyAction::PageDown),
        ("ScrollToTop", KeyAction::ScrollToTop),
        ("ScrollToBottom", KeyAction::ScrollToBottom),
        // Focus navigation
        ("FocusMain", KeyAction::FocusMain),
        ("FocusSubagent", KeyAction::FocusSubagent),
        ("FocusStats", KeyAction::FocusStats),
        ("CycleFocus", KeyAction::CycleFocus),
        // Tab navigation
        ("NextTab", KeyAction::NextTab),
        ("PrevTab", KeyAction::PrevTab),
        ("SelectTab", KeyAction::SelectTab(1)), // Representative
        // Message interaction
        ("ExpandMessage", KeyAction::ExpandMessage),
        ("CollapseMessage", KeyAction::CollapseMessage),
        ("ToggleExpand", KeyAction::ToggleExpand),
        // Search
        ("StartSearch", KeyAction::StartSearch),
        ("SubmitSearch", KeyAction::SubmitSearch),
        ("CancelSearch", KeyAction::CancelSearch),
        ("NextMatch", KeyAction::NextMatch),
        ("PrevMatch", KeyAction::PrevMatch),
        // Stats
        ("ToggleStats", KeyAction::ToggleStats),
        ("FilterGlobal", KeyAction::FilterGlobal),
        ("FilterMainAgent", KeyAction::FilterMainAgent),
        ("FilterSubagent", KeyAction::FilterSubagent),
        // Auto-scroll
        ("ToggleAutoScroll", KeyAction::ToggleAutoScroll),
        ("ScrollToLatest", KeyAction::ScrollToLatest),
        // Application
        ("Quit", KeyAction::Quit),
        ("Help", KeyAction::Help),
        ("Refresh", KeyAction::Refresh),
    ];

    // Check that every required action has a binding
    for (name, action) in required_actions {
        assert!(
            has_binding(action),
            "KeyAction::{} has no default key binding (FR-045 violation)",
            name
        );
    }
}
