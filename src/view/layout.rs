//! Split pane layout rendering.
//!
//! Pure layout logic - calculates layout constraints and renders
//! placeholder widgets for main agent, subagent tabs, and status bar.

use crate::state::AppState;
use ratatui::{
    layout::{Constraint, Rect},
    Frame,
};

/// Render the split pane layout with main agent (left), subagent tabs (right),
/// and status bar (bottom).
///
/// When session has no subagents, right pane is hidden and left pane takes full width.
pub fn render_layout(_frame: &mut Frame, _state: &AppState) {
    todo!("render_layout")
}

/// Calculate the horizontal split constraints based on subagent presence.
///
/// Returns (main_pane_width, subagent_pane_width):
/// - With subagents: (60%, 40%)
/// - Without subagents: (100%, 0%)
fn calculate_horizontal_constraints(_has_subagents: bool) -> (Constraint, Constraint) {
    todo!("calculate_horizontal_constraints")
}

/// Render the main agent pane with placeholder content.
fn render_main_pane(_frame: &mut Frame, _area: Rect, _state: &AppState) {
    todo!("render_main_pane")
}

/// Render the subagent tabs pane with placeholder content.
fn render_subagent_pane(_frame: &mut Frame, _area: Rect, _state: &AppState) {
    todo!("render_subagent_pane")
}

/// Render the status bar with hints and live mode indicator.
fn render_status_bar(_frame: &mut Frame, _area: Rect, _state: &AppState) {
    todo!("render_status_bar")
}

// ===== Tests =====

#[cfg(test)]
#[path = "layout_tests.rs"]
mod tests;
