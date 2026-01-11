//! Stdin-based log source for piped input.
//!
//! Provides StdinSource for reading JSONL from stdin with support for
//! both streaming (live) and complete (EOF reached) modes.

use crate::model::error::InputError;
use std::io::{BufRead, BufReader, IsTerminal, Read};
use std::sync::mpsc::{Receiver, TryRecvError, channel};
use std::thread::{self, JoinHandle};

/// Message sent from background reader thread to main thread.
enum ReaderMessage {
    Line(String),
    Eof,
}

/// Stdin source for piped JSONL input.
///
/// Supports both streaming mode (data arriving incrementally, like `tail -f | cclv`)
/// and complete mode (EOF reached, like `cat file.jsonl | cclv`).
///
/// # Design
///
/// - Detects TTY vs piped input at construction
/// - Non-blocking poll() via background thread + channel
/// - Background thread performs blocking read_line() and sends results to channel
/// - Tracks EOF state via `complete` flag
#[derive(Debug)]
pub struct StdinSource {
    rx: Receiver<ReaderMessage>,
    _reader_thread: JoinHandle<()>,
    complete: bool,
}

impl StdinSource {
    /// Create a new StdinSource from stdin.
    ///
    /// Spawns a background thread that performs blocking reads from stdin
    /// and sends lines via a channel. This allows `poll()` to be non-blocking.
    ///
    /// # Errors
    ///
    /// Returns `InputError::NoInput` if stdin is a TTY (interactive terminal).
    /// This prevents the TUI from blocking waiting for user input when the
    /// user forgot to pipe data.
    pub fn new() -> Result<Self, InputError> {
        if Self::is_tty() {
            return Err(InputError::NoInput);
        }

        let (tx, rx) = channel();
        let reader_thread = thread::spawn(move || {
            let stdin = std::io::stdin();
            let mut reader = BufReader::new(stdin);
            let mut buffer = String::new();

            loop {
                buffer.clear();
                match reader.read_line(&mut buffer) {
                    Ok(0) => {
                        // EOF with no data
                        let _ = tx.send(ReaderMessage::Eof);
                        break;
                    }
                    Ok(_) => {
                        // Check if line is complete (has newline)
                        if buffer.ends_with('\n') {
                            // Complete line - strip newline and send
                            let line = buffer.trim_end_matches('\n').to_string();
                            if tx.send(ReaderMessage::Line(line)).is_err() {
                                // Receiver dropped, exit thread
                                break;
                            }
                        } else {
                            // Partial line at EOF - discard it and send EOF
                            let _ = tx.send(ReaderMessage::Eof);
                            break;
                        }
                    }
                    Err(_) => {
                        // I/O error - exit thread
                        break;
                    }
                }
            }
        });

        Ok(Self {
            rx,
            _reader_thread: reader_thread,
            complete: false,
        })
    }

    /// Check if stdin is a TTY (interactive terminal).
    ///
    /// Used internally to detect piped vs interactive stdin.
    fn is_tty() -> bool {
        std::io::stdin().is_terminal()
    }
}

impl StdinSource {
    /// Create StdinSource from any reader (for testing).
    ///
    /// Spawns a background thread to read from the provided reader.
    /// Test-only constructor - bypasses TTY check for testing.
    ///
    /// This is public to support integration tests in the `tests/` directory.
    /// Should not be used in production code.
    pub fn from_reader<R: Read + Send + 'static>(reader: R) -> Self {
        let (tx, rx) = channel();
        let reader_thread = thread::spawn(move || {
            let mut reader = BufReader::new(reader);
            let mut buffer = String::new();

            loop {
                buffer.clear();
                match reader.read_line(&mut buffer) {
                    Ok(0) => {
                        // EOF with no data
                        let _ = tx.send(ReaderMessage::Eof);
                        break;
                    }
                    Ok(_) => {
                        // Check if line is complete (has newline)
                        if buffer.ends_with('\n') {
                            // Complete line - strip newline and send
                            let line = buffer.trim_end_matches('\n').to_string();
                            if tx.send(ReaderMessage::Line(line)).is_err() {
                                // Receiver dropped, exit thread
                                break;
                            }
                        } else {
                            // Partial line at EOF - discard it and send EOF
                            let _ = tx.send(ReaderMessage::Eof);
                            break;
                        }
                    }
                    Err(_) => {
                        // I/O error - exit thread
                        break;
                    }
                }
            }
        });

        Self {
            rx,
            _reader_thread: reader_thread,
            complete: false,
        }
    }

    /// Poll for a new line from stdin (returns raw string).
    ///
    /// **Non-blocking**: returns immediately with `None` if no complete line available.
    /// Returns `Some(line)` when a complete line is ready from the background reader thread.
    ///
    /// Sets `complete` flag to true when EOF is reached.
    ///
    /// # Errors
    ///
    /// Currently infallible - returns `Ok` always. Signature kept for future extensibility.
    pub fn poll(&mut self) -> Result<Option<String>, InputError> {
        match self.rx.try_recv() {
            Ok(ReaderMessage::Line(line)) => Ok(Some(line)),
            Ok(ReaderMessage::Eof) => {
                self.complete = true;
                Ok(None)
            }
            Err(TryRecvError::Empty) => {
                // No data available yet - non-blocking return
                Ok(None)
            }
            Err(TryRecvError::Disconnected) => {
                // Reader thread died - treat as EOF
                self.complete = true;
                Ok(None)
            }
        }
    }

    /// Poll and parse lines into LogEntry vector.
    ///
    /// Drains all available lines from the channel, parses each to LogEntry.
    /// Malformed lines are silently skipped (FR-010).
    ///
    /// # Errors
    ///
    /// Returns `InputError` for I/O errors.
    pub fn poll_and_parse(&mut self) -> Result<Vec<crate::model::LogEntry>, InputError> {
        let mut entries = Vec::new();

        // Drain all available lines from channel
        while let Some(line) = self.poll()? {
            // Skip empty lines
            if line.trim().is_empty() {
                continue;
            }

            // Parse entry - malformed lines are silently skipped (FR-010)
            if let Ok(entry) = crate::model::LogEntry::parse(&line) {
                entries.push(entry);
            }
        }

        Ok(entries)
    }

    /// Check if EOF has been reached (no more data will arrive).
    pub fn is_complete(&self) -> bool {
        self.complete
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    /// Helper to poll with retry for async background thread.
    /// The background thread needs time to read and send data.
    fn poll_with_retry(source: &mut StdinSource, max_attempts: u32) -> Option<String> {
        for _ in 0..max_attempts {
            if let Ok(Some(line)) = source.poll() {
                return Some(line);
            }
            thread::sleep(Duration::from_millis(10));
        }
        source.poll().unwrap()
    }

    #[test]
    fn poll_returns_line_when_data_available() {
        let data = b"{\"line\": 1}\n{\"line\": 2}\n";
        let mut source = StdinSource::from_reader(&data[..]);

        let line1 = poll_with_retry(&mut source, 10);
        assert_eq!(line1, Some("{\"line\": 1}".to_string()));

        let line2 = poll_with_retry(&mut source, 10);
        assert_eq!(line2, Some("{\"line\": 2}".to_string()));
    }

    #[test]
    fn poll_returns_none_at_eof() {
        let data = b"{\"line\": 1}\n";
        let mut source = StdinSource::from_reader(&data[..]);

        // Read the one line
        poll_with_retry(&mut source, 10);

        // Next poll should return None and set complete flag
        thread::sleep(Duration::from_millis(50)); // Give thread time to send EOF
        let result = source.poll().unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn is_complete_true_after_eof() {
        let data = b"{\"line\": 1}\n";
        let mut source = StdinSource::from_reader(&data[..]);

        assert!(!source.is_complete(), "Should not be complete initially");

        // Read until EOF
        poll_with_retry(&mut source, 10);

        // Wait for EOF message from thread
        thread::sleep(Duration::from_millis(50));
        source.poll().unwrap();

        assert!(source.is_complete(), "Should be complete after EOF");
    }

    #[test]
    fn poll_handles_partial_lines() {
        // Data without trailing newline - background thread will send the complete line
        // but not the partial one (it waits for newline)
        let data = b"{\"line\": 1}\n{\"partial";
        let mut source = StdinSource::from_reader(&data[..]);

        // Should read the complete line
        let line1 = poll_with_retry(&mut source, 10);
        assert_eq!(line1, Some("{\"line\": 1}".to_string()));

        // Partial line won't be sent until newline arrives (or EOF)
        // Give thread time to reach EOF and send it
        thread::sleep(Duration::from_millis(50));
        let line2 = source.poll().unwrap();
        assert_eq!(line2, None);
    }

    #[test]
    fn poll_returns_none_for_empty_input() {
        let data = b"";
        let mut source = StdinSource::from_reader(&data[..]);

        // Give thread time to detect EOF
        thread::sleep(Duration::from_millis(50));

        let result = source.poll().unwrap();
        assert_eq!(result, None);
        assert!(source.is_complete(), "Empty input should be complete");
    }

    #[test]
    fn poll_strips_newline_from_result() {
        let data = b"line with newline\n";
        let mut source = StdinSource::from_reader(&data[..]);

        let line = poll_with_retry(&mut source, 10);
        assert_eq!(line, Some("line with newline".to_string()));
        assert!(
            !line.as_ref().unwrap().contains('\n'),
            "Should not include newline"
        );
    }

    #[test]
    fn poll_handles_multiple_consecutive_calls() {
        let data = b"line1\nline2\nline3\n";
        let mut source = StdinSource::from_reader(&data[..]);

        assert_eq!(poll_with_retry(&mut source, 10), Some("line1".to_string()));
        assert_eq!(poll_with_retry(&mut source, 10), Some("line2".to_string()));
        assert_eq!(poll_with_retry(&mut source, 10), Some("line3".to_string()));

        // Wait for EOF
        thread::sleep(Duration::from_millis(50));
        assert_eq!(source.poll().unwrap(), None);
        assert!(source.is_complete());
    }

    #[test]
    fn poll_is_non_blocking() {
        // This test verifies that poll() returns immediately
        use std::time::Instant;

        let data = b""; // Empty - will send EOF immediately
        let mut source = StdinSource::from_reader(&data[..]);

        let start = Instant::now();
        // First poll should return None immediately (no data ready yet)
        let result = source.poll().unwrap();
        let elapsed = start.elapsed();

        // Should return in well under 10ms (non-blocking)
        assert!(
            elapsed.as_millis() < 10,
            "poll() took {}ms, should be < 10ms",
            elapsed.as_millis()
        );

        // Result could be None (no data yet) or None+complete (EOF already received)
        // Both are valid for this timing-sensitive test
        assert_eq!(result, None);
    }
}
