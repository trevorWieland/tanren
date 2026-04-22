use std::collections::HashMap;

use chrono::{DateTime, Utc};
use tanren_domain::methodology::events::MethodologyEvent;
use tanren_domain::methodology::task::{
    RequiredGuard, TaskAbandonDisposition, TaskGuardFlags, TaskStatus,
};
use tanren_domain::{EventId, SpecId, TaskId};

use super::artifact_projection::{SpecState, TaskProjectionRow};
use super::artifact_projection_helpers::{
    apply_demo_patch, apply_spec_patch, task_evidence, update_guard,
};
use super::phase_events::PhaseEventLine;

#[derive(Debug)]
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
}

pub(super) fn fold_projection_lines(
    spec_id: SpecId,
    lines: &[PhaseEventLine],
    required_guards: &[RequiredGuard],
) -> FoldedProjectionState {
    let scoped_lines = lines
        .iter()
        .filter(|line| line.spec_id == spec_id)
        .collect::<Vec<_>>();
    let generated_at = scoped_lines.last().map_or_else(
        || DateTime::from_timestamp(0, 0).expect("unix epoch"),
        |line| line.timestamp,
    );
    let mut spec_state = SpecState::default();
    let mut task_rows: HashMap<TaskId, TaskProjectionRow> = HashMap::new();
    let mut demo_steps = Vec::new();
    let mut demo_results = Vec::new();
    let mut last_demo_mutation = None;
    for line in &scoped_lines {
        if spec_state.created_at.is_none() {
            spec_state.created_at = Some(line.timestamp);
        }
        apply_projection_line(
            line,
            required_guards,
            &mut spec_state,
            &mut task_rows,
            &mut demo_steps,
            &mut demo_results,
            &mut last_demo_mutation,
        );
    }
    let mut tasks = task_rows.into_values().collect::<Vec<_>>();
    tasks.sort_by(|a, b| {
        a.task
            .created_at
            .cmp(&b.task.created_at)
            .then(a.task.id.into_uuid().cmp(&b.task.id.into_uuid()))
    });
    FoldedProjectionState {
        generated_at,
        spec_state,
        tasks,
        demo_steps,
        demo_results,
        last_demo_mutation,
        first_event_at: scoped_lines.first().map(|line| line.timestamp),
        last_event_at: scoped_lines.last().map(|line| line.timestamp),
        latest_event_id: scoped_lines.last().map(|line| line.event_id),
        latest_phase: scoped_lines.last().map(|line| line.phase.clone()),
    }
}

fn apply_projection_line(
    line: &PhaseEventLine,
    required_guards: &[RequiredGuard],
    spec_state: &mut SpecState,
    task_rows: &mut HashMap<TaskId, TaskProjectionRow>,
    demo_steps: &mut Vec<tanren_domain::methodology::evidence::demo::DemoStep>,
    demo_results: &mut Vec<tanren_domain::methodology::evidence::demo::DemoResult>,
    last_demo_mutation: &mut Option<DateTime<Utc>>,
) {
    match &line.payload {
        MethodologyEvent::SpecDefined(e) => {
            spec_state.title = Some(e.spec.title.clone());
            spec_state
                .problem_statement
                .clone_from(&e.spec.problem_statement);
            spec_state.motivations.clone_from(&e.spec.motivations);
            spec_state.expectations.clone_from(&e.spec.expectations);
            spec_state
                .planned_behaviors
                .clone_from(&e.spec.planned_behaviors);
            spec_state
                .implementation_plan
                .clone_from(&e.spec.implementation_plan);
            spec_state
                .non_negotiables
                .clone_from(&e.spec.non_negotiables);
            spec_state
                .acceptance_criteria
                .clone_from(&e.spec.acceptance_criteria);
            spec_state.demo_environment = e.spec.demo_environment.clone();
            spec_state.dependencies = e.spec.dependencies.clone();
            spec_state.base_branch = Some(e.spec.base_branch.clone());
            spec_state.relevance_context = e.spec.relevance_context.clone();
            spec_state.created_at = Some(e.spec.created_at);
        }
        MethodologyEvent::SpecFrontmatterUpdated(e) => apply_spec_patch(spec_state, &e.patch),
        MethodologyEvent::DemoFrontmatterUpdated(e) => {
            *last_demo_mutation = Some(line.timestamp);
            apply_demo_patch(demo_steps, demo_results, line, &e.patch);
        }
        _ => apply_task_event(line, required_guards, task_rows),
    }
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
