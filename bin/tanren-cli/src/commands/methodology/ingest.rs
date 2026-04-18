//! `tanren ingest-phase-events` — append a JSONL file into the store.

use std::path::PathBuf;

use clap::Args;
use tanren_app_services::methodology::{
    MethodologyError, MethodologyService, ReplayStats, ingest_phase_events,
};

use super::emit_result;

#[derive(Debug, Args)]
pub(crate) struct IngestArgs {
    /// Path to a `phase-events.jsonl` file.
    pub path: PathBuf,
}

pub(crate) async fn run(service: &MethodologyService, args: IngestArgs) -> u8 {
    let store = service.store();
    match ingest_phase_events(store, &args.path).await {
        Ok(stats) => emit_result::<ReplayStats>(Ok(stats)),
        Err(e) => emit_result::<()>(Err(MethodologyError::Internal(e.to_string()))),
    }
}
