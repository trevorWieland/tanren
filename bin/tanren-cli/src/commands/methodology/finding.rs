//! `tanren finding add` — §3.2 tool.

use clap::Subcommand;
use tanren_app_services::methodology::PhaseId;
use tanren_app_services::methodology::{CapabilityScope, MethodologyService};
use tanren_contract::methodology::AddFindingParams;

use super::{ParamsInput, emit_result, load_params};

#[derive(Debug, Subcommand)]
pub(crate) enum FindingCommand {
    /// Add a finding (audit / demo / investigation / feedback).
    Add(ParamsInput),
}

pub(crate) async fn run(
    service: &MethodologyService,
    scope: &CapabilityScope,
    phase: &PhaseId,
    cmd: FindingCommand,
) -> u8 {
    match cmd {
        FindingCommand::Add(i) => match load_params::<AddFindingParams>(&i) {
            Ok(params) => emit_result(service.add_finding(scope, phase, params).await),
            Err(e) => emit_result::<()>(Err(e)),
        },
    }
}
