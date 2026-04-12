//! Postgres-backed integration tests for `tanren-store`.
//!
//! Gated behind the `postgres-integration` cargo feature so it does
//! not run on every local `cargo nextest run`. Each test either
//! spins up a fresh `testcontainers::Postgres` container or
//! connects to `TANREN_TEST_POSTGRES_URL` (for CI where a service
//! container is already running). Tests share no state — each call
//! to [`postgres_store`] migrates a fresh schema.
//!
//! The most important test in this file is [`dequeue_is_race_safe`],
//! which exercises the `FOR UPDATE SKIP LOCKED` path that `SQLite`
//! cannot reproduce and is the single most critical correctness
//! guarantee the store provides.

#![cfg(feature = "postgres-integration")]

mod common;

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

// Nextest serializes this binary via the `postgres-integration`
// test group in `.config/nextest.toml`. Running tests one at a
// time sidesteps pg_catalog races when multiple tests all drop +
// recreate the `public` schema against the same `Postgres`
// instance in parallel.

use common::{
    ack_and_enqueue_execute, actor, assert_dispatch_status, create_dispatch,
    create_dispatch_params, duplicate_create_params, enqueue_step_params, execute_payload,
    execute_result, now, provision_payload, provision_result, seed_steps, snapshot,
    step_completed_event, try_dequeue,
};
use tanren_domain::{DispatchStatus, DomainEvent, EntityRef, Lane, StepId, StepPayload, StepType};
use tanren_store::{EventFilter, EventStore, JobQueue, StateStore, Store};
use testcontainers::ContainerAsync;
use testcontainers::runners::AsyncRunner;
use testcontainers_modules::postgres::Postgres as PostgresImage;

/// Container handle + connected store. Keep both around — dropping
/// the container shuts down the database.
struct Fixture {
    _container: Option<ContainerAsync<PostgresImage>>,
    store: Store,
}

/// Acquire a running Postgres and migrate a fresh schema. Uses
/// `TANREN_TEST_POSTGRES_URL` when set (CI path); otherwise spins up
/// a testcontainer.
async fn postgres_fixture() -> Fixture {
    if let Ok(url) = std::env::var("TANREN_TEST_POSTGRES_URL") {
        // Tests share the same database when running against CI's
        // service container; the nextest `postgres-integration`
        // test group enforces serial execution, so it is safe to
        // drop + recreate the `public` schema between tests.
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
    use sea_orm::{ConnectionTrait, Database};
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

    // 1. create_dispatch helper + get_dispatch + assert_dispatch_status
    let id = create_dispatch(store, "alpha", actor_ctx, Lane::Impl)
        .await
        .expect("create");
    assert_dispatch_status(store, &id, DispatchStatus::Pending).await;

    // 2. Duplicate create must fail cleanly without corrupting state.
    assert!(
        store
            .create_dispatch_projection(duplicate_create_params(id, dup_actor, Lane::Impl))
            .await
            .is_err()
    );

    // 3. Enqueue the provision step first (sequence 0). The dequeue
    //    order is `created_at ASC, step_sequence ASC`, so enqueuing
    //    provision before the execute seeds guarantees it is the
    //    first row claimed.
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
    // A small sleep guarantees the following execute seeds have
    // observably-later `created_at` timestamps, independent of
    // clock resolution.
    tokio::time::sleep(Duration::from_millis(10)).await;

    // 4. Seed a couple of execute steps via the seed_steps helper.
    let _seeded = seed_steps(store, id, &snap, Lane::Impl, 2)
        .await
        .expect("seed");

    // 5. First dequeue returns the provision step. Ack it with
    //    provision_result so both provision_result and ack paths
    //    are exercised.
    let claimed = try_dequeue(store, "worker-prov", Some(Lane::Impl), 99)
        .await
        .expect("dequeue")
        .expect("claim");
    assert_eq!(claimed.step_id, provision_step);
    assert!(matches!(claimed.payload, StepPayload::Provision(_)));
    store
        .ack(&claimed.step_id, &provision_result())
        .await
        .expect("ack provision");

    // 6. Pick one execute step, hand it off via ack_and_enqueue.
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

    // 7. Append a StepCompleted envelope via the helper and verify
    //    it lands.
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

    // 8. Build a DispatchStarted envelope inline using `now()` to
    //    exercise that helper.
    let started = tanren_domain::EventEnvelope {
        schema_version: tanren_domain::SCHEMA_VERSION,
        event_id: tanren_domain::EventId::from_uuid(uuid::Uuid::now_v7()),
        timestamp: now(),
        entity_ref: EntityRef::Dispatch(id),
        payload: DomainEvent::DispatchStarted { dispatch_id: id },
    };
    store.append(&started).await.expect("append started");

    let events = store
        .query_events(&EventFilter {
            limit: 100,
            ..EventFilter::new()
        })
        .await
        .expect("query");
    assert!(events.total_count >= 6);

    // Ensure `create_dispatch_params` is still directly invokable in
    // isolation (exercised above through `create_dispatch`, but
    // clippy wants an explicit reference so the helper isn't pruned).
    let _params = create_dispatch_params("second", actor(), Lane::Audit);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn dequeue_is_race_safe() {
    let fixture = postgres_fixture().await;
    let store = Arc::new(fixture.store);
    let snap = snapshot("alpha");
    let id = create_dispatch(&store, "alpha", actor(), Lane::Impl)
        .await
        .expect("create");

    // Seed 5 pending execute-lane steps.
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

    // Spawn 20 concurrent dequeues with max_concurrent = 5. Exactly
    // 5 must succeed; no step may be claimed twice.
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

/// B-01 regression: `dequeue(None)` counts running rows across ALL
/// lanes while `dequeue(Some(Impl))` counts only impl rows. With
/// a per-lane advisory lock this was a race; the global lock must
/// prevent over-claiming when these two call shapes are mixed.
#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn cross_lane_dequeue_respects_global_cap() {
    let fixture = postgres_fixture().await;
    let store = Arc::new(fixture.store);
    let snap = snapshot("alpha");
    let id = create_dispatch(&store, "alpha", actor(), Lane::Impl)
        .await
        .expect("create");

    // Seed 3 steps in the Impl lane.
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

    // Spawn 10 workers: half call dequeue(None, max_concurrent=1),
    // half call dequeue(Some(Impl), max_concurrent=1). With the
    // global lock, the total running count across the entire step
    // table never exceeds 1, because both call shapes share the
    // same serialization point.
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
    // `max_concurrent=1` means at most 1 step should be in
    // `running` state. All concurrent callers see the same cap
    // because the advisory lock serializes them.
    assert_eq!(
        claimed, 1,
        "cross-lane max_concurrent=1 must still allow exactly one claim"
    );
}
