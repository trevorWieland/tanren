use chrono::Utc;
use tanren_domain::events::{DomainEvent, EventEnvelope};
use tanren_domain::methodology::events::{
    FindingAdded, MethodologyEvent, SignpostAdded, TaskCompleted, TaskCreated, TaskGateChecked,
    TaskImplemented, TaskStarted,
};
use tanren_domain::methodology::finding::{Finding, FindingSeverity, FindingSource};
use tanren_domain::methodology::phase_id::PhaseId;
use tanren_domain::methodology::signpost::{Signpost, SignpostStatus};
use tanren_domain::methodology::task::TaskOrigin;
use tanren_domain::methodology::task::{Task, TaskGuardFlags, TaskStatus};
use tanren_domain::{EventId, FindingId, NonEmptyString, SignpostId, SpecId, TaskId};
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

#[tokio::test]
async fn task_spec_projection_upserts_from_task_created() {
    let store = fresh_store().await;
    let spec_id = SpecId::new();
    let task_id = TaskId::new();

    store
        .append_methodology_event(&envelope(MethodologyEvent::TaskCreated(TaskCreated {
            task: Box::new(seed_task(spec_id, task_id)),
            origin: TaskOrigin::User,
            idempotency_key: None,
        })))
        .await
        .expect("append task created");

    let projected = store
        .load_methodology_task_spec_projection(task_id)
        .await
        .expect("load projection");
    assert_eq!(projected, Some(spec_id));
}

#[tokio::test]
async fn signpost_spec_projection_upserts_from_signpost_added() {
    let store = fresh_store().await;
    let spec_id = SpecId::new();
    let signpost_id = SignpostId::new();

    store
        .append_methodology_event(&envelope(MethodologyEvent::SignpostAdded(SignpostAdded {
            signpost: Box::new(Signpost {
                id: signpost_id,
                spec_id,
                task_id: None,
                status: SignpostStatus::Unresolved,
                problem: NonEmptyString::try_new("problem").expect("problem"),
                evidence: NonEmptyString::try_new("evidence").expect("evidence"),
                tried: vec![],
                solution: None,
                resolution: None,
                files_affected: vec![],
                created_at: Utc::now(),
                updated_at: Utc::now(),
            }),
        })))
        .await
        .expect("append signpost added");

    let projected = store
        .load_methodology_signpost_spec_projection(signpost_id)
        .await
        .expect("load projection");
    assert_eq!(projected, Some(spec_id));
}

#[tokio::test]
async fn task_finding_projection_upserts_from_finding_added() {
    let store = fresh_store().await;
    let spec_id = SpecId::new();
    let task_id = TaskId::new();
    let finding_id = FindingId::new();

    store
        .append_methodology_event(&envelope(MethodologyEvent::TaskCreated(TaskCreated {
            task: Box::new(seed_task(spec_id, task_id)),
            origin: TaskOrigin::User,
            idempotency_key: None,
        })))
        .await
        .expect("append task created");

    store
        .append_methodology_event(&envelope(MethodologyEvent::FindingAdded(FindingAdded {
            finding: Box::new(Finding {
                id: finding_id,
                spec_id,
                severity: FindingSeverity::FixNow,
                title: NonEmptyString::try_new("fix").expect("title"),
                description: "desc".into(),
                affected_files: vec!["src/lib.rs".into()],
                line_numbers: vec![1],
                source: FindingSource::Audit {
                    phase: PhaseId::try_new("audit-task").expect("phase"),
                    pillar: Some(NonEmptyString::try_new("security").expect("pillar")),
                },
                attached_task: Some(task_id),
                created_at: Utc::now(),
            }),
            idempotency_key: None,
        })))
        .await
        .expect("append finding");

    let finding_ids = store
        .load_methodology_finding_ids_for_task_projection(spec_id, task_id)
        .await
        .expect("load finding projection");
    assert_eq!(finding_ids, vec![finding_id]);
}
