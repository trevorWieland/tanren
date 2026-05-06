//! Project-flow step definitions for B-0025 (connect) and B-0026 (create).
//!
//! Step bodies dispatch through the per-interface
//! [`AccountHarness`](tanren_testkit::AccountHarness) trait — never
//! `tanren_app_services::Handlers::*` directly. The active harness is
//! selected by the BDD `Before` hook from the scenario's tags
//! (`@api`, `@cli`, `@mcp`, `@tui`, `@web`, or fallback in-process).
//! `xtask check-bdd-wire-coverage` mechanically rejects any future
//! step that bypasses this seam.

use cucumber::{given, then, when};
use tanren_contract::{ConnectProjectRequest, CreateProjectRequest, ProjectContentCounts};
use tanren_identity_policy::AccountId;
use tanren_testkit::{HarnessOutcome, normalize_repository_identity, record_failure};

use crate::TanrenWorld;

#[given(expr = "a repository {string} is accessible on host {string}")]
async fn given_accessible_repo(world: &mut TanrenWorld, url: String, host: String) {
    let ctx = world.ensure_account_ctx().await;
    ctx.harness
        .seed_accessible_repository(&host, &url)
        .await
        .expect("seed accessible repository");
}

#[given(expr = "host {string} is inaccessible")]
async fn given_inaccessible_host(world: &mut TanrenWorld, host: String) {
    let ctx = world.ensure_account_ctx().await;
    ctx.harness
        .seed_inaccessible_host(&host)
        .await
        .expect("seed inaccessible host");
}

#[given(expr = "a repository {string} exists but is inaccessible")]
async fn given_inaccessible_repository(world: &mut TanrenWorld, url: String) {
    let ctx = world.ensure_account_ctx().await;
    ctx.harness
        .seed_inaccessible_repository(&url)
        .await
        .expect("seed inaccessible repository");
}

#[given(expr = "host {string} is accessible")]
async fn given_accessible_host(world: &mut TanrenWorld, host: String) {
    let ctx = world.ensure_account_ctx().await;
    ctx.harness
        .seed_accessible_host(&host)
        .await
        .expect("seed accessible host");
}

#[given(expr = "the repository byte identity for {string} is captured")]
async fn given_capture_identity(world: &mut TanrenWorld, url: String) {
    let ctx = world.ensure_account_ctx().await;
    let identity = normalize_repository_identity(&url);
    ctx.captured_repository_identity = Some(identity);
}

#[when(expr = "{word} connects repository {string} as project {string}")]
async fn when_connect_project(
    world: &mut TanrenWorld,
    actor: String,
    repository_url: String,
    name: String,
) {
    let account_id = require_account_id(world, &actor).await;
    let ctx = world.ensure_account_ctx().await;
    let request = ConnectProjectRequest {
        name,
        repository_url,
        org: None,
    };
    let result = ctx.harness.connect_project(account_id, request).await;
    let outcome = match result {
        Ok(view) => {
            ctx.last_project_account_id = Some(account_id);
            ctx.last_project = Some(view.clone());
            HarnessOutcome::ProjectConnected(view)
        }
        Err(err) => {
            let entry = ctx.actors.entry(actor.clone()).or_default();
            record_failure(err, entry)
        }
    };
    ctx.last_outcome = Some(outcome);
}

#[when(expr = "{word} creates project {string} on host {string}")]
async fn when_create_project(
    world: &mut TanrenWorld,
    actor: String,
    name: String,
    provider_host: String,
) {
    let account_id = require_account_id(world, &actor).await;
    let ctx = world.ensure_account_ctx().await;
    let request = CreateProjectRequest {
        name,
        provider_host,
        org: None,
    };
    let result = ctx.harness.create_project(account_id, request).await;
    let outcome = match result {
        Ok(view) => {
            ctx.last_project_account_id = Some(account_id);
            ctx.last_project = Some(view.clone());
            HarnessOutcome::ProjectCreated(view)
        }
        Err(err) => {
            let entry = ctx.actors.entry(actor.clone()).or_default();
            record_failure(err, entry)
        }
    };
    ctx.last_outcome = Some(outcome);
}

#[then(expr = "the project is set as active for {word}")]
async fn then_active_project(world: &mut TanrenWorld, actor: String) {
    let account_id = require_account_id(world, &actor).await;
    let ctx = world.ensure_account_ctx().await;
    let active = ctx
        .harness
        .active_project(account_id)
        .await
        .expect("active_project should succeed");
    let expected = ctx
        .last_project
        .as_ref()
        .expect("a project must have been connected or created first");
    let active_view = active
        .as_ref()
        .expect("expected an active project")
        .project
        .clone();
    assert_eq!(
        active_view.id, expected.id,
        "active project id must match the last connected/created project"
    );
    ctx.last_outcome = Some(HarnessOutcome::ActiveProject(active));
}

#[then(expr = "the project has empty content counts")]
async fn then_empty_counts(world: &mut TanrenWorld) {
    let ctx = world.ensure_account_ctx().await;
    let project = ctx
        .last_project
        .as_ref()
        .expect("a project must have been connected or created first");
    assert_eq!(
        project.content_counts,
        ProjectContentCounts::empty(),
        "newly created project must have empty content counts"
    );
}

#[then(expr = "the project has no prior Tanren activity")]
async fn then_no_prior_activity(world: &mut TanrenWorld) {
    let ctx = world.ensure_account_ctx().await;
    let project = ctx
        .last_project
        .as_ref()
        .expect("a project must have been connected or created first");
    assert_eq!(
        project.content_counts.specs, 0,
        "no specs should exist on a fresh project"
    );
    assert_eq!(
        project.content_counts.milestones, 0,
        "no milestones should exist on a fresh project"
    );
    assert_eq!(
        project.content_counts.initiatives, 0,
        "no initiatives should exist on a fresh project"
    );
}

#[then(expr = "the project request fails with code {string}")]
async fn then_project_fails_with(world: &mut TanrenWorld, code: String) {
    let ctx = world.ensure_account_ctx().await;
    let actual = ctx
        .last_outcome
        .as_ref()
        .and_then(HarnessOutcome::failure_code);
    assert_eq!(actual, Some(code), "expected project failure code");
}

#[then(expr = "the project repository identity matches the captured identity")]
async fn then_repository_identity_matches(world: &mut TanrenWorld) {
    let ctx = world.ensure_account_ctx().await;
    let project = ctx
        .last_project
        .as_ref()
        .expect("a project must have been connected or created first");
    let captured = ctx
        .captured_repository_identity
        .as_ref()
        .expect("repository identity must have been captured before connect");
    let actual = normalize_repository_identity(&project.repository.url);
    assert_eq!(
        &actual, captured,
        "project repository identity must match the pre-connect captured identity"
    );
}

#[then(expr = "the repository identity for {string} is {string}")]
async fn then_repository_identity(world: &mut TanrenWorld, url: String, expected: String) {
    let _ = world.ensure_account_ctx().await;
    let actual = normalize_repository_identity(&url);
    assert_eq!(actual, expected, "repository identity mismatch");
}

#[then(expr = "one project per repository is enforced")]
async fn then_one_project_per_repo(world: &mut TanrenWorld) {
    let ctx = world.ensure_account_ctx().await;
    let original = ctx
        .last_project
        .as_ref()
        .expect("a project must have been connected first");
    let account_id = ctx
        .last_project_account_id
        .expect("account id must have been captured during connect");
    let repo_url = original.repository.url.clone();
    let duplicate_request = ConnectProjectRequest {
        name: format!("{}-duplicate", original.name),
        repository_url: repo_url,
        org: None,
    };
    let result = ctx
        .harness
        .connect_project(account_id, duplicate_request)
        .await;
    let err = result
        .expect_err("duplicate connect should have been rejected (duplicate_repository expected)");
    assert_eq!(
        err.code(),
        "duplicate_repository",
        "expected duplicate_repository failure, got: {err:?}"
    );
}

#[then(expr = "the API-created project is readable for {word}")]
async fn then_api_project_readable(world: &mut TanrenWorld, actor: String) {
    let account_id = require_account_id(world, &actor).await;
    let ctx = world.ensure_account_ctx().await;
    let expected = ctx
        .last_project
        .as_ref()
        .expect("a project must have been connected or created first");
    let active = ctx
        .harness
        .active_project(account_id)
        .await
        .expect("active_project harness call should succeed")
        .expect("expected an active project");
    assert_eq!(
        active.project.id, expected.id,
        "read-back project id must match"
    );
    assert_eq!(
        active.project.name, expected.name,
        "read-back project name must match"
    );
    assert_eq!(
        active.project.repository.url, expected.repository.url,
        "read-back repository url must match"
    );
    assert_eq!(
        active.project.content_counts, expected.content_counts,
        "read-back content counts must match"
    );
}

async fn require_account_id(world: &mut TanrenWorld, actor: &str) -> AccountId {
    let ctx = world.ensure_account_ctx().await;
    let entry = ctx
        .actors
        .get(actor)
        .expect("actor must have signed up first");
    entry
        .sign_up
        .as_ref()
        .map(|s| s.account_id)
        .or_else(|| entry.sign_in.as_ref().map(|s| s.account_id))
        .expect("actor must have a session")
}
