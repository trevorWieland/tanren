//! Emit the canonical Tanren API `OpenAPI` document as JSON.

use anyhow::{Context, Result};
use std::io::Write as _;
use tanren_api_app::generate_openapi_document;

fn main() -> Result<()> {
    let openapi = generate_openapi_document();
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    serde_json::to_writer_pretty(&mut handle, &openapi).context("serialize openapi to json")?;
    handle.write_all(b"\n").context("write trailing newline")
}
