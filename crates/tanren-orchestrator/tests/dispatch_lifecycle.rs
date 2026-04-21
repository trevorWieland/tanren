//! Integration tests for dispatch lifecycle operations.

#[path = "support/dispatch_fixtures.rs"]
mod dispatch_fixtures;

use dispatch_fixtures::{sample_actor, sample_command, sample_replay_guard};
use tanren_domain::{
    CancelDispatch, Cli, DispatchId, DispatchMode, DispatchStatus, DomainError, EntityRef,
    FiniteF64, NonEmptyString, Outcome, Phase, StepReadyState, StepStatus, StepType,
};
use tanren_orchestrator::{Orchestrator, OrchestratorError};
use tanren_policy::PolicyEngine;
use tanren_store::{DispatchFilter, EventFilter, StateStore, Store, StoreError};

async fn setup() -> Orchestrator<Store> {
    let store = Store::open_and_migrate("sqlite::memory:")
        .await
        .expect("store");
    let policy = PolicyEngine::new();
    Orchestrator::new(store, policy)
}

#[tokio::test]
async fn create_dispatch_returns_pending_view() {
    let orch = setup().await;
    let actor = sample_actor();
    let cmd = sample_command(actor.clone());
    let view = orch
        .create_dispatch(cmd, sample_replay_guard())
        .await
        .expect("create");

    assert_eq!(view.status, DispatchStatus::Pending);
    assert_eq!(view.dispatch.project.as_str(), "test-project");
    assert_eq!(view.dispatch.phase, Phase::DoTask);
    assert_eq!(view.dispatch.cli, Cli::Claude);
    assert_eq!(view.mode, DispatchMode::Manual);
}

#[tokio::test]
async fn get_dispatch_after_create() {
    let orch = setup().await;
    let actor = sample_actor();
    let cmd = sample_command(actor.clone());
    let created = orch
        .create_dispatch(cmd, sample_replay_guard())
        .await
        .expect("create");

    let fetched = orch
        .get_dispatch_for_actor(&created.dispatch_id, &actor)
        .await
        .expect("get")
        .expect("should exist");

    assert_eq!(fetched.dispatch_id, created.dispatch_id);
    assert_eq!(fetched.status, DispatchStatus::Pending);
}

#[tokio::test]
async fn get_nonexistent_dispatch_returns_none() {
    let orch = setup().await;
    let result = orch
        .get_dispatch_for_actor(&DispatchId::new(), &sample_actor())
        .await
        .expect("get");
    assert!(result.is_none());
}

#[tokio::test]
#[cfg(feature = "test-hooks")]
async fn create_dispatch_enqueues_provision_step() {
    let orch = setup().await;
    let actor = sample_actor();
    let cmd = sample_command(actor);
    let created = orch
        .create_dispatch(cmd, sample_replay_guard())
        .await
        .expect("create");

    let steps = orch
        .store_for_tests()
        .get_steps_for_dispatch(&created.dispatch_id)
        .await
        .expect("steps");

    assert_eq!(steps.len(), 1, "expected exactly one step (provision)");
    let step = &steps[0];
    assert_eq!(step.dispatch_id, created.dispatch_id);
    assert_eq!(step.step_type, StepType::Provision);
    assert_eq!(step.step_sequence, 0);
    assert_eq!(step.status, StepStatus::Pending);
    assert_eq!(step.ready_state, StepReadyState::Ready);
}

#[tokio::test]
async fn list_dispatches_returns_created() {
    let orch = setup().await;
    let actor = sample_actor();

    for _ in 0..3 {
        let cmd = sample_command(actor.clone());
        orch.create_dispatch(cmd, sample_replay_guard())
            .await
            .expect("create");
    }

    let list = orch
        .list_dispatches_for_actor(DispatchFilter::new(), &actor)
        .await
        .expect("list");
    assert_eq!(list.dispatches.len(), 3);
    assert!(list.next_cursor.is_none());
}

#[tokio::test]
async fn list_dispatches_with_project_filter() {
    let orch = setup().await;
    let actor = sample_actor();
    let list_actor = actor.clone();

    let cmd = sample_command(actor.clone());
    orch.create_dispatch(cmd, sample_replay_guard())
        .await
        .expect("create");

    let mut cmd2 = sample_command(actor);
    cmd2.project = NonEmptyString::try_new("other-project".to_owned()).expect("non-empty");
    orch.create_dispatch(cmd2, sample_replay_guard())
        .await
        .expect("create");

    let mut filter = DispatchFilter::new();
    filter.project = Some("test-project".to_owned());
    let list = orch
        .list_dispatches_for_actor(filter, &list_actor)
        .await
        .expect("list");
    assert_eq!(list.dispatches.len(), 1);
    assert_eq!(list.dispatches[0].dispatch.project.as_str(), "test-project");
}

#[tokio::test]
async fn list_empty_returns_empty_vec() {
    let orch = setup().await;
    let list = orch
        .list_dispatches_for_actor(DispatchFilter::new(), &sample_actor())
        .await
        .expect("list");
    assert!(list.dispatches.is_empty());
    assert!(list.next_cursor.is_none());
}

#[tokio::test]
async fn cancel_dispatch_transitions_to_cancelled() {
    let orch = setup().await;
    let actor = sample_actor();
    let read_actor = actor.clone();
    let cmd = sample_command(actor.clone());
    let created = orch
        .create_dispatch(cmd, sample_replay_guard())
        .await
        .expect("create");

    let cancel_cmd = CancelDispatch {
        actor,
        dispatch_id: created.dispatch_id,
        reason: Some("test cancel".to_owned()),
    };
    orch.cancel_dispatch(cancel_cmd, sample_replay_guard())
        .await
        .expect("cancel");

    let view = orch
        .get_dispatch_for_actor(&created.dispatch_id, &read_actor)
        .await
        .expect("get")
        .expect("should exist");
    assert_eq!(view.status, DispatchStatus::Cancelled);
}

#[tokio::test]
async fn cancel_already_cancelled_returns_error() {
    let orch = setup().await;
    let actor = sample_actor();
    let cmd = sample_command(actor.clone());
    let created = orch
        .create_dispatch(cmd, sample_replay_guard())
        .await
        .expect("create");

    let cancel1 = CancelDispatch {
        actor: actor.clone(),
        dispatch_id: created.dispatch_id,
        reason: None,
    };
    orch.cancel_dispatch(cancel1, sample_replay_guard())
        .await
        .expect("first cancel");

    let cancel2 = CancelDispatch {
        actor,
        dispatch_id: created.dispatch_id,
        reason: None,
    };
    let err = orch
        .cancel_dispatch(cancel2, sample_replay_guard())
        .await
        .expect_err("second cancel should fail");

    assert!(
        matches!(
            err,
            OrchestratorError::Store(StoreError::InvalidTransition { .. })
        ),
        "expected store InvalidTransition, got: {err:?}"
    );
}

#[tokio::test]
async fn cancel_nonexistent_dispatch_returns_not_found() {
    let orch = setup().await;
    let cancel_cmd = CancelDispatch {
        actor: sample_actor(),
        dispatch_id: DispatchId::new(),
        reason: None,
    };
    let err = orch
        .cancel_dispatch(cancel_cmd, sample_replay_guard())
        .await
        .expect_err("should fail");

    assert!(
        matches!(err, OrchestratorError::Domain(DomainError::NotFound { .. })),
        "expected NotFound, got: {err:?}"
    );
}

#[tokio::test]
async fn create_emits_dispatch_created_and_step_enqueued() {
    let orch = setup().await;
    let actor = sample_actor();
    let cmd = sample_command(actor);
    let created = orch
        .create_dispatch(cmd, sample_replay_guard())
        .await
        .expect("create");

    let filter = EventFilter {
        entity_ref: Some(EntityRef::Dispatch(created.dispatch_id)),
        ..EventFilter::new()
    };
    let result = orch.query_events(&filter).await.expect("events");

    assert_eq!(
        result.events.len(),
        2,
        "expected DispatchCreated + StepEnqueued, got {} events",
        result.events.len()
    );
    assert!(
        matches!(
            result.events[0].payload,
            tanren_domain::DomainEvent::DispatchCreated { .. }
        ),
        "expected DispatchCreated, got: {:?}",
        result.events[0].payload
    );
    assert!(
        matches!(
            result.events[1].payload,
            tanren_domain::DomainEvent::StepEnqueued { .. }
        ),
        "expected StepEnqueued, got: {:?}",
        result.events[1].payload
    );
}

#[tokio::test]
async fn cancel_emits_dispatch_cancelled_not_failed() {
    let orch = setup().await;
    let actor = sample_actor();
    let cmd = sample_command(actor.clone());
    let created = orch
        .create_dispatch(cmd, sample_replay_guard())
        .await
        .expect("create");

    let cancel_cmd = CancelDispatch {
        actor,
        dispatch_id: created.dispatch_id,
        reason: Some("test".to_owned()),
    };
    orch.cancel_dispatch(cancel_cmd, sample_replay_guard())
        .await
        .expect("cancel");

    let filter = EventFilter {
        entity_ref: Some(EntityRef::Dispatch(created.dispatch_id)),
        ..EventFilter::new()
    };
    let result = orch.query_events(&filter).await.expect("events");

    // Create: DispatchCreated + StepEnqueued
    // Cancel: StepCancelled + DispatchCancelled = 4 total
    assert!(
        result.events.len() >= 3,
        "expected at least 3 events (DispatchCreated + StepEnqueued + DispatchCancelled), got {}",
        result.events.len()
    );

    // Last event must be DispatchCancelled
    let last = result.events.last().expect("at least one event");
    assert!(
        matches!(
            last.payload,
            tanren_domain::DomainEvent::DispatchCancelled { .. }
        ),
        "last event should be DispatchCancelled, got: {:?}",
        last.payload
    );

    // Must NOT contain DispatchFailed
    for event in &result.events {
        assert!(
            !matches!(
                event.payload,
                tanren_domain::DomainEvent::DispatchFailed { .. }
            ),
            "cancellation must not produce DispatchFailed"
        );
    }
}

#[tokio::test]
async fn finalize_success_emits_dispatch_completed() {
    let orch = setup().await;
    let actor = sample_actor();
    let created = orch
        .create_dispatch(sample_command(actor.clone()), sample_replay_guard())
        .await
        .expect("create");
    orch.start_dispatch(created.dispatch_id)
        .await
        .expect("start");
    orch.finalize_dispatch(
        created.dispatch_id,
        Outcome::Success,
        FiniteF64::try_new(3.2).expect("finite"),
        None,
        None,
        None,
    )
    .await
    .expect("finalize");

    let view = orch
        .get_dispatch_for_actor(&created.dispatch_id, &actor)
        .await
        .expect("get")
        .expect("exists");
    assert_eq!(view.status, DispatchStatus::Completed);
    assert_eq!(view.outcome, Some(Outcome::Success));

    let events = orch
        .query_events(&EventFilter {
            entity_ref: Some(EntityRef::Dispatch(created.dispatch_id)),
            ..EventFilter::new()
        })
        .await
        .expect("events");
    let last = events.events.last().expect("last");
    assert!(matches!(
        last.payload,
        tanren_domain::DomainEvent::DispatchCompleted {
            outcome: Outcome::Success,
            ..
        }
    ));
}

#[tokio::test]
async fn finalize_non_success_outcomes_emit_dispatch_failed() {
    let outcomes = [
        Outcome::Fail,
        Outcome::Blocked,
        Outcome::Error,
        Outcome::Timeout,
    ];

    for outcome in outcomes {
        let orch = setup().await;
        let actor = sample_actor();
        let created = orch
            .create_dispatch(sample_command(actor.clone()), sample_replay_guard())
            .await
            .expect("create");
        orch.start_dispatch(created.dispatch_id)
            .await
            .expect("start");
        orch.finalize_dispatch(
            created.dispatch_id,
            outcome,
            FiniteF64::try_new(1.0).expect("finite"),
            None,
            None,
            Some("failure".to_owned()),
        )
        .await
        .expect("finalize");

        let view = orch
            .get_dispatch_for_actor(&created.dispatch_id, &actor)
            .await
            .expect("get")
            .expect("exists");
        assert_eq!(view.status, DispatchStatus::Failed);
        assert_eq!(view.outcome, Some(outcome));

        let events = orch
            .query_events(&EventFilter {
                entity_ref: Some(EntityRef::Dispatch(created.dispatch_id)),
                ..EventFilter::new()
            })
            .await
            .expect("events");
        let last = events.events.last().expect("last");
        assert!(matches!(
            last.payload,
            tanren_domain::DomainEvent::DispatchFailed { outcome: o, .. } if o == outcome
        ));
    }
}

#[tokio::test]
async fn finalize_without_running_state_rejected() {
    let orch = setup().await;
    let created = orch
        .create_dispatch(sample_command(sample_actor()), sample_replay_guard())
        .await
        .expect("create");
    let err = orch
        .finalize_dispatch(
            created.dispatch_id,
            Outcome::Success,
            FiniteF64::try_new(1.0).expect("finite"),
            None,
            None,
            None,
        )
        .await
        .expect_err("finalize should fail from pending");

    assert!(
        matches!(
            err,
            OrchestratorError::Store(StoreError::InvalidTransition { .. })
        ),
        "expected invalid transition, got {err:?}"
    );
}
