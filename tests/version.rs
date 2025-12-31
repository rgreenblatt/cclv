//! Integration test: Verify binary prints correct version

use std::process::Command;

#[test]
fn binary_prints_version() {
    // EXPECT: Binary runs and prints version "0.1.0" to stdout
    let output = Command::new(env!("CARGO_BIN_EXE_cclv"))
        .output()
        .expect("Failed to execute binary");

    // Convert stdout to string
    let stdout = String::from_utf8_lossy(&output.stdout);

    // VERIFY: Output contains version number from Cargo.toml
    assert!(
        stdout.contains("0.1.0"),
        "Expected output to contain version '0.1.0', but got: {}",
        stdout
    );
}
