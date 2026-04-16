//! Lean list-path coverage for `StateStore::query_dispatch_summaries`.
//!
//! Directly validates the audit-finding 4 guarantee that the summary
//! query does not depend on the JSON `dispatch` / `actor` columns by
//! corrupting them with non-conforming JSON and asserting:
//!
//! - `query_dispatch_summaries` still succeeds (scalar-only path).
//! - `query_dispatches` fails on the same row (heavy path does decode).
//!
//! Defines its own minimal helpers rather than importing the shared
//! `common` module, matching the pattern used by `sqlite_dequeue.rs`.

#![cfg(feature = "test-hooks")]

use chrono::Utc;
use sea_orm::{ConnectionTrait, Database, DbBackend, Statement};
use tanren_domain::{
    ActorContext, AuthMode, Cli, ConfigKeys, DispatchId, DispatchMode, DispatchSnapshot,
    DomainEvent, EventEnvelope, EventId, GraphRevision, Lane, NonEmptyString, OrgId, Phase,
    TimeoutSecs, UserId,
};
use tanren_store::{CreateDispatchParams, DispatchFilter, StateStore, Store};
use uuid::Uuid;

fn temp_sqlite_url() -> (String, std::path::PathBuf) {
    let path = std::env::temp_dir().join(format!("tanren-summary-{}.db", Uuid::now_v7()));
    let url = format!("sqlite:{}?mode=rwc", path.display());
    (url, path)
}

fn snapshot(project: &str) -> DispatchSnapshot {
    DispatchSnapshot {
        project: NonEmptyString::try_new(project.to_owned()).expect("project"),
        phase: Phase::DoTask,
        cli: Cli::Claude,
        auth_mode: AuthMode::ApiKey,
        branch: NonEmptyString::try_new("main".to_owned()).expect("branch"),
        spec_folder: NonEmptyString::try_new("spec".to_owned()).expect("spec"),
        workflow_id: NonEmptyString::try_new("wf".to_owned()).expect("wf"),
        timeout: TimeoutSecs::try_new(60).expect("timeout"),
        environment_profile: NonEmptyString::try_new("default".to_owned()).expect("env"),
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

async fn seed_dispatch(store: &Store, project: &str) -> DispatchId {
    let dispatch_id = DispatchId::new();
    let actor = actor();
    let snap = snapshot(project);
    let created_at = Utc::now();
    let event = EventEnvelope::new(
        EventId::from_uuid(Uuid::now_v7()),
        created_at,
        DomainEvent::DispatchCreated {
            dispatch_id,
            dispatch: Box::new(snap.clone()),
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
            dispatch: snap,
            actor,
            graph_revision: GraphRevision::INITIAL,
            created_at,
            creation_event: event,
        })
        .await
        .expect("create dispatch");
    dispatch_id
}

#[tokio::test]
async fn query_dispatch_summaries_returns_created_rows() {
    let (url, path) = temp_sqlite_url();
    let store = Store::open_and_migrate(&url).await.expect("store");
    let _id = seed_dispatch(&store, "alpha").await;

    let page = store
        .query_dispatch_summaries(&DispatchFilter {
            limit: 100,
            ..DispatchFilter::new()
        })
        .await
        .expect("summaries");
    assert_eq!(page.summaries.len(), 1);
    assert_eq!(page.summaries[0].project.as_str(), "alpha");

    drop(store);
    let _ = std::fs::remove_file(&path);
}

#[tokio::test]
async fn query_dispatch_summaries_skips_json_snapshot_decode() {
    let (url, path) = temp_sqlite_url();
    let store = Store::open_and_migrate(&url).await.expect("store");
    let dispatch_id = seed_dispatch(&store, "alpha").await;

    // Corrupt the JSON columns with a document that cannot be
    // deserialized into `DispatchSnapshot` or `ActorContext`. A
    // separate connection opens the same sqlite file so the update
    // targets the real projection row.
    let raw_conn = Database::connect(&url).await.expect("raw connect");
    raw_conn
        .execute(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "UPDATE dispatch_projection SET dispatch = ?, actor = ? WHERE dispatch_id = ?",
            [
                "{\"unexpected\":\"snapshot shape\"}".into(),
                "{\"unexpected\":\"actor shape\"}".into(),
                dispatch_id.into_uuid().into(),
            ],
        ))
        .await
        .expect("corrupt json columns");

    // Summary path is scalar-only and must succeed.
    let page = store
        .query_dispatch_summaries(&DispatchFilter {
            limit: 100,
            ..DispatchFilter::new()
        })
        .await
        .expect("summary query must not touch json columns");
    assert_eq!(page.summaries.len(), 1);
    assert_eq!(page.summaries[0].project.as_str(), "alpha");

    // The heavy path must still fail on the same corrupted row. This
    // is the lever that certifies the slim path is the only read
    // surface that is safe against JSON drift.
    let heavy = store
        .query_dispatches(&DispatchFilter {
            limit: 100,
            ..DispatchFilter::new()
        })
        .await;
    let err = heavy.expect_err("heavy path must fail on corrupted dispatch json");
    assert!(
        err.to_string()
            .contains("dispatch snapshot deserialize failed"),
        "unexpected error: {err}"
    );

    drop(store);
    drop(raw_conn);
    let _ = std::fs::remove_file(&path);
}
