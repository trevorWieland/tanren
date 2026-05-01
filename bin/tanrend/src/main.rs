//! Tanren control-plane daemon.
//!
//! F-0001 ships an empty event loop that boots, logs liveness, and shuts
//! down gracefully on SIGTERM/SIGINT. Concrete control-plane workers
//! (projection, outbox, scheduler, reconciliation) arrive with R-* slices.

use anyhow::{Context, Result};
use tanren_app_services::Handlers;
use tokio::signal;
use tokio::signal::unix::{SignalKind, signal as unix_signal};

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

async fn wait_for_shutdown() -> Result<()> {
    let mut sigterm = unix_signal(SignalKind::terminate()).context("install SIGTERM handler")?;
    tokio::select! {
        _ = signal::ctrl_c() => Ok(()),
        _ = sigterm.recv() => Ok(()),
    }
}
