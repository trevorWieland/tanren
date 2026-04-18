//! `tanren replay <spec-folder>` — ingest a spec folder's
//! `phase-events.jsonl` into the store.
//!
//! `replay` is the spec-folder-aware companion to
//! `ingest-phase-events`. The CLI resolves `<spec-folder>/phase-events.jsonl`
//! and delegates to the same ingest path so the two commands share
//! store semantics and event shape.

use std::path::PathBuf;

use clap::Args;
use tanren_app_services::methodology::{
    MethodologyError, MethodologyService, ReplayStats, ingest_phase_events,
};

use super::emit_result;

#[derive(Debug, Args)]
pub(crate) struct ReplayArgs {
    /// Path to a spec folder (expected to contain `phase-events.jsonl`).
    pub spec_folder: PathBuf,
}

pub(crate) async fn run(service: &MethodologyService, args: ReplayArgs) -> u8 {
    let path = args.spec_folder.join("phase-events.jsonl");
    if !path.is_file() {
        return emit_result::<()>(Err(MethodologyError::NotFound {
            resource: "phase-events.jsonl".into(),
            key: path.display().to_string(),
        }));
    }
    let store = service.store();
    match ingest_phase_events(store, &path).await {
        Ok(stats) => emit_result::<ReplayStats>(Ok(stats)),
        Err(e) => emit_result::<()>(Err(MethodologyError::Internal(e.to_string()))),
    }
}
