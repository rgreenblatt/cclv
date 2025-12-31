//! E2E smoke tests for cclv binary
//!
//! These tests verify basic end-to-end functionality by executing the compiled binary.
//! They are gated behind the `e2e-tests` feature flag.
//!
//! Run with: `cargo test --features e2e-tests`

#![cfg(feature = "e2e-tests")]

use std::fs;
use std::path::PathBuf;
use std::time::Duration;

use expectrl::{spawn, ControlCode, Eof, Regex};

/// Helper to find the cclv binary in target directory
fn find_binary() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

    // Try debug first (most common during testing)
    let debug_binary = manifest_dir.join("target/debug/cclv");
    if debug_binary.exists() {
        return debug_binary;
    }

    // Fall back to release
    let release_binary = manifest_dir.join("target/release/cclv");
    if release_binary.exists() {
        return release_binary;
    }

    panic!("cclv binary not found - run `cargo build` first");
}

/// Helper to create a test fixture file with sample JSONL data
fn create_test_fixture(name: &str, content: &str) -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixtures_dir = manifest_dir.join("target/test-fixtures");
    fs::create_dir_all(&fixtures_dir).unwrap();

    let fixture_path = fixtures_dir.join(name);
    fs::write(&fixture_path, content).unwrap();
    fixture_path
}

#[test]
#[ignore] // Ignored by default - requires built binary
fn smoke_help_flag() {
    let binary = find_binary();

    let mut session = spawn(format!("{} --help", binary.display()))
        .expect("Failed to spawn cclv");

    // Should see help output with basic usage
    let _ = session
        .expect(Regex("Usage:"))
        .expect("Failed to find help output");

    // Should see description
    let _ = session
        .expect(Regex("TUI application for viewing Claude Code JSONL session logs"))
        .expect("Failed to find description");

    // Should exit cleanly
    let _ = session.expect(Eof).expect("Process should exit");
}

#[test]
#[ignore] // Ignored by default - requires built binary
fn smoke_version_flag() {
    let binary = find_binary();

    let mut session = spawn(format!("{} --version", binary.display()))
        .expect("Failed to spawn cclv");

    // Should see version output
    let _ = session
        .expect(Regex(r"cclv \d+\.\d+\.\d+"))
        .expect("Failed to find version output");

    // Should exit cleanly
    let _ = session.expect(Eof).expect("Process should exit");
}

#[test]
#[ignore] // Ignored by default - requires built binary
fn smoke_loads_valid_file() {
    let binary = find_binary();

    // Create a minimal valid JSONL fixture
    let content = r#"{"type":"session_started","timestamp":"2024-01-01T12:00:00Z","session_id":"test-session-123"}
{"type":"user_message","timestamp":"2024-01-01T12:00:01Z","session_id":"test-session-123","message_id":"msg-1","content":"Hello"}
{"type":"assistant_message","timestamp":"2024-01-01T12:00:02Z","session_id":"test-session-123","message_id":"msg-2","content":[{"type":"text","text":"Hi there!"}]}
"#;
    let fixture = create_test_fixture("smoke_test.jsonl", content);

    let mut session = spawn(format!("{} {}", binary.display(), fixture.display()))
        .expect("Failed to spawn cclv");

    // Give TUI time to initialize and render
    std::thread::sleep(Duration::from_millis(500));

    // Should be running (not crashed)
    let is_alive = session.is_alive().expect("Failed to check process status");
    assert!(is_alive, "Process should be running");

    // Send quit command (q)
    session
        .send(ControlCode::try_from('q').unwrap())
        .expect("Failed to send quit command");

    // Should exit cleanly
    let _ = session.expect(Eof).expect("Process should exit");
}
