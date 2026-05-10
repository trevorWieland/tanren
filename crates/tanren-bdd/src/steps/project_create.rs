//! Create-new-project step definitions for B-0026.

use cucumber::{given, then, when};
use tanren_contract::{
    CreateProjectRequest, ListVisibleProjectsRequest, ProjectFailureReason, ProjectPageRequest,
};
use tanren_identity_policy::{AccountId, DesignatedHost, RepositoryRef};

use crate::{HostFixtureState, ProjectActorState, TanrenWorld};

#[given(expr = "designated fixture host {string} is accessible to {word}")]
async fn given_host_accessible(world: &mut TanrenWorld, host: String, actor: String) {
    upsert_host_fixture(world, host, actor, true).await;
}

#[given(expr = "designated fixture host {string} is not accessible to {word}")]
async fn given_host_inaccessible(world: &mut TanrenWorld, host: String, actor: String) {
    upsert_host_fixture(world, host, actor, false).await;
}

#[given(expr = "the project store fails project registration")]
async fn given_project_store_fails_registration(world: &mut TanrenWorld) {
    let ctx = world.ensure_project_ctx().await;
    ctx.harness
        .break_project_store_for_testing()
        .await
        .expect("project-store fault injection should be available");
}

#[when(
    expr = "{word} creates new repository {string} at designated host {string} as an active project"
)]
async fn when_create_new_project(
    world: &mut TanrenWorld,
    actor: String,
    repository: String,
    designated_host: String,
) {
    create_new_project_impl(world, actor, repository, designated_host, false).await;
}

#[then(expr = "the project creation succeeds")]
async fn then_project_creation_succeeds(world: &mut TanrenWorld) {
    let ctx = world.ensure_project_ctx().await;
    assert!(
        ctx.last_failure_code.is_none(),
        "expected successful project creation, got failure {:?}",
        ctx.last_failure_code
    );
}

#[then(expr = "source-control provider create checks were called {int} times")]
async fn then_create_provider_check_calls(world: &mut TanrenWorld, expected: u64) {
    let ctx = world.ensure_project_ctx().await;
    let counters = ctx
        .harness
        .source_control_call_counters()
        .await
        .expect("source-control provider counters should be available");
    assert_eq!(
        counters.ensure_provider_reachable, expected,
        "expected provider reachability checks to run {expected} times"
    );
    assert_eq!(
        counters.ensure_host_reachable, expected,
        "expected host reachability checks to run {expected} times"
    );
    assert_eq!(
        counters.can_create_repository_at_host, expected,
        "expected host-create access checks to run {expected} times"
    );
    assert_eq!(
        counters.create_repository, expected,
        "expected repository create calls to run {expected} times"
    );
}

#[then(expr = "repository {string} exists at designated host {string}")]
async fn then_repository_exists_at_host(
    world: &mut TanrenWorld,
    repository: String,
    designated_host: String,
) {
    let repository = RepositoryRef::parse(&repository).expect("repository must parse");
    let host = DesignatedHost::parse(&designated_host).expect("designated host must parse");
    let ctx = world.ensure_project_ctx().await;
    let created = ctx
        .harness
        .repository_created_at_host(&host, &repository)
        .await
        .expect("fixture provider created-repository observation should succeed");
    assert!(
        created,
        "expected repository {repository} to exist at designated host {host}"
    );
}

#[then(expr = "repository {string} does not exist at designated host {string}")]
async fn then_repository_does_not_exist_at_host(
    world: &mut TanrenWorld,
    repository: String,
    designated_host: String,
) {
    let repository = RepositoryRef::parse(&repository).expect("repository must parse");
    let host = DesignatedHost::parse(&designated_host).expect("designated host must parse");
    let ctx = world.ensure_project_ctx().await;
    let created = ctx
        .harness
        .repository_created_at_host(&host, &repository)
        .await
        .expect("fixture provider created-repository observation should succeed");
    assert!(
        !created,
        "expected repository {repository} to be absent from designated host {host}"
    );
}

#[then(expr = "repository {string} starts with zero Tanren activity counts")]
async fn then_zero_initial_activity_counts(world: &mut TanrenWorld, repository: String) {
    let parsed = RepositoryRef::parse(&repository).expect("repository must parse");
    let account_ids = {
        let ctx = world.ensure_project_ctx().await;
        ctx.actors
            .values()
            .filter_map(|actor| actor.account_id)
            .collect::<Vec<_>>()
    };
    let ctx = world.ensure_project_ctx().await;
    let mut found = false;
    for account_id in account_ids {
        let list = ctx
            .harness
            .list_visible_projects(ListVisibleProjectsRequest {
                owning_account_id: account_id,
                page: ProjectPageRequest::default(),
            })
            .await
            .expect("list projects should succeed");
        if let Some(project) = list
            .projects
            .iter()
            .find(|project| project.repository.repository == parsed)
        {
            assert_eq!(
                project.counts.specs, 0,
                "expected zero specs at project creation"
            );
            assert_eq!(
                project.counts.milestones, 0,
                "expected zero milestones at project creation"
            );
            assert_eq!(
                project.counts.initiatives, 0,
                "expected zero initiatives at project creation"
            );
            found = true;
            break;
        }
    }
    assert!(
        found,
        "repository {parsed} should be visible in at least one actor project list"
    );
}

async fn create_new_project_impl(
    world: &mut TanrenWorld,
    actor: String,
    repository: String,
    designated_host: String,
    without_account: bool,
) {
    let Ok(parsed_repository) = RepositoryRef::parse(&repository) else {
        let ctx = world.ensure_project_ctx().await;
        let entry = ctx
            .actors
            .entry(actor)
            .or_insert_with(ProjectActorState::default);
        entry.last_connected_repository = None;
        entry.last_created_repository = None;
        ctx.last_failure_code = Some(ProjectFailureReason::ValidationFailed.code().to_owned());
        ctx.last_failure_summary =
            Some(ProjectFailureReason::ValidationFailed.summary().to_owned());
        return;
    };
    let Ok(parsed_host) = DesignatedHost::parse(&designated_host) else {
        let ctx = world.ensure_project_ctx().await;
        let entry = ctx
            .actors
            .entry(actor)
            .or_insert_with(ProjectActorState::default);
        entry.last_connected_repository = None;
        entry.last_created_repository = None;
        ctx.last_failure_code = Some(ProjectFailureReason::ValidationFailed.code().to_owned());
        ctx.last_failure_summary =
            Some(ProjectFailureReason::ValidationFailed.summary().to_owned());
        return;
    };
    let ctx = world.ensure_project_ctx().await;
    let host_key = normalized_host(parsed_host.as_str());
    assert!(
        ctx.hosts.contains_key(&host_key),
        "designated host fixture must be seeded before create attempt"
    );

    let request_account_id = if without_account {
        AccountId::fresh()
    } else {
        actor_account_id(&ctx.actors, &actor)
    };

    let result = ctx
        .harness
        .create_project(CreateProjectRequest {
            owning_account_id: request_account_id,
            repository: parsed_repository,
            designated_host: parsed_host,
            select_as_active: true,
        })
        .await;

    let entry = ctx
        .actors
        .entry(actor)
        .or_insert_with(ProjectActorState::default);
    entry.last_designated_host = Some(host_key.clone());
    match result {
        Ok(response) => {
            entry.last_connected_repository = None;
            entry.last_created_repository = Some(response.project.repository.repository.clone());
            ctx.last_failure_code = None;
            ctx.last_failure_summary = None;
        }
        Err(err) => {
            entry.last_connected_repository = None;
            entry.last_created_repository = None;
            ctx.last_failure_code = Some(err.code());
            ctx.last_failure_summary = err.summary().map(str::to_owned);
        }
    }
}

async fn upsert_host_fixture(
    world: &mut TanrenWorld,
    host: String,
    actor: String,
    can_create: bool,
) {
    let parsed_host = DesignatedHost::parse(&host).expect("designated host must parse");
    let normalized = normalized_host(parsed_host.as_str());
    let ctx = world.ensure_project_ctx().await;
    assert!(
        ctx.actors.contains_key(&actor),
        "actor {actor} must have a fixture project account before host access setup"
    );
    let account_id = actor_account_id(&ctx.actors, &actor);
    ctx.harness
        .set_designated_host_create_access(account_id, parsed_host, can_create)
        .await
        .expect("host fixture access should configure");
    ctx.hosts
        .entry(normalized.clone())
        .and_modify(|state| state.can_create = can_create)
        .or_insert_with(|| HostFixtureState {
            host: normalized,
            can_create,
            created_repositories: std::collections::HashSet::new(),
        });
}

fn actor_account_id(
    actors: &std::collections::HashMap<String, ProjectActorState>,
    actor: &str,
) -> AccountId {
    actors
        .get(actor)
        .and_then(|entry| entry.account_id)
        .expect("actor must have a fixture account")
}

fn normalized_host(host: &str) -> String {
    host.trim().to_ascii_lowercase()
}
