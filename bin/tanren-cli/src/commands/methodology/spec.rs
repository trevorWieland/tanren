//! `tanren spec {…}` subcommands — §3.3 spec-frontmatter tools.

use clap::Subcommand;
use tanren_app_services::methodology::PhaseId;
use tanren_app_services::methodology::{CapabilityScope, MethodologyService};
use tanren_contract::methodology::{
    AddSpecAcceptanceCriterionParams, SetSpecBaseBranchParams, SetSpecDemoEnvironmentParams,
    SetSpecDependenciesParams, SetSpecExpectationsParams, SetSpecImplementationPlanParams,
    SetSpecMotivationsParams, SetSpecNonNegotiablesParams, SetSpecPlannedBehaviorsParams,
    SetSpecProblemStatementParams, SetSpecRelevanceContextParams, SetSpecTitleParams,
    SpecStatusParams,
};

use super::{ParamsInput, emit_result, load_params};

#[derive(Debug, Subcommand)]
pub(crate) enum SpecCommand {
    /// Read orchestration status for one spec.
    Status(ParamsInput),
    /// Set the spec's title.
    SetTitle(ParamsInput),
    /// Set the problem statement.
    SetProblemStatement(ParamsInput),
    /// Replace motivations.
    SetMotivations(ParamsInput),
    /// Replace expectations / acceptance intent.
    SetExpectations(ParamsInput),
    /// Replace planned behaviors.
    SetPlannedBehaviors(ParamsInput),
    /// Replace ordered implementation plan.
    SetImplementationPlan(ParamsInput),
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
        SpecCommand::Status(i) => match load_params::<SpecStatusParams>(&i) {
            Ok(params) => emit_result(service.spec_status(scope, phase, params).await),
            Err(e) => emit_result::<()>(Err(e)),
        },
        SpecCommand::SetTitle(i) => match load_params::<SetSpecTitleParams>(&i) {
            Ok(params) => emit_result(service.set_spec_title(scope, phase, params).await),
            Err(e) => emit_result::<()>(Err(e)),
        },
        SpecCommand::SetProblemStatement(i) => {
            match load_params::<SetSpecProblemStatementParams>(&i) {
                Ok(params) => emit_result(
                    service
                        .set_spec_problem_statement(scope, phase, params)
                        .await,
                ),
                Err(e) => emit_result::<()>(Err(e)),
            }
        }
        SpecCommand::SetMotivations(i) => match load_params::<SetSpecMotivationsParams>(&i) {
            Ok(params) => emit_result(service.set_spec_motivations(scope, phase, params).await),
            Err(e) => emit_result::<()>(Err(e)),
        },
        SpecCommand::SetExpectations(i) => match load_params::<SetSpecExpectationsParams>(&i) {
            Ok(params) => emit_result(service.set_spec_expectations(scope, phase, params).await),
            Err(e) => emit_result::<()>(Err(e)),
        },
        SpecCommand::SetPlannedBehaviors(i) => {
            match load_params::<SetSpecPlannedBehaviorsParams>(&i) {
                Ok(params) => emit_result(
                    service
                        .set_spec_planned_behaviors(scope, phase, params)
                        .await,
                ),
                Err(e) => emit_result::<()>(Err(e)),
            }
        }
        SpecCommand::SetImplementationPlan(i) => {
            match load_params::<SetSpecImplementationPlanParams>(&i) {
                Ok(params) => emit_result(
                    service
                        .set_spec_implementation_plan(scope, phase, params)
                        .await,
                ),
                Err(e) => emit_result::<()>(Err(e)),
            }
        }
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
