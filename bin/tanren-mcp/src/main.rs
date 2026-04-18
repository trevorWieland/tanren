//! `tanren-mcp` — Model Context Protocol server exposing the
//! methodology tool surface over stdio.
//!
//! Lane 0.5 scope:
//!
//! - stdio transport only (rmcp `transport-io` feature).
//! - Capability scope derived from `TANREN_PHASE_CAPABILITIES` env var
//!   supplied by the orchestrator at dispatch time.
//! - Every tool dispatched through
//!   `tanren_app_services::methodology::MethodologyService` — the same
//!   path the CLI uses, so the event trail is byte-identical across
//!   transports.
//! - `tracing` writes to **stderr** only (non-negotiable #14 — stdout
//!   is reserved for the MCP framing).
#![deny(clippy::disallowed_types, clippy::disallowed_methods)]

use std::process::ExitCode;

use anyhow::Result;
use tracing_subscriber::EnvFilter;

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
    tracing::info!(
        capability_count = scope.0.len(),
        "tanren-mcp starting on stdio transport"
    );
    handler::serve_stdio(scope).await
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
