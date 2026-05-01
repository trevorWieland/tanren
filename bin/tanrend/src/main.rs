//! Tanren control-plane daemon.
//!
//! F-0001 ships an empty event loop that boots, logs liveness, and shuts
//! down gracefully on SIGTERM/SIGINT. Concrete control-plane workers
//! (projection, outbox, scheduler, reconciliation) arrive with R-* slices.

use anyhow::{Context, Result};
use tanren_app_services::Handlers;

#[tokio::main]
async fn main() -> Result<()> {
    tanren_observability::init().context("install tracing subscriber")?;

    let report = Handlers::new().health(env!("CARGO_PKG_VERSION"));
    tracing::info!(
        target: "tanrend",
        version = report.version,
        contract_version = report.contract_version.value(),
        "tanrend started"
    );

    wait_for_shutdown()
        .await
        .context("wait for shutdown signal")?;
    tracing::info!(target: "tanrend", "tanrend shutting down");
    Ok(())
}

/// Wait for SIGINT or, on Unix, SIGTERM. SIGTERM is what Kubernetes and
/// systemd send during normal rollouts; SIGINT is what an interactive
/// `Ctrl+C` produces. Windows lacks a direct SIGTERM equivalent, so the
/// non-Unix branch falls back to SIGINT only.
#[cfg(unix)]
async fn wait_for_shutdown() -> Result<()> {
    use tokio::signal::unix::{SignalKind, signal};
    let mut sigterm = signal(SignalKind::terminate()).context("install SIGTERM handler")?;
    tokio::select! {
        _ = tokio::signal::ctrl_c() => Ok(()),
        _ = sigterm.recv() => Ok(()),
    }
}

#[cfg(not(unix))]
async fn wait_for_shutdown() -> Result<()> {
    tokio::signal::ctrl_c()
        .await
        .context("install SIGINT handler")?;
    Ok(())
}
