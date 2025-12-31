//! Tests for tracing Layer integration.

use super::*;
use std::sync::mpsc;
use tracing::Level;

#[test]
fn new_creates_layer_with_sender() {
    // GIVEN a channel for log entries
    let (tx, _rx) = mpsc::channel();

    // WHEN creating a new LogPaneLayer
    let _layer = LogPaneLayer::new(tx);

    // THEN the layer is created successfully
    // Type system proves this - if it compiles, it worked
}

#[test]
fn on_event_sends_info_level_entry() {
    // GIVEN a layer with a channel receiver
    let (tx, rx) = mpsc::channel();
    let layer = LogPaneLayer::new(tx);

    // WHEN a tracing INFO event is emitted
    use tracing_subscriber::layer::SubscriberExt;
    let subscriber = tracing_subscriber::registry().with(layer);
    tracing::subscriber::with_default(subscriber, || {
        tracing::info!("test info message");
    });

    // THEN a LogPaneEntry is received
    let entry = rx.try_recv().expect("should receive entry from channel");

    // AND it has INFO level
    assert_eq!(entry.level, Level::INFO);

    // AND it contains the message
    assert_eq!(entry.message, "test info message");
}

#[test]
fn on_event_sends_error_level_entry() {
    // GIVEN a layer with a channel receiver
    let (tx, rx) = mpsc::channel();
    let layer = LogPaneLayer::new(tx);

    // WHEN a tracing ERROR event is emitted
    use tracing_subscriber::layer::SubscriberExt;
    let subscriber = tracing_subscriber::registry().with(layer);
    tracing::subscriber::with_default(subscriber, || {
        tracing::error!("test error message");
    });

    // THEN a LogPaneEntry is received with ERROR level
    let entry = rx.try_recv().expect("should receive entry from channel");
    assert_eq!(entry.level, Level::ERROR);
    assert_eq!(entry.message, "test error message");
}

#[test]
fn on_event_handles_dropped_receiver_gracefully() {
    // GIVEN a layer with a channel, but receiver is dropped
    let (tx, rx) = mpsc::channel();
    let layer = LogPaneLayer::new(tx);
    drop(rx); // Drop receiver before logging

    // WHEN a tracing event is emitted
    use tracing_subscriber::layer::SubscriberExt;
    let subscriber = tracing_subscriber::registry().with(layer);
    tracing::subscriber::with_default(subscriber, || {
        tracing::info!("this should not panic");
    });

    // THEN no panic occurs (FR-059: errors must not break UI flow)
    // Test passes if we reach here without panic
}

#[test]
fn on_event_captures_timestamp() {
    // GIVEN a layer with a channel receiver
    let (tx, rx) = mpsc::channel();
    let layer = LogPaneLayer::new(tx);

    // WHEN a tracing event is emitted
    let before = chrono::Utc::now();
    use tracing_subscriber::layer::SubscriberExt;
    let subscriber = tracing_subscriber::registry().with(layer);
    tracing::subscriber::with_default(subscriber, || {
        tracing::warn!("timestamped message");
    });
    let after = chrono::Utc::now();

    // THEN the entry timestamp is between before and after
    let entry = rx.try_recv().expect("should receive entry");
    assert!(entry.timestamp >= before);
    assert!(entry.timestamp <= after);
}

#[test]
fn on_event_handles_multiline_messages() {
    // GIVEN a layer with a channel receiver
    let (tx, rx) = mpsc::channel();
    let layer = LogPaneLayer::new(tx);

    // WHEN a tracing event with multiline message is emitted
    use tracing_subscriber::layer::SubscriberExt;
    let subscriber = tracing_subscriber::registry().with(layer);
    tracing::subscriber::with_default(subscriber, || {
        tracing::debug!("line one\nline two\nline three");
    });

    // THEN the entry contains the full multiline message
    let entry = rx.try_recv().expect("should receive entry");
    assert_eq!(entry.message, "line one\nline two\nline three");
}

#[test]
fn on_event_handles_formatted_messages() {
    // GIVEN a layer with a channel receiver
    let (tx, rx) = mpsc::channel();
    let layer = LogPaneLayer::new(tx);

    // WHEN a tracing event with formatted args is emitted
    use tracing_subscriber::layer::SubscriberExt;
    let subscriber = tracing_subscriber::registry().with(layer);
    tracing::subscriber::with_default(subscriber, || {
        tracing::info!("value: {}, count: {}", 42, 7);
    });

    // THEN the entry contains the formatted message
    let entry = rx.try_recv().expect("should receive entry");
    assert_eq!(entry.message, "value: 42, count: 7");
}

#[test]
fn multiple_events_send_multiple_entries() {
    // GIVEN a layer with a channel receiver
    let (tx, rx) = mpsc::channel();
    let layer = LogPaneLayer::new(tx);

    // WHEN multiple tracing events are emitted
    use tracing_subscriber::layer::SubscriberExt;
    let subscriber = tracing_subscriber::registry().with(layer);
    tracing::subscriber::with_default(subscriber, || {
        tracing::info!("first");
        tracing::warn!("second");
        tracing::error!("third");
    });

    // THEN all entries are received in order
    let first = rx.try_recv().expect("should receive first");
    assert_eq!(first.message, "first");
    assert_eq!(first.level, Level::INFO);

    let second = rx.try_recv().expect("should receive second");
    assert_eq!(second.message, "second");
    assert_eq!(second.level, Level::WARN);

    let third = rx.try_recv().expect("should receive third");
    assert_eq!(third.message, "third");
    assert_eq!(third.level, Level::ERROR);
}
