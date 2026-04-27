//! Spec-status query surface used by Phase 0 orchestration.

use tanren_contract::methodology::{
    ListTasksParams, PhaseOutcomeTag, SchemaVersion, SpecCheckKind, SpecStatusParams,
    SpecStatusResponse, SpecStatusTransition,
};
use tanren_domain::events::DomainEvent;
use tanren_domain::methodology::capability::{CapabilityScope, ToolCapability};
use tanren_domain::methodology::events::MethodologyEvent;
use tanren_domain::methodology::finding::{FindingSource, FindingView};
use tanren_domain::methodology::phase_id::{KnownPhase, PhaseId};
use tanren_domain::methodology::phase_outcome::PhaseOutcome;
use tanren_domain::methodology::task::RequiredGuard;
use tanren_store::{EventFilter, EventStore};

use super::capabilities::enforce;
use super::errors::MethodologyResult;
use super::service::MethodologyService;
use super::service_spec_status_planner::{
    empty_spec_outcome_state, fold_task_counts, is_task_mutation_event, next_open_task,
    phase_is_task_scoped, phase_to_spec_check, plan_transition,
};

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
    ) -> MethodologyResult<super::service_spec_status_planner::SpecOutcomeState> {
        let mut state = empty_spec_outcome_state();
        let mut cursor = None;
        let mut seq: u64 = 0;
        loop {
            let filter = EventFilter {
                spec_id: Some(spec_id),
                event_type: Some("methodology".into()),
                limit: 1_000,
                cursor,
                ..EventFilter::new()
            };
            let page = EventStore::query_events(self.store(), &filter).await?;
            for envelope in page.events {
                let DomainEvent::Methodology { event } = envelope.payload else {
                    continue;
                };
                seq = seq.saturating_add(1);
                if is_task_mutation_event(&event) {
                    state.last_task_mutation_seq = Some(seq);
                }
                let MethodologyEvent::PhaseOutcomeReported(outcome) = event else {
                    continue;
                };
                let is_walk_spec = outcome.phase.is_known(KnownPhase::WalkSpec);
                if let Some(check) = phase_to_spec_check(&outcome.phase) {
                    match outcome.outcome {
                        PhaseOutcome::Complete { .. } => {
                            state.latest_spec_check_complete.insert(check, seq);
                        }
                        PhaseOutcome::Blocked { .. } | PhaseOutcome::Error { .. } => {
                            state.latest_spec_check_complete.remove(&check);
                        }
                    }
                }
                match outcome.outcome {
                    PhaseOutcome::Blocked { reason, summary } => {
                        state.last_blocker_phase = Some(outcome.phase.clone());
                        state.last_blocker_summary = Some(summary.clone());
                        state.last_blocker_reason = Some(reason.clone());
                        if outcome.phase.is_known(KnownPhase::Investigate) {
                            state.blockers_active = true;
                            state.pending_investigation = None;
                            state.post_investigation_recovery = None;
                        } else {
                            state.blockers_active = false;
                            state.pending_investigation =
                                Some(super::service_spec_status_planner::PendingInvestigation {
                                    source_phase: outcome.phase.clone(),
                                    source_outcome: PhaseOutcomeTag::Blocked,
                                    source_summary: summary,
                                    task_scoped: phase_is_task_scoped(&outcome.phase),
                                    source_task_id: outcome.task_id,
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
                        state.pending_investigation =
                            Some(super::service_spec_status_planner::PendingInvestigation {
                                source_phase: outcome.phase.clone(),
                                source_outcome: PhaseOutcomeTag::Error,
                                source_summary: summary,
                                task_scoped: phase_is_task_scoped(&outcome.phase),
                                source_task_id: outcome.task_id,
                            });
                        state.post_investigation_recovery = None;
                        if is_walk_spec {
                            state.walk_spec_completed = false;
                        }
                    }
                }
            }
            if !page.has_more {
                break;
            }
            cursor = page.next_cursor;
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

        let mut plan = plan_transition(
            has_any_event,
            next_task_id,
            next_task,
            &outcomes,
            self.required_guards(),
        );
        if matches!(plan.transition, SpecStatusTransition::WalkSpecRequired) {
            let open_blockers = self.open_blocking_findings(spec_id).await?;
            if !open_blockers.is_empty() {
                apply_open_blocker_plan(&mut plan, &open_blockers, self.required_guards());
            }
        }
        next_task_id = plan.next_task_id;
        let ready_for_walk_spec = matches!(plan.transition, SpecStatusTransition::WalkSpecRequired);
        let expose_blocker_details =
            outcomes.blockers_active || outcomes.pending_investigation.is_some();
        let (last_blocker_phase, last_blocker_summary, last_blocker_reason) =
            if expose_blocker_details {
                (
                    outcomes.last_blocker_phase,
                    outcomes.last_blocker_summary,
                    outcomes.last_blocker_reason,
                )
            } else {
                (None, None, None)
            };

        Ok(SpecStatusResponse {
            schema_version: SchemaVersion::current(),
            spec_id,
            spec_exists: has_any_event,
            blockers_active: outcomes.blockers_active,
            ready_for_walk_spec,
            next_transition: plan.transition,
            next_task_id,
            pending_task_checks: plan.pending_task_checks,
            pending_spec_checks: plan.pending_spec_checks,
            transition_reason: plan.transition_reason,
            investigate_source_phase: plan.investigate_source_phase,
            investigate_source_outcome: plan.investigate_source_outcome,
            investigate_source_summary: plan.investigate_source_summary,
            investigate_source_task_id: plan.investigate_source_task_id,
            last_blocker_phase,
            last_blocker_summary,
            last_blocker_reason,
            required_guards: self.required_guards().to_vec(),
            total_tasks: counts.total,
            completed_tasks: counts.complete,
            abandoned_tasks: counts.abandoned,
            implemented_tasks: counts.implemented,
            in_progress_tasks: counts.in_progress,
            pending_tasks: counts.pending,
        })
    }

    async fn open_blocking_findings(
        &self,
        spec_id: tanren_domain::SpecId,
    ) -> MethodologyResult<Vec<FindingView>> {
        let findings =
            tanren_store::methodology::finding_views_for_spec(self.store(), spec_id).await?;
        Ok(findings
            .into_iter()
            .filter(FindingView::is_open_blocking)
            .collect())
    }
}

fn apply_open_blocker_plan(
    plan: &mut super::service_spec_status_planner::TransitionPlan,
    findings: &[FindingView],
    required_guards: &[RequiredGuard],
) {
    if let Some(task_id) = findings.iter().find_map(|view| view.finding.attached_task) {
        let guards = task_guards_for_findings(findings, task_id, required_guards);
        plan.transition = if guards.is_empty() {
            SpecStatusTransition::TaskInvestigate
        } else {
            SpecStatusTransition::TaskCheckBatch
        };
        plan.next_task_id = Some(task_id);
        plan.pending_task_checks = guards;
        plan.pending_spec_checks.clear();
        plan.investigate_source_task_id = Some(task_id);
        plan.transition_reason = Some(format!(
            "open task-scoped blocking findings remain for task {task_id}; rerun task checks"
        ));
        return;
    }

    plan.transition = SpecStatusTransition::SpecCheckBatch;
    plan.next_task_id = None;
    plan.pending_task_checks.clear();
    plan.pending_spec_checks = spec_checks_for_findings(findings);
    plan.transition_reason =
        Some("open spec-scoped blocking findings remain; rerun resolving spec checks".to_owned());
}

fn task_guards_for_findings(
    findings: &[FindingView],
    task_id: tanren_domain::TaskId,
    required_guards: &[RequiredGuard],
) -> Vec<RequiredGuard> {
    let mut guards = Vec::new();
    for view in findings
        .iter()
        .filter(|view| view.finding.attached_task == Some(task_id))
    {
        let guard = match &view.finding.source {
            FindingSource::Audit { .. } => Some(RequiredGuard::Audited),
            FindingSource::Adherence { .. } => Some(RequiredGuard::Adherent),
            FindingSource::Demo { .. } => required_guards.iter().find_map(|guard| match guard {
                RequiredGuard::Extra(name) if name == "demo" => Some(guard.clone()),
                _ => None,
            }),
            FindingSource::Investigation { .. }
            | FindingSource::Triage
            | FindingSource::Feedback { .. } => None,
        };
        if let Some(guard) = guard
            && !guards.contains(&guard)
        {
            guards.push(guard);
        }
    }
    guards
}

fn spec_checks_for_findings(findings: &[FindingView]) -> Vec<SpecCheckKind> {
    let mut checks = Vec::new();
    for view in findings
        .iter()
        .filter(|view| view.finding.attached_task.is_none())
    {
        let check = match &view.finding.source {
            FindingSource::Adherence { .. } => SpecCheckKind::AdhereSpec,
            FindingSource::Demo { .. } => SpecCheckKind::RunDemo,
            FindingSource::Audit { .. }
            | FindingSource::Investigation { .. }
            | FindingSource::Triage
            | FindingSource::Feedback { .. } => SpecCheckKind::AuditSpec,
        };
        if !checks.contains(&check) {
            checks.push(check);
        }
    }
    if checks.is_empty() {
        checks.push(SpecCheckKind::AuditSpec);
        checks.push(SpecCheckKind::AdhereSpec);
    }
    checks
}
