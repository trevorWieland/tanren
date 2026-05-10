//! Project-setup step definitions for B-0025 and B-0026.
//!
//! Step bodies dispatch through [`ProjectHarness`](tanren_testkit::ProjectHarness)
//! implementations selected from scenario interface tags.

use chrono::Utc;
use cucumber::{given, then, when};
use secrecy::SecretString;
use tanren_contract::{
    ActiveProjectRequest, ConnectProjectRepositoryRequest, ListVisibleProjectsRequest,
    ProjectFailureReason, ProjectPageRequest, SignUpRequest,
};
use tanren_identity_policy::{AccountId, Email, RepositoryRef};

use crate::{ProjectActorState, RepositoryFixtureState, TanrenWorld};

#[given(expr = "{word} has a project account")]
async fn given_project_account(world: &mut TanrenWorld, actor: String) {
    let ctx = world.ensure_project_ctx().await;
    let email_raw = format!(
        "{actor}-project-{}@bdd.tanren",
        Utc::now()
            .timestamp_nanos_opt()
            .expect("timestamp nanos should be available")
    );
    let email = Email::parse(&email_raw).expect("synthesized fixture email must parse");
    let password = SecretString::from("fixture-password".to_owned());
    let session = ctx
        .harness
        .sign_up(SignUpRequest {
            email,
            password,
            display_name: format!("{actor} project account"),
        })
        .await
        .expect("fixture account sign-up should succeed");
    let entry = ctx.actors.entry(actor).or_default();
    entry.account_id = Some(session.account_id);
    entry.last_connected_repository = None;
    entry.last_created_repository = None;
    entry.last_designated_host = None;
    let existing_repositories = ctx
        .repositories
        .values()
        .map(|fixture| fixture.repository.clone())
        .collect::<Vec<_>>();
    for repository in existing_repositories {
        ctx.harness
            .set_repository_access(session.account_id, repository, true)
            .await
            .expect("default repository access grant should configure");
    }
    ctx.last_failure_code = None;
    ctx.last_failure_summary = None;
}

#[given(expr = "repository fixture {string} has fingerprint {string} and {int} prior commits")]
async fn given_repository_fixture(
    world: &mut TanrenWorld,
    repository: String,
    fingerprint: String,
    prior_commits: usize,
) {
    assert!(
        prior_commits > 0,
        "fixture prior commit count must be > 0 for this witness"
    );
    let ctx = world.ensure_project_ctx().await;
    let parsed = RepositoryRef::parse(&repository).expect("fixture repository must parse");
    let expected = repository_fingerprint(&parsed);
    assert_eq!(
        fingerprint, expected,
        "fixture fingerprint must match canonical repository fingerprint"
    );
    ctx.repositories.insert(
        parsed.as_str().to_owned(),
        RepositoryFixtureState {
            repository: parsed.clone(),
            fingerprint,
            prior_commits,
        },
    );
    let known_accounts = ctx
        .actors
        .values()
        .filter_map(|state| state.account_id)
        .collect::<Vec<_>>();
    for account_id in known_accounts {
        ctx.harness
            .set_repository_access(account_id, parsed.clone(), true)
            .await
            .expect("default repository access grant should configure");
    }
}

#[given(expr = "repository fixture {string} is accessible to {word}")]
async fn given_repository_accessible_to_actor(
    world: &mut TanrenWorld,
    repository: String,
    actor: String,
) {
    set_repository_access(world, repository, actor, true).await;
}

#[given(expr = "repository fixture {string} is not accessible to {word}")]
async fn given_repository_not_accessible_to_actor(
    world: &mut TanrenWorld,
    repository: String,
    actor: String,
) {
    set_repository_access(world, repository, actor, false).await;
}

#[when(expr = "{word} connects existing repository {string} as an active project")]
async fn when_connect_existing(world: &mut TanrenWorld, actor: String, repository: String) {
    connect_existing_impl(world, actor, repository, false).await;
}

#[when(
    expr = "{word} uses their credential to connect existing repository {string} for {word} as an active project"
)]
async fn when_connect_existing_for_other_account(
    world: &mut TanrenWorld,
    actor: String,
    repository: String,
    target_actor: String,
) {
    let Ok(parsed_repository) = RepositoryRef::parse(&repository) else {
        let ctx = world.ensure_project_ctx().await;
        let entry = ctx.actors.entry(actor).or_default();
        entry.last_connected_repository = None;
        entry.last_created_repository = None;
        entry.last_designated_host = None;
        ctx.last_failure_code = Some("validation_failed".to_owned());
        ctx.last_failure_summary =
            Some(ProjectFailureReason::ValidationFailed.summary().to_owned());
        return;
    };
    let ctx = world.ensure_project_ctx().await;
    assert!(
        ctx.repositories.contains_key(parsed_repository.as_str()),
        "repository fixture must be seeded before connect attempt"
    );
    let actor_credential_account_id = actor_account_id(&ctx.actors, &actor);
    let target_account_id = actor_account_id(&ctx.actors, &target_actor);
    let result = ctx
        .harness
        .connect_project_repository_as_actor(
            actor_credential_account_id,
            ConnectProjectRepositoryRequest {
                owning_account_id: target_account_id,
                repository: parsed_repository.clone(),
                select_as_active: true,
            },
        )
        .await;
    let entry = ctx.actors.entry(actor).or_default();
    entry.account_id = Some(actor_credential_account_id);
    match result {
        Ok(response) => {
            entry.last_connected_repository = Some(response.project.repository.repository);
            entry.last_created_repository = None;
            entry.last_designated_host = None;
            ctx.last_failure_code = None;
            ctx.last_failure_summary = None;
        }
        Err(err) => {
            entry.last_connected_repository = None;
            entry.last_created_repository = None;
            entry.last_designated_host = None;
            ctx.last_failure_code = Some(err.code());
            ctx.last_failure_summary = err.summary().map(str::to_owned);
        }
    }
}

#[when(expr = "{word} tries to connect existing repository {string} without an account")]
async fn when_connect_without_account(world: &mut TanrenWorld, actor: String, repository: String) {
    connect_existing_impl(world, actor, repository, true).await;
}

#[then(expr = "the connection succeeds")]
async fn then_connection_succeeds(world: &mut TanrenWorld) {
    let ctx = world.ensure_project_ctx().await;
    assert!(
        ctx.last_failure_code.is_none(),
        "expected successful connection, got failure {:?}",
        ctx.last_failure_code
    );
}

#[then(expr = "the project request fails with code {string}")]
async fn then_project_failure(world: &mut TanrenWorld, code: String) {
    let ctx = world.ensure_project_ctx().await;
    assert_eq!(
        ctx.last_failure_code.as_deref(),
        Some(code.as_str()),
        "expected project failure code {code}, got {:?}",
        ctx.last_failure_code
    );
}

#[then(expr = "the project failure summary is {string}")]
async fn then_project_failure_summary(world: &mut TanrenWorld, summary: String) {
    let ctx = world.ensure_project_ctx().await;
    assert_eq!(
        ctx.last_failure_summary.as_deref(),
        Some(summary.as_str()),
        "expected project failure summary {summary:?}, got {:?}",
        ctx.last_failure_summary
    );
}

#[then(expr = "source-control provider connect checks were called {int} times")]
async fn then_connect_provider_check_calls(world: &mut TanrenWorld, expected: u64) {
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
        counters.can_access_repository, expected,
        "expected repository-access checks to run {expected} times"
    );
}

#[then(expr = "repository {string} keeps fingerprint {string}")]
async fn then_fingerprint_unchanged(
    world: &mut TanrenWorld,
    repository: String,
    fingerprint: String,
) {
    let ctx = world.ensure_project_ctx().await;
    let parsed = RepositoryRef::parse(&repository).expect("repository must parse");
    let fixture = ctx
        .repositories
        .get(parsed.as_str())
        .expect("repository fixture must be present");
    assert_eq!(
        fixture.fingerprint, fingerprint,
        "fixture fingerprint should remain unchanged"
    );
    let actual = repository_fingerprint(&fixture.repository);
    assert_eq!(
        actual, fingerprint,
        "connected repository fingerprint should match fixture"
    );
    assert!(
        ctx.actors
            .values()
            .any(|actor| actor.last_connected_repository.as_ref() == Some(&parsed)),
        "expected at least one actor to connect repository {parsed}"
    );
}

#[then(expr = "{word} sees repository {string} in their project list")]
async fn then_repository_visible(world: &mut TanrenWorld, actor: String, repository: String) {
    let parsed = RepositoryRef::parse(&repository).expect("repository must parse");
    let account_id = {
        let ctx = world.ensure_project_ctx().await;
        actor_account_id(&ctx.actors, &actor)
    };
    let ctx = world.ensure_project_ctx().await;
    let list = ctx
        .harness
        .list_visible_projects(ListVisibleProjectsRequest {
            owning_account_id: account_id,
            page: ProjectPageRequest::default(),
        })
        .await
        .expect("list_visible_projects should succeed");
    assert!(
        list.projects
            .iter()
            .any(|project| project.repository.repository == parsed),
        "expected repository {parsed} in project list"
    );
}

#[then(expr = "{word} has active project repository {string}")]
async fn then_active_repository(world: &mut TanrenWorld, actor: String, repository: String) {
    let parsed = RepositoryRef::parse(&repository).expect("repository must parse");
    let account_id = {
        let ctx = world.ensure_project_ctx().await;
        actor_account_id(&ctx.actors, &actor)
    };
    let ctx = world.ensure_project_ctx().await;
    let active = ctx
        .harness
        .active_project(ActiveProjectRequest {
            owning_account_id: account_id,
        })
        .await
        .expect("active_project should succeed");
    let project = active
        .active_project
        .expect("expected active project after connect-existing with select_as_active=true");
    assert_eq!(project.repository.repository, parsed);
    assert!(
        project.selection.is_active,
        "active project should be active"
    );
}

#[then(expr = "{word} has exactly {int} connected project records")]
async fn then_project_record_count(world: &mut TanrenWorld, actor: String, expected: usize) {
    let account_id = {
        let ctx = world.ensure_project_ctx().await;
        actor_account_id(&ctx.actors, &actor)
    };
    let ctx = world.ensure_project_ctx().await;
    let list = ctx
        .harness
        .list_visible_projects(ListVisibleProjectsRequest {
            owning_account_id: account_id,
            page: ProjectPageRequest::default(),
        })
        .await
        .expect("list projects should succeed");
    assert_eq!(
        list.projects.len(),
        expected,
        "expected {expected} project records, got {}",
        list.projects.len()
    );
}

#[then(expr = "repository {string} has zero Tanren activity counts")]
async fn then_zero_activity_counts(world: &mut TanrenWorld, repository: String) {
    let parsed = RepositoryRef::parse(&repository).expect("repository must parse");
    let account_ids = {
        let ctx = world.ensure_project_ctx().await;
        let fixture = ctx
            .repositories
            .get(parsed.as_str())
            .expect("repository fixture must be present");
        assert!(
            fixture.prior_commits > 0,
            "this witness requires a fixture repository with prior commits"
        );
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
            assert_eq!(project.counts.specs, 0, "expected zero imported specs");
            assert_eq!(
                project.counts.milestones, 0,
                "expected zero imported milestones"
            );
            assert_eq!(
                project.counts.initiatives, 0,
                "expected zero imported initiatives"
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

async fn connect_existing_impl(
    world: &mut TanrenWorld,
    actor: String,
    repository: String,
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
        entry.last_designated_host = None;
        ctx.last_failure_code = Some(ProjectFailureReason::ValidationFailed.code().to_owned());
        ctx.last_failure_summary =
            Some(ProjectFailureReason::ValidationFailed.summary().to_owned());
        return;
    };
    let ctx = world.ensure_project_ctx().await;
    assert!(
        ctx.repositories.contains_key(parsed_repository.as_str()),
        "repository fixture must be seeded before connect attempt"
    );

    let request_account_id = if without_account {
        AccountId::fresh()
    } else {
        actor_account_id(&ctx.actors, &actor)
    };

    let result = ctx
        .harness
        .connect_project_repository(ConnectProjectRepositoryRequest {
            owning_account_id: request_account_id,
            repository: parsed_repository.clone(),
            select_as_active: true,
        })
        .await;

    let entry = ctx
        .actors
        .entry(actor)
        .or_insert_with(ProjectActorState::default);
    entry.account_id = Some(request_account_id);
    match result {
        Ok(response) => {
            entry.last_connected_repository = Some(response.project.repository.repository);
            entry.last_created_repository = None;
            entry.last_designated_host = None;
            ctx.last_failure_code = None;
            ctx.last_failure_summary = None;
        }
        Err(err) => {
            entry.last_connected_repository = None;
            entry.last_created_repository = None;
            entry.last_designated_host = None;
            ctx.last_failure_code = Some(err.code());
            ctx.last_failure_summary = err.summary().map(str::to_owned);
        }
    }
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

fn repository_fingerprint(repository: &RepositoryRef) -> String {
    format!("repo-fp::{}", repository.as_str())
}

async fn set_repository_access(
    world: &mut TanrenWorld,
    repository: String,
    actor: String,
    allowed: bool,
) {
    let parsed_repository = RepositoryRef::parse(&repository).expect("repository must parse");
    let ctx = world.ensure_project_ctx().await;
    assert!(
        ctx.repositories.contains_key(parsed_repository.as_str()),
        "repository fixture must be seeded before access configuration"
    );
    let account_id = actor_account_id(&ctx.actors, &actor);
    ctx.harness
        .set_repository_access(account_id, parsed_repository, allowed)
        .await
        .expect("repository fixture access should configure");
}
