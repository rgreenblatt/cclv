//! Tests for configuration file loading.

use super::*;
use serial_test::serial;
use std::env;
use std::fs;

#[test]
fn default_config_path_returns_some_path() {
    let path = default_config_path();
    assert!(
        path.is_some(),
        "default_config_path should return Some on supported platforms"
    );
}

#[test]
fn default_config_path_contains_cclv_config_toml() {
    let path = default_config_path().expect("Should have default path");
    let path_str = path.to_string_lossy();
    assert!(
        path_str.contains("cclv") && path_str.ends_with("config.toml"),
        "Path should contain 'cclv' and end with 'config.toml', got: {}",
        path_str
    );
}

#[test]
fn load_config_file_returns_ok_none_for_missing_file() {
    let result = load_config_file("/nonexistent/path/to/config.toml");
    assert_eq!(
        result,
        Ok(None),
        "Missing config file should return Ok(None), not an error"
    );
}

#[test]
fn load_config_file_parses_valid_toml() {
    // Create temporary config file
    let temp_dir = env::temp_dir();
    let config_path = temp_dir.join("cclv_test_config.toml");

    let toml_content = r#"
theme = "solarized-dark"
show_stats = true
collapse_threshold = 20
summary_lines = 5
line_wrap = false
log_buffer_capacity = 500
"#;

    fs::write(&config_path, toml_content).expect("Failed to write test config");

    let result = load_config_file(&config_path);
    assert!(result.is_ok(), "Should successfully parse valid TOML");

    let config = result.unwrap();
    assert!(
        config.is_some(),
        "Should return Some(ConfigFile) for existing file"
    );

    let config = config.unwrap();
    assert_eq!(config.theme, Some(THEME_SOLARIZED_DARK.to_string()));
    assert_eq!(config.show_stats, Some(true));
    assert_eq!(config.collapse_threshold, Some(20));
    assert_eq!(config.summary_lines, Some(5));
    assert_eq!(config.line_wrap, Some(false));
    assert_eq!(config.log_buffer_capacity, Some(500));

    // Cleanup
    fs::remove_file(config_path).ok();
}

#[test]
fn load_config_file_returns_error_for_invalid_toml() {
    let temp_dir = env::temp_dir();
    let config_path = temp_dir.join("cclv_test_invalid.toml");

    let invalid_toml = "this is not valid TOML ][}{";
    fs::write(&config_path, invalid_toml).expect("Failed to write invalid test config");

    let result = load_config_file(&config_path);
    assert!(
        result.is_err(),
        "Invalid TOML should return Err(ConfigError::ParseError)"
    );

    match result {
        Err(ConfigError::ParseError { path, reason: _ }) => {
            assert_eq!(path, config_path);
        }
        _ => panic!("Expected ParseError, got {:?}", result),
    }

    // Cleanup
    fs::remove_file(config_path).ok();
}

#[test]
fn load_config_file_handles_partial_config() {
    let temp_dir = env::temp_dir();
    let config_path = temp_dir.join("cclv_test_partial.toml");

    let partial_toml = r#"
theme = "monokai"
# Other fields omitted
"#;

    fs::write(&config_path, partial_toml).expect("Failed to write partial test config");

    let result = load_config_file(&config_path);
    assert!(result.is_ok(), "Should parse partial config");

    let config = result.unwrap().unwrap();
    assert_eq!(config.theme, Some(THEME_MONOKAI.to_string()));
    assert_eq!(config.show_stats, None);

    // Cleanup
    fs::remove_file(config_path).ok();
}

#[test]
fn merge_config_uses_defaults_when_none() {
    let resolved = merge_config(None);
    let defaults = ResolvedConfig::default();

    assert_eq!(resolved.theme, defaults.theme);
    assert_eq!(resolved.show_stats, defaults.show_stats);
    assert_eq!(resolved.collapse_threshold, defaults.collapse_threshold);
    assert_eq!(resolved.summary_lines, defaults.summary_lines);
    assert_eq!(resolved.line_wrap, defaults.line_wrap);
    assert_eq!(resolved.log_buffer_capacity, defaults.log_buffer_capacity);
}

#[test]
fn merge_config_overrides_with_config_file_values() {
    let config_file = ConfigFile {
        theme: Some(THEME_SOLARIZED_LIGHT.to_string()),
        show_stats: Some(true),
        collapse_threshold: Some(15),
        summary_lines: Some(2),
        line_wrap: Some(false),
        log_buffer_capacity: Some(2000),
        log_file_path: None,
        keybindings: None,
        pricing: None,
        max_context_tokens: None,
    };

    let resolved = merge_config(Some(config_file));

    assert_eq!(resolved.theme, THEME_SOLARIZED_LIGHT);
    assert!(resolved.show_stats);
    assert_eq!(resolved.collapse_threshold, 15);
    assert_eq!(resolved.summary_lines, 2);
    assert!(!resolved.line_wrap);
    assert_eq!(resolved.log_buffer_capacity, 2000);
}

#[test]
fn merge_config_uses_defaults_for_none_fields() {
    let config_file = ConfigFile {
        theme: Some("custom".to_string()),
        show_stats: None,
        collapse_threshold: None,
        summary_lines: None,
        line_wrap: None,
        log_buffer_capacity: None,
        log_file_path: None,
        keybindings: None,
        pricing: None,
        max_context_tokens: None,
    };

    let resolved = merge_config(Some(config_file));
    let defaults = ResolvedConfig::default();

    assert_eq!(resolved.theme, "custom");
    assert_eq!(resolved.show_stats, defaults.show_stats);
    assert_eq!(resolved.collapse_threshold, defaults.collapse_threshold);
    assert_eq!(resolved.summary_lines, defaults.summary_lines);
    assert_eq!(resolved.line_wrap, defaults.line_wrap);
    assert_eq!(resolved.log_buffer_capacity, defaults.log_buffer_capacity);
}

/// RAII guard to ensure environment variable cleanup even under test parallelism.
/// Removes the var on drop, preventing test pollution in parallel execution.
struct EnvGuard(&'static str);

impl EnvGuard {
    fn new(name: &'static str) -> Self {
        env::remove_var(name);
        EnvGuard(name)
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        env::remove_var(self.0);
    }
}

#[test]
#[serial(cclv_theme)]
fn apply_env_overrides_respects_cclv_theme() {
    let _guard = EnvGuard::new("CCLV_THEME");

    let base = ResolvedConfig::default();

    // Set env var
    env::set_var("CCLV_THEME", "test-theme");

    let result = apply_env_overrides(base.clone());

    assert_eq!(
        result.theme, "test-theme",
        "CCLV_THEME should override theme"
    );
}

#[test]
#[serial(cclv_theme)]
fn apply_env_overrides_leaves_other_fields_unchanged() {
    let _guard = EnvGuard::new("CCLV_THEME");

    let base = ResolvedConfig {
        theme: "original".to_string(),
        show_stats: true,
        collapse_threshold: 99,
        summary_lines: 7,
        line_wrap: false,
        log_buffer_capacity: 123,
        log_file_path: default_log_path(),
        max_context_tokens: 200_000,
    };

    // Set env var
    env::set_var("CCLV_THEME", "override");

    let result = apply_env_overrides(base.clone());

    assert_eq!(result.theme, "override");
    assert_eq!(result.show_stats, base.show_stats);
    assert_eq!(result.collapse_threshold, base.collapse_threshold);
    assert_eq!(result.summary_lines, base.summary_lines);
    assert_eq!(result.line_wrap, base.line_wrap);
    assert_eq!(result.log_buffer_capacity, base.log_buffer_capacity);
}

#[test]
#[serial(cclv_theme)]
fn apply_env_overrides_no_change_when_env_var_not_set() {
    let _guard = EnvGuard::new("CCLV_THEME");

    let base = ResolvedConfig::default();
    let result = apply_env_overrides(base.clone());

    assert_eq!(
        result, base,
        "Config should be unchanged when CCLV_THEME not set"
    );
}

#[test]
#[serial(cclv_config)]
fn load_config_with_precedence_prefers_explicit_path() {
    // Clean up any stale env vars from other tests
    env::remove_var("CCLV_CONFIG");

    let temp_dir = env::temp_dir();
    let explicit_path = temp_dir.join("cclv_explicit.toml");

    fs::write(
        &explicit_path,
        r#"
theme = "explicit-theme"
"#,
    )
    .expect("Failed to write explicit config");

    // Set CCLV_CONFIG to different path (should be ignored)
    let env_path = temp_dir.join("cclv_env.toml");
    fs::write(&env_path, r#"theme = "env-theme""#).expect("Failed to write env config");
    env::set_var("CCLV_CONFIG", env_path.to_str().unwrap());

    let result = load_config_with_precedence(Some(explicit_path.clone()));
    assert!(result.is_ok());

    let config = result.unwrap().unwrap();
    assert_eq!(
        config.theme,
        Some("explicit-theme".to_string()),
        "Should use explicit path, not CCLV_CONFIG env var"
    );

    // Cleanup
    fs::remove_file(explicit_path).ok();
    fs::remove_file(env_path).ok();
    env::remove_var("CCLV_CONFIG");
}

#[test]
#[serial(cclv_config)]
fn load_config_with_precedence_uses_env_var_when_no_explicit_path() {
    // Clean up any stale env vars from other tests
    env::remove_var("CCLV_CONFIG");

    let temp_dir = env::temp_dir();
    let env_path = temp_dir.join("cclv_env_only.toml");

    fs::write(
        &env_path,
        r#"
theme = "env-var-theme"
"#,
    )
    .expect("Failed to write env config");

    env::set_var("CCLV_CONFIG", env_path.to_str().unwrap());

    let result = load_config_with_precedence(None);
    assert!(result.is_ok());

    let config = result.unwrap().unwrap();
    assert_eq!(
        config.theme,
        Some("env-var-theme".to_string()),
        "Should use CCLV_CONFIG when no explicit path"
    );

    // Cleanup
    fs::remove_file(env_path).ok();
    env::remove_var("CCLV_CONFIG");
}

#[test]
#[serial(cclv_config)]
fn load_config_with_precedence_falls_back_to_default_path() {
    // Ensure CCLV_CONFIG not set
    env::remove_var("CCLV_CONFIG");

    // This will try default path, which likely doesn't exist
    let result = load_config_with_precedence(None);
    assert!(result.is_ok());

    // Since default path likely doesn't exist, should be Ok(None)
    assert_eq!(
        result.unwrap(),
        None,
        "Should return Ok(None) when no config exists at default path"
    );
}

#[test]
fn resolved_config_default_has_expected_values() {
    let config = ResolvedConfig::default();

    assert_eq!(config.theme, THEME_BASE16_OCEAN);
    assert!(!config.show_stats);
    assert_eq!(config.collapse_threshold, 10);
    assert_eq!(config.summary_lines, 3);
    assert!(config.line_wrap);
    assert_eq!(config.log_buffer_capacity, 1000);
}

#[test]
fn config_file_rejects_unknown_fields() {
    let toml_with_unknown = r#"
theme = "base16-ocean"
unknown_field = "should fail"
"#;

    let result: Result<ConfigFile, _> = toml::from_str(toml_with_unknown);
    assert!(
        result.is_err(),
        "Should reject TOML with unknown fields due to deny_unknown_fields"
    );
}

// ===== Pricing Section Tests =====

#[test]
fn pricing_section_parses_single_model() {
    let toml_content = r#"
[pricing.models.opus]
input = 15.0
output = 75.0
cached_input = 1.5
"#;

    let config: ConfigFile = toml::from_str(toml_content).expect("Should parse pricing section");

    assert!(config.pricing.is_some(), "Pricing section should be parsed");

    let pricing = config.pricing.unwrap();
    assert_eq!(pricing.models.len(), 1, "Should have one model");

    let opus = pricing.models.get("opus").expect("Should have opus model");
    assert_eq!(opus.input, 15.0, "Input cost should match");
    assert_eq!(opus.output, 75.0, "Output cost should match");
    assert_eq!(
        opus.cached_input,
        Some(1.5),
        "Cached input cost should match"
    );
}

#[test]
fn pricing_section_parses_multiple_models() {
    let toml_content = r#"
[pricing.models.opus]
input = 15.0
output = 75.0
cached_input = 1.5

[pricing.models.sonnet]
input = 3.0
output = 15.0
cached_input = 0.3

[pricing.models.haiku]
input = 0.8
output = 4.0
cached_input = 0.08
"#;

    let config: ConfigFile = toml::from_str(toml_content).expect("Should parse multiple models");

    let pricing = config.pricing.expect("Pricing section should be present");
    assert_eq!(pricing.models.len(), 3, "Should have three models");

    // Check opus
    let opus = pricing.models.get("opus").expect("Should have opus");
    assert_eq!(opus.input, 15.0);
    assert_eq!(opus.output, 75.0);

    // Check sonnet
    let sonnet = pricing.models.get("sonnet").expect("Should have sonnet");
    assert_eq!(sonnet.input, 3.0);
    assert_eq!(sonnet.output, 15.0);

    // Check haiku
    let haiku = pricing.models.get("haiku").expect("Should have haiku");
    assert_eq!(haiku.input, 0.8);
    assert_eq!(haiku.output, 4.0);
}

#[test]
fn pricing_entry_allows_missing_cached_input() {
    let toml_content = r#"
[pricing.models.custom]
input = 10.0
output = 50.0
"#;

    let config: ConfigFile =
        toml::from_str(toml_content).expect("Should parse without cached_input");

    let pricing = config.pricing.expect("Pricing section should be present");
    let custom = pricing
        .models
        .get("custom")
        .expect("Should have custom model");

    assert_eq!(custom.input, 10.0);
    assert_eq!(custom.output, 50.0);
    assert_eq!(
        custom.cached_input, None,
        "Cached input should be None when omitted"
    );
}

#[test]
fn pricing_section_parses_default_pricing() {
    let toml_content = r#"
[pricing.default]
input = 20.0
output = 100.0
cached_input = 2.0
"#;

    let config: ConfigFile = toml::from_str(toml_content).expect("Should parse default pricing");

    let pricing = config.pricing.expect("Pricing section should be present");
    let default = pricing.default.expect("Should have default pricing");

    assert_eq!(default.input, 20.0);
    assert_eq!(default.output, 100.0);
    assert_eq!(default.cached_input, Some(2.0));
}

#[test]
fn pricing_section_parses_models_and_default() {
    let toml_content = r#"
[pricing.models.opus]
input = 15.0
output = 75.0

[pricing.default]
input = 10.0
output = 50.0
"#;

    let config: ConfigFile = toml::from_str(toml_content).expect("Should parse models and default");

    let pricing = config.pricing.expect("Pricing section should be present");
    assert_eq!(pricing.models.len(), 1);
    assert!(pricing.default.is_some());
}

#[test]
fn pricing_section_rejects_unknown_fields_in_entry() {
    let toml_content = r#"
[pricing.models.opus]
input = 15.0
output = 75.0
unknown_field = "fail"
"#;

    let result: Result<ConfigFile, _> = toml::from_str(toml_content);
    assert!(
        result.is_err(),
        "Should reject unknown fields in pricing entry"
    );
}

#[test]
fn pricing_section_rejects_unknown_fields_in_section() {
    let toml_content = r#"
[pricing]
unknown_section = "fail"
"#;

    let result: Result<ConfigFile, _> = toml::from_str(toml_content);
    assert!(
        result.is_err(),
        "Should reject unknown fields in pricing section"
    );
}

#[test]
fn full_config_with_pricing_parses_correctly() {
    let temp_dir = env::temp_dir();
    let config_path = temp_dir.join("cclv_test_full_pricing.toml");

    let toml_content = r#"
theme = "solarized-dark"
show_stats = true

[pricing.models.opus]
input = 15.0
output = 75.0
cached_input = 1.5

[pricing.models.sonnet]
input = 3.0
output = 15.0
"#;

    fs::write(&config_path, toml_content).expect("Failed to write test config");

    let result = load_config_file(&config_path);
    assert!(result.is_ok(), "Should parse full config with pricing");

    let config = result.unwrap().expect("Should have config");
    assert_eq!(config.theme, Some(THEME_SOLARIZED_DARK.to_string()));

    let pricing = config.pricing.expect("Should have pricing section");
    assert_eq!(pricing.models.len(), 2);
    assert!(pricing.models.contains_key("opus"));
    assert!(pricing.models.contains_key("sonnet"));

    // Cleanup
    fs::remove_file(config_path).ok();
}

// ===== CLI Override Tests =====

#[test]
fn apply_cli_overrides_theme_override() {
    let base = ResolvedConfig {
        theme: THEME_BASE16_OCEAN.to_string(),
        show_stats: false,
        collapse_threshold: 10,
        summary_lines: 3,
        line_wrap: true,
        log_buffer_capacity: 1000,
        log_file_path: default_log_path(),
        max_context_tokens: 200_000,
    };

    let result = apply_cli_overrides(base.clone(), Some(THEME_MONOKAI.to_string()), None);

    assert_eq!(result.theme, THEME_MONOKAI, "CLI theme should override");
    assert_eq!(result.show_stats, base.show_stats, "Other fields unchanged");
}

#[test]
fn apply_cli_overrides_stats_override() {
    let base = ResolvedConfig::default();

    let result = apply_cli_overrides(base.clone(), None, Some(true));

    assert!(result.show_stats, "CLI stats should override");
    assert_eq!(result.theme, base.theme, "Other fields unchanged");
}

#[test]
fn apply_cli_overrides_multiple_overrides() {
    let base = ResolvedConfig {
        theme: THEME_BASE16_OCEAN.to_string(),
        show_stats: false,
        collapse_threshold: 10,
        summary_lines: 3,
        line_wrap: true,
        log_buffer_capacity: 1000,
        log_file_path: default_log_path(),
        max_context_tokens: 200_000,
    };

    let result = apply_cli_overrides(base.clone(), Some(THEME_SOLARIZED_DARK.to_string()), Some(true));

    assert_eq!(result.theme, THEME_SOLARIZED_DARK);
    assert!(result.show_stats);
    assert_eq!(
        result.collapse_threshold, base.collapse_threshold,
        "Non-overridden fields unchanged"
    );
}

#[test]
fn apply_cli_overrides_no_overrides() {
    let base = ResolvedConfig::default();

    let result = apply_cli_overrides(base.clone(), None, None);

    assert_eq!(result, base, "No overrides should leave config unchanged");
}

#[test]
fn precedence_chain_defaults_to_config_file() {
    // Test: Defaults → Config File
    let config_file = ConfigFile {
        theme: Some("custom-theme".to_string()),
        show_stats: None,
        collapse_threshold: None,
        summary_lines: None,
        line_wrap: None,
        log_buffer_capacity: None,
        log_file_path: None,
        keybindings: None,
        pricing: None,
        max_context_tokens: None,
    };

    let resolved = merge_config(Some(config_file));

    assert_eq!(
        resolved.theme, "custom-theme",
        "Config file overrides default"
    );
    assert_eq!(
        resolved.show_stats,
        ResolvedConfig::default().show_stats,
        "Defaults used when config file has None"
    );
}

#[test]
#[serial(cclv_theme)]
fn precedence_chain_config_file_to_env_vars() {
    // Clean up
    env::remove_var("CCLV_THEME");

    // Test: Config File → Env Vars
    let config_file = ConfigFile {
        theme: Some("config-theme".to_string()),
        show_stats: None,
        collapse_threshold: None,
        summary_lines: None,
        line_wrap: None,
        log_buffer_capacity: None,
        log_file_path: None,
        keybindings: None,
        pricing: None,
        max_context_tokens: None,
    };

    let merged = merge_config(Some(config_file));
    assert_eq!(merged.theme, "config-theme");

    // Set env var
    env::set_var("CCLV_THEME", "env-theme");
    let with_env = apply_env_overrides(merged);

    assert_eq!(
        with_env.theme, "env-theme",
        "Env var should override config file"
    );

    // Cleanup
    env::remove_var("CCLV_THEME");
}

#[test]
#[serial(cclv_theme)]
fn precedence_chain_env_vars_to_cli_args() {
    // Clean up
    env::remove_var("CCLV_THEME");

    // Test: Env Vars → CLI Args
    let base = ResolvedConfig {
        theme: "base".to_string(),
        show_stats: false,
        collapse_threshold: 10,
        summary_lines: 3,
        line_wrap: true,
        log_buffer_capacity: 1000,
        log_file_path: default_log_path(),
        max_context_tokens: 200_000,
    };

    // Apply env override
    env::set_var("CCLV_THEME", "env-theme");
    let with_env = apply_env_overrides(base);
    assert_eq!(with_env.theme, "env-theme");

    // Apply CLI override
    let with_cli = apply_cli_overrides(with_env, Some("cli-theme".to_string()), None);
    assert_eq!(with_cli.theme, "cli-theme", "CLI should override env var");

    // Cleanup
    env::remove_var("CCLV_THEME");
}

#[test]
#[serial(cclv_theme)]
fn precedence_chain_full_defaults_to_cli() {
    // Clean up
    env::remove_var("CCLV_THEME");

    // Test full precedence chain: Defaults → Config File → Env Vars → CLI Args
    let config_file = ConfigFile {
        theme: Some("config-theme".to_string()),
        show_stats: None,
        collapse_threshold: None,
        summary_lines: None,
        line_wrap: None,
        log_buffer_capacity: None,
        log_file_path: None,
        keybindings: None,
        pricing: None,
        max_context_tokens: None,
    };

    // Step 1: Defaults → Config File
    let merged = merge_config(Some(config_file));
    assert_eq!(merged.theme, "config-theme");

    // Step 2: → Env Vars
    env::set_var("CCLV_THEME", "env-theme");
    let with_env = apply_env_overrides(merged);
    assert_eq!(with_env.theme, "env-theme", "Env overrides config file");

    // Step 3: → CLI Args
    let with_cli = apply_cli_overrides(with_env, Some("cli-theme".to_string()), Some(true));
    assert_eq!(with_cli.theme, "cli-theme", "CLI overrides env");
    assert!(with_cli.show_stats, "CLI overrides default");

    // Cleanup
    env::remove_var("CCLV_THEME");
}

// ===== max_context_tokens Tests =====

#[test]
fn config_file_parses_max_context_tokens() {
    let toml_content = r#"
theme = "base16-ocean"
max_context_tokens = 300000
"#;

    let config: ConfigFile = toml::from_str(toml_content).expect("Should parse max_context_tokens");

    assert_eq!(
        config.max_context_tokens,
        Some(300000),
        "max_context_tokens should be parsed"
    );
}

#[test]
fn config_file_allows_missing_max_context_tokens() {
    let toml_content = r#"
theme = "base16-ocean"
"#;

    let config: ConfigFile =
        toml::from_str(toml_content).expect("Should parse without max_context_tokens");

    assert_eq!(
        config.max_context_tokens, None,
        "max_context_tokens should be None when omitted"
    );
}

#[test]
fn resolved_config_default_max_context_tokens_is_200k() {
    let config = ResolvedConfig::default();

    assert_eq!(
        config.max_context_tokens, 200_000,
        "Default max_context_tokens should be 200,000"
    );
}

#[test]
fn merge_config_uses_config_file_max_context_tokens() {
    let config_file = ConfigFile {
        theme: None,
        show_stats: None,
        collapse_threshold: None,
        summary_lines: None,
        line_wrap: None,
        log_buffer_capacity: None,
        log_file_path: None,
        keybindings: None,
        pricing: None,
        max_context_tokens: Some(500_000),
    };

    let resolved = merge_config(Some(config_file));

    assert_eq!(
        resolved.max_context_tokens, 500_000,
        "Config file max_context_tokens should override default"
    );
}

#[test]
fn merge_config_uses_default_when_max_context_tokens_none() {
    let config_file = ConfigFile {
        theme: None,
        show_stats: None,
        collapse_threshold: None,
        summary_lines: None,
        line_wrap: None,
        log_buffer_capacity: None,
        log_file_path: None,
        keybindings: None,
        pricing: None,
        max_context_tokens: None,
    };

    let resolved = merge_config(Some(config_file));

    assert_eq!(
        resolved.max_context_tokens,
        ResolvedConfig::default().max_context_tokens,
        "Should use default max_context_tokens when config file has None"
    );
}

// Tests for theme constants (cclv-5ur.67.9)
#[test]
fn theme_base16_ocean_constant_matches_string() {
    assert_eq!(
        THEME_BASE16_OCEAN, "base16-ocean",
        "THEME_BASE16_OCEAN constant must match expected string"
    );
}

#[test]
fn theme_solarized_dark_constant_matches_string() {
    assert_eq!(
        THEME_SOLARIZED_DARK, "solarized-dark",
        "THEME_SOLARIZED_DARK constant must match expected string"
    );
}

#[test]
fn theme_solarized_light_constant_matches_string() {
    assert_eq!(
        THEME_SOLARIZED_LIGHT, "solarized-light",
        "THEME_SOLARIZED_LIGHT constant must match expected string"
    );
}

#[test]
fn theme_monokai_constant_matches_string() {
    assert_eq!(
        THEME_MONOKAI, "monokai",
        "THEME_MONOKAI constant must match expected string"
    );
}

#[test]
fn theme_default_points_to_base16_ocean() {
    assert_eq!(
        THEME_DEFAULT, THEME_BASE16_OCEAN,
        "THEME_DEFAULT must point to base16-ocean per CLI contract"
    );
}

#[test]
fn valid_themes_array_contains_all_themes() {
    assert_eq!(
        VALID_THEMES.len(),
        4,
        "VALID_THEMES must contain exactly 4 theme names"
    );
    assert!(
        VALID_THEMES.contains(&THEME_BASE16_OCEAN),
        "VALID_THEMES must contain base16-ocean"
    );
    assert!(
        VALID_THEMES.contains(&THEME_SOLARIZED_DARK),
        "VALID_THEMES must contain solarized-dark"
    );
    assert!(
        VALID_THEMES.contains(&THEME_SOLARIZED_LIGHT),
        "VALID_THEMES must contain solarized-light"
    );
    assert!(
        VALID_THEMES.contains(&THEME_MONOKAI),
        "VALID_THEMES must contain monokai"
    );
}
