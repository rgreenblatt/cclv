//! E2E tests for stdin-specific bugs
//!
//! These tests specifically exercise the stdin input path by running the actual
//! binary with piped stdin. Some bugs only manifest when data is piped via stdin
//! and don't reproduce through the test harness.
//!
//! Run with: `cargo test --features e2e-tests --test e2e_stdin_repro`

#![cfg(feature = "e2e-tests")]

use expectrl::{spawn, Eof, Regex};
use std::path::PathBuf;
use std::time::Duration;

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

/// Bug reproduction: stdin piping causes vertical character rendering in subagent expanded prompt
///
/// EXPECTED: When expanding a collapsed initial prompt in subagent view, text should render
///           horizontally, e.g.: "## Review Task"
///
/// ACTUAL (BUG): Text renders vertically with one character per line:
///         R
///         e
///         v
///         i
///         e
///         w
///         ...
///
/// Steps to reproduce manually:
/// 1. cargo run --release < tests/fixtures/cc-session-log.jsonl
/// 2. Press ] to navigate to first subagent tab
/// 3. Press g to go to top
/// 4. Press Enter to expand collapsed initial prompt
/// 5. Observe: characters render in single vertical column
///
/// Does NOT occur when file is passed as argument:
/// cargo run --release tests/fixtures/cc-session-log.jsonl
///
/// Bead: cclv-5ur.58
#[test]
fn bug_stdin_subagent_expand_vertical_chars() {
    let binary = find_binary();
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixture = manifest_dir.join("tests/fixtures/cc-session-log.jsonl");

    // Spawn with stdin piping (this is the key difference from file argument)
    // Use cat to pipe the file content to the binary
    let mut session = spawn(format!("cat {} | {}", fixture.display(), binary.display()))
        .expect("Failed to spawn cclv with piped stdin");

    // Give TUI time to initialize (longer timeout for large file + stdin processing)
    std::thread::sleep(Duration::from_millis(3000));

    // Verify app started successfully
    let is_alive = session.is_alive().expect("Failed to check process status");
    assert!(
        is_alive,
        "Process should be running after loading via stdin"
    );

    // Press ] to navigate to first subagent
    session.send("]").expect("Failed to send ] key");
    std::thread::sleep(Duration::from_millis(500));

    // Press g to go to top
    session.send("g").expect("Failed to send g key");
    std::thread::sleep(Duration::from_millis(500));

    // Press Enter to expand the collapsed initial prompt
    session
        .send(expectrl::ControlCode::CarriageReturn)
        .expect("Failed to send Enter key");
    std::thread::sleep(Duration::from_millis(1000));

    // DETECTION STRATEGY:
    // The bug causes each character to render on its own line. In the screen buffer:
    // - BUG: "R" on row N, "e" on row N+1, "v" on row N+2... (not adjacent)
    // - FIXED: "Review Task" as adjacent characters on one row
    //
    // Since the bug is fixed, we just verify the correct horizontal rendering exists.
    // The original buggy pattern was too loose and matched unrelated content.

    // Check for the correct horizontal pattern
    let correct_horizontal = session.check(Regex("Review Task"));

    // Clean up
    session.send("q").expect("Failed to send quit command");
    let _ = session.expect(Eof);

    // ASSERTION: The text should render horizontally
    assert!(
        correct_horizontal.is_ok(),
        "BUG cclv-5ur.58 DETECTED!\n\
         \n\
         Could not find 'Review Task' rendered horizontally in the screen buffer.\n\
         Text may be rendering vertically (one char per line) instead of horizontally.\n\
         \n\
         This bug only occurs when piping via stdin:\n\
           cargo run --release < tests/fixtures/cc-session-log.jsonl\n\
         \n\
         Does NOT occur with file argument:\n\
           cargo run --release tests/fixtures/cc-session-log.jsonl\n"
    );
}
