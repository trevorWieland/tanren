//! Shared telemetry primitives for the tanren workspace.
//!
//! # Responsibilities
//!
//! - Tracing subscriber initialization for binary crates
//! - Structured logging with configurable filter levels
//! - Foundation for future OpenTelemetry integration (Lane 0.5+)
//!
//! # Design Rules
//!
//! - No crate emits unstructured logs without correlation context
//! - All telemetry uses structured tracing, never `println!` or `eprintln!`
//! - Binary crates call [`init_tracing`] once at startup

use tracing_subscriber::EnvFilter;

/// Errors that can occur during observability initialization.
#[derive(Debug, thiserror::Error)]
pub enum ObservabilityError {
    /// The log filter string could not be parsed.
    #[error("failed to parse log filter: {0}")]
    FilterParse(String),

    /// The tracing subscriber has already been initialized.
    #[error("tracing subscriber already initialized")]
    AlreadyInitialized,
}

/// Initialize the global tracing subscriber with the given filter level.
///
/// Call once at binary startup. Uses compact human-readable format
/// suitable for CLI and development use. Future lanes will add JSON
/// format for daemon/production use.
///
/// The `level` parameter accepts any valid [`EnvFilter`] directive:
/// - Simple levels: `"info"`, `"debug"`, `"warn"`
/// - Targeted: `"tanren=debug,sea_orm=warn"`
///
/// # Errors
///
/// Returns [`ObservabilityError::FilterParse`] if the level string is
/// invalid, or [`ObservabilityError::AlreadyInitialized`] if the global
/// subscriber was already set.
pub fn init_tracing(level: &str) -> Result<(), ObservabilityError> {
    let filter =
        EnvFilter::try_new(level).map_err(|e| ObservabilityError::FilterParse(e.to_string()))?;

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(true)
        .compact()
        .try_init()
        .map_err(|_| ObservabilityError::AlreadyInitialized)
}
