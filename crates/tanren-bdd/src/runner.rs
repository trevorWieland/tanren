//! Cucumber harness runner. Invoked by `just tests`.
//!
//! Looks for `.feature` files under `tests/bdd/features/` (relative to the
//! repository root) and runs them through the Tanren cucumber `World`.

use std::path::PathBuf;

use anyhow::Context;
use tanren_bdd::run_features;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tanren_observability::init("error,tanren_testkit::tui=info")
        .context("install tracing subscriber")?;
    let features_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/bdd/features");
    run_features(features_dir).await;
    Ok(())
}
