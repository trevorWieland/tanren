use chrono::Utc;
use serde_json::json;
use tanren_domain::methodology::event_tool::canonical_tool_for_event;
use tanren_domain::methodology::events::{
    MethodologyEvent, TaskCreated, TaskGateChecked, TaskGuardsReset, TaskImplemented, TaskStarted,
};
use tanren_domain::methodology::task::{RequiredGuard, Task, TaskOrigin, TaskStatus};
use tanren_domain::{NonEmptyString, SpecId, TaskId};
use tanren_store::Store;
use tanren_store::methodology::ingest_phase_events;

fn temp_path(name: &str) -> std::path::PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!(
        "tanren-store-replay-{name}-{}.jsonl",
        uuid::Uuid::now_v7()
    ));
    path
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

fn line_json(
    spec_id: SpecId,
    event_id: uuid::Uuid,
    event: &MethodologyEvent,
    tool: &str,
) -> String {
    serde_json::to_string(&json!({
        "schema_version": "1.0.0",
        "event_id": event_id,
        "spec_id": spec_id,
        "phase": "do-task",
        "agent_session_id": "session-1",
        "timestamp": Utc::now(),
        "origin_kind": "tool_primary",
        "tool": tool,
        "payload": event,
    }))
    .expect("serialize")
}

#[tokio::test]
async fn replay_applies_task_guard_reset_event_to_projection() {
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
        MethodologyEvent::TaskGuardsReset(TaskGuardsReset {
            task_id,
            spec_id,
            reason: tanren_domain::NonEmptyString::try_new("retry after batch failure")
                .expect("reason"),
            idempotency_key: None,
        }),
    ];
    let path = temp_path("replay-task-guard-reset");
    let mut content = String::new();
    for event in &events {
        content.push_str(&line_json(
            spec_id,
            uuid::Uuid::now_v7(),
            event,
            canonical_tool_for_event(event),
        ));
        content.push('\n');
    }
    std::fs::write(&path, content).expect("write jsonl");

    ingest_phase_events(&store, &path, &[RequiredGuard::GateChecked])
        .await
        .expect("ingest");

    let projection = store
        .load_methodology_task_status_projection(spec_id, task_id)
        .await
        .expect("load projection")
        .expect("projection row");
    assert!(
        matches!(
            projection.status,
            tanren_domain::methodology::task::TaskStatus::Implemented { .. }
        ),
        "task should remain implemented after guard reset"
    );
    if let tanren_domain::methodology::task::TaskStatus::Implemented { guards } = projection.status
    {
        assert!(!guards.gate_checked);
        assert!(!guards.audited);
        assert!(!guards.adherent);
        assert!(guards.extra.is_empty());
    }
}
