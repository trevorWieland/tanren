//! Spec-status query surface used by Phase 0 orchestration.

use tanren_contract::methodology::{
    ListTasksParams, SchemaVersion, SpecStatusNextAction, SpecStatusParams, SpecStatusResponse,
};
use tanren_domain::EntityKind;
use tanren_domain::TaskId;
use tanren_domain::events::DomainEvent;
use tanren_domain::methodology::capability::{CapabilityScope, ToolCapability};
use tanren_domain::methodology::events::MethodologyEvent;
use tanren_domain::methodology::phase_id::KnownPhase;
use tanren_domain::methodology::phase_id::PhaseId;
use tanren_domain::methodology::phase_outcome::PhaseOutcome;
use tanren_domain::methodology::task::{Task, TaskStatus};
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

fn next_open_task_id(tasks: &[Task]) -> Option<TaskId> {
    tasks
        .iter()
        .find(|task| !task.status.is_terminal())
        .map(|task| task.id)
}

#[derive(Debug, Clone)]
struct SpecOutcomeState {
    blockers_active: bool,
    walk_spec_completed: bool,
    last_blocker_phase: Option<String>,
    last_blocker_summary: Option<tanren_domain::NonEmptyString>,
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
        let mut state = SpecOutcomeState {
            blockers_active: false,
            walk_spec_completed: false,
            last_blocker_phase: None,
            last_blocker_summary: None,
        };
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
                PhaseOutcome::Blocked { summary, .. } => {
                    state.blockers_active = true;
                    state.last_blocker_phase = Some(outcome.phase.as_str().to_owned());
                    state.last_blocker_summary = Some(summary);
                    if is_walk_spec {
                        state.walk_spec_completed = false;
                    }
                }
                PhaseOutcome::Complete { .. } => {
                    state.blockers_active = false;
                    if is_walk_spec {
                        state.walk_spec_completed = true;
                    }
                }
                PhaseOutcome::Error { .. } => {
                    state.blockers_active = false;
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
        let mut next_task_id = next_open_task_id(&tasks);

        let mut outcomes = if has_any_event {
            self.spec_outcome_state(spec_id).await?
        } else {
            SpecOutcomeState {
                blockers_active: false,
                walk_spec_completed: false,
                last_blocker_phase: None,
                last_blocker_summary: None,
            }
        };
        if next_task_id.is_some() {
            outcomes.walk_spec_completed = false;
        }

        let ready_for_walk_spec = has_any_event
            && counts.total > 0
            && next_task_id.is_none()
            && !outcomes.blockers_active
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
        if !matches!(next_action, SpecStatusNextAction::RunLoop) {
            next_task_id = None;
        }

        Ok(SpecStatusResponse {
            schema_version: SchemaVersion::current(),
            spec_id,
            spec_exists: has_any_event,
            blockers_active: outcomes.blockers_active,
            ready_for_walk_spec,
            next_action,
            next_task_id,
            last_blocker_phase: outcomes.last_blocker_phase,
            last_blocker_summary: outcomes.last_blocker_summary,
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
