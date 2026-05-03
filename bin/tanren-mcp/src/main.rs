//! Tanren MCP (Model Context Protocol) server.
//!
//! Thin entry point per
//! `profiles/rust-cargo/architecture/thin-binary-crate.md`. All runtime
//! logic — rmcp tool registry, API-key middleware, host-header
//! allowlist, streamable-HTTP server — lives in `tanren-mcp-app`; this
//! `main` initializes tracing and hands off.

use anyhow::{Context, Result};
use tanren_mcp_app::{Config, serve};

#[tokio::main]
async fn main() -> Result<()> {
    tanren_observability::init(tanren_observability::default_filter())
        .context("install tracing subscriber")?;
    serve(Config::from_env()).await
}
