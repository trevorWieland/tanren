#![cfg(feature = "postgres-integration")]

#[path = "common/support_postgres.rs"]
mod support_postgres;

use chrono::Utc;
use serde_json::json;
use support_postgres::postgres_fixture;
use tanren_domain::methodology::event_tool::canonical_tool_for_event;
use tanren_domain::methodology::events::{
    FindingAdded, MethodologyEvent, TaskAbandoned, TaskAdherent, TaskAudited, TaskCompleted,
    TaskCreated, TaskGateChecked, TaskImplemented, TaskStarted,
};
use tanren_domain::methodology::finding::{Finding, FindingSeverity, FindingSource};
use tanren_domain::methodology::phase_id::PhaseId;
use tanren_domain::methodology::task::{
    ExplicitUserDiscardProvenance, RequiredGuard, Task, TaskAbandonDisposition, TaskOrigin,
    TaskStatus,
};
use tanren_domain::{EntityKind, FindingId, NonEmptyString, SpecId, TaskId};
use tanren_store::methodology::{ReplayError, ingest_phase_events};
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
    line_json_with_attribution(spec_id, event_id, event, tool, Some("tool_primary"), None)
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
    obj.insert("schema_version".into(), json!("1.0.0"));
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
    let dir = std::env::temp_dir().join(format!("tanren-store-{name}-{}", uuid::Uuid::now_v7()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    dir.join("phase-events.jsonl")
}

fn seed_finding(spec_id: SpecId, id: FindingId) -> Finding {
    Finding {
        id,
        spec_id,
        severity: FindingSeverity::FixNow,
        title: NonEmptyString::try_new("f").expect("title"),
        description: String::new(),
        affected_files: vec!["src/lib.rs".into()],
        line_numbers: vec![1],
        source: FindingSource::Audit {
            phase: PhaseId::try_new("audit-task").expect("phase"),
            pillar: None,
        },
        attached_task: None,
        created_at: Utc::now(),
    }
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn replay_rejects_report_phase_outcome_alias_for_bridged_guard_completion_postgres() {
    let fixture = postgres_fixture().await;
    let _postgres_url = fixture.url.clone();
    let store = fixture.store;
    let spec_id = SpecId::new();
    let task_id = TaskId::new();

    let events = vec![
        (
            MethodologyEvent::TaskCreated(TaskCreated {
                task: Box::new(seed_task(spec_id, task_id)),
                origin: TaskOrigin::User,
                idempotency_key: None,
            }),
            "create_task",
            Some("tool_primary"),
            None,
        ),
        (
            MethodologyEvent::TaskStarted(TaskStarted { task_id, spec_id }),
            "start_task",
            Some("tool_primary"),
            None,
        ),
        (
            MethodologyEvent::TaskImplemented(TaskImplemented {
                task_id,
                spec_id,
                evidence_refs: vec![],
            }),
            "complete_task",
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

    let path = temp_path("replay-postgres-phase-outcome-bridge-tool-alias");
    let mut content = String::new();
    for (event, tool, origin_kind, caused_by_tool_call_id) in &events {
        content.push_str(&line_json_with_attribution(
            spec_id,
            uuid::Uuid::now_v7(),
            event,
            tool,
            *origin_kind,
            *caused_by_tool_call_id,
        ));
        content.push('\n');
    }
    std::fs::write(&path, content).expect("write jsonl");

    let err = ingest_phase_events(
        &store,
        &path,
        &[RequiredGuard::Audited, RequiredGuard::Adherent],
    )
    .await
    .expect_err("non-canonical alias must fail replay");
    assert!(matches!(err, ReplayError::ToolMismatch { .. }));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn replay_rejects_invalid_abandon_semantics_postgres() {
    let fixture = postgres_fixture().await;
    let store = fixture.store;
    let spec_id = SpecId::new();
    let task_id = TaskId::new();

    let created = MethodologyEvent::TaskCreated(TaskCreated {
        task: Box::new(seed_task(spec_id, task_id)),
        origin: TaskOrigin::User,
        idempotency_key: None,
    });
    let abandoned = MethodologyEvent::TaskAbandoned(TaskAbandoned {
        task_id,
        spec_id,
        reason: NonEmptyString::try_new("explicit discard").expect("reason"),
        disposition: TaskAbandonDisposition::ExplicitUserDiscard,
        replacements: vec![],
        explicit_user_discard_provenance: Some(ExplicitUserDiscardProvenance::ResolveBlockers {
            resolution_note: NonEmptyString::try_new("approved").expect("note"),
        }),
    });

    let path = temp_path("replay-postgres-invalid-abandon");
    std::fs::write(
        &path,
        format!(
            "{}\n{}\n",
            line_json(
                spec_id,
                uuid::Uuid::now_v7(),
                &created,
                canonical_tool_for_event(&created),
            ),
            line_json(
                spec_id,
                uuid::Uuid::now_v7(),
                &abandoned,
                canonical_tool_for_event(&abandoned),
            )
        ),
    )
    .expect("write jsonl");

    let err = ingest_phase_events(&store, &path, &[RequiredGuard::GateChecked])
        .await
        .expect_err("invalid abandon semantics must fail");
    assert!(matches!(
        err,
        ReplayError::FieldValidation { ref details } if details.field_path == "/disposition"
    ));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn replay_rejects_invalid_finding_payload_postgres() {
    let fixture = postgres_fixture().await;
    let store = fixture.store;
    let spec_id = SpecId::new();
    let mut finding = seed_finding(spec_id, FindingId::new());
    finding.line_numbers = vec![0];
    let event = MethodologyEvent::FindingAdded(FindingAdded {
        finding: Box::new(finding),
        idempotency_key: None,
    });
    let path = temp_path("replay-postgres-invalid-finding");
    std::fs::write(
        &path,
        format!(
            "{}\n",
            line_json(
                spec_id,
                uuid::Uuid::now_v7(),
                &event,
                canonical_tool_for_event(&event),
            )
        ),
    )
    .expect("write jsonl");

    let err = ingest_phase_events(&store, &path, &[RequiredGuard::GateChecked])
        .await
        .expect_err("invalid finding payload must fail");
    assert!(matches!(
        err,
        ReplayError::FieldValidation { ref details } if details.field_path == "/line_numbers/0"
    ));
}
