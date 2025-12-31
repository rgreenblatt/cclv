//! Internal test modules - whitebox tests with crate access
//!
//! This module contains tests that require internal access to crate types.
//! Tests here can access private items and implementation details for
//! comprehensive validation of internal invariants and edge cases.

mod key_action_tests;
mod parse_real_logs;
mod property_tests;
mod tui_integration;
mod view_state_invariants;
mod view_state_us2_expand_collapse;
mod view_state_us3_mouse_hit_testing;

// Harness-based acceptance tests
mod acceptance_scroll;
mod acceptance_stats_session_mismatch;
mod acceptance_us1;
mod acceptance_us2;
mod acceptance_us3;
mod acceptance_us4;
mod acceptance_us5;
mod crash_regression;
mod harness_test;
mod help_overlay_tests;

// Whitebox tests with internal access
mod event_driven_rendering;
mod scroll_properties;
mod stats_multi_scope_tests;
mod tool_block_nowrap_default;
mod view_snapshots;
