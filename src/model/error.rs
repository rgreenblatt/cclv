//! Error types for cclv application.
//!
//! Hierarchical error types using thiserror for clean error handling.

use std::path::PathBuf;
use thiserror::Error;

/// Top-level application error.
#[derive(Debug, Error)]
pub enum AppError {
    #[error("Failed to read input: {0}")]
    InputRead(#[from] InputError),

    #[error("Failed to parse log entry: {0}")]
    Parse(#[from] ParseError),

    #[error("Terminal error: {0}")]
    Terminal(#[from] std::io::Error),
}

/// Input source errors.
#[derive(Debug, Error)]
pub enum InputError {
    #[error("File not found: {path}")]
    FileNotFound { path: PathBuf },

    #[error("File deleted during viewing")]
    FileDeleted,

    #[error("No input source: provide a file path or pipe data to stdin")]
    NoInput,

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// JSONL parsing errors.
#[derive(Debug, Error)]
pub enum ParseError {
    #[error("Invalid JSON at line {line}: {message}")]
    InvalidJson { line: usize, message: String },

    #[error("Missing required field '{field}' at line {line}")]
    MissingField { line: usize, field: &'static str },

    #[error("Invalid timestamp '{raw}' at line {line}")]
    InvalidTimestamp { line: usize, raw: String },
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
