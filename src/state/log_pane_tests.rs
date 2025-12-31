//! Tests for log pane state.

#![cfg(test)]

use super::{LogPaneEntry, LogPaneState};
use chrono::Utc;
use tracing::Level;

/// Helper to create a log entry with specific level and message
fn entry(level: Level, message: &str) -> LogPaneEntry {
    LogPaneEntry {
        timestamp: Utc::now(),
        level,
        message: message.to_string(),
    }
}

#[test]
fn new_creates_empty_state_not_visible() {
    let state = LogPaneState::new(100);

    assert_eq!(state.entries().len(), 0);
    assert!(!state.is_visible());
    assert_eq!(state.unread_count(), 0);
    assert_eq!(state.unread_max_level(), None);
}

#[test]
fn push_adds_entry_to_buffer() {
    let mut state = LogPaneState::new(100);

    state.push(entry(Level::INFO, "first"));

    assert_eq!(state.entries().len(), 1);
    assert_eq!(state.entries()[0].message, "first");
}

#[test]
fn push_maintains_chronological_order() {
    let mut state = LogPaneState::new(100);

    state.push(entry(Level::INFO, "first"));
    state.push(entry(Level::INFO, "second"));
    state.push(entry(Level::INFO, "third"));

    assert_eq!(state.entries().len(), 3);
    assert_eq!(state.entries()[0].message, "first");
    assert_eq!(state.entries()[1].message, "second");
    assert_eq!(state.entries()[2].message, "third");
}

#[test]
fn push_evicts_oldest_when_at_capacity() {
    let mut state = LogPaneState::new(3);

    state.push(entry(Level::INFO, "first"));
    state.push(entry(Level::INFO, "second"));
    state.push(entry(Level::INFO, "third"));
    state.push(entry(Level::INFO, "fourth"));

    assert_eq!(state.entries().len(), 3);
    assert_eq!(state.entries()[0].message, "second");
    assert_eq!(state.entries()[1].message, "third");
    assert_eq!(state.entries()[2].message, "fourth");
}

#[test]
fn push_tracks_unread_count_when_not_visible() {
    let mut state = LogPaneState::new(100);

    state.push(entry(Level::INFO, "first"));
    state.push(entry(Level::INFO, "second"));

    assert_eq!(state.unread_count(), 2);
}

#[test]
fn push_does_not_track_unread_when_visible() {
    let mut state = LogPaneState::new(100);
    state.toggle_visible(); // Make visible

    state.push(entry(Level::INFO, "first"));
    state.push(entry(Level::INFO, "second"));

    assert_eq!(state.unread_count(), 0);
}

#[test]
fn push_tracks_max_level_info() {
    let mut state = LogPaneState::new(100);

    state.push(entry(Level::INFO, "info message"));

    assert_eq!(state.unread_max_level(), Some(Level::INFO));
}

#[test]
fn push_tracks_max_level_warn_over_info() {
    let mut state = LogPaneState::new(100);

    state.push(entry(Level::INFO, "info message"));
    state.push(entry(Level::WARN, "warn message"));

    assert_eq!(state.unread_max_level(), Some(Level::WARN));
}

#[test]
fn push_tracks_max_level_error_over_warn() {
    let mut state = LogPaneState::new(100);

    state.push(entry(Level::WARN, "warn message"));
    state.push(entry(Level::ERROR, "error message"));

    assert_eq!(state.unread_max_level(), Some(Level::ERROR));
}

#[test]
fn push_retains_highest_level_when_lower_added() {
    let mut state = LogPaneState::new(100);

    state.push(entry(Level::ERROR, "error message"));
    state.push(entry(Level::INFO, "info message"));

    assert_eq!(state.unread_max_level(), Some(Level::ERROR));
}

#[test]
fn push_does_not_track_max_level_when_visible() {
    let mut state = LogPaneState::new(100);
    state.toggle_visible(); // Make visible

    state.push(entry(Level::ERROR, "error message"));

    assert_eq!(state.unread_max_level(), None);
}

#[test]
fn toggle_visible_makes_visible_when_not_visible() {
    let mut state = LogPaneState::new(100);

    assert!(!state.is_visible());

    state.toggle_visible();

    assert!(state.is_visible());
}

#[test]
fn toggle_visible_makes_not_visible_when_visible() {
    let mut state = LogPaneState::new(100);
    state.toggle_visible(); // Make visible

    assert!(state.is_visible());

    state.toggle_visible();

    assert!(!state.is_visible());
}

#[test]
fn toggle_visible_clears_unread_count_when_opening() {
    let mut state = LogPaneState::new(100);

    state.push(entry(Level::INFO, "first"));
    state.push(entry(Level::INFO, "second"));

    assert_eq!(state.unread_count(), 2);

    state.toggle_visible(); // Open pane

    assert_eq!(state.unread_count(), 0);
}

#[test]
fn toggle_visible_clears_max_level_when_opening() {
    let mut state = LogPaneState::new(100);

    state.push(entry(Level::ERROR, "error message"));

    assert_eq!(state.unread_max_level(), Some(Level::ERROR));

    state.toggle_visible(); // Open pane

    assert_eq!(state.unread_max_level(), None);
}

#[test]
fn toggle_visible_does_not_clear_unread_when_closing() {
    let mut state = LogPaneState::new(100);
    state.toggle_visible(); // Open

    state.push(entry(Level::INFO, "while open"));

    assert_eq!(state.unread_count(), 0);

    state.toggle_visible(); // Close

    assert_eq!(state.unread_count(), 0); // Should remain 0
}

#[test]
fn unread_accumulates_after_closing() {
    let mut state = LogPaneState::new(100);
    state.toggle_visible(); // Open
    state.toggle_visible(); // Close

    state.push(entry(Level::INFO, "after close"));

    assert_eq!(state.unread_count(), 1);
}

#[test]
fn capacity_zero_maintains_empty_buffer() {
    let mut state = LogPaneState::new(0);

    state.push(entry(Level::INFO, "first"));
    state.push(entry(Level::INFO, "second"));

    assert_eq!(state.entries().len(), 0);
}

#[test]
fn capacity_one_keeps_only_latest() {
    let mut state = LogPaneState::new(1);

    state.push(entry(Level::INFO, "first"));
    state.push(entry(Level::INFO, "second"));
    state.push(entry(Level::INFO, "third"));

    assert_eq!(state.entries().len(), 1);
    assert_eq!(state.entries()[0].message, "third");
}

#[test]
fn unread_count_still_increments_when_capacity_zero() {
    let mut state = LogPaneState::new(0);

    state.push(entry(Level::INFO, "first"));
    state.push(entry(Level::INFO, "second"));

    // Even though entries aren't stored, unread count should track them
    assert_eq!(state.unread_count(), 2);
}
