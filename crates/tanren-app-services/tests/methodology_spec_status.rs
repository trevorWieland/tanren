use std::sync::Arc;

use tanren_app_services::methodology::service::PhaseEventsRuntime;
use tanren_app_services::methodology::{CapabilityScope, MethodologyService, PhaseId};
use tanren_contract::methodology::{
    CompleteTaskParams, CreateTaskParams, MarkTaskGuardSatisfiedParams, ReportPhaseOutcomeParams,
    SchemaVersion, SpecStatusNextAction, SpecStatusNextStep, SpecStatusParams, SpecStatusResponse,
};
use tanren_domain::methodology::capability::ToolCapability;
use tanren_domain::methodology::phase_outcome::{BlockedReason, PhaseOutcome};
use tanren_domain::methodology::task::{RequiredGuard, TaskOrigin};
use tanren_domain::{NonEmptyString, SpecId, TaskId};
use tanren_store::Store;

fn phase(tag: &str) -> PhaseId {
    PhaseId::try_new(tag).expect("phase")
}

fn scope(caps: &[ToolCapability]) -> CapabilityScope {
    CapabilityScope::from_iter_caps(caps.iter().copied())
}

async fn mk_service(required: Vec<RequiredGuard>, spec_id: SpecId) -> MethodologyService {
    let store = Store::open_and_migrate("sqlite::memory:?cache=shared")
        .await
        .expect("open");
    let runtime = PhaseEventsRuntime {
        spec_id,
        spec_folder: std::env::temp_dir()
            .join(format!("tanren-spec-status-{}", uuid::Uuid::now_v7())),
        agent_session_id: "spec-status-session".into(),
    };
    MethodologyService::with_runtime(Arc::new(store), required, Some(runtime), vec![])
}

async fn spec_status(svc: &MethodologyService, spec_id: SpecId) -> SpecStatusResponse {
    svc.spec_status(
        &scope(&[ToolCapability::TaskRead]),
        &phase("do-task"),
        SpecStatusParams {
            schema_version: SchemaVersion::current(),
            spec_id,
        },
    )
    .await
    .expect("spec status")
}

async fn implement_task(svc: &MethodologyService, task_id: TaskId) {
    let task_scope = scope(&[ToolCapability::TaskStart, ToolCapability::TaskComplete]);
    svc.start_task(
        &task_scope,
        &phase("do-task"),
        tanren_contract::methodology::StartTaskParams {
            schema_version: SchemaVersion::current(),
            task_id,
            idempotency_key: None,
        },
    )
    .await
    .expect("start");
    svc.complete_task(
        &task_scope,
        &phase("do-task"),
        CompleteTaskParams {
            schema_version: SchemaVersion::current(),
            task_id,
            evidence_refs: vec![],
            idempotency_key: None,
        },
    )
    .await
    .expect("complete");
}

#[tokio::test]
async fn spec_status_emits_step_hints_for_task_guard_progression() {
    let spec_id = SpecId::new();
    let svc = mk_service(
        vec![
            RequiredGuard::GateChecked,
            RequiredGuard::Audited,
            RequiredGuard::Adherent,
        ],
        spec_id,
    )
    .await;
    let task_scope = scope(&[ToolCapability::TaskCreate]);
    let task_id = svc
        .create_task(
            &task_scope,
            &phase("shape-spec"),
            CreateTaskParams {
                schema_version: SchemaVersion::current(),
                spec_id,
                title: "Task 1".into(),
                description: String::new(),
                parent_task_id: None,
                depends_on: vec![],
                origin: TaskOrigin::User,
                acceptance_criteria: vec![],
                idempotency_key: None,
            },
        )
        .await
        .expect("create")
        .task_id;
    let pending = spec_status(&svc, spec_id).await;
    assert_eq!(pending.next_action, SpecStatusNextAction::RunLoop);
    assert_eq!(pending.next_task_id, Some(task_id));
    assert_eq!(pending.next_step, Some(SpecStatusNextStep::TaskDoTask));

    implement_task(&svc, task_id).await;

    let implemented = spec_status(&svc, spec_id).await;
    assert_eq!(implemented.next_step, Some(SpecStatusNextStep::TaskGate));
    assert_eq!(
        implemented.pending_required_guards,
        vec![
            "gate_checked".to_owned(),
            "audited".to_owned(),
            "adherent".to_owned()
        ]
    );

    svc.mark_task_guard_satisfied_with_params(
        &scope(&[ToolCapability::TaskComplete]),
        &phase("do-task"),
        MarkTaskGuardSatisfiedParams {
            schema_version: SchemaVersion::current(),
            task_id,
            guard: RequiredGuard::GateChecked,
            idempotency_key: None,
        },
    )
    .await
    .expect("mark gate");
    let after_gate = svc
        .spec_status(
            &scope(&[ToolCapability::TaskRead]),
            &phase("do-task"),
            SpecStatusParams {
                schema_version: SchemaVersion::current(),
                spec_id,
            },
        )
        .await
        .expect("spec status after gate");
    assert_eq!(after_gate.next_step, Some(SpecStatusNextStep::TaskAudit));
    assert_eq!(
        after_gate.pending_required_guards,
        vec!["audited".to_owned(), "adherent".to_owned()]
    );
}

#[tokio::test]
async fn spec_status_uses_spec_pipeline_when_no_open_task() {
    let spec_id = SpecId::new();
    let svc = mk_service(vec![RequiredGuard::GateChecked], spec_id).await;
    svc.report_phase_outcome(
        &scope(&[ToolCapability::PhaseOutcome]),
        &phase("run-demo"),
        ReportPhaseOutcomeParams {
            schema_version: SchemaVersion::current(),
            spec_id,
            task_id: None,
            outcome: PhaseOutcome::Blocked {
                reason: BlockedReason::Other {
                    detail: NonEmptyString::try_new("blocked for test").expect("detail"),
                },
                summary: NonEmptyString::try_new("blocked").expect("summary"),
            },
            idempotency_key: None,
        },
    )
    .await
    .expect("phase outcome");
    svc.report_phase_outcome(
        &scope(&[ToolCapability::PhaseOutcome]),
        &phase("resolve-blockers"),
        ReportPhaseOutcomeParams {
            schema_version: SchemaVersion::current(),
            spec_id,
            task_id: None,
            outcome: PhaseOutcome::Complete {
                summary: NonEmptyString::try_new("resolved").expect("summary"),
                next_action_hint: None,
            },
            idempotency_key: None,
        },
    )
    .await
    .expect("resolve blockers");

    let status = spec_status(&svc, spec_id).await;
    assert_eq!(status.next_action, SpecStatusNextAction::RunLoop);
    assert_eq!(status.next_task_id, None);
    assert_eq!(status.next_step, Some(SpecStatusNextStep::SpecPipeline));
    assert!(status.pending_required_guards.is_empty());
}
