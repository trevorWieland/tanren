//! Pure planning helpers for `spec_status`.

use std::collections::BTreeMap;

use tanren_contract::methodology::{PhaseOutcomeTag, SpecCheckKind, SpecStatusTransition};
use tanren_domain::NonEmptyString;
use tanren_domain::TaskId;
use tanren_domain::methodology::events::MethodologyEvent;
use tanren_domain::methodology::phase_id::{KnownPhase, PhaseId};
use tanren_domain::methodology::phase_outcome::BlockedReason;
use tanren_domain::methodology::task::{RequiredGuard, Task, TaskStatus};

/// Canonical state-machine edge table used by conformance tests and
/// Mermaid drift checks.
pub(super) const PHASE0_TRANSITION_TABLE: &[(SpecStatusTransition, SpecStatusTransition)] = &[
    (
        SpecStatusTransition::ShapeSpecRequired,
        SpecStatusTransition::TaskDo,
    ),
    (
        SpecStatusTransition::TaskDo,
        SpecStatusTransition::TaskCheckBatch,
    ),
    (
        SpecStatusTransition::TaskCheckBatch,
        SpecStatusTransition::TaskDo,
    ),
    (
        SpecStatusTransition::TaskCheckBatch,
        SpecStatusTransition::TaskInvestigate,
    ),
    (
        SpecStatusTransition::TaskInvestigate,
        SpecStatusTransition::TaskDo,
    ),
    (
        SpecStatusTransition::TaskInvestigate,
        SpecStatusTransition::ResolveBlockersRequired,
    ),
    (
        SpecStatusTransition::TaskDo,
        SpecStatusTransition::SpecCheckBatch,
    ),
    (
        SpecStatusTransition::SpecCheckBatch,
        SpecStatusTransition::SpecInvestigate,
    ),
    (
        SpecStatusTransition::SpecInvestigate,
        SpecStatusTransition::TaskDo,
    ),
    (
        SpecStatusTransition::SpecInvestigate,
        SpecStatusTransition::SpecCheckBatch,
    ),
    (
        SpecStatusTransition::SpecInvestigate,
        SpecStatusTransition::ResolveBlockersRequired,
    ),
    (
        SpecStatusTransition::SpecCheckBatch,
        SpecStatusTransition::WalkSpecRequired,
    ),
    (
        SpecStatusTransition::ResolveBlockersRequired,
        SpecStatusTransition::TaskDo,
    ),
    (
        SpecStatusTransition::WalkSpecRequired,
        SpecStatusTransition::Complete,
    ),
];

#[derive(Debug, Clone, Copy, Default)]
pub(super) struct TaskStatusCounts {
    pub total: u64,
    pub pending: u64,
    pub in_progress: u64,
    pub implemented: u64,
    pub complete: u64,
    pub abandoned: u64,
}

pub(super) fn fold_task_counts(tasks: &[Task]) -> TaskStatusCounts {
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

pub(super) fn next_open_task(tasks: &[Task]) -> Option<&Task> {
    tasks.iter().find(|task| !task.status.is_terminal())
}

pub(super) fn pending_task_checks(
    task: &Task,
    required_guards: &[RequiredGuard],
) -> Vec<RequiredGuard> {
    let TaskStatus::Implemented { guards } = &task.status else {
        return Vec::new();
    };
    required_guards
        .iter()
        .filter(|guard| !guards.get(guard))
        .cloned()
        .collect()
}

pub(super) fn phase_is_task_scoped(phase: &PhaseId) -> bool {
    matches!(
        phase.known(),
        Some(KnownPhase::DoTask | KnownPhase::AuditTask | KnownPhase::AdhereTask)
    )
}

pub(super) fn phase_to_spec_check(phase: &PhaseId) -> Option<SpecCheckKind> {
    match phase.known() {
        Some(KnownPhase::SpecGate) => Some(SpecCheckKind::SpecGate),
        Some(KnownPhase::RunDemo) => Some(SpecCheckKind::RunDemo),
        Some(KnownPhase::AuditSpec) => Some(SpecCheckKind::AuditSpec),
        Some(KnownPhase::AdhereSpec) => Some(SpecCheckKind::AdhereSpec),
        _ => None,
    }
}

pub(super) fn is_task_mutation_event(event: &MethodologyEvent) -> bool {
    matches!(
        event,
        MethodologyEvent::TaskCreated(_)
            | MethodologyEvent::TaskStarted(_)
            | MethodologyEvent::TaskImplemented(_)
            | MethodologyEvent::TaskGateChecked(_)
            | MethodologyEvent::TaskAudited(_)
            | MethodologyEvent::TaskAdherent(_)
            | MethodologyEvent::TaskXChecked(_)
            | MethodologyEvent::TaskGuardsReset(_)
            | MethodologyEvent::TaskCompleted(_)
            | MethodologyEvent::TaskAbandoned(_)
            | MethodologyEvent::TaskRevised(_)
    )
}

#[derive(Debug, Clone)]
pub(super) struct PendingInvestigation {
    pub source_phase: PhaseId,
    pub source_outcome: PhaseOutcomeTag,
    pub source_summary: NonEmptyString,
    pub task_scoped: bool,
    pub source_task_id: Option<TaskId>,
}

#[derive(Debug, Clone)]
pub(super) struct SpecOutcomeState {
    pub blockers_active: bool,
    pub walk_spec_completed: bool,
    pub pending_investigation: Option<PendingInvestigation>,
    pub post_investigation_recovery: Option<PendingInvestigation>,
    pub last_blocker_phase: Option<PhaseId>,
    pub last_blocker_summary: Option<NonEmptyString>,
    pub last_blocker_reason: Option<BlockedReason>,
    pub latest_spec_check_complete: BTreeMap<SpecCheckKind, u64>,
    pub last_task_mutation_seq: Option<u64>,
}

pub(super) fn empty_spec_outcome_state() -> SpecOutcomeState {
    SpecOutcomeState {
        blockers_active: false,
        walk_spec_completed: false,
        pending_investigation: None,
        post_investigation_recovery: None,
        last_blocker_phase: None,
        last_blocker_summary: None,
        last_blocker_reason: None,
        latest_spec_check_complete: BTreeMap::new(),
        last_task_mutation_seq: None,
    }
}

fn all_spec_checks() -> [SpecCheckKind; 4] {
    [
        SpecCheckKind::SpecGate,
        SpecCheckKind::RunDemo,
        SpecCheckKind::AuditSpec,
        SpecCheckKind::AdhereSpec,
    ]
}

pub(super) fn pending_spec_checks(outcomes: &SpecOutcomeState) -> Vec<SpecCheckKind> {
    all_spec_checks()
        .into_iter()
        .filter(
            |check| match outcomes.latest_spec_check_complete.get(check) {
                None => true,
                Some(check_ts) => outcomes
                    .last_task_mutation_seq
                    .is_some_and(|task_seq| *check_ts <= task_seq),
            },
        )
        .collect()
}

#[derive(Debug, Clone)]
pub(super) struct TransitionPlan {
    pub transition: SpecStatusTransition,
    pub next_task_id: Option<TaskId>,
    pub pending_task_checks: Vec<RequiredGuard>,
    pub pending_spec_checks: Vec<SpecCheckKind>,
    pub transition_reason: Option<String>,
    pub investigate_source_phase: Option<PhaseId>,
    pub investigate_source_outcome: Option<PhaseOutcomeTag>,
    pub investigate_source_summary: Option<NonEmptyString>,
    pub investigate_source_task_id: Option<TaskId>,
}

fn empty_plan(next_task_id: Option<TaskId>) -> TransitionPlan {
    TransitionPlan {
        transition: SpecStatusTransition::ShapeSpecRequired,
        next_task_id,
        pending_task_checks: Vec::new(),
        pending_spec_checks: Vec::new(),
        transition_reason: None,
        investigate_source_phase: None,
        investigate_source_outcome: None,
        investigate_source_summary: None,
        investigate_source_task_id: None,
    }
}

fn outcome_tag_label(outcome: PhaseOutcomeTag) -> &'static str {
    match outcome {
        PhaseOutcomeTag::Complete => "complete",
        PhaseOutcomeTag::Blocked => "blocked",
        PhaseOutcomeTag::Error => "error",
    }
}

fn plan_for_pending_investigation(
    mut plan: TransitionPlan,
    pending: PendingInvestigation,
    next_task_id: Option<TaskId>,
) -> TransitionPlan {
    let task_id = pending.source_task_id.or(next_task_id);
    if pending.task_scoped && task_id.is_some() {
        plan.transition = SpecStatusTransition::TaskInvestigate;
        plan.next_task_id = task_id;
        plan.investigate_source_task_id = task_id;
    } else {
        plan.transition = SpecStatusTransition::SpecInvestigate;
        plan.next_task_id = None;
    }
    plan.transition_reason = Some(format!(
        "latest {} outcome in {} requires investigate",
        outcome_tag_label(pending.source_outcome),
        pending.source_phase.as_str()
    ));
    plan.investigate_source_phase = Some(pending.source_phase);
    plan.investigate_source_outcome = Some(pending.source_outcome);
    plan.investigate_source_summary = Some(pending.source_summary);
    plan
}

fn plan_for_recovery(
    mut plan: TransitionPlan,
    recovery: PendingInvestigation,
    outcomes: &SpecOutcomeState,
    next_task_id: Option<TaskId>,
) -> TransitionPlan {
    let task_id = recovery.source_task_id.or(next_task_id);
    if recovery.task_scoped && task_id.is_some() {
        plan.transition = SpecStatusTransition::TaskDo;
        plan.next_task_id = task_id;
        plan.investigate_source_task_id = task_id;
        plan.transition_reason = Some(format!(
            "investigate completed for latest {} outcome in {}; rerun do-task",
            outcome_tag_label(recovery.source_outcome),
            recovery.source_phase.as_str()
        ));
    } else if next_task_id.is_some() {
        plan.transition = SpecStatusTransition::TaskDo;
        plan.transition_reason =
            Some("investigate completed; resume task implementation loop".to_owned());
    } else {
        plan.transition = SpecStatusTransition::SpecCheckBatch;
        plan.next_task_id = None;
        plan.pending_spec_checks = pending_spec_checks(outcomes);
        plan.transition_reason = Some(format!(
            "investigate completed for latest {} outcome in {}; rerun spec checks",
            outcome_tag_label(recovery.source_outcome),
            recovery.source_phase.as_str()
        ));
    }
    plan.investigate_source_phase = Some(recovery.source_phase);
    plan.investigate_source_outcome = Some(recovery.source_outcome);
    plan.investigate_source_summary = Some(recovery.source_summary);
    plan
}

fn plan_for_task(
    mut plan: TransitionPlan,
    task: &Task,
    outcomes: &SpecOutcomeState,
    required_guards: &[RequiredGuard],
) -> TransitionPlan {
    plan.next_task_id = Some(task.id);
    match task.status {
        TaskStatus::Pending | TaskStatus::InProgress => {
            plan.transition = SpecStatusTransition::TaskDo;
            plan.transition_reason = Some(format!(
                "task {} is {}; run do-task",
                task.id,
                task.status.tag()
            ));
        }
        TaskStatus::Implemented { .. } => {
            let pending = pending_task_checks(task, required_guards);
            if pending.is_empty() {
                plan.transition = SpecStatusTransition::TaskDo;
                plan.transition_reason = Some(format!(
                    "task {} has all guards; rerun do-task until completion converges",
                    task.id
                ));
            } else {
                plan.transition = SpecStatusTransition::TaskCheckBatch;
                plan.pending_task_checks = pending;
                plan.transition_reason = Some(format!(
                    "task {} missing required checks; run task check batch",
                    task.id
                ));
            }
        }
        TaskStatus::Complete | TaskStatus::Abandoned { .. } => {
            plan.transition = SpecStatusTransition::SpecCheckBatch;
            plan.next_task_id = None;
            plan.pending_spec_checks = pending_spec_checks(outcomes);
            plan.transition_reason = Some("all tasks terminal; run spec check batch".to_owned());
        }
    }
    plan
}

fn plan_for_spec_checks(mut plan: TransitionPlan, outcomes: &SpecOutcomeState) -> TransitionPlan {
    let pending_spec = pending_spec_checks(outcomes);
    if pending_spec.is_empty() {
        plan.transition = SpecStatusTransition::WalkSpecRequired;
        plan.next_task_id = None;
        plan.transition_reason =
            Some("all spec checks green after latest task mutation; run walk-spec".to_owned());
    } else {
        plan.transition = SpecStatusTransition::SpecCheckBatch;
        plan.next_task_id = None;
        plan.pending_spec_checks = pending_spec;
        plan.transition_reason = Some("all tasks complete; run spec check batch".to_owned());
    }
    plan
}

pub(super) fn plan_transition(
    has_any_event: bool,
    next_task_id: Option<TaskId>,
    next_task: Option<&Task>,
    outcomes: &SpecOutcomeState,
    required_guards: &[RequiredGuard],
) -> TransitionPlan {
    let _ = PHASE0_TRANSITION_TABLE;
    let mut plan = empty_plan(next_task_id);

    if !has_any_event {
        return plan;
    }
    if outcomes.blockers_active {
        plan.transition = SpecStatusTransition::ResolveBlockersRequired;
        plan.next_task_id = None;
        plan.transition_reason = Some(
            "latest investigate outcome escalated; manual resolve-blockers required".to_owned(),
        );
        return plan;
    }
    if outcomes.walk_spec_completed {
        plan.transition = SpecStatusTransition::Complete;
        plan.next_task_id = None;
        plan.transition_reason = Some("walk-spec already completed".to_owned());
        return plan;
    }

    if let Some(pending) = outcomes.pending_investigation.clone() {
        return plan_for_pending_investigation(plan, pending, next_task_id);
    }

    if let Some(recovery) = outcomes.post_investigation_recovery.clone() {
        return plan_for_recovery(plan, recovery, outcomes, next_task_id);
    }

    if let Some(task) = next_task {
        return plan_for_task(plan, task, outcomes, required_guards);
    }

    plan_for_spec_checks(plan, outcomes)
}
