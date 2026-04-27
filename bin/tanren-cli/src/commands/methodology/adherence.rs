//! `tanren adherence add-finding` — §3.8 tool.

use clap::Subcommand;
use tanren_app_services::methodology::PhaseId;
use tanren_app_services::methodology::{CapabilityScope, MethodologyService};
use tanren_contract::methodology::RecordAdherenceFindingParams;

use super::{ParamsInput, emit_result, load_params};

#[derive(Debug, Subcommand)]
pub(crate) enum AdherenceCommand {
    /// Record an adherence finding (enforces critical-cannot-defer).
    AddFinding(ParamsInput),
}

pub(crate) async fn run(
    service: &MethodologyService,
    scope: &CapabilityScope,
    phase: &PhaseId,
    cmd: AdherenceCommand,
) -> u8 {
    match cmd {
        AdherenceCommand::AddFinding(i) => match load_params::<RecordAdherenceFindingParams>(&i) {
            Ok(params) => emit_result(service.record_adherence_finding(scope, phase, params).await),
            Err(e) => emit_result::<()>(Err(e)),
        },
    }
}
