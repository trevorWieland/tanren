//! Integration tests for finding validation and projection-backed task listing.

use std::sync::Arc;

use tanren_app_services::methodology::{CapabilityScope, MethodologyService};
use tanren_contract::methodology::{AddFindingParams, CreateTaskParams, ListTasksParams};
use tanren_domain::methodology::finding::{FindingSeverity, FindingSource};
use tanren_domain::methodology::phase_id::PhaseId;
use tanren_domain::methodology::task::{RequiredGuard, TaskOrigin, TaskStatus};
use tanren_domain::{SpecId, TaskId};
use tanren_store::Store;

async fn mk_service(required: Vec<RequiredGuard>) -> MethodologyService {
    let url = "sqlite::memory:?cache=shared";
    let store = Store::open_and_migrate(url).await.expect("open");
    let runtime = tanren_app_services::methodology::service::PhaseEventsRuntime {
        spec_id: SpecId::new(),
        spec_folder: std::env::temp_dir().join(format!(
            "tanren-methodology-remediation-projection-{}",
            uuid::Uuid::now_v7()
        )),
        agent_session_id: "test-session".into(),
    };
    MethodologyService::with_runtime(Arc::new(store), required, Some(runtime), vec![])
}

fn admin_scope() -> CapabilityScope {
    use tanren_domain::methodology::capability::ToolCapability::{
        FindingAdd, TaskComplete, TaskCreate, TaskRead, TaskStart,
    };
    CapabilityScope::from_iter_caps([TaskCreate, TaskStart, TaskComplete, TaskRead, FindingAdd])
}

fn phase(tag: &str) -> PhaseId {
    PhaseId::try_new(tag).expect("phase")
}

fn runtime_spec_id(svc: &MethodologyService) -> SpecId {
    svc.phase_events_runtime().expect("runtime").spec_id
}

fn audit_finding_source() -> FindingSource {
    FindingSource::Audit {
        phase: PhaseId::try_new("audit-task").expect("phase"),
        pillar: None,
    }
}

fn finding_params(
    spec_id: SpecId,
    attached_task: Option<TaskId>,
    line_numbers: Vec<u32>,
) -> AddFindingParams {
    AddFindingParams {
        schema_version: tanren_contract::methodology::SchemaVersion::current(),
        spec_id,
        severity: FindingSeverity::FixNow,
        title: "audit finding".into(),
        description: "details".into(),
        affected_files: vec!["src/lib.rs".into()],
        line_numbers,
        source: audit_finding_source(),
        attached_task,
        idempotency_key: None,
    }
}

#[tokio::test]
async fn add_finding_rejects_unknown_attached_task() {
    let svc = mk_service(vec![RequiredGuard::GateChecked]).await;
    let scope = admin_scope();
    let spec_id = runtime_spec_id(&svc);
    let err = svc
        .add_finding(
            &scope,
            &phase("audit-task"),
            finding_params(spec_id, Some(TaskId::new()), vec![7]),
        )
        .await
        .expect_err("unknown attached_task must fail");
    assert!(matches!(
        err,
        tanren_app_services::methodology::MethodologyError::NotFound { ref resource, .. }
            if resource == "task"
    ));
}

#[tokio::test]
async fn add_finding_rejects_cross_spec_attached_task() {
    let svc = mk_service(vec![RequiredGuard::GateChecked]).await;
    let scope = admin_scope();
    let spec_a = runtime_spec_id(&svc);
    let spec_b = SpecId::new();
    let created = svc
        .create_task(
            &scope,
            &phase("do-task"),
            CreateTaskParams {
                schema_version: tanren_contract::methodology::SchemaVersion::current(),
                idempotency_key: None,
                spec_id: spec_a,
                title: "cross spec task".into(),
                description: String::new(),
                acceptance_criteria: vec![],
                depends_on: vec![],
                parent_task_id: None,
                origin: TaskOrigin::ShapeSpec,
            },
        )
        .await
        .expect("create");
    let err = svc
        .add_finding(
            &scope,
            &phase("audit-task"),
            finding_params(spec_b, Some(created.task_id), vec![3]),
        )
        .await
        .expect_err("cross-spec attached_task must fail");
    assert!(matches!(
        err,
        tanren_app_services::methodology::MethodologyError::FieldValidation { ref field_path, .. }
            if field_path == "/attached_task"
    ));
}

#[tokio::test]
async fn add_finding_rejects_zero_line_numbers_with_indexed_path() {
    let svc = mk_service(vec![RequiredGuard::GateChecked]).await;
    let scope = admin_scope();
    let spec_id = runtime_spec_id(&svc);
    let err = svc
        .add_finding(
            &scope,
            &phase("audit-task"),
            finding_params(spec_id, None, vec![14, 0, 22]),
        )
        .await
        .expect_err("line number zero must fail");
    assert!(matches!(
        err,
        tanren_app_services::methodology::MethodologyError::FieldValidation { ref field_path, .. }
            if field_path == "/line_numbers/1"
    ));
}

#[tokio::test]
async fn list_tasks_projection_first_matches_fold_truth() {
    let svc = mk_service(vec![RequiredGuard::GateChecked]).await;
    let scope = admin_scope();
    let spec_id = runtime_spec_id(&svc);
    let created = svc
        .create_task(
            &scope,
            &phase("do-task"),
            CreateTaskParams {
                schema_version: tanren_contract::methodology::SchemaVersion::current(),
                idempotency_key: None,
                spec_id,
                title: "projection parity".into(),
                description: "ensure projection list parity".into(),
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
        &phase("do-task"),
        tanren_contract::methodology::StartTaskParams {
            schema_version: tanren_contract::methodology::SchemaVersion::current(),
            idempotency_key: None,
            task_id: created.task_id,
        },
    )
    .await
    .expect("start");
    svc.complete_task(
        &scope,
        &phase("do-task"),
        tanren_contract::methodology::CompleteTaskParams {
            schema_version: tanren_contract::methodology::SchemaVersion::current(),
            idempotency_key: None,
            task_id: created.task_id,
            evidence_refs: vec![],
        },
    )
    .await
    .expect("implement");
    let projected_rows = svc
        .store()
        .load_methodology_task_list_projection(spec_id)
        .await
        .expect("load projection rows");
    assert!(
        projected_rows.iter().all(|row| row.task.is_some()),
        "projection-first path requires full task snapshots"
    );

    let listed = svc
        .list_tasks(
            &scope,
            &phase("do-task"),
            ListTasksParams {
                schema_version: tanren_contract::methodology::SchemaVersion::current(),
                spec_id: Some(spec_id),
            },
        )
        .await
        .expect("list");
    let folded = tanren_store::methodology::projections::tasks_for_spec(
        svc.store(),
        spec_id,
        svc.required_guards(),
    )
    .await
    .expect("fold");
    assert_eq!(listed.tasks, folded);
}

#[tokio::test]
async fn list_tasks_fallback_backfills_missing_projection_snapshots() {
    let svc = mk_service(vec![RequiredGuard::GateChecked]).await;
    let scope = admin_scope();
    let spec_id = runtime_spec_id(&svc);
    let created = svc
        .create_task(
            &scope,
            &phase("do-task"),
            CreateTaskParams {
                schema_version: tanren_contract::methodology::SchemaVersion::current(),
                idempotency_key: None,
                spec_id,
                title: "projection backfill".into(),
                description: "force fallback".into(),
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
        &phase("do-task"),
        tanren_contract::methodology::StartTaskParams {
            schema_version: tanren_contract::methodology::SchemaVersion::current(),
            idempotency_key: None,
            task_id: created.task_id,
        },
    )
    .await
    .expect("start");

    // Simulate legacy/incomplete projection state with no task snapshot payload.
    svc.store()
        .upsert_methodology_task_status_projection(
            spec_id,
            created.task_id,
            &TaskStatus::InProgress,
        )
        .await
        .expect("downgrade projection to status-only row");
    let degraded = svc
        .store()
        .load_methodology_task_list_projection(spec_id)
        .await
        .expect("load degraded projection");
    assert!(
        degraded.iter().any(|row| row.task.is_none()),
        "expected status-only projection row before fallback/backfill"
    );

    let listed = svc
        .list_tasks(
            &scope,
            &phase("do-task"),
            ListTasksParams {
                schema_version: tanren_contract::methodology::SchemaVersion::current(),
                spec_id: Some(spec_id),
            },
        )
        .await
        .expect("list via fallback");
    assert!(
        listed.tasks.iter().any(|task| task.id == created.task_id),
        "fallback path must still return folded tasks"
    );

    let healed = svc
        .store()
        .load_methodology_task_list_projection(spec_id)
        .await
        .expect("load healed projection");
    assert!(
        healed.iter().all(|row| row.task.is_some()),
        "fallback path must backfill missing task snapshots"
    );
}
