//! `SQLite` integration tests for atomic dispatch cancellation.

use std::sync::Arc;

use chrono::Utc;
use tanren_domain::{
    ActorContext, AuthMode, Cli, ConfigKeys, DispatchId, DispatchMode, DispatchSnapshot,
    DispatchStatus, DomainEvent, EntityRef, EventEnvelope, EventId, GraphRevision, Lane,
    NonEmptyString, OrgId, Phase, StepId, StepPayload, StepReadyState, StepStatus, StepType,
    TimeoutSecs, UserId,
};
use tanren_store::{
    CancelDispatchParams, CreateDispatchParams, EnqueueStepParams, EventFilter, EventStore,
    JobQueue, StateStore, Store, StoreError, UpdateDispatchStatusParams,
};
use uuid::Uuid;

async fn fresh_store() -> Store {
    let store = Store::new("sqlite::memory:").await.expect("connect");
    store.run_migrations().await.expect("migrate");
    store
}

fn actor() -> ActorContext {
    ActorContext::new(OrgId::new(), UserId::new())
}

fn snapshot(project: &str) -> DispatchSnapshot {
    DispatchSnapshot {
        project: NonEmptyString::try_new(project.to_owned()).expect("project"),
        phase: Phase::DoTask,
        cli: Cli::Claude,
        auth_mode: AuthMode::ApiKey,
        branch: NonEmptyString::try_new("main".to_owned()).expect("branch"),
        spec_folder: NonEmptyString::try_new("spec".to_owned()).expect("spec"),
        workflow_id: NonEmptyString::try_new("wf-1".to_owned()).expect("workflow"),
        timeout: TimeoutSecs::try_new(60).expect("timeout"),
        environment_profile: NonEmptyString::try_new("default".to_owned()).expect("profile"),
        gate_cmd: None,
        context: None,
        model: None,
        project_env: ConfigKeys::default(),
        required_secrets: vec![],
        preserve_on_failure: false,
        created_at: Utc::now(),
    }
}

fn create_dispatch_params(
    project: &str,
    lane: Lane,
    actor_ctx: ActorContext,
) -> CreateDispatchParams {
    let dispatch_id = DispatchId::new();
    let dispatch = snapshot(project);
    let created_at = Utc::now();
    CreateDispatchParams {
        dispatch_id,
        mode: DispatchMode::Manual,
        lane,
        dispatch: dispatch.clone(),
        actor: actor_ctx.clone(),
        graph_revision: GraphRevision::INITIAL,
        created_at,
        creation_event: EventEnvelope::new(
            EventId::from_uuid(Uuid::now_v7()),
            created_at,
            DomainEvent::DispatchCreated {
                dispatch_id,
                dispatch: Box::new(dispatch),
                mode: DispatchMode::Manual,
                lane,
                actor: actor_ctx,
                graph_revision: GraphRevision::INITIAL,
            },
        ),
    }
}

async fn create_dispatch(store: &Store, project: &str, lane: Lane) -> DispatchId {
    let params = create_dispatch_params(project, lane, actor());
    let id = params.dispatch_id;
    store
        .create_dispatch_projection(params)
        .await
        .expect("create dispatch");
    id
}

fn enqueue_step_params(
    dispatch_id: DispatchId,
    step_id: StepId,
    step_type: StepType,
    sequence: u32,
    lane: Option<Lane>,
    payload: StepPayload,
) -> EnqueueStepParams {
    EnqueueStepParams {
        dispatch_id,
        step_id,
        step_type,
        step_sequence: sequence,
        lane,
        depends_on: vec![],
        graph_revision: GraphRevision::INITIAL,
        payload,
        ready_state: StepReadyState::Ready,
        enqueue_event: EventEnvelope::new(
            EventId::from_uuid(Uuid::now_v7()),
            Utc::now(),
            DomainEvent::StepEnqueued {
                dispatch_id,
                step_id,
                step_type,
                step_sequence: sequence,
                lane,
                depends_on: vec![],
                graph_revision: GraphRevision::INITIAL,
            },
        ),
    }
}

async fn seed_execute_steps(
    store: &Store,
    dispatch_id: DispatchId,
    dispatch: &DispatchSnapshot,
    count: u32,
) {
    for seq in 0..count {
        let step_id = StepId::new();
        store
            .enqueue_step(enqueue_step_params(
                dispatch_id,
                step_id,
                StepType::Execute,
                seq,
                Some(Lane::Impl),
                StepPayload::Execute(Box::new(tanren_domain::ExecutePayload {
                    dispatch: dispatch.clone(),
                    handle: tanren_domain::EnvironmentHandle {
                        id: NonEmptyString::try_new("h".to_owned()).expect("h"),
                        runtime_type: NonEmptyString::try_new("local".to_owned()).expect("r"),
                    },
                })),
            ))
            .await
            .expect("enqueue execute");
    }
}

fn cancel_dispatch_params(
    dispatch_id: DispatchId,
    actor_ctx: ActorContext,
    reason: Option<String>,
) -> CancelDispatchParams {
    let reason_for_event = reason.clone();
    CancelDispatchParams {
        dispatch_id,
        actor: actor_ctx.clone(),
        reason,
        status_event: EventEnvelope::new(
            EventId::from_uuid(Uuid::now_v7()),
            Utc::now(),
            DomainEvent::DispatchCancelled {
                dispatch_id,
                actor: actor_ctx,
                reason: reason_for_event,
            },
        ),
    }
}

fn running_status_params(dispatch_id: DispatchId) -> UpdateDispatchStatusParams {
    UpdateDispatchStatusParams {
        dispatch_id,
        status: DispatchStatus::Running,
        outcome: None,
        status_event: EventEnvelope::new(
            EventId::from_uuid(Uuid::now_v7()),
            Utc::now(),
            DomainEvent::DispatchStarted { dispatch_id },
        ),
    }
}

#[tokio::test]
async fn cancel_dispatch_is_atomic_for_steps_and_dispatch_status() {
    let store = fresh_store().await;
    let snap = snapshot("alpha");
    let id = create_dispatch(&store, "alpha", Lane::Impl).await;
    seed_execute_steps(&store, id, &snap, 3).await;

    let teardown_id = StepId::new();
    store
        .enqueue_step(enqueue_step_params(
            id,
            teardown_id,
            StepType::Teardown,
            10,
            Some(Lane::Impl),
            StepPayload::Teardown(Box::new(tanren_domain::TeardownPayload {
                dispatch: snap,
                handle: tanren_domain::EnvironmentHandle {
                    id: NonEmptyString::try_new("h".to_owned()).expect("h"),
                    runtime_type: NonEmptyString::try_new("local".to_owned()).expect("r"),
                },
                preserve: false,
            })),
        ))
        .await
        .expect("teardown enqueue");
    store
        .update_dispatch_status(running_status_params(id))
        .await
        .expect("set running");

    let cancelled = store
        .cancel_dispatch(cancel_dispatch_params(
            id,
            actor(),
            Some("integration-cancel".to_owned()),
        ))
        .await
        .expect("atomic cancel");
    assert_eq!(cancelled, 3);

    let dispatch = store.get_dispatch(&id).await.expect("get").expect("exists");
    assert_eq!(dispatch.status, DispatchStatus::Cancelled);

    let steps = store.get_steps_for_dispatch(&id).await.expect("steps");
    let cancelled_steps = steps
        .iter()
        .filter(|s| s.status == StepStatus::Cancelled)
        .count();
    let pending_steps = steps
        .iter()
        .filter(|s| s.status == StepStatus::Pending)
        .count();
    assert_eq!(cancelled_steps, 3);
    assert_eq!(pending_steps, 1, "teardown should stay pending");

    let step_cancelled = store
        .query_events(&EventFilter {
            entity_ref: Some(EntityRef::Dispatch(id)),
            event_type: Some("step_cancelled".to_owned()),
            limit: 50,
            ..EventFilter::new()
        })
        .await
        .expect("step events");
    assert_eq!(step_cancelled.total_count, 3);

    let dispatch_cancelled = store
        .query_events(&EventFilter {
            entity_ref: Some(EntityRef::Dispatch(id)),
            event_type: Some("dispatch_cancelled".to_owned()),
            limit: 10,
            ..EventFilter::new()
        })
        .await
        .expect("dispatch events");
    assert_eq!(dispatch_cancelled.total_count, 1);
}

#[tokio::test]
async fn cancel_dispatch_rolls_back_on_dispatch_event_insert_failure() {
    let store = fresh_store().await;
    let snap = snapshot("alpha");
    let id = create_dispatch(&store, "alpha", Lane::Impl).await;
    seed_execute_steps(&store, id, &snap, 2).await;

    let params = cancel_dispatch_params(id, actor(), Some("collision".to_owned()));
    store
        .append(&params.status_event)
        .await
        .expect("seed duplicate event id");

    let err = store
        .cancel_dispatch(params)
        .await
        .expect_err("cancel should fail");
    assert!(
        matches!(err, StoreError::Database(_)),
        "expected DB uniqueness error, got {err:?}"
    );

    let dispatch = store.get_dispatch(&id).await.expect("get").expect("exists");
    assert_eq!(
        dispatch.status,
        DispatchStatus::Pending,
        "dispatch status update must roll back"
    );

    let steps = store.get_steps_for_dispatch(&id).await.expect("steps");
    assert!(
        steps.iter().all(|s| s.status == StepStatus::Pending),
        "step updates must roll back"
    );

    let step_cancelled = store
        .query_events(&EventFilter {
            entity_ref: Some(EntityRef::Dispatch(id)),
            event_type: Some("step_cancelled".to_owned()),
            limit: 50,
            ..EventFilter::new()
        })
        .await
        .expect("events");
    assert_eq!(step_cancelled.total_count, 0);
}

#[tokio::test]
async fn cancel_dispatch_concurrent_calls_allow_only_one_winner() {
    let store = Arc::new(fresh_store().await);
    let snap = snapshot("alpha");
    let id = create_dispatch(&store, "alpha", Lane::Impl).await;
    seed_execute_steps(&store, id, &snap, 2).await;
    store
        .update_dispatch_status(running_status_params(id))
        .await
        .expect("running");

    let params_a = cancel_dispatch_params(id, actor(), Some("a".to_owned()));
    let params_b = cancel_dispatch_params(id, actor(), Some("b".to_owned()));

    let store_a = Arc::clone(&store);
    let store_b = Arc::clone(&store);
    let fut_a = async move { store_a.cancel_dispatch(params_a).await };
    let fut_b = async move { store_b.cancel_dispatch(params_b).await };
    let (res_a, res_b) = tokio::join!(fut_a, fut_b);
    let outcomes = [res_a, res_b];

    let success_count = outcomes.iter().filter(|r| r.is_ok()).count();
    assert_eq!(success_count, 1, "expected exactly one successful cancel");
    assert!(
        outcomes.iter().any(|r| {
            matches!(
                r,
                Err(StoreError::Conflict(_)
                    | StoreError::InvalidTransition { .. }
                    | StoreError::Database(_))
            )
        }),
        "losing call should fail with contention signal: {outcomes:?}",
    );

    let dispatch = store.get_dispatch(&id).await.expect("get").expect("exists");
    assert_eq!(dispatch.status, DispatchStatus::Cancelled);

    let dispatch_cancelled = store
        .query_events(&EventFilter {
            entity_ref: Some(EntityRef::Dispatch(id)),
            event_type: Some("dispatch_cancelled".to_owned()),
            limit: 10,
            ..EventFilter::new()
        })
        .await
        .expect("events");
    assert_eq!(dispatch_cancelled.total_count, 1);
}
