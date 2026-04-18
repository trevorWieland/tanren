use chrono::Utc;
use serde_json::json;
use tanren_domain::methodology::events::{
    FindingAdded, MethodologyEvent, TaskAdherent, TaskAudited, TaskCompleted, TaskCreated,
    TaskGateChecked, TaskImplemented, TaskStarted,
};
use tanren_domain::methodology::finding::{Finding, FindingSeverity, FindingSource};
use tanren_domain::methodology::task::{RequiredGuard, Task, TaskOrigin, TaskStatus};
use tanren_domain::{EntityKind, FindingId, NonEmptyString, SpecId, TaskId};
use tanren_store::methodology::{ReplayError, ingest_phase_events};
use tanren_store::{EventFilter, EventStore, Store};

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

fn tool_for(event: &MethodologyEvent) -> &'static str {
    match event {
        MethodologyEvent::TaskCreated(_) => "create_task",
        MethodologyEvent::TaskStarted(_) => "start_task",
        MethodologyEvent::TaskImplemented(_) => "complete_task",
        MethodologyEvent::TaskGateChecked(_)
        | MethodologyEvent::TaskAudited(_)
        | MethodologyEvent::TaskAdherent(_) => "<guard-phase>",
        MethodologyEvent::TaskCompleted(_) => "<orchestrator>",
        _ => "<unsupported>",
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

fn seed_finding(spec_id: SpecId, task_id: TaskId, id: FindingId, title: &str) -> Finding {
    Finding {
        id,
        spec_id,
        severity: FindingSeverity::FixNow,
        title: NonEmptyString::try_new(title).expect("title"),
        description: String::new(),
        affected_files: vec!["src/lib.rs".into()],
        line_numbers: vec![1],
        source: FindingSource::Audit {
            phase: NonEmptyString::try_new("audit-task").expect("phase"),
            pillar: Some(NonEmptyString::try_new("security").expect("pillar")),
        },
        attached_task: Some(task_id),
        created_at: Utc::now(),
    }
}

#[tokio::test]
async fn replay_ingests_canonical_phase_event_lines() {
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

    let path = temp_path("replay-canonical");
    let mut content = String::new();
    for event in &events {
        content.push_str(&line_json(
            spec_id,
            uuid::Uuid::now_v7(),
            event,
            tool_for(event),
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
    assert_eq!(stats.events_skipped_duplicate_event_id, 0);

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

#[tokio::test]
async fn replay_rejects_tool_mismatch() {
    let store = fresh_store().await;
    let spec_id = SpecId::new();
    let task_id = TaskId::new();
    let event = MethodologyEvent::TaskCreated(TaskCreated {
        task: Box::new(seed_task(spec_id, task_id)),
        origin: TaskOrigin::User,
        idempotency_key: None,
    });

    let path = temp_path("replay-tool-mismatch");
    std::fs::write(
        &path,
        format!(
            "{}\n",
            line_json(spec_id, uuid::Uuid::now_v7(), &event, "start_task")
        ),
    )
    .expect("write");

    let err = ingest_phase_events(&store, &path, &[RequiredGuard::GateChecked])
        .await
        .expect_err("tool mismatch");
    assert!(matches!(err, ReplayError::ToolMismatch { .. }));
}

#[tokio::test]
async fn replay_rejects_invalid_sequence() {
    let store = fresh_store().await;
    let spec_id = SpecId::new();
    let task_id = TaskId::new();
    let event = MethodologyEvent::TaskCompleted(TaskCompleted { task_id, spec_id });

    let path = temp_path("replay-invalid-seq");
    std::fs::write(
        &path,
        format!(
            "{}\n",
            line_json(spec_id, uuid::Uuid::now_v7(), &event, tool_for(&event))
        ),
    )
    .expect("write");

    let err = ingest_phase_events(&store, &path, &[RequiredGuard::GateChecked])
        .await
        .expect_err("invalid sequence");
    assert!(matches!(err, ReplayError::MissingTaskCreate { .. }));
}

#[tokio::test]
async fn replay_dedupes_duplicate_event_ids() {
    let store = fresh_store().await;
    let spec_id = SpecId::new();
    let task_id = TaskId::new();
    let event = MethodologyEvent::TaskCreated(TaskCreated {
        task: Box::new(seed_task(spec_id, task_id)),
        origin: TaskOrigin::User,
        idempotency_key: None,
    });
    let event_id = uuid::Uuid::now_v7();

    let line = line_json(spec_id, event_id, &event, tool_for(&event));
    let path = temp_path("replay-dedupe");
    std::fs::write(&path, format!("{line}\n{line}\n")).expect("write");

    let stats = ingest_phase_events(&store, &path, &[RequiredGuard::GateChecked])
        .await
        .expect("ingest");
    assert_eq!(stats.events_appended, 1);
    assert_eq!(stats.events_skipped_duplicate_event_id, 1);
}

#[tokio::test]
async fn replay_reports_malformed_line_with_raw_context() {
    let store = fresh_store().await;
    let path = temp_path("replay-malformed");
    std::fs::write(&path, "{not json}\n").expect("write");

    let err = ingest_phase_events(&store, &path, &[RequiredGuard::GateChecked])
        .await
        .expect_err("malformed");
    assert!(matches!(err, ReplayError::MalformedLine { .. }));
}

#[tokio::test]
async fn findings_by_ids_uses_sparse_lookup() {
    let store = fresh_store().await;
    let spec_id = SpecId::new();
    let task_id = TaskId::new();
    let id1 = FindingId::new();
    let id2 = FindingId::new();
    let f1 = MethodologyEvent::FindingAdded(FindingAdded {
        finding: Box::new(seed_finding(spec_id, task_id, id1, "one")),
        idempotency_key: None,
    });
    let f2 = MethodologyEvent::FindingAdded(FindingAdded {
        finding: Box::new(seed_finding(spec_id, task_id, id2, "two")),
        idempotency_key: None,
    });

    let path = temp_path("findings-by-ids");
    std::fs::write(
        &path,
        format!(
            "{}\n{}\n",
            line_json(spec_id, uuid::Uuid::now_v7(), &f1, "add_finding"),
            line_json(spec_id, uuid::Uuid::now_v7(), &f2, "add_finding")
        ),
    )
    .expect("write");
    ingest_phase_events(&store, &path, &[RequiredGuard::GateChecked])
        .await
        .expect("ingest");

    let one = tanren_store::methodology::projections::findings_by_ids(&store, spec_id, &[id2])
        .await
        .expect("lookup");
    assert_eq!(one.len(), 1);
    assert_eq!(one[0].id, id2);
}
