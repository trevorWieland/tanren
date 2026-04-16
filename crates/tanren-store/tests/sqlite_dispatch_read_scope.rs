//! Read-scope query tests for `SQLite` dispatch projection reads.

use chrono::{Duration, Utc};
use tanren_domain::{
    ActorContext, AuthMode, ConfigKeys, DispatchId, DispatchMode, DispatchReadScope, DomainEvent,
    EventEnvelope, EventId, GraphRevision, Lane, NonEmptyString, OrgId, Phase, ProjectId,
    TimeoutSecs, UserId,
};
use tanren_store::{CreateDispatchParams, DispatchFilter, StateStore, Store};

async fn fresh_store() -> Store {
    let store = Store::new("sqlite::memory:").await.expect("connect");
    store.run_migrations().await.expect("migrate");
    store
}

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

async fn create_dispatch(
    store: &Store,
    project_name: &str,
    actor: ActorContext,
    lane: Lane,
) -> tanren_store::StoreResult<DispatchId> {
    let dispatch_id = DispatchId::new();
    let dispatch = snapshot(project_name);
    let now = Utc::now();
    create_dispatch_at(store, dispatch_id, dispatch, actor, lane, now).await?;
    Ok(dispatch_id)
}

async fn create_dispatch_at(
    store: &Store,
    dispatch_id: DispatchId,
    dispatch: tanren_domain::DispatchSnapshot,
    actor: ActorContext,
    lane: Lane,
    created_at: chrono::DateTime<Utc>,
) -> tanren_store::StoreResult<()> {
    let creation_event = EventEnvelope::new(
        EventId::from_uuid(uuid::Uuid::now_v7()),
        created_at,
        DomainEvent::DispatchCreated {
            dispatch_id,
            dispatch: Box::new(dispatch.clone()),
            mode: DispatchMode::Manual,
            lane,
            actor: actor.clone(),
            graph_revision: GraphRevision::INITIAL,
        },
    );

    store
        .create_dispatch_projection(CreateDispatchParams {
            dispatch_id,
            mode: DispatchMode::Manual,
            lane,
            dispatch,
            actor,
            graph_revision: GraphRevision::INITIAL,
            created_at,
            creation_event,
        })
        .await
}

#[tokio::test]
async fn query_dispatches_filters_by_policy_read_scope() {
    let store = fresh_store().await;

    let org = OrgId::new();
    let scoped_project = ProjectId::new();
    let scoped_actor = ActorContext {
        org_id: org,
        user_id: UserId::new(),
        team_id: None,
        api_key_id: None,
        project_id: Some(scoped_project),
    };
    let unscoped_actor = ActorContext {
        org_id: org,
        user_id: UserId::new(),
        team_id: None,
        api_key_id: None,
        project_id: None,
    };
    let cross_org_actor = ActorContext {
        org_id: OrgId::new(),
        user_id: UserId::new(),
        team_id: None,
        api_key_id: None,
        project_id: None,
    };

    let scoped_id = create_dispatch(&store, "alpha", scoped_actor, Lane::Impl)
        .await
        .expect("scoped");
    let unscoped_id = create_dispatch(&store, "alpha", unscoped_actor, Lane::Impl)
        .await
        .expect("unscoped");
    let _ = create_dispatch(&store, "alpha", cross_org_actor, Lane::Impl)
        .await
        .expect("cross-org");

    let scoped_reader = DispatchReadScope {
        org_id: org,
        project_id: Some(scoped_project),
        team_id: None,
        api_key_id: None,
    };
    let scoped_page = store
        .query_dispatches(&DispatchFilter {
            read_scope: Some(scoped_reader),
            limit: 10,
            ..DispatchFilter::new()
        })
        .await
        .expect("scoped query");
    assert_eq!(scoped_page.dispatches.len(), 2);
    assert!(
        scoped_page
            .dispatches
            .iter()
            .any(|dispatch| dispatch.dispatch_id == scoped_id)
    );
    assert!(
        scoped_page
            .dispatches
            .iter()
            .any(|dispatch| dispatch.dispatch_id == unscoped_id)
    );

    let unscoped_reader = DispatchReadScope {
        org_id: org,
        project_id: None,
        team_id: None,
        api_key_id: None,
    };
    let unscoped_page = store
        .query_dispatches(&DispatchFilter {
            read_scope: Some(unscoped_reader),
            limit: 10,
            ..DispatchFilter::new()
        })
        .await
        .expect("unscoped query");
    assert_eq!(unscoped_page.dispatches.len(), 1);
    assert_eq!(unscoped_page.dispatches[0].dispatch_id, unscoped_id);
}

#[tokio::test]
async fn get_dispatch_scoped_enforces_scope_and_keeps_unscoped_visibility() {
    let store = fresh_store().await;
    let org = OrgId::new();
    let project = ProjectId::new();
    let scoped_dispatch = create_dispatch(
        &store,
        "alpha",
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
    .expect("create scoped");
    let unscoped_dispatch = create_dispatch(
        &store,
        "alpha",
        ActorContext {
            org_id: org,
            user_id: UserId::new(),
            team_id: None,
            api_key_id: None,
            project_id: None,
        },
        Lane::Impl,
    )
    .await
    .expect("create unscoped");

    let unscoped_reader = DispatchReadScope {
        org_id: org,
        project_id: None,
        team_id: None,
        api_key_id: None,
    };
    let hidden = store
        .get_dispatch_scoped(&scoped_dispatch, unscoped_reader)
        .await
        .expect("scoped get");
    assert!(hidden.is_none(), "scope mismatch should be hidden");

    let scope = DispatchReadScope {
        org_id: org,
        project_id: Some(project),
        team_id: None,
        api_key_id: None,
    };
    let scoped_view = store
        .get_dispatch_scoped(&scoped_dispatch, scope)
        .await
        .expect("scoped get")
        .expect("visible");
    assert_eq!(scoped_view.dispatch_id, scoped_dispatch);

    let unscoped_view = store
        .get_dispatch_scoped(&unscoped_dispatch, scope)
        .await
        .expect("unscoped get")
        .expect("visible");
    assert_eq!(unscoped_view.dispatch_id, unscoped_dispatch);
}

#[tokio::test]
async fn get_dispatch_actor_context_for_cancel_auth_returns_scope_fields_only() {
    let store = fresh_store().await;
    let actor = ActorContext {
        org_id: OrgId::new(),
        user_id: UserId::new(),
        team_id: Some(tanren_domain::TeamId::new()),
        api_key_id: Some(tanren_domain::ApiKeyId::new()),
        project_id: Some(ProjectId::new()),
    };
    let dispatch_id = create_dispatch(&store, "alpha", actor.clone(), Lane::Impl)
        .await
        .expect("create");

    let fetched = store
        .get_dispatch_actor_context_for_cancel_auth(&dispatch_id)
        .await
        .expect("lookup")
        .expect("dispatch must exist");
    assert_eq!(fetched, actor);

    let missing = store
        .get_dispatch_actor_context_for_cancel_auth(&DispatchId::new())
        .await
        .expect("lookup missing");
    assert!(missing.is_none());
}

#[tokio::test]
async fn query_dispatches_respects_full_scope_tuple_matching() {
    let store = fresh_store().await;
    let fixture = seed_full_scope_fixture(&store).await;

    let scoped_page = store
        .query_dispatches(&DispatchFilter {
            read_scope: Some(fixture.scope),
            limit: 20,
            ..DispatchFilter::new()
        })
        .await
        .expect("scoped query");

    let ids: std::collections::HashSet<_> = scoped_page
        .dispatches
        .iter()
        .map(|dispatch| dispatch.dispatch_id)
        .collect();
    assert_eq!(ids.len(), scoped_page.dispatches.len(), "no duplicate rows");
    assert_eq!(ids.len(), 5);
    for expected in fixture.allowed_dispatches {
        assert!(
            ids.contains(&expected),
            "missing expected dispatch {expected}"
        );
    }
}

#[tokio::test]
async fn query_dispatches_scoped_cursor_paginates_without_duplicates() {
    let store = fresh_store().await;
    let org = OrgId::new();
    let project = ProjectId::new();
    let team = tanren_domain::TeamId::new();
    let api_key = tanren_domain::ApiKeyId::new();
    let base = Utc::now();

    let mut expected = Vec::new();
    for i in 0..5 {
        let dispatch_id = DispatchId::new();
        expected.push(dispatch_id);
        let actor = match i {
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
            _ => ActorContext {
                org_id: org,
                user_id: UserId::new(),
                team_id: Some(team),
                api_key_id: Some(api_key),
                project_id: Some(project),
            },
        };
        create_dispatch_at(
            &store,
            dispatch_id,
            snapshot("alpha"),
            actor,
            Lane::Impl,
            base + Duration::milliseconds(i64::from(i)),
        )
        .await
        .expect("create");
    }

    let mut cursor = None;
    let mut seen = Vec::new();
    loop {
        let page = store
            .query_dispatches(&DispatchFilter {
                read_scope: Some(DispatchReadScope {
                    org_id: org,
                    project_id: Some(project),
                    team_id: Some(team),
                    api_key_id: Some(api_key),
                }),
                limit: 2,
                cursor,
                ..DispatchFilter::new()
            })
            .await
            .expect("page");
        for dispatch in &page.dispatches {
            seen.push(dispatch.dispatch_id);
        }
        cursor = page.next_cursor;
        if cursor.is_none() {
            break;
        }
    }

    let seen_set: std::collections::HashSet<_> = seen.iter().copied().collect();
    assert_eq!(
        seen_set.len(),
        seen.len(),
        "cursor pages must not duplicate rows"
    );
    assert_eq!(seen_set.len(), expected.len());
    for dispatch_id in expected {
        assert!(seen_set.contains(&dispatch_id));
    }
}

#[derive(Debug, Clone, Copy)]
struct ScopeFixture {
    scope: DispatchReadScope,
    allowed_dispatches: [DispatchId; 5],
}

async fn seed_full_scope_fixture(store: &Store) -> ScopeFixture {
    let org = OrgId::new();
    let project = ProjectId::new();
    let team = tanren_domain::TeamId::new();
    let api_key = tanren_domain::ApiKeyId::new();
    let base = Utc::now();

    let mut created = Vec::new();
    for (index, (team_id, api_key_id, project_id)) in [
        (None, None, None),
        (None, None, Some(project)),
        (Some(team), None, Some(project)),
        (None, Some(api_key), Some(project)),
        (Some(team), Some(api_key), Some(project)),
    ]
    .into_iter()
    .enumerate()
    {
        let dispatch_id = DispatchId::new();
        create_dispatch_at(
            store,
            dispatch_id,
            snapshot("alpha"),
            ActorContext {
                org_id: org,
                user_id: UserId::new(),
                team_id,
                api_key_id,
                project_id,
            },
            Lane::Impl,
            base + Duration::milliseconds(i64::try_from(index).expect("index fits i64") + 1),
        )
        .await
        .expect("allowed dispatch");
        created.push(dispatch_id);
    }

    create_dispatch_at(
        store,
        DispatchId::new(),
        snapshot("alpha"),
        ActorContext {
            org_id: org,
            user_id: UserId::new(),
            team_id: None,
            api_key_id: None,
            project_id: Some(ProjectId::new()),
        },
        Lane::Impl,
        base + Duration::milliseconds(10),
    )
    .await
    .expect("other project");

    create_dispatch_at(
        store,
        DispatchId::new(),
        snapshot("alpha"),
        ActorContext {
            org_id: OrgId::new(),
            user_id: UserId::new(),
            team_id: Some(team),
            api_key_id: Some(api_key),
            project_id: Some(project),
        },
        Lane::Impl,
        base + Duration::milliseconds(11),
    )
    .await
    .expect("cross-org");

    ScopeFixture {
        scope: DispatchReadScope {
            org_id: org,
            project_id: Some(project),
            team_id: Some(team),
            api_key_id: Some(api_key),
        },
        allowed_dispatches: [created[0], created[1], created[2], created[3], created[4]],
    }
}
