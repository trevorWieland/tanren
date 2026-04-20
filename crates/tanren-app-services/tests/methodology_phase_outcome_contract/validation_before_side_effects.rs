use std::sync::Arc;

use super::*;
use tanren_store::Store;

#[tokio::test]
async fn report_phase_outcome_rejects_missing_task_id_without_side_effects() {
    let spec_id = SpecId::new();
    let runtime = PhaseEventsRuntime {
        spec_id,
        spec_folder: std::env::temp_dir().join(format!(
            "tanren-phase-outcome-missing-task-id-{}",
            uuid::Uuid::now_v7()
        )),
        agent_session_id: "runtime-session-missing-task-id".into(),
    };
    let svc = mk_service(vec![RequiredGuard::Audited], runtime).await;
    let phase_scope = scope(&[ToolCapability::PhaseOutcome]);

    let err = svc
        .report_phase_outcome(
            &phase_scope,
            &phase("audit-task"),
            ReportPhaseOutcomeParams {
                schema_version: SchemaVersion::current(),
                spec_id,
                task_id: None,
                outcome: PhaseOutcome::Complete {
                    summary: NonEmptyString::try_new("audit complete").expect("summary"),
                    next_action_hint: None,
                },
                idempotency_key: Some("phase-outcome-missing-task-id".into()),
            },
        )
        .await
        .expect_err("task-scoped complete must require task_id");
    assert!(matches!(
        err,
        MethodologyError::FieldValidation { ref field_path, .. } if field_path == "/task_id"
    ));

    let events =
        tanren_store::methodology::projections::load_methodology_events(svc.store(), spec_id, 100)
            .await
            .expect("load events");
    assert!(
        events.is_empty(),
        "validation failures must not append methodology events"
    );
    let pending = svc
        .store()
        .load_pending_phase_event_outbox(Some(spec_id), 100)
        .await
        .expect("pending outbox");
    assert!(
        pending.is_empty(),
        "validation failures must not enqueue outbox rows"
    );
}

#[tokio::test]
async fn report_phase_outcome_rejects_cross_spec_task_without_side_effects() {
    let store = Arc::new(
        Store::open_and_migrate("sqlite::memory:?cache=shared")
            .await
            .expect("open"),
    );
    let spec_a = SpecId::new();
    let spec_b = SpecId::new();
    let svc_a = MethodologyService::with_runtime(
        Arc::clone(&store),
        vec![RequiredGuard::Audited],
        Some(PhaseEventsRuntime {
            spec_id: spec_a,
            spec_folder: std::env::temp_dir().join(format!(
                "tanren-phase-outcome-cross-spec-a-{}",
                uuid::Uuid::now_v7()
            )),
            agent_session_id: "runtime-session-a".into(),
        }),
        vec![],
    );
    let svc_b = MethodologyService::with_runtime(
        Arc::clone(&store),
        vec![RequiredGuard::Audited],
        Some(PhaseEventsRuntime {
            spec_id: spec_b,
            spec_folder: std::env::temp_dir().join(format!(
                "tanren-phase-outcome-cross-spec-b-{}",
                uuid::Uuid::now_v7()
            )),
            agent_session_id: "runtime-session-b".into(),
        }),
        vec![],
    );
    let task_id = create_implemented_task(&svc_b, spec_b, "do-task").await;
    let phase_scope = scope(&[ToolCapability::PhaseOutcome]);

    let err = svc_a
        .report_phase_outcome(
            &phase_scope,
            &phase("audit-task"),
            ReportPhaseOutcomeParams {
                schema_version: SchemaVersion::current(),
                spec_id: spec_a,
                task_id: Some(task_id),
                outcome: PhaseOutcome::Complete {
                    summary: NonEmptyString::try_new("audit complete").expect("summary"),
                    next_action_hint: None,
                },
                idempotency_key: Some("phase-outcome-cross-spec".into()),
            },
        )
        .await
        .expect_err("cross-spec task_id must fail validation");
    assert!(matches!(
        err,
        MethodologyError::FieldValidation { ref field_path, .. } if field_path == "/task_id"
    ));

    let events_a =
        tanren_store::methodology::projections::load_methodology_events(svc_a.store(), spec_a, 100)
            .await
            .expect("load spec-a events");
    assert!(
        events_a.is_empty(),
        "failed cross-spec validation must not append spec-a events"
    );
    let pending_a = svc_a
        .store()
        .load_pending_phase_event_outbox(Some(spec_a), 100)
        .await
        .expect("pending outbox");
    assert!(
        pending_a.is_empty(),
        "failed cross-spec validation must not enqueue spec-a outbox rows"
    );
}
