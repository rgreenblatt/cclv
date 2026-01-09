//! View-state layer - Layout, scrolling, and view-state management
//!
//! This module implements the view-state layer for the TUI application,
//! responsible for layout computation, scrolling, visible range calculation,
//! and hit-testing.
//!
//! # Module Structure
//!
//! - `types`: Core newtypes (LineHeight, LineOffset, EntryIndex, SessionIndex, ViewportDimensions)
//! - `layout`: EntryLayout - per-entry layout information
//! - `entry_view`: EntryView - owned entry with layout and view state
//! - `scroll`: ScrollPosition - semantic scroll position enum
//! - `visible_range`: VisibleRange - result of visible range calculation
//! - `hit_test`: HitTestResult - result of mouse hit-testing
//! - `layout_params`: LayoutParams - global layout parameters
//! - `conversation`: ConversationViewState - view-state for single conversation
//! - `session`: SessionViewState - view-state for single session
//! - `log`: LogViewState - top-level view-state for entire log
//! - `height_index`: HeightIndex - O(log n) prefix sums via Fenwick tree
//! - `renderer`: Entry rendering with consistent collapse logic
//! - `token_divider`: Token statistics divider rendering

pub mod conversation;
pub mod entry_view;
pub mod height_index;
pub mod hit_test;
pub mod layout;
pub mod layout_params;
pub mod log;
pub mod renderer;
pub mod scroll;
pub mod session;
pub mod token_divider;
pub mod types;
pub mod visible_range;
