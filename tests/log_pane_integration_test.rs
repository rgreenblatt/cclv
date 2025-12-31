//! Integration test for log pane subscriber integration with TuiApp.
//!
//! Tests the end-to-end flow of tracing events being captured and displayed.

use cclv::state::log_pane::LogPaneState;
use std::sync::mpsc;
use tracing::{error, info, warn};

/// Test that init_with_log_pane creates a working subscriber
#[test]
fn init_with_log_pane_sends_log_entries() {
    let (tx, rx) = mpsc::channel();

    // Initialize subscriber (may fail if already initialized, that's okay)
    let _ = cclv::logging::init_with_log_pane(tx);

    // Emit log entries with unique markers
    info!("unique_test_info");
    warn!("unique_test_warn");
    error!("unique_test_error");

    // Small delay to ensure events propagate
    std::thread::sleep(std::time::Duration::from_millis(10));

    // Collect all entries
    let mut entries = Vec::new();
    while let Ok(entry) = rx.try_recv() {
        entries.push(entry);
    }

    // Verify we got at least our entries (may have more from other tests)
    assert!(
        entries.len() >= 3,
        "Should capture at least info, warn, error"
    );

    // Find our specific entries
    let has_info = entries
        .iter()
        .any(|e| e.message.contains("unique_test_info"));
    let has_warn = entries
        .iter()
        .any(|e| e.message.contains("unique_test_warn"));
    let has_error = entries
        .iter()
        .any(|e| e.message.contains("unique_test_error"));

    assert!(has_info, "Should have info entry");
    assert!(has_warn, "Should have warn entry");
    assert!(has_error, "Should have error entry");
}

/// Test that log entries pushed to LogPaneState are tracked correctly
#[test]
fn log_pane_state_receives_entries() {
    let (tx, rx) = mpsc::channel();
    let mut log_pane = LogPaneState::new(100);

    // Simulate sending log entries
    let entry1 = cclv::state::log_pane::LogPaneEntry {
        timestamp: chrono::Utc::now(),
        level: tracing::Level::INFO,
        message: "first message".to_string(),
    };
    let entry2 = cclv::state::log_pane::LogPaneEntry {
        timestamp: chrono::Utc::now(),
        level: tracing::Level::ERROR,
        message: "second message".to_string(),
    };

    tx.send(entry1.clone()).unwrap();
    tx.send(entry2.clone()).unwrap();

    // Poll receiver and push to log pane
    while let Ok(entry) = rx.try_recv() {
        log_pane.push(entry);
    }

    // Verify entries are in log pane
    assert_eq!(log_pane.entries().len(), 2);
    assert_eq!(log_pane.entries()[0].message, "first message");
    assert_eq!(log_pane.entries()[1].message, "second message");
}

/// Test that unread tracking works when log pane is not visible
#[test]
fn log_pane_tracks_unread_entries() {
    let mut log_pane = LogPaneState::new(100);

    // Log pane starts not visible
    assert!(!log_pane.is_visible());

    // Add entries while not visible
    let entry1 = cclv::state::log_pane::LogPaneEntry {
        timestamp: chrono::Utc::now(),
        level: tracing::Level::INFO,
        message: "unread message 1".to_string(),
    };
    let entry2 = cclv::state::log_pane::LogPaneEntry {
        timestamp: chrono::Utc::now(),
        level: tracing::Level::ERROR,
        message: "unread message 2".to_string(),
    };

    log_pane.push(entry1);
    log_pane.push(entry2);

    // Verify unread count and max level
    assert_eq!(log_pane.unread_count(), 2);
    assert_eq!(
        log_pane.unread_max_level(),
        Some(tracing::Level::ERROR),
        "Max level should be ERROR"
    );

    // Make visible - should clear unread tracking
    log_pane.set_visible(true);
    assert_eq!(log_pane.unread_count(), 0);
    assert_eq!(log_pane.unread_max_level(), None);
}

/// Test simulated polling flow: receiver -> log_pane (what TuiApp does)
#[test]
fn simulated_polling_flow() {
    // This simulates what TuiApp.poll_log_entries() does:
    // Poll receiver with try_recv() and push to log pane.

    let (tx, rx) = mpsc::channel();
    let mut log_pane = LogPaneState::new(100);

    // Simulate log entries arriving on the channel
    let entry1 = cclv::state::log_pane::LogPaneEntry {
        timestamp: chrono::Utc::now(),
        level: tracing::Level::INFO,
        message: "simulated log 1".to_string(),
    };
    let entry2 = cclv::state::log_pane::LogPaneEntry {
        timestamp: chrono::Utc::now(),
        level: tracing::Level::WARN,
        message: "simulated log 2".to_string(),
    };

    tx.send(entry1).unwrap();
    tx.send(entry2).unwrap();

    // Poll all available entries (what poll_log_entries does)
    while let Ok(entry) = rx.try_recv() {
        log_pane.push(entry);
    }

    // Verify entries were consumed
    assert_eq!(log_pane.entries().len(), 2);
    assert_eq!(log_pane.entries()[0].message, "simulated log 1");
    assert_eq!(log_pane.entries()[1].message, "simulated log 2");
}
