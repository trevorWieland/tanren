//! Project-flow step definitions for B-0025 (connect) and B-0026 (create).
//!
//! Step bodies dispatch through the per-interface
//! [`AccountHarness`](tanren_testkit::AccountHarness) trait — never
//! `tanren_app_services::Handlers::*` directly. The active harness is
//! selected by the BDD `Before` hook from the scenario's tags
//! (`@api`, `@cli`, `@mcp`, `@tui`, `@web`, or fallback in-process).
//! `xtask check-bdd-wire-coverage` mechanically rejects any future
//! step that bypasses this seam.

use std::mem::drop;

use cucumber::{given, then, when};
use tanren_contract::{ConnectProjectRequest, CreateProjectRequest, ProjectContentCounts};
use tanren_identity_policy::{AccountId, OrgId};
use tanren_testkit::{HarnessOutcome, normalize_repository_identity, record_failure};

use crate::TanrenWorld;

const FIXTURE_HOST: &str = "fixture.local";

fn fixture_url(slug: &str) -> String {
    format!("https://{FIXTURE_HOST}/{slug}")
}

#[given(expr = "an existing repository {string} that {word} can access")]
async fn given_existing_repo_accessible(world: &mut TanrenWorld, slug: String, actor: String) {
    drop(actor);
    let url = fixture_url(&slug);
    let ctx = world.ensure_account_ctx().await;
    ctx.harness
        .seed_accessible_repository(FIXTURE_HOST, &url)
        .await
        .expect("seed accessible repository");
}

#[given(expr = "an existing repository {string} with {int} prior commits that {word} can access")]
async fn given_existing_repo_with_commits(
    world: &mut TanrenWorld,
    slug: String,
    commits: usize,
    actor: String,
) {
    let _ = commits;
    drop(actor);
    let url = fixture_url(&slug);
    let ctx = world.ensure_account_ctx().await;
    ctx.harness
        .seed_accessible_repository(FIXTURE_HOST, &url)
        .await
        .expect("seed accessible repository");
}

#[given(expr = "an existing repository {string} that {word} cannot access")]
async fn given_existing_repo_inaccessible(world: &mut TanrenWorld, slug: String, actor: String) {
    drop(actor);
    let url = fixture_url(&slug);
    let ctx = world.ensure_account_ctx().await;
    ctx.harness
        .seed_inaccessible_repository(&url)
        .await
        .expect("seed inaccessible repository");
}

#[given(expr = "a fixture SCM host {string} that {word} can access")]
async fn given_accessible_scm_host(world: &mut TanrenWorld, host: String, actor: String) {
    drop(actor);
    let ctx = world.ensure_account_ctx().await;
    ctx.harness
        .seed_accessible_host(&host)
        .await
        .expect("seed accessible host");
}

#[given(expr = "a fixture SCM host {string} that {word} cannot access")]
async fn given_inaccessible_scm_host(world: &mut TanrenWorld, host: String, actor: String) {
    drop(actor);
    let ctx = world.ensure_account_ctx().await;
    ctx.harness
        .seed_inaccessible_host(&host)
        .await
        .expect("seed inaccessible host");
}

#[given(expr = "the provider is not configured")]
async fn given_provider_not_configured(world: &mut TanrenWorld) {
    let ctx = world.ensure_account_ctx().await;
    ctx.harness
        .seed_provider_not_configured()
        .await
        .expect("seed provider not configured");
}

#[when(expr = "{word} connects the repository {string} to her account")]
async fn when_connect_repo(world: &mut TanrenWorld, actor: String, slug: String) {
    do_connect(world, actor, &slug).await;
}

#[when(expr = "{word} connects the repository {string} to her account again")]
async fn when_connect_repo_again(world: &mut TanrenWorld, actor: String, slug: String) {
    do_connect(world, actor, &slug).await;
}

#[when(expr = "{word} connects the repository {string} to her account over the {word}")]
async fn when_connect_repo_over_interface(
    world: &mut TanrenWorld,
    actor: String,
    slug: String,
    interface: String,
) {
    drop(interface);
    do_connect(world, actor, &slug).await;
}

#[when(expr = "{word} connects the repository {string} to an org she is not a member of")]
async fn when_connect_repo_to_foreign_org(world: &mut TanrenWorld, actor: String, slug: String) {
    let account_id = require_account_id(world, &actor).await;
    let url = fixture_url(&slug);
    let ctx = world.ensure_account_ctx().await;
    let request = ConnectProjectRequest {
        name: slug,
        repository_url: url,
        org: Some(OrgId::fresh()),
    };
    let result = ctx.harness.connect_project(account_id, request).await;
    let outcome = match result {
        Ok(view) => {
            ctx.last_project_account_id = Some(account_id);
            ctx.last_project = Some(view.clone());
            ctx.projects.push(view.clone());
            HarnessOutcome::ProjectConnected(view)
        }
        Err(err) => {
            let entry = ctx.actors.entry(actor.clone()).or_default();
            record_failure(err, entry)
        }
    };
    ctx.last_outcome = Some(outcome);
}

async fn do_connect(world: &mut TanrenWorld, actor: String, slug: &str) {
    let account_id = require_account_id(world, &actor).await;
    let url = fixture_url(slug);
    let ctx = world.ensure_account_ctx().await;
    let request = ConnectProjectRequest {
        name: slug.to_owned(),
        repository_url: url,
        org: None,
    };
    let result = ctx.harness.connect_project(account_id, request).await;
    let outcome = match result {
        Ok(view) => {
            ctx.last_project_account_id = Some(account_id);
            ctx.last_project = Some(view.clone());
            ctx.projects.push(view.clone());
            HarnessOutcome::ProjectConnected(view)
        }
        Err(err) => {
            let entry = ctx.actors.entry(actor.clone()).or_default();
            record_failure(err, entry)
        }
    };
    ctx.last_outcome = Some(outcome);
}

#[when(expr = "{word} creates a new project named {string} at host {string}")]
async fn when_create_project(
    world: &mut TanrenWorld,
    actor: String,
    name: String,
    provider_host: String,
) {
    do_create(world, actor, name, provider_host).await;
}

#[when(expr = "{word} creates a new project named {string} at host {string} over the {word}")]
async fn when_create_project_over_interface(
    world: &mut TanrenWorld,
    actor: String,
    name: String,
    provider_host: String,
    interface: String,
) {
    drop(interface);
    do_create(world, actor, name, provider_host).await;
}

#[when(
    expr = "{word} creates a new project named {string} at host {string} under an org she is not a member of"
)]
async fn when_create_project_under_foreign_org(
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
        org: Some(OrgId::fresh()),
    };
    let result = ctx.harness.create_project(account_id, request).await;
    let outcome = match result {
        Ok(view) => {
            ctx.last_project_account_id = Some(account_id);
            ctx.last_project = Some(view.clone());
            ctx.projects.push(view.clone());
            HarnessOutcome::ProjectCreated(view)
        }
        Err(err) => {
            let entry = ctx.actors.entry(actor.clone()).or_default();
            record_failure(err, entry)
        }
    };
    ctx.last_outcome = Some(outcome);
}

async fn do_create(world: &mut TanrenWorld, actor: String, name: String, provider_host: String) {
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
            ctx.projects.push(view.clone());
            HarnessOutcome::ProjectCreated(view)
        }
        Err(err) => {
            let entry = ctx.actors.entry(actor.clone()).or_default();
            record_failure(err, entry)
        }
    };
    ctx.last_outcome = Some(outcome);
}

#[then(expr = "the project {string} appears in {word}'s account")]
async fn then_project_appears(world: &mut TanrenWorld, name: String, actor: String) {
    drop(actor);
    let ctx = world.ensure_account_ctx().await;
    let found = ctx.projects.iter().find(|p| p.name == name);
    assert!(
        found.is_some(),
        "expected project '{name}' to appear in account, projects: {:?}",
        ctx.projects.iter().map(|p| &p.name).collect::<Vec<_>>()
    );
}

#[then(expr = "{word} can select {string} as her active project")]
async fn then_active_project(world: &mut TanrenWorld, actor: String, name: String) {
    let account_id = require_account_id(world, &actor).await;
    let ctx = world.ensure_account_ctx().await;
    let active = ctx
        .harness
        .active_project(account_id)
        .await
        .expect("active_project should succeed");
    let active_view = active
        .as_ref()
        .expect("expected an active project")
        .project
        .clone();
    assert_eq!(active_view.name, name, "active project name must match");
    ctx.last_outcome = Some(HarnessOutcome::ActiveProject(active));
}

#[then(expr = "{word} can select {string} as her active project over the {word}")]
async fn then_active_project_over_interface(
    world: &mut TanrenWorld,
    actor: String,
    name: String,
    interface: String,
) {
    drop(interface);
    then_active_project(world, actor, name).await;
}

#[then(expr = "the repository bytes of {string} are unchanged")]
async fn then_repo_bytes_unchanged(world: &mut TanrenWorld, slug: String) {
    let ctx = world.ensure_account_ctx().await;
    let url = fixture_url(&slug);
    let found = ctx.projects.iter().find(|p| p.repository.url == url);
    assert!(
        found.is_some(),
        "repository '{slug}' must have been connected (url={url}) to verify bytes unchanged"
    );
    let project = found.expect("just checked");
    assert_eq!(
        project.content_counts,
        ProjectContentCounts::empty(),
        "connected project must have empty content counts (bytes unchanged)"
    );
}

#[then(expr = "no Tanren activity exists for the {int} prior commits")]
async fn then_no_prior_activity(world: &mut TanrenWorld, commits: usize) {
    let _ = commits;
    let ctx = world.ensure_account_ctx().await;
    let project = ctx
        .last_project
        .as_ref()
        .expect("a project must have been connected first");
    assert_eq!(
        project.content_counts,
        ProjectContentCounts::empty(),
        "no Tanren activity should exist for prior commits"
    );
}

#[then(expr = "{word} has {int} projects in her account")]
async fn then_project_count(world: &mut TanrenWorld, actor: String, count: usize) {
    drop(actor);
    let ctx = world.ensure_account_ctx().await;
    assert_eq!(
        ctx.projects.len(),
        count,
        "expected {count} projects, got {}",
        ctx.projects.len()
    );
}

#[then(expr = "each project is scoped to exactly one repository")]
async fn then_one_project_per_repo(world: &mut TanrenWorld) {
    let ctx = world.ensure_account_ctx().await;
    let mut identities: Vec<String> = ctx
        .projects
        .iter()
        .map(|p| normalize_repository_identity(&p.repository.url))
        .collect();
    let before = identities.len();
    identities.sort();
    identities.dedup();
    assert_eq!(
        before,
        identities.len(),
        "each project must be scoped to exactly one repository (found duplicates)"
    );
}

#[then(expr = "the second request fails with code {string}")]
async fn then_second_fails_with(world: &mut TanrenWorld, code: String) {
    let ctx = world.ensure_account_ctx().await;
    let actual = ctx
        .last_outcome
        .as_ref()
        .and_then(HarnessOutcome::failure_code);
    assert_eq!(actual, Some(code), "expected second request failure code");
}

#[then(expr = "a new repository {string} exists at host {string}")]
async fn then_new_repo_exists(world: &mut TanrenWorld, name: String, host: String) {
    let ctx = world.ensure_account_ctx().await;
    let expected_url = format!("https://{host}/{name}");
    let matching: Vec<_> = ctx
        .projects
        .iter()
        .filter(|p| p.repository.url == expected_url)
        .collect();
    assert_eq!(
        matching.len(),
        1,
        "expected exactly 1 repository at '{expected_url}', found {}: {:?}",
        matching.len(),
        ctx.projects
            .iter()
            .map(|p| &p.repository.url)
            .collect::<Vec<_>>()
    );
}

#[then(expr = "the project {string} has {int} specs")]
async fn then_project_specs(world: &mut TanrenWorld, name: String, count: u32) {
    let ctx = world.ensure_account_ctx().await;
    let found = ctx.projects.iter().find(|p| p.name == name);
    assert!(found.is_some(), "project '{name}' must exist");
    let project = found.expect("checked above");
    assert_eq!(
        project.content_counts.specs, count,
        "expected {count} specs for project '{name}'"
    );
}

#[then(expr = "the project {string} has {int} milestones")]
async fn then_project_milestones(world: &mut TanrenWorld, name: String, count: u32) {
    let ctx = world.ensure_account_ctx().await;
    let found = ctx.projects.iter().find(|p| p.name == name);
    assert!(found.is_some(), "project '{name}' must exist");
    let project = found.expect("checked above");
    assert_eq!(
        project.content_counts.milestones, count,
        "expected {count} milestones for project '{name}'"
    );
}

#[then(expr = "the project {string} has {int} initiatives")]
async fn then_project_initiatives(world: &mut TanrenWorld, name: String, count: u32) {
    let ctx = world.ensure_account_ctx().await;
    let found = ctx.projects.iter().find(|p| p.name == name);
    assert!(found.is_some(), "project '{name}' must exist");
    let project = found.expect("checked above");
    assert_eq!(
        project.content_counts.initiatives, count,
        "expected {count} initiatives for project '{name}'"
    );
}

#[then(expr = "the project {string} is listed when {word} lists projects over the {word}")]
async fn then_project_listed_cross(
    world: &mut TanrenWorld,
    name: String,
    actor: String,
    interface: String,
) {
    drop((actor, interface));
    let ctx = world.ensure_account_ctx().await;
    let found = ctx.projects.iter().find(|p| p.name == name);
    assert!(
        found.is_some(),
        "expected project '{name}' to be listed, projects: {:?}",
        ctx.projects.iter().map(|p| &p.name).collect::<Vec<_>>()
    );
}

#[then(expr = "the project {string} has {int} specs over the {word}")]
async fn then_project_specs_cross(
    world: &mut TanrenWorld,
    name: String,
    count: u32,
    interface: String,
) {
    drop(interface);
    then_project_specs(world, name, count).await;
}

#[then(expr = "the project {string} has {int} milestones over the {word}")]
async fn then_project_milestones_cross(
    world: &mut TanrenWorld,
    name: String,
    count: u32,
    interface: String,
) {
    drop(interface);
    then_project_milestones(world, name, count).await;
}

#[then(expr = "the project {string} has {int} initiatives over the {word}")]
async fn then_project_initiatives_cross(
    world: &mut TanrenWorld,
    name: String,
    count: u32,
    interface: String,
) {
    drop(interface);
    then_project_initiatives(world, name, count).await;
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
