//! Integrity enforcement tests — audit-fix deltas for P1 and P2
//! findings: dispatch lifecycle enforcement, orphan step prevention,
//! duplicate step sequence rejection.

use chrono::Utc;
use tanren_domain::{
    ActorContext, AuthMode, Cli, ConfigKeys, DispatchId, DispatchMode, DispatchSnapshot,
    DomainEvent, EnvironmentHandle, EventEnvelope, EventId, ExecutePayload, FiniteF64,
    GraphRevision, Lane, NonEmptyString, OrgId, Outcome, Phase, StepId, StepPayload,
    StepReadyState, StepType, TimeoutSecs, UserId,
};
use tanren_store::{
    CreateDispatchParams, EnqueueStepParams, JobQueue, StateStore, Store, StoreError,
    UpdateDispatchStatusParams,
};
use uuid::Uuid;

async fn fresh_store() -> Store {
    let store = Store::new("sqlite::memory:").await.expect("connect");
    store.run_migrations().await.expect("migrate");
    store
}

fn snapshot() -> DispatchSnapshot {
    DispatchSnapshot {
        project: NonEmptyString::try_new("alpha".to_owned()).expect("p"),
        phase: Phase::DoTask,
        cli: Cli::Claude,
        auth_mode: AuthMode::ApiKey,
        branch: NonEmptyString::try_new("main".to_owned()).expect("b"),
        spec_folder: NonEmptyString::try_new("spec".to_owned()).expect("s"),
        workflow_id: NonEmptyString::try_new("wf".to_owned()).expect("w"),
        timeout: TimeoutSecs::try_new(60).expect("t"),
        environment_profile: NonEmptyString::try_new("d".to_owned()).expect("e"),
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

async fn create_dispatch(store: &Store, lane: Lane) -> DispatchId {
    let snap = snapshot();
    let a = actor();
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
            actor: a.clone(),
            graph_revision: GraphRevision::INITIAL,
        },
    );
    store
        .create_dispatch_projection(CreateDispatchParams {
            dispatch_id,
            mode: DispatchMode::Manual,
            lane,
            dispatch: snap,
            actor: a,
            graph_revision: GraphRevision::INITIAL,
            created_at,
            creation_event: event,
        })
        .await
        .expect("create dispatch");
    dispatch_id
}

fn execute_payload() -> StepPayload {
    StepPayload::Execute(Box::new(ExecutePayload {
        dispatch: snapshot(),
        handle: EnvironmentHandle {
            id: NonEmptyString::try_new("h".to_owned()).expect("h"),
            runtime_type: NonEmptyString::try_new("local".to_owned()).expect("rt"),
        },
    }))
}

fn enqueue_params(
    dispatch_id: DispatchId,
    step_type: StepType,
    step_sequence: u32,
    lane: Lane,
) -> EnqueueStepParams {
    let step_id = StepId::new();
    let event = EventEnvelope::new(
        EventId::from_uuid(Uuid::now_v7()),
        Utc::now(),
        DomainEvent::StepEnqueued {
            dispatch_id,
            step_id,
            step_type,
            step_sequence,
            lane: Some(lane),
            depends_on: vec![],
            graph_revision: GraphRevision::INITIAL,
        },
    );
    EnqueueStepParams {
        dispatch_id,
        step_id,
        step_type,
        step_sequence,
        lane: Some(lane),
        depends_on: vec![],
        graph_revision: GraphRevision::INITIAL,
        payload: execute_payload(),
        ready_state: StepReadyState::Ready,
        enqueue_event: event,
    }
}

fn status_params(
    dispatch_id: DispatchId,
    status: tanren_domain::DispatchStatus,
    outcome: Option<Outcome>,
) -> UpdateDispatchStatusParams {
    let payload = match status {
        tanren_domain::DispatchStatus::Running => DomainEvent::DispatchStarted { dispatch_id },
        tanren_domain::DispatchStatus::Completed => DomainEvent::DispatchCompleted {
            dispatch_id,
            outcome: outcome.unwrap_or(Outcome::Success),
            total_duration_secs: FiniteF64::try_new(1.0).expect("f"),
        },
        tanren_domain::DispatchStatus::Failed => DomainEvent::DispatchFailed {
            dispatch_id,
            outcome: outcome.unwrap_or(Outcome::Fail),
            failed_step_id: None,
            failed_step_type: None,
            error: "test failure".to_owned(),
        },
        tanren_domain::DispatchStatus::Cancelled => DomainEvent::DispatchCancelled {
            dispatch_id,
            actor: actor(),
            reason: None,
        },
        tanren_domain::DispatchStatus::Pending => {
            unreachable!("status_params must not be called with Pending")
        }
    };
    let event = EventEnvelope::new(EventId::from_uuid(Uuid::now_v7()), Utc::now(), payload);
    UpdateDispatchStatusParams {
        dispatch_id,
        status,
        outcome,
        status_event: event,
    }
}

// ---------------------------------------------------------------------------
// Dispatch lifecycle enforcement (P1)
// ---------------------------------------------------------------------------

/// `Pending -> Completed` is not legal — only `Pending -> Running`
/// and `Pending -> Cancelled` are allowed.
#[tokio::test]
async fn dispatch_rejects_illegal_transition() {
    let store = fresh_store().await;
    let id = create_dispatch(&store, Lane::Impl).await;

    let err = store
        .update_dispatch_status(status_params(
            id,
            tanren_domain::DispatchStatus::Completed,
            Some(Outcome::Success),
        ))
        .await
        .expect_err("Pending -> Completed must fail");
    assert!(matches!(err, StoreError::InvalidTransition { .. }));
}

/// Terminal states must not allow any outgoing transitions.
#[tokio::test]
async fn dispatch_rejects_terminal_rewrite() {
    let store = fresh_store().await;
    let id = create_dispatch(&store, Lane::Impl).await;

    store
        .update_dispatch_status(status_params(
            id,
            tanren_domain::DispatchStatus::Running,
            None,
        ))
        .await
        .expect("Pending -> Running");
    store
        .update_dispatch_status(status_params(
            id,
            tanren_domain::DispatchStatus::Completed,
            Some(Outcome::Success),
        ))
        .await
        .expect("Running -> Completed");

    let err = store
        .update_dispatch_status(status_params(
            id,
            tanren_domain::DispatchStatus::Running,
            None,
        ))
        .await
        .expect_err("Completed -> Running must fail");
    assert!(matches!(err, StoreError::InvalidTransition { .. }));
}

// ---------------------------------------------------------------------------
// Orphan step prevention (P1)
// ---------------------------------------------------------------------------

/// Enqueuing a step against a nonexistent dispatch must fail.
#[tokio::test]
async fn enqueue_step_rejects_orphan_dispatch() {
    let store = fresh_store().await;
    let fake_id = DispatchId::new();
    let params = enqueue_params(fake_id, StepType::Execute, 0, Lane::Impl);
    let err = store
        .enqueue_step(params)
        .await
        .expect_err("orphan step must fail");
    assert!(
        matches!(err, StoreError::NotFound { .. }),
        "expected NotFound, got {err:?}"
    );
}

// ---------------------------------------------------------------------------
// Duplicate step sequence prevention (P2)
// ---------------------------------------------------------------------------

/// Two steps with the same `(dispatch_id, step_sequence)` must fail.
#[tokio::test]
async fn duplicate_step_sequence_rejected() {
    let store = fresh_store().await;
    let id = create_dispatch(&store, Lane::Impl).await;

    store
        .enqueue_step(enqueue_params(id, StepType::Execute, 0, Lane::Impl))
        .await
        .expect("first enqueue");

    let err = store
        .enqueue_step(enqueue_params(id, StepType::Provision, 0, Lane::Impl))
        .await
        .expect_err("duplicate step_sequence must fail");
    assert!(
        matches!(err, StoreError::Database(_)),
        "expected Database error, got {err:?}"
    );
}
