use clap::Subcommand;
use tanren_app_services::methodology::{CapabilityScope, MethodologyService, PhaseId};
use tanren_contract::methodology::{
    LinkRootCauseToFindingParams, ListInvestigationAttemptsParams, RecordInvestigationAttemptParams,
};

use super::{ParamsInput, emit_result, load_params};

#[derive(Debug, Subcommand)]
pub(crate) enum InvestigationCommand {
    /// Record a durable investigation attempt.
    RecordAttempt(ParamsInput),
    /// List durable investigation attempts for a fingerprint.
    ListAttempts(ParamsInput),
    /// Link a root cause to a finding.
    LinkRootCause(ParamsInput),
}

pub(crate) async fn run(
    service: &MethodologyService,
    scope: &CapabilityScope,
    phase: &PhaseId,
    cmd: InvestigationCommand,
) -> u8 {
    match cmd {
        InvestigationCommand::RecordAttempt(i) => {
            match load_params::<RecordInvestigationAttemptParams>(&i) {
                Ok(params) => emit_result(
                    service
                        .record_investigation_attempt(scope, phase, params)
                        .await,
                ),
                Err(e) => emit_result::<()>(Err(e)),
            }
        }
        InvestigationCommand::ListAttempts(i) => {
            match load_params::<ListInvestigationAttemptsParams>(&i) {
                Ok(params) => emit_result(
                    service
                        .list_investigation_attempts(scope, phase, params)
                        .await,
                ),
                Err(e) => emit_result::<()>(Err(e)),
            }
        }
        InvestigationCommand::LinkRootCause(i) => {
            match load_params::<LinkRootCauseToFindingParams>(&i) {
                Ok(params) => emit_result(
                    service
                        .link_root_cause_to_finding(scope, phase, params)
                        .await,
                ),
                Err(e) => emit_result::<()>(Err(e)),
            }
        }
    }
}
