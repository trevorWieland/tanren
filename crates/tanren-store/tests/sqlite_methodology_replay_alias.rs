use chrono::Utc;
use serde_json::json;
use tanren_domain::methodology::event_tool::canonical_tool_for_event;
use tanren_domain::methodology::events::{
    MethodologyEvent, TaskAdherent, TaskAudited, TaskCompleted, TaskCreated, TaskImplemented,
    TaskStarted,
};
use tanren_domain::methodology::task::{RequiredGuard, Task, TaskOrigin, TaskStatus};
use tanren_domain::{NonEmptyString, SpecId, TaskId};
use tanren_store::Store;
use tanren_store::methodology::ingest_phase_events;

#[tokio::test]
async fn replay_accepts_report_phase_outcome_alias_for_bridged_guard_completion() {
    let store = fresh_store().await;
    let spec_id = SpecId::new();
    let task_id = TaskId::new();
    let events = vec![
        (
            MethodologyEvent::TaskCreated(TaskCreated {
                task: Box::new(seed_task(spec_id, task_id)),
                origin: TaskOrigin::User,
                idempotency_key: None,
            }),
            canonical_tool_for_event(&MethodologyEvent::TaskCreated(TaskCreated {
                task: Box::new(seed_task(spec_id, task_id)),
                origin: TaskOrigin::User,
                idempotency_key: None,
            })),
            Some("tool_primary"),
            None,
        ),
        (
            MethodologyEvent::TaskStarted(TaskStarted { task_id, spec_id }),
            canonical_tool_for_event(&MethodologyEvent::TaskStarted(TaskStarted {
                task_id,
                spec_id,
            })),
            Some("tool_primary"),
            None,
        ),
        (
            MethodologyEvent::TaskImplemented(TaskImplemented {
                task_id,
                spec_id,
                evidence_refs: vec![],
            }),
            canonical_tool_for_event(&MethodologyEvent::TaskImplemented(TaskImplemented {
                task_id,
                spec_id,
                evidence_refs: vec![],
            })),
            Some("tool_primary"),
            None,
        ),
        (
            MethodologyEvent::TaskAudited(TaskAudited {
                task_id,
                spec_id,
                idempotency_key: None,
            }),
            "report_phase_outcome",
            Some("tool_derived"),
            Some("phase-outcome-bridge"),
        ),
        (
            MethodologyEvent::TaskAdherent(TaskAdherent {
                task_id,
                spec_id,
                idempotency_key: None,
            }),
            "report_phase_outcome",
            Some("tool_derived"),
            Some("phase-outcome-bridge"),
        ),
        (
            MethodologyEvent::TaskCompleted(TaskCompleted { task_id, spec_id }),
            "report_phase_outcome",
            Some("tool_derived"),
            Some("phase-outcome-bridge"),
        ),
    ];

    let path = temp_path("replay-phase-outcome-bridge-tool-alias");
    let mut content = String::new();
    for (event, tool, origin_kind, caused_by) in &events {
        content.push_str(&line_json_with_attribution(
            spec_id,
            uuid::Uuid::now_v7(),
            event,
            tool,
            *origin_kind,
            *caused_by,
        ));
        content.push('\n');
    }
    std::fs::write(&path, content).expect("write jsonl");

    let stats = ingest_phase_events(
        &store,
        &path,
        &[RequiredGuard::Audited, RequiredGuard::Adherent],
    )
    .await
    .expect("ingest bridged alias");
    assert_eq!(stats.events_appended, events.len());
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

fn line_json_with_attribution(
    spec_id: SpecId,
    event_id: uuid::Uuid,
    event: &MethodologyEvent,
    tool: &str,
    origin_kind: Option<&str>,
    caused_by_tool_call_id: Option<&str>,
) -> String {
    let mut obj = serde_json::Map::new();
    obj.insert("event_id".into(), json!(event_id));
    obj.insert("spec_id".into(), json!(spec_id));
    obj.insert("phase".into(), json!("do-task"));
    obj.insert("agent_session_id".into(), json!("session-1"));
    obj.insert("timestamp".into(), json!(Utc::now()));
    obj.insert("origin_kind".into(), json!("tool_primary"));
    obj.insert("tool".into(), json!(tool));
    obj.insert("payload".into(), json!(event));
    if let Some(origin_kind) = origin_kind {
        obj.insert("origin_kind".into(), json!(origin_kind));
    }
    if let Some(caused_by_tool_call_id) = caused_by_tool_call_id {
        obj.insert(
            "caused_by_tool_call_id".into(),
            json!(caused_by_tool_call_id),
        );
    }
    serde_json::to_string(&serde_json::Value::Object(obj)).expect("serialize")
}

fn temp_path(name: &str) -> std::path::PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!(
        "tanren-store-replay-{name}-{}.jsonl",
        uuid::Uuid::now_v7()
    ));
    path
}
