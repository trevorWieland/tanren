//! `tanren phase {outcome|escalate|reply}` — §3.6 tools.

use clap::Subcommand;
use tanren_app_services::methodology::{CapabilityScope, MethodologyService};
use tanren_contract::methodology::{
    EscalateToBlockerParams, PostReplyDirectiveParams, ReportPhaseOutcomeParams,
};

use super::{ParamsInput, emit_result, load_params};

#[derive(Debug, Subcommand)]
pub(crate) enum PhaseCommand {
    /// Report the typed phase outcome (complete | blocked | error).
    Outcome(ParamsInput),
    /// Escalate to a blocker — capability-scoped to `investigate`.
    Escalate(ParamsInput),
    /// Post a reply directive on a feedback thread —
    /// capability-scoped to `handle-feedback`.
    Reply(ParamsInput),
}

pub(crate) async fn run(
    service: &MethodologyService,
    scope: &CapabilityScope,
    phase: &str,
    cmd: PhaseCommand,
) -> u8 {
    match cmd {
        PhaseCommand::Outcome(i) => match load_params::<ReportPhaseOutcomeParams>(&i) {
            Ok(params) => emit_result(
                service
                    .report_phase_outcome(scope, phase, params)
                    .await
                    .map(|()| Empty {}),
            ),
            Err(e) => emit_result::<()>(Err(e)),
        },
        PhaseCommand::Escalate(i) => match load_params::<EscalateToBlockerParams>(&i) {
            Ok(params) => emit_result(
                service
                    .escalate_to_blocker(scope, phase, params)
                    .await
                    .map(|()| Empty {}),
            ),
            Err(e) => emit_result::<()>(Err(e)),
        },
        PhaseCommand::Reply(i) => match load_params::<PostReplyDirectiveParams>(&i) {
            Ok(params) => emit_result(
                service
                    .post_reply_directive(scope, phase, params)
                    .await
                    .map(|()| Empty {}),
            ),
            Err(e) => emit_result::<()>(Err(e)),
        },
    }
}

#[derive(serde::Serialize)]
struct Empty {}
