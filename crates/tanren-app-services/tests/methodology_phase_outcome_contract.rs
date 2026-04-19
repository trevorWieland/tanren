use std::sync::Arc;

use tanren_app_services::methodology::service::PhaseEventsRuntime;
use tanren_app_services::methodology::{
    CapabilityScope, MethodologyError, MethodologyService, PhaseEventLine, PhaseId,
};
use tanren_contract::methodology::{
    CompleteTaskParams, CreateTaskParams, ReportPhaseOutcomeParams, SchemaVersion, StartTaskParams,
};
use tanren_domain::methodology::capability::ToolCapability;
use tanren_domain::methodology::events::MethodologyEvent;
use tanren_domain::methodology::phase_outcome::PhaseOutcome;
use tanren_domain::methodology::task::{RequiredGuard, TaskOrigin};
use tanren_domain::{NonEmptyString, SpecId, TaskId};
use tanren_store::Store;

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
        tanren_domain::EntityRef::Task(task_id),
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
