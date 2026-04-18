//! `tanren demo {…}` subcommands — §3.4 demo-frontmatter tools.

use clap::Subcommand;
use tanren_app_services::methodology::PhaseId;
use tanren_app_services::methodology::{CapabilityScope, MethodologyService};
use tanren_contract::methodology::{
    AddDemoStepParams, AppendDemoResultParams, MarkDemoStepSkipParams,
};

use super::{ParamsInput, emit_result, load_params};

#[derive(Debug, Subcommand)]
pub(crate) enum DemoCommand {
    /// Add a demo step to the spec's demo frontmatter.
    AddStep(ParamsInput),
    /// Mark a demo step as skipped (capability-scoped).
    MarkStepSkip(ParamsInput),
    /// Append an observed demo result for a step.
    AppendResult(ParamsInput),
}

pub(crate) async fn run(
    service: &MethodologyService,
    scope: &CapabilityScope,
    phase: &PhaseId,
    cmd: DemoCommand,
) -> u8 {
    match cmd {
        DemoCommand::AddStep(i) => match load_params::<AddDemoStepParams>(&i) {
            Ok(params) => emit_result(
                service
                    .add_demo_step(scope, phase, params)
                    .await
                    .map(|()| Empty {}),
            ),
            Err(e) => emit_result::<()>(Err(e)),
        },
        DemoCommand::MarkStepSkip(i) => match load_params::<MarkDemoStepSkipParams>(&i) {
            Ok(params) => emit_result(
                service
                    .mark_demo_step_skip(scope, phase, params)
                    .await
                    .map(|()| Empty {}),
            ),
            Err(e) => emit_result::<()>(Err(e)),
        },
        DemoCommand::AppendResult(i) => match load_params::<AppendDemoResultParams>(&i) {
            Ok(params) => emit_result(
                service
                    .append_demo_result(scope, phase, params)
                    .await
                    .map(|()| Empty {}),
            ),
            Err(e) => emit_result::<()>(Err(e)),
        },
    }
}

#[derive(serde::Serialize)]
struct Empty {}
