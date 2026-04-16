//! Unit-level orchestrator tests with a recording store.

#[path = "support/dispatch_fixtures.rs"]
mod dispatch_fixtures;
#[path = "support/recording_store.rs"]
mod recording_store;

use dispatch_fixtures::{sample_actor, sample_command, sample_replay_guard};
use recording_store::RecordingStore;
use tanren_domain::{
    CancelDispatch, Cli, DispatchStatus, DomainError, EntityRef, FiniteF64, Outcome, StepPayload,
    StepReadyState, StepType, cli_to_lane,
};
use tanren_orchestrator::{Orchestrator, OrchestratorError};
use tanren_policy::PolicyEngine;
use tanren_store::{DispatchFilter, StoreError};

#[tokio::test]
async fn create_dispatch_records_atomic_store_operation() {
    let store = RecordingStore::default();
    let orch = Orchestrator::new(store.clone(), PolicyEngine::new());

    let created = orch
        .create_dispatch(sample_command(sample_actor()), sample_replay_guard())
        .await
        .expect("create");
    let snapshot = store.snapshot().await;

    assert_eq!(snapshot.created_dispatches.len(), 1);
    let params = &snapshot.created_dispatches[0];
    assert_eq!(params.dispatch.dispatch_id, created.dispatch_id);
    assert_eq!(params.dispatch.lane, cli_to_lane(&Cli::Claude));
    assert_eq!(params.initial_step.step_type, StepType::Provision);
    assert_eq!(params.initial_step.step_sequence, 0);
    assert_eq!(params.initial_step.ready_state, StepReadyState::Ready);
    assert!(
        matches!(
            params.initial_step.payload,
            StepPayload::ProvisionRef(ref payload)
                if payload.dispatch_ref.dispatch_id == created.dispatch_id
        ),
        "create path must store a typed snapshot reference payload"
    );

    let stored = snapshot
        .dispatches
        .get(&created.dispatch_id)
        .expect("dispatch should be stored");
    assert_eq!(stored.dispatch.required_secrets, vec!["OPENAI_API_KEY"]);
    assert!(stored.dispatch.preserve_on_failure);
}

#[tokio::test]
async fn cancel_dispatch_records_cancel_params_and_updates_status() {
    let store = RecordingStore::default();
    let orch = Orchestrator::new(store.clone(), PolicyEngine::new());
    let actor = sample_actor();

    let created = orch
        .create_dispatch(sample_command(actor.clone()), sample_replay_guard())
        .await
        .expect("create");

    orch.cancel_dispatch(
        CancelDispatch {
            actor,
            dispatch_id: created.dispatch_id,
            reason: Some("user cancelled".to_owned()),
        },
        sample_replay_guard(),
    )
    .await
    .expect("cancel");

    let snapshot = store.snapshot().await;
    assert_eq!(snapshot.cancelled_dispatches.len(), 1);
    assert_eq!(
        snapshot.cancelled_dispatches[0].reason.as_deref(),
        Some("user cancelled")
    );

    let stored = snapshot
        .dispatches
        .get(&created.dispatch_id)
        .expect("dispatch should be stored");
    assert_eq!(stored.status, DispatchStatus::Cancelled);
}

#[tokio::test]
async fn finalize_dispatch_enforces_single_terminal_failed_path() {
    let store = RecordingStore::default();
    let orch = Orchestrator::new(store.clone(), PolicyEngine::new());

    let created = orch
        .create_dispatch(sample_command(sample_actor()), sample_replay_guard())
        .await
        .expect("create");
    orch.start_dispatch(created.dispatch_id)
        .await
        .expect("start");
    orch.finalize_dispatch(
        created.dispatch_id,
        Outcome::Error,
        FiniteF64::try_new(3.1).expect("finite"),
        None,
        None,
        Some("failed".to_owned()),
    )
    .await
    .expect("finalize");

    let snapshot = store.snapshot().await;
    let updates = snapshot.dispatch_status_updates;
    assert_eq!(updates.len(), 2, "running + failed updates expected");
    assert_eq!(updates[0].status, DispatchStatus::Running);
    assert_eq!(updates[1].status, DispatchStatus::Failed);
    assert_eq!(updates[1].outcome, Some(Outcome::Error));
    assert!(matches!(
        updates[1].status_event.payload,
        tanren_domain::DomainEvent::DispatchFailed {
            outcome: Outcome::Error,
            ..
        }
    ));
}

#[tokio::test]
async fn cancel_nonexistent_dispatch_returns_not_found() {
    let store = RecordingStore::default();
    let orch = Orchestrator::new(store.clone(), PolicyEngine::new());

    let err = orch
        .cancel_dispatch(
            CancelDispatch {
                actor: sample_actor(),
                dispatch_id: tanren_domain::DispatchId::new(),
                reason: None,
            },
            sample_replay_guard(),
        )
        .await
        .expect_err("cancel should fail");
    assert!(matches!(
        err,
        OrchestratorError::Domain(DomainError::NotFound {
            entity: EntityRef::Dispatch(_),
        })
    ));
    let snapshot = store.snapshot().await;
    assert_eq!(snapshot.policy_decision_events.len(), 1);
    assert!(matches!(
        snapshot.policy_decision_events[0].payload,
        tanren_domain::DomainEvent::PolicyDecision { ref decision, .. }
            if decision.reason_code == Some(tanren_domain::PolicyReasonCode::CancelDispatchNotFound)
    ));
    assert!(snapshot.cancelled_dispatches.is_empty());
}

#[tokio::test]
async fn cancel_dispatch_hides_actor_scope_mismatch_as_not_found() {
    let store = RecordingStore::default();
    let orch = Orchestrator::new(store.clone(), PolicyEngine::new());
    let created = orch
        .create_dispatch(sample_command(sample_actor()), sample_replay_guard())
        .await
        .expect("create");

    let err = orch
        .cancel_dispatch(
            CancelDispatch {
                actor: tanren_domain::ActorContext::new(
                    tanren_domain::OrgId::new(),
                    tanren_domain::UserId::new(),
                ),
                dispatch_id: created.dispatch_id,
                reason: Some("mismatch".to_owned()),
            },
            sample_replay_guard(),
        )
        .await
        .expect_err("cancel should fail");
    assert!(matches!(
        err,
        OrchestratorError::Domain(DomainError::NotFound {
            entity: EntityRef::Dispatch(_),
        })
    ));

    let snapshot = store.snapshot().await;
    assert_eq!(snapshot.policy_decision_events.len(), 1);
    assert!(matches!(
        snapshot.policy_decision_events[0].payload,
        tanren_domain::DomainEvent::PolicyDecision { ref decision, .. }
            if decision.reason_code == Some(tanren_domain::PolicyReasonCode::CancelOrgMismatch)
    ));
    assert!(snapshot.cancelled_dispatches.is_empty());
}

#[tokio::test]
async fn list_dispatches_applies_filters_without_store_sql_knowledge() {
    let store = RecordingStore::default();
    let orch = Orchestrator::new(store, PolicyEngine::new());
    let actor = sample_actor();

    let created = orch
        .create_dispatch(sample_command(actor.clone()), sample_replay_guard())
        .await
        .expect("create");

    let mut filter = DispatchFilter::new();
    filter.status = Some(DispatchStatus::Pending);
    let page = orch
        .list_dispatches_for_actor(filter, &actor)
        .await
        .expect("list");

    assert_eq!(page.dispatches.len(), 1);
    assert_eq!(page.dispatches[0].dispatch_id, created.dispatch_id);
    assert_eq!(page.dispatches[0].status, DispatchStatus::Pending);
}

#[tokio::test]
async fn denied_create_consumes_replay_guard_atomically() {
    let store = RecordingStore::default();
    let orch = Orchestrator::new(store.clone(), PolicyEngine::new());

    let mut cmd = sample_command(sample_actor());
    cmd.mode = tanren_domain::DispatchMode::Auto;
    cmd.preserve_on_failure = true;

    let replay = sample_replay_guard();
    let err = orch
        .create_dispatch(cmd, replay.clone())
        .await
        .expect_err("policy should deny");
    assert!(matches!(err, OrchestratorError::PolicyDenied { .. }));

    let snapshot = store.snapshot().await;
    assert_eq!(snapshot.policy_decision_events.len(), 1);
    assert!(
        snapshot.consumed_replay_jtis.contains(&replay.jti),
        "denied create must consume replay jti"
    );
}

#[tokio::test]
async fn denied_create_replayed_jti_is_rejected_as_replay() {
    let store = RecordingStore::default();
    let orch = Orchestrator::new(store.clone(), PolicyEngine::new());
    let replay = sample_replay_guard();

    let mut cmd = sample_command(sample_actor());
    cmd.mode = tanren_domain::DispatchMode::Auto;
    cmd.preserve_on_failure = true;

    let _first = orch
        .create_dispatch(cmd.clone(), replay.clone())
        .await
        .expect_err("first denied call");

    let second = orch
        .create_dispatch(cmd, replay)
        .await
        .expect_err("second call reusing jti must be replay-rejected");
    assert!(
        matches!(second, OrchestratorError::Store(StoreError::ReplayRejected)),
        "expected ReplayRejected, got {second:?}"
    );

    let snapshot = store.snapshot().await;
    assert_eq!(snapshot.policy_decision_events.len(), 1);
}

#[tokio::test]
async fn denied_cancel_consumes_replay_guard_atomically() {
    let store = RecordingStore::default();
    let orch = Orchestrator::new(store.clone(), PolicyEngine::new());
    let created = orch
        .create_dispatch(sample_command(sample_actor()), sample_replay_guard())
        .await
        .expect("create");

    let replay = sample_replay_guard();
    let err = orch
        .cancel_dispatch(
            CancelDispatch {
                actor: tanren_domain::ActorContext::new(
                    tanren_domain::OrgId::new(),
                    tanren_domain::UserId::new(),
                ),
                dispatch_id: created.dispatch_id,
                reason: Some("mismatch".to_owned()),
            },
            replay.clone(),
        )
        .await
        .expect_err("cancel should fail");
    assert!(matches!(
        err,
        OrchestratorError::Domain(DomainError::NotFound {
            entity: EntityRef::Dispatch(_),
        })
    ));

    let snapshot = store.snapshot().await;
    assert!(snapshot.consumed_replay_jtis.contains(&replay.jti));
    assert!(snapshot.cancelled_dispatches.is_empty());
}

#[tokio::test]
async fn denied_cancel_replayed_jti_is_rejected_as_replay() {
    let store = RecordingStore::default();
    let orch = Orchestrator::new(store.clone(), PolicyEngine::new());
    let created = orch
        .create_dispatch(sample_command(sample_actor()), sample_replay_guard())
        .await
        .expect("create");

    let replay = sample_replay_guard();
    let foreign_actor =
        tanren_domain::ActorContext::new(tanren_domain::OrgId::new(), tanren_domain::UserId::new());

    let _first = orch
        .cancel_dispatch(
            CancelDispatch {
                actor: foreign_actor.clone(),
                dispatch_id: created.dispatch_id,
                reason: Some("denied first".to_owned()),
            },
            replay.clone(),
        )
        .await
        .expect_err("first denied cancel");

    let second = orch
        .cancel_dispatch(
            CancelDispatch {
                actor: foreign_actor,
                dispatch_id: created.dispatch_id,
                reason: Some("denied second".to_owned()),
            },
            replay,
        )
        .await
        .expect_err("second denied cancel");
    assert!(
        matches!(second, OrchestratorError::Store(StoreError::ReplayRejected)),
        "expected ReplayRejected, got {second:?}"
    );

    let snapshot = store.snapshot().await;
    assert_eq!(snapshot.policy_decision_events.len(), 1);
}

#[tokio::test]
async fn missing_cancel_consumes_replay_guard_atomically() {
    let store = RecordingStore::default();
    let orch = Orchestrator::new(store.clone(), PolicyEngine::new());

    let replay = sample_replay_guard();
    let err = orch
        .cancel_dispatch(
            CancelDispatch {
                actor: sample_actor(),
                dispatch_id: tanren_domain::DispatchId::new(),
                reason: None,
            },
            replay.clone(),
        )
        .await
        .expect_err("missing cancel should return not-found");
    assert!(matches!(
        err,
        OrchestratorError::Domain(DomainError::NotFound {
            entity: EntityRef::Dispatch(_),
        })
    ));

    let snapshot = store.snapshot().await;
    assert_eq!(snapshot.policy_decision_events.len(), 1);
    assert!(snapshot.consumed_replay_jtis.contains(&replay.jti));
}
