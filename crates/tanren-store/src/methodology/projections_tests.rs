use chrono::Utc;
use tanren_domain::NonEmptyString;
use tanren_domain::methodology::task::{Task, TaskOrigin, TaskStatus};

use super::*;

fn seed_task(spec: SpecId) -> Task {
    Task {
        id: TaskId::new(),
        spec_id: spec,
        title: NonEmptyString::try_new("t").expect("non-empty"),
        description: String::new(),
        acceptance_criteria: vec![],
        origin: TaskOrigin::ShapeSpec,
        status: TaskStatus::Pending,
        depends_on: vec![],
        parent_task_id: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    }
}

#[test]
fn fold_tasks_returns_pending_on_creation() {
    let spec = SpecId::new();
    let t = seed_task(spec);
    let events = vec![MethodologyEvent::TaskCreated(EvTaskCreated {
        task: Box::new(t.clone()),
        origin: TaskOrigin::ShapeSpec,
        idempotency_key: None,
    })];
    let required = [
        RequiredGuard::GateChecked,
        RequiredGuard::Audited,
        RequiredGuard::Adherent,
    ];
    let tasks = fold_tasks(&events, &required);
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].id, t.id);
    assert_eq!(tasks[0].status, TaskStatus::Pending);
}
