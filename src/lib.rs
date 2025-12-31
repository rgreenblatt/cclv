//! Claude Code Log Viewer (cclv)
//!
//! TUI application for viewing Claude Code JSONL session logs.
//!
//! This is the library root. Implementation modules will be added
//! in later phases following the Pure Core / Impure Shell architecture.

pub mod model;
pub mod parser;
pub mod source;
pub mod state;
pub mod view;
