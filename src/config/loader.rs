//! Configuration file loading with precedence handling.

use serde::Deserialize;
use std::path::PathBuf;
use thiserror::Error;

/// Errors that can occur during config loading.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ConfigError {
    /// Config file path contains invalid UTF-8 or cannot be resolved.
    #[error("Invalid config path: {0}")]
    InvalidPath(String),

    /// Failed to read config file (file may not exist or have permission issues).
    #[error("Failed to read config file at {path}: {reason}")]
    ReadError {
        /// Path that failed to read.
        path: PathBuf,
        /// Reason for failure.
        reason: String,
    },

    /// Config file contains invalid TOML syntax.
    #[error("Invalid TOML in {path}: {reason}")]
    ParseError {
        /// Path with invalid TOML.
        path: PathBuf,
        /// Parse error details.
        reason: String,
    },
}

/// TOML configuration file structure.
///
/// All fields are optional - if not specified, hardcoded defaults are used.
/// Corresponds to `~/.config/cclv/config.toml`.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ConfigFile {
    /// Theme name (e.g., "base16-ocean", "solarized-dark").
    #[serde(default)]
    pub theme: Option<String>,

    /// Default follow mode (live tailing).
    #[serde(default)]
    pub follow: Option<bool>,

    /// Show stats panel on startup.
    #[serde(default)]
    pub show_stats: Option<bool>,

    /// Collapse threshold in lines.
    #[serde(default)]
    pub collapse_threshold: Option<usize>,

    /// Summary lines for collapsed messages.
    #[serde(default)]
    pub summary_lines: Option<usize>,

    /// Line wrapping enabled.
    #[serde(default)]
    pub line_wrap: Option<bool>,

    /// Log buffer capacity for logging pane.
    #[serde(default)]
    pub log_buffer_capacity: Option<usize>,

    /// Path to log file for tracing output (FR-055).
    #[serde(default)]
    pub log_file_path: Option<PathBuf>,

    /// Custom key bindings (future use).
    #[serde(default)]
    pub keybindings: Option<toml::Value>,

    /// Pricing section for cost estimation.
    #[serde(default)]
    pub pricing: Option<PricingConfigSection>,
}

/// Pricing configuration section from TOML.
///
/// Structure matches the TOML format:
/// ```toml
/// [pricing.models.opus]
/// input = 15.0
/// output = 75.0
/// cached_input = 1.5
/// ```
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PricingConfigSection {
    /// Per-model pricing entries (e.g., "opus", "sonnet", "haiku").
    #[serde(default)]
    pub models: std::collections::HashMap<String, PricingEntry>,

    /// Default pricing for unknown models.
    #[serde(default)]
    pub default: Option<PricingEntry>,
}

/// Pricing entry for a specific model.
///
/// All costs are per million tokens in USD.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PricingEntry {
    /// Cost per million input tokens.
    pub input: f64,

    /// Cost per million output tokens.
    pub output: f64,

    /// Cost per million cached input tokens (optional).
    #[serde(default)]
    pub cached_input: Option<f64>,
}

/// Resolved configuration after applying precedence rules.
///
/// Created by merging defaults, config file, env vars, and CLI args.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedConfig {
    /// Theme name.
    pub theme: String,
    /// Follow mode.
    pub follow: bool,
    /// Show stats on startup.
    pub show_stats: bool,
    /// Collapse threshold.
    pub collapse_threshold: usize,
    /// Summary lines.
    pub summary_lines: usize,
    /// Line wrapping.
    pub line_wrap: bool,
    /// Log buffer capacity.
    pub log_buffer_capacity: usize,
    /// Path to log file for tracing output (FR-055).
    pub log_file_path: PathBuf,
}

impl Default for ResolvedConfig {
    fn default() -> Self {
        Self {
            theme: "base16-ocean".to_string(),
            follow: true,
            show_stats: false,
            collapse_threshold: 10,
            summary_lines: 3,
            line_wrap: true,
            log_buffer_capacity: 1000,
            log_file_path: default_log_path(),
        }
    }
}

/// Resolve default log file path.
///
/// Returns `~/.local/state/cclv/cclv.log` on Unix-like systems,
/// or appropriate platform path on other systems (FR-055).
///
/// If state directory cannot be determined, falls back to current directory.
pub fn default_log_path() -> PathBuf {
    // Try to get platform-appropriate state directory
    if let Some(state_dir) = dirs::state_dir() {
        state_dir.join("cclv").join("cclv.log")
    } else {
        // Fallback to current directory
        PathBuf::from("cclv.log")
    }
}

/// Load configuration file from a specific path.
///
/// Returns `Ok(None)` if file doesn't exist (not an error - use defaults).
/// Returns `Err` if file exists but cannot be read or parsed.
///
/// # Arguments
///
/// * `path` - Path to config file
///
/// # Errors
///
/// Returns error if file exists but has read or parse errors.
pub fn load_config_file(path: impl Into<PathBuf>) -> Result<Option<ConfigFile>, ConfigError> {
    let path = path.into();

    // Missing file is not an error - use defaults
    if !path.exists() {
        return Ok(None);
    }

    // Read file contents
    let contents = std::fs::read_to_string(&path).map_err(|e| ConfigError::ReadError {
        path: path.clone(),
        reason: e.to_string(),
    })?;

    // Parse TOML
    let config: ConfigFile = toml::from_str(&contents).map_err(|e| ConfigError::ParseError {
        path: path.clone(),
        reason: e.to_string(),
    })?;

    Ok(Some(config))
}

/// Resolve default config file path.
///
/// Returns `~/.config/cclv/config.toml` on Unix, appropriate path on other platforms.
/// Returns `None` if home directory cannot be determined.
pub fn default_config_path() -> Option<PathBuf> {
    dirs::config_dir().map(|dir| dir.join("cclv").join("config.toml"))
}

/// Load configuration with precedence handling.
///
/// Precedence (highest to lowest):
/// 1. Explicit `config_path` argument (like CLI `--config`)
/// 2. `CCLV_CONFIG` environment variable
/// 3. Default path `~/.config/cclv/config.toml`
///
/// Missing config files are NOT errors - defaults are used.
///
/// # Arguments
///
/// * `config_path` - Optional explicit config path (e.g., from CLI `--config`)
///
/// # Errors
///
/// Returns error only if a config file exists but cannot be read or parsed.
pub fn load_config_with_precedence(
    config_path: Option<PathBuf>,
) -> Result<Option<ConfigFile>, ConfigError> {
    // 1. Explicit path (like CLI --config)
    if let Some(path) = config_path {
        return load_config_file(path);
    }

    // 2. CCLV_CONFIG environment variable
    if let Ok(env_path) = std::env::var("CCLV_CONFIG") {
        return load_config_file(PathBuf::from(env_path));
    }

    // 3. Default path
    if let Some(default_path) = default_config_path() {
        return load_config_file(default_path);
    }

    // No config path available
    Ok(None)
}

/// Apply environment variable overrides to resolved config.
///
/// Checks for:
/// - `CCLV_THEME`: Override theme
///
/// # Arguments
///
/// * `config` - Base resolved config
///
/// # Returns
///
/// Config with environment overrides applied.
pub fn apply_env_overrides(mut config: ResolvedConfig) -> ResolvedConfig {
    // Override theme if CCLV_THEME is set
    if let Ok(theme) = std::env::var("CCLV_THEME") {
        config.theme = theme;
    }

    config
}

/// Merge config file into defaults to create resolved config.
///
/// For each field in `ConfigFile`, if `Some(value)`, use it; otherwise use default.
///
/// # Arguments
///
/// * `config_file` - Optional loaded config file
///
/// # Returns
///
/// Fully resolved configuration.
pub fn merge_config(config_file: Option<ConfigFile>) -> ResolvedConfig {
    let defaults = ResolvedConfig::default();

    let Some(config) = config_file else {
        return defaults;
    };

    ResolvedConfig {
        theme: config.theme.unwrap_or(defaults.theme),
        follow: config.follow.unwrap_or(defaults.follow),
        show_stats: config.show_stats.unwrap_or(defaults.show_stats),
        collapse_threshold: config
            .collapse_threshold
            .unwrap_or(defaults.collapse_threshold),
        summary_lines: config.summary_lines.unwrap_or(defaults.summary_lines),
        line_wrap: config.line_wrap.unwrap_or(defaults.line_wrap),
        log_buffer_capacity: config
            .log_buffer_capacity
            .unwrap_or(defaults.log_buffer_capacity),
        log_file_path: config.log_file_path.unwrap_or(defaults.log_file_path),
    }
}

/// Apply CLI argument overrides to resolved config.
///
/// CLI args have the highest precedence and override all other sources.
/// Only applies overrides for flags that were explicitly set by the user.
///
/// Precedence chain: Defaults → Config File → Env Vars → CLI Args (highest)
///
/// # Arguments
///
/// * `config` - Base resolved config (already merged with defaults, file, and env vars)
/// * `theme_override` - Optional theme from `--theme` flag
/// * `follow_override` - Optional follow mode from `--follow` flag
/// * `stats_override` - Optional stats visibility from `--stats` flag
///
/// # Returns
///
/// Config with CLI overrides applied.
pub fn apply_cli_overrides(
    mut config: ResolvedConfig,
    theme_override: Option<String>,
    follow_override: Option<bool>,
    stats_override: Option<bool>,
) -> ResolvedConfig {
    // Apply theme override if provided
    if let Some(theme) = theme_override {
        config.theme = theme;
    }

    // Apply follow override if provided
    if let Some(follow) = follow_override {
        config.follow = follow;
    }

    // Apply stats override if provided
    if let Some(stats) = stats_override {
        config.show_stats = stats;
    }

    config
}

#[cfg(test)]
#[path = "loader_tests.rs"]
mod tests;

#[cfg(test)]
mod log_path_tests {
    use super::*;

    #[test]
    fn default_log_path_ends_with_cclv_log() {
        let path = default_log_path();
        assert!(
            path.to_string_lossy().ends_with("cclv.log"),
            "Default log path should end with 'cclv.log', got: {:?}",
            path
        );
    }

    #[test]
    fn default_log_path_contains_cclv_directory() {
        let path = default_log_path();
        let path_str = path.to_string_lossy();
        assert!(
            path_str.contains("cclv"),
            "Default log path should contain 'cclv' directory, got: {:?}",
            path
        );
    }

    #[test]
    fn default_log_path_is_absolute_or_relative() {
        let path = default_log_path();
        // Should return a PathBuf (either absolute or fallback to relative)
        assert!(!path.as_os_str().is_empty(), "Path should not be empty");
    }

    #[test]
    fn resolved_config_default_includes_log_path() {
        let config = ResolvedConfig::default();
        assert!(
            !config.log_file_path.as_os_str().is_empty(),
            "Default config should have non-empty log_file_path"
        );
    }

    #[test]
    fn config_file_log_path_overrides_default() {
        let custom_path = PathBuf::from("/custom/path/to/app.log");
        let config_file = ConfigFile {
            theme: None,
            follow: None,
            show_stats: None,
            collapse_threshold: None,
            summary_lines: None,
            line_wrap: None,
            log_buffer_capacity: None,
            log_file_path: Some(custom_path.clone()),
            keybindings: None,
            pricing: None,
        };

        let resolved = merge_config(Some(config_file));
        assert_eq!(
            resolved.log_file_path, custom_path,
            "Config file log_file_path should override default"
        );
    }

    #[test]
    fn missing_config_file_log_path_uses_default() {
        let config_file = ConfigFile {
            theme: None,
            follow: None,
            show_stats: None,
            collapse_threshold: None,
            summary_lines: None,
            line_wrap: None,
            log_buffer_capacity: None,
            log_file_path: None,
            keybindings: None,
            pricing: None,
        };

        let resolved = merge_config(Some(config_file));
        assert_eq!(
            resolved.log_file_path,
            default_log_path(),
            "Missing log_file_path in config should use default"
        );
    }
}
