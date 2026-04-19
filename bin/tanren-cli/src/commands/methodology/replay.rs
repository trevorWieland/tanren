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
    MethodologyError, MethodologyService, ReplayOptions, ReplayStats,
    ingest_phase_events_with_options,
};

use super::emit_result;

#[derive(Debug, Args)]
pub(crate) struct ReplayArgs {
    /// Path to a spec folder (expected to contain `phase-events.jsonl`).
    pub spec_folder: PathBuf,

    /// Allow replaying legacy lines that omit provenance metadata.
    #[arg(long, default_value_t = false)]
    pub allow_legacy_provenance: bool,
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
    let options = ReplayOptions {
        allow_legacy_provenance: args.allow_legacy_provenance,
    };
    match ingest_phase_events_with_options(store, &path, service.required_guards(), options).await {
        Ok(stats) => emit_result::<ReplayStats>(Ok(stats)),
        // Preserve typed `ReplayError` variants as structured
        // `MethodologyError::Replay*`/`Io` so the CLI stderr output
        // surfaces `{ field_path, expected, actual, remediation }`
        // with the line number + raw snippet (audit finding #10).
        Err(e) => emit_result::<()>(Err(MethodologyError::from(e))),
    }
}
