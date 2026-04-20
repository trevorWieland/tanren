use chrono::Utc;
use tanren_domain::events::{DomainEvent, EventEnvelope};
use tanren_domain::methodology::events::{FindingAdded, MethodologyEvent, TaskCreated};
use tanren_domain::methodology::finding::{Finding, FindingSeverity, FindingSource};
use tanren_domain::methodology::phase_id::PhaseId;
use tanren_domain::methodology::task::{Task, TaskOrigin, TaskStatus};
use tanren_domain::{EventId, FindingId, NonEmptyString, SpecId, TaskId};
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
        title: NonEmptyString::try_new("task").expect("title"),
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

fn seed_finding(spec_id: SpecId, task_id: TaskId, finding_id: FindingId, label: &str) -> Finding {
    Finding {
        id: finding_id,
        spec_id,
        severity: FindingSeverity::FixNow,
        title: NonEmptyString::try_new(format!("finding-{label}")).expect("title"),
        description: format!("desc-{label}"),
        affected_files: vec!["src/lib.rs".into()],
        line_numbers: vec![1],
        source: FindingSource::Audit {
            phase: PhaseId::try_new("audit-task").expect("phase"),
            pillar: Some(NonEmptyString::try_new("security").expect("pillar")),
        },
        attached_task: Some(task_id),
        created_at: Utc::now(),
    }
}

#[tokio::test]
async fn findings_for_task_uses_task_projection_lookup() {
    let store = fresh_store().await;
    let spec_id = SpecId::new();
    let task_a = TaskId::new();
    let task_b = TaskId::new();
    let id_a = FindingId::new();
    let id_b = FindingId::new();

    for task_id in [task_a, task_b] {
        store
            .append_methodology_event(&envelope(MethodologyEvent::TaskCreated(TaskCreated {
                task: Box::new(seed_task(spec_id, task_id)),
                origin: TaskOrigin::User,
                idempotency_key: None,
            })))
            .await
            .expect("append task");
    }

    for finding in [
        seed_finding(spec_id, task_a, id_a, "a"),
        seed_finding(spec_id, task_b, id_b, "b"),
    ] {
        store
            .append_methodology_event(&envelope(MethodologyEvent::FindingAdded(FindingAdded {
                finding: Box::new(finding),
                idempotency_key: None,
            })))
            .await
            .expect("append finding");
    }

    let task_findings =
        tanren_store::methodology::projections::findings_for_task(&store, spec_id, task_a)
            .await
            .expect("task findings");
    assert_eq!(task_findings.len(), 1);
    assert_eq!(task_findings[0].id, id_a);
}
