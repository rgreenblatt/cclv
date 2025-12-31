//! Error types for cclv application.
//!
//! This module defines a hierarchical error taxonomy using `thiserror` for structured error
//! handling. Errors follow the Railway-Oriented Programming pattern, composing cleanly via
//! `?` and `From` conversions.
//!
//! # Error Hierarchy
//!
//! - [`AppError`] - Top-level application error wrapping all domain-specific failures
//!   - [`InputError`] - Log file/stdin reading failures (file not found, deleted, IO)
//!   - [`ParseError`] - JSONL parsing failures (malformed JSON, missing fields, bad timestamps)
//!   - `std::io::Error` - Terminal/TUI rendering failures
//!
//! # Error Recovery Strategy
//!
//! Per FR-010 requirement, parsing errors are **non-fatal**: malformed JSONL lines are logged
//! to the logging pane and skipped, allowing the UI to remain functional with partial data.
//! Input and terminal errors are fatal and propagate to the top-level error handler.
//!
//! # Design Principles
//!
//! - **Total functions**: All error variants provide actionable context (paths, line numbers)
//! - **Type safety**: No stringly-typed errors; each variant carries structured data
//! - **Composability**: `From` impls enable seamless `?` operator usage
//! - **Railway programming**: Error context propagates up the call stack without manual mapping

use std::path::PathBuf;
use thiserror::Error;

/// Top-level application error encompassing all failure modes.
///
/// This is the unified error type returned from main application logic. All domain-specific
/// error types (`InputError`, `ParseError`) automatically convert to `AppError` via `From`
/// implementations, enabling clean error propagation with the `?` operator.
///
/// # Recovery Behavior
///
/// - **InputRead/Terminal errors**: Fatal - propagate to main loop for graceful shutdown
/// - **Parse errors**: Non-fatal - logged to logging pane, execution continues (see FR-010)
///
/// # Examples
///
/// ```no_run
/// use cclv::model::error::{AppError, InputError};
///
/// fn run_app() -> Result<(), AppError> {
///     // InputError automatically converts to AppError via From
///     let _input = read_log_file()?;
///     Ok(())
/// }
/// # fn read_log_file() -> Result<(), InputError> { Ok(()) }
/// ```
#[derive(Debug, Error)]
pub enum AppError {
    /// Failed to read input from file or stdin.
    ///
    /// This indicates a fundamental inability to access the log data source. Common causes
    /// include file not found, permission denied, or the file being deleted during live
    /// viewing. This is a **fatal error** - the application cannot proceed without input.
    ///
    /// **Recovery**: Display error to user and exit gracefully. For live-follow mode
    /// (FR-139), consider offering retry if file was deleted.
    #[error("Failed to read input: {0}")]
    InputRead(#[from] InputError),

    /// Failed to parse a JSONL log entry.
    ///
    /// This indicates malformed JSON, missing required fields, or invalid data in a log line.
    /// Per FR-010, this is a **non-fatal error**: the invalid line is logged to the logging
    /// pane with line number and error details, then parsing continues with the next line.
    ///
    /// **Recovery**: Log error to logging pane, increment error badge count in status bar,
    /// skip the malformed line, continue processing. The UI remains functional with partial
    /// data.
    #[error("Failed to parse log entry: {0}")]
    Parse(#[from] ParseError),

    /// Terminal or TUI rendering error.
    ///
    /// This indicates failures in the crossterm/ratatui layer, such as terminal resize
    /// failures, broken pipes, or I/O errors during rendering. This is a **fatal error** -
    /// without a working terminal, the TUI cannot function.
    ///
    /// **Recovery**: Attempt graceful terminal cleanup, then exit. Error message should
    /// be written to stderr before exiting.
    #[error("Terminal error: {0}")]
    Terminal(#[from] std::io::Error),
}

/// Errors encountered when reading log input from files or stdin.
///
/// Input sources (file paths, stdin pipes) can fail in multiple distinct ways. This enum
/// captures all input-related failure modes with sufficient context for error reporting
/// and recovery decisions.
///
/// # Recovery Patterns
///
/// - **FileNotFound**: Display error and exit (user provided invalid path)
/// - **FileDeleted**: For live-follow mode, optionally retry after delay
/// - **NoInput**: Display usage help - user must provide file path or pipe stdin
/// - **Io**: Generic I/O failures (permissions, disk errors) - display and exit
///
/// # Design Notes
///
/// This type distinguishes specific failure modes (file not found vs deleted vs no input)
/// rather than collapsing them into generic I/O errors. This enables targeted error
/// messages and recovery logic.
#[derive(Debug, Error)]
pub enum InputError {
    /// The specified log file does not exist at the given path.
    ///
    /// This occurs when the user provides a file path argument that doesn't exist in the
    /// filesystem. The `path` field contains the full path that was attempted, enabling
    /// precise error reporting.
    ///
    /// **When this occurs**: At application startup when opening the log file, or when
    /// attempting to re-open after FR-139 file deletion event.
    ///
    /// **Recovery**: Display error showing the full path, suggest checking the path or
    /// using `--help` for usage. Exit with non-zero status.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::path::PathBuf;
    /// use cclv::model::error::InputError;
    ///
    /// let err = InputError::FileNotFound {
    ///     path: PathBuf::from("/tmp/missing.jsonl")
    /// };
    /// assert!(err.to_string().contains("/tmp/missing.jsonl"));
    /// ```
    #[error("File not found: {path}")]
    FileNotFound {
        /// The filesystem path that was not found.
        ///
        /// This is the full path (as provided by the user or resolved from arguments) that
        /// failed to open. Used for error display and logging.
        path: PathBuf,
    },

    /// The log file was deleted while being actively viewed in live-follow mode.
    ///
    /// This occurs during FR-007 live tailing when the file being followed is removed from
    /// the filesystem (e.g., user runs `rm` on the log file, or Claude Code cleans up old
    /// sessions). Per FR-139, this should show an error notification and stop following.
    ///
    /// **When this occurs**: During file watching (notify crate) when the watched file is
    /// removed or moved.
    ///
    /// **Recovery**: Display error notification in status bar or logging pane. Stop file
    /// watching. Optionally offer retry mechanism if file reappears.
    ///
    /// **Design note**: This is distinct from `FileNotFound` - deletion during viewing is
    /// a different user experience than initial file not found, warranting separate handling.
    #[error("File deleted during viewing")]
    FileDeleted,

    /// No input source was provided - user must supply a file path or pipe stdin.
    ///
    /// This occurs when the application is invoked without arguments and stdin is not a
    /// pipe (e.g., running `cclv` with no arguments in an interactive terminal). Per FR-041,
    /// the application supports both file path arguments and piped stdin input.
    ///
    /// **When this occurs**: At application startup when parsing CLI arguments, if no file
    /// path is provided and stdin is a TTY (not a pipe).
    ///
    /// **Recovery**: Display usage help showing both invocation modes:
    /// - `cclv /path/to/log.jsonl` (file mode)
    /// - `cat log.jsonl | cclv` (stdin mode)
    ///
    /// Exit with non-zero status.
    ///
    /// # Examples
    ///
    /// ```
    /// use cclv::model::error::InputError;
    ///
    /// let err = InputError::NoInput;
    /// let msg = err.to_string();
    /// assert!(msg.contains("file path or pipe data to stdin"));
    /// ```
    #[error("No input source: provide a file path or pipe data to stdin")]
    NoInput,

    /// Generic I/O error reading from input source.
    ///
    /// This captures all other I/O failures not covered by specific variants: permission
    /// denied, disk read errors, broken pipes when reading from stdin, etc. The wrapped
    /// `std::io::Error` provides detailed error information from the OS.
    ///
    /// **When this occurs**: During file opening, stdin reading, or file watching operations
    /// when underlying I/O operations fail.
    ///
    /// **Recovery**: Display the I/O error details to user. These are typically unrecoverable
    /// (disk failure, permission issues) - exit gracefully after showing error.
    ///
    /// **Automatic conversion**: The `#[from]` attribute enables automatic conversion from
    /// `std::io::Error` to `InputError::Io`, allowing clean error propagation:
    ///
    /// ```no_run
    /// use std::fs::File;
    /// use cclv::model::error::InputError;
    ///
    /// fn open_log(path: &str) -> Result<File, InputError> {
    ///     // io::Error automatically converts to InputError::Io via ?
    ///     Ok(File::open(path)?)
    /// }
    /// ```
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Errors encountered when parsing JSONL log entries.
///
/// Per FR-010, parsing errors are **non-fatal**: malformed lines are logged to the logging
/// pane and skipped, allowing the application to display partial data. This enum captures
/// all parsing failure modes with line numbers for precise error reporting.
///
/// # Recovery Strategy
///
/// When any `ParseError` occurs during log parsing:
/// 1. Log error to logging pane with line number and details
/// 2. Increment error badge count in status bar (color-coded red)
/// 3. Skip the malformed line
/// 4. Continue parsing subsequent lines
///
/// This ensures the UI remains functional even with corrupted or incomplete log files.
///
/// # Design Notes
///
/// All variants include `line: usize` for error reporting - users need to know which line
/// in the JSONL file is malformed. Additional context (raw values, field names) helps users
/// diagnose and fix log generation issues.
#[derive(Debug, Error)]
pub enum ParseError {
    /// A log line contains syntactically invalid JSON.
    ///
    /// This occurs when a line in the JSONL file cannot be parsed as valid JSON by
    /// `serde_json`. Common causes include:
    /// - Truncated output (incomplete write before crash)
    /// - Corrupted file contents
    /// - Mixed encoding issues
    /// - Non-JSON content accidentally written to log
    ///
    /// **When this occurs**: During initial log loading (FR-008) or live tailing (FR-007)
    /// when parsing each JSONL line with `serde_json::from_str`.
    ///
    /// **Recovery**: Log to logging pane showing line number and JSON parser error message.
    /// Skip this line. Continue processing remaining lines.
    ///
    /// **Why `message` is `String` not `serde_json::Error`**: We extract the parser error
    /// message rather than wrapping the full error to avoid carrying `serde_json` error
    /// state through the application. The message provides sufficient context for users.
    ///
    /// # Examples
    ///
    /// ```
    /// use cclv::model::error::ParseError;
    ///
    /// let err = ParseError::InvalidJson {
    ///     line: 42,
    ///     message: "unexpected character '}' at position 15".to_string()
    /// };
    /// assert!(err.to_string().contains("line 42"));
    /// assert!(err.to_string().contains("unexpected character"));
    /// ```
    #[error("Invalid JSON at line {line}: {message}")]
    InvalidJson {
        /// The 1-based line number in the JSONL file where parsing failed.
        ///
        /// Used for error reporting in the logging pane. Line numbers match what users see
        /// in text editors, enabling quick navigation to problematic lines.
        line: usize,

        /// The JSON parser error message describing what went wrong.
        ///
        /// Extracted from `serde_json::Error::to_string()`. Contains details like "unexpected
        /// character", "unexpected EOF", "invalid escape sequence", etc.
        message: String,
    },

    /// A JSON object is missing a required field for the Claude Code JSONL schema.
    ///
    /// This occurs when the JSON is syntactically valid but semantically incomplete - a
    /// required field like "uuid", "timestamp", "type", or "agentId" is missing from the
    /// JSON object. This indicates the log file doesn't conform to the Claude Code JSONL
    /// format.
    ///
    /// **When this occurs**: During field extraction after successful JSON parsing, when
    /// calling `.get("field_name")` on the parsed object returns `None`.
    ///
    /// **Recovery**: Log to logging pane showing which field is missing and the line number.
    /// Skip this entry. Continue parsing - other entries may be valid.
    ///
    /// **Why `field` is `&'static str`**: Field names are compile-time constants from the
    /// JSONL schema ("uuid", "timestamp", etc.), not runtime strings. Static lifetime avoids
    /// allocations and cloning.
    ///
    /// # Examples
    ///
    /// ```
    /// use cclv::model::error::ParseError;
    ///
    /// let err = ParseError::MissingField {
    ///     line: 15,
    ///     field: "timestamp"
    /// };
    /// assert!(err.to_string().contains("'timestamp'"));
    /// assert!(err.to_string().contains("line 15"));
    /// ```
    #[error("Missing required field '{field}' at line {line}")]
    MissingField {
        /// The 1-based line number where the incomplete JSON object was found.
        ///
        /// Used for error reporting. Helps users locate malformed log entries.
        line: usize,

        /// The name of the missing required field.
        ///
        /// Common values: "uuid", "timestamp", "type", "agentId", "model", "content".
        /// This is the JSON key that was expected but not present in the object.
        field: &'static str,
    },

    /// A timestamp field contains a value that cannot be parsed as a valid timestamp.
    ///
    /// This occurs when the "timestamp" field exists but contains a value that doesn't
    /// conform to the expected timestamp format (ISO 8601, Unix epoch, etc.). This indicates
    /// clock issues, serialization bugs, or corrupted data.
    ///
    /// **When this occurs**: During timestamp parsing when converting the string or number
    /// value to a `Timestamp` domain type.
    ///
    /// **Recovery**: Log to logging pane showing the invalid timestamp value and line number.
    /// Options:
    /// - Skip the entry (lose temporal ordering info)
    /// - Use a fallback timestamp (e.g., Unix epoch, previous entry's time)
    /// - Mark entry as "unknown timestamp" in UI
    ///
    /// **Why `raw` is `String`**: Preserving the exact invalid value helps users diagnose
    /// timestamp formatting issues in their log generation pipeline.
    ///
    /// # Examples
    ///
    /// ```
    /// use cclv::model::error::ParseError;
    ///
    /// let err = ParseError::InvalidTimestamp {
    ///     line: 8,
    ///     raw: "not-a-timestamp".to_string()
    /// };
    /// assert!(err.to_string().contains("'not-a-timestamp'"));
    /// assert!(err.to_string().contains("line 8"));
    /// ```
    #[error("Invalid timestamp '{raw}' at line {line}")]
    InvalidTimestamp {
        /// The 1-based line number containing the invalid timestamp.
        ///
        /// Used for error reporting and debugging.
        line: usize,

        /// The raw timestamp value that failed to parse.
        ///
        /// This is the exact string or number value from the JSON "timestamp" field,
        /// preserved for diagnostic purposes. Examples: "not-a-date", "99999999999999",
        /// "2025-13-45T99:99:99Z".
        raw: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io;

    #[test]
    fn input_error_file_not_found_display() {
        let err = InputError::FileNotFound {
            path: PathBuf::from("/tmp/missing.jsonl"),
        };
        let msg = err.to_string();
        assert!(msg.contains("File not found"));
        assert!(msg.contains("/tmp/missing.jsonl"));
    }

    #[test]
    fn input_error_file_deleted_display() {
        let err = InputError::FileDeleted;
        let msg = err.to_string();
        assert_eq!(msg, "File deleted during viewing");
    }

    #[test]
    fn input_error_no_input_display() {
        let err = InputError::NoInput;
        let msg = err.to_string();
        assert!(msg.contains("No input source"));
        assert!(msg.contains("file path or pipe data to stdin"));
    }

    #[test]
    fn input_error_io_conversion() {
        let io_err = io::Error::new(io::ErrorKind::PermissionDenied, "access denied");
        let input_err: InputError = io_err.into();
        let msg = input_err.to_string();
        assert!(msg.contains("IO error"));
        assert!(msg.contains("access denied"));
    }

    #[test]
    fn parse_error_invalid_json_display() {
        let err = ParseError::InvalidJson {
            line: 42,
            message: "unexpected character '}'".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("Invalid JSON"));
        assert!(msg.contains("line 42"));
        assert!(msg.contains("unexpected character '}'"));
    }

    #[test]
    fn parse_error_missing_field_display() {
        let err = ParseError::MissingField {
            line: 15,
            field: "timestamp",
        };
        let msg = err.to_string();
        assert!(msg.contains("Missing required field"));
        assert!(msg.contains("'timestamp'"));
        assert!(msg.contains("line 15"));
    }

    #[test]
    fn parse_error_invalid_timestamp_display() {
        let err = ParseError::InvalidTimestamp {
            line: 8,
            raw: "not-a-timestamp".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("Invalid timestamp"));
        assert!(msg.contains("'not-a-timestamp'"));
        assert!(msg.contains("line 8"));
    }

    #[test]
    fn app_error_from_input_error() {
        let input_err = InputError::NoInput;
        let app_err: AppError = input_err.into();
        let msg = app_err.to_string();
        assert!(msg.contains("Failed to read input"));
        assert!(msg.contains("No input source"));
    }

    #[test]
    fn app_error_from_parse_error() {
        let parse_err = ParseError::MissingField {
            line: 10,
            field: "uuid",
        };
        let app_err: AppError = parse_err.into();
        let msg = app_err.to_string();
        assert!(msg.contains("Failed to parse log entry"));
        assert!(msg.contains("Missing required field"));
        assert!(msg.contains("'uuid'"));
    }

    #[test]
    fn app_error_from_io_error() {
        let io_err = io::Error::new(io::ErrorKind::BrokenPipe, "pipe broken");
        let app_err: AppError = io_err.into();
        let msg = app_err.to_string();
        assert!(msg.contains("Terminal error"));
        assert!(msg.contains("pipe broken"));
    }

    #[test]
    fn app_error_nested_io_through_input_error() {
        let io_err = io::Error::new(io::ErrorKind::NotFound, "file not found");
        let input_err: InputError = io_err.into();
        let app_err: AppError = input_err.into();
        let msg = app_err.to_string();
        assert!(msg.contains("Failed to read input"));
        assert!(msg.contains("IO error"));
        assert!(msg.contains("file not found"));
    }

    #[test]
    fn parse_error_preserves_line_numbers() {
        let errors = vec![
            ParseError::InvalidJson {
                line: 1,
                message: "msg1".to_string(),
            },
            ParseError::MissingField {
                line: 100,
                field: "test",
            },
            ParseError::InvalidTimestamp {
                line: 9999,
                raw: "bad".to_string(),
            },
        ];

        for err in errors {
            let msg = err.to_string();
            match err {
                ParseError::InvalidJson { line, .. } => {
                    assert!(msg.contains(&format!("line {}", line)));
                }
                ParseError::MissingField { line, .. } => {
                    assert!(msg.contains(&format!("line {}", line)));
                }
                ParseError::InvalidTimestamp { line, .. } => {
                    assert!(msg.contains(&format!("line {}", line)));
                }
            }
        }
    }
}
