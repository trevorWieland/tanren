//! Tracing, structured logging, and span emission primitives shared by all
//! Tanren binaries.
//!
//! Crates emit logs through the `tracing` macros, never via `println!` /
//! `eprintln!` — the workspace lints forbid the latter. Binaries call
//! [`init`] once at startup to install a global subscriber that respects the
//! `RUST_LOG` environment variable, defaulting to [`default_log_level`] when
//! unset.

use thiserror::Error;
use tracing::Level;
use tracing_subscriber::EnvFilter;

/// Recommended default log level for Tanren binaries when no environment
/// override is supplied.
#[must_use]
pub const fn default_log_level() -> Level {
    Level::INFO
}

/// Install a global `tracing` subscriber for this process. Subsequent calls
/// after a successful install are a no-op-error so binaries can call this
/// idempotently from `main`.
///
/// `RUST_LOG` always wins when set; otherwise the supplied `default_filter`
/// is parsed. Each binary's `main.rs` calls `tanren_observability::init(...)`
/// before any other work so the early `tracing::info!` calls in this crate
/// and downstream crates land in the global subscriber. The
/// `xtask check-tracing-init` AST scan asserts this convention is upheld.
///
/// # Errors
///
/// Returns [`ObservabilityError::FilterParse`] if the resolved filter
/// expression is malformed, or [`ObservabilityError::SubscriberInstall`]
/// if a subscriber is already installed.
pub fn init(default_filter: impl Into<String>) -> Result<(), ObservabilityError> {
    let filter = if std::env::var_os("RUST_LOG").is_some() {
        EnvFilter::try_from_default_env()
            .map_err(|err| ObservabilityError::FilterParse(err.to_string()))?
    } else {
        EnvFilter::try_new(default_filter.into())
            .map_err(|err| ObservabilityError::FilterParse(err.to_string()))?
    };

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .try_init()
        .map_err(|err| ObservabilityError::SubscriberInstall(err.to_string()))
}

/// Convenience: the canonical default filter used by Tanren binaries when
/// `RUST_LOG` is unset. Pairs with [`init`] for the typical
/// `tanren_observability::init(default_filter())` boot sequence.
#[must_use]
pub fn default_filter() -> String {
    format!(
        "{level},tanren=debug",
        level = default_log_level().as_str().to_lowercase()
    )
}

/// Errors raised when initializing the tracing subscriber.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ObservabilityError {
    /// `RUST_LOG` (or the default fallback) failed to parse as a valid
    /// `tracing-subscriber` filter expression.
    #[error("failed to parse tracing filter: {0}")]
    FilterParse(String),
    /// A global subscriber was already installed before this call.
    #[error("failed to install tracing subscriber: {0}")]
    SubscriberInstall(String),
}
