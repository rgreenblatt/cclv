//! Configuration module.

pub mod keybindings;

pub use keybindings::KeyBindings;

/// Application-level configuration.
///
/// Holds global settings that affect application behavior.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppConfig {
    /// Whether line wrapping is enabled globally.
    ///
    /// When `true`, long lines wrap to fit the viewport width.
    /// When `false`, long lines require horizontal scrolling.
    pub line_wrap: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        todo!("AppConfig::default")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_has_line_wrap_enabled() {
        let config = AppConfig::default();
        assert!(
            config.line_wrap,
            "Default config should have line_wrap=true per FR-039"
        );
    }

    #[test]
    fn default_config_is_cloneable() {
        let config = AppConfig::default();
        let cloned = config.clone();
        assert_eq!(
            config, cloned,
            "Cloned config should equal original"
        );
    }

    #[test]
    fn can_create_config_with_wrap_disabled() {
        let config = AppConfig { line_wrap: false };
        assert!(!config.line_wrap, "Should allow line_wrap=false");
    }

    #[test]
    fn can_create_config_with_wrap_enabled() {
        let config = AppConfig { line_wrap: true };
        assert!(config.line_wrap, "Should allow line_wrap=true");
    }
}
