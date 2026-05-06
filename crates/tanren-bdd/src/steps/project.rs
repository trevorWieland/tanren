//! Project-flow step definitions for R-0020.
//!
//! Step bodies dispatch through the per-interface
//! [`ProjectHarness`](tanren_testkit::ProjectHarness) trait — never
//! `tanren_app_services::Handlers::*` directly. The active harness is
//! selected from the scenario's tags via the same `Before` hook that
//! drives the account-flow harness.

use chrono::Utc;
use cucumber::{given, then, when};
use tanren_identity_policy::{AccountId, ProjectId, SpecId};
use tanren_testkit::{HarnessError, HarnessProjectFixture, HarnessSpecFixture};

use crate::TanrenWorld;

#[given(expr = "account {word} has project {word} named {string}")]
async fn given_account_has_project(
    world: &mut TanrenWorld,
    account_name: String,
    project_name: String,
    display_name: String,
) {
    let ctx = world.ensure_project_ctx().await;
    let account_id = ctx
        .account_ids
        .get(&account_name)
        .copied()
        .unwrap_or_else(AccountId::fresh);
    if !ctx.account_ids.contains_key(&account_name) {
        ctx.account_ids.insert(account_name.clone(), account_id);
    }
    let project_id = ProjectId::fresh();
    let fixture = HarnessProjectFixture {
        id: project_id,
        account_id,
        name: display_name,
        state: "active".to_owned(),
        created_at: Utc::now(),
    };
    ctx.harness
        .seed_project(fixture)
        .await
        .expect("seed project");
    ctx.project_ids.insert(project_name, project_id);
}

#[given(
    expr = "project {word} has a spec {word} named {string} needing attention because {string}"
)]
async fn given_project_has_attention_spec(
    world: &mut TanrenWorld,
    project_name: String,
    spec_name: String,
    display_name: String,
    reason: String,
) {
    let ctx = world.ensure_project_ctx().await;
    let project_id = ctx
        .project_ids
        .get(&project_name)
        .copied()
        .expect("project must have been seeded");
    let spec_id = SpecId::fresh();
    let fixture = HarnessSpecFixture {
        id: spec_id,
        project_id,
        name: display_name,
        needs_attention: true,
        attention_reason: Some(reason),
        created_at: Utc::now(),
    };
    ctx.harness.seed_spec(fixture).await.expect("seed spec");
    ctx.spec_ids.insert(spec_name, spec_id);
}

#[given(expr = "project {word} has a spec {word} named {string} with no attention needed")]
async fn given_project_has_healthy_spec(
    world: &mut TanrenWorld,
    project_name: String,
    spec_name: String,
    display_name: String,
) {
    let ctx = world.ensure_project_ctx().await;
    let project_id = ctx
        .project_ids
        .get(&project_name)
        .copied()
        .expect("project must have been seeded");
    let spec_id = SpecId::fresh();
    let fixture = HarnessSpecFixture {
        id: spec_id,
        project_id,
        name: display_name,
        needs_attention: false,
        attention_reason: None,
        created_at: Utc::now(),
    };
    ctx.harness.seed_spec(fixture).await.expect("seed spec");
    ctx.spec_ids.insert(spec_name, spec_id);
}

#[given(expr = "account {word} has view state for project {word}")]
async fn given_view_state(world: &mut TanrenWorld, account_name: String, project_name: String) {
    let ctx = world.ensure_project_ctx().await;
    let account_id = ctx
        .account_ids
        .get(&account_name)
        .copied()
        .expect("account must have been seeded");
    let project_id = ctx
        .project_ids
        .get(&project_name)
        .copied()
        .expect("project must have been seeded");
    let state = serde_json::json!({"scroll_position": 42});
    ctx.harness
        .seed_view_state(account_id, project_id, state)
        .await
        .expect("seed view state");
}

#[when(expr = "account {word} lists their projects")]
async fn when_list_projects(world: &mut TanrenWorld, account_name: String) {
    let ctx = world.ensure_project_ctx().await;
    let account_id = ctx
        .account_ids
        .get(&account_name)
        .copied()
        .expect("account must have been seeded");
    match ctx.harness.list_projects(account_id).await {
        Ok(projects) => {
            ctx.last_project_list = Some(projects);
            ctx.last_failure = None;
        }
        Err(HarnessError::Project(reason, _)) => {
            ctx.last_failure = Some(reason);
            ctx.last_project_list = None;
        }
        Err(HarnessError::Transport(_) | HarnessError::Account(_, _)) => {
            ctx.last_failure = None;
            ctx.last_project_list = None;
        }
    }
}

#[when(expr = "account {word} switches to project {word}")]
async fn when_switch_project(world: &mut TanrenWorld, account_name: String, project_name: String) {
    let ctx = world.ensure_project_ctx().await;
    let account_id = ctx
        .account_ids
        .get(&account_name)
        .copied()
        .expect("account must have been seeded");
    let project_id = ctx
        .project_ids
        .get(&project_name)
        .copied()
        .expect("project must have been seeded");
    match ctx
        .harness
        .switch_active_project(account_id, project_id)
        .await
    {
        Ok(response) => {
            ctx.last_switch_response = Some(response);
            ctx.last_failure = None;
        }
        Err(HarnessError::Project(reason, _)) => {
            ctx.last_failure = Some(reason);
            ctx.last_switch_response = None;
        }
        Err(_) => {
            ctx.last_switch_response = None;
        }
    }
}

#[when(expr = "account {word} drills down into the attention spec in project {word} named {word}")]
async fn when_drill_down_attention(
    world: &mut TanrenWorld,
    account_name: String,
    project_name: String,
    spec_name: String,
) {
    let ctx = world.ensure_project_ctx().await;
    let account_id = ctx
        .account_ids
        .get(&account_name)
        .copied()
        .expect("account must have been seeded");
    let project_id = ctx
        .project_ids
        .get(&project_name)
        .copied()
        .expect("project must have been seeded");
    let spec_id = ctx
        .spec_ids
        .get(&spec_name)
        .copied()
        .expect("spec must have been seeded");
    match ctx
        .harness
        .attention_spec(account_id, project_id, spec_id)
        .await
    {
        Ok(view) => {
            ctx.last_attention_spec = Some(view);
            ctx.last_failure = None;
        }
        Err(HarnessError::Project(reason, _)) => {
            ctx.last_failure = Some(reason);
            ctx.last_attention_spec = None;
        }
        Err(_) => {
            ctx.last_attention_spec = None;
        }
    }
}

#[when(expr = "account {word} views the scoped views for their active project")]
async fn when_scoped_views(world: &mut TanrenWorld, account_name: String) {
    let ctx = world.ensure_project_ctx().await;
    let account_id = ctx
        .account_ids
        .get(&account_name)
        .copied()
        .expect("account must have been seeded");
    match ctx.harness.project_scoped_views(account_id).await {
        Ok(views) => {
            ctx.last_scoped_views = Some(views);
            ctx.last_failure = None;
        }
        Err(HarnessError::Project(reason, _)) => {
            ctx.last_failure = Some(reason);
            ctx.last_scoped_views = None;
        }
        Err(_) => {
            ctx.last_scoped_views = None;
        }
    }
}

#[then(expr = "the project list contains {int} projects")]
async fn then_project_count(world: &mut TanrenWorld, count: usize) {
    let ctx = world.ensure_project_ctx().await;
    let list = ctx
        .last_project_list
        .as_ref()
        .expect("project list must have been fetched");
    assert_eq!(
        list.len(),
        count,
        "expected {count} projects, got {}",
        list.len()
    );
}

#[then(expr = "project {word} needs attention")]
async fn then_project_needs_attention(world: &mut TanrenWorld, project_name: String) {
    let ctx = world.ensure_project_ctx().await;
    let project_id = ctx
        .project_ids
        .get(&project_name)
        .copied()
        .expect("project must have been seeded");
    let list = ctx
        .last_project_list
        .as_ref()
        .expect("project list must have been fetched");
    let project = list
        .iter()
        .find(|p| p.id == project_id)
        .expect("project not found in list");
    assert!(
        project.needs_attention,
        "expected {project_name} to need attention"
    );
}

#[then(expr = "project {word} does not need attention")]
async fn then_project_no_attention(world: &mut TanrenWorld, project_name: String) {
    let ctx = world.ensure_project_ctx().await;
    let project_id = ctx
        .project_ids
        .get(&project_name)
        .copied()
        .expect("project must have been seeded");
    let list = ctx
        .last_project_list
        .as_ref()
        .expect("project list must have been fetched");
    let project = list
        .iter()
        .find(|p| p.id == project_id)
        .expect("project not found in list");
    assert!(
        !project.needs_attention,
        "expected {project_name} to not need attention"
    );
}

#[then(expr = "the project list does not contain project {word}")]
async fn then_project_not_in_list(world: &mut TanrenWorld, project_name: String) {
    let ctx = world.ensure_project_ctx().await;
    let project_id = ctx
        .project_ids
        .get(&project_name)
        .copied()
        .expect("project must have been seeded");
    let list = ctx
        .last_project_list
        .as_ref()
        .expect("project list must have been fetched");
    let found = list.iter().any(|p| p.id == project_id);
    assert!(!found, "project {project_name} should not be in the list");
}

#[then(expr = "the active project is {word}")]
async fn then_active_project(world: &mut TanrenWorld, project_name: String) {
    let ctx = world.ensure_project_ctx().await;
    let expected_id = ctx
        .project_ids
        .get(&project_name)
        .copied()
        .expect("project must have been seeded");
    let resp = ctx
        .last_switch_response
        .as_ref()
        .expect("switch response must exist");
    assert_eq!(
        resp.project.id, expected_id,
        "expected active project to be {project_name}"
    );
}

#[then(expr = "the attention spec reason is {string}")]
async fn then_attention_reason(world: &mut TanrenWorld, expected_reason: String) {
    let ctx = world.ensure_project_ctx().await;
    let spec = ctx
        .last_attention_spec
        .as_ref()
        .expect("attention spec must have been fetched");
    assert_eq!(spec.reason, expected_reason, "attention reason mismatch");
}

#[then(expr = "the scoped views show {int} specs")]
async fn then_scoped_specs_count(world: &mut TanrenWorld, count: usize) {
    let ctx = world.ensure_project_ctx().await;
    let views = ctx
        .last_scoped_views
        .as_ref()
        .expect("scoped views must have been fetched");
    assert_eq!(
        views.specs.len(),
        count,
        "expected {count} specs in scoped views"
    );
}
