//! Concurrency and pagination stability tests on real `SQLite` store.

use std::sync::Arc;

use tanren_domain::{
    ActorContext, AuthMode, CancelDispatch, Cli, ConfigEnv, CreateDispatch, DispatchMode,
    DispatchStatus, DomainError, NonEmptyString, OrgId, Phase, TimeoutSecs, UserId,
};
use tanren_orchestrator::{Orchestrator, OrchestratorError};
use tanren_policy::PolicyEngine;
use tanren_store::{DispatchFilter, Store};

fn sample_actor() -> ActorContext {
    ActorContext::new(OrgId::new(), UserId::new())
}

fn sample_command(actor: ActorContext) -> CreateDispatch {
    CreateDispatch {
        actor,
        project: NonEmptyString::try_new("test-project".to_owned()).expect("non-empty"),
        phase: Phase::DoTask,
        cli: Cli::Claude,
        auth_mode: AuthMode::ApiKey,
        branch: NonEmptyString::try_new("main".to_owned()).expect("non-empty"),
        spec_folder: NonEmptyString::try_new("spec".to_owned()).expect("non-empty"),
        workflow_id: NonEmptyString::try_new("wf-1".to_owned()).expect("non-empty"),
        mode: DispatchMode::Manual,
        timeout: TimeoutSecs::try_new(60).expect("positive"),
        environment_profile: NonEmptyString::try_new("default".to_owned()).expect("non-empty"),
        gate_cmd: None,
        context: None,
        model: None,
        project_env: ConfigEnv::default(),
        required_secrets: vec![],
        preserve_on_failure: false,
    }
}

async fn setup() -> Orchestrator<Store> {
    let store = Store::open_and_migrate("sqlite::memory:")
        .await
        .expect("store");
    let policy = PolicyEngine::new();
    Orchestrator::new(store, policy)
}

#[tokio::test]
async fn concurrent_create_and_list_is_stable() {
    let orch = Arc::new(setup().await);
    let actor = sample_actor();

    let mut tasks = Vec::new();
    for _ in 0..25 {
        let orch = Arc::clone(&orch);
        let actor = actor.clone();
        tasks.push(tokio::spawn(async move {
            let cmd = sample_command(actor);
            orch.create_dispatch(cmd).await
        }));
    }

    let mut created_ids = Vec::new();
    for task in tasks {
        let created = task.await.expect("task join").expect("create");
        created_ids.push(created.dispatch_id);
    }
    assert_eq!(created_ids.len(), 25);

    let mut filter = DispatchFilter::new();
    filter.limit = 10;
    let page1 = orch.list_dispatches(filter.clone()).await.expect("page1");
    assert_eq!(page1.dispatches.len(), 10);
    assert!(page1.next_cursor.is_some(), "first page should have cursor");

    filter.cursor = page1.next_cursor;
    let page2 = orch.list_dispatches(filter).await.expect("page2");
    assert!(
        !page2.dispatches.is_empty(),
        "second page should not be empty"
    );

    let first_page_ids: std::collections::HashSet<_> =
        page1.dispatches.iter().map(|d| d.dispatch_id).collect();
    let second_page_ids: std::collections::HashSet<_> =
        page2.dispatches.iter().map(|d| d.dispatch_id).collect();
    let overlap_count = first_page_ids.intersection(&second_page_ids).count();
    assert_eq!(overlap_count, 0, "cursor pagination pages must not overlap");
}

#[tokio::test]
async fn concurrent_cancel_results_in_single_success_path() {
    let orch = Arc::new(setup().await);
    let actor = sample_actor();
    let created = orch
        .create_dispatch(sample_command(actor.clone()))
        .await
        .expect("create");

    let mut tasks = Vec::new();
    for _ in 0..8 {
        let orch = Arc::clone(&orch);
        let actor = actor.clone();
        let dispatch_id = created.dispatch_id;
        tasks.push(tokio::spawn(async move {
            orch.cancel_dispatch(CancelDispatch {
                actor,
                dispatch_id,
                reason: Some("parallel cancel".to_owned()),
            })
            .await
        }));
    }

    let mut successes = 0_u64;
    let mut failures = 0_u64;
    for task in tasks {
        match task.await.expect("task join") {
            Ok(()) => successes += 1,
            Err(
                OrchestratorError::Domain(DomainError::InvalidTransition { .. })
                | OrchestratorError::Store(
                    tanren_store::StoreError::InvalidTransition { .. }
                    | tanren_store::StoreError::Conflict { .. },
                ),
            ) => failures += 1,
            Err(other) => {
                assert!(
                    matches!(
                        other,
                        OrchestratorError::Domain(DomainError::InvalidTransition { .. })
                            | OrchestratorError::Store(
                                tanren_store::StoreError::InvalidTransition { .. }
                                    | tanren_store::StoreError::Conflict { .. }
                            )
                    ),
                    "unexpected cancel result: {other:?}"
                );
            }
        }
    }
    assert_eq!(successes, 1, "exactly one cancellation should win");
    assert_eq!(
        failures, 7,
        "all other cancels should fail deterministically"
    );
}

#[tokio::test]
async fn cancel_after_concurrent_cancels_reads_cancelled() {
    let orch = setup().await;
    let actor = sample_actor();
    let created = orch
        .create_dispatch(sample_command(actor.clone()))
        .await
        .expect("create");

    orch.cancel_dispatch(CancelDispatch {
        actor,
        dispatch_id: created.dispatch_id,
        reason: Some("normal cancel".to_owned()),
    })
    .await
    .expect("cancel");

    let view = orch
        .get_dispatch(&created.dispatch_id)
        .await
        .expect("get")
        .expect("exists");
    assert_eq!(view.status, DispatchStatus::Cancelled);
}
