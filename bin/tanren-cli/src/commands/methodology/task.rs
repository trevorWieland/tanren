//! `tanren task {…}` subcommands — the §3.1 task-lifecycle tools.

use clap::Subcommand;
use tanren_app_services::methodology::{CapabilityScope, MethodologyService};
use tanren_contract::methodology::{
    AbandonTaskParams, CompleteTaskParams, CreateTaskParams, ListTasksParams,
    MarkTaskGuardSatisfiedParams, ReviseTaskParams, StartTaskParams,
};

use super::{ParamsInput, emit_result, load_params};

#[derive(Debug, Subcommand)]
pub(crate) enum TaskCommand {
    /// Create a new task (emits `TaskCreated`).
    Create(ParamsInput),
    /// Transition `Pending → InProgress` (emits `TaskStarted`).
    Start(ParamsInput),
    /// Transition `InProgress → Implemented` (emits `TaskImplemented`).
    Complete(ParamsInput),
    /// Mark one required completion guard satisfied.
    Guard(ParamsInput),
    /// Non-transitional description/acceptance revision.
    Revise(ParamsInput),
    /// Terminal abandonment with replacements or an explicit
    /// discard note.
    Abandon(ParamsInput),
    /// Read-only projection of tasks for a spec.
    List(ParamsInput),
}

pub(crate) async fn run(
    service: &MethodologyService,
    scope: &CapabilityScope,
    phase: &str,
    cmd: TaskCommand,
) -> u8 {
    match cmd {
        TaskCommand::Create(i) => match load_params::<CreateTaskParams>(&i) {
            Ok(params) => emit_result(service.create_task(scope, phase, params).await),
            Err(e) => emit_result::<()>(Err(e)),
        },
        TaskCommand::Start(i) => match load_params::<StartTaskParams>(&i) {
            Ok(params) => emit_result(
                service
                    .start_task(scope, phase, params)
                    .await
                    .map(|()| Empty {}),
            ),
            Err(e) => emit_result::<()>(Err(e)),
        },
        TaskCommand::Complete(i) => match load_params::<CompleteTaskParams>(&i) {
            Ok(params) => emit_result(
                service
                    .complete_task(scope, phase, params)
                    .await
                    .map(|()| Empty {}),
            ),
            Err(e) => emit_result::<()>(Err(e)),
        },
        TaskCommand::Guard(i) => match load_params::<MarkTaskGuardSatisfiedParams>(&i) {
            Ok(params) => emit_result(
                service
                    .mark_task_guard_satisfied(
                        scope,
                        phase,
                        params.task_id,
                        params.guard,
                        params.idempotency_key,
                    )
                    .await
                    .map(|()| Empty {}),
            ),
            Err(e) => emit_result::<()>(Err(e)),
        },
        TaskCommand::Revise(i) => match load_params::<ReviseTaskParams>(&i) {
            Ok(params) => emit_result(
                service
                    .revise_task(scope, phase, params)
                    .await
                    .map(|()| Empty {}),
            ),
            Err(e) => emit_result::<()>(Err(e)),
        },
        TaskCommand::Abandon(i) => match load_params::<AbandonTaskParams>(&i) {
            Ok(params) => emit_result(
                service
                    .abandon_task(scope, phase, params)
                    .await
                    .map(|()| Empty {}),
            ),
            Err(e) => emit_result::<()>(Err(e)),
        },
        TaskCommand::List(i) => match load_params::<ListTasksParams>(&i) {
            Ok(params) => emit_result(service.list_tasks(scope, phase, params).await),
            Err(e) => emit_result::<()>(Err(e)),
        },
    }
}

/// Empty-body response marker used when a service method returns
/// `()`. Serializes as `{}` so the CLI contract is always a JSON
/// object, never a bare `null`.
#[derive(serde::Serialize)]
struct Empty {}
