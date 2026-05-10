//! `xtask export-openapi` — materialize the API's generated `OpenAPI` JSON.

use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

pub(crate) fn run(out: &Path) -> Result<()> {
    let json = tanren_api_app::openapi_json_pretty()?;
    if let Some(parent) = out.parent() {
        fs::create_dir_all(parent).with_context(|| format!("mkdir -p {}", parent.display()))?;
    }
    fs::write(out, json).with_context(|| format!("write {}", out.display()))?;
    Ok(())
}
