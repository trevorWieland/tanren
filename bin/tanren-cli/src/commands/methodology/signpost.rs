//! `tanren signpost {add|update-status}` — §3.5 tools.

use clap::Subcommand;
use tanren_app_services::methodology::PhaseId;
use tanren_app_services::methodology::{CapabilityScope, MethodologyService};
use tanren_contract::methodology::{AddSignpostParams, UpdateSignpostStatusParams};

use super::{ParamsInput, emit_result, load_params};

#[derive(Debug, Subcommand)]
pub(crate) enum SignpostCommand {
    /// Add a signpost.
    Add(ParamsInput),
    /// Update a signpost's status (and optional resolution).
    UpdateStatus(ParamsInput),
}

pub(crate) async fn run(
    service: &MethodologyService,
    scope: &CapabilityScope,
    phase: &PhaseId,
    cmd: SignpostCommand,
) -> u8 {
    match cmd {
        SignpostCommand::Add(i) => match load_params::<AddSignpostParams>(&i) {
            Ok(params) => emit_result(service.add_signpost(scope, phase, params).await),
            Err(e) => emit_result::<()>(Err(e)),
        },
        SignpostCommand::UpdateStatus(i) => match load_params::<UpdateSignpostStatusParams>(&i) {
            Ok(params) => emit_result(
                service
                    .update_signpost_status(scope, phase, params)
                    .await
                    .map(|()| Empty {}),
            ),
            Err(e) => emit_result::<()>(Err(e)),
        },
    }
}

#[derive(serde::Serialize)]
struct Empty {}
