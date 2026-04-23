use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tanren_domain::methodology::events::MethodologyEvent;
use tanren_domain::methodology::finding::Finding;
use tanren_domain::methodology::pillar::PillarScope;
use tanren_domain::methodology::rubric::{ComplianceStatus, NonNegotiableCompliance, RubricScore};
use tanren_domain::methodology::signpost::Signpost;
use tanren_domain::methodology::task::{
    RequiredGuard, TaskAbandonDisposition, TaskGuardFlags, TaskStatus,
};
use tanren_domain::{EventId, SpecId, TaskId};

use super::artifact_projection::{SpecState, TaskProjectionRow};
use super::artifact_projection_helpers::{
    apply_demo_patch, apply_spec_patch, task_evidence, update_guard,
};
use super::phase_events::PhaseEventLine;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct FoldedProjectionState {
    pub(super) generated_at: DateTime<Utc>,
    pub(super) spec_state: SpecState,
    pub(super) tasks: Vec<TaskProjectionRow>,
    pub(super) demo_steps: Vec<tanren_domain::methodology::evidence::demo::DemoStep>,
    pub(super) demo_results: Vec<tanren_domain::methodology::evidence::demo::DemoResult>,
    pub(super) last_demo_mutation: Option<DateTime<Utc>>,
    pub(super) first_event_at: Option<DateTime<Utc>>,
    pub(super) last_event_at: Option<DateTime<Utc>>,
    pub(super) latest_event_id: Option<EventId>,
    pub(super) latest_phase: Option<String>,
    pub(super) findings: Vec<Finding>,
    pub(super) rubric_scores: Vec<RubricScore>,
    pub(super) non_negotiables_compliance: Vec<NonNegotiableCompliance>,
    pub(super) audit_scope: PillarScope,
    pub(super) audit_scope_target_id: Option<String>,
    pub(super) signposts: Vec<Signpost>,
}

pub(super) fn empty_folded_projection_state() -> FoldedProjectionState {
    FoldedProjectionState {
        generated_at: DateTime::from_timestamp(0, 0).expect("unix epoch"),
        spec_state: SpecState::default(),
        tasks: Vec::new(),
        demo_steps: Vec::new(),
        demo_results: Vec::new(),
        last_demo_mutation: None,
        first_event_at: None,
        last_event_at: None,
        latest_event_id: None,
        latest_phase: None,
        findings: Vec::new(),
        rubric_scores: Vec::new(),
        non_negotiables_compliance: Vec::new(),
        audit_scope: PillarScope::Spec,
        audit_scope_target_id: None,
        signposts: Vec::new(),
    }
}

pub(super) fn fold_projection_lines(
    spec_id: SpecId,
    lines: &[PhaseEventLine],
    required_guards: &[RequiredGuard],
) -> FoldedProjectionState {
    fold_projection_lines_incremental(
        empty_folded_projection_state(),
        spec_id,
        lines,
        required_guards,
    )
}

pub(super) fn fold_projection_lines_incremental(
    mut state: FoldedProjectionState,
    spec_id: SpecId,
    lines: &[PhaseEventLine],
    required_guards: &[RequiredGuard],
) -> FoldedProjectionState {
    let mut task_rows: HashMap<TaskId, TaskProjectionRow> = state
        .tasks
        .drain(..)
        .map(|row| (row.task.id, row))
        .collect();
    let mut findings: HashMap<_, Finding> = state
        .findings
        .drain(..)
        .map(|finding| (finding.id, finding))
        .collect();
    let mut rubric_scores: HashMap<String, RubricScore> = state
        .rubric_scores
        .drain(..)
        .map(|score| (score.pillar.as_str().to_owned(), score))
        .collect();
    let mut non_negotiables: HashMap<String, NonNegotiableCompliance> = state
        .non_negotiables_compliance
        .drain(..)
        .map(|record| (record.name.as_str().to_owned(), record))
        .collect();
    let mut signposts: HashMap<_, Signpost> = state
        .signposts
        .drain(..)
        .map(|signpost| (signpost.id, signpost))
        .collect();

    let scoped_lines = lines.iter().filter(|line| line.spec_id == spec_id);
    for line in scoped_lines {
        if state.first_event_at.is_none() {
            state.first_event_at = Some(line.timestamp);
        }
        if state.spec_state.created_at.is_none() {
            state.spec_state.created_at = Some(line.timestamp);
        }
        let mut maps = ProjectionMaps {
            task_rows: &mut task_rows,
            findings: &mut findings,
            rubric_scores: &mut rubric_scores,
            non_negotiables: &mut non_negotiables,
            signposts: &mut signposts,
        };
        apply_projection_line(line, required_guards, &mut state, &mut maps);
        state.generated_at = line.timestamp;
        state.last_event_at = Some(line.timestamp);
        state.latest_event_id = Some(line.event_id);
        state.latest_phase = Some(line.phase.clone());
    }

    state.tasks = task_rows.into_values().collect::<Vec<_>>();
    state.tasks.sort_by(|a, b| {
        a.task
            .created_at
            .cmp(&b.task.created_at)
            .then(a.task.id.into_uuid().cmp(&b.task.id.into_uuid()))
    });

    state.findings = findings.into_values().collect::<Vec<_>>();
    state.findings.sort_by(|a, b| {
        a.created_at
            .cmp(&b.created_at)
            .then(a.id.to_string().cmp(&b.id.to_string()))
    });

    state.rubric_scores = rubric_scores.into_values().collect::<Vec<_>>();
    state
        .rubric_scores
        .sort_by(|a, b| a.pillar.as_str().cmp(b.pillar.as_str()));

    state.non_negotiables_compliance = non_negotiables.into_values().collect::<Vec<_>>();
    state.non_negotiables_compliance.sort_by(|a, b| {
        a.name
            .as_str()
            .cmp(b.name.as_str())
            .then_with(|| match (a.status, b.status) {
                (ComplianceStatus::Pass, ComplianceStatus::Fail) => std::cmp::Ordering::Less,
                (ComplianceStatus::Fail, ComplianceStatus::Pass) => std::cmp::Ordering::Greater,
                _ => std::cmp::Ordering::Equal,
            })
    });

    state.signposts = signposts.into_values().collect::<Vec<_>>();
    state.signposts.sort_by(|a, b| {
        a.created_at
            .cmp(&b.created_at)
            .then(a.id.to_string().cmp(&b.id.to_string()))
    });

    state
}

fn apply_projection_line(
    line: &PhaseEventLine,
    required_guards: &[RequiredGuard],
    state: &mut FoldedProjectionState,
    maps: &mut ProjectionMaps<'_>,
) {
    match &line.payload {
        MethodologyEvent::SpecDefined(e) => {
            state.spec_state.title = Some(e.spec.title.clone());
            state
                .spec_state
                .problem_statement
                .clone_from(&e.spec.problem_statement);
            state.spec_state.motivations.clone_from(&e.spec.motivations);
            state
                .spec_state
                .expectations
                .clone_from(&e.spec.expectations);
            state
                .spec_state
                .planned_behaviors
                .clone_from(&e.spec.planned_behaviors);
            state
                .spec_state
                .implementation_plan
                .clone_from(&e.spec.implementation_plan);
            state
                .spec_state
                .non_negotiables
                .clone_from(&e.spec.non_negotiables);
            state
                .spec_state
                .acceptance_criteria
                .clone_from(&e.spec.acceptance_criteria);
            state.spec_state.demo_environment = e.spec.demo_environment.clone();
            state.spec_state.dependencies = e.spec.dependencies.clone();
            state.spec_state.base_branch = Some(e.spec.base_branch.clone());
            state.spec_state.relevance_context = e.spec.relevance_context.clone();
            state.spec_state.created_at = Some(e.spec.created_at);
        }
        MethodologyEvent::SpecFrontmatterUpdated(e) => {
            apply_spec_patch(&mut state.spec_state, &e.patch);
        }
        MethodologyEvent::DemoFrontmatterUpdated(e) => {
            state.last_demo_mutation = Some(line.timestamp);
            apply_demo_patch(
                &mut state.demo_steps,
                &mut state.demo_results,
                line,
                &e.patch,
            );
        }
        MethodologyEvent::FindingAdded(e) => {
            maps.findings.insert(e.finding.id, (*e.finding).clone());
        }
        MethodologyEvent::AdherenceFindingAdded(e) => {
            maps.findings.insert(e.finding.id, (*e.finding).clone());
        }
        MethodologyEvent::RubricScoreRecorded(e) => {
            state.audit_scope = e.scope;
            state.audit_scope_target_id.clone_from(&e.scope_target_id);
            maps.rubric_scores
                .insert(e.score.pillar.as_str().to_owned(), e.score.clone());
        }
        MethodologyEvent::NonNegotiableComplianceRecorded(e) => {
            state.audit_scope = e.scope;
            maps.non_negotiables
                .insert(e.compliance.name.as_str().to_owned(), e.compliance.clone());
        }
        MethodologyEvent::SignpostAdded(e) => {
            maps.signposts.insert(e.signpost.id, (*e.signpost).clone());
        }
        MethodologyEvent::SignpostStatusUpdated(e) => {
            if let Some(signpost) = maps.signposts.get_mut(&e.signpost_id) {
                signpost.status = e.status;
                signpost.resolution.clone_from(&e.resolution);
                signpost.updated_at = line.timestamp;
            }
        }
        _ => apply_task_event(line, required_guards, maps.task_rows),
    }
}

struct ProjectionMaps<'a> {
    task_rows: &'a mut HashMap<TaskId, TaskProjectionRow>,
    findings: &'a mut HashMap<tanren_domain::FindingId, Finding>,
    rubric_scores: &'a mut HashMap<String, RubricScore>,
    non_negotiables: &'a mut HashMap<String, NonNegotiableCompliance>,
    signposts: &'a mut HashMap<tanren_domain::SignpostId, Signpost>,
}

fn apply_task_event(
    line: &PhaseEventLine,
    required_guards: &[RequiredGuard],
    task_rows: &mut HashMap<TaskId, TaskProjectionRow>,
) {
    match &line.payload {
        MethodologyEvent::TaskCreated(e) => handle_task_created(line, task_rows, e),
        MethodologyEvent::TaskRevised(e) => handle_task_revised(line, task_rows, e),
        MethodologyEvent::TaskStarted(e) => handle_task_started(line, task_rows, e.task_id),
        MethodologyEvent::TaskImplemented(e) => handle_task_implemented(line, task_rows, e.task_id),
        MethodologyEvent::TaskGateChecked(e) => handle_task_guard(
            line,
            task_rows,
            required_guards,
            e.task_id,
            &RequiredGuard::GateChecked,
        ),
        MethodologyEvent::TaskAudited(e) => handle_task_guard(
            line,
            task_rows,
            required_guards,
            e.task_id,
            &RequiredGuard::Audited,
        ),
        MethodologyEvent::TaskAdherent(e) => handle_task_guard(
            line,
            task_rows,
            required_guards,
            e.task_id,
            &RequiredGuard::Adherent,
        ),
        MethodologyEvent::TaskXChecked(e) => handle_task_guard(
            line,
            task_rows,
            required_guards,
            e.task_id,
            &RequiredGuard::Extra(e.guard_name.as_str().to_owned()),
        ),
        MethodologyEvent::TaskGuardsReset(e) => handle_task_guard_reset(line, task_rows, e.task_id),
        MethodologyEvent::TaskCompleted(e) => {
            handle_task_completed(line, task_rows, required_guards, e.task_id);
        }
        MethodologyEvent::TaskAbandoned(e) => handle_task_abandoned(line, task_rows, e),
        _ => {}
    }
}

fn handle_task_created(
    line: &PhaseEventLine,
    task_rows: &mut HashMap<TaskId, TaskProjectionRow>,
    event: &tanren_domain::methodology::events::TaskCreated,
) {
    let row = TaskProjectionRow {
        task: (*event.task).clone(),
        guards: TaskGuardFlags::default(),
        evidence: task_evidence(line, "task created"),
    };
    task_rows.insert(event.task.id, row);
}

fn handle_task_revised(
    line: &PhaseEventLine,
    task_rows: &mut HashMap<TaskId, TaskProjectionRow>,
    event: &tanren_domain::methodology::events::TaskRevised,
) {
    if let Some(row) = task_rows.get_mut(&event.task_id) {
        row.task.description.clone_from(&event.revised_description);
        row.task
            .acceptance_criteria
            .clone_from(&event.revised_acceptance);
        row.task.updated_at = line.timestamp;
    }
}

fn handle_task_started(
    line: &PhaseEventLine,
    task_rows: &mut HashMap<TaskId, TaskProjectionRow>,
    task_id: TaskId,
) {
    if let Some(row) = task_rows.get_mut(&task_id)
        && !matches!(
            row.task.status,
            TaskStatus::Complete | TaskStatus::Abandoned { .. }
        )
    {
        row.task.status = TaskStatus::InProgress;
        row.task.updated_at = line.timestamp;
        row.evidence = task_evidence(line, "task started");
    }
}

fn handle_task_implemented(
    line: &PhaseEventLine,
    task_rows: &mut HashMap<TaskId, TaskProjectionRow>,
    task_id: TaskId,
) {
    if let Some(row) = task_rows.get_mut(&task_id)
        && !matches!(
            row.task.status,
            TaskStatus::Complete | TaskStatus::Abandoned { .. }
        )
    {
        row.task.status = TaskStatus::Implemented {
            guards: row.guards.clone(),
        };
        row.task.updated_at = line.timestamp;
        row.evidence = task_evidence(line, "implementation recorded");
    }
}

fn handle_task_guard(
    line: &PhaseEventLine,
    task_rows: &mut HashMap<TaskId, TaskProjectionRow>,
    required_guards: &[RequiredGuard],
    task_id: TaskId,
    guard: &RequiredGuard,
) {
    update_guard(task_rows.get_mut(&task_id), required_guards, line, guard);
}

fn handle_task_completed(
    line: &PhaseEventLine,
    task_rows: &mut HashMap<TaskId, TaskProjectionRow>,
    required_guards: &[RequiredGuard],
    task_id: TaskId,
) {
    if let Some(row) = task_rows.get_mut(&task_id)
        && matches!(row.task.status, TaskStatus::Implemented { .. })
        && row.guards.satisfies(required_guards)
    {
        row.task.status = TaskStatus::Complete;
        row.task.updated_at = line.timestamp;
        row.evidence = task_evidence(line, "completion guards converged");
    }
}

fn handle_task_guard_reset(
    line: &PhaseEventLine,
    task_rows: &mut HashMap<TaskId, TaskProjectionRow>,
    task_id: TaskId,
) {
    if let Some(row) = task_rows.get_mut(&task_id)
        && matches!(row.task.status, TaskStatus::Implemented { .. })
    {
        row.guards = TaskGuardFlags::default();
        row.task.status = TaskStatus::Implemented {
            guards: row.guards.clone(),
        };
        row.task.updated_at = line.timestamp;
        row.evidence = task_evidence(line, "task guards reset for remediation");
    }
}

fn handle_task_abandoned(
    line: &PhaseEventLine,
    task_rows: &mut HashMap<TaskId, TaskProjectionRow>,
    event: &tanren_domain::methodology::events::TaskAbandoned,
) {
    if let Some(row) = task_rows.get_mut(&event.task_id)
        && !matches!(row.task.status, TaskStatus::Complete)
    {
        row.task.status = TaskStatus::Abandoned {
            disposition: event.disposition,
            replacements: event.replacements.clone(),
            explicit_user_discard_provenance: event.explicit_user_discard_provenance.clone(),
        };
        row.task.updated_at = line.timestamp;
        let rationale = match event.disposition {
            TaskAbandonDisposition::Replacement => "task abandoned via replacement",
            TaskAbandonDisposition::ExplicitUserDiscard => {
                "task abandoned via explicit user discard"
            }
        };
        row.evidence = task_evidence(line, rationale);
    }
}
