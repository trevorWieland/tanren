//! Organization-switching step definitions for R-0004.
//!
//! Step bodies dispatch through the per-interface
//! [`AccountHarness`](tanren_testkit::AccountHarness) trait — never
//! `tanren_app_services::Handlers::*` directly. The active harness is
//! selected by the BDD `Before` hook from the scenario's tags.

use cucumber::{given, then, when};
use tanren_identity_policy::{OrgId, ProjectId};
use tanren_testkit::{HarnessOrganization, HarnessOutcome, HarnessProject, record_failure};
use uuid::Uuid;

use crate::TanrenWorld;

fn parse_org_id(s: &str) -> OrgId {
    OrgId::new(Uuid::parse_str(s).expect("scenario org ids must be valid UUIDs"))
}

fn parse_project_id(s: &str) -> ProjectId {
    ProjectId::new(Uuid::parse_str(s).expect("scenario project ids must be valid UUIDs"))
}

#[given(
    expr = "{word} has signed up and belongs to organization {string} named {string} and organization {string} named {string}"
)]
async fn given_account_with_two_orgs(
    world: &mut TanrenWorld,
    actor: String,
    first_org_id: String,
    first_org_name: String,
    second_org_id: String,
    second_org_name: String,
) {
    let ctx = world.ensure_account_ctx().await;
    let entry = ctx.actors.entry(actor.clone()).or_default();
    let account_id = entry
        .sign_up
        .as_ref()
        .map(|s| s.account_id)
        .or_else(|| entry.sign_in.as_ref().map(|s| s.account_id))
        .expect("actor must have signed up before org fixture steps");

    let org_x = parse_org_id(&first_org_id);
    let org_y = parse_org_id(&second_org_id);

    ctx.harness
        .seed_organization(HarnessOrganization {
            org_id: org_x,
            org_name: first_org_name,
        })
        .await
        .expect("seed org X");
    ctx.harness
        .seed_organization(HarnessOrganization {
            org_id: org_y,
            org_name: second_org_name,
        })
        .await
        .expect("seed org Y");
    ctx.harness
        .seed_membership(account_id, org_x)
        .await
        .expect("seed membership org X");
    ctx.harness
        .seed_membership(account_id, org_y)
        .await
        .expect("seed membership org Y");
}

#[given(expr = "organization {string} has project {string} named {string}")]
async fn given_org_has_project(
    world: &mut TanrenWorld,
    org_id_str: String,
    project_id_str: String,
    project_name: String,
) {
    let ctx = world.ensure_account_ctx().await;
    let org_id = parse_org_id(&org_id_str);
    let project_id = parse_project_id(&project_id_str);
    ctx.harness
        .seed_project(HarnessProject {
            project_id,
            org_id,
            project_name,
        })
        .await
        .expect("seed project");
}

#[given(expr = "{word} is a personal account with zero organizations")]
async fn given_personal_account(world: &mut TanrenWorld, actor: String) {
    let ctx = world.ensure_account_ctx().await;
    let entry = ctx.actors.entry(actor.clone()).or_default();
    assert!(
        entry.sign_up.is_some() || entry.sign_in.is_some(),
        "actor {actor} must have signed up before the personal-account assertion"
    );
    let account_id = entry
        .sign_up
        .as_ref()
        .map(|s| s.account_id)
        .or_else(|| entry.sign_in.as_ref().map(|s| s.account_id))
        .expect("actor must have a session");
    let result = ctx.harness.list_organizations(account_id).await;
    match result {
        Ok(switcher) => {
            assert!(
                switcher.memberships.is_empty(),
                "personal account should have zero org memberships"
            );
            assert!(
                switcher.active_org.is_none(),
                "personal account should have no active org"
            );
        }
        Err(err) => {
            let outcome = record_failure(err, entry);
            ctx.last_outcome = Some(outcome);
        }
    }
}

#[when(expr = "{word} lists their organizations")]
async fn when_list_organizations(world: &mut TanrenWorld, actor: String) {
    let ctx = world.ensure_account_ctx().await;
    let account_id = actor_account_id(ctx, &actor);
    let result = ctx.harness.list_organizations(account_id).await;
    let entry = ctx.actors.entry(actor.clone()).or_default();
    let outcome = match result {
        Ok(switcher) => {
            entry.last_org_list = Some(switcher.clone());
            HarnessOutcome::OrganizationsListed(switcher)
        }
        Err(err) => record_failure(err, entry),
    };
    ctx.last_outcome = Some(outcome);
}

#[when(expr = "{word} switches active organization to {string}")]
async fn when_switch_active_org(world: &mut TanrenWorld, actor: String, org_id_str: String) {
    let ctx = world.ensure_account_ctx().await;
    let account_id = actor_account_id(ctx, &actor);
    let org_id = parse_org_id(&org_id_str);
    let result = ctx.harness.switch_active_org(account_id, org_id).await;
    let entry = ctx.actors.entry(actor.clone()).or_default();
    let outcome = match result {
        Ok(response) => {
            entry.last_org_switch = Some(response.clone());
            HarnessOutcome::OrganizationSwitched(response)
        }
        Err(err) => record_failure(err, entry),
    };
    ctx.last_outcome = Some(outcome);
}

#[when(expr = "{word} tries to switch active organization to {string}")]
async fn when_try_switch_active_org(world: &mut TanrenWorld, actor: String, org_id_str: String) {
    when_switch_active_org(world, actor, org_id_str).await;
}

#[when(expr = "{word} lists the active organization projects")]
async fn when_list_active_org_projects(world: &mut TanrenWorld, actor: String) {
    let ctx = world.ensure_account_ctx().await;
    let account_id = actor_account_id(ctx, &actor);
    let result = ctx.harness.list_active_org_projects(account_id).await;
    let entry = ctx.actors.entry(actor.clone()).or_default();
    let outcome = match result {
        Ok(projects) => {
            entry.last_project_list = Some(projects.clone());
            HarnessOutcome::OrgProjectsListed(projects)
        }
        Err(err) => record_failure(err, entry),
    };
    ctx.last_outcome = Some(outcome);
}

#[then(expr = "{word} sees {int} organization memberships")]
async fn then_sees_n_org_memberships(world: &mut TanrenWorld, actor: String, count: usize) {
    let ctx = world.ensure_account_ctx().await;
    let entry = ctx
        .actors
        .get(&actor)
        .expect("actor must have an outcome recorded");
    let switcher = entry
        .last_org_list
        .as_ref()
        .expect("actor must have listed organizations");
    assert_eq!(
        switcher.memberships.len(),
        count,
        "expected {count} org memberships, got {}",
        switcher.memberships.len()
    );
}

#[then(expr = "{word} sees the organization switcher is empty or disabled")]
async fn then_org_switcher_empty_or_disabled(world: &mut TanrenWorld, actor: String) {
    let ctx = world.ensure_account_ctx().await;
    let entry = ctx
        .actors
        .get(&actor)
        .expect("actor must have an outcome recorded");
    let switcher = entry
        .last_org_list
        .as_ref()
        .expect("actor must have listed organizations");
    assert!(
        switcher.memberships.is_empty(),
        "expected empty switcher (no memberships), got {} memberships",
        switcher.memberships.len()
    );
    assert!(
        switcher.active_org.is_none(),
        "expected no active org for personal account"
    );
}

#[then(expr = "{word} sees only projects belonging to {string}")]
async fn then_sees_only_projects_for_org(
    world: &mut TanrenWorld,
    actor: String,
    org_id_str: String,
) {
    let ctx = world.ensure_account_ctx().await;
    let org_id = parse_org_id(&org_id_str);
    let entry = ctx
        .actors
        .get(&actor)
        .expect("actor must have an outcome recorded");
    let projects = entry
        .last_project_list
        .as_ref()
        .expect("actor must have listed projects");
    for project in &projects.projects {
        assert_eq!(
            project.org, org_id,
            "project '{}' belongs to org {}, expected {}",
            project.name, project.org, org_id
        );
    }
}

#[then(expr = "{word} sees {int} projects")]
async fn then_sees_n_projects(world: &mut TanrenWorld, actor: String, count: usize) {
    let ctx = world.ensure_account_ctx().await;
    let entry = ctx
        .actors
        .get(&actor)
        .expect("actor must have an outcome recorded");
    let projects = entry
        .last_project_list
        .as_ref()
        .expect("actor must have listed projects");
    assert_eq!(
        projects.projects.len(),
        count,
        "expected {count} projects, got {}",
        projects.projects.len()
    );
}

#[then(expr = "the active organization is {string}")]
async fn then_active_org_is(world: &mut TanrenWorld, org_id_str: String) {
    let ctx = world.ensure_account_ctx().await;
    let org_id = parse_org_id(&org_id_str);
    let response = ctx
        .last_outcome
        .as_ref()
        .expect("expected an outcome but none was recorded");
    let switched = match response {
        HarnessOutcome::OrganizationSwitched(r) => Some(r),
        _ => None,
    }
    .expect("expected OrganizationSwitched outcome");
    assert_eq!(
        switched.account.org,
        Some(org_id),
        "expected active org to be {org_id_str}"
    );
}

fn actor_account_id(ctx: &crate::AccountContext, actor: &str) -> tanren_identity_policy::AccountId {
    let msg = format!("actor {actor} must have signed up first");
    let entry = ctx.actors.get(actor).expect(&msg);
    let session_msg = format!("actor {actor} must have a session");
    entry
        .sign_up
        .as_ref()
        .map(|s| s.account_id)
        .or_else(|| entry.sign_in.as_ref().map(|s| s.account_id))
        .expect(&session_msg)
}
