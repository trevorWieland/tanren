//! `tanren methodology reconcile-phase-events` — reconcile pending
//! outbox rows to `phase-events.jsonl`.

use serde::Serialize;
use tanren_app_services::methodology::{MethodologyError, MethodologyService};

use super::{MethodologyGlobal, emit_result};

#[derive(Debug, clap::Args, Clone)]
pub(crate) struct ReconcilePhaseEventsArgs {}

#[derive(Debug, Serialize)]
struct ReconcilePhaseEventsResponse {
    projected: u64,
}

pub(crate) async fn run(service: &MethodologyService, global: &MethodologyGlobal) -> u8 {
    let Some(spec_folder) = global.spec_folder.as_deref() else {
        return emit_result::<serde_json::Value>(Err(MethodologyError::FieldValidation {
            field_path: "/spec_folder".into(),
            expected: "--spec-folder <PATH> for reconcile-phase-events".into(),
            actual: "missing".into(),
            remediation: "pass --spec-folder <spec_dir> to select which outbox rows to reconcile"
                .into(),
        }));
    };
    emit_result(
        service
            .reconcile_phase_events_outbox_for_folder(spec_folder)
            .await
            .map(|projected| ReconcilePhaseEventsResponse { projected }),
    )
}
