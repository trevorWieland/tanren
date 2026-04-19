use chrono::Utc;
use serde_json::json;
use tanren_domain::methodology::events::MethodologyEvent;
use tanren_domain::methodology::finding::{Finding, FindingSeverity, FindingSource};
use tanren_domain::methodology::phase_id::PhaseId;
use tanren_domain::methodology::task::{Task, TaskOrigin, TaskStatus};
use tanren_domain::{FindingId, NonEmptyString, SpecId, TaskId};
use tanren_store::Store;

pub(super) async fn fresh_store() -> Store {
    let store = Store::new("sqlite::memory:").await.expect("connect");
    store.run_migrations().await.expect("migrate");
    store
}

pub(super) fn seed_task(spec_id: SpecId, task_id: TaskId) -> Task {
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

pub(super) fn line_json(
    spec_id: SpecId,
    event_id: uuid::Uuid,
    event: &MethodologyEvent,
    tool: &str,
) -> String {
    line_json_with_attribution(spec_id, event_id, event, tool, None, None)
}

pub(super) fn line_json_with_attribution(
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

pub(super) fn temp_path(name: &str) -> std::path::PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!(
        "tanren-store-replay-{name}-{}.jsonl",
        uuid::Uuid::now_v7()
    ));
    path
}

pub(super) fn seed_finding(
    spec_id: SpecId,
    task_id: TaskId,
    id: FindingId,
    title: &str,
) -> Finding {
    Finding {
        id,
        spec_id,
        severity: FindingSeverity::FixNow,
        title: NonEmptyString::try_new(title).expect("title"),
        description: String::new(),
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
