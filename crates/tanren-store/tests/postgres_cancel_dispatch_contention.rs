#![cfg(feature = "postgres-integration")]

use std::sync::Arc;

use chrono::Utc;
use sea_orm::{ConnectionTrait, Database, DbBackend, Statement};
use tanren_domain::{
    ActorContext, AuthMode, Cli, ConfigKeys, DispatchId, DispatchMode, DispatchReadScope,
    DispatchSnapshot, DispatchStatus, DomainEvent, EventEnvelope, EventId, GraphRevision, Lane,
    NonEmptyString, OrgId, Phase, StepId, StepPayload, StepReadyState, StepType, TimeoutSecs,
    UserId,
};
use tanren_store::{
    CancelDispatchParams, CreateDispatchParams, DispatchFilter, EnqueueStepParams, JobQueue,
    StateStore, Store, StoreConflictClass, StoreError, UpdateDispatchStatusParams,
    dispatch_query_statement_for_backend,
};
use testcontainers::ContainerAsync;
use testcontainers::runners::AsyncRunner;
use testcontainers_modules::postgres::Postgres as PostgresImage;
use uuid::Uuid;

struct Fixture {
    _container: Option<ContainerAsync<PostgresImage>>,
    store: Store,
    url: String,
}

async fn postgres_fixture() -> Fixture {
    if let Ok(url) = std::env::var("TANREN_TEST_POSTGRES_URL") {
        reset_schema(&url).await;
        let store = migrate_fresh(&url).await;
        Fixture {
            _container: None,
            store,
            url,
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
            url,
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

fn explain_statement(stmt: Statement) -> Statement {
    let explain_sql = format!("EXPLAIN (COSTS FALSE) {}", stmt.sql);
    match stmt.values {
        Some(values) => Statement::from_sql_and_values(DbBackend::Postgres, explain_sql, values),
        None => Statement::from_string(DbBackend::Postgres, explain_sql),
    }
}

async fn explain_plan_lines(url: &str, stmt: Statement, force_index_path: bool) -> Vec<String> {
    let conn = Database::connect(url).await.expect("explain connection");
    conn.execute_unprepared("ANALYZE").await.expect("analyze");
    if force_index_path {
        conn.execute_unprepared("SET enable_seqscan = off")
            .await
            .expect("disable seqscan");
        conn.execute_unprepared("SET enable_bitmapscan = off")
            .await
            .expect("disable bitmap scans");
    }

    let rows = conn
        .query_all(explain_statement(stmt))
        .await
        .expect("explain plan");
    rows.into_iter()
        .map(|row| {
            row.try_get("", "QUERY PLAN")
                .or_else(|_| row.try_get("", "query_plan"))
                .expect("query plan line")
        })
        .collect()
}

fn assert_scope_index_usage(lines: &[String]) {
    const ACCEPTED_SCOPE_INDEX_NAMES: [&str; 6] = [
        "IX_DISPATCH_SCOPE_ORG_PROJECT_CREATED_DISPATCH",
        "IX_DISPATCH_SCOPE_TUPLE_CREATED_DISPATCH",
        "IX_DISPATCH_SCOPE_PROJECT",
        "IX_DISPATCH_SCOPE_TEAM",
        "IX_DISPATCH_SCOPE_API_KEY",
        "IX_DISPATCH_ORG_CREATED_DISPATCH",
    ];
    let lines_upper = lines
        .iter()
        .map(|line| line.to_ascii_uppercase())
        .collect::<Vec<_>>();
    assert!(
        lines_upper.iter().any(|line| {
            line.contains("INDEX")
                && ACCEPTED_SCOPE_INDEX_NAMES
                    .iter()
                    .any(|index_name| line.contains(index_name))
        }),
        "expected scoped index in postgres plan: {lines:?}"
    );
}

fn assert_no_seq_scan(lines: &[String]) {
    let lines_upper = lines
        .iter()
        .map(|line| line.to_ascii_uppercase())
        .collect::<Vec<_>>();
    assert!(
        lines_upper.iter().all(|line| !line.contains("SEQ SCAN")),
        "expected no sequential scan for scoped query: {lines:?}"
    );
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

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn scoped_dispatch_query_plan_uses_scope_indexes_postgres_natural_planner() {
    let fixture = postgres_fixture().await;
    let store = &fixture.store;
    let org = OrgId::new();
    let project = tanren_domain::ProjectId::new();

    for _ in 0..140 {
        create_dispatch(
            store,
            "scope-plan-target",
            ActorContext::new(org, UserId::new()),
            Lane::Impl,
        )
        .await
        .expect("create target scoped dispatch");
    }

    for _ in 0..600 {
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

    for _ in 0..1200 {
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
    let lines = explain_plan_lines(&fixture.url, stmt, false).await;
    assert_scope_index_usage(&lines);
}
