//! Tanren MCP (Model Context Protocol) server.
//!
//! F-0001 ships an empty tool registry; the rmcp framework's default
//! [`ServerHandler`] impl serves a `list-tools` response with `[]`.
//! Behavior tools arrive with R-* slices, each adding `#[rmcp::tool]`
//! routes that route through `tanren-app-services`.

use anyhow::{Context, Result};
use rmcp::ServerHandler;
use rmcp::ServiceExt;
use rmcp::transport::io::stdio;

#[derive(Debug, Clone, Default)]
struct TanrenMcp;

impl ServerHandler for TanrenMcp {}

#[tokio::main]
async fn main() -> Result<()> {
    tanren_observability::init().context("install tracing subscriber")?;
    tracing::info!(target: "tanren_mcp", "tanren-mcp starting on stdio transport");

    let service = TanrenMcp
        .serve(stdio())
        .await
        .context("start mcp server on stdio")?;
    service
        .waiting()
        .await
        .context("mcp server exited unexpectedly")?;
    Ok(())
}
