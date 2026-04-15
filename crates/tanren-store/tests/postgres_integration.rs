#![cfg(feature = "postgres-integration")]

mod common;

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use common::{
    ack_and_enqueue_execute, ack_params, actor, assert_dispatch_status,
    cancel_pending_steps_params, create_dispatch, create_dispatch_params, duplicate_create_params,
    enqueue_step_params, execute_payload, execute_result, now, provision_payload, provision_result,
    seed_steps, snapshot, step_completed_event, try_dequeue, update_dispatch_status_params,
};
use sea_orm::{ConnectionTrait, Database};
use tanren_domain::{DispatchStatus, DomainEvent, Lane, StepId, StepPayload, StepType};
use tanren_store::{EventFilter, EventStore, JobQueue, StateStore, Store};
use testcontainers::ContainerAsync;
use testcontainers::runners::AsyncRunner;
use testcontainers_modules::postgres::Postgres as PostgresImage;

struct Fixture {
    _container: Option<ContainerAsync<PostgresImage>>,
    store: Store,
}

async fn postgres_fixture() -> Fixture {
    if let Ok(url) = std::env::var("TANREN_TEST_POSTGRES_URL") {
        reset_schema(&url).await;
        let store = migrate_fresh(&url).await;
        Fixture {
            _container: None,
            store,
        }
    } else {
        let container = PostgresImage::default()
            .start()
            .await
            .expect("start postgres container");
        let host = container.get_host().await.expect("host");
        let port = container.get_host_port_ipv4(5432).await.expect("port");
        let url = format!("postgres://postgres:postgres@{host}:{port}/postgres");
        let store = migrate_fresh(&url).await;
        Fixture {
            _container: Some(container),
            store,
        }
    }
}

async fn migrate_fresh(url: &str) -> Store {
    let store = Store::new(url).await.expect("connect to postgres");
    store.run_migrations().await.expect("run migrations");
    store
}

async fn reset_schema(url: &str) {
    let conn = Database::connect(url).await.expect("bootstrap connect");
    conn.execute_unprepared("DROP SCHEMA public CASCADE; CREATE SCHEMA public;")
        .await
        .expect("reset schema");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn full_lifecycle_passes_on_postgres() {
    let fixture = postgres_fixture().await;
    let store = &fixture.store;
    let snap = snapshot("alpha");
    let actor_ctx = actor();
    let dup_actor = actor_ctx.clone();

    let id = create_dispatch(store, "alpha", actor_ctx, Lane::Impl)
        .await
        .expect("create");
    assert_dispatch_status(store, &id, DispatchStatus::Pending).await;

    assert!(
        store
            .create_dispatch_projection(duplicate_create_params(id, dup_actor, Lane::Impl))
            .await
            .is_err()
    );

    let provision_step = StepId::new();
    store
        .enqueue_step(enqueue_step_params(
            id,
            provision_step,
            StepType::Provision,
            0,
            Some(Lane::Impl),
            provision_payload(snap.clone()),
        ))
        .await
        .expect("provision enqueue");
    tokio::time::sleep(Duration::from_millis(10)).await;

    let _seeded = seed_steps(store, id, &snap, Lane::Impl, 2, 1)
        .await
        .expect("seed");

    let claimed = try_dequeue(store, "worker-prov", Some(Lane::Impl), 99)
        .await
        .expect("dequeue")
        .expect("claim");
    assert_eq!(claimed.step_id, provision_step);
    assert!(matches!(claimed.payload, StepPayload::Provision(_)));
    store
        .ack(ack_params(
            id,
            claimed.step_id,
            StepType::Provision,
            provision_result(),
        ))
        .await
        .expect("ack provision");

    let execute_step = try_dequeue(store, "worker-exec", Some(Lane::Impl), 99)
        .await
        .expect("dequeue")
        .expect("claim");
    let next_step = StepId::new();
    store
        .ack_and_enqueue(ack_and_enqueue_execute(
            id,
            execute_step.step_id,
            StepType::Execute,
            &snap,
            next_step,
            42,
            Some(Lane::Impl),
        ))
        .await
        .expect("ack_and_enqueue");

    let standalone_completed = step_completed_event(
        id,
        execute_step.step_id,
        StepType::Execute,
        &execute_result(),
    );
    store
        .append(&standalone_completed)
        .await
        .expect("append completed");

    let started = tanren_domain::EventEnvelope::new(
        tanren_domain::EventId::from_uuid(uuid::Uuid::now_v7()),
        now(),
        DomainEvent::DispatchStarted { dispatch_id: id },
    );
    store.append(&started).await.expect("append started");

    let events = store
        .query_events(&EventFilter {
            limit: 100,
            ..EventFilter::new()
        })
        .await
        .expect("query");
    assert!(events.total_count >= 8);

    let _params = create_dispatch_params("second", actor(), Lane::Audit);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn dispatch_status_and_cancel_on_postgres() {
    let fixture = postgres_fixture().await;
    let store = &fixture.store;
    let snap = snapshot("beta");
    let id = create_dispatch(store, "beta", actor(), Lane::Impl)
        .await
        .expect("create");
    let _ = seed_steps(store, id, &snap, Lane::Impl, 3, 0)
        .await
        .expect("seed");

    store
        .update_dispatch_status(update_dispatch_status_params(
            id,
            DispatchStatus::Running,
            None,
        ))
        .await
        .expect("update status");
    assert_dispatch_status(store, &id, DispatchStatus::Running).await;

    let cancelled = store
        .cancel_pending_steps(cancel_pending_steps_params(id))
        .await
        .expect("cancel");
    assert_eq!(cancelled, 3);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn dequeue_is_race_safe() {
    let fixture = postgres_fixture().await;
    let store = Arc::new(fixture.store);
    let snap = snapshot("alpha");
    let id = create_dispatch(&store, "alpha", actor(), Lane::Impl)
        .await
        .expect("create");

    let mut seeded = Vec::new();
    for seq in 0..5 {
        let step_id = StepId::new();
        store
            .enqueue_step(enqueue_step_params(
                id,
                step_id,
                StepType::Execute,
                seq,
                Some(Lane::Impl),
                execute_payload(snap.clone()),
            ))
            .await
            .expect("enqueue");
        seeded.push(step_id);
    }

    let mut handles = Vec::new();
    for n in 0..20 {
        let store = Arc::clone(&store);
        let worker_id = format!("worker-{n}");
        handles.push(tokio::spawn(async move {
            try_dequeue(&store, &worker_id, Some(Lane::Impl), 5)
                .await
                .expect("dequeue")
        }));
    }

    let mut claimed: Vec<StepId> = Vec::new();
    for handle in handles {
        if let Some(queued) = handle.await.expect("join") {
            claimed.push(queued.step_id);
        }
    }

    assert_eq!(
        claimed.len(),
        5,
        "exactly 5 claims should have been awarded"
    );
    let unique: HashSet<_> = claimed.iter().copied().collect();
    assert_eq!(
        unique.len(),
        claimed.len(),
        "no step should be claimed by two workers"
    );
    for claimed_id in &claimed {
        assert!(
            seeded.contains(claimed_id),
            "claim must be one of the seeded steps"
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn dequeue_respects_max_concurrent_one() {
    let fixture = postgres_fixture().await;
    let store = Arc::new(fixture.store);
    let snap = snapshot("alpha");
    let id = create_dispatch(&store, "alpha", actor(), Lane::Impl)
        .await
        .expect("create");
    for seq in 0..5 {
        store
            .enqueue_step(enqueue_step_params(
                id,
                StepId::new(),
                StepType::Execute,
                seq,
                Some(Lane::Impl),
                execute_payload(snap.clone()),
            ))
            .await
            .expect("enqueue");
    }

    let mut handles = Vec::new();
    for n in 0..10 {
        let store = Arc::clone(&store);
        handles.push(tokio::spawn(async move {
            try_dequeue(&store, &format!("w{n}"), Some(Lane::Impl), 1)
                .await
                .expect("dequeue")
        }));
    }

    let mut claimed = 0;
    for handle in handles {
        if handle.await.expect("join").is_some() {
            claimed += 1;
        }
    }
    assert_eq!(claimed, 1, "max_concurrent=1 allows exactly one claim");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn cross_lane_dequeue_respects_global_cap() {
    let fixture = postgres_fixture().await;
    let store = Arc::new(fixture.store);
    let snap = snapshot("alpha");
    let id = create_dispatch(&store, "alpha", actor(), Lane::Impl)
        .await
        .expect("create");

    for seq in 0..3 {
        store
            .enqueue_step(enqueue_step_params(
                id,
                StepId::new(),
                StepType::Execute,
                seq,
                Some(Lane::Impl),
                execute_payload(snap.clone()),
            ))
            .await
            .expect("enqueue");
    }

    let mut handles = Vec::new();
    for n in 0..10 {
        let store = Arc::clone(&store);
        let lane = if n % 2 == 0 { None } else { Some(Lane::Impl) };
        handles.push(tokio::spawn(async move {
            try_dequeue(&store, &format!("w{n}"), lane, 1)
                .await
                .expect("dequeue")
        }));
    }

    let mut claimed = 0;
    for handle in handles {
        if handle.await.expect("join").is_some() {
            claimed += 1;
        }
    }
    assert_eq!(
        claimed, 1,
        "cross-lane max_concurrent=1 must still allow exactly one claim"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn different_lanes_dequeue_in_parallel() {
    let fixture = postgres_fixture().await;
    let store = Arc::new(fixture.store);
    let snap = snapshot("alpha");
    let id = create_dispatch(&store, "alpha", actor(), Lane::Impl)
        .await
        .expect("create");

    for seq in 0..3_u32 {
        store
            .enqueue_step(enqueue_step_params(
                id,
                StepId::new(),
                StepType::Execute,
                seq,
                Some(Lane::Impl),
                execute_payload(snap.clone()),
            ))
            .await
            .expect("enqueue impl");
        store
            .enqueue_step(enqueue_step_params(
                id,
                StepId::new(),
                StepType::Execute,
                10 + seq,
                Some(Lane::Audit),
                execute_payload(snap.clone()),
            ))
            .await
            .expect("enqueue audit");
    }

    let mut handles = Vec::new();
    for n in 0..10 {
        let store = Arc::clone(&store);
        let lane = if n % 2 == 0 {
            Some(Lane::Impl)
        } else {
            Some(Lane::Audit)
        };
        handles.push(tokio::spawn(async move {
            try_dequeue(&store, &format!("w{n}"), lane, 3)
                .await
                .expect("dequeue")
        }));
    }

    let mut claimed = 0;
    for handle in handles {
        if handle.await.expect("join").is_some() {
            claimed += 1;
        }
    }
    assert_eq!(
        claimed, 6,
        "different lanes must dequeue in parallel: expected 6 total claims"
    );
}
