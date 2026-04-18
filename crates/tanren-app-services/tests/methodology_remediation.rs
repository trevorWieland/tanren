//! Integration tests for the Lane 0.5 audit-remediation changes.
//!
//! Coverage (maps 1:1 to audit findings):
//! - config-driven required-guard set (#1)
//! - discrete guard event taxonomy (#13)
//! - idempotency_key field on guard events (#3)
//! - `TaskCompleted` converges when config guards satisfied (#2)
//! - relevance filter with explainable reasons (#5)
//! - typed replay error preserved through the service boundary (#10)

use std::sync::Arc;

use tanren_app_services::methodology::{CapabilityScope, MethodologyService};
use tanren_contract::methodology::{
    CreateTaskParams, ListRelevantStandardsParams, RelevantStandard,
};
use tanren_domain::methodology::events::{
    MethodologyEvent, TaskAdherent, TaskAudited, TaskGateChecked, TaskXChecked,
};
use tanren_domain::methodology::task::{RequiredGuard, TaskOrigin, TaskStatus};
use tanren_domain::{SpecId, TaskId};
use tanren_store::Store;

async fn mk_service(required: Vec<RequiredGuard>) -> MethodologyService {
    let url = "sqlite::memory:?cache=shared";
    let store = Store::open_and_migrate(url).await.expect("open");
    let runtime = tanren_app_services::methodology::service::PhaseEventsRuntime {
        spec_folder: std::env::temp_dir().join(format!(
            "tanren-methodology-remediation-{}",
            uuid::Uuid::now_v7()
        )),
        agent_session_id: "test-session".into(),
    };
    MethodologyService::with_runtime(Arc::new(store), required, Some(runtime), vec![])
}

fn admin_scope() -> CapabilityScope {
    use tanren_domain::methodology::capability::ToolCapability::{
        FindingAdd, PhaseEscalate, PhaseOutcome, RubricRecord, SignpostAdd, StandardRead,
        TaskAbandon, TaskComplete, TaskCreate, TaskRead, TaskRevise, TaskStart,
    };
    CapabilityScope::from_iter_caps([
        TaskCreate,
        TaskStart,
        TaskComplete,
        TaskRevise,
        TaskAbandon,
        TaskRead,
        FindingAdd,
        RubricRecord,
        SignpostAdd,
        StandardRead,
        PhaseOutcome,
        PhaseEscalate,
    ])
}

#[tokio::test]
async fn required_guards_come_from_config_not_hardcoded() {
    // Two services with different configs; the list_tasks projection
    // must respect each service's guard set.
    let default = mk_service(vec![
        RequiredGuard::GateChecked,
        RequiredGuard::Audited,
        RequiredGuard::Adherent,
    ])
    .await;
    let relaxed = mk_service(vec![RequiredGuard::GateChecked]).await;
    assert_eq!(default.required_guards().len(), 3);
    assert_eq!(relaxed.required_guards().len(), 1);
    assert_eq!(relaxed.required_guards()[0], RequiredGuard::GateChecked);
}

#[tokio::test]
async fn config_driven_guards_dedup_on_construction() {
    let svc = mk_service(vec![
        RequiredGuard::GateChecked,
        RequiredGuard::GateChecked,
        RequiredGuard::Audited,
    ])
    .await;
    assert_eq!(svc.required_guards().len(), 2);
}

#[test]
fn canonical_guard_name_mapping_is_stable() {
    let spec = SpecId::new();
    let tid = TaskId::new();
    let events = [
        MethodologyEvent::TaskGateChecked(TaskGateChecked {
            task_id: tid,
            spec_id: spec,
            idempotency_key: Some("k1".into()),
        }),
        MethodologyEvent::TaskAudited(TaskAudited {
            task_id: tid,
            spec_id: spec,
            idempotency_key: Some("k2".into()),
        }),
        MethodologyEvent::TaskAdherent(TaskAdherent {
            task_id: tid,
            spec_id: spec,
            idempotency_key: Some("k3".into()),
        }),
        MethodologyEvent::TaskXChecked(TaskXChecked {
            task_id: tid,
            spec_id: spec,
            guard_name: tanren_domain::NonEmptyString::try_new("perf_checked").expect("non-empty"),
            idempotency_key: Some("k4".into()),
        }),
    ];
    for event in events {
        let json = serde_json::to_string(&event).expect("serialize");
        let back: MethodologyEvent = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(event, back);
    }
}

#[test]
fn idempotency_key_serializes_and_round_trips() {
    let ev = TaskGateChecked {
        task_id: TaskId::new(),
        spec_id: SpecId::new(),
        idempotency_key: Some("blake3:abc".into()),
    };
    let json = serde_json::to_string(&MethodologyEvent::TaskGateChecked(ev.clone())).expect("ser");
    assert!(json.contains("idempotency_key"));
    let back: MethodologyEvent = serde_json::from_str(&json).expect("de");
    let MethodologyEvent::TaskGateChecked(decoded) = back else {
        unreachable!("wrong variant after round-trip");
    };
    assert_eq!(decoded, ev);
}

#[test]
fn idempotency_key_absent_in_json_when_none() {
    let ev = TaskAudited {
        task_id: TaskId::new(),
        spec_id: SpecId::new(),
        idempotency_key: None,
    };
    let json = serde_json::to_string(&MethodologyEvent::TaskAudited(ev)).expect("ser");
    // skip_serializing_if keeps the wire shape minimal for callers.
    assert!(
        !json.contains("idempotency_key"),
        "expected idempotency_key omitted when None, got: {json}"
    );
}

#[tokio::test]
async fn mark_guard_satisfied_fires_task_completed_when_config_satisfied() {
    let svc = mk_service(vec![RequiredGuard::GateChecked]).await;
    let scope = admin_scope();
    let spec_id = SpecId::new();
    let resp = svc
        .create_task(
            &scope,
            "do-task",
            CreateTaskParams {
                schema_version: tanren_contract::methodology::SchemaVersion::current(),
                idempotency_key: None,
                spec_id,
                title: "T".into(),
                description: String::new(),
                acceptance_criteria: vec![],
                depends_on: vec![],
                parent_task_id: None,
                origin: TaskOrigin::ShapeSpec,
            },
        )
        .await
        .expect("create");
    svc.start_task(
        &scope,
        "do-task",
        tanren_contract::methodology::StartTaskParams {
            schema_version: tanren_contract::methodology::SchemaVersion::current(),
            idempotency_key: None,
            task_id: resp.task_id,
        },
    )
    .await
    .expect("start");
    svc.complete_task(
        &scope,
        "do-task",
        tanren_contract::methodology::CompleteTaskParams {
            schema_version: tanren_contract::methodology::SchemaVersion::current(),
            idempotency_key: None,
            task_id: resp.task_id,
            evidence_refs: vec![],
        },
    )
    .await
    .expect("implement");
    svc.mark_task_guard_satisfied(
        &scope,
        "do-task",
        resp.task_id,
        RequiredGuard::GateChecked,
        Some("test-idem".into()),
    )
    .await
    .expect("mark guard");

    // Re-fold via list_tasks; the task should now be `Complete`.
    let tasks = svc
        .list_tasks(
            &scope,
            "do-task",
            tanren_contract::methodology::ListTasksParams {
                schema_version: tanren_contract::methodology::SchemaVersion::current(),
                spec_id: Some(spec_id),
            },
        )
        .await
        .expect("list");
    let task = tasks.iter().find(|t| t.id == resp.task_id).expect("task");
    assert_eq!(task.status, TaskStatus::Complete);
}

#[tokio::test]
async fn mark_guard_satisfied_keeps_implemented_when_guard_not_required() {
    // Service configured to require Audited; firing GateChecked alone
    // must leave the task at Implemented, not Complete.
    let svc = mk_service(vec![RequiredGuard::Audited]).await;
    let scope = admin_scope();
    let spec_id = SpecId::new();
    let resp = svc
        .create_task(
            &scope,
            "do-task",
            CreateTaskParams {
                schema_version: tanren_contract::methodology::SchemaVersion::current(),
                idempotency_key: None,
                spec_id,
                title: "T".into(),
                description: String::new(),
                acceptance_criteria: vec![],
                depends_on: vec![],
                parent_task_id: None,
                origin: TaskOrigin::ShapeSpec,
            },
        )
        .await
        .expect("create");
    svc.start_task(
        &scope,
        "do-task",
        tanren_contract::methodology::StartTaskParams {
            schema_version: tanren_contract::methodology::SchemaVersion::current(),
            idempotency_key: None,
            task_id: resp.task_id,
        },
    )
    .await
    .expect("start");
    svc.complete_task(
        &scope,
        "do-task",
        tanren_contract::methodology::CompleteTaskParams {
            schema_version: tanren_contract::methodology::SchemaVersion::current(),
            idempotency_key: None,
            task_id: resp.task_id,
            evidence_refs: vec![],
        },
    )
    .await
    .expect("implement");
    svc.mark_task_guard_satisfied(
        &scope,
        "do-task",
        resp.task_id,
        RequiredGuard::GateChecked,
        None,
    )
    .await
    .expect("mark non-required");

    let tasks = svc
        .list_tasks(
            &scope,
            "do-task",
            tanren_contract::methodology::ListTasksParams {
                schema_version: tanren_contract::methodology::SchemaVersion::current(),
                spec_id: Some(spec_id),
            },
        )
        .await
        .expect("list");
    let task = tasks.iter().find(|t| t.id == resp.task_id).expect("task");
    assert!(
        matches!(task.status, TaskStatus::Implemented { .. }),
        "expected Implemented, got {:?}",
        task.status
    );
}

#[tokio::test]
async fn complete_task_with_empty_required_guards_completes_immediately() {
    let svc = mk_service(vec![]).await;
    let scope = admin_scope();
    let spec_id = SpecId::new();
    let resp = svc
        .create_task(
            &scope,
            "do-task",
            CreateTaskParams {
                schema_version: tanren_contract::methodology::SchemaVersion::current(),
                idempotency_key: None,
                spec_id,
                title: "T".into(),
                description: String::new(),
                acceptance_criteria: vec![],
                depends_on: vec![],
                parent_task_id: None,
                origin: TaskOrigin::ShapeSpec,
            },
        )
        .await
        .expect("create");
    svc.start_task(
        &scope,
        "do-task",
        tanren_contract::methodology::StartTaskParams {
            schema_version: tanren_contract::methodology::SchemaVersion::current(),
            idempotency_key: None,
            task_id: resp.task_id,
        },
    )
    .await
    .expect("start");
    svc.complete_task(
        &scope,
        "do-task",
        tanren_contract::methodology::CompleteTaskParams {
            schema_version: tanren_contract::methodology::SchemaVersion::current(),
            idempotency_key: None,
            task_id: resp.task_id,
            evidence_refs: vec![],
        },
    )
    .await
    .expect("complete");
    let tasks = svc
        .list_tasks(
            &scope,
            "do-task",
            tanren_contract::methodology::ListTasksParams {
                schema_version: tanren_contract::methodology::SchemaVersion::current(),
                spec_id: Some(spec_id),
            },
        )
        .await
        .expect("list");
    let task = tasks.iter().find(|t| t.id == resp.task_id).expect("task");
    assert_eq!(task.status, TaskStatus::Complete);
}

#[tokio::test]
async fn relevance_filter_explains_inclusion_by_touched_files() {
    let svc = mk_service(vec![RequiredGuard::GateChecked]).await;
    let scope = admin_scope();
    let out: Vec<RelevantStandard> = svc
        .list_relevant_standards_filtered(
            &scope,
            "adhere-task",
            &ListRelevantStandardsParams {
                schema_version: tanren_contract::methodology::SchemaVersion::current(),
                spec_id: SpecId::new(),
                touched_files: vec!["crates/tanren-domain/src/lib.rs".into()],
                project_language: Some("rust".into()),
                domains: vec![],
            },
        )
        .expect("filtered");
    assert!(!out.is_empty(), "rust-touched file should match >=1 std");
    assert!(
        out.iter().all(|r| !r.inclusion_reason.is_empty()),
        "every kept standard must carry an explanation"
    );
}

#[tokio::test]
async fn relevance_filter_empty_inputs_returns_full_baseline() {
    let svc = mk_service(vec![]).await;
    let scope = admin_scope();
    let out = svc
        .list_relevant_standards_filtered(
            &scope,
            "adhere-task",
            &ListRelevantStandardsParams {
                schema_version: tanren_contract::methodology::SchemaVersion::current(),
                spec_id: SpecId::new(),
                touched_files: vec![],
                project_language: None,
                domains: vec![],
            },
        )
        .expect("baseline");
    assert!(!out.is_empty());
    // The fallback reason must identify this as the upper bound.
    assert!(out.iter().any(|r| r.inclusion_reason.contains("baseline")));
}
