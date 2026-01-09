//! Tests for LiveIndicator widget (FR-042b).

use super::super::styles::MUTED_TEXT;
use super::*;
use ratatui::style::{Color, Style};

// ===== Static Mode Tests =====

#[test]
fn static_mode_renders_gray_text() {
    let indicator = LiveIndicator::new(InputMode::Static, false, true);
    let span = indicator.render();

    assert_eq!(
        span.content, "[LIVE] ",
        "Static mode should show '[LIVE] ' text"
    );
    assert_eq!(
        span.style, MUTED_TEXT,
        "Static mode should use MUTED_TEXT style"
    );
}

#[test]
fn static_mode_ignores_blink_state() {
    // When blink_on=true in Static mode, should still be gray (not green)
    let indicator = LiveIndicator::new(InputMode::Static, true, true);
    let span = indicator.render();

    assert_eq!(
        span.style, MUTED_TEXT,
        "Static mode should ignore blink_on and use MUTED_TEXT"
    );
}

// ===== EOF Mode Tests =====

#[test]
fn eof_mode_renders_gray_text() {
    let indicator = LiveIndicator::new(InputMode::Eof, false, true);
    let span = indicator.render();

    assert_eq!(
        span.content, "[LIVE] ",
        "EOF mode should show '[LIVE] ' text"
    );
    assert_eq!(
        span.style, MUTED_TEXT,
        "EOF mode should use MUTED_TEXT style"
    );
}

#[test]
fn eof_mode_ignores_blink_state() {
    // When blink_on=true in EOF mode, should still be gray (not green)
    let indicator = LiveIndicator::new(InputMode::Eof, true, true);
    let span = indicator.render();

    assert_eq!(
        span.style, MUTED_TEXT,
        "EOF mode should ignore blink_on and use MUTED_TEXT"
    );
}

// ===== Streaming Mode Tests =====

#[test]
fn streaming_mode_with_blink_on_renders_green_text() {
    let indicator = LiveIndicator::new(InputMode::Streaming, true, true);
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
    let indicator = LiveIndicator::new(InputMode::Streaming, false, true);
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
    let indicator_on = LiveIndicator::new(InputMode::Streaming, true, true);
    let span_on = indicator_on.render();
    assert_eq!(span_on.content, "[LIVE] ", "Blink ON should be visible");
    assert_eq!(
        span_on.style.fg,
        Some(Color::Green),
        "Blink ON should be green"
    );

    // Blink OFF - should be hidden
    let indicator_off = LiveIndicator::new(InputMode::Streaming, false, true);
    let span_off = indicator_off.render();
    assert_eq!(span_off.content, "", "Blink OFF should be hidden");

    // This verifies the blinking effect: alternating between visible green and hidden
}

// ===== Widget Construction Tests =====

#[test]
fn new_creates_indicator_with_correct_state() {
    let indicator = LiveIndicator::new(InputMode::Streaming, true, true);
    assert_eq!(indicator.mode, InputMode::Streaming);
    assert!(indicator.blink_on);
    assert!(indicator.tailing_enabled);

    let indicator2 = LiveIndicator::new(InputMode::Static, false, false);
    assert_eq!(indicator2.mode, InputMode::Static);
    assert!(!indicator2.blink_on);
    assert!(!indicator2.tailing_enabled);
}

// ===== Edge Cases =====

#[test]
fn text_format_is_consistent() {
    // All modes that show text should use the same text format
    let static_text = LiveIndicator::new(InputMode::Static, false, true)
        .render()
        .content;
    let eof_text = LiveIndicator::new(InputMode::Eof, false, true)
        .render()
        .content;
    let streaming_text = LiveIndicator::new(InputMode::Streaming, true, true)
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

// ===== Tailing Enabled Tests (cclv-463.4.3) =====

#[test]
fn streaming_mode_hidden_when_tailing_disabled() {
    // When viewing historical session (tailing_enabled=false),
    // LIVE indicator should be hidden even if InputMode is Streaming
    let indicator = LiveIndicator::new(InputMode::Streaming, true, false);
    let span = indicator.render();

    assert_eq!(
        span.content, "",
        "LIVE indicator should be hidden when tailing is disabled, even if Streaming"
    );
}

#[test]
fn streaming_mode_visible_when_tailing_enabled() {
    // When viewing last session (tailing_enabled=true) with Streaming mode,
    // LIVE indicator should blink as normal
    let indicator_on = LiveIndicator::new(InputMode::Streaming, true, true);
    let span_on = indicator_on.render();

    assert_eq!(
        span_on.content, "[LIVE] ",
        "LIVE indicator should be visible when tailing is enabled and Streaming"
    );
    assert_eq!(
        span_on.style,
        Style::default().fg(Color::Green),
        "LIVE indicator should be green when tailing enabled and blink_on"
    );

    // Blink off still hides
    let indicator_off = LiveIndicator::new(InputMode::Streaming, false, true);
    let span_off = indicator_off.render();
    assert_eq!(
        span_off.content, "",
        "LIVE indicator should be hidden during blink-off phase"
    );
}

#[test]
fn static_mode_unaffected_by_tailing_state() {
    // Static mode should show gray indicator regardless of tailing state
    let indicator_tailing_on = LiveIndicator::new(InputMode::Static, false, true);
    let span_tailing_on = indicator_tailing_on.render();

    let indicator_tailing_off = LiveIndicator::new(InputMode::Static, false, false);
    let span_tailing_off = indicator_tailing_off.render();

    assert_eq!(
        span_tailing_on.content, "[LIVE] ",
        "Static mode with tailing enabled should show LIVE"
    );
    assert_eq!(
        span_tailing_on.style, MUTED_TEXT,
        "Static mode should use MUTED_TEXT"
    );

    assert_eq!(
        span_tailing_off.content, "[LIVE] ",
        "Static mode with tailing disabled should show LIVE"
    );
    assert_eq!(
        span_tailing_off.style, MUTED_TEXT,
        "Static mode should use MUTED_TEXT regardless of tailing state"
    );
}
