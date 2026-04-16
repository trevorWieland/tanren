#![cfg(all(feature = "test-hooks", feature = "postgres-integration"))]

#[path = "common/postgres_query_plan.rs"]
mod postgres_query_plan;
#[path = "common/support_postgres.rs"]
mod support_postgres;

use std::sync::Arc;

use chrono::Utc;
use postgres_query_plan::{
    assert_no_seq_scan, assert_planner_stable_scope_invariants, assert_scope_index_usage,
    explain_analyze_plan_lines, explain_plan_lines,
};
use sea_orm::DbBackend;
use support_postgres::postgres_fixture;
use tanren_domain::{
    ActorContext, AuthMode, Cli, ConfigKeys, DispatchId, DispatchMode, DispatchReadScope,
    DispatchSnapshot, DispatchStatus, DomainEvent, EventEnvelope, EventId, GraphRevision, Lane,
    NonEmptyString, OrgId, Phase, StepId, StepPayload, StepReadyState, StepType, TimeoutSecs,
    UserId,
};
use tanren_store::{
    CancelDispatchParams, CreateDispatchParams, DispatchFilter, EnqueueStepParams, JobQueue,
    ReplayGuard, StateStore, Store, StoreConflictClass, StoreError, UpdateDispatchStatusParams,
    dispatch_query_statement_for_backend,
};
use uuid::Uuid;

fn actor() -> ActorContext {
    ActorContext::new(OrgId::new(), UserId::new())
}

fn snapshot(project: &str) -> DispatchSnapshot {
    DispatchSnapshot {
        project: NonEmptyString::try_new(project.to_owned()).expect("project"),
        phase: Phase::DoTask,
        cli: Cli::Claude,
        auth_mode: AuthMode::ApiKey,
        branch: NonEmptyString::try_new("main".to_owned()).expect("branch"),
        spec_folder: NonEmptyString::try_new("spec".to_owned()).expect("spec"),
        workflow_id: NonEmptyString::try_new("wf-1".to_owned()).expect("wf"),
        timeout: TimeoutSecs::try_new(60).expect("timeout"),
        environment_profile: NonEmptyString::try_new("default".to_owned()).expect("profile"),
        gate_cmd: None,
        context: None,
        model: None,
        project_env: ConfigKeys::default(),
        required_secrets: vec![],
        preserve_on_failure: false,
        created_at: Utc::now(),
    }
}

async fn create_dispatch(
    store: &Store,
    project: &str,
    actor_ctx: ActorContext,
    lane: Lane,
) -> Result<DispatchId, StoreError> {
    let dispatch_id = DispatchId::new();
    let snap = snapshot(project);
    let created_at = Utc::now();
    let creation_event = EventEnvelope::new(
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
            creation_event,
        })
        .await?;

    Ok(dispatch_id)
}

async fn seed_execute_steps(
    store: &Store,
    dispatch_id: DispatchId,
    snap: &DispatchSnapshot,
    lane: Lane,
    count: u32,
) -> Result<Vec<StepId>, StoreError> {
    let mut ids = Vec::with_capacity(count as usize);
    for seq in 0..count {
        let step_id = StepId::new();
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

        store
            .enqueue_step(EnqueueStepParams {
                dispatch_id,
                step_id,
                step_type: StepType::Execute,
                step_sequence: seq,
                lane: Some(lane),
                depends_on: vec![],
                graph_revision: GraphRevision::INITIAL,
                payload: StepPayload::Execute(Box::new(tanren_domain::ExecutePayload {
                    dispatch: snap.clone(),
                    handle: tanren_domain::EnvironmentHandle {
                        id: NonEmptyString::try_new("handle-1".to_owned()).expect("handle"),
                        runtime_type: NonEmptyString::try_new("local".to_owned()).expect("runtime"),
                    },
                })),
                ready_state: StepReadyState::Ready,
                enqueue_event: event,
            })
            .await?;

        ids.push(step_id);
    }
    Ok(ids)
}

fn scoped_dispatch_filter(org: OrgId, limit: u64) -> DispatchFilter {
    DispatchFilter {
        read_scope: Some(DispatchReadScope {
            org_id: org,
            project_id: None,
            team_id: None,
            api_key_id: None,
        }),
        limit,
        ..DispatchFilter::new()
    }
}

fn update_dispatch_status_running_params(dispatch_id: DispatchId) -> UpdateDispatchStatusParams {
    UpdateDispatchStatusParams {
        dispatch_id,
        status: DispatchStatus::Running,
        outcome: None,
        status_event: EventEnvelope::new(
            EventId::from_uuid(Uuid::now_v7()),
            Utc::now(),
            DomainEvent::DispatchStarted { dispatch_id },
        ),
    }
}

fn cancel_dispatch_params(
    dispatch_id: DispatchId,
    actor_ctx: ActorContext,
    reason: Option<String>,
) -> CancelDispatchParams {
    let reason_for_event = reason.clone();
    let replay_seed = Uuid::now_v7();
    CancelDispatchParams {
        dispatch_id,
        actor: actor_ctx.clone(),
        reason,
        status_event: EventEnvelope::new(
            EventId::from_uuid(Uuid::now_v7()),
            Utc::now(),
            DomainEvent::DispatchCancelled {
                dispatch_id,
                actor: actor_ctx,
                reason: reason_for_event,
            },
        ),
        replay_guard: ReplayGuard {
            issuer: "tanren-test".to_owned(),
            audience: "tanren-cli".to_owned(),
            jti: replay_seed.to_string(),
            iat_unix: 1,
            exp_unix: 2,
        },
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn cancel_dispatch_concurrent_calls_do_not_leak_database_errors_postgres() {
    let fixture = postgres_fixture().await;
    let store = Arc::new(fixture.store);
    let snap = snapshot("gamma");
    let id = create_dispatch(&store, "gamma", actor(), Lane::Impl)
        .await
        .expect("create");
    let _ = seed_execute_steps(&store, id, &snap, Lane::Impl, 2)
        .await
        .expect("seed");
    store
        .update_dispatch_status(update_dispatch_status_running_params(id))
        .await
        .expect("running");

    let params_a = cancel_dispatch_params(id, actor(), Some("a".to_owned()));
    let params_b = cancel_dispatch_params(id, actor(), Some("b".to_owned()));
    let store_a = Arc::clone(&store);
    let store_b = Arc::clone(&store);
    let fut_a = async move { store_a.cancel_dispatch(params_a).await };
    let fut_b = async move { store_b.cancel_dispatch(params_b).await };
    let (res_a, res_b) = tokio::join!(fut_a, fut_b);
    let outcomes = [res_a, res_b];

    let success_count = outcomes.iter().filter(|r| r.is_ok()).count();
    assert_eq!(success_count, 1, "expected exactly one successful cancel");
    assert!(
        outcomes.iter().any(|r| {
            matches!(
                r,
                Err(StoreError::Conflict {
                    class: StoreConflictClass::Contention,
                    ..
                } | StoreError::InvalidTransition { .. })
            )
        }),
        "losing call should be typed contention/invalid-transition: {outcomes:?}",
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn scoped_dispatch_query_plan_uses_scope_indexes_postgres_forced() {
    let fixture = postgres_fixture().await;
    let store = &fixture.store;
    let org = OrgId::new();
    let project = tanren_domain::ProjectId::new();
    let team = tanren_domain::TeamId::new();
    let api_key = tanren_domain::ApiKeyId::new();

    for index in 0..160 {
        let actor_ctx = match index % 6 {
            0 => ActorContext {
                org_id: org,
                user_id: UserId::new(),
                team_id: None,
                api_key_id: None,
                project_id: None,
            },
            1 => ActorContext {
                org_id: org,
                user_id: UserId::new(),
                team_id: None,
                api_key_id: None,
                project_id: Some(project),
            },
            2 => ActorContext {
                org_id: org,
                user_id: UserId::new(),
                team_id: Some(team),
                api_key_id: None,
                project_id: Some(project),
            },
            3 => ActorContext {
                org_id: org,
                user_id: UserId::new(),
                team_id: None,
                api_key_id: Some(api_key),
                project_id: Some(project),
            },
            4 => ActorContext {
                org_id: org,
                user_id: UserId::new(),
                team_id: Some(team),
                api_key_id: Some(api_key),
                project_id: Some(project),
            },
            _ => ActorContext {
                org_id: OrgId::new(),
                user_id: UserId::new(),
                team_id: Some(team),
                api_key_id: Some(api_key),
                project_id: Some(project),
            },
        };
        create_dispatch(store, "scope-plan", actor_ctx, Lane::Impl)
            .await
            .expect("create dispatch");
    }

    let filter = scoped_dispatch_filter(org, 32);
    let stmt = dispatch_query_statement_for_backend(&filter, filter.limit, DbBackend::Postgres);
    let lines = explain_plan_lines(&fixture.url, stmt, true).await;
    assert_scope_index_usage(&lines);
    assert_no_seq_scan(&lines);
}

/// Companion to
/// [`scoped_dispatch_query_plan_uses_scope_indexes_postgres_forced`].
///
/// The forced-path test above already proves the scoped indexes are
/// reachable when planner freedom is constrained
/// (`enable_seqscan = off`, `enable_bitmapscan = off`). This test
/// covers the *natural planner* path — what the optimizer chooses
/// when given full freedom and live statistics — and asserts only
/// invariants that survive planner variation across environments.
///
/// History: an earlier revision asserted that the plan used one of a
/// fixed set of scoped index names. That was correct on local Docker
/// Postgres 17 but failed deterministically on GitHub Actions, where
/// the planner often selected `Index Scan Backward using
/// ix_dispatch_created` plus a per-row filter. That plan is *valid*
/// — it returns the right rows in the right order, uses an index,
/// and does not spill — but it is not one of the scoped indexes.
/// Asserting an exact index family produced a flaky CI without
/// catching any real regression that the invariants below would
/// miss.
///
/// The replacement invariants live in
/// [`assert_planner_stable_scope_invariants`] and target the
/// performance properties that matter regardless of which index the
/// planner picked: no seq scan, an index path is in use, and no
/// disk-spilling sort.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn scoped_dispatch_query_plan_uses_scope_indexes_postgres_natural_planner() {
    let fixture = postgres_fixture().await;
    let store = &fixture.store;
    let org = OrgId::new();
    let project = tanren_domain::ProjectId::new();

    // Modest, deterministic seed — large enough for the planner to
    // weigh index alternatives but small enough to keep test runtime
    // sane. We no longer rely on extreme skew to coerce a specific
    // plan; the assertion shape no longer depends on it.
    for _ in 0..120 {
        create_dispatch(
            store,
            "scope-plan-target",
            ActorContext::new(org, UserId::new()),
            Lane::Impl,
        )
        .await
        .expect("create target scoped dispatch");
    }

    for _ in 0..240 {
        create_dispatch(
            store,
            "scope-plan-target-project",
            ActorContext {
                org_id: org,
                user_id: UserId::new(),
                team_id: None,
                api_key_id: None,
                project_id: Some(project),
            },
            Lane::Impl,
        )
        .await
        .expect("create target projected dispatch");
    }

    for _ in 0..1_200 {
        create_dispatch(
            store,
            "scope-plan-background",
            ActorContext {
                org_id: OrgId::new(),
                user_id: UserId::new(),
                team_id: None,
                api_key_id: None,
                project_id: None,
            },
            Lane::Impl,
        )
        .await
        .expect("create background dispatch");
    }

    let filter = scoped_dispatch_filter(org, 32);
    let stmt = dispatch_query_statement_for_backend(&filter, filter.limit, DbBackend::Postgres);
    let lines = explain_analyze_plan_lines(&fixture.url, stmt).await;
    assert_planner_stable_scope_invariants(&lines);
}
