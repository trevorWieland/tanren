//! `tanren spec {…}` subcommands — §3.3 spec-frontmatter tools.

use clap::Subcommand;
use tanren_app_services::methodology::PhaseId;
use tanren_app_services::methodology::{CapabilityScope, MethodologyService};
use tanren_contract::methodology::{
    AddSpecAcceptanceCriterionParams, SetSpecBaseBranchParams, SetSpecDemoEnvironmentParams,
    SetSpecDependenciesParams, SetSpecNonNegotiablesParams, SetSpecRelevanceContextParams,
    SetSpecTitleParams,
};

use super::{ParamsInput, emit_result, load_params};

#[derive(Debug, Subcommand)]
pub(crate) enum SpecCommand {
    /// Set the spec's title.
    SetTitle(ParamsInput),
    /// Replace the non-negotiables list.
    SetNonNegotiables(ParamsInput),
    /// Append one acceptance criterion.
    AddAcceptanceCriterion(ParamsInput),
    /// Set the demo environment block.
    SetDemoEnvironment(ParamsInput),
    /// Set the dependency graph entries.
    SetDependencies(ParamsInput),
    /// Set the base branch.
    SetBaseBranch(ParamsInput),
    /// Set relevance context used by server-side standards derivation.
    SetRelevanceContext(ParamsInput),
}

pub(crate) async fn run(
    service: &MethodologyService,
    scope: &CapabilityScope,
    phase: &PhaseId,
    cmd: SpecCommand,
) -> u8 {
    match cmd {
        SpecCommand::SetTitle(i) => match load_params::<SetSpecTitleParams>(&i) {
            Ok(params) => emit_result(service.set_spec_title(scope, phase, params).await),
            Err(e) => emit_result::<()>(Err(e)),
        },
        SpecCommand::SetNonNegotiables(i) => match load_params::<SetSpecNonNegotiablesParams>(&i) {
            Ok(params) => emit_result(service.set_spec_non_negotiables(scope, phase, params).await),
            Err(e) => emit_result::<()>(Err(e)),
        },
        SpecCommand::AddAcceptanceCriterion(i) => {
            match load_params::<AddSpecAcceptanceCriterionParams>(&i) {
                Ok(params) => emit_result(
                    service
                        .add_spec_acceptance_criterion(scope, phase, params)
                        .await,
                ),
                Err(e) => emit_result::<()>(Err(e)),
            }
        }
        SpecCommand::SetDemoEnvironment(i) => {
            match load_params::<SetSpecDemoEnvironmentParams>(&i) {
                Ok(params) => emit_result(
                    service
                        .set_spec_demo_environment(scope, phase, params)
                        .await,
                ),
                Err(e) => emit_result::<()>(Err(e)),
            }
        }
        SpecCommand::SetDependencies(i) => match load_params::<SetSpecDependenciesParams>(&i) {
            Ok(params) => emit_result(service.set_spec_dependencies(scope, phase, params).await),
            Err(e) => emit_result::<()>(Err(e)),
        },
        SpecCommand::SetBaseBranch(i) => match load_params::<SetSpecBaseBranchParams>(&i) {
            Ok(params) => emit_result(service.set_spec_base_branch(scope, phase, params).await),
            Err(e) => emit_result::<()>(Err(e)),
        },
        SpecCommand::SetRelevanceContext(i) => {
            match load_params::<SetSpecRelevanceContextParams>(&i) {
                Ok(params) => emit_result(
                    service
                        .set_spec_relevance_context(scope, phase, params)
                        .await,
                ),
                Err(e) => emit_result::<()>(Err(e)),
            }
        }
    }
}
