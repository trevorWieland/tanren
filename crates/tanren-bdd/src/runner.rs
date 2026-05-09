//! Cucumber harness runner. Invoked by `just tests`.
//!
//! By default this scans `tests/bdd/features/` (relative to the repository
//! root) and runs all scenarios through the Tanren cucumber `World`.
//! Callers may optionally set `TANREN_BDD_FEATURES_PATH` to a specific
//! `.feature` path to run one behavior witness explicitly.

use std::path::PathBuf;
use tanren_bdd::run_features;

#[tokio::main]
async fn main() {
    let default_features_dir =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/bdd/features");
    let features_path =
        std::env::var_os("TANREN_BDD_FEATURES_PATH").map_or(default_features_dir, PathBuf::from);
    run_features(features_path).await;
}
