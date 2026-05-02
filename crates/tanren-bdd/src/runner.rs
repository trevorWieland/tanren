//! Cucumber harness runner. Invoked by `just tests`.
//!
//! Looks for `.feature` files under `tests/bdd/features/` (relative to the
//! repository root) and runs them through the Tanren cucumber `World`.
//! In F-0001 the directory is empty, so the harness reports zero scenarios
//! and exits 0 — the assertion is that the pipeline runs without error.

use std::path::PathBuf;
use tanren_bdd::run_features;

#[tokio::main]
async fn main() {
    let features_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/bdd/features");
    run_features(features_dir).await;
}
