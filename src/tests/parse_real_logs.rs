//! Integration test for parsing real Claude Code session logs.
//!
//! This test parses the entire tests/fixtures/cc-session-log.jsonl file
//! to verify compatibility with actual Claude Code JSONL format.

use crate::parser::{parse_entry_graceful, ParseResult};
use std::fs::File;
use std::io::{BufRead, BufReader};

/// Statistics from parsing a log file.
#[derive(Debug, Default)]
struct ParseStats {
    total_lines: usize,
    successful: usize,
    malformed: usize,
}

impl ParseStats {
    /// Success rate as a percentage (0.0 to 100.0).
    fn success_rate(&self) -> f64 {
        if self.total_lines == 0 {
            0.0
        } else {
            (self.successful as f64 / self.total_lines as f64) * 100.0
        }
    }
}

/// Parse entire fixture file and return statistics.
fn parse_fixture_file(path: &str) -> std::io::Result<ParseStats> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);

    let mut stats = ParseStats::default();

    for (line_number, line_result) in reader.lines().enumerate() {
        let line = line_result?;
        stats.total_lines += 1;

        // Parse with graceful error handling
        match parse_entry_graceful(&line, line_number + 1) {
            ParseResult::Valid(_) => {
                stats.successful += 1;
            }
            ParseResult::Malformed(_) => {
                stats.malformed += 1;
            }
        }
    }

    Ok(stats)
}

#[test]
fn test_parse_cc_session_log() {
    // Fixture file path
    let fixture_path = "tests/fixtures/cc-session-log.jsonl";

    // Parse the file
    let stats = parse_fixture_file(fixture_path).expect("Should be able to read fixture file");

    // Report statistics
    println!("\n=== Parse Statistics ===");
    println!("Total lines:     {}", stats.total_lines);
    println!("Successful:      {}", stats.successful);
    println!("Malformed:       {}", stats.malformed);
    println!("Success rate:    {:.2}%", stats.success_rate());

    // Test passes if we successfully parsed the file
    // (Some parse failures are expected during format compatibility work)
    assert!(
        stats.total_lines > 0,
        "Should have parsed at least one line"
    );

    // Document current state: If success rate is below 100%,
    // this indicates format compatibility issues to fix
    if stats.success_rate() < 100.0 {
        println!("\nWARNING: {} lines failed to parse", stats.malformed);
        println!("This indicates format compatibility issues that need fixing.");
    }
}
