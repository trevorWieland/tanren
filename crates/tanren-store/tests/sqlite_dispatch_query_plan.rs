#![cfg(feature = "test-hooks")]

//! Query-plan regression tests for scoped `SQLite` dispatch reads.

use chrono::{Duration, Utc};
use sea_orm::{ConnectionTrait, Database, DbBackend, Statement};
use tanren_domain::{
    ActorContext, AuthMode, ConfigKeys, DispatchId, DispatchMode, DispatchReadScope, DomainEvent,
    EventEnvelope, EventId, GraphRevision, Lane, NonEmptyString, OrgId, Phase, ProjectId,
    TimeoutSecs, UserId,
};
use tanren_store::{
    CreateDispatchParams, DispatchFilter, StateStore, Store, dispatch_query_statement_for_backend,
};

fn snapshot(project_name: &str) -> tanren_domain::DispatchSnapshot {
    tanren_domain::DispatchSnapshot {
        project: NonEmptyString::try_new(project_name.to_owned()).expect("project"),
        phase: Phase::DoTask,
        cli: tanren_domain::Cli::Claude,
        auth_mode: AuthMode::ApiKey,
        branch: NonEmptyString::try_new("main".to_owned()).expect("branch"),
        spec_folder: NonEmptyString::try_new("spec".to_owned()).expect("spec"),
        workflow_id: NonEmptyString::try_new("wf-1".to_owned()).expect("workflow"),
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

async fn create_dispatch_at(
    store: &Store,
    dispatch_id: DispatchId,
    actor: ActorContext,
    created_at: chrono::DateTime<Utc>,
) {
    let dispatch = snapshot("alpha");
    let creation_event = EventEnvelope::new(
        EventId::from_uuid(uuid::Uuid::now_v7()),
        created_at,
        DomainEvent::DispatchCreated {
            dispatch_id,
            dispatch: Box::new(dispatch.clone()),
            mode: DispatchMode::Manual,
            lane: Lane::Impl,
            actor: actor.clone(),
            graph_revision: GraphRevision::INITIAL,
        },
    );
    store
        .create_dispatch_projection(CreateDispatchParams {
            dispatch_id,
            mode: DispatchMode::Manual,
            lane: Lane::Impl,
            dispatch,
            actor,
            graph_revision: GraphRevision::INITIAL,
            created_at,
            creation_event,
        })
        .await
        .expect("create dispatch");
}

async fn seed_scope_fixture(store: &Store) -> DispatchReadScope {
    let org = OrgId::new();
    let base = Utc::now();

    for offset in 0..40 {
        create_dispatch_at(
            store,
            DispatchId::new(),
            ActorContext {
                org_id: org,
                user_id: UserId::new(),
                team_id: None,
                api_key_id: None,
                project_id: None,
            },
            base + Duration::milliseconds(i64::from(offset)),
        )
        .await;
    }

    for offset in 0..700 {
        create_dispatch_at(
            store,
            DispatchId::new(),
            ActorContext {
                org_id: org,
                user_id: UserId::new(),
                team_id: None,
                api_key_id: None,
                project_id: Some(ProjectId::new()),
            },
            base + Duration::milliseconds(i64::from(offset) + 50),
        )
        .await;
    }

    for offset in 0..300 {
        create_dispatch_at(
            store,
            DispatchId::new(),
            ActorContext {
                org_id: OrgId::new(),
                user_id: UserId::new(),
                team_id: None,
                api_key_id: None,
                project_id: None,
            },
            base + Duration::milliseconds(i64::from(offset) + 760),
        )
        .await;
    }

    DispatchReadScope {
        org_id: org,
        project_id: None,
        team_id: None,
        api_key_id: None,
    }
}

#[tokio::test]
async fn scoped_dispatch_query_plan_uses_scope_indexes_sqlite() {
    let db_path =
        std::env::temp_dir().join(format!("tanren-scope-plan-{}.db", uuid::Uuid::now_v7()));
    let db_url = format!("sqlite:{}?mode=rwc", db_path.display());

    let store = Store::new(&db_url).await.expect("connect");
    store.run_migrations().await.expect("migrate");
    let conn = Database::connect(&db_url)
        .await
        .expect("inspect connection");
    let scope = seed_scope_fixture(&store).await;
    conn.execute_unprepared("ANALYZE").await.expect("analyze");

    let filter = DispatchFilter {
        read_scope: Some(scope),
        limit: 20,
        ..DispatchFilter::new()
    };
    let stmt = dispatch_query_statement_for_backend(&filter, filter.limit, DbBackend::Sqlite);
    let explain_sql = format!("EXPLAIN QUERY PLAN {}", stmt.sql);
    let explain_stmt = match stmt.values {
        Some(values) => Statement::from_sql_and_values(DbBackend::Sqlite, explain_sql, values),
        None => Statement::from_string(DbBackend::Sqlite, explain_sql),
    };
    let rows = conn.query_all(explain_stmt).await.expect("explain plan");
    let details: Vec<String> = rows
        .into_iter()
        .map(|row| row.try_get("", "detail").expect("detail"))
        .collect();
    let details_upper = details
        .iter()
        .map(|detail| detail.to_ascii_uppercase())
        .collect::<Vec<_>>();

    assert!(
        details_upper
            .iter()
            .any(|detail| detail.contains("USING INDEX IX_DISPATCH_SCOPE_TUPLE_CREATED_DISPATCH")),
        "expected scoped index usage in plan: {details:?}"
    );
    assert!(
        details_upper
            .iter()
            .all(|detail| !detail.contains("USE TEMP B-TREE")),
        "expected no temp B-tree sort for scoped query: {details:?}"
    );
    let _ = std::fs::remove_file(db_path);
}
