use std::sync::Arc;

use tanren_app_services::methodology::service::PhaseEventsRuntime;
use tanren_app_services::methodology::{
    CapabilityScope, MethodologyError, MethodologyService, PhaseEventLine, PhaseId,
    line_for_envelope,
};
use tanren_contract::methodology::{
    AbandonTaskParams, CompleteTaskParams, CreateTaskParams, ReportPhaseOutcomeParams,
    SchemaVersion, StartTaskParams,
};
use tanren_domain::events::{DomainEvent, EventEnvelope};
use tanren_domain::methodology::capability::ToolCapability;
use tanren_domain::methodology::events::{MethodologyEvent, TaskStarted};
use tanren_domain::methodology::phase_outcome::PhaseOutcome;
use tanren_domain::methodology::task::{
    ExplicitUserDiscardProvenance, RequiredGuard, TaskAbandonDisposition, TaskOrigin,
};
use tanren_domain::{EntityRef, EventId, NonEmptyString, SpecId, TaskId};
use tanren_store::Store;
use tanren_store::methodology::AppendPhaseEventOutboxParams;

fn phase(tag: &str) -> PhaseId {
    PhaseId::try_new(tag).expect("phase")
}

fn scope(caps: &[ToolCapability]) -> CapabilityScope {
    CapabilityScope::from_iter_caps(caps.iter().copied())
}

async fn mk_service(
    required: Vec<RequiredGuard>,
    runtime: PhaseEventsRuntime,
) -> MethodologyService {
    let store = Store::open_and_migrate("sqlite::memory:?cache=shared")
        .await
        .expect("open");
    MethodologyService::with_runtime(Arc::new(store), required, Some(runtime), vec![])
}

async fn create_implemented_task(
    svc: &MethodologyService,
    spec_id: SpecId,
    phase_name: &str,
) -> TaskId {
    let task_scope = scope(&[
        ToolCapability::TaskCreate,
        ToolCapability::TaskStart,
        ToolCapability::TaskComplete,
    ]);
    let created = svc
        .create_task(
            &task_scope,
            &phase(phase_name),
            CreateTaskParams {
                schema_version: SchemaVersion::current(),
                spec_id,
                title: "task".into(),
                description: String::new(),
                parent_task_id: None,
                depends_on: vec![],
                origin: TaskOrigin::User,
                acceptance_criteria: vec![],
                idempotency_key: None,
            },
        )
        .await
        .expect("create task");
    svc.start_task(
        &task_scope,
        &phase(phase_name),
        StartTaskParams {
            schema_version: SchemaVersion::current(),
            task_id: created.task_id,
            idempotency_key: None,
        },
    )
    .await
    .expect("start task");
    svc.complete_task(
        &task_scope,
        &phase(phase_name),
        CompleteTaskParams {
            schema_version: SchemaVersion::current(),
            task_id: created.task_id,
            evidence_refs: vec![],
            idempotency_key: None,
        },
    )
    .await
    .expect("complete task");
    created.task_id
}

fn phase_events(svc: &MethodologyService) -> Vec<PhaseEventLine> {
    let runtime = svc.phase_events_runtime().expect("runtime");
    let path = runtime.spec_folder.join("phase-events.jsonl");
    let raw = std::fs::read_to_string(path).expect("read phase events");
    raw.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str::<PhaseEventLine>(line).expect("phase event line"))
        .collect()
}

#[path = "methodology_phase_outcome_contract/abandon_disposition.rs"]
mod abandon_disposition;

#[tokio::test]
async fn report_phase_outcome_complete_bridges_task_guard_and_completion() {
    let spec_id = SpecId::new();
    let runtime = PhaseEventsRuntime {
        spec_id,
        spec_folder: std::env::temp_dir().join(format!(
            "tanren-phase-outcome-bridge-{}",
            uuid::Uuid::now_v7()
        )),
        agent_session_id: "runtime-session-audit".into(),
    };
    let svc = mk_service(vec![RequiredGuard::Audited], runtime).await;
    let task_id = create_implemented_task(&svc, spec_id, "do-task").await;

    let phase_scope = scope(&[ToolCapability::PhaseOutcome]);
    svc.report_phase_outcome(
        &phase_scope,
        &phase("audit-task"),
        ReportPhaseOutcomeParams {
            schema_version: SchemaVersion::current(),
            spec_id,
            task_id: Some(task_id),
            outcome: PhaseOutcome::Complete {
                summary: NonEmptyString::try_new("audit complete").expect("summary"),
                next_action_hint: None,
            },
            idempotency_key: Some("phase-outcome-bridge".into()),
        },
    )
    .await
    .expect("report outcome");

    let events = tanren_store::methodology::projections::load_methodology_events_for_entity(
        svc.store(),
        EntityRef::Task(task_id),
        Some(spec_id),
        100,
    )
    .await
    .expect("load events");

    assert!(
        events.iter().any(|e| {
            matches!(
                e,
                MethodologyEvent::TaskAudited(ev) if ev.task_id == task_id
            )
        }),
        "audit-task complete should emit TaskAudited"
    );
    assert!(
        events.iter().any(|e| {
            matches!(
                e,
                MethodologyEvent::TaskCompleted(ev) if ev.task_id == task_id
            )
        }),
        "required guards should converge to TaskCompleted"
    );
}

#[tokio::test]
async fn report_phase_outcome_uses_runtime_phase_and_session_attribution() {
    let spec_id = SpecId::new();
    let runtime = PhaseEventsRuntime {
        spec_id,
        spec_folder: std::env::temp_dir().join(format!(
            "tanren-phase-outcome-attribution-{}",
            uuid::Uuid::now_v7()
        )),
        agent_session_id: "runtime-session-123".into(),
    };
    let svc = mk_service(vec![], runtime.clone()).await;

    let phase_scope = scope(&[ToolCapability::PhaseOutcome]);
    svc.report_phase_outcome(
        &phase_scope,
        &phase("shape-spec"),
        ReportPhaseOutcomeParams {
            schema_version: SchemaVersion::current(),
            spec_id,
            task_id: None,
            outcome: PhaseOutcome::Complete {
                summary: NonEmptyString::try_new("shaped").expect("summary"),
                next_action_hint: None,
            },
            idempotency_key: Some("phase-outcome-attribution".into()),
        },
    )
    .await
    .expect("report outcome");

    let lines = phase_events(&svc);
    let line = lines
        .iter()
        .rev()
        .find(|line| matches!(line.payload, MethodologyEvent::PhaseOutcomeReported(_)))
        .expect("phase outcome line");
    let MethodologyEvent::PhaseOutcomeReported(payload) = &line.payload else {
        return;
    };

    assert_eq!(line.phase.as_str(), "shape-spec");
    assert_eq!(line.agent_session_id.as_str(), runtime.agent_session_id);
    assert_eq!(payload.phase.as_str(), "shape-spec");
    assert_eq!(payload.agent_session_id.as_str(), runtime.agent_session_id);
}

#[tokio::test]
async fn runtime_spec_binding_rejects_cross_spec_mutations() {
    let runtime_spec_id = SpecId::new();
    let runtime = PhaseEventsRuntime {
        spec_id: runtime_spec_id,
        spec_folder: std::env::temp_dir().join(format!(
            "tanren-phase-outcome-spec-binding-{}",
            uuid::Uuid::now_v7()
        )),
        agent_session_id: "runtime-session-binding".into(),
    };
    let svc = mk_service(vec![], runtime).await;
    let wrong_spec = SpecId::new();

    let err = svc
        .create_task(
            &scope(&[ToolCapability::TaskCreate]),
            &phase("shape-spec"),
            CreateTaskParams {
                schema_version: SchemaVersion::current(),
                spec_id: wrong_spec,
                title: "wrong-spec".into(),
                description: String::new(),
                parent_task_id: None,
                depends_on: vec![],
                origin: TaskOrigin::User,
                acceptance_criteria: vec![],
                idempotency_key: None,
            },
        )
        .await
        .expect_err("cross-spec mutation must be rejected");
    assert!(matches!(
        err,
        MethodologyError::FieldValidation { ref field_path, .. } if field_path == "/spec_id"
    ));

    let pending = svc
        .store()
        .load_pending_phase_event_outbox(None, 100)
        .await
        .expect("pending outbox");
    assert!(
        pending.is_empty(),
        "rejected mutations must not append outbox rows"
    );
}

#[tokio::test]
async fn projection_fail_closed_then_reconcile_recovers_without_duplicates() {
    let spec_id = SpecId::new();
    let temp_root = std::env::temp_dir().join(format!(
        "tanren-phase-outcome-outbox-{}",
        uuid::Uuid::now_v7()
    ));
    std::fs::create_dir_all(&temp_root).expect("mkdir temp root");
    let broken_folder = temp_root.join("spec-folder-as-file");
    std::fs::write(&broken_folder, "not a directory").expect("seed file");

    let runtime = PhaseEventsRuntime {
        spec_id,
        spec_folder: broken_folder.clone(),
        agent_session_id: "runtime-session-outbox".into(),
    };
    let svc = mk_service(vec![], runtime).await;

    let err = svc
        .create_task(
            &scope(&[ToolCapability::TaskCreate]),
            &phase("shape-spec"),
            CreateTaskParams {
                schema_version: SchemaVersion::current(),
                spec_id,
                title: "task".into(),
                description: String::new(),
                parent_task_id: None,
                depends_on: vec![],
                origin: TaskOrigin::User,
                acceptance_criteria: vec![],
                idempotency_key: Some("fail-closed-outbox".into()),
            },
        )
        .await
        .expect_err("projection write failure should fail closed");
    assert!(
        !err.to_string().is_empty(),
        "failing projection should return a typed error"
    );

    let pending = svc
        .store()
        .load_pending_phase_event_outbox(Some(spec_id), 100)
        .await
        .expect("pending rows after failure");
    assert_eq!(pending.len(), 1, "one row should remain pending");

    std::fs::remove_file(&broken_folder).expect("remove blocking file");
    std::fs::create_dir_all(&broken_folder).expect("create folder");

    let recovered = svc
        .reconcile_phase_events_outbox_for_folder(&broken_folder)
        .await
        .expect("reconcile");
    assert_eq!(recovered, 1, "reconcile should project the pending row");

    let after = svc
        .store()
        .load_pending_phase_event_outbox(Some(spec_id), 100)
        .await
        .expect("pending rows after reconcile");
    assert!(after.is_empty(), "pending rows should be drained");

    let events_path = broken_folder.join("phase-events.jsonl");
    let content = std::fs::read_to_string(&events_path).expect("phase-events.jsonl written");
    assert_eq!(
        content
            .lines()
            .filter(|line| !line.trim().is_empty())
            .count(),
        1,
        "reconcile should write exactly one line"
    );

    let second = svc
        .reconcile_phase_events_outbox_for_folder(&broken_folder)
        .await
        .expect("second reconcile");
    assert_eq!(second, 0, "no extra pending rows on second reconcile");

    let content_after =
        std::fs::read_to_string(events_path).expect("phase-events.jsonl still readable");
    assert_eq!(
        content_after
            .lines()
            .filter(|line| !line.trim().is_empty())
            .count(),
        1,
        "second reconcile must not append duplicates"
    );
}

#[tokio::test]
async fn drain_budget_does_not_fail_mutation_when_backlog_remains() {
    let spec_id = SpecId::new();
    let root = tempfile::tempdir().expect("tempdir");
    let spec_folder = root
        .path()
        .join(format!("2026-01-01-0101-{spec_id}-outbox-backlog"));
    std::fs::create_dir_all(&spec_folder).expect("mkdir spec folder");
    let runtime = PhaseEventsRuntime {
        spec_id,
        spec_folder: spec_folder.clone(),
        agent_session_id: "runtime-session-backlog".into(),
    };
    let svc = mk_service(vec![], runtime).await;
    let phase = phase("shape-spec");
    let phase_name = phase.as_str().to_owned();
    let spec_folder_raw = spec_folder.to_string_lossy().to_string();

    let backlog_rows = 2_080usize;
    for _ in 0..backlog_rows {
        let task_id = TaskId::new();
        let envelope = EventEnvelope {
            schema_version: tanren_domain::SCHEMA_VERSION,
            event_id: EventId::new(),
            timestamp: chrono::Utc::now(),
            entity_ref: EntityRef::Task(task_id),
            payload: DomainEvent::Methodology {
                event: MethodologyEvent::TaskStarted(TaskStarted { task_id, spec_id }),
            },
        };
        let line = line_for_envelope(
            &envelope,
            spec_id,
            phase_name.as_str(),
            "runtime-session-backlog",
        )
        .expect("project line");
        let line_json = serde_json::to_string(&line).expect("line json");
        svc.store()
            .append_methodology_event_with_outbox(
                &envelope,
                Some(AppendPhaseEventOutboxParams {
                    spec_id,
                    spec_folder: spec_folder_raw.clone(),
                    line_json,
                }),
            )
            .await
            .expect("seed outbox");
    }

    let result = svc
        .create_task(
            &scope(&[ToolCapability::TaskCreate]),
            &phase,
            CreateTaskParams {
                schema_version: SchemaVersion::current(),
                spec_id,
                title: "mutation-under-backlog".into(),
                description: String::new(),
                parent_task_id: None,
                depends_on: vec![],
                origin: TaskOrigin::User,
                acceptance_criteria: vec![],
                idempotency_key: Some("backlog-mutation-success".into()),
            },
        )
        .await;
    assert!(
        result.is_ok(),
        "mutation should succeed after durable append even when backlog remains: {result:?}"
    );

    let pending = svc
        .store()
        .load_pending_phase_event_outbox(Some(spec_id), 10_000)
        .await
        .expect("load pending");
    assert!(
        !pending.is_empty(),
        "budgeted draining should leave pending rows under heavy backlog"
    );

    let phase_events = spec_folder.join("phase-events.jsonl");
    let content = std::fs::read_to_string(phase_events).expect("phase events");
    assert!(
        !content.trim().is_empty(),
        "drainer should still project a best-effort prefix of the backlog"
    );
}
