//! Claude Code Log Viewer - Entry Point

use clap::Parser;
use std::path::PathBuf;
use tracing::info;

/// Claude Code Log Viewer - TUI for viewing Claude Code JSONL logs
#[derive(Parser, Debug)]
#[command(name = "cclv")]
#[command(version)]
#[command(about = "TUI application for viewing Claude Code JSONL session logs")]
pub struct Args {
    /// Path to JSONL log file (reads from stdin if not provided)
    pub file: Option<PathBuf>,

    /// Follow the file for new content (like tail -f)
    #[arg(short, long)]
    pub follow: bool,

    /// Start at specific line number (must be positive)
    #[arg(short, long, default_value = "1", value_parser = clap::value_parser!(u32).range(1..))]
    pub line: u32,

    /// Start with search query active
    #[arg(short, long)]
    pub search: Option<String>,

    /// Show statistics panel on startup
    #[arg(long)]
    pub stats: bool,

    /// Disable colors
    #[arg(long)]
    pub no_color: bool,

    /// Color theme for syntax highlighting
    #[arg(long, default_value = "base16-ocean", value_parser = ["base16-ocean", "solarized-dark", "solarized-light", "monokai"])]
    pub theme: String,

    /// Path to configuration file
    #[arg(long)]
    pub config: Option<PathBuf>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    // Set NO_COLOR env var if --no-color flag is passed
    // This ensures consistent color handling throughout the application
    if args.no_color {
        std::env::set_var("NO_COLOR", "1");
    }

    // Load configuration with full precedence chain:
    // Defaults → Config File → Env Vars → CLI Args
    let config = {
        // 1. Load config file (or None if missing)
        let config_file = cclv::config::load_config_with_precedence(args.config.clone())?;

        // 2. Merge with defaults
        let merged = cclv::config::merge_config(config_file);

        // 3. Apply environment variable overrides
        let with_env = cclv::config::apply_env_overrides(merged);

        // 4. Apply CLI argument overrides
        // For theme: always use CLI value (has default)
        // For follow/stats: only override if flag was explicitly set (true)
        let theme_override = Some(args.theme.clone());
        let follow_override = if args.follow { Some(true) } else { None };
        let stats_override = if args.stats { Some(true) } else { None };

        cclv::config::apply_cli_overrides(with_env, theme_override, follow_override, stats_override)
    };

    // Initialize tracing with configured log file path (FR-054/055)
    cclv::logging::init(&config.log_file_path)?;

    info!(
        config = ?config,
        "Configuration loaded and resolved"
    );

    // Detect input source (file or stdin)
    let input_source = cclv::source::detect_input_source(args.file.clone())?;

    // Create CliArgs for TUI using resolved config
    let cli_args = cclv::view::CliArgs::new(config.theme, config.show_stats, config.follow);

    // Run the TUI with the input source
    cclv::view::run_with_source(input_source, cli_args)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn test_help_does_not_error() {
        // Help should succeed (exits with code 0)
        let result = Args::try_parse_from(["cclv", "--help"]);
        // Help returns Err with DisplayHelp, which is success
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.kind(), clap::error::ErrorKind::DisplayHelp);
    }

    #[test]
    fn test_version_does_not_error() {
        let result = Args::try_parse_from(["cclv", "--version"]);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.kind(), clap::error::ErrorKind::DisplayVersion);
    }

    #[test]
    fn test_no_args_defaults() {
        let args = Args::parse_from(["cclv"]);
        assert_eq!(args.file, None);
        assert!(!args.follow);
        assert_eq!(args.line, 1);
        assert_eq!(args.search, None);
        assert!(!args.stats);
        assert!(!args.no_color);
        assert_eq!(args.theme, "base16-ocean");
        assert_eq!(args.config, None);
    }

    #[test]
    fn test_file_path_populates_file_field() {
        let args = Args::parse_from(["cclv", "test.jsonl"]);
        assert_eq!(args.file, Some(PathBuf::from("test.jsonl")));
    }

    #[test]
    fn test_follow_flag_short() {
        let args = Args::parse_from(["cclv", "-f"]);
        assert!(args.follow);
    }

    #[test]
    fn test_follow_flag_long() {
        let args = Args::parse_from(["cclv", "--follow"]);
        assert!(args.follow);
    }

    #[test]
    fn test_line_short_flag() {
        let args = Args::parse_from(["cclv", "-l", "50"]);
        assert_eq!(args.line, 50);
    }

    #[test]
    fn test_line_long_flag() {
        let args = Args::parse_from(["cclv", "--line", "100"]);
        assert_eq!(args.line, 100);
    }

    #[test]
    fn test_line_default_is_one() {
        let args = Args::parse_from(["cclv"]);
        assert_eq!(args.line, 1);
    }

    #[test]
    fn test_line_rejects_zero() {
        let result = Args::try_parse_from(["cclv", "-l", "0"]);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.kind(), clap::error::ErrorKind::ValueValidation);
    }

    #[test]
    fn test_line_rejects_negative() {
        let result = Args::try_parse_from(["cclv", "-l", "-1"]);
        assert!(result.is_err());
    }

    #[test]
    fn test_stats_flag() {
        let args = Args::parse_from(["cclv", "--stats"]);
        assert!(args.stats);
    }

    #[test]
    fn test_no_color_flag() {
        let args = Args::parse_from(["cclv", "--no-color"]);
        assert!(args.no_color);
    }

    #[test]
    fn test_search_short_flag() {
        let args = Args::parse_from(["cclv", "-s", "error"]);
        assert_eq!(args.search, Some("error".to_string()));
    }

    #[test]
    fn test_search_long_flag() {
        let args = Args::parse_from(["cclv", "--search", "warning"]);
        assert_eq!(args.search, Some("warning".to_string()));
    }

    #[test]
    fn test_theme_base16_ocean() {
        let args = Args::parse_from(["cclv", "--theme", "base16-ocean"]);
        assert_eq!(args.theme, "base16-ocean");
    }

    #[test]
    fn test_theme_solarized_dark() {
        let args = Args::parse_from(["cclv", "--theme", "solarized-dark"]);
        assert_eq!(args.theme, "solarized-dark");
    }

    #[test]
    fn test_theme_solarized_light() {
        let args = Args::parse_from(["cclv", "--theme", "solarized-light"]);
        assert_eq!(args.theme, "solarized-light");
    }

    #[test]
    fn test_theme_monokai() {
        let args = Args::parse_from(["cclv", "--theme", "monokai"]);
        assert_eq!(args.theme, "monokai");
    }

    #[test]
    fn test_theme_invalid_rejects() {
        let result = Args::try_parse_from(["cclv", "--theme", "invalid-theme"]);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.kind(), clap::error::ErrorKind::InvalidValue);
    }

    #[test]
    fn test_config_path() {
        let args = Args::parse_from(["cclv", "--config", "/custom/config.toml"]);
        assert_eq!(args.config, Some(PathBuf::from("/custom/config.toml")));
    }

    #[test]
    fn test_combined_flags() {
        let args = Args::parse_from([
            "cclv",
            "session.jsonl",
            "-f",
            "-l",
            "42",
            "-s",
            "error",
            "--stats",
            "--theme",
            "monokai",
        ]);
        assert_eq!(args.file, Some(PathBuf::from("session.jsonl")));
        assert!(args.follow);
        assert_eq!(args.line, 42);
        assert_eq!(args.search, Some("error".to_string()));
        assert!(args.stats);
        assert_eq!(args.theme, "monokai");
    }

    #[test]
    fn test_theme_flows_through_config_precedence_chain() {
        use cclv::config::{apply_cli_overrides, apply_env_overrides, merge_config, ConfigFile};

        // Simulate full precedence chain: Defaults → Config File → Env Vars → CLI Args
        let config_file = ConfigFile {
            theme: Some("solarized-dark".to_string()),
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

        // Step 1: Merge with defaults
        let merged = merge_config(Some(config_file));
        assert_eq!(
            merged.theme, "solarized-dark",
            "Config file should override default theme"
        );

        // Step 2: Apply env override (simulated - not actually setting env var)
        let with_env = apply_env_overrides(merged);
        // Theme unchanged since CCLV_THEME not set
        assert_eq!(with_env.theme, "solarized-dark");

        // Step 3: Apply CLI override
        let with_cli = apply_cli_overrides(with_env, Some("monokai".to_string()), None, None);
        assert_eq!(
            with_cli.theme, "monokai",
            "CLI theme should override all other sources"
        );
    }

    #[test]
    fn test_theme_default_is_base16_ocean() {
        use cclv::config::ResolvedConfig;

        let config = ResolvedConfig::default();
        assert_eq!(
            config.theme, "base16-ocean",
            "Default theme should be base16-ocean per CLI contract"
        );
    }
}
