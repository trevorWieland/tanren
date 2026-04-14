//! `SQLite` regression tests for dispatch cursor pagination and atomic create.

use chrono::Utc;
use tanren_domain::{
    ActorContext, AuthMode, Cli, ConfigKeys, DispatchId, DispatchMode, DispatchSnapshot,
    DomainEvent, EntityRef, EventEnvelope, EventId, GraphRevision, Lane, NonEmptyString, OrgId,
    Phase, ProvisionPayload, StepId, StepPayload, StepReadyState, StepType, TimeoutSecs, UserId,
};
use tanren_store::{
    CreateDispatchParams, CreateDispatchWithInitialStepParams, DispatchFilter, EventFilter,
    EventStore, JobQueue, StateStore, Store,
};
use uuid::Uuid;

async fn fresh_store() -> Store {
    let store = Store::new("sqlite::memory:").await.expect("connect");
    store.run_migrations().await.expect("migrate");
    store
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

fn create_dispatch_params(project: &str, lane: Lane) -> CreateDispatchParams {
    let dispatch_id = DispatchId::new();
    let dispatch = snapshot(project);
    let actor_ctx = actor();
    let created_at = Utc::now();
    CreateDispatchParams {
        dispatch_id,
        mode: DispatchMode::Manual,
        lane,
        dispatch: dispatch.clone(),
        actor: actor_ctx.clone(),
        graph_revision: GraphRevision::INITIAL,
        created_at,
        creation_event: EventEnvelope::new(
            EventId::from_uuid(Uuid::now_v7()),
            created_at,
            DomainEvent::DispatchCreated {
                dispatch_id,
                dispatch: Box::new(dispatch),
                mode: DispatchMode::Manual,
                lane,
                actor: actor_ctx,
                graph_revision: GraphRevision::INITIAL,
            },
        ),
    }
}

async fn create_dispatch(store: &Store, project: &str, lane: Lane) -> DispatchId {
    let params = create_dispatch_params(project, lane);
    let id = params.dispatch_id;
    store
        .create_dispatch_projection(params)
        .await
        .expect("create dispatch");
    id
}

fn provision_step(
    dispatch_id: DispatchId,
    step_id: StepId,
    dispatch: DispatchSnapshot,
    lane: Lane,
) -> tanren_store::EnqueueStepParams {
    tanren_store::EnqueueStepParams {
        dispatch_id,
        step_id,
        step_type: StepType::Provision,
        step_sequence: 0,
        lane: Some(lane),
        depends_on: vec![],
        graph_revision: GraphRevision::INITIAL,
        payload: StepPayload::Provision(Box::new(ProvisionPayload { dispatch })),
        ready_state: StepReadyState::Ready,
        enqueue_event: EventEnvelope::new(
            EventId::from_uuid(Uuid::now_v7()),
            Utc::now(),
            DomainEvent::StepEnqueued {
                dispatch_id,
                step_id,
                step_type: StepType::Provision,
                step_sequence: 0,
                lane: Some(lane),
                depends_on: vec![],
                graph_revision: GraphRevision::INITIAL,
            },
        ),
    }
}

#[tokio::test]
async fn query_dispatches_cursor_paginates_stably() {
    let store = fresh_store().await;
    let _ = create_dispatch(&store, "alpha", Lane::Impl).await;
    let _ = create_dispatch(&store, "beta", Lane::Impl).await;
    let _ = create_dispatch(&store, "gamma", Lane::Impl).await;

    let mut first_filter = DispatchFilter::new();
    first_filter.limit = 1;
    let first = store
        .query_dispatches(&first_filter)
        .await
        .expect("first page");
    assert_eq!(first.dispatches.len(), 1);
    let next_cursor = first.next_cursor.expect("next cursor");

    let mut second_filter = DispatchFilter::new();
    second_filter.limit = 1;
    second_filter.cursor = Some(next_cursor);
    let second = store
        .query_dispatches(&second_filter)
        .await
        .expect("second page");
    assert_eq!(second.dispatches.len(), 1);
    assert_ne!(
        first.dispatches[0].dispatch_id,
        second.dispatches[0].dispatch_id,
    );
}

#[tokio::test]
async fn create_dispatch_with_initial_step_rolls_back_on_step_conflict() {
    let store = fresh_store().await;

    let dispatch_a = create_dispatch(&store, "alpha", Lane::Impl).await;
    let conflicting_step_id = StepId::new();
    store
        .enqueue_step(provision_step(
            dispatch_a,
            conflicting_step_id,
            snapshot("alpha"),
            Lane::Impl,
        ))
        .await
        .expect("seed conflicting step");

    let create_b = create_dispatch_params("beta", Lane::Impl);
    let dispatch_b = create_b.dispatch_id;
    let result = store
        .create_dispatch_with_initial_step(CreateDispatchWithInitialStepParams {
            dispatch: create_b.clone(),
            initial_step: provision_step(
                dispatch_b,
                conflicting_step_id,
                create_b.dispatch.clone(),
                Lane::Impl,
            ),
        })
        .await;
    assert!(result.is_err(), "expected step PK conflict");

    let dispatch_b_view = store
        .get_dispatch(&dispatch_b)
        .await
        .expect("get dispatch b");
    assert!(dispatch_b_view.is_none(), "dispatch row must roll back");

    let events = store
        .query_events(&EventFilter {
            entity_ref: Some(EntityRef::Dispatch(dispatch_b)),
            limit: 10,
            ..EventFilter::new()
        })
        .await
        .expect("events");
    assert_eq!(
        events.total_count, 0,
        "events must roll back with projection"
    );
}
