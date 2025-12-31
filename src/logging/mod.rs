//! Tracing subscriber initialization.
//!
//! Logs are written to a file instead of being captured in-app.
//! Users can monitor logs via `tail -f` in a separate terminal.

/// Initialize the tracing subscriber with file-based logging.
///
/// Logs are written to a file for users to monitor with `tail -f`.
/// Respects RUST_LOG environment variable, defaults to "info" level.
///
/// # Returns
/// * `Ok(())` if initialization succeeded
/// * `Err(msg)` if the subscriber was already initialized
pub fn init() -> Result<(), String> {
    use tracing_subscriber::EnvFilter;

    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .try_init()
        .map_err(|e| format!("Failed to initialize tracing subscriber: {}", e))
}
