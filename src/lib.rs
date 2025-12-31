//! Claude Code Log Viewer (cclv)
//!
//! TUI application for viewing Claude Code JSONL session logs.
//!
//! This is the library root. Implementation modules will be added
//! in later phases following the Pure Core / Impure Shell architecture.

// Allow deprecated during migration phase (cclv-5ur.6.x)
// Deprecated stubs will be removed by subsequent beads
#![allow(deprecated)]

pub mod config;
pub mod logging;
pub mod model;
pub mod parser;
pub mod source;
pub mod state;
pub mod view;
pub mod view_state;

// Re-export main loop integration
pub mod integration;

#[cfg(test)]
mod test_harness;

#[cfg(test)]
mod tests;
