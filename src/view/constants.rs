//! Layout dimension constants for TUI rendering.
//!
//! Centralized location for all layout-related numeric values to enable
//! consistent tuning across the application.

/// Height of the tab bar in lines (border + content).
///
/// Used in layout calculations for the tab bar area at the top of conversations.
/// Includes border and title rendering.
pub const TAB_BAR_HEIGHT: u16 = 3;

/// Height of the status bar in lines.
///
/// Used in layout calculations for the status bar at the bottom of the screen.
/// Single line for status text and keyboard hints.
pub const STATUS_BAR_HEIGHT: u16 = 1;

/// Height of the search input widget in lines.
///
/// Used when search is active (typing or showing matches).
/// Includes border and text input area.
pub const SEARCH_INPUT_HEIGHT: u16 = 3;

/// Height of the stats panel in lines.
///
/// Used when stats panel is visible at the bottom of the content area.
/// Fixed height to accommodate token usage, cost, and tool counts.
pub const STATS_PANEL_HEIGHT: u16 = 10;

/// Width percentage for help overlay popup.
///
/// Percentage of screen width (0-100) for the help overlay modal.
pub const HELP_POPUP_WIDTH_PERCENT: u16 = 70;

/// Height percentage for help overlay popup.
///
/// Percentage of screen height (0-100) for the help overlay modal.
pub const HELP_POPUP_HEIGHT_PERCENT: u16 = 80;
