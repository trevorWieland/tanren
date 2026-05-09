//! Create-new-project step definitions for B-0026.

use cucumber::{given, then, when};
use tanren_contract::{CreateProjectRequest, ListVisibleProjectsRequest};
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

#[then(expr = "repository {string} exists at designated host {string}")]
async fn then_repository_exists_at_host(
    world: &mut TanrenWorld,
    repository: String,
    designated_host: String,
) {
    let ctx = world.ensure_project_ctx().await;
    let repository = RepositoryRef::parse(&repository).expect("repository must parse");
    let host_key = normalized_host(&designated_host);
    let host = ctx
        .hosts
        .get(&host_key)
        .expect("designated host fixture must be seeded");
    assert!(
        host.created_repositories.contains(&repository),
        "expected repository {} to exist at designated host {}",
        repository,
        host.host
    );
}

#[then(expr = "repository {string} does not exist at designated host {string}")]
async fn then_repository_does_not_exist_at_host(
    world: &mut TanrenWorld,
    repository: String,
    designated_host: String,
) {
    let ctx = world.ensure_project_ctx().await;
    let repository = RepositoryRef::parse(&repository).expect("repository must parse");
    let host_key = normalized_host(&designated_host);
    let host = ctx
        .hosts
        .get(&host_key)
        .expect("designated host fixture must be seeded");
    assert!(
        !host.created_repositories.contains(&repository),
        "expected repository {} to be absent from designated host {}",
        repository,
        host.host
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
    let parsed_repository = RepositoryRef::parse(&repository).expect("repository must parse");
    let host_key = normalized_host(&designated_host);
    let parsed_host = DesignatedHost::parse(&designated_host).expect("designated host must parse");
    let ctx = world.ensure_project_ctx().await;
    let host_can_create = ctx
        .hosts
        .get(&host_key)
        .map(|state| state.can_create)
        .expect("designated host fixture must be seeded before create attempt");

    let request_account_id = if without_account || !host_can_create {
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
            let host = ctx
                .hosts
                .get_mut(&host_key)
                .expect("designated host fixture must be present");
            host.created_repositories
                .insert(response.project.repository.repository);
            ctx.last_failure_code = None;
        }
        Err(err) => {
            entry.last_connected_repository = None;
            entry.last_created_repository = None;
            ctx.last_failure_code = Some(err.code());
        }
    }
}

async fn upsert_host_fixture(
    world: &mut TanrenWorld,
    host: String,
    actor: String,
    can_create: bool,
) {
    let normalized = normalized_host(&host);
    let ctx = world.ensure_project_ctx().await;
    assert!(
        ctx.actors.contains_key(&actor),
        "actor {actor} must have a fixture project account before host access setup"
    );
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
