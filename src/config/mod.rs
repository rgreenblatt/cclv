//! Configuration module.

pub mod keybindings;
pub mod loader;

pub use keybindings::KeyBindings;
pub use loader::{
    apply_cli_overrides, apply_env_overrides, default_config_path, load_config_file,
    load_config_with_precedence, merge_config, ConfigError, ConfigFile, PricingConfigSection,
    PricingEntry, ResolvedConfig,
};
