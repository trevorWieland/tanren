#![cfg(feature = "postgres-integration")]

#[path = "common/support_postgres.rs"]
mod support_postgres;

use chrono::Utc;
use serde_json::json;
use support_postgres::postgres_fixture;
use tanren_domain::methodology::event_tool::canonical_tool_for_event;
use tanren_domain::methodology::events::{
    MethodologyEvent, TaskAdherent, TaskAudited, TaskCompleted, TaskCreated, TaskGateChecked,
    TaskImplemented, TaskStarted,
};
use tanren_domain::methodology::task::{RequiredGuard, Task, TaskOrigin, TaskStatus};
use tanren_domain::{EntityKind, NonEmptyString, SpecId, TaskId};
use tanren_store::methodology::ingest_phase_events;
use tanren_store::{EventFilter, EventStore};

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

fn temp_path(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("tanren-store-{name}-{}", uuid::Uuid::now_v7()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    dir.join("phase-events.jsonl")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn replay_ingests_canonical_phase_event_lines_postgres() {
    let fixture = postgres_fixture().await;
    let _postgres_url = fixture.url.clone();
    let store = fixture.store;
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
        MethodologyEvent::TaskAudited(TaskAudited {
            task_id,
            spec_id,
            idempotency_key: None,
        }),
        MethodologyEvent::TaskAdherent(TaskAdherent {
            task_id,
            spec_id,
            idempotency_key: None,
        }),
        MethodologyEvent::TaskCompleted(TaskCompleted { task_id, spec_id }),
    ];

    let path = temp_path("replay-postgres-canonical");
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

    let stats = ingest_phase_events(
        &store,
        &path,
        &[
            RequiredGuard::GateChecked,
            RequiredGuard::Audited,
            RequiredGuard::Adherent,
        ],
    )
    .await
    .expect("ingest");

    assert_eq!(stats.events_appended, events.len());

    let queried = store
        .query_events(&EventFilter {
            entity_kind: Some(EntityKind::Task),
            event_type: Some("methodology".into()),
            spec_id: Some(spec_id),
            limit: 100,
            ..EventFilter::new()
        })
        .await
        .expect("query");
    assert_eq!(queried.events.len(), events.len());
}
