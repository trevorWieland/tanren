//! Shared fixture helpers for the integration test suites.
//!
//! This module is `mod common;` in each integration test binary.
//! It intentionally does **not** declare tests itself — it only
//! provides builders for `tanren-domain` values and the typed param
//! structs that the store's traits consume. Each integration binary
//! (`sqlite_integration`, `postgres_integration`) runs the same set
//! of `#[tokio::test]` functions against a freshly-created
//! `tanren_store::Store`.
//!
//! The helpers favor determinism over realism: timestamps are taken
//! from `chrono::Utc::now()`, UUIDs from `uuid::Uuid::now_v7()`, and
//! every envelope is built from raw domain constructors so the
//! integration tests do not depend on any orchestrator or planner
//! logic that doesn't exist yet.

use chrono::{DateTime, Utc};
use tanren_domain::{
    ActorContext, AuthMode, Cli, ConfigKeys, DispatchId, DispatchMode, DispatchSnapshot,
    DispatchStatus, DomainEvent, EntityRef, EventEnvelope, EventId, ExecutePayload, ExecuteResult,
    FiniteF64, GraphRevision, Lane, NonEmptyString, OrgId, Outcome, Phase, ProvisionPayload,
    ProvisionResult, SCHEMA_VERSION, StepId, StepPayload, StepReadyState, StepResult, StepType,
    TimeoutSecs, UserId,
};
use tanren_store::{
    AckAndEnqueueParams, CreateDispatchParams, DequeueParams, EnqueueStepParams, JobQueue,
    StateStore, Store, StoreResult,
};
use uuid::Uuid;

/// Build a canonical [`DispatchSnapshot`] for tests.
pub(crate) fn snapshot(project: &str) -> DispatchSnapshot {
    DispatchSnapshot {
        project: NonEmptyString::try_new(project.to_owned()).expect("project"),
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

/// Build a canonical [`ActorContext`] for tests.
pub(crate) fn actor() -> ActorContext {
    ActorContext {
        org_id: OrgId::new(),
        user_id: UserId::new(),
        team_id: None,
        api_key_id: None,
        project_id: None,
    }
}

/// Build a [`CreateDispatchParams`] with a fresh dispatch id and a
/// canonical `DispatchCreated` envelope.
pub(crate) fn create_dispatch_params(
    project: &str,
    actor: ActorContext,
    lane: Lane,
) -> CreateDispatchParams {
    let dispatch_id = DispatchId::new();
    let snap = snapshot(project);
    let created_at = Utc::now();
    let event = EventEnvelope {
        schema_version: SCHEMA_VERSION,
        event_id: EventId::from_uuid(Uuid::now_v7()),
        timestamp: created_at,
        entity_ref: EntityRef::Dispatch(dispatch_id),
        payload: DomainEvent::DispatchCreated {
            dispatch_id,
            dispatch: Box::new(snap.clone()),
            mode: DispatchMode::Manual,
            lane,
            actor: actor.clone(),
            graph_revision: GraphRevision::INITIAL,
        },
    };
    CreateDispatchParams {
        dispatch_id,
        mode: DispatchMode::Manual,
        lane,
        dispatch: snap,
        actor,
        graph_revision: GraphRevision::INITIAL,
        created_at,
        creation_event: event,
    }
}

/// Build an [`EnqueueStepParams`] pointing at `dispatch_id` with the
/// given type and sequence. Includes a `StepEnqueued` envelope.
pub(crate) fn enqueue_step_params(
    dispatch_id: DispatchId,
    step_id: StepId,
    step_type: StepType,
    step_sequence: u32,
    lane: Option<Lane>,
    payload: StepPayload,
) -> EnqueueStepParams {
    let event = EventEnvelope {
        schema_version: SCHEMA_VERSION,
        event_id: EventId::from_uuid(Uuid::now_v7()),
        timestamp: Utc::now(),
        entity_ref: EntityRef::Step(step_id),
        payload: DomainEvent::StepEnqueued {
            dispatch_id,
            step_id,
            step_type,
            step_sequence,
            lane,
            depends_on: vec![],
            graph_revision: GraphRevision::INITIAL,
        },
    };
    EnqueueStepParams {
        dispatch_id,
        step_id,
        step_type,
        step_sequence,
        lane,
        depends_on: vec![],
        graph_revision: GraphRevision::INITIAL,
        payload,
        ready_state: StepReadyState::Ready,
        enqueue_event: event,
    }
}

/// Build a `StepCompleted` envelope matching the given result.
pub(crate) fn step_completed_event(
    dispatch_id: DispatchId,
    step_id: StepId,
    step_type: StepType,
    result: &StepResult,
) -> EventEnvelope {
    EventEnvelope {
        schema_version: SCHEMA_VERSION,
        event_id: EventId::from_uuid(Uuid::now_v7()),
        timestamp: Utc::now(),
        entity_ref: EntityRef::Step(step_id),
        payload: DomainEvent::StepCompleted {
            dispatch_id,
            step_id,
            step_type,
            duration_secs: FiniteF64::try_new(1.0).expect("finite"),
            result_payload: Box::new(result.clone()),
        },
    }
}

/// A canonical `StepResult::Provision` for tests.
pub(crate) fn provision_result() -> StepResult {
    StepResult::Provision(Box::new(ProvisionResult {
        handle: tanren_domain::EnvironmentHandle {
            id: NonEmptyString::try_new("handle-1".to_owned()).expect("handle"),
            runtime_type: NonEmptyString::try_new("local".to_owned()).expect("runtime"),
        },
    }))
}

/// A canonical `StepPayload::Provision` for tests.
pub(crate) fn provision_payload(snap: DispatchSnapshot) -> StepPayload {
    StepPayload::Provision(Box::new(ProvisionPayload { dispatch: snap }))
}

/// A canonical `StepPayload::Execute` for tests.
pub(crate) fn execute_payload(snap: DispatchSnapshot) -> StepPayload {
    StepPayload::Execute(Box::new(ExecutePayload {
        dispatch: snap,
        handle: tanren_domain::EnvironmentHandle {
            id: NonEmptyString::try_new("handle-1".to_owned()).expect("handle"),
            runtime_type: NonEmptyString::try_new("local".to_owned()).expect("runtime"),
        },
    }))
}

/// A canonical `StepResult::Execute` for tests.
pub(crate) fn execute_result() -> StepResult {
    StepResult::Execute(Box::new(ExecuteResult {
        outcome: Outcome::Success,
        signal: None,
        exit_code: Some(0),
        duration_secs: FiniteF64::try_new(1.5).expect("finite"),
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

/// Convenience: create a dispatch projection and return its id.
pub(crate) async fn create_dispatch(
    store: &Store,
    project: &str,
    actor: ActorContext,
    lane: Lane,
) -> StoreResult<DispatchId> {
    let params = create_dispatch_params(project, actor, lane);
    let id = params.dispatch_id;
    store.create_dispatch_projection(params).await?;
    Ok(id)
}

/// Seed a dispatch with `count` pending execute-lane steps. Returns
/// the vector of generated step ids in insertion order.
pub(crate) async fn seed_steps(
    store: &Store,
    dispatch_id: DispatchId,
    snap: &DispatchSnapshot,
    lane: Lane,
    count: u32,
) -> StoreResult<Vec<StepId>> {
    let mut ids = Vec::with_capacity(count as usize);
    for seq in 0..count {
        let step_id = StepId::new();
        let params = enqueue_step_params(
            dispatch_id,
            step_id,
            StepType::Execute,
            seq,
            Some(lane),
            execute_payload(snap.clone()),
        );
        store.enqueue_step(params).await?;
        ids.push(step_id);
    }
    Ok(ids)
}

/// Attempt a dequeue with the given knobs — helper so tests don't
/// repeat the param construction.
pub(crate) async fn try_dequeue(
    store: &Store,
    worker_id: &str,
    lane: Option<Lane>,
    max_concurrent: u64,
) -> StoreResult<Option<tanren_store::QueuedStep>> {
    store
        .dequeue(DequeueParams {
            worker_id: worker_id.to_owned(),
            lane,
            max_concurrent,
        })
        .await
}

/// Build an `AckAndEnqueueParams` that ACKs `step_id` with an execute
/// result and enqueues a successor step with the same dispatch id.
pub(crate) fn ack_and_enqueue_execute(
    dispatch_id: DispatchId,
    ack_step_id: StepId,
    ack_step_type: StepType,
    snap: &DispatchSnapshot,
    next_step_id: StepId,
    next_seq: u32,
    next_lane: Option<Lane>,
) -> AckAndEnqueueParams {
    let result = execute_result();
    let completion_event = step_completed_event(dispatch_id, ack_step_id, ack_step_type, &result);
    let next = enqueue_step_params(
        dispatch_id,
        next_step_id,
        StepType::Execute,
        next_seq,
        next_lane,
        execute_payload(snap.clone()),
    );
    AckAndEnqueueParams {
        step_id: ack_step_id,
        result,
        completion_event,
        next_step: Some(next),
    }
}

/// Build a creation params whose `dispatch_id` equals `existing` — the
/// unique-constraint violation is the trigger for rollback tests.
pub(crate) fn duplicate_create_params(
    existing: DispatchId,
    actor: ActorContext,
    lane: Lane,
) -> CreateDispatchParams {
    let snap = snapshot("clashing");
    let created_at = Utc::now();
    let event = EventEnvelope {
        schema_version: SCHEMA_VERSION,
        event_id: EventId::from_uuid(Uuid::now_v7()),
        timestamp: created_at,
        entity_ref: EntityRef::Dispatch(existing),
        payload: DomainEvent::DispatchCreated {
            dispatch_id: existing,
            dispatch: Box::new(snap.clone()),
            mode: DispatchMode::Manual,
            lane,
            actor: actor.clone(),
            graph_revision: GraphRevision::INITIAL,
        },
    };
    CreateDispatchParams {
        dispatch_id: existing,
        mode: DispatchMode::Manual,
        lane,
        dispatch: snap,
        actor,
        graph_revision: GraphRevision::INITIAL,
        created_at,
        creation_event: event,
    }
}

/// Return the current server-side wall clock inside a transaction
/// (wrapped in a helper so the caller doesn't need to import chrono
/// just to get `Utc::now()`).
pub(crate) fn now() -> DateTime<Utc> {
    Utc::now()
}

/// Assert that the given dispatch projection exists and has the
/// expected status.
pub(crate) async fn assert_dispatch_status(
    store: &Store,
    dispatch_id: &DispatchId,
    expected: DispatchStatus,
) {
    let view = store
        .get_dispatch(dispatch_id)
        .await
        .expect("get dispatch")
        .expect("dispatch should exist");
    assert_eq!(view.status, expected);
}
