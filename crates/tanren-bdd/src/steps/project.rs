use cucumber::{given, then, when};
use tanren_contract::{ConnectProjectRequest, DisconnectProjectRequest};
use tanren_identity_policy::ProjectId;
use tanren_testkit::{ProjectOutcome, RepositoryFixture, record_project_failure};

use crate::TanrenWorld;

#[given(expr = "a connected project {string}")]
async fn given_connected_project(world: &mut TanrenWorld, name: String) {
    let ctx = world.project.as_mut().expect("project context required");
    let provider_connection_id = ctx.harness.provider_connection_id();
    let resource_id = name.replace(' ', "-");
    let result = ctx
        .harness
        .connect_project(ConnectProjectRequest {
            account_id: None,
            org_id: ctx.org_id,
            name: name.clone(),
            provider_connection_id,
            resource_id,
        })
        .await;
    match result {
        Ok(response) => {
            ctx.connected_project_id = Some(response.project.id);
            ctx.connected_project = Some(response.project);
        }
        Err(err) => {
            let _ = ctx.harness.kind();
            ctx.last_outcome = Some(record_project_failure(err, &mut ctx.last_failure));
        }
    }
}

#[given(expr = "a spec titled {string} exists for the project")]
async fn given_spec(world: &mut TanrenWorld, title: String) {
    let ctx = world.project.as_mut().expect("project context required");
    let project_id = ctx
        .connected_project_id
        .expect("project must be connected first");
    let spec_id = ctx
        .harness
        .seed_spec(project_id, title)
        .await
        .expect("seed spec");
    ctx.seeded_spec_ids.push(spec_id);
}

#[given(expr = "a cross-project dependency from the project to project {string}")]
async fn given_cross_project_dependency(world: &mut TanrenWorld, target_name: String) {
    let _ = target_name;
    let ctx = world.project.as_mut().expect("project context required");
    let source_project_id = ctx
        .connected_project_id
        .expect("project must be connected first");
    let source_spec_id = ctx
        .seeded_spec_ids
        .first()
        .copied()
        .expect("at least one spec must exist");
    let target_project_id = ProjectId::fresh();
    ctx.harness
        .seed_dependency(source_project_id, source_spec_id, target_project_id)
        .await
        .expect("seed dependency");
}

#[given(expr = "an active implementation loop exists on the project")]
async fn given_active_loop(world: &mut TanrenWorld) {
    let ctx = world.project.as_mut().expect("project context required");
    let project_id = ctx
        .connected_project_id
        .expect("project must be connected first");
    ctx.harness
        .seed_active_loop(project_id)
        .await
        .expect("seed active loop");
}

#[given(expr = "a temp repository fixture")]
async fn given_temp_repo(world: &mut TanrenWorld) {
    let ctx = world.ensure_project_ctx().await;
    let repo = RepositoryFixture::create("bdd-temp").expect("create temp repo");
    let checksum = repo.checksum().expect("compute checksum");
    ctx.temp_repo = Some(repo);
    ctx.checksum_before = Some(checksum);
}

#[when(expr = "the project is disconnected")]
async fn when_disconnect(world: &mut TanrenWorld) {
    let ctx = world.project.as_mut().expect("project context required");
    let project_id = ctx.connected_project_id.expect("project must be connected");
    ctx.spec_count_before_disconnect = ctx
        .harness
        .project_specs(project_id)
        .await
        .ok()
        .map(|specs| specs.len());
    let result = ctx
        .harness
        .disconnect_project(DisconnectProjectRequest {
            project_id,
            account_id: None,
        })
        .await;
    match result {
        Ok(response) => {
            ctx.last_disconnect_unresolved
                .clone_from(&response.unresolved_inbound_dependencies);
            ctx.last_outcome = Some(ProjectOutcome::Disconnected(response));
        }
        Err(err) => {
            ctx.last_outcome = Some(record_project_failure(err, &mut ctx.last_failure));
        }
    }
}

#[when(expr = "the project is reconnected")]
async fn when_reconnect(world: &mut TanrenWorld) {
    let ctx = world.project.as_mut().expect("project context required");
    let project_id = ctx
        .connected_project_id
        .expect("project must have been connected");
    let result = ctx.harness.reconnect_project(project_id).await;
    match result {
        Ok(response) => {
            ctx.last_outcome = Some(ProjectOutcome::Reconnected(response.project));
        }
        Err(err) => {
            ctx.last_outcome = Some(record_project_failure(err, &mut ctx.last_failure));
        }
    }
}

#[when(expr = "projects are listed")]
async fn when_list_projects(world: &mut TanrenWorld) {
    let ctx = world.project.as_mut().expect("project context required");
    let account_id = ctx.account_id;
    let result = ctx.harness.list_projects(account_id).await;
    match result {
        Ok(response) => {
            ctx.last_listed_projects = response.projects;
        }
        Err(err) => {
            ctx.last_outcome = Some(record_project_failure(err, &mut ctx.last_failure));
        }
    }
}

#[then(expr = "the project no longer appears in the project list")]
async fn then_project_not_listed(world: &mut TanrenWorld) {
    let ctx = world.ensure_project_ctx().await;
    let project_id = ctx
        .connected_project_id
        .expect("project must have been connected");
    let listed = &ctx.last_listed_projects;
    let found = listed.iter().any(|p| p.id == project_id);
    assert!(!found, "project should not appear in the project list");
}

#[then(expr = "the project appears in the project list")]
async fn then_project_listed(world: &mut TanrenWorld) {
    let ctx = world.ensure_project_ctx().await;
    let project_id = ctx
        .connected_project_id
        .expect("project must have been connected");
    let listed = &ctx.last_listed_projects;
    let found = listed.iter().any(|p| p.id == project_id);
    assert!(found, "project should appear in the project list");
}

#[then(expr = "the repository byte checksum is unchanged")]
async fn then_repo_unchanged(world: &mut TanrenWorld) {
    let ctx = world.ensure_project_ctx().await;
    let repo = ctx.temp_repo.as_ref().expect("temp repo must exist");
    let before = ctx
        .checksum_before
        .as_ref()
        .expect("checksum before must exist");
    let after = repo.checksum().expect("compute checksum");
    assert_eq!(
        before, &after,
        "repository checksum must be unchanged after disconnect"
    );
}

#[then(expr = "{int} unresolved inbound dependencies are reported")]
async fn then_unresolved_deps(world: &mut TanrenWorld, count: usize) {
    let ctx = world.ensure_project_ctx().await;
    let unresolved = &ctx.last_disconnect_unresolved;
    assert_eq!(
        unresolved.len(),
        count,
        "unexpected number of unresolved dependencies"
    );
}

#[then(expr = "the disconnect is rejected with code {string}")]
async fn then_disconnect_rejected(world: &mut TanrenWorld, code: String) {
    let ctx = world.ensure_project_ctx().await;
    let outcome = ctx.last_outcome.as_ref().expect("must have an outcome");
    assert!(
        matches!(outcome, ProjectOutcome::Failure(r) if r.code() == code),
        "expected project failure with code {code}, got {outcome:?}"
    );
}

#[then(expr = "the prior specs reappear for the project")]
async fn then_specs_reappear(world: &mut TanrenWorld) {
    let ctx = world.project.as_mut().expect("project context required");
    let project_id = ctx.connected_project_id.expect("project must be connected");
    let current = ctx
        .harness
        .project_specs(project_id)
        .await
        .expect("fetch specs");
    if let Some(before_count) = ctx.spec_count_before_disconnect {
        assert_eq!(
            current.len(),
            before_count,
            "spec count must match before disconnect"
        );
    } else {
        let expected = ctx.seeded_spec_ids.len();
        assert!(
            current.len() >= expected,
            "specs should reappear after reconnect"
        );
    }
}
