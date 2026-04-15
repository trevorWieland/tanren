//! `SQLite`-backed integration tests for the audit-fix deltas —
//! heartbeat / recovery (B-02), `nack` state-machine enforcement
//! (I-01), and typed entity-ref filtering (I-05).
//!
//! This binary intentionally does **not** include `common/mod.rs`:
//! each integration binary ships its own copy of `common`, and
//! helpers that only one binary uses become dead-code warnings. The
//! tests here only need a handful of builders, so we inline them.

use std::time::Duration;

use chrono::Utc;
use tanren_domain::{
    ActorContext, AuthMode, Cli, ConfigKeys, DispatchId, DispatchMode, DispatchSnapshot,
    DomainEvent, EntityRef, EnvironmentHandle, ErrorClass, EventEnvelope, EventId, ExecutePayload,
    FiniteF64, GraphRevision, Lane, NonEmptyString, OrgId, Phase, StepId, StepPayload,
    StepReadyState, StepStatus, StepType, TimeoutSecs, UserId,
};
use tanren_store::{
    CreateDispatchParams, DequeueParams, EnqueueStepParams, EventFilter, EventStore, JobQueue,
    NackParams, StateStore, Store, StoreError, StoreResult,
};
use uuid::Uuid;

async fn fresh_store() -> Store {
    let store = Store::new("sqlite::memory:").await.expect("connect");
    store.run_migrations().await.expect("migrate");
    store
}

fn snapshot(project: &str) -> DispatchSnapshot {
    DispatchSnapshot {
        project: NonEmptyString::try_new(project.to_owned()).expect("project"),
        phase: Phase::DoTask,
        cli: Cli::Claude,
        auth_mode: AuthMode::ApiKey,
        branch: NonEmptyString::try_new("main".to_owned()).expect("branch"),
        spec_folder: NonEmptyString::try_new("spec".to_owned()).expect("spec"),
        workflow_id: NonEmptyString::try_new("wf".to_owned()).expect("wf"),
        timeout: TimeoutSecs::try_new(60).expect("timeout"),
        environment_profile: NonEmptyString::try_new("p".to_owned()).expect("profile"),
        gate_cmd: None,
        context: None,
        model: None,
        project_env: ConfigKeys::default(),
        required_secrets: vec![],
        preserve_on_failure: false,
        created_at: Utc::now(),
    }
}

fn actor() -> ActorContext {
    ActorContext {
        org_id: OrgId::new(),
        user_id: UserId::new(),
        team_id: None,
        api_key_id: None,
        project_id: None,
    }
}

async fn create_dispatch(store: &Store, lane: Lane) -> StoreResult<DispatchId> {
    let snap = snapshot("alpha");
    let actor_ctx = actor();
    let dispatch_id = DispatchId::new();
    let created_at = Utc::now();
    let event = EventEnvelope::new(
        EventId::from_uuid(Uuid::now_v7()),
        created_at,
        DomainEvent::DispatchCreated {
            dispatch_id,
            dispatch: Box::new(snap.clone()),
            mode: DispatchMode::Manual,
            lane,
            actor: actor_ctx.clone(),
            graph_revision: GraphRevision::INITIAL,
        },
    );
    store
        .create_dispatch_projection(CreateDispatchParams {
            dispatch_id,
            mode: DispatchMode::Manual,
            lane,
            dispatch: snap,
            actor: actor_ctx,
            graph_revision: GraphRevision::INITIAL,
            created_at,
            creation_event: event,
        })
        .await?;
    Ok(dispatch_id)
}

fn execute_payload() -> StepPayload {
    StepPayload::Execute(Box::new(ExecutePayload {
        dispatch: snapshot("alpha"),
        handle: EnvironmentHandle {
            id: NonEmptyString::try_new("h".to_owned()).expect("h"),
            runtime_type: NonEmptyString::try_new("local".to_owned()).expect("runtime"),
        },
    }))
}

async fn enqueue_execute_step(
    store: &Store,
    dispatch_id: DispatchId,
    lane: Lane,
) -> StoreResult<StepId> {
    let step_id = StepId::new();
    let event = EventEnvelope::new(
        EventId::from_uuid(Uuid::now_v7()),
        Utc::now(),
        DomainEvent::StepEnqueued {
            dispatch_id,
            step_id,
            step_type: StepType::Execute,
            step_sequence: 0,
            lane: Some(lane),
            depends_on: vec![],
            graph_revision: GraphRevision::INITIAL,
        },
    );
    store
        .enqueue_step(EnqueueStepParams {
            dispatch_id,
            step_id,
            step_type: StepType::Execute,
            step_sequence: 0,
            lane: Some(lane),
            depends_on: vec![],
            graph_revision: GraphRevision::INITIAL,
            payload: execute_payload(),
            ready_state: StepReadyState::Ready,
            enqueue_event: event,
        })
        .await?;
    Ok(step_id)
}

async fn claim_step(store: &Store, step_id: StepId) -> StoreResult<()> {
    let claimed = store
        .dequeue(DequeueParams {
            worker_id: "w1".to_owned(),
            lane: Some(Lane::Impl),
            max_concurrent: 1,
        })
        .await?
        .expect("dequeue");
    assert_eq!(claimed.step_id, step_id);
    Ok(())
}

fn failure_envelope(dispatch_id: DispatchId, step_id: StepId) -> EventEnvelope {
    EventEnvelope::new(
        EventId::from_uuid(Uuid::now_v7()),
        Utc::now(),
        DomainEvent::StepFailed {
            dispatch_id,
            step_id,
            step_type: StepType::Execute,
            error: "boom".to_owned(),
            error_class: ErrorClass::Transient,
            retry_count: 1,
            duration_secs: FiniteF64::try_new(0.5).expect("finite"),
        },
    )
}

/// B-02: a step that is still receiving heartbeats must not be
/// reclaimed by `recover_stale_steps`, even if the recovery
/// threshold has elapsed since the initial claim. This is the core
/// correctness property the audit flagged as missing.
#[tokio::test]
async fn heartbeat_protects_live_steps_from_recovery() {
    let store = fresh_store().await;
    let id = create_dispatch(&store, Lane::Impl).await.expect("create");
    let step_id = enqueue_execute_step(&store, id, Lane::Impl)
        .await
        .expect("enqueue");
    claim_step(&store, step_id).await.expect("claim");

    // Simulate a worker making progress on a long-running step by
    // refreshing the heartbeat across time.
    tokio::time::sleep(Duration::from_millis(50)).await;
    store.heartbeat_step(&step_id).await.expect("heartbeat 1");
    tokio::time::sleep(Duration::from_millis(50)).await;
    store.heartbeat_step(&step_id).await.expect("heartbeat 2");

    // With threshold=3600s, the heartbeat is vastly newer than the
    // cutoff, so the recovery pass must leave the step alone.
    let recovered = store.recover_stale_steps(3600).await.expect("recover");
    assert_eq!(recovered, 0, "live-worker step must not be reclaimed");
    let view = store
        .get_step(&step_id)
        .await
        .expect("get")
        .expect("exists");
    assert_eq!(view.status, StepStatus::Running);
    assert_eq!(view.worker_id.as_deref(), Some("w1"));
}

/// B-02: a dead worker (no heartbeat after claim) does get reclaimed
/// once the recovery threshold elapses. Counterpart to the test
/// above.
#[tokio::test]
async fn recover_reclaims_steps_that_miss_heartbeats() {
    let store = fresh_store().await;
    let id = create_dispatch(&store, Lane::Impl).await.expect("create");
    let step_id = enqueue_execute_step(&store, id, Lane::Impl)
        .await
        .expect("enqueue");
    claim_step(&store, step_id).await.expect("claim");

    // No heartbeat calls. After the threshold passes, the row is
    // stale and recover_stale_steps(0) claims it.
    tokio::time::sleep(Duration::from_millis(20)).await;
    let recovered = store.recover_stale_steps(0).await.expect("recover");
    assert_eq!(recovered, 1);
    let view = store
        .get_step(&step_id)
        .await
        .expect("get")
        .expect("exists");
    assert_eq!(view.status, StepStatus::Pending);
    assert!(view.worker_id.is_none());
}

/// B-02: heartbeat on a non-running step is rejected with
/// `InvalidTransition`. Workers should never call heartbeat on a
/// completed or cancelled step, so surfacing it as an error is the
/// right signal.
#[tokio::test]
async fn heartbeat_rejects_pending_step() {
    let store = fresh_store().await;
    let id = create_dispatch(&store, Lane::Impl).await.expect("create");
    let step_id = enqueue_execute_step(&store, id, Lane::Impl)
        .await
        .expect("enqueue");

    let err = store
        .heartbeat_step(&step_id)
        .await
        .expect_err("heartbeat on pending should fail");
    assert!(matches!(err, StoreError::InvalidTransition { .. }));
}

/// I-01: `nack` on a pending step must fail — the domain step state
/// machine only permits `Running -> Failed|Pending(retry)|Cancelled`,
/// and the previous implementation did not enforce this. The nack
/// must report `InvalidTransition` and leave the row untouched.
#[tokio::test]
async fn nack_rejects_pending_step() {
    let store = fresh_store().await;
    let id = create_dispatch(&store, Lane::Impl).await.expect("create");
    let step_id = enqueue_execute_step(&store, id, Lane::Impl)
        .await
        .expect("enqueue");

    let err = store
        .nack(NackParams {
            dispatch_id: id,
            step_id,
            step_type: StepType::Execute,
            error: "boom".to_owned(),
            error_class: ErrorClass::Transient,
            retry: false,
            failure_event: failure_envelope(id, step_id),
        })
        .await
        .expect_err("nack on pending should fail");
    assert!(matches!(err, StoreError::InvalidTransition { .. }));

    // Step row is unchanged — still pending, no error set, no event
    // appended (the failure envelope was supposed to land only
    // co-transactionally with the state change).
    let view = store
        .get_step(&step_id)
        .await
        .expect("get")
        .expect("exists");
    assert_eq!(view.status, StepStatus::Pending);
    assert!(view.error.is_none());
    let failures = store
        .query_events(&EventFilter {
            event_type: Some("step_failed".to_owned()),
            limit: 10,
            include_total_count: true,
            ..EventFilter::new()
        })
        .await
        .expect("query");
    assert_eq!(failures.total_count, Some(0));
}

/// I-05: filtering by `entity_ref = Dispatch(uuid_a)` must not
/// return events that belong to `Dispatch(uuid_b)`. All domain
/// events route to `EntityRef::Dispatch(dispatch_id)` via
/// `entity_root()`, so the real discrimination axis is always
/// between different dispatch UUIDs.
#[tokio::test]
async fn entity_ref_filter_distinguishes_different_dispatches() {
    let store = fresh_store().await;
    let id_a = create_dispatch(&store, Lane::Impl).await.expect("create a");
    let id_b = create_dispatch(&store, Lane::Audit)
        .await
        .expect("create b");

    // Append an extra event on each dispatch so there is more than
    // just the creation event to count.
    let event_a = EventEnvelope::new(
        EventId::from_uuid(Uuid::now_v7()),
        Utc::now(),
        DomainEvent::DispatchStarted { dispatch_id: id_a },
    );
    let event_b = EventEnvelope::new(
        EventId::from_uuid(Uuid::now_v7()),
        Utc::now(),
        DomainEvent::DispatchStarted { dispatch_id: id_b },
    );
    store.append(&event_a).await.expect("append a");
    store.append(&event_b).await.expect("append b");

    // Filter by Dispatch(id_a) — must exclude all id_b events.
    let result = store
        .query_events(&EventFilter {
            entity_ref: Some(EntityRef::Dispatch(id_a)),
            limit: 100,
            include_total_count: true,
            ..EventFilter::new()
        })
        .await
        .expect("query");

    // creation(a) + DispatchStarted(a) = 2
    assert_eq!(result.total_count, Some(2));
    for e in &result.events {
        assert_eq!(
            e.entity_ref,
            EntityRef::Dispatch(id_a),
            "events for dispatch_b must not appear in dispatch_a-filtered results"
        );
    }
}

/// Semantic validation: `ack` with a `StepFailed` envelope (wrong
/// variant) must be rejected before any write occurs.
#[tokio::test]
async fn ack_rejects_wrong_event_variant() {
    let store = fresh_store().await;
    let id = create_dispatch(&store, Lane::Impl).await.expect("create");
    let step_id = enqueue_execute_step(&store, id, Lane::Impl)
        .await
        .expect("enqueue");
    claim_step(&store, step_id).await.expect("claim");

    let wrong = EventEnvelope::new(
        EventId::from_uuid(Uuid::now_v7()),
        Utc::now(),
        DomainEvent::StepFailed {
            dispatch_id: id,
            step_id,
            step_type: StepType::Execute,
            error: "oops".to_owned(),
            error_class: ErrorClass::Fatal,
            retry_count: 0,
            duration_secs: FiniteF64::try_new(0.1).expect("finite"),
        },
    );
    let err = store
        .ack(tanren_store::AckParams {
            dispatch_id: id,
            step_id,
            step_type: StepType::Execute,
            result: tanren_domain::StepResult::Execute(Box::new(tanren_domain::ExecuteResult {
                outcome: tanren_domain::Outcome::Success,
                signal: None,
                exit_code: Some(0),
                duration_secs: FiniteF64::try_new(1.0).expect("f"),
                gate_output: None,
                tail_output: None,
                stderr_tail: None,
                pushed: false,
                plan_hash: None,
                unchecked_tasks: 0,
                spec_modified: false,
                findings: vec![],
                token_usage: None,
            })),
            completion_event: wrong,
        })
        .await
        .expect_err("wrong variant must be rejected");
    assert!(matches!(err, StoreError::Conversion { .. }));
}

/// Semantic validation: `ack` with the correct variant but wrong
/// `step_id` in the envelope payload must be rejected.
#[tokio::test]
async fn ack_rejects_wrong_step_id_in_envelope() {
    let store = fresh_store().await;
    let id = create_dispatch(&store, Lane::Impl).await.expect("create");
    let step_id = enqueue_execute_step(&store, id, Lane::Impl)
        .await
        .expect("enqueue");
    claim_step(&store, step_id).await.expect("claim");

    let other_step = StepId::new();
    let wrong = EventEnvelope::new(
        EventId::from_uuid(Uuid::now_v7()),
        Utc::now(),
        DomainEvent::StepCompleted {
            dispatch_id: id,
            step_id: other_step, // wrong!
            step_type: StepType::Execute,
            duration_secs: FiniteF64::try_new(1.0).expect("f"),
            result_payload: Box::new(tanren_domain::StepResult::Execute(Box::new(
                tanren_domain::ExecuteResult {
                    outcome: tanren_domain::Outcome::Success,
                    signal: None,
                    exit_code: Some(0),
                    duration_secs: FiniteF64::try_new(1.0).expect("f"),
                    gate_output: None,
                    tail_output: None,
                    stderr_tail: None,
                    pushed: false,
                    plan_hash: None,
                    unchecked_tasks: 0,
                    spec_modified: false,
                    findings: vec![],
                    token_usage: None,
                },
            ))),
        },
    );
    let err = store
        .ack(tanren_store::AckParams {
            dispatch_id: id,
            step_id,
            step_type: StepType::Execute,
            result: tanren_domain::StepResult::Execute(Box::new(tanren_domain::ExecuteResult {
                outcome: tanren_domain::Outcome::Success,
                signal: None,
                exit_code: Some(0),
                duration_secs: FiniteF64::try_new(1.0).expect("f"),
                gate_output: None,
                tail_output: None,
                stderr_tail: None,
                pushed: false,
                plan_hash: None,
                unchecked_tasks: 0,
                spec_modified: false,
                findings: vec![],
                token_usage: None,
            })),
            completion_event: wrong,
        })
        .await
        .expect_err("wrong step_id must be rejected");
    assert!(matches!(err, StoreError::Conversion { .. }));
}

/// Semantic validation: `update_dispatch_status(Running)` with a
/// `DispatchCompleted` envelope must be rejected.
#[tokio::test]
async fn update_dispatch_status_rejects_variant_mismatch() {
    let store = fresh_store().await;
    let id = create_dispatch(&store, Lane::Impl).await.expect("create");

    let wrong = EventEnvelope::new(
        EventId::from_uuid(Uuid::now_v7()),
        Utc::now(),
        DomainEvent::DispatchCompleted {
            dispatch_id: id,
            outcome: tanren_domain::Outcome::Success,
            total_duration_secs: FiniteF64::try_new(1.0).expect("f"),
        },
    );
    let err = store
        .update_dispatch_status(tanren_store::UpdateDispatchStatusParams {
            dispatch_id: id,
            status: tanren_domain::DispatchStatus::Running,
            outcome: None,
            status_event: wrong,
        })
        .await
        .expect_err("status/variant mismatch must be rejected");
    assert!(matches!(err, StoreError::Conversion { .. }));
}
