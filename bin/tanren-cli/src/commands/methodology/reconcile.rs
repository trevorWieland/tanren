//! `tanren-cli methodology reconcile-phase-events` — reconcile pending
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

#[derive(Debug, clap::Args, Clone)]
pub(crate) struct ReconcileProjectionsArgs {}

#[derive(Debug, Serialize)]
struct ReconcileProjectionsResponse {
    tasks_rebuilt: u64,
    task_spec_rows_repaired: u64,
    signpost_spec_rows_repaired: u64,
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

pub(crate) async fn run_projection_reconcile(
    service: &MethodologyService,
    global: &MethodologyGlobal,
) -> u8 {
    let Some(spec_id) = global.spec_id else {
        return emit_result::<serde_json::Value>(Err(MethodologyError::FieldValidation {
            field_path: "/spec_id".into(),
            expected: "--spec-id <UUID> for reconcile-projections".into(),
            actual: "missing".into(),
            remediation:
                "pass --spec-id <spec_uuid> to rebuild methodology projections for one spec".into(),
        }));
    };
    emit_result(
        service
            .reconcile_methodology_projections_for_spec(spec_id)
            .await
            .map(|report| ReconcileProjectionsResponse {
                tasks_rebuilt: report.tasks_rebuilt,
                task_spec_rows_repaired: report.task_spec_rows_repaired,
                signpost_spec_rows_repaired: report.signpost_spec_rows_repaired,
            }),
    )
}
