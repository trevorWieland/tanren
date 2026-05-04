//! Tanren HTTP API server.
//!
//! Thin entry point per
//! `profiles/rust-cargo/architecture/thin-binary-crate.md`. All runtime
//! logic — axum router, account-flow handlers, configurable CORS,
//! tower-sessions cookie middleware, utoipa `OpenAPI` — lives in
//! `tanren-api-app`; this `main` initializes tracing and hands off.

use anyhow::{Context, Result};
use tanren_api_app::{Config, serve};

#[tokio::main]
async fn main() -> Result<()> {
    tanren_observability::init(tanren_observability::default_filter())
        .context("install tracing subscriber")?;
    let config = Config::from_env().context("load tanren-api config from environment")?;
    serve(config).await
}
