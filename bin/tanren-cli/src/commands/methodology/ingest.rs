//! `tanren ingest-phase-events` — append a JSONL file into the store.

use std::path::PathBuf;

use clap::Args;
use tanren_app_services::methodology::{
    MethodologyError, MethodologyService, ReplayOptions, ReplayStats,
    ingest_phase_events_with_options,
};

use super::emit_result;

#[derive(Debug, Args)]
pub(crate) struct IngestArgs {
    /// Path to a `phase-events.jsonl` file.
    pub path: PathBuf,

    /// Allow replaying legacy lines that omit provenance metadata.
    #[arg(long, default_value_t = false)]
    pub allow_legacy_provenance: bool,
}

pub(crate) async fn run(service: &MethodologyService, args: IngestArgs) -> u8 {
    let store = service.store();
    let options = ReplayOptions {
        allow_legacy_provenance: args.allow_legacy_provenance,
    };
    match ingest_phase_events_with_options(store, &args.path, service.required_guards(), options)
        .await
    {
        Ok(stats) => emit_result::<ReplayStats>(Ok(stats)),
        // Preserve typed `ReplayError` variants (audit finding #10).
        Err(e) => emit_result::<()>(Err(MethodologyError::from(e))),
    }
}
