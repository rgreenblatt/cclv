//! Keyboard bindings configuration.

use crate::model::key_action::KeyAction;
use crossterm::event::KeyEvent;
use std::collections::HashMap;

/// Maps keyboard events to domain actions.
///
/// Provides default vim-style bindings with option to override via configuration.
#[derive(Debug, Clone)]
pub struct KeyBindings {
    bindings: HashMap<KeyEvent, KeyAction>,
}

impl KeyBindings {
    /// Look up the action for a key event.
    pub fn get(&self, key: KeyEvent) -> Option<KeyAction> {
        self.bindings.get(&key).copied()
    }
}

impl Default for KeyBindings {
    fn default() -> Self {
        use crossterm::event::{KeyCode, KeyModifiers};

        let mut bindings = HashMap::new();

        // Vim-style scrolling
        bindings.insert(
            KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE),
            KeyAction::ScrollDown,
        );
        bindings.insert(
            KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE),
            KeyAction::ScrollUp,
        );
        bindings.insert(
            KeyEvent::new(KeyCode::Char('h'), KeyModifiers::NONE),
            KeyAction::ScrollLeft,
        );
        bindings.insert(
            KeyEvent::new(KeyCode::Char('l'), KeyModifiers::NONE),
            KeyAction::ScrollRight,
        );
        bindings.insert(
            KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE),
            KeyAction::ScrollToTop,
        );
        bindings.insert(
            KeyEvent::new(KeyCode::Char('G'), KeyModifiers::SHIFT),
            KeyAction::ScrollToBottom,
        );
        bindings.insert(
            KeyEvent::new(KeyCode::End, KeyModifiers::NONE),
            KeyAction::ScrollToLatest,
        );

        // Arrow key scrolling
        bindings.insert(
            KeyEvent::new(KeyCode::Up, KeyModifiers::NONE),
            KeyAction::ScrollUp,
        );
        bindings.insert(
            KeyEvent::new(KeyCode::Down, KeyModifiers::NONE),
            KeyAction::ScrollDown,
        );
        bindings.insert(
            KeyEvent::new(KeyCode::Left, KeyModifiers::NONE),
            KeyAction::ScrollLeft,
        );
        bindings.insert(
            KeyEvent::new(KeyCode::Right, KeyModifiers::NONE),
            KeyAction::ScrollRight,
        );

        // Page navigation
        bindings.insert(
            KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL),
            KeyAction::PageDown,
        );
        bindings.insert(
            KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL),
            KeyAction::PageUp,
        );
        bindings.insert(
            KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE),
            KeyAction::PageDown,
        );
        bindings.insert(
            KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE),
            KeyAction::PageUp,
        );

        // Focus switching
        bindings.insert(
            KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE),
            KeyAction::CycleFocus,
        );
        bindings.insert(
            KeyEvent::new(KeyCode::Char('1'), KeyModifiers::NONE),
            KeyAction::FocusMain,
        );
        bindings.insert(
            KeyEvent::new(KeyCode::Char('2'), KeyModifiers::NONE),
            KeyAction::FocusSubagent,
        );
        bindings.insert(
            KeyEvent::new(KeyCode::Char('3'), KeyModifiers::NONE),
            KeyAction::FocusStats,
        );

        // Tab navigation
        bindings.insert(
            KeyEvent::new(KeyCode::Char(']'), KeyModifiers::NONE),
            KeyAction::NextTab,
        );
        bindings.insert(
            KeyEvent::new(KeyCode::Char('['), KeyModifiers::NONE),
            KeyAction::PrevTab,
        );
        bindings.insert(
            KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT),
            KeyAction::PrevTab,
        );

        // Direct tab selection (4-9, since 1-3 are for focus)
        bindings.insert(
            KeyEvent::new(KeyCode::Char('4'), KeyModifiers::NONE),
            KeyAction::SelectTab(4),
        );
        bindings.insert(
            KeyEvent::new(KeyCode::Char('5'), KeyModifiers::NONE),
            KeyAction::SelectTab(5),
        );
        bindings.insert(
            KeyEvent::new(KeyCode::Char('6'), KeyModifiers::NONE),
            KeyAction::SelectTab(6),
        );
        bindings.insert(
            KeyEvent::new(KeyCode::Char('7'), KeyModifiers::NONE),
            KeyAction::SelectTab(7),
        );
        bindings.insert(
            KeyEvent::new(KeyCode::Char('8'), KeyModifiers::NONE),
            KeyAction::SelectTab(8),
        );
        bindings.insert(
            KeyEvent::new(KeyCode::Char('9'), KeyModifiers::NONE),
            KeyAction::SelectTab(9),
        );

        // Message interaction
        bindings.insert(
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            KeyAction::ToggleExpand,
        );
        bindings.insert(
            KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE),
            KeyAction::ToggleExpand,
        );
        bindings.insert(
            KeyEvent::new(KeyCode::Char('e'), KeyModifiers::NONE),
            KeyAction::ExpandMessage,
        );
        bindings.insert(
            KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE),
            KeyAction::CollapseMessage,
        );

        // Entry navigation (keyboard focus)
        bindings.insert(
            KeyEvent::new(KeyCode::Char('j'), KeyModifiers::CONTROL),
            KeyAction::NextEntry,
        );
        bindings.insert(
            KeyEvent::new(KeyCode::Char('k'), KeyModifiers::CONTROL),
            KeyAction::PrevEntry,
        );

        // Search
        bindings.insert(
            KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE),
            KeyAction::StartSearch,
        );
        bindings.insert(
            KeyEvent::new(KeyCode::Char('f'), KeyModifiers::CONTROL),
            KeyAction::StartSearch,
        );
        bindings.insert(
            KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE),
            KeyAction::NextMatch,
        );
        bindings.insert(
            KeyEvent::new(KeyCode::Char('N'), KeyModifiers::SHIFT),
            KeyAction::PrevMatch,
        );
        bindings.insert(
            KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE),
            KeyAction::CancelSearch,
        );
        bindings.insert(
            KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL),
            KeyAction::SubmitSearch,
        );

        // Stats
        bindings.insert(
            KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE),
            KeyAction::ToggleStats,
        );
        bindings.insert(
            KeyEvent::new(KeyCode::Char('f'), KeyModifiers::NONE),
            KeyAction::FilterGlobal,
        );
        bindings.insert(
            KeyEvent::new(KeyCode::Char('m'), KeyModifiers::NONE),
            KeyAction::FilterMainAgent,
        );
        bindings.insert(
            KeyEvent::new(KeyCode::Char('S'), KeyModifiers::SHIFT),
            KeyAction::FilterSubagent,
        );

        // Live mode
        bindings.insert(
            KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE),
            KeyAction::ToggleAutoScroll,
        );

        // Wrap toggle
        bindings.insert(
            KeyEvent::new(KeyCode::Char('w'), KeyModifiers::NONE),
            KeyAction::ToggleWrap,
        );
        bindings.insert(
            KeyEvent::new(KeyCode::Char('W'), KeyModifiers::SHIFT),
            KeyAction::ToggleGlobalWrap,
        );

        // Application controls
        bindings.insert(
            KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE),
            KeyAction::Quit,
        );
        bindings.insert(
            KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE),
            KeyAction::Help,
        );
        bindings.insert(
            KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE),
            KeyAction::Refresh,
        );

        Self { bindings }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyModifiers};

    #[test]
    fn default_bindings_map_lowercase_w_to_toggle_wrap() {
        let bindings = KeyBindings::default();
        let key_event = KeyEvent::new(KeyCode::Char('w'), KeyModifiers::NONE);

        assert_eq!(
            bindings.get(key_event),
            Some(KeyAction::ToggleWrap),
            "Lowercase 'w' should map to ToggleWrap"
        );
    }

    #[test]
    fn default_bindings_map_uppercase_w_to_toggle_global_wrap() {
        let bindings = KeyBindings::default();
        let key_event = KeyEvent::new(KeyCode::Char('W'), KeyModifiers::SHIFT);

        assert_eq!(
            bindings.get(key_event),
            Some(KeyAction::ToggleGlobalWrap),
            "Uppercase 'W' (shift+w) should map to ToggleGlobalWrap"
        );
    }
}
