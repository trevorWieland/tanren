use std::sync::Arc;

use tanren_app_services::methodology::service::PhaseEventsRuntime;
use tanren_app_services::methodology::{CapabilityScope, MethodologyService, PhaseId};
use tanren_contract::methodology::{
    CompleteTaskParams, CreateTaskParams, ListTasksParams, ResetTaskGuardsParams, SchemaVersion,
    StartTaskParams,
};
use tanren_domain::methodology::capability::ToolCapability;
use tanren_domain::methodology::events::MethodologyEvent;
use tanren_domain::methodology::task::{RequiredGuard, TaskOrigin, TaskStatus};
use tanren_domain::{EntityRef, SpecId, TaskId};
use tanren_store::Store;
use tanren_store::methodology::projections::load_methodology_events_for_entity;

fn phase(tag: &str) -> PhaseId {
    PhaseId::try_new(tag).expect("phase")
}

fn scope(caps: &[ToolCapability]) -> CapabilityScope {
    CapabilityScope::from_iter_caps(caps.iter().copied())
}

fn required_guards() -> Vec<RequiredGuard> {
    vec![
        RequiredGuard::GateChecked,
        RequiredGuard::Audited,
        RequiredGuard::Adherent,
    ]
}

async fn create_implemented_task(service: &MethodologyService, spec_id: SpecId) -> TaskId {
    let task_id = service
        .create_task(
            &scope(&[ToolCapability::TaskCreate]),
            &phase("shape-spec"),
            CreateTaskParams {
                schema_version: SchemaVersion::current(),
                spec_id,
                title: "task".into(),
                description: "desc".into(),
                parent_task_id: None,
                depends_on: vec![],
                origin: TaskOrigin::User,
                acceptance_criteria: vec![],
                idempotency_key: Some("reset-guards-create".into()),
            },
        )
        .await
        .expect("create task")
        .task_id;

    service
        .start_task(
            &scope(&[ToolCapability::TaskStart, ToolCapability::TaskComplete]),
            &phase("do-task"),
            StartTaskParams {
                schema_version: SchemaVersion::current(),
                task_id,
                idempotency_key: Some("reset-guards-start".into()),
            },
        )
        .await
        .expect("start task");

    service
        .complete_task(
            &scope(&[ToolCapability::TaskStart, ToolCapability::TaskComplete]),
            &phase("do-task"),
            CompleteTaskParams {
                schema_version: SchemaVersion::current(),
                task_id,
                evidence_refs: vec![],
                idempotency_key: Some("reset-guards-complete".into()),
            },
        )
        .await
        .expect("complete task");

    service
        .mark_task_guard_satisfied(
            &scope(&[ToolCapability::TaskComplete]),
            &phase("do-task"),
            task_id,
            RequiredGuard::GateChecked,
            Some("reset-guards-mark-gate".into()),
        )
        .await
        .expect("mark gate");

    task_id
}

async fn load_task_status(
    service: &MethodologyService,
    spec_id: SpecId,
    task_id: TaskId,
) -> TaskStatus {
    service
        .list_tasks(
            &scope(&[ToolCapability::TaskRead]),
            &phase("do-task"),
            ListTasksParams {
                schema_version: SchemaVersion::current(),
                spec_id: Some(spec_id),
            },
        )
        .await
        .expect("list tasks")
        .tasks
        .into_iter()
        .find(|task| task.id == task_id)
        .expect("task present")
        .status
}

fn assert_guards_cleared(status: &TaskStatus, stage: &str) {
    assert!(
        matches!(status, TaskStatus::Implemented { .. }),
        "task should remain implemented {stage}"
    );

    if let TaskStatus::Implemented { guards } = status {
        assert!(!guards.gate_checked);
        assert!(!guards.audited);
        assert!(!guards.adherent);
        assert!(guards.extra.is_empty());
    }
}

async fn assert_reset_event_exists(store: &Store, spec_id: SpecId, task_id: TaskId) {
    let task_events =
        load_methodology_events_for_entity(store, EntityRef::Task(task_id), Some(spec_id), 1_000)
            .await
            .expect("load task events");

    assert!(
        task_events.iter().any(
            |event| matches!(event, MethodologyEvent::TaskGuardsReset(ev) if ev.task_id == task_id)
        ),
        "task event stream should contain TaskGuardsReset"
    );
}

#[tokio::test]
async fn reset_task_guards_clears_projection_and_replays_from_event_log() {
    let store = Arc::new(
        Store::open_and_migrate("sqlite::memory:?cache=shared")
            .await
            .expect("open store"),
    );
    let spec_id = SpecId::new();
    let required = required_guards();
    let runtime = PhaseEventsRuntime {
        spec_id,
        spec_folder: std::env::temp_dir()
            .join(format!("tanren-reset-guards-{}", uuid::Uuid::now_v7())),
        agent_session_id: "reset-guards-session".into(),
    };
    std::fs::create_dir_all(&runtime.spec_folder).expect("mkdir spec folder");
    let service =
        MethodologyService::with_runtime(store.clone(), required.clone(), Some(runtime), vec![]);

    let task_id = create_implemented_task(&service, spec_id).await;

    service
        .reset_task_guards_with_params(
            &scope(&[ToolCapability::TaskComplete]),
            &phase("investigate"),
            ResetTaskGuardsParams {
                schema_version: SchemaVersion::current(),
                task_id,
                reason: "batch failure, rerun checks".into(),
                idempotency_key: Some("reset-guards-event".into()),
            },
        )
        .await
        .expect("reset guards");

    let projected = load_task_status(&service, spec_id, task_id).await;
    assert_guards_cleared(&projected, "after reset");
    assert_reset_event_exists(&store, spec_id, task_id).await;

    let replay_service = MethodologyService::with_runtime(store, required, None, vec![]);
    let replayed = load_task_status(&replay_service, spec_id, task_id).await;
    assert_guards_cleared(&replayed, "after replay");
}
