use clap::Subcommand;
use tanren_app_services::methodology::{CapabilityScope, MethodologyService, PhaseId};
use tanren_contract::methodology::{RecordCheckResultParams, StartCheckRunParams};

use super::{ParamsInput, emit_result, load_params};

#[derive(Debug, Subcommand)]
pub(crate) enum CheckCommand {
    /// Start a generic check run.
    Start(ParamsInput),
    /// Record a generic check result.
    Result(ParamsInput),
    /// Record a generic check failure.
    Failure(ParamsInput),
}

pub(crate) async fn run(
    service: &MethodologyService,
    scope: &CapabilityScope,
    phase: &PhaseId,
    cmd: CheckCommand,
) -> u8 {
    match cmd {
        CheckCommand::Start(i) => match load_params::<StartCheckRunParams>(&i) {
            Ok(params) => emit_result(service.start_check_run(scope, phase, params).await),
            Err(e) => emit_result::<()>(Err(e)),
        },
        CheckCommand::Result(i) => match load_params::<RecordCheckResultParams>(&i) {
            Ok(params) => emit_result(service.record_check_result(scope, phase, params).await),
            Err(e) => emit_result::<()>(Err(e)),
        },
        CheckCommand::Failure(i) => match load_params::<RecordCheckResultParams>(&i) {
            Ok(params) => emit_result(service.record_check_failure(scope, phase, params).await),
            Err(e) => emit_result::<()>(Err(e)),
        },
    }
}
