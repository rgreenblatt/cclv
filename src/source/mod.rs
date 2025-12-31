//! Log input sources.
//!
//! This module provides input sources for JSONL log data:
//! - File tailing for live log following
//! - Stdin for piped input

pub mod file;
pub mod stdin;

pub use file::FileTailer;
pub use stdin::StdinSource;
