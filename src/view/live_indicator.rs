//! LIVE indicator widget for status bar.
//!
//! Displays streaming status per FR-042b:
//! - Gray when Static or Eof
//! - Blinking green when Streaming

use crate::state::InputMode;
use ratatui::{
    style::{Color, Style},
    text::Span,
};

/// Text content for the LIVE indicator.
const LIVE_INDICATOR_PREFIX: &str = "[LIVE] ";

/// LIVE indicator widget that renders based on InputMode and blink state.
///
/// # Functional Requirement
///
/// **FR-042b**: System MUST display a "LIVE" indicator in the status bar:
/// gray when static mode or after EOF, blinking green when actively streaming from stdin.
///
/// # Design
///
/// This widget is pure and stateless. It accepts:
/// - `input_mode`: Current input mode (Static, Streaming, Eof)
/// - `blink_on`: Whether the blink animation is currently ON (managed externally by timer)
///
/// The blink state is passed in rather than managed internally, following the
/// principle of separating state management from rendering.
///
/// # Examples
///
/// ```rust
/// use cclv::view::live_indicator::LiveIndicator;
/// use cclv::state::InputMode;
///
/// // Static mode - always gray
/// let indicator = LiveIndicator::new(InputMode::Static, false);
///
/// // Streaming mode - blinking green
/// let indicator_visible = LiveIndicator::new(InputMode::Streaming, true);
/// let indicator_hidden = LiveIndicator::new(InputMode::Streaming, false);
///
/// // EOF - always gray
/// let indicator_eof = LiveIndicator::new(InputMode::Eof, true);
/// ```
#[derive(Debug, Clone)]
pub struct LiveIndicator {
    mode: InputMode,
    blink_on: bool,
}

impl LiveIndicator {
    /// Create a new LiveIndicator with the given mode and blink state.
    ///
    /// # Arguments
    ///
    /// * `mode` - The current input mode (Static, Streaming, or Eof)
    /// * `blink_on` - Whether the blink animation is currently ON (only relevant for Streaming mode)
    pub fn new(mode: InputMode, blink_on: bool) -> Self {
        Self { mode, blink_on }
    }

    /// Render the indicator as a ratatui Span.
    ///
    /// # Behavior
    ///
    /// - `InputMode::Static` → Gray "[LIVE]" text
    /// - `InputMode::Eof` → Gray "[LIVE]" text
    /// - `InputMode::Streaming` with `blink_on=true` → Green "[LIVE]" text
    /// - `InputMode::Streaming` with `blink_on=false` → Empty string (hidden)
    ///
    /// # Returns
    ///
    /// A `Span` containing the styled indicator text.
    pub fn render(&self) -> Span<'static> {
        match self.mode {
            InputMode::Static | InputMode::Eof => {
                // Always gray when static or EOF
                Span::styled(LIVE_INDICATOR_PREFIX, Style::default().fg(Color::Gray))
            }
            InputMode::Streaming => {
                if self.blink_on {
                    // Green when blinking ON
                    Span::styled(LIVE_INDICATOR_PREFIX, Style::default().fg(Color::Green))
                } else {
                    // Hidden when blinking OFF
                    Span::raw("")
                }
            }
        }
    }
}

// ===== Tests =====

#[cfg(test)]
#[path = "live_indicator_tests.rs"]
mod tests;
