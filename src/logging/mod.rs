//! Tracing subscriber integration for log pane.
//!
//! Provides a custom `tracing_subscriber::Layer` that captures log events
//! and sends them to the UI thread via a channel for display in the log pane.

use crate::state::log_pane::LogPaneEntry;
use std::sync::mpsc;
use tracing::Subscriber;
use tracing_subscriber::layer::Context;
use tracing_subscriber::Layer;

#[cfg(test)]
#[path = "logging_tests.rs"]
mod tests;

/// Visitor for extracting the message from a tracing event.
#[derive(Default)]
struct MessageVisitor {
    message: String,
}

impl tracing::field::Visit for MessageVisitor {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            self.message = format!("{:?}", value);
            // Remove surrounding quotes from Debug formatting
            if self.message.starts_with('"') && self.message.ends_with('"') {
                self.message = self.message[1..self.message.len() - 1].to_string();
                // Unescape common escape sequences
                self.message = self.message.replace("\\n", "\n");
            }
        }
    }
}

/// A tracing Layer that sends log entries to the UI via a channel.
///
/// This allows tracing output (e.g., `tracing::info!`, `tracing::error!`)
/// to appear in the log pane without blocking the logging thread.
pub struct LogPaneLayer {
    /// Sender for log entries to the UI thread
    sender: mpsc::Sender<LogPaneEntry>,
}

impl LogPaneLayer {
    /// Create a new LogPaneLayer with the given sender.
    ///
    /// # Arguments
    /// * `sender` - Channel sender for log entries
    ///
    /// # Returns
    /// A new `LogPaneLayer` that will send entries via the provided sender.
    pub fn new(sender: mpsc::Sender<LogPaneEntry>) -> Self {
        Self { sender }
    }
}

impl<S> Layer<S> for LogPaneLayer
where
    S: Subscriber,
{
    /// Handle a tracing event by converting it to a LogPaneEntry and sending via channel.
    ///
    /// If the channel send fails (receiver dropped), the event is silently ignored
    /// to satisfy FR-059: errors in logging must not break the main UI flow.
    fn on_event(&self, event: &tracing::Event<'_>, _ctx: Context<'_, S>) {
        // Extract message from the event
        let mut visitor = MessageVisitor::default();
        event.record(&mut visitor);

        // Create log pane entry
        let entry = LogPaneEntry {
            timestamp: chrono::Utc::now(),
            level: *event.metadata().level(),
            message: visitor.message,
        };

        // Send to UI thread, ignoring errors (receiver may be dropped)
        let _ = self.sender.send(entry);
    }
}

/// Initialize the tracing subscriber with a LogPaneLayer and EnvFilter.
///
/// This sets up the global default subscriber to send log entries to the UI.
/// Respects RUST_LOG environment variable, defaults to "info" level.
///
/// # Arguments
/// * `sender` - Channel sender for log entries
///
/// # Returns
/// * `Ok(())` if initialization succeeded
/// * `Err(msg)` if the subscriber was already initialized
pub fn init_with_log_pane(sender: mpsc::Sender<LogPaneEntry>) -> Result<(), String> {
    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::EnvFilter;

    let layer = LogPaneLayer::new(sender);
    let env_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    let subscriber = tracing_subscriber::registry()
        .with(env_filter)
        .with(layer);

    tracing::subscriber::set_global_default(subscriber)
        .map_err(|e| format!("Failed to set global subscriber: {}", e))
}
