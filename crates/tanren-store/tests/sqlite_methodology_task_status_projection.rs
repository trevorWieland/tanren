use chrono::Utc;
use tanren_domain::events::{DomainEvent, EventEnvelope};
use tanren_domain::methodology::events::{
    MethodologyEvent, TaskCompleted, TaskCreated, TaskGateChecked, TaskImplemented, TaskStarted,
};
use tanren_domain::methodology::task::{Task, TaskGuardFlags, TaskOrigin, TaskStatus};
use tanren_domain::{EventId, NonEmptyString, SpecId, TaskId};
use tanren_store::Store;

fn envelope(event: MethodologyEvent) -> EventEnvelope {
    EventEnvelope::new(
        EventId::new(),
        Utc::now(),
        DomainEvent::Methodology { event },
    )
}

async fn fresh_store() -> Store {
    let store = Store::new("sqlite::memory:").await.expect("connect");
    store.run_migrations().await.expect("migrate");
    store
}

fn seed_task(spec_id: SpecId, task_id: TaskId) -> Task {
    Task {
        id: task_id,
        spec_id,
        title: NonEmptyString::try_new("t").expect("title"),
        description: String::new(),
        acceptance_criteria: vec![],
        origin: TaskOrigin::User,
        status: TaskStatus::Pending,
        depends_on: vec![],
        parent_task_id: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    }
}

#[tokio::test]
async fn append_updates_task_status_projection_to_complete() {
    let store = fresh_store().await;
    let spec_id = SpecId::new();
    let task_id = TaskId::new();

    let events = vec![
        MethodologyEvent::TaskCreated(TaskCreated {
            task: Box::new(seed_task(spec_id, task_id)),
            origin: TaskOrigin::User,
            idempotency_key: None,
        }),
        MethodologyEvent::TaskStarted(TaskStarted { task_id, spec_id }),
        MethodologyEvent::TaskImplemented(TaskImplemented {
            task_id,
            spec_id,
            evidence_refs: vec![],
        }),
        MethodologyEvent::TaskGateChecked(TaskGateChecked {
            task_id,
            spec_id,
            idempotency_key: None,
        }),
        MethodologyEvent::TaskCompleted(TaskCompleted { task_id, spec_id }),
    ];

    for event in events {
        store
            .append_methodology_event(&envelope(event))
            .await
            .expect("append methodology event");
    }

    let projection = store
        .load_methodology_task_status_projection(spec_id, task_id)
        .await
        .expect("load projection")
        .expect("projection exists");

    assert_eq!(projection.spec_id, spec_id);
    assert_eq!(projection.task_id, task_id);
    assert_eq!(projection.status, TaskStatus::Complete);
}

#[tokio::test]
async fn projection_retains_guard_flags_for_implemented_state() {
    let store = fresh_store().await;
    let spec_id = SpecId::new();
    let task_id = TaskId::new();

    let events = vec![
        MethodologyEvent::TaskCreated(TaskCreated {
            task: Box::new(seed_task(spec_id, task_id)),
            origin: TaskOrigin::User,
            idempotency_key: None,
        }),
        MethodologyEvent::TaskStarted(TaskStarted { task_id, spec_id }),
        MethodologyEvent::TaskImplemented(TaskImplemented {
            task_id,
            spec_id,
            evidence_refs: vec![],
        }),
        MethodologyEvent::TaskGateChecked(TaskGateChecked {
            task_id,
            spec_id,
            idempotency_key: None,
        }),
    ];

    for event in events {
        store
            .append_methodology_event(&envelope(event))
            .await
            .expect("append methodology event");
    }

    let projection = store
        .load_methodology_task_status_projection(spec_id, task_id)
        .await
        .expect("load projection")
        .expect("projection exists");

    assert_eq!(
        projection.status,
        TaskStatus::Implemented {
            guards: TaskGuardFlags {
                gate_checked: true,
                audited: false,
                adherent: false,
                extra: std::collections::BTreeMap::default(),
            },
        }
    );
}
