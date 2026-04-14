//! `SQLite`-backed integration tests for `tanren-store`.

mod common;

use std::time::Duration;

use common::{
    ack_and_enqueue_execute, ack_params, actor, assert_dispatch_status,
    cancel_pending_steps_params, create_dispatch, create_dispatch_params, duplicate_create_params,
    enqueue_step_params, execute_payload, execute_result, now, provision_payload, provision_result,
    seed_steps, snapshot, step_completed_event, try_dequeue, update_dispatch_status_params,
};
use tanren_domain::{
    DispatchStatus, DomainEvent, EntityKind, EntityRef, Lane, Outcome, StepId, StepPayload,
    StepStatus, StepType,
};
use tanren_store::{
    DispatchFilter, EventFilter, EventStore, JobQueue, NackParams, StateStore, Store,
};

/// Build a fresh in-memory `SQLite`-backed `Store`. Kept local to
/// this binary so the common module stays backend-agnostic.
async fn fresh_store() -> Store {
    let store = Store::new("sqlite::memory:").await.expect("connect");
    store.run_migrations().await.expect("migrate");
    store
}

#[tokio::test]
async fn dispatch_create_get_and_status_lifecycle() {
    let store = fresh_store().await;
    store.run_migrations().await.expect("idempotent");
    let _ = now();
    let actor_ctx = actor();
    let user_id = actor_ctx.user_id;
    let dup_actor = actor_ctx.clone();
    let id = create_dispatch(&store, "alpha", actor_ctx, Lane::Impl)
        .await
        .expect("create");
    assert!(
        store
            .create_dispatch_projection(duplicate_create_params(id, dup_actor, Lane::Impl))
            .await
            .is_err()
    );

    let view = store.get_dispatch(&id).await.expect("get").expect("exists");
    assert_eq!(view.dispatch_id, id);
    assert_eq!(view.status, DispatchStatus::Pending);
    assert_eq!(view.lane, Lane::Impl);
    assert_eq!(view.actor.user_id, user_id);

    store
        .update_dispatch_status(update_dispatch_status_params(
            id,
            DispatchStatus::Running,
            None,
        ))
        .await
        .expect("running");
    assert_dispatch_status(&store, &id, DispatchStatus::Running).await;
    store
        .update_dispatch_status(update_dispatch_status_params(
            id,
            DispatchStatus::Completed,
            Some(Outcome::Success),
        ))
        .await
        .expect("completed");
    let view = store.get_dispatch(&id).await.expect("get").expect("exists");
    assert_eq!(view.status, DispatchStatus::Completed);
    assert_eq!(view.outcome, Some(Outcome::Success));
}

#[tokio::test]
async fn query_dispatches_filters_by_lane_and_user() {
    let store = fresh_store().await;
    let actor_a = actor();
    let actor_b = actor();
    let actor_b_user = actor_b.user_id;
    let id_a = create_dispatch(&store, "alpha", actor_a, Lane::Impl)
        .await
        .expect("a");
    let _b = create_dispatch(&store, "alpha", actor_b, Lane::Audit)
        .await
        .expect("b");

    let by_lane = store
        .query_dispatches(&DispatchFilter {
            lane: Some(Lane::Impl),
            limit: 10,
            ..DispatchFilter::new()
        })
        .await
        .expect("by lane");
    assert_eq!(by_lane.len(), 1);
    assert_eq!(by_lane[0].dispatch_id, id_a);

    let by_user = store
        .query_dispatches(&DispatchFilter {
            user_id: Some(actor_b_user),
            limit: 10,
            ..DispatchFilter::new()
        })
        .await
        .expect("by user");
    assert_eq!(by_user.len(), 1);
    assert_eq!(by_user[0].actor.user_id, actor_b_user);
}

#[tokio::test]
async fn event_append_query_and_filter() {
    let store = fresh_store().await;
    let params = create_dispatch_params("alpha", actor(), Lane::Impl);
    let creation_event_id = params.creation_event.event_id;
    let dispatch_id = params.dispatch_id;
    store
        .create_dispatch_projection(params)
        .await
        .expect("create");

    let started = tanren_domain::EventEnvelope::new(
        tanren_domain::EventId::from_uuid(uuid::Uuid::now_v7()),
        now(),
        DomainEvent::DispatchStarted { dispatch_id },
    );
    let completed = step_completed_event(
        dispatch_id,
        StepId::new(),
        StepType::Execute,
        &execute_result(),
    );
    store
        .append_batch(&[started.clone(), completed.clone()])
        .await
        .expect("batch");
    store.append_batch(&[]).await.expect("empty batch");

    let by_ref = store
        .query_events(&EventFilter {
            entity_ref: Some(EntityRef::Dispatch(dispatch_id)),
            limit: 10,
            ..EventFilter::new()
        })
        .await
        .expect("by ref");
    assert_eq!(by_ref.total_count, 3); // creation + started + completed (all route to Dispatch)
    assert!(!by_ref.has_more);
    let ids: Vec<_> = by_ref.events.iter().map(|e| e.event_id).collect();
    assert!(ids.contains(&creation_event_id));
    assert!(ids.contains(&started.event_id));

    let by_kind = store
        .query_events(&EventFilter {
            entity_kind: Some(EntityKind::Dispatch),
            limit: 10,
            ..EventFilter::new()
        })
        .await
        .expect("by kind");
    assert_eq!(by_kind.total_count, 3);

    let by_type = store
        .query_events(&EventFilter {
            event_type: Some("step_completed".to_owned()),
            limit: 10,
            ..EventFilter::new()
        })
        .await
        .expect("by type");
    assert_eq!(by_type.total_count, 1);
}

#[tokio::test]
async fn enqueue_dequeue_ack_lifecycle() {
    let store = fresh_store().await;
    let snap = snapshot("alpha");
    let id = create_dispatch(&store, "alpha", actor(), Lane::Impl)
        .await
        .expect("create");
    let step_id = StepId::new();
    store
        .enqueue_step(enqueue_step_params(
            id,
            step_id,
            StepType::Provision,
            0,
            Some(Lane::Impl),
            provision_payload(snap),
        ))
        .await
        .expect("enqueue");

    let claimed = try_dequeue(&store, "w1", Some(Lane::Impl), 1)
        .await
        .expect("dequeue")
        .expect("claim");
    assert_eq!(claimed.step_id, step_id);
    assert!(matches!(claimed.payload, StepPayload::Provision(_)));

    let none = try_dequeue(&store, "w2", Some(Lane::Impl), 1)
        .await
        .expect("dequeue");
    assert!(none.is_none(), "max_concurrent saturated");

    store
        .ack(ack_params(
            id,
            step_id,
            StepType::Provision,
            provision_result(),
        ))
        .await
        .expect("ack");
    let view = store
        .get_step(&step_id)
        .await
        .expect("get")
        .expect("exists");
    assert_eq!(view.status, StepStatus::Completed);
    assert!(view.result.is_some());

    let count = store
        .count_running_steps(Some(&Lane::Impl))
        .await
        .expect("count");
    assert_eq!(count, 0);
}

#[tokio::test]
async fn ack_and_enqueue_is_atomic() {
    let store = fresh_store().await;
    let snap = snapshot("alpha");
    let id = create_dispatch(&store, "alpha", actor(), Lane::Impl)
        .await
        .expect("create");
    let step_a = StepId::new();
    store
        .enqueue_step(enqueue_step_params(
            id,
            step_a,
            StepType::Execute,
            0,
            Some(Lane::Impl),
            execute_payload(snap.clone()),
        ))
        .await
        .expect("enqueue a");
    let _ = try_dequeue(&store, "w1", Some(Lane::Impl), 1)
        .await
        .expect("dequeue");

    let step_b = StepId::new();
    store
        .ack_and_enqueue(ack_and_enqueue_execute(
            id,
            step_a,
            StepType::Execute,
            &snap,
            step_b,
            1,
            Some(Lane::Impl),
        ))
        .await
        .expect("ack_and_enqueue");

    let view_a = store
        .get_step(&step_a)
        .await
        .expect("get a")
        .expect("a exists");
    assert_eq!(view_a.status, StepStatus::Completed);
    let view_b = store
        .get_step(&step_b)
        .await
        .expect("get b")
        .expect("b exists");
    assert_eq!(view_b.status, StepStatus::Pending);

    // 1 create + 1 enqueue(a) + 1 dequeue(a) + 1 completion(a) + 1 enqueue(b) = 5
    let events = store
        .query_events(&EventFilter {
            limit: 100,
            ..EventFilter::new()
        })
        .await
        .expect("query");
    assert_eq!(events.total_count, 5);
}

#[tokio::test]
async fn ack_and_enqueue_rolls_back_on_pk_collision() {
    let store = fresh_store().await;
    let snap = snapshot("alpha");
    let id = create_dispatch(&store, "alpha", actor(), Lane::Impl)
        .await
        .expect("create");
    let step_a = StepId::new();
    store
        .enqueue_step(enqueue_step_params(
            id,
            step_a,
            StepType::Execute,
            0,
            Some(Lane::Impl),
            execute_payload(snap.clone()),
        ))
        .await
        .expect("enqueue a");
    let _ = try_dequeue(&store, "w1", Some(Lane::Impl), 1)
        .await
        .expect("dequeue");

    // Trigger a PK collision by making the "next step" reuse step_a's id.
    let mut params = ack_and_enqueue_execute(
        id,
        step_a,
        StepType::Execute,
        &snap,
        step_a,
        1,
        Some(Lane::Impl),
    );
    if let Some(ref mut next) = params.next_step {
        next.step_id = step_a;
        next.enqueue_event = enqueue_step_params(
            id,
            step_a,
            StepType::Execute,
            1,
            Some(Lane::Impl),
            execute_payload(snap.clone()),
        )
        .enqueue_event;
    }
    assert!(store.ack_and_enqueue(params).await.is_err());

    let view = store.get_step(&step_a).await.expect("get").expect("exists");
    assert_eq!(
        view.status,
        StepStatus::Running,
        "ack must have rolled back"
    );

    let events = store
        .query_events(&EventFilter {
            event_type: Some("step_completed".to_owned()),
            limit: 10,
            ..EventFilter::new()
        })
        .await
        .expect("query");
    assert_eq!(events.total_count, 0);
}

#[tokio::test]
async fn cancel_pending_steps_excludes_teardown() {
    let store = fresh_store().await;
    let snap = snapshot("alpha");
    let id = create_dispatch(&store, "alpha", actor(), Lane::Impl)
        .await
        .expect("create");
    let _ = seed_steps(&store, id, &snap, Lane::Impl, 3, 0)
        .await
        .expect("seed");
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
                    id: tanren_domain::NonEmptyString::try_new("h".to_owned()).expect("h"),
                    runtime_type: tanren_domain::NonEmptyString::try_new("local".to_owned())
                        .expect("r"),
                },
                preserve: false,
            })),
        ))
        .await
        .expect("teardown enqueue");

    assert_eq!(
        store
            .cancel_pending_steps(cancel_pending_steps_params(id))
            .await
            .expect("cancel"),
        3
    );
    let tv = store
        .get_step(&teardown_id)
        .await
        .expect("get")
        .expect("exists");
    assert_eq!(tv.status, StepStatus::Pending);
}

#[tokio::test]
async fn nack_retry_resets_to_pending_and_bumps_count() {
    let store = fresh_store().await;
    let snap = snapshot("alpha");
    let id = create_dispatch(&store, "alpha", actor(), Lane::Impl)
        .await
        .expect("create");
    let step_id = StepId::new();
    store
        .enqueue_step(enqueue_step_params(
            id,
            step_id,
            StepType::Execute,
            0,
            Some(Lane::Impl),
            execute_payload(snap),
        ))
        .await
        .expect("enqueue");
    let _ = try_dequeue(&store, "w1", Some(Lane::Impl), 1)
        .await
        .expect("dequeue");

    let failure_event = tanren_domain::EventEnvelope::new(
        tanren_domain::EventId::from_uuid(uuid::Uuid::now_v7()),
        now(),
        DomainEvent::StepFailed {
            dispatch_id: id,
            step_id,
            step_type: StepType::Execute,
            error: "boom".to_owned(),
            error_class: tanren_domain::ErrorClass::Transient,
            retry_count: 1,
            duration_secs: tanren_domain::FiniteF64::try_new(0.5).expect("finite"),
        },
    );
    store
        .nack(NackParams {
            dispatch_id: id,
            step_id,
            step_type: StepType::Execute,
            error: "boom".to_owned(),
            error_class: tanren_domain::ErrorClass::Transient,
            retry: true,
            failure_event,
        })
        .await
        .expect("nack");

    let view = store
        .get_step(&step_id)
        .await
        .expect("get")
        .expect("exists");
    assert_eq!(view.status, StepStatus::Pending);
    assert_eq!(view.retry_count, 1);
    assert!(view.error.is_none(), "retry=true must clear error");
}

#[tokio::test]
async fn recover_stale_steps_resets_long_running() {
    let store = fresh_store().await;
    let snap = snapshot("alpha");
    let id = create_dispatch(&store, "alpha", actor(), Lane::Impl)
        .await
        .expect("create");
    let step_id = StepId::new();
    store
        .enqueue_step(enqueue_step_params(
            id,
            step_id,
            StepType::Execute,
            0,
            Some(Lane::Impl),
            execute_payload(snap),
        ))
        .await
        .expect("enqueue");
    let _ = try_dequeue(&store, "w1", Some(Lane::Impl), 1)
        .await
        .expect("dequeue");

    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(store.recover_stale_steps(0).await.expect("recover"), 1);

    let view = store
        .get_step(&step_id)
        .await
        .expect("get")
        .expect("exists");
    assert_eq!(view.status, StepStatus::Pending);
    assert!(view.worker_id.is_none());
}
