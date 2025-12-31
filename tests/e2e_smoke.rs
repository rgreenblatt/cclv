//! E2E smoke tests for cclv binary
//!
//! These tests verify basic end-to-end functionality by executing the compiled binary.
//! They are gated behind the `e2e-tests` feature flag.
//!
//! Run with: `cargo test --features e2e-tests`

#![cfg(feature = "e2e-tests")]

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

#[test]
fn smoke_help_flag() {
    let binary = find_binary();

    let mut session = spawn(format!("{} --help", binary.display())).expect("Failed to spawn cclv");

    // Should see description first
    let _ = session
        .expect(Regex(
            "TUI application for viewing Claude Code JSONL session logs",
        ))
        .expect("Failed to find description");

    // Should see usage after description
    let _ = session
        .expect(Regex("Usage:"))
        .expect("Failed to find help output");

    // Should exit cleanly
    let _ = session.expect(Eof).expect("Process should exit");
}

#[test]
fn smoke_version_flag() {
    let binary = find_binary();

    let mut session =
        spawn(format!("{} --version", binary.display())).expect("Failed to spawn cclv");

    // Should see version output
    let _ = session
        .expect(Regex(r"cclv \d+\.\d+\.\d+"))
        .expect("Failed to find version output");

    // Should exit cleanly
    let _ = session.expect(Eof).expect("Process should exit");
}

#[test]
fn smoke_loads_valid_file() {
    let binary = find_binary();
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixture = manifest_dir.join("tests/fixtures/minimal_session.jsonl");

    let mut session =
        spawn(format!("{} {}", binary.display(), fixture.display())).expect("Failed to spawn cclv");

    // Give TUI time to initialize and render
    std::thread::sleep(Duration::from_millis(500));

    // Should be running (not crashed)
    let is_alive = session.is_alive().expect("Failed to check process status");
    assert!(is_alive, "Process should be running");

    // Send quit command (q)
    session.send("q").expect("Failed to send quit command");

    // Should exit cleanly
    let _ = session.expect(Eof).expect("Process should exit");
}

/// Smoke test: App starts and quits cleanly
///
/// Validates that the application can launch with a valid fixture,
/// initialize its TUI, and cleanly exit when sent the quit command.
#[test]
fn smoke_app_starts_and_quits() {
    let binary = find_binary();
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixture = manifest_dir.join("tests/fixtures/minimal_session.jsonl");

    let mut session =
        spawn(format!("{} {}", binary.display(), fixture.display())).expect("Failed to spawn cclv");

    // Give TUI time to initialize and render
    std::thread::sleep(Duration::from_millis(500));

    // Should be running (not crashed)
    let is_alive = session.is_alive().expect("Failed to check process status");
    assert!(is_alive, "Process should be running after startup");

    // Send quit command (q)
    session.send("q").expect("Failed to send quit command");

    // Should exit cleanly
    let _ = session.expect(Eof).expect("Process should exit");
}

/// Smoke test: Scrolling doesn't crash the application
///
/// Validates that the application can handle repeated scroll operations
/// on a large fixture without crashing. This is a regression test for
/// scroll-related crashes discovered during acceptance testing.
#[test]
fn smoke_scroll_does_not_crash() {
    let binary = find_binary();
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixture = manifest_dir.join("tests/fixtures/cc-session-log.jsonl");

    let mut session =
        spawn(format!("{} {}", binary.display(), fixture.display())).expect("Failed to spawn cclv");

    // Give TUI time to initialize (longer timeout for large file)
    std::thread::sleep(Duration::from_millis(2000));

    // Verify app started successfully
    let is_alive = session.is_alive().expect("Failed to check process status");
    assert!(
        is_alive,
        "Process should be running after loading large file"
    );

    // Send scroll down key ('j') multiple times
    for _ in 0..10 {
        session.send("j").expect("Failed to send scroll down");
        std::thread::sleep(Duration::from_millis(50));
    }

    // Verify app is still alive after scrolling down
    let is_alive = session.is_alive().expect("Failed to check process status");
    assert!(is_alive, "Process should be running after scrolling down");

    // Send scroll up key ('k') multiple times
    for _ in 0..10 {
        session.send("k").expect("Failed to send scroll up");
        std::thread::sleep(Duration::from_millis(50));
    }

    // Verify app is still alive after scrolling
    let is_alive = session.is_alive().expect("Failed to check process status");
    assert!(is_alive, "Process should be running after scrolling");

    // Send quit command (q)
    session.send("q").expect("Failed to send quit command");

    // Should exit cleanly
    let _ = session.expect(Eof).expect("Process should exit");
}

/// Smoke test: Search functionality works end-to-end
///
/// Validates that the application can open search, accept input,
/// submit the search, and continue operating normally. This ensures
/// the search state machine works in the real application.
#[test]
fn smoke_search_works() {
    let binary = find_binary();
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixture = manifest_dir.join("tests/fixtures/minimal_session.jsonl");

    let mut session =
        spawn(format!("{} {}", binary.display(), fixture.display())).expect("Failed to spawn cclv");

    // Give TUI time to initialize
    std::thread::sleep(Duration::from_millis(500));

    // Verify app started successfully
    let is_alive = session.is_alive().expect("Failed to check process status");
    assert!(is_alive, "Process should be running after startup");

    // Send '/' to open search
    session.send("/").expect("Failed to send search command");

    std::thread::sleep(Duration::from_millis(100));

    // Type a search term
    session.send("test").expect("Failed to send search term");

    std::thread::sleep(Duration::from_millis(100));

    // Send Enter to submit search
    session
        .send(ControlCode::CarriageReturn)
        .expect("Failed to send Enter");

    std::thread::sleep(Duration::from_millis(100));

    // Verify app is still alive after search
    let is_alive = session.is_alive().expect("Failed to check process status");
    assert!(is_alive, "Process should be running after search");

    // Send Escape to close search
    session
        .send(ControlCode::Escape)
        .expect("Failed to send Escape");

    std::thread::sleep(Duration::from_millis(100));

    // Verify app is still alive
    let is_alive = session.is_alive().expect("Failed to check process status");
    assert!(is_alive, "Process should be running after closing search");

    // Send quit command (q)
    session.send("q").expect("Failed to send quit command");

    // Should exit cleanly
    let _ = session.expect(Eof).expect("Process should exit");
}
