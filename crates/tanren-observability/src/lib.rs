//! Tracing, structured logging, and span emission primitives shared by all
//! Tanren binaries.
//!
//! Crates emit logs through the `tracing` macros, never via `println!` /
//! `eprintln!` — the workspace lints forbid the latter. Binaries call
//! [`init`] once at startup to install a global subscriber that respects the
//! `RUST_LOG` environment variable, defaulting to [`default_log_level`] when
//! unset.
//!
//! Binaries that own the terminal — currently only `tanren-tui`, which
//! enables raw mode and the alternate screen on stdout — must call
//! [`init_to_file`] instead so log lines never overlay the rendered UI.

use std::path::PathBuf;

use thiserror::Error;
use tracing::Level;
use tracing_appender::non_blocking::WorkerGuard;
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
    let filter = build_filter(default_filter.into())?;
    tracing_subscriber::fmt()
        .with_env_filter(filter)
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

/// Install a tracing subscriber whose writer is a daily-rolling file under
/// the Tanren state directory (or a single fixed path when
/// `TANREN_TUI_LOG_FILE` is set).
///
/// Use this for binaries that own the terminal — currently only
/// `tanren-tui`, which enables raw mode and the alternate screen on stdout
/// (`crates/tanren-tui-app/src/lib.rs::setup_terminal`). Calling [`init`]
/// from such a binary would render `INFO …` lines on top of the ratatui
/// frame.
///
/// `binary_name` is used as the rolling-file prefix (e.g. `tanren-tui` →
/// `tanren-tui.log.YYYY-MM-DD`). Resolution order for the directory:
///
/// 1. `TANREN_TUI_LOG_FILE` (non-empty) — used verbatim as a fixed path;
///    its parent is auto-created and the appender writes a single file.
/// 2. `$XDG_STATE_HOME/tanren/logs/`.
/// 3. `$HOME/.local/state/tanren/logs/`.
/// 4. `./tanren/logs/` (last-resort fallback).
///
/// The returned [`WorkerGuard`] flushes the non-blocking writer on drop;
/// callers must keep it alive for the lifetime of the process (typically
/// `let _log_guard = init_to_file(...)?;` inside `main`).
///
/// # Errors
///
/// Returns [`ObservabilityError::FilterParse`] if the filter expression is
/// malformed, [`ObservabilityError::LogFileIo`] if the log directory cannot
/// be created, or [`ObservabilityError::SubscriberInstall`] if a subscriber
/// is already installed.
pub fn init_to_file(
    default_filter: impl Into<String>,
    binary_name: &str,
) -> Result<WorkerGuard, ObservabilityError> {
    let filter = build_filter(default_filter.into())?;
    let (non_blocking, guard) = build_file_writer(binary_name)?;

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(non_blocking)
        .with_ansi(false)
        .try_init()
        .map_err(|err| ObservabilityError::SubscriberInstall(err.to_string()))?;
    Ok(guard)
}

/// Resolve the Tanren state directory using the same XDG cascade the rest
/// of the workspace observes (CLI session file, TUI log dir, etc.). Returns
/// `$XDG_STATE_HOME/tanren`, `$HOME/.local/state/tanren`, or `./tanren` as
/// last resort.
#[must_use]
pub fn xdg_state_dir() -> PathBuf {
    if let Ok(explicit) = std::env::var("XDG_STATE_HOME") {
        if !explicit.is_empty() {
            return PathBuf::from(explicit).join("tanren");
        }
    }
    if let Ok(home) = std::env::var("HOME") {
        if !home.is_empty() {
            return PathBuf::from(home).join(".local/state").join("tanren");
        }
    }
    PathBuf::from(".").join("tanren")
}

fn build_filter(default_filter: String) -> Result<EnvFilter, ObservabilityError> {
    if std::env::var_os("RUST_LOG").is_some() {
        EnvFilter::try_from_default_env()
            .map_err(|err| ObservabilityError::FilterParse(err.to_string()))
    } else {
        EnvFilter::try_new(default_filter)
            .map_err(|err| ObservabilityError::FilterParse(err.to_string()))
    }
}

fn build_file_writer(
    binary_name: &str,
) -> Result<(tracing_appender::non_blocking::NonBlocking, WorkerGuard), ObservabilityError> {
    if let Ok(explicit) = std::env::var("TANREN_TUI_LOG_FILE") {
        if !explicit.is_empty() {
            let path = PathBuf::from(explicit);
            if let Some(parent) = path.parent() {
                if !parent.as_os_str().is_empty() {
                    std::fs::create_dir_all(parent).map_err(|err| {
                        ObservabilityError::LogFileIo(format!(
                            "create log dir {}: {err}",
                            parent.display()
                        ))
                    })?;
                }
            }
            let dir = path.parent().map_or_else(|| PathBuf::from("."), Into::into);
            let file_name = path.file_name().map_or_else(
                || format!("{binary_name}.log").into(),
                std::ffi::OsStr::to_os_string,
            );
            let appender = tracing_appender::rolling::never(dir, file_name);
            let (nb, guard) = tracing_appender::non_blocking(appender);
            return Ok((nb, guard));
        }
    }

    let dir = xdg_state_dir().join("logs");
    std::fs::create_dir_all(&dir).map_err(|err| {
        ObservabilityError::LogFileIo(format!("create log dir {}: {err}", dir.display()))
    })?;
    let appender = tracing_appender::rolling::daily(dir, format!("{binary_name}.log"));
    let (nb, guard) = tracing_appender::non_blocking(appender);
    Ok((nb, guard))
}

/// Errors raised when initializing the tracing subscriber.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ObservabilityError {
    /// `RUST_LOG` (or the default fallback) failed to parse as a valid
    /// `tracing-subscriber` filter expression.
    #[error("failed to parse tracing filter: {0}")]
    FilterParse(String),
    /// The log directory could not be created or the appender failed to
    /// open its target file.
    #[error("log file io: {0}")]
    LogFileIo(String),
    /// A global subscriber was already installed before this call.
    #[error("failed to install tracing subscriber: {0}")]
    SubscriberInstall(String),
}
