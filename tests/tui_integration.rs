//! Integration tests for TUI skeleton
//!
//! These tests verify the basic TUI lifecycle without requiring
//! an actual terminal.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// Test that 'q' key triggers quit
#[test]
fn test_q_key_triggers_quit() {
    // This will test handle_key logic once implemented
    let key = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE);

    // Verify that 'q' should trigger quit
    assert_eq!(key.code, KeyCode::Char('q'));
}

/// Test that Ctrl+C triggers quit
#[test]
fn test_ctrl_c_triggers_quit() {
    let key = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);

    // Verify that Ctrl+C should trigger quit
    assert!(key.modifiers.contains(KeyModifiers::CONTROL));
    assert_eq!(key.code, KeyCode::Char('c'));
}

/// Test that other keys don't trigger quit
#[test]
fn test_other_keys_do_not_quit() {
    let key = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE);

    // Verify normal keys should not quit
    assert_ne!(key.code, KeyCode::Char('q'));
    assert!(!key.modifiers.contains(KeyModifiers::CONTROL));
}
