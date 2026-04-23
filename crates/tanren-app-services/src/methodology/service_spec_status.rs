//! Spec-status query surface used by Phase 0 orchestration.

use tanren_contract::methodology::{
    ListTasksParams, SchemaVersion, SpecStatusNextAction, SpecStatusNextStep, SpecStatusParams,
    SpecStatusResponse,
};
use tanren_domain::EntityKind;
use tanren_domain::NonEmptyString;
use tanren_domain::TaskId;
use tanren_domain::events::DomainEvent;
use tanren_domain::methodology::capability::{CapabilityScope, ToolCapability};
use tanren_domain::methodology::events::MethodologyEvent;
use tanren_domain::methodology::phase_id::KnownPhase;
use tanren_domain::methodology::phase_id::PhaseId;
use tanren_domain::methodology::phase_outcome::{BlockedReason, PhaseOutcome};
use tanren_domain::methodology::task::{RequiredGuard, Task, TaskStatus};
use tanren_store::{EventFilter, EventStore};

use super::capabilities::enforce;
use super::errors::MethodologyResult;
use super::service::MethodologyService;

#[derive(Debug, Clone, Copy, Default)]
struct TaskStatusCounts {
    total: u64,
    pending: u64,
    in_progress: u64,
    implemented: u64,
    complete: u64,
    abandoned: u64,
}

fn fold_task_counts(tasks: &[Task]) -> TaskStatusCounts {
    let mut counts = TaskStatusCounts::default();
    for task in tasks {
        counts.total += 1;
        match task.status {
            TaskStatus::Pending => counts.pending += 1,
            TaskStatus::InProgress => counts.in_progress += 1,
            TaskStatus::Implemented { .. } => counts.implemented += 1,
            TaskStatus::Complete => counts.complete += 1,
            TaskStatus::Abandoned { .. } => counts.abandoned += 1,
        }
    }
    counts
}

fn next_open_task(tasks: &[Task]) -> Option<&Task> {
    tasks.iter().find(|task| !task.status.is_terminal())
}

fn pending_required_guards(task: &Task, required_guards: &[RequiredGuard]) -> Vec<String> {
    let TaskStatus::Implemented { guards } = &task.status else {
        return Vec::new();
    };
    required_guards
        .iter()
        .filter(|guard| !guards.get(guard))
        .map(ToString::to_string)
        .collect()
}

fn next_step_from_task(
    task: Option<&Task>,
    required_guards: &[RequiredGuard],
) -> (SpecStatusNextStep, Vec<String>, Option<String>) {
    let Some(task) = task else {
        return (
            SpecStatusNextStep::SpecPipeline,
            Vec::new(),
            Some("no open tasks; run spec-level checks".to_owned()),
        );
    };
    match &task.status {
        TaskStatus::Pending | TaskStatus::InProgress => (
            SpecStatusNextStep::TaskDoTask,
            Vec::new(),
            Some(format!(
                "task {} is {}; run do-task",
                task.id,
                task.status.tag()
            )),
        ),
        TaskStatus::Implemented { guards } => {
            let pending = pending_required_guards(task, required_guards);
            for guard in required_guards {
                if guards.get(guard) {
                    continue;
                }
                let (step, reason) = match guard {
                    RequiredGuard::GateChecked => (
                        SpecStatusNextStep::TaskGate,
                        format!("task {} missing guard gate_checked", task.id),
                    ),
                    RequiredGuard::Audited => (
                        SpecStatusNextStep::TaskAudit,
                        format!("task {} missing guard audited", task.id),
                    ),
                    RequiredGuard::Adherent => (
                        SpecStatusNextStep::TaskAdhere,
                        format!("task {} missing guard adherent", task.id),
                    ),
                    RequiredGuard::Extra(name) => (
                        SpecStatusNextStep::TaskAdhere,
                        format!(
                            "task {} missing extra guard `{name}`; defaulting to adhere-task",
                            task.id
                        ),
                    ),
                };
                return (step, pending, Some(reason));
            }
            (
                SpecStatusNextStep::SpecPipeline,
                Vec::new(),
                Some(format!(
                    "task {} has all required guards; waiting completion convergence",
                    task.id
                )),
            )
        }
        TaskStatus::Complete | TaskStatus::Abandoned { .. } => (
            SpecStatusNextStep::SpecPipeline,
            Vec::new(),
            Some(format!(
                "task {} is terminal; run spec-level checks",
                task.id
            )),
        ),
    }
}

#[derive(Debug, Clone)]
struct PendingInvestigation {
    source_phase: String,
    source_outcome: String,
    source_summary: NonEmptyString,
    task_scoped: bool,
}

fn phase_is_task_scoped(phase: &PhaseId) -> bool {
    matches!(
        phase.known(),
        Some(KnownPhase::DoTask | KnownPhase::AuditTask | KnownPhase::AdhereTask)
    )
}

fn parse_prompt_options(prompt: &str) -> Vec<String> {
    prompt
        .lines()
        .filter_map(|line| line.trim_start().strip_prefix("- "))
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn blocker_reason_details(reason: &BlockedReason) -> (String, String, Vec<String>) {
    match reason {
        BlockedReason::AwaitingHumanInput { prompt } => (
            "awaiting_human_input".to_owned(),
            prompt.as_str().to_owned(),
            parse_prompt_options(prompt.as_str()),
        ),
        BlockedReason::ExternalDependency { name, detail } => (
            "external_dependency".to_owned(),
            format!("{}: {}", name.as_str(), detail),
            Vec::new(),
        ),
        BlockedReason::InvestigationLoopCap { loop_index } => (
            "investigation_loop_cap".to_owned(),
            format!("loop_index={loop_index}"),
            Vec::new(),
        ),
        BlockedReason::SpecAmbiguity { detail } => (
            "spec_ambiguity".to_owned(),
            detail.as_str().to_owned(),
            Vec::new(),
        ),
        BlockedReason::Other { detail } => {
            ("other".to_owned(), detail.as_str().to_owned(), Vec::new())
        }
    }
}

#[derive(Debug, Clone)]
struct SpecOutcomeState {
    blockers_active: bool,
    walk_spec_completed: bool,
    pending_investigation: Option<PendingInvestigation>,
    post_investigation_recovery: Option<PendingInvestigation>,
    last_blocker_phase: Option<String>,
    last_blocker_summary: Option<NonEmptyString>,
    last_blocker_reason_kind: Option<String>,
    last_blocker_reason_detail: Option<String>,
    last_blocker_options: Vec<String>,
}

fn empty_spec_outcome_state() -> SpecOutcomeState {
    SpecOutcomeState {
        blockers_active: false,
        walk_spec_completed: false,
        pending_investigation: None,
        post_investigation_recovery: None,
        last_blocker_phase: None,
        last_blocker_summary: None,
        last_blocker_reason_kind: None,
        last_blocker_reason_detail: None,
        last_blocker_options: Vec::new(),
    }
}

#[derive(Debug, Clone)]
struct RunLoopPlan {
    next_task_id: Option<TaskId>,
    next_step: Option<SpecStatusNextStep>,
    pending_required_guards: Vec<String>,
    next_step_reason: Option<String>,
    investigate_source_phase: Option<String>,
    investigate_source_outcome: Option<String>,
    investigate_source_summary: Option<NonEmptyString>,
    investigate_source_task_id: Option<TaskId>,
}

fn plan_run_loop(
    next_action: SpecStatusNextAction,
    next_task_id: Option<TaskId>,
    next_task: Option<&Task>,
    outcomes: &SpecOutcomeState,
    required_guards: &[RequiredGuard],
) -> RunLoopPlan {
    let mut plan = RunLoopPlan {
        next_task_id,
        next_step: None,
        pending_required_guards: Vec::new(),
        next_step_reason: None,
        investigate_source_phase: None,
        investigate_source_outcome: None,
        investigate_source_summary: None,
        investigate_source_task_id: None,
    };

    if !matches!(next_action, SpecStatusNextAction::RunLoop) {
        plan.next_task_id = None;
        return plan;
    }

    if let Some(pending) = outcomes.pending_investigation.clone() {
        let task_step = pending.task_scoped && next_task_id.is_some();
        if task_step {
            plan.investigate_source_task_id = next_task_id;
        }
        plan.next_step = Some(if task_step {
            SpecStatusNextStep::TaskInvestigate
        } else {
            SpecStatusNextStep::SpecInvestigate
        });
        plan.next_step_reason = Some(format!(
            "latest {} outcome in {} requires investigate",
            pending.source_outcome, pending.source_phase
        ));
        plan.investigate_source_phase = Some(pending.source_phase);
        plan.investigate_source_outcome = Some(pending.source_outcome);
        plan.investigate_source_summary = Some(pending.source_summary);
        return plan;
    }

    if let Some(recovery) = outcomes.post_investigation_recovery.clone() {
        if recovery.task_scoped && next_task_id.is_some() {
            plan.next_step = Some(SpecStatusNextStep::TaskDoTask);
            plan.next_step_reason = Some(format!(
                "investigate completed for latest {} outcome in {}; rerun do-task",
                recovery.source_outcome, recovery.source_phase
            ));
            plan.investigate_source_task_id = next_task_id;
        } else {
            plan.next_step = Some(SpecStatusNextStep::SpecPipeline);
            plan.next_step_reason = Some(format!(
                "investigate completed for latest {} outcome in {}; rerun spec pipeline",
                recovery.source_outcome, recovery.source_phase
            ));
        }
        plan.investigate_source_phase = Some(recovery.source_phase);
        plan.investigate_source_outcome = Some(recovery.source_outcome);
        plan.investigate_source_summary = Some(recovery.source_summary);
        return plan;
    }

    let (step, pending, reason) = next_step_from_task(next_task, required_guards);
    plan.next_step = Some(step);
    plan.pending_required_guards = pending;
    plan.next_step_reason = reason;
    plan
}

impl MethodologyService {
    async fn spec_has_any_event(&self, spec_id: tanren_domain::SpecId) -> MethodologyResult<bool> {
        let filter = EventFilter {
            spec_id: Some(spec_id),
            event_type: Some("methodology".into()),
            limit: 1,
            ..EventFilter::new()
        };
        let page = EventStore::query_events(self.store(), &filter).await?;
        Ok(page
            .events
            .into_iter()
            .any(|env| matches!(env.payload, DomainEvent::Methodology { .. })))
    }

    async fn spec_outcome_state(
        &self,
        spec_id: tanren_domain::SpecId,
    ) -> MethodologyResult<SpecOutcomeState> {
        let mut state = empty_spec_outcome_state();
        let events = tanren_store::methodology::projections::load_methodology_events_for_kind(
            self.store(),
            spec_id,
            1_000,
            EntityKind::Spec,
        )
        .await?;
        for event in events {
            let MethodologyEvent::PhaseOutcomeReported(outcome) = event else {
                continue;
            };
            let is_walk_spec = outcome.phase.is_known(KnownPhase::WalkSpec);
            match outcome.outcome {
                PhaseOutcome::Blocked { reason, summary } => {
                    let (kind, detail, options) = blocker_reason_details(&reason);
                    state.last_blocker_phase = Some(outcome.phase.as_str().to_owned());
                    state.last_blocker_summary = Some(summary.clone());
                    state.last_blocker_reason_kind = Some(kind);
                    state.last_blocker_reason_detail = Some(detail);
                    state.last_blocker_options = options;
                    if outcome.phase.is_known(KnownPhase::Investigate) {
                        state.blockers_active = true;
                        state.pending_investigation = None;
                        state.post_investigation_recovery = None;
                    } else {
                        state.blockers_active = false;
                        state.pending_investigation = Some(PendingInvestigation {
                            source_phase: outcome.phase.as_str().to_owned(),
                            source_outcome: "blocked".to_owned(),
                            source_summary: summary,
                            task_scoped: phase_is_task_scoped(&outcome.phase),
                        });
                        state.post_investigation_recovery = None;
                    }
                    if is_walk_spec {
                        state.walk_spec_completed = false;
                    }
                }
                PhaseOutcome::Complete { .. } => {
                    state.blockers_active = false;
                    if outcome.phase.is_known(KnownPhase::Investigate) {
                        state.post_investigation_recovery = state.pending_investigation.take();
                    } else {
                        state.pending_investigation = None;
                        state.post_investigation_recovery = None;
                    }
                    if is_walk_spec {
                        state.walk_spec_completed = true;
                    }
                }
                PhaseOutcome::Error { summary, .. } => {
                    state.blockers_active = false;
                    state.pending_investigation = Some(PendingInvestigation {
                        source_phase: outcome.phase.as_str().to_owned(),
                        source_outcome: "error".to_owned(),
                        source_summary: summary,
                        task_scoped: phase_is_task_scoped(&outcome.phase),
                    });
                    state.post_investigation_recovery = None;
                    if is_walk_spec {
                        state.walk_spec_completed = false;
                    }
                }
            }
        }
        Ok(state)
    }

    /// `spec_status` — read-only orchestration status for one spec.
    ///
    /// # Errors
    /// See [`super::errors::MethodologyError`].
    pub async fn spec_status(
        &self,
        scope: &CapabilityScope,
        phase: &PhaseId,
        params: SpecStatusParams,
    ) -> MethodologyResult<SpecStatusResponse> {
        enforce(scope, ToolCapability::TaskRead, phase)?;
        let spec_id = params.spec_id;
        let has_any_event = self.spec_has_any_event(spec_id).await?;

        let tasks = self
            .list_tasks(
                scope,
                phase,
                ListTasksParams {
                    schema_version: SchemaVersion::current(),
                    spec_id: Some(spec_id),
                },
            )
            .await?
            .tasks;
        let counts = fold_task_counts(&tasks);
        let next_task = next_open_task(&tasks);
        let mut next_task_id = next_task.map(|task| task.id);

        let mut outcomes = if has_any_event {
            self.spec_outcome_state(spec_id).await?
        } else {
            empty_spec_outcome_state()
        };
        if next_task_id.is_some() {
            outcomes.walk_spec_completed = false;
        }

        let ready_for_walk_spec = has_any_event
            && counts.total > 0
            && next_task_id.is_none()
            && !outcomes.blockers_active
            && outcomes.pending_investigation.is_none()
            && outcomes.post_investigation_recovery.is_none()
            && !outcomes.walk_spec_completed;
        let next_action = if !has_any_event {
            SpecStatusNextAction::ShapeSpecRequired
        } else if outcomes.blockers_active {
            SpecStatusNextAction::ResolveBlockersRequired
        } else if outcomes.walk_spec_completed {
            SpecStatusNextAction::Complete
        } else if ready_for_walk_spec {
            SpecStatusNextAction::WalkSpecRequired
        } else {
            SpecStatusNextAction::RunLoop
        };
        let plan = plan_run_loop(
            next_action,
            next_task_id,
            next_task,
            &outcomes,
            self.required_guards(),
        );
        next_task_id = plan.next_task_id;

        Ok(SpecStatusResponse {
            schema_version: SchemaVersion::current(),
            spec_id,
            spec_exists: has_any_event,
            blockers_active: outcomes.blockers_active,
            ready_for_walk_spec,
            next_action,
            next_task_id,
            next_step: plan.next_step,
            pending_required_guards: plan.pending_required_guards,
            next_step_reason: plan.next_step_reason,
            investigate_source_phase: plan.investigate_source_phase,
            investigate_source_outcome: plan.investigate_source_outcome,
            investigate_source_summary: plan.investigate_source_summary,
            investigate_source_task_id: plan.investigate_source_task_id,
            last_blocker_phase: outcomes.last_blocker_phase,
            last_blocker_summary: outcomes.last_blocker_summary,
            last_blocker_reason_kind: outcomes.last_blocker_reason_kind,
            last_blocker_reason_detail: outcomes.last_blocker_reason_detail,
            last_blocker_options: outcomes.last_blocker_options,
            required_guards: self.required_guards().to_vec(),
            total_tasks: counts.total,
            completed_tasks: counts.complete,
            abandoned_tasks: counts.abandoned,
            implemented_tasks: counts.implemented,
            in_progress_tasks: counts.in_progress,
            pending_tasks: counts.pending,
        })
    }
}
