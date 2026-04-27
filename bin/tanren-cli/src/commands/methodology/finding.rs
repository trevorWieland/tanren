//! `tanren finding add` — §3.2 tool.

use clap::Subcommand;
use tanren_app_services::methodology::PhaseId;
use tanren_app_services::methodology::{CapabilityScope, MethodologyService};
use tanren_contract::methodology::{
    AddFindingParams, FindingLifecycleParams, ListFindingsParams, SupersedeFindingParams,
};

use super::{ParamsInput, emit_result, load_params};

#[derive(Debug, Subcommand)]
pub(crate) enum FindingCommand {
    /// Add a finding (audit / demo / investigation / feedback).
    Add(ParamsInput),
    /// List findings with projected lifecycle status.
    List(ParamsInput),
    /// Resolve a finding with verification evidence.
    Resolve(ParamsInput),
    /// Reopen a resolved/deferred/superseded finding.
    Reopen(ParamsInput),
    /// Defer a finding.
    Defer(ParamsInput),
    /// Supersede a finding with replacement findings.
    Supersede(ParamsInput),
    /// Record a failed recheck for an open finding.
    StillOpen(ParamsInput),
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
        FindingCommand::List(i) => match load_params::<ListFindingsParams>(&i) {
            Ok(params) => emit_result(service.list_findings(scope, phase, params).await),
            Err(e) => emit_result::<()>(Err(e)),
        },
        FindingCommand::Resolve(i) => match load_params::<FindingLifecycleParams>(&i) {
            Ok(params) => emit_result(service.resolve_finding(scope, phase, params).await),
            Err(e) => emit_result::<()>(Err(e)),
        },
        FindingCommand::Reopen(i) => match load_params::<FindingLifecycleParams>(&i) {
            Ok(params) => emit_result(service.reopen_finding(scope, phase, params).await),
            Err(e) => emit_result::<()>(Err(e)),
        },
        FindingCommand::Defer(i) => match load_params::<FindingLifecycleParams>(&i) {
            Ok(params) => emit_result(service.defer_finding(scope, phase, params).await),
            Err(e) => emit_result::<()>(Err(e)),
        },
        FindingCommand::Supersede(i) => match load_params::<SupersedeFindingParams>(&i) {
            Ok(params) => emit_result(service.supersede_finding(scope, phase, params).await),
            Err(e) => emit_result::<()>(Err(e)),
        },
        FindingCommand::StillOpen(i) => match load_params::<FindingLifecycleParams>(&i) {
            Ok(params) => emit_result(
                service
                    .record_finding_still_open(scope, phase, params)
                    .await,
            ),
            Err(e) => emit_result::<()>(Err(e)),
        },
    }
}
