//! `tanren standard list` — §3.8 tool.

use clap::Subcommand;
use tanren_app_services::methodology::PhaseId;
use tanren_app_services::methodology::{CapabilityScope, MethodologyService};
use tanren_contract::methodology::ListRelevantStandardsParams;

use super::{ParamsInput, emit_result, load_params};

#[derive(Debug, Subcommand)]
pub(crate) enum StandardCommand {
    /// List the baseline standards relevant to a spec.
    List(ParamsInput),
}

pub(crate) async fn run(
    service: &MethodologyService,
    scope: &CapabilityScope,
    phase: &PhaseId,
    cmd: StandardCommand,
) -> u8 {
    match cmd {
        StandardCommand::List(i) => match load_params::<ListRelevantStandardsParams>(&i) {
            Ok(params) => emit_result(
                service
                    .list_relevant_standards_filtered(scope, phase, &params)
                    .await,
            ),
            Err(e) => emit_result::<()>(Err(e)),
        },
    }
}
