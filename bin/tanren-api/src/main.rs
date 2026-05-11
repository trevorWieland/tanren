//! Tanren HTTP API server.
//!
//! Thin entry point per
//! `profiles/rust-cargo/architecture/thin-binary-crate.md`. All runtime
//! logic — axum router, account-flow handlers, configurable CORS,
//! tower-sessions cookie middleware, utoipa `OpenAPI` — lives in
//! `tanren-api-app`; this `main` initializes tracing and hands off.

use anyhow::{Context, Result};
use std::io::Write;
use tanren_api_app::{Config, openapi_document, serve};

#[tokio::main]
async fn main() -> Result<()> {
    if std::env::args().any(|arg| arg == "--emit-openapi") {
        let document = openapi_document();
        let encoded = serde_json::to_vec_pretty(&document)?;
        let mut stdout = std::io::stdout().lock();
        stdout.write_all(&encoded)?;
        stdout.write_all(b"\n")?;
        return Ok(());
    }
    tanren_observability::init(tanren_observability::default_filter())
        .context("install tracing subscriber")?;
    let config = Config::from_env().context("load tanren-api config from environment")?;
    serve(config).await
}
