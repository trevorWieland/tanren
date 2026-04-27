//! `tanren issue create` — §3.7 tool.

use clap::Subcommand;
use tanren_app_services::methodology::PhaseId;
use tanren_app_services::methodology::{CapabilityScope, MethodologyService};
use tanren_contract::methodology::CreateIssueParams;

use super::{ParamsInput, emit_result, load_params};

#[derive(Debug, Subcommand)]
pub(crate) enum IssueCommand {
    /// Create a backlog issue.
    Create(ParamsInput),
}

pub(crate) async fn run(
    service: &MethodologyService,
    scope: &CapabilityScope,
    phase: &PhaseId,
    cmd: IssueCommand,
) -> u8 {
    match cmd {
        IssueCommand::Create(i) => match load_params::<CreateIssueParams>(&i) {
            Ok(params) => emit_result(service.create_issue(scope, phase, params).await),
            Err(e) => emit_result::<()>(Err(e)),
        },
    }
}
