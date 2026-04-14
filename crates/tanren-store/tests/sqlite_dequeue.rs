//! Dequeue and recovery edge-case tests on `SQLite`.
//!
//! These exercises supplement the broader integration suite with
//! targeted coverage for the dequeue path, lane filtering,
//! `max_concurrent` enforcement, stale-step recovery (including
//! `NULL` heartbeats), CAS-safe dispatch status updates, and
//! impossible-state rejection.
//!
//! This file defines its own minimal helpers rather than importing
//! the shared `common` module to avoid dead-code warnings from
//! unused shared helpers.

use chrono::Utc;
use tanren_domain::{
    ActorContext, AuthMode, Cli, ConfigKeys, DispatchId, DispatchMode, DispatchSnapshot,
    DispatchStatus, DomainEvent, EnvironmentHandle, EventEnvelope, EventId, ExecutePayload,
    FiniteF64, GraphRevision, Lane, NonEmptyString, OrgId, Outcome, Phase, StepId, StepPayload,
    StepReadyState, StepType, TimeoutSecs, UserId,
};
use tanren_store::{
    DequeueParams, EnqueueStepParams, JobQueue, QueuedStep, StateStore, Store, StoreError,
    StoreResult, UpdateDispatchStatusParams,
};
use uuid::Uuid;

// ---- minimal helpers -------------------------------------------------------

async fn fresh_store() -> Store {
    let store = Store::new("sqlite::memory:").await.expect("connect");
    store.run_migrations().await.expect("migrate");
    store
}

fn snap() -> DispatchSnapshot {
    DispatchSnapshot {
        project: NonEmptyString::try_new("test".to_owned()).expect("project"),
        phase: Phase::DoTask,
        cli: Cli::Claude,
        auth_mode: AuthMode::ApiKey,
        branch: NonEmptyString::try_new("main".to_owned()).expect("branch"),
        spec_folder: NonEmptyString::try_new("spec".to_owned()).expect("spec"),
        workflow_id: NonEmptyString::try_new("wf-1".to_owned()).expect("wf"),
        timeout: TimeoutSecs::try_new(60).expect("timeout"),
        environment_profile: NonEmptyString::try_new("default".to_owned()).expect("prof"),
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
    let id = DispatchId::new();
    let s = snap();
    let a = actor();
    let created_at = Utc::now();
    let event = EventEnvelope::new(
        EventId::from_uuid(Uuid::now_v7()),
        created_at,
        DomainEvent::DispatchCreated {
            dispatch_id: id,
            dispatch: Box::new(s.clone()),
            mode: DispatchMode::Manual,
            lane,
            actor: a.clone(),
            graph_revision: GraphRevision::INITIAL,
        },
    );
    store
        .create_dispatch_projection(tanren_store::CreateDispatchParams {
            dispatch_id: id,
            mode: DispatchMode::Manual,
            lane,
            dispatch: s,
            actor: a,
            graph_revision: GraphRevision::INITIAL,
            created_at,
            creation_event: event,
        })
        .await?;
    Ok(id)
}

fn enqueue_params(dispatch_id: DispatchId, seq: u32, lane: Lane) -> EnqueueStepParams {
    let step_id = StepId::new();
    let s = snap();
    let event = EventEnvelope::new(
        EventId::from_uuid(Uuid::now_v7()),
        Utc::now(),
        DomainEvent::StepEnqueued {
            dispatch_id,
            step_id,
            step_type: StepType::Execute,
            step_sequence: seq,
            lane: Some(lane),
            depends_on: vec![],
            graph_revision: GraphRevision::INITIAL,
        },
    );
    EnqueueStepParams {
        dispatch_id,
        step_id,
        step_type: StepType::Execute,
        step_sequence: seq,
        lane: Some(lane),
        depends_on: vec![],
        graph_revision: GraphRevision::INITIAL,
        payload: StepPayload::Execute(Box::new(ExecutePayload {
            dispatch: s,
            handle: EnvironmentHandle {
                id: NonEmptyString::try_new("h-1".to_owned()).expect("handle"),
                runtime_type: NonEmptyString::try_new("local".to_owned()).expect("rt"),
            },
        })),
        ready_state: StepReadyState::Ready,
        enqueue_event: event,
    }
}

async fn dequeue(
    store: &Store,
    worker: &str,
    lane: Option<Lane>,
    max: u64,
) -> StoreResult<Option<QueuedStep>> {
    store
        .dequeue(DequeueParams {
            worker_id: worker.to_owned(),
            lane,
            max_concurrent: max,
        })
        .await
}

fn status_params(
    id: DispatchId,
    status: DispatchStatus,
    outcome: Option<Outcome>,
) -> UpdateDispatchStatusParams {
    let payload = match status {
        DispatchStatus::Pending => unreachable!("test helper"),
        DispatchStatus::Running => DomainEvent::DispatchStarted { dispatch_id: id },
        DispatchStatus::Completed => DomainEvent::DispatchCompleted {
            dispatch_id: id,
            outcome: outcome.unwrap_or(Outcome::Success),
            total_duration_secs: FiniteF64::try_new(1.0).expect("finite"),
        },
        DispatchStatus::Failed => DomainEvent::DispatchFailed {
            dispatch_id: id,
            outcome: outcome.unwrap_or(Outcome::Fail),
            failed_step_id: None,
            failed_step_type: None,
            error: "test".to_owned(),
        },
        DispatchStatus::Cancelled => DomainEvent::DispatchCancelled {
            dispatch_id: id,
            actor: actor(),
            reason: None,
        },
    };
    let event = EventEnvelope::new(EventId::from_uuid(Uuid::now_v7()), Utc::now(), payload);
    UpdateDispatchStatusParams {
        dispatch_id: id,
        status,
        outcome,
        status_event: event,
    }
}

// ---- dequeue edge cases ---------------------------------------------------

#[tokio::test]
async fn dequeue_returns_none_when_no_pending_steps() {
    let store = fresh_store().await;
    let _id = create_dispatch(&store, Lane::Impl).await.expect("create");
    let result = dequeue(&store, "w-1", Some(Lane::Impl), 10)
        .await
        .expect("dequeue");
    assert!(result.is_none());
}

#[tokio::test]
async fn dequeue_returns_none_at_max_concurrent() {
    let store = fresh_store().await;
    let id = create_dispatch(&store, Lane::Impl).await.expect("create");
    store
        .enqueue_step(enqueue_params(id, 0, Lane::Impl))
        .await
        .expect("enqueue 0");
    store
        .enqueue_step(enqueue_params(id, 1, Lane::Impl))
        .await
        .expect("enqueue 1");

    let first = dequeue(&store, "w-1", Some(Lane::Impl), 1)
        .await
        .expect("dequeue first");
    assert!(first.is_some(), "first dequeue should succeed");

    let second = dequeue(&store, "w-2", Some(Lane::Impl), 1)
        .await
        .expect("dequeue second");
    assert!(second.is_none(), "second dequeue should return None");
}

#[tokio::test]
async fn dequeue_respects_lane_filter() {
    let store = fresh_store().await;
    let id = create_dispatch(&store, Lane::Impl).await.expect("create");

    store
        .enqueue_step(enqueue_params(id, 0, Lane::Impl))
        .await
        .expect("enqueue impl");
    let audit_params = enqueue_params(id, 1, Lane::Audit);
    let audit_step = audit_params.step_id;
    store
        .enqueue_step(audit_params)
        .await
        .expect("enqueue audit");

    let claimed = dequeue(&store, "w-1", Some(Lane::Audit), 10)
        .await
        .expect("dequeue audit");
    let claimed = claimed.expect("audit-lane dequeue should succeed");
    assert_eq!(claimed.step_id, audit_step);

    let none = dequeue(&store, "w-2", Some(Lane::Audit), 10)
        .await
        .expect("dequeue audit again");
    assert!(none.is_none());
}

// ---- stale-step recovery --------------------------------------------------

#[tokio::test]
async fn recover_null_heartbeat_steps() {
    let store = fresh_store().await;
    let id = create_dispatch(&store, Lane::Impl).await.expect("create");
    store
        .enqueue_step(enqueue_params(id, 0, Lane::Impl))
        .await
        .expect("enqueue");

    let claimed = dequeue(&store, "w-1", Some(Lane::Impl), 10)
        .await
        .expect("dequeue");
    assert!(claimed.is_some());

    // Heartbeat is NULL (never called heartbeat_step). Recovery
    // with timeout=0 must reclaim the step.
    let recovered = store.recover_stale_steps(0).await.expect("recover");
    assert_eq!(recovered, 1, "NULL-heartbeat step must be recovered");
}

// ---- CAS dispatch status --------------------------------------------------

#[tokio::test]
async fn cas_dispatch_status_conflict() {
    let store = fresh_store().await;
    let id = create_dispatch(&store, Lane::Impl).await.expect("create");

    store
        .update_dispatch_status(status_params(id, DispatchStatus::Running, None))
        .await
        .expect("pending -> running");

    // Running -> Running is not a valid transition.
    let err = store
        .update_dispatch_status(status_params(id, DispatchStatus::Running, None))
        .await
        .expect_err("duplicate Running transition must fail");
    assert!(
        matches!(err, StoreError::InvalidTransition { .. }),
        "expected InvalidTransition, got {err:?}"
    );
}

// ---- impossible-state rejection -------------------------------------------

#[tokio::test]
async fn started_with_outcome_rejected() {
    let store = fresh_store().await;
    let id = create_dispatch(&store, Lane::Impl).await.expect("create");

    let event = EventEnvelope::new(
        EventId::from_uuid(Uuid::now_v7()),
        Utc::now(),
        DomainEvent::DispatchStarted { dispatch_id: id },
    );
    let params = UpdateDispatchStatusParams {
        dispatch_id: id,
        status: DispatchStatus::Running,
        outcome: Some(Outcome::Success),
        status_event: event,
    };
    let err = store
        .update_dispatch_status(params)
        .await
        .expect_err("started with outcome must be rejected");
    assert!(
        matches!(err, StoreError::Conversion { .. }),
        "expected Conversion, got {err:?}"
    );
}

#[tokio::test]
async fn cancelled_with_outcome_rejected() {
    let store = fresh_store().await;
    let id = create_dispatch(&store, Lane::Impl).await.expect("create");

    store
        .update_dispatch_status(status_params(id, DispatchStatus::Running, None))
        .await
        .expect("pending -> running");

    let event = EventEnvelope::new(
        EventId::from_uuid(Uuid::now_v7()),
        Utc::now(),
        DomainEvent::DispatchCancelled {
            dispatch_id: id,
            actor: actor(),
            reason: None,
        },
    );
    let params = UpdateDispatchStatusParams {
        dispatch_id: id,
        status: DispatchStatus::Cancelled,
        outcome: Some(Outcome::Success),
        status_event: event,
    };
    let err = store
        .update_dispatch_status(params)
        .await
        .expect_err("cancelled with outcome must be rejected");
    assert!(
        matches!(err, StoreError::Conversion { .. }),
        "expected Conversion, got {err:?}"
    );
}
