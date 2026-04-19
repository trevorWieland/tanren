use std::collections::HashMap;
use std::path::Path;

use tanren_domain::methodology::events::MethodologyEvent;
use tanren_domain::methodology::task::{RequiredGuard, TaskStatus, TaskTransitionKind};
use tanren_domain::{EntityRef, SpecId, TaskId};

use crate::Store;
use crate::methodology::projections;

use super::ReplayError;

const TASK_EVENT_PAGE_SIZE: u64 = 1_000;

#[derive(Debug, Clone)]
pub(super) struct TaskReplayState {
    pub has_created: bool,
    pub current: TaskStatus,
}

#[derive(Debug, Default)]
pub(super) struct TaskValidationState {
    pub by_task: HashMap<TaskId, TaskReplayState>,
}

pub(super) async fn validate_task_transition(
    store: &Store,
    event: &MethodologyEvent,
    spec_id: SpecId,
    line_no: usize,
    path: &Path,
    required_guards: &[RequiredGuard],
    task_state: &mut TaskValidationState,
) -> Result<(), ReplayError> {
    let Some((task_id, kind)) = task_transition_kind(event) else {
        return Ok(());
    };
    ensure_task_state_loaded(store, spec_id, task_id, required_guards, task_state).await?;
    let state = task_state
        .by_task
        .get_mut(&task_id)
        .expect("task state is loaded");

    if matches!(event, MethodologyEvent::TaskCreated(_)) {
        if state.has_created {
            return Err(ReplayError::DuplicateTaskCreate {
                path: path.to_path_buf(),
                line: line_no,
                task_id,
            });
        }
        state.has_created = true;
        state.current = TaskStatus::Pending;
        return Ok(());
    }
    if !state.has_created {
        return Err(ReplayError::MissingTaskCreate {
            path: path.to_path_buf(),
            line: line_no,
            task_id,
        });
    }
    if matches!(event, MethodologyEvent::TaskCompleted(_))
        && !matches!(
            state.current,
            TaskStatus::Implemented { ref guards } if guards.satisfies(required_guards)
        )
    {
        return Err(ReplayError::TaskCompletedMissingGuards {
            path: path.to_path_buf(),
            line: line_no,
            task_id,
        });
    }
    state
        .current
        .legal_next(kind)
        .map_err(|e| ReplayError::InvalidTaskTransition {
            path: path.to_path_buf(),
            line: line_no,
            task_id,
            from: e.from.to_owned(),
            attempted: e.attempted.to_owned(),
        })?;
    apply_task_transition(&mut state.current, event, required_guards);
    Ok(())
}

async fn ensure_task_state_loaded(
    store: &Store,
    spec_id: SpecId,
    task_id: TaskId,
    required_guards: &[RequiredGuard],
    task_state: &mut TaskValidationState,
) -> Result<(), ReplayError> {
    if task_state.by_task.contains_key(&task_id) {
        return Ok(());
    }
    let existing = projections::load_methodology_events_for_entity(
        store,
        EntityRef::Task(task_id),
        Some(spec_id),
        TASK_EVENT_PAGE_SIZE,
    )
    .await
    .map_err(|source| match source {
        projections::MethodologyEventFetchError::Store { source } => ReplayError::Store { source },
    })?;
    let has_created = existing
        .iter()
        .any(|event| matches!(event, MethodologyEvent::TaskCreated(e) if e.task.id == task_id));
    let current = tanren_domain::methodology::events::fold_task_status(
        task_id,
        required_guards,
        existing.iter(),
    )
    .unwrap_or(TaskStatus::Pending);
    task_state.by_task.insert(
        task_id,
        TaskReplayState {
            has_created,
            current,
        },
    );
    Ok(())
}

fn apply_task_transition(
    current: &mut TaskStatus,
    event: &MethodologyEvent,
    required: &[RequiredGuard],
) {
    match event {
        MethodologyEvent::TaskCreated(_) => {
            *current = TaskStatus::Pending;
        }
        MethodologyEvent::TaskStarted(_) => {
            if !matches!(current, TaskStatus::Complete | TaskStatus::Abandoned { .. }) {
                *current = TaskStatus::InProgress;
            }
        }
        MethodologyEvent::TaskImplemented(_) => {
            if !matches!(current, TaskStatus::Complete | TaskStatus::Abandoned { .. }) {
                let guards = match current {
                    TaskStatus::Implemented { guards } => guards.clone(),
                    _ => tanren_domain::methodology::task::TaskGuardFlags::default(),
                };
                *current = TaskStatus::Implemented { guards };
            }
        }
        MethodologyEvent::TaskGateChecked(_) => set_guard(current, &RequiredGuard::GateChecked),
        MethodologyEvent::TaskAudited(_) => set_guard(current, &RequiredGuard::Audited),
        MethodologyEvent::TaskAdherent(_) => set_guard(current, &RequiredGuard::Adherent),
        MethodologyEvent::TaskXChecked(e) => {
            set_guard(
                current,
                &RequiredGuard::Extra(e.guard_name.as_str().to_owned()),
            );
        }
        MethodologyEvent::TaskCompleted(_) => {
            if let TaskStatus::Implemented { guards } = current
                && guards.satisfies(required)
            {
                *current = TaskStatus::Complete;
            }
        }
        MethodologyEvent::TaskAbandoned(e) => {
            if !matches!(current, TaskStatus::Complete) {
                *current = TaskStatus::Abandoned {
                    disposition: e.disposition,
                    replacements: e.replacements.clone(),
                    explicit_user_discard_provenance: e.explicit_user_discard_provenance.clone(),
                };
            }
        }
        _ => {}
    }
}

fn set_guard(current: &mut TaskStatus, guard: &RequiredGuard) {
    if let TaskStatus::Implemented { guards } = current {
        guards.set(guard, true);
    }
}

fn task_transition_kind(event: &MethodologyEvent) -> Option<(TaskId, TaskTransitionKind)> {
    match event {
        MethodologyEvent::TaskCreated(e) => Some((e.task.id, TaskTransitionKind::Start)),
        MethodologyEvent::TaskStarted(e) => Some((e.task_id, TaskTransitionKind::Start)),
        MethodologyEvent::TaskImplemented(e) => Some((e.task_id, TaskTransitionKind::Implement)),
        MethodologyEvent::TaskGateChecked(e) => Some((e.task_id, TaskTransitionKind::Guard)),
        MethodologyEvent::TaskAudited(e) => Some((e.task_id, TaskTransitionKind::Guard)),
        MethodologyEvent::TaskAdherent(e) => Some((e.task_id, TaskTransitionKind::Guard)),
        MethodologyEvent::TaskXChecked(e) => Some((e.task_id, TaskTransitionKind::Guard)),
        MethodologyEvent::TaskCompleted(e) => Some((e.task_id, TaskTransitionKind::Complete)),
        MethodologyEvent::TaskRevised(e) => Some((e.task_id, TaskTransitionKind::Revise)),
        MethodologyEvent::TaskAbandoned(e) => Some((e.task_id, TaskTransitionKind::Abandon)),
        _ => None,
    }
}
