//! Regression tests for audit findings B-01, I-01, I-02, and S-02.
//!
//! Self-contained helpers (no `mod common;`) to avoid dead-code
//! warnings from shared helpers this binary doesn't exercise.

use chrono::Utc;
use tanren_domain::{
    ActorContext, AuthMode, Cli, ConfigKeys, DispatchId, DispatchMode, DomainEvent,
    EnvironmentHandle, ErrorClass, EventEnvelope, EventId, ExecutePayload, ExecuteResult,
    FiniteF64, GraphRevision, Lane, NonEmptyString, OrgId, Outcome, Phase, StepId, StepPayload,
    StepReadyState, StepResult, StepStatus, StepType, TimeoutSecs, UserId,
};
use tanren_store::{
    AckAndEnqueueParams, AckParams, CreateDispatchParams, DequeueParams, EnqueueStepParams,
    JobQueue, NackParams, QueuedStep, StateStore, Store, StoreError, StoreResult,
};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Minimal fixture helpers
// ---------------------------------------------------------------------------

async fn fresh_store() -> Store {
    let store = Store::new("sqlite::memory:").await.expect("connect");
    store.run_migrations().await.expect("migrate");
    store
}

fn snap() -> tanren_domain::DispatchSnapshot {
    tanren_domain::DispatchSnapshot {
        project: NonEmptyString::try_new("alpha".to_owned()).expect("p"),
        phase: Phase::DoTask,
        cli: Cli::Claude,
        auth_mode: AuthMode::ApiKey,
        branch: NonEmptyString::try_new("main".to_owned()).expect("b"),
        spec_folder: NonEmptyString::try_new("spec".to_owned()).expect("s"),
        workflow_id: NonEmptyString::try_new("wf-1".to_owned()).expect("w"),
        timeout: TimeoutSecs::try_new(60).expect("t"),
        environment_profile: NonEmptyString::try_new("default".to_owned()).expect("e"),
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

fn handle() -> EnvironmentHandle {
    EnvironmentHandle {
        id: NonEmptyString::try_new("h-1".to_owned()).expect("h"),
        runtime_type: NonEmptyString::try_new("local".to_owned()).expect("r"),
    }
}

fn exec_payload() -> StepPayload {
    StepPayload::Execute(Box::new(ExecutePayload {
        dispatch: snap(),
        handle: handle(),
    }))
}

fn exec_result() -> StepResult {
    StepResult::Execute(Box::new(ExecuteResult {
        outcome: Outcome::Success,
        signal: None,
        exit_code: Some(0),
        duration_secs: FiniteF64::try_new(1.5).expect("f"),
        gate_output: None,
        tail_output: None,
        stderr_tail: None,
        pushed: false,
        plan_hash: None,
        unchecked_tasks: 0,
        spec_modified: false,
        findings: vec![],
        token_usage: None,
    }))
}

async fn create_dispatch(store: &Store, _project: &str, lane: Lane) -> StoreResult<DispatchId> {
    let id = DispatchId::new();
    let s = snap();
    let act = actor();
    let now = Utc::now();
    let event = EventEnvelope::new(
        EventId::from_uuid(Uuid::now_v7()),
        now,
        DomainEvent::DispatchCreated {
            dispatch_id: id,
            dispatch: Box::new(s.clone()),
            mode: DispatchMode::Manual,
            lane,
            actor: act.clone(),
            graph_revision: GraphRevision::INITIAL,
        },
    );
    store
        .create_dispatch_projection(CreateDispatchParams {
            dispatch_id: id,
            mode: DispatchMode::Manual,
            lane,
            dispatch: s,
            actor: act,
            graph_revision: GraphRevision::INITIAL,
            created_at: now,
            creation_event: event,
        })
        .await?;
    Ok(id)
}

fn enqueue_params(
    dispatch_id: DispatchId,
    step_id: StepId,
    seq: u32,
    lane: Option<Lane>,
) -> EnqueueStepParams {
    let event = EventEnvelope::new(
        EventId::from_uuid(Uuid::now_v7()),
        Utc::now(),
        DomainEvent::StepEnqueued {
            dispatch_id,
            step_id,
            step_type: StepType::Execute,
            step_sequence: seq,
            lane,
            depends_on: vec![],
            graph_revision: GraphRevision::INITIAL,
        },
    );
    EnqueueStepParams {
        dispatch_id,
        step_id,
        step_type: StepType::Execute,
        step_sequence: seq,
        lane,
        depends_on: vec![],
        graph_revision: GraphRevision::INITIAL,
        payload: exec_payload(),
        ready_state: StepReadyState::Ready,
        enqueue_event: event,
    }
}

async fn dequeue(store: &Store, lane: Option<Lane>) -> StoreResult<Option<QueuedStep>> {
    store
        .dequeue(DequeueParams {
            worker_id: "w1".to_owned(),
            lane,
            max_concurrent: 10,
        })
        .await
}

fn completed_event(dispatch_id: DispatchId, step_id: StepId, result: &StepResult) -> EventEnvelope {
    EventEnvelope::new(
        EventId::from_uuid(Uuid::now_v7()),
        Utc::now(),
        DomainEvent::StepCompleted {
            dispatch_id,
            step_id,
            step_type: StepType::Execute,
            duration_secs: FiniteF64::try_new(1.0).expect("f"),
            result_payload: Box::new(result.clone()),
        },
    )
}

fn nack(dispatch_id: DispatchId, step_id: StepId, error: &str, retry: bool) -> NackParams {
    let failure_event = EventEnvelope::new(
        EventId::from_uuid(Uuid::now_v7()),
        Utc::now(),
        DomainEvent::StepFailed {
            dispatch_id,
            step_id,
            step_type: StepType::Execute,
            error: error.to_owned(),
            error_class: ErrorClass::Transient,
            retry_count: u32::from(retry),
            duration_secs: FiniteF64::try_new(0.5).expect("f"),
        },
    );
    NackParams {
        dispatch_id,
        step_id,
        step_type: StepType::Execute,
        error: error.to_owned(),
        error_class: ErrorClass::Transient,
        retry,
        failure_event,
    }
}

// ---------------------------------------------------------------------------
// B-01: cross-dispatch ack_and_enqueue must be rejected
// ---------------------------------------------------------------------------

#[tokio::test]
async fn cross_dispatch_ack_and_enqueue_rejected() {
    let store = fresh_store().await;
    let id_a = create_dispatch(&store, "alpha", Lane::Impl)
        .await
        .expect("A");
    let id_b = create_dispatch(&store, "beta", Lane::Impl)
        .await
        .expect("B");

    let step_id = StepId::new();
    store
        .enqueue_step(enqueue_params(id_a, step_id, 0, Some(Lane::Impl)))
        .await
        .expect("enqueue");
    let _ = dequeue(&store, Some(Lane::Impl)).await.expect("dequeue");

    let result = exec_result();
    let completion = completed_event(id_a, step_id, &result);
    let next_for_b = enqueue_params(id_b, StepId::new(), 0, Some(Lane::Impl));
    let params = AckAndEnqueueParams {
        dispatch_id: id_a,
        step_id,
        step_type: StepType::Execute,
        result,
        completion_event: completion,
        next_step: Some(next_for_b),
    };

    let err = store
        .ack_and_enqueue(params)
        .await
        .expect_err("cross-dispatch must be rejected");
    assert!(
        matches!(err, StoreError::Conversion { .. }),
        "expected Conversion, got {err:?}"
    );
}

// ---------------------------------------------------------------------------
// I-01: nack(retry=true) must clear error; ack after retry must stay clean
// ---------------------------------------------------------------------------

#[tokio::test]
async fn nack_retry_clears_error() {
    let store = fresh_store().await;
    let id = create_dispatch(&store, "alpha", Lane::Impl)
        .await
        .expect("create");
    let step_id = StepId::new();
    store
        .enqueue_step(enqueue_params(id, step_id, 0, Some(Lane::Impl)))
        .await
        .expect("enqueue");
    let _ = dequeue(&store, Some(Lane::Impl)).await.expect("dequeue");

    store
        .nack(nack(id, step_id, "boom", true))
        .await
        .expect("nack");

    let view = store
        .get_step(&step_id)
        .await
        .expect("get")
        .expect("exists");
    assert_eq!(view.status, StepStatus::Pending);
    assert!(view.error.is_none(), "retry=true must clear error");
}

#[tokio::test]
async fn ack_clears_error_after_retry_cycle() {
    let store = fresh_store().await;
    let id = create_dispatch(&store, "alpha", Lane::Impl)
        .await
        .expect("create");
    let step_id = StepId::new();
    store
        .enqueue_step(enqueue_params(id, step_id, 0, Some(Lane::Impl)))
        .await
        .expect("enqueue");

    // First attempt: dequeue -> nack(retry)
    let _ = dequeue(&store, Some(Lane::Impl)).await.expect("d1");
    store
        .nack(nack(id, step_id, "transient", true))
        .await
        .expect("nack");

    // Second attempt: dequeue -> ack
    let _ = dequeue(&store, Some(Lane::Impl)).await.expect("d2");
    let result = exec_result();
    let completion = completed_event(id, step_id, &result);
    store
        .ack(AckParams {
            dispatch_id: id,
            step_id,
            step_type: StepType::Execute,
            result,
            completion_event: completion,
        })
        .await
        .expect("ack");

    let view = store
        .get_step(&step_id)
        .await
        .expect("get")
        .expect("exists");
    assert_eq!(view.status, StepStatus::Completed);
    assert!(view.error.is_none(), "ack must clear error");
}

// ---------------------------------------------------------------------------
// S-02: dequeue returns None when all steps are blocked
// ---------------------------------------------------------------------------

#[tokio::test]
async fn dequeue_returns_none_when_all_blocked() {
    let store = fresh_store().await;
    let id = create_dispatch(&store, "alpha", Lane::Impl)
        .await
        .expect("create");
    let step_id = StepId::new();
    let mut params = enqueue_params(id, step_id, 0, Some(Lane::Impl));
    params.ready_state = StepReadyState::Blocked;
    store.enqueue_step(params).await.expect("enqueue");

    let result = dequeue(&store, Some(Lane::Impl)).await.expect("dequeue");
    assert!(result.is_none(), "blocked step must not be dequeued");
}

// ---------------------------------------------------------------------------
// S-02: ack_and_enqueue rolls back on constraint violation
// ---------------------------------------------------------------------------

#[tokio::test]
async fn ack_and_enqueue_rolls_back_on_constraint_violation() {
    let store = fresh_store().await;
    let id = create_dispatch(&store, "alpha", Lane::Impl)
        .await
        .expect("create");

    let s0 = StepId::new();
    let s1 = StepId::new();
    store
        .enqueue_step(enqueue_params(id, s0, 0, Some(Lane::Impl)))
        .await
        .expect("enqueue s0");
    store
        .enqueue_step(enqueue_params(id, s1, 1, Some(Lane::Impl)))
        .await
        .expect("enqueue s1");

    let _ = dequeue(&store, Some(Lane::Impl)).await.expect("dequeue");

    // Ack s0 and try to enqueue at sequence 1 (already taken by s1).
    let result = exec_result();
    let completion = completed_event(id, s0, &result);
    let dup_next = enqueue_params(id, StepId::new(), 1, Some(Lane::Impl));
    let params = AckAndEnqueueParams {
        dispatch_id: id,
        step_id: s0,
        step_type: StepType::Execute,
        result,
        completion_event: completion,
        next_step: Some(dup_next),
    };

    store
        .ack_and_enqueue(params)
        .await
        .expect_err("duplicate sequence must fail");

    // The original step must still be Running — the tx rolled back.
    let view = store.get_step(&s0).await.expect("get").expect("exists");
    assert_eq!(
        view.status,
        StepStatus::Running,
        "step must remain Running after rollback"
    );
}

// ---------------------------------------------------------------------------
// I-03: Store::open_and_migrate triple-apply is idempotent
// ---------------------------------------------------------------------------

#[tokio::test]
async fn triple_migration_is_idempotent() {
    let store = Store::open_and_migrate("sqlite::memory:")
        .await
        .expect("first open");
    store.run_migrations().await.expect("second apply");
    store.run_migrations().await.expect("third apply");
}
