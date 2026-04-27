//! `tanren compliance record` — §3.2 non-negotiable compliance tool.

use clap::Subcommand;
use tanren_app_services::methodology::PhaseId;
use tanren_app_services::methodology::{CapabilityScope, MethodologyService};
use tanren_contract::methodology::RecordNonNegotiableComplianceParams;

use super::{ParamsInput, emit_result, load_params};

#[derive(Debug, Subcommand)]
pub(crate) enum ComplianceCommand {
    /// Record a non-negotiable compliance check.
    Record(ParamsInput),
}

pub(crate) async fn run(
    service: &MethodologyService,
    scope: &CapabilityScope,
    phase: &PhaseId,
    cmd: ComplianceCommand,
) -> u8 {
    match cmd {
        ComplianceCommand::Record(i) => {
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
