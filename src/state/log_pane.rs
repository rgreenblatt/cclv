//! Log pane state for toggleable internal logging display.
//!
//! Provides a ring buffer for capturing log entries with unread tracking.

use std::collections::VecDeque;

#[cfg(test)]
#[path = "log_pane_tests.rs"]
mod tests;

/// A single log entry captured for display in the log pane.
#[derive(Debug, Clone)]
pub struct LogPaneEntry {
    /// When the log entry was created
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// Severity level of the log entry
    pub level: tracing::Level,
    /// The log message text
    pub message: String,
}

/// State for the toggleable log pane.
///
/// Maintains a ring buffer of log entries with capacity limiting
/// and unread tracking when the pane is not visible.
#[derive(Debug)]
pub struct LogPaneState {
    /// Ring buffer of log entries (oldest at front, newest at back)
    entries: VecDeque<LogPaneEntry>,
    /// Maximum entries to retain (configurable)
    capacity: usize,
    /// Count of unread entries since pane was last opened
    unread_count: usize,
    /// Highest severity among unread entries
    unread_max_level: Option<tracing::Level>,
    /// Whether the pane is currently visible
    visible: bool,
}

impl LogPaneState {
    /// Create a new log pane state with the given capacity.
    ///
    /// # Arguments
    /// * `capacity` - Maximum number of log entries to retain
    ///
    /// # Returns
    /// A new `LogPaneState` initialized as not visible with no entries.
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: VecDeque::with_capacity(capacity),
            capacity,
            unread_count: 0,
            unread_max_level: None,
            visible: false,
        }
    }

    /// Add a new log entry to the pane.
    ///
    /// If at capacity, the oldest entry is removed before adding the new one.
    /// If the pane is not visible, increments unread count and updates unread max level.
    ///
    /// # Arguments
    /// * `entry` - The log entry to add
    pub fn push(&mut self, entry: LogPaneEntry) {
        // If at capacity and capacity > 0, remove oldest entry
        if self.capacity > 0 && self.entries.len() >= self.capacity {
            self.entries.pop_front();
        }

        // Track unread if not visible
        if !self.visible {
            self.unread_count += 1;
            self.unread_max_level = match self.unread_max_level {
                None => Some(entry.level),
                Some(existing) => Some(max_level(existing, entry.level)),
            };
        }

        // Add entry if capacity allows
        if self.capacity > 0 {
            self.entries.push_back(entry);
        }
    }

    /// Toggle the visibility of the log pane.
    ///
    /// When toggled to visible, resets unread count and unread max level.
    pub fn toggle_visible(&mut self) {
        self.visible = !self.visible;

        // Clear unread tracking when opening (becoming visible)
        if self.visible {
            self.unread_count = 0;
            self.unread_max_level = None;
        }
    }

    /// Check if the log pane is currently visible.
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Get the count of unread entries.
    pub fn unread_count(&self) -> usize {
        self.unread_count
    }

    /// Get the highest severity level among unread entries.
    pub fn unread_max_level(&self) -> Option<tracing::Level> {
        self.unread_max_level
    }

    /// Get all log entries (oldest to newest).
    pub fn entries(&self) -> &VecDeque<LogPaneEntry> {
        &self.entries
    }
}

/// Compare two log levels and return the higher severity.
///
/// Level ordering from lowest to highest: TRACE < DEBUG < INFO < WARN < ERROR
fn max_level(a: tracing::Level, b: tracing::Level) -> tracing::Level {
    use tracing::Level;

    // Use numeric comparison - lower number = higher severity in tracing
    match (a, b) {
        (Level::ERROR, _) | (_, Level::ERROR) => Level::ERROR,
        (Level::WARN, _) | (_, Level::WARN) => Level::WARN,
        (Level::INFO, _) | (_, Level::INFO) => Level::INFO,
        (Level::DEBUG, _) | (_, Level::DEBUG) => Level::DEBUG,
        _ => Level::TRACE,
    }
}
