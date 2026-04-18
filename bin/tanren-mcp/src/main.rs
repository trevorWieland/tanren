//! `tanren-mcp` — Model Context Protocol server exposing the
//! methodology tool surface over stdio.
//!
//! Lane 0.5 scope:
//!
//! - stdio transport only (rmcp `transport-io` feature).
//! - Capability scope derived from `TANREN_PHASE_CAPABILITIES` env var
//!   supplied by the orchestrator at dispatch time.
//! - Phase name from `TANREN_MCP_PHASE` env var (default `"mcp"`).
//! - Database URL from `TANREN_DATABASE_URL` (default
//!   `"sqlite:tanren.db?mode=rwc"`) — the same store path the CLI
//!   uses, so event trails are byte-identical across transports.
//! - Every tool dispatched through
//!   `tanren_app_services::methodology::MethodologyService`.
//! - `tracing` writes to **stderr** only (non-negotiable #14 — stdout
//!   is reserved for MCP framing).
#![deny(clippy::disallowed_types, clippy::disallowed_methods)]

use std::process::ExitCode;
use std::sync::Arc;

use anyhow::{Context, Result};
use tracing_subscriber::EnvFilter;

mod catalog;
mod dispatch;
mod handler;
mod scope;

#[tokio::main]
async fn main() -> ExitCode {
    if let Err(err) = init_tracing() {
        let _ = writeln_stderr(&format!("failed to initialize tracing: {err}"));
        return ExitCode::from(2);
    }
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            tracing::error!(?err, "tanren-mcp exited with error");
            ExitCode::from(1)
        }
    }
}

async fn run() -> Result<()> {
    let scope = scope::parse_from_env();
    let phase = std::env::var("TANREN_MCP_PHASE").unwrap_or_else(|_| "mcp".to_owned());
    let database_url = std::env::var("TANREN_DATABASE_URL")
        .unwrap_or_else(|_| "sqlite:tanren.db?mode=rwc".to_owned());
    let service = tanren_app_services::compose::build_methodology_service(&database_url)
        .await
        .context("building methodology service")?;
    tracing::info!(
        capability_count = scope.0.len(),
        phase = %phase,
        tools = catalog::all_tools().len(),
        "tanren-mcp starting on stdio transport"
    );
    handler::serve_stdio(scope, Arc::new(service), phase).await
}

fn init_tracing() -> Result<()> {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("tanren_mcp=info,rmcp=warn"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .try_init()
        .map_err(|e| anyhow::anyhow!("tracing init: {e}"))?;
    Ok(())
}

fn writeln_stderr(msg: &str) -> std::io::Result<()> {
    use std::io::Write as _;
    writeln!(std::io::stderr(), "{msg}")
}
