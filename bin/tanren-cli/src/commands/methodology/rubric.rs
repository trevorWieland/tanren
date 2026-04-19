//! `tanren rubric {record|compliance}` — §3.2 tools.

use clap::Subcommand;
use tanren_app_services::methodology::PhaseId;
use tanren_app_services::methodology::{CapabilityScope, MethodologyService};
use tanren_contract::methodology::{RecordNonNegotiableComplianceParams, RecordRubricScoreParams};

use super::{ParamsInput, emit_result, load_params};

#[derive(Debug, Subcommand)]
pub(crate) enum RubricCommand {
    /// Record a per-pillar rubric score.
    Record(ParamsInput),
    /// Record a non-negotiable compliance check.
    Compliance(ParamsInput),
}

pub(crate) async fn run(
    service: &MethodologyService,
    scope: &CapabilityScope,
    phase: &PhaseId,
    cmd: RubricCommand,
) -> u8 {
    match cmd {
        RubricCommand::Record(i) => match load_params::<RecordRubricScoreParams>(&i) {
            Ok(params) => emit_result(service.record_rubric_score(scope, phase, params).await),
            Err(e) => emit_result::<()>(Err(e)),
        },
        RubricCommand::Compliance(i) => {
            match load_params::<RecordNonNegotiableComplianceParams>(&i) {
                Ok(params) => emit_result(
                    service
                        .record_non_negotiable_compliance(scope, phase, params)
                        .await,
                ),
                Err(e) => emit_result::<()>(Err(e)),
            }
        }
    }
}
