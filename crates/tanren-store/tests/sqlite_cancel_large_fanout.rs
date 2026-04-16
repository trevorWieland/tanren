//! `SQLite` large-fanout cancel test — regression for the lane 0.4 audit
//! finding #6 `UPDATE ... RETURNING` batch path.
//!
//! The cancel path must cancel every pending non-teardown step across
//! many batches (>= `CANCEL_BATCH_SIZE`), emit exactly one
//! `StepCancelled` event per cancelled row, leave teardown steps
//! untouched, and end with a single `DispatchCancelled` event.

use chrono::Utc;
use tanren_domain::{
    ActorContext, AuthMode, Cli, ConfigKeys, DispatchId, DispatchMode, DispatchSnapshot,
    DispatchStatus, DomainEvent, EntityRef, EventEnvelope, EventId, GraphRevision, Lane,
    NonEmptyString, OrgId, Phase, StepId, StepPayload, StepReadyState, StepStatus, StepType,
    TimeoutSecs, UserId,
};
use tanren_store::{
    CancelDispatchParams, CreateDispatchParams, EnqueueStepParams, EventFilter, EventStore,
    JobQueue, ReplayGuard, StateStore, Store, UpdateDispatchStatusParams,
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

async fn create_dispatch(store: &Store, project: &str) -> DispatchId {
    let dispatch_id = DispatchId::new();
    let snap = snapshot(project);
    let created_at = Utc::now();
    let actor_ctx = actor();

    store
        .create_dispatch_projection(CreateDispatchParams {
            dispatch_id,
            mode: DispatchMode::Manual,
            lane: Lane::Impl,
            dispatch: snap.clone(),
            actor: actor_ctx.clone(),
            graph_revision: GraphRevision::INITIAL,
            created_at,
            creation_event: EventEnvelope::new(
                EventId::from_uuid(Uuid::now_v7()),
                created_at,
                DomainEvent::DispatchCreated {
                    dispatch_id,
                    dispatch: Box::new(snap),
                    mode: DispatchMode::Manual,
                    lane: Lane::Impl,
                    actor: actor_ctx,
                    graph_revision: GraphRevision::INITIAL,
                },
            ),
        })
        .await
        .expect("create dispatch");
    dispatch_id
}

async fn seed_execute_steps(
    store: &Store,
    dispatch_id: DispatchId,
    snap: &DispatchSnapshot,
    count: u32,
) {
    for seq in 0..count {
        let step_id = StepId::new();
        store
            .enqueue_step(EnqueueStepParams {
                dispatch_id,
                step_id,
                step_type: StepType::Execute,
                step_sequence: seq,
                lane: Some(Lane::Impl),
                depends_on: vec![],
                graph_revision: GraphRevision::INITIAL,
                payload: StepPayload::Execute(Box::new(tanren_domain::ExecutePayload {
                    dispatch: snap.clone(),
                    handle: tanren_domain::EnvironmentHandle {
                        id: NonEmptyString::try_new("h".to_owned()).expect("h"),
                        runtime_type: NonEmptyString::try_new("local".to_owned()).expect("r"),
                    },
                })),
                ready_state: StepReadyState::Ready,
                enqueue_event: EventEnvelope::new(
                    EventId::from_uuid(Uuid::now_v7()),
                    Utc::now(),
                    DomainEvent::StepEnqueued {
                        dispatch_id,
                        step_id,
                        step_type: StepType::Execute,
                        step_sequence: seq,
                        lane: Some(Lane::Impl),
                        depends_on: vec![],
                        graph_revision: GraphRevision::INITIAL,
                    },
                ),
            })
            .await
            .expect("enqueue execute");
    }
}

async fn seed_teardown_step(
    store: &Store,
    dispatch_id: DispatchId,
    snap: &DispatchSnapshot,
    seq: u32,
) -> StepId {
    let step_id = StepId::new();
    store
        .enqueue_step(EnqueueStepParams {
            dispatch_id,
            step_id,
            step_type: StepType::Teardown,
            step_sequence: seq,
            lane: Some(Lane::Impl),
            depends_on: vec![],
            graph_revision: GraphRevision::INITIAL,
            payload: StepPayload::Teardown(Box::new(tanren_domain::TeardownPayload {
                dispatch: snap.clone(),
                handle: tanren_domain::EnvironmentHandle {
                    id: NonEmptyString::try_new("h".to_owned()).expect("h"),
                    runtime_type: NonEmptyString::try_new("local".to_owned()).expect("r"),
                },
                preserve: false,
            })),
            ready_state: StepReadyState::Ready,
            enqueue_event: EventEnvelope::new(
                EventId::from_uuid(Uuid::now_v7()),
                Utc::now(),
                DomainEvent::StepEnqueued {
                    dispatch_id,
                    step_id,
                    step_type: StepType::Teardown,
                    step_sequence: seq,
                    lane: Some(Lane::Impl),
                    depends_on: vec![],
                    graph_revision: GraphRevision::INITIAL,
                },
            ),
        })
        .await
        .expect("teardown enqueue");
    step_id
}

fn cancel_params(dispatch_id: DispatchId) -> CancelDispatchParams {
    let actor_ctx = actor();
    CancelDispatchParams {
        dispatch_id,
        actor: actor_ctx.clone(),
        reason: Some("large-fanout".to_owned()),
        status_event: EventEnvelope::new(
            EventId::from_uuid(Uuid::now_v7()),
            Utc::now(),
            DomainEvent::DispatchCancelled {
                dispatch_id,
                actor: actor_ctx,
                reason: Some("large-fanout".to_owned()),
            },
        ),
        replay_guard: ReplayGuard {
            issuer: "tanren-test".to_owned(),
            audience: "tanren-cli".to_owned(),
            jti: Uuid::now_v7().to_string(),
            iat_unix: 1,
            exp_unix: 2,
        },
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
async fn cancel_large_fanout_uses_batched_returning_path() {
    // 1_500 pending + 1 teardown: spans three CANCEL_BATCH_SIZE=500 batches
    // so the RETURNING loop is exercised through multiple round trips.
    const SEEDED_EXECUTE_STEPS: u32 = 1_500;

    let store = fresh_store().await;
    let snap = snapshot("large-fanout");
    let id = create_dispatch(&store, "large-fanout").await;
    seed_execute_steps(&store, id, &snap, SEEDED_EXECUTE_STEPS).await;
    let _teardown = seed_teardown_step(&store, id, &snap, SEEDED_EXECUTE_STEPS).await;

    store
        .update_dispatch_status(running_status_params(id))
        .await
        .expect("set running");

    let cancelled = store
        .cancel_dispatch(cancel_params(id))
        .await
        .expect("atomic cancel");
    assert_eq!(cancelled, u64::from(SEEDED_EXECUTE_STEPS));

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
    assert_eq!(cancelled_steps, SEEDED_EXECUTE_STEPS as usize);
    assert_eq!(pending_steps, 1, "teardown must remain pending");

    let step_cancelled = store
        .query_events(&EventFilter {
            entity_ref: Some(EntityRef::Dispatch(id)),
            event_type: Some("step_cancelled".to_owned()),
            limit: u64::from(SEEDED_EXECUTE_STEPS) + 10,
            include_total_count: true,
            ..EventFilter::new()
        })
        .await
        .expect("step events");
    assert_eq!(
        step_cancelled.total_count,
        Some(u64::from(SEEDED_EXECUTE_STEPS))
    );

    let dispatch_cancelled = store
        .query_events(&EventFilter {
            entity_ref: Some(EntityRef::Dispatch(id)),
            event_type: Some("dispatch_cancelled".to_owned()),
            limit: 10,
            include_total_count: true,
            ..EventFilter::new()
        })
        .await
        .expect("dispatch events");
    assert_eq!(dispatch_cancelled.total_count, Some(1));
}
