//! Syntax highlighting with configurable themes using syntect + two-face.
//!
//! This module provides code syntax highlighting that respects the user's theme
//! configuration, using the two-face crate which includes themes like gruvbox.
//!
//! # Theme Support
//!
//! Built-in themes (from two-face):
//! - `gruvbox-dark` / `gruvbox-light` - Warm retro groove colors
//! - `base16-ocean` - Ocean-inspired colors (default)
//! - `solarized-dark` / `solarized-light` - Precision colors
//! - `monokai` - Sublime Text classic
//! - `nord` - Arctic, north-bluish colors
//! - `dracula` - Dark theme for vampires
//! - And many more (see `VALID_THEMES`)

use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};
use std::sync::{LazyLock, OnceLock};
use syntect::highlighting::FontStyle;
use two_face::theme::{EmbeddedLazyThemeSet, EmbeddedThemeName};

/// Global configured theme name, set at startup.
static CONFIGURED_THEME: OnceLock<String> = OnceLock::new();

/// Initialize the global theme configuration.
///
/// This should be called once at application startup with the resolved theme name.
/// If not called, the default theme will be used.
///
/// # Arguments
/// * `theme_name` - The theme name from config (e.g., "gruvbox-dark")
pub fn init_theme(theme_name: &str) {
    let _ = CONFIGURED_THEME.set(theme_name.to_string());
}

/// Get the currently configured theme name.
pub fn get_configured_theme() -> &'static str {
    CONFIGURED_THEME.get().map(|s| s.as_str()).unwrap_or(DEFAULT_THEME)
}

/// All valid theme names that can be used in configuration.
pub const VALID_THEMES: &[&str] = &[
    "ansi",
    "base16",
    "base16-256",
    "base16-eighties-dark",
    "base16-mocha-dark",
    "base16-ocean-dark",
    "base16-ocean-light",
    "coldark-cold",
    "coldark-dark",
    "dark-neon",
    "dracula",
    "github",
    "gruvbox-dark",
    "gruvbox-light",
    "inspired-github",
    "leet",
    "monokai",
    "monokai-bright",
    "monokai-light",
    "monokai-origin",
    "nord",
    "one-half-dark",
    "one-half-light",
    "solarized-dark",
    "solarized-light",
    "sublime-snazzy",
    "two-dark",
    "visual-studio-dark-plus",
    "zenburn",
];

/// Default theme name.
pub const DEFAULT_THEME: &str = "base16-ocean-dark";

/// Lazy-loaded theme set containing all two-face themes.
static THEME_SET: LazyLock<EmbeddedLazyThemeSet> = LazyLock::new(two_face::theme::extra);

/// Map a theme name string to the corresponding EmbeddedThemeName.
fn theme_name_to_embedded(name: &str) -> Option<EmbeddedThemeName> {
    match name {
        "ansi" => Some(EmbeddedThemeName::Ansi),
        "base16" => Some(EmbeddedThemeName::Base16),
        "base16-256" => Some(EmbeddedThemeName::Base16_256),
        "base16-eighties-dark" => Some(EmbeddedThemeName::Base16EightiesDark),
        "base16-mocha-dark" => Some(EmbeddedThemeName::Base16MochaDark),
        "base16-ocean-dark" => Some(EmbeddedThemeName::Base16OceanDark),
        "base16-ocean-light" => Some(EmbeddedThemeName::Base16OceanLight),
        "coldark-cold" => Some(EmbeddedThemeName::ColdarkCold),
        "coldark-dark" => Some(EmbeddedThemeName::ColdarkDark),
        "dark-neon" => Some(EmbeddedThemeName::DarkNeon),
        "dracula" => Some(EmbeddedThemeName::Dracula),
        "github" => Some(EmbeddedThemeName::Github),
        "gruvbox-dark" => Some(EmbeddedThemeName::GruvboxDark),
        "gruvbox-light" => Some(EmbeddedThemeName::GruvboxLight),
        "inspired-github" => Some(EmbeddedThemeName::InspiredGithub),
        "leet" => Some(EmbeddedThemeName::Leet),
        "monokai" | "monokai-extended" => Some(EmbeddedThemeName::MonokaiExtended),
        "monokai-bright" => Some(EmbeddedThemeName::MonokaiExtendedBright),
        "monokai-light" => Some(EmbeddedThemeName::MonokaiExtendedLight),
        "monokai-origin" => Some(EmbeddedThemeName::MonokaiExtendedOrigin),
        "nord" => Some(EmbeddedThemeName::Nord),
        "one-half-dark" => Some(EmbeddedThemeName::OneHalfDark),
        "one-half-light" => Some(EmbeddedThemeName::OneHalfLight),
        "solarized-dark" => Some(EmbeddedThemeName::SolarizedDark),
        "solarized-light" => Some(EmbeddedThemeName::SolarizedLight),
        "sublime-snazzy" => Some(EmbeddedThemeName::SublimeSnazzy),
        "two-dark" => Some(EmbeddedThemeName::TwoDark),
        "visual-studio-dark-plus" => Some(EmbeddedThemeName::VisualStudioDarkPlus),
        "zenburn" => Some(EmbeddedThemeName::Zenburn),
        _ => None,
    }
}

/// Get the EmbeddedThemeName for a theme name string.
fn get_embedded_theme(name: &str) -> EmbeddedThemeName {
    theme_name_to_embedded(name).unwrap_or(EmbeddedThemeName::Base16OceanDark)
}

/// Check if a theme name is valid.
pub fn is_valid_theme(name: &str) -> bool {
    theme_name_to_embedded(name).is_some()
}

/// Syntax highlighter with configurable theme.
pub struct SyntaxHighlighter {
    theme_name: EmbeddedThemeName,
}

impl SyntaxHighlighter {
    /// Create a new highlighter with the specified theme.
    ///
    /// If the theme name is invalid, falls back to the default theme.
    pub fn new(theme_name: &str) -> Self {
        Self {
            theme_name: get_embedded_theme(theme_name),
        }
    }

    /// Create a highlighter with the default theme.
    pub fn default_theme() -> Self {
        Self::new(DEFAULT_THEME)
    }

    /// Highlight a code block with syntax highlighting.
    ///
    /// # Arguments
    /// * `code` - The source code to highlight
    /// * `language` - Optional language hint (e.g., "rust", "python")
    ///
    /// # Returns
    /// Vector of ratatui Lines with syntax highlighting applied.
    pub fn highlight_code(&self, code: &str, language: Option<&str>) -> Vec<Line<'static>> {
        use syntect::easy::HighlightLines;
        use syntect::parsing::SyntaxSet;
        use syntect::util::LinesWithEndings;

        // Load syntax definitions
        static SYNTAX_SET: LazyLock<SyntaxSet> =
            LazyLock::new(two_face::syntax::extra_newlines);

        let theme = THEME_SET.get(self.theme_name);

        // Find syntax definition for language
        let syntax = language
            .and_then(|lang| SYNTAX_SET.find_syntax_by_token(lang))
            .unwrap_or_else(|| SYNTAX_SET.find_syntax_plain_text());

        let mut highlighter = HighlightLines::new(syntax, theme);
        let mut lines = Vec::new();

        for line in LinesWithEndings::from(code) {
            match highlighter.highlight_line(line, &SYNTAX_SET) {
                Ok(highlighted) => {
                    let spans: Vec<Span<'static>> = highlighted
                        .into_iter()
                        .map(|(style, text)| {
                            let ratatui_style = syntect_style_to_ratatui(style);
                            Span::styled(text.to_string(), ratatui_style)
                        })
                        .collect();
                    lines.push(Line::from(spans));
                }
                Err(_) => {
                    // Fallback: render as plain text
                    lines.push(Line::from(line.trim_end().to_string()));
                }
            }
        }

        lines
    }
}

impl Default for SyntaxHighlighter {
    fn default() -> Self {
        Self::default_theme()
    }
}

/// Convert syntect highlighting style to ratatui style.
fn syntect_style_to_ratatui(style: syntect::highlighting::Style) -> Style {
    let fg = Color::Rgb(style.foreground.r, style.foreground.g, style.foreground.b);

    let mut ratatui_style = Style::default().fg(fg);

    // Apply font style modifiers
    if style.font_style.contains(FontStyle::BOLD) {
        ratatui_style = ratatui_style.add_modifier(Modifier::BOLD);
    }
    if style.font_style.contains(FontStyle::ITALIC) {
        ratatui_style = ratatui_style.add_modifier(Modifier::ITALIC);
    }
    if style.font_style.contains(FontStyle::UNDERLINE) {
        ratatui_style = ratatui_style.add_modifier(Modifier::UNDERLINED);
    }

    ratatui_style
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_themes_are_recognized() {
        for theme in VALID_THEMES {
            assert!(
                is_valid_theme(theme),
                "Theme '{}' should be valid",
                theme
            );
        }
    }

    #[test]
    fn invalid_theme_is_rejected() {
        assert!(!is_valid_theme("not-a-real-theme"));
    }

    #[test]
    fn highlighter_uses_default_for_invalid_theme() {
        let highlighter = SyntaxHighlighter::new("invalid-theme");
        // Should not panic and use default theme
        let lines = highlighter.highlight_code("fn main() {}", Some("rust"));
        assert!(!lines.is_empty());
    }

    #[test]
    fn gruvbox_dark_works() {
        let highlighter = SyntaxHighlighter::new("gruvbox-dark");
        let lines = highlighter.highlight_code("let x = 42;", Some("rust"));
        assert!(!lines.is_empty());
        // Gruvbox should apply colors (not just plain text)
        let first_line = &lines[0];
        assert!(
            first_line.spans.iter().any(|s| s.style.fg.is_some()),
            "Gruvbox should apply foreground colors"
        );
    }

    #[test]
    fn highlight_rust_code() {
        let highlighter = SyntaxHighlighter::default();
        let code = r#"fn main() {
    println!("Hello, world!");
}"#;
        let lines = highlighter.highlight_code(code, Some("rust"));
        assert_eq!(lines.len(), 3);
    }

    #[test]
    fn highlight_unknown_language_falls_back_to_plain() {
        let highlighter = SyntaxHighlighter::default();
        let lines = highlighter.highlight_code("some text", Some("not-a-language"));
        assert!(!lines.is_empty());
    }
}
