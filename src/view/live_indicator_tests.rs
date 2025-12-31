//! Tests for LiveIndicator widget (FR-042b).

use super::*;
use ratatui::style::{Color, Style};

// ===== Static Mode Tests =====

#[test]
fn static_mode_renders_gray_text() {
    let indicator = LiveIndicator::new(InputMode::Static, false);
    let span = indicator.render();

    assert_eq!(
        span.content, "[LIVE] ",
        "Static mode should show '[LIVE] ' text"
    );
    assert_eq!(
        span.style,
        Style::default().fg(Color::Gray),
        "Static mode should use gray color"
    );
}

#[test]
fn static_mode_ignores_blink_state() {
    // When blink_on=true in Static mode, should still be gray (not green)
    let indicator = LiveIndicator::new(InputMode::Static, true);
    let span = indicator.render();

    assert_eq!(
        span.style,
        Style::default().fg(Color::Gray),
        "Static mode should ignore blink_on and stay gray"
    );
}

// ===== EOF Mode Tests =====

#[test]
fn eof_mode_renders_gray_text() {
    let indicator = LiveIndicator::new(InputMode::Eof, false);
    let span = indicator.render();

    assert_eq!(
        span.content, "[LIVE] ",
        "EOF mode should show '[LIVE] ' text"
    );
    assert_eq!(
        span.style,
        Style::default().fg(Color::Gray),
        "EOF mode should use gray color"
    );
}

#[test]
fn eof_mode_ignores_blink_state() {
    // When blink_on=true in EOF mode, should still be gray (not green)
    let indicator = LiveIndicator::new(InputMode::Eof, true);
    let span = indicator.render();

    assert_eq!(
        span.style,
        Style::default().fg(Color::Gray),
        "EOF mode should ignore blink_on and stay gray"
    );
}

// ===== Streaming Mode Tests =====

#[test]
fn streaming_mode_with_blink_on_renders_green_text() {
    let indicator = LiveIndicator::new(InputMode::Streaming, true);
    let span = indicator.render();

    assert_eq!(
        span.content, "[LIVE] ",
        "Streaming mode with blink_on should show '[LIVE] ' text"
    );
    assert_eq!(
        span.style,
        Style::default().fg(Color::Green),
        "Streaming mode with blink_on should use green color"
    );
}

#[test]
fn streaming_mode_with_blink_off_renders_empty() {
    let indicator = LiveIndicator::new(InputMode::Streaming, false);
    let span = indicator.render();

    assert_eq!(
        span.content, "",
        "Streaming mode with blink_off should render empty string (hidden)"
    );
}

// ===== Blink Animation Verification =====

#[test]
fn streaming_mode_blink_toggles_visibility() {
    // Blink ON - should be visible green
    let indicator_on = LiveIndicator::new(InputMode::Streaming, true);
    let span_on = indicator_on.render();
    assert_eq!(span_on.content, "[LIVE] ", "Blink ON should be visible");
    assert_eq!(
        span_on.style.fg,
        Some(Color::Green),
        "Blink ON should be green"
    );

    // Blink OFF - should be hidden
    let indicator_off = LiveIndicator::new(InputMode::Streaming, false);
    let span_off = indicator_off.render();
    assert_eq!(span_off.content, "", "Blink OFF should be hidden");

    // This verifies the blinking effect: alternating between visible green and hidden
}

// ===== Widget Construction Tests =====

#[test]
fn new_creates_indicator_with_correct_state() {
    let indicator = LiveIndicator::new(InputMode::Streaming, true);
    assert_eq!(indicator.mode, InputMode::Streaming);
    assert!(indicator.blink_on);

    let indicator2 = LiveIndicator::new(InputMode::Static, false);
    assert_eq!(indicator2.mode, InputMode::Static);
    assert!(!indicator2.blink_on);
}

// ===== Edge Cases =====

#[test]
fn text_format_is_consistent() {
    // All modes that show text should use the same text format
    let static_text = LiveIndicator::new(InputMode::Static, false)
        .render()
        .content;
    let eof_text = LiveIndicator::new(InputMode::Eof, false).render().content;
    let streaming_text = LiveIndicator::new(InputMode::Streaming, true)
        .render()
        .content;

    assert_eq!(
        static_text, streaming_text,
        "Static and Streaming (when visible) should use same text"
    );
    assert_eq!(
        eof_text, streaming_text,
        "EOF and Streaming (when visible) should use same text"
    );
}
