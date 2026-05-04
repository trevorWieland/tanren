use cucumber::{given, then, when};
use tanren_contract::CreateOrganizationRequest;
use tanren_identity_policy::OrgName;
use tanren_testkit::{HarnessOutcome, record_failure};

use crate::TanrenWorld;

#[when(expr = "{word} creates an organization named {string}")]
async fn when_create_org(world: &mut TanrenWorld, actor: String, name: String) {
    do_create_org(world, actor, name).await;
}

#[given(expr = "{word} has created an organization named {string}")]
async fn given_created_org(world: &mut TanrenWorld, actor: String, name: String) {
    do_create_org(world, actor, name).await;
    let ctx = world.account.as_mut().expect("ctx initialized");
    assert!(
        matches!(ctx.last_outcome, Some(HarnessOutcome::OrgCreated(_))),
        "background org creation must succeed (got {last:?})",
        last = ctx.last_outcome,
    );
}

async fn do_create_org(world: &mut TanrenWorld, actor: String, name: String) {
    let ctx = world.ensure_account_ctx().await;
    let org_name = OrgName::parse(&name).expect("scenario org names must parse");
    let request = CreateOrganizationRequest { name: org_name };
    let result = ctx.harness.create_organization(request).await;
    let entry = ctx.actors.entry(actor.clone()).or_default();
    let outcome = match result {
        Ok(creation) => {
            let org_id = creation.organization.id;
            ctx.orgs_by_name.insert(name.clone(), org_id);
            entry.identifier = Some(name.clone());
            HarnessOutcome::OrgCreated(creation)
        }
        Err(err) => record_failure(err, entry),
    };
    ctx.last_outcome = Some(outcome);
}

#[when(expr = "an unsigned-in attempt creates an organization named {string}")]
async fn when_unauthed_create_org(world: &mut TanrenWorld, name: String) {
    let ctx = world.ensure_account_ctx().await;
    let org_name = OrgName::parse(&name).expect("scenario org names must parse");
    let request = CreateOrganizationRequest { name: org_name };
    let result = ctx.harness.create_organization(request).await;
    let outcome = match result {
        Ok(creation) => HarnessOutcome::OrgCreated(creation),
        Err(err) => {
            let mut dummy = tanren_testkit::ActorState::default();
            record_failure(err, &mut dummy)
        }
    };
    ctx.last_outcome = Some(outcome);
}

#[then(expr = "the response includes full bootstrap permissions")]
async fn then_full_bootstrap_permissions(world: &mut TanrenWorld) {
    let ctx = world.ensure_account_ctx().await;
    match &ctx.last_outcome {
        Some(HarnessOutcome::OrgCreated(creation)) => {
            assert!(
                creation.permissions.is_full(),
                "expected full bootstrap permissions, got {permissions:?}",
                permissions = creation.permissions,
            );
        }
        other => {
            let _ = other;
            unreachable!("expected OrgCreated outcome");
        }
    }
}

#[then(expr = "{string} appears in {word}'s organization list")]
async fn then_org_appears_in_list(world: &mut TanrenWorld, org_name: String, actor: String) {
    let ctx = world.ensure_account_ctx().await;
    let result = ctx.harness.list_organizations().await;
    match result {
        Ok(orgs) => {
            let names: Vec<String> = orgs.iter().map(|o| o.name.as_str().to_owned()).collect();
            assert!(
                names.iter().any(|n| n.eq_ignore_ascii_case(&org_name)),
                "expected '{org_name}' in {actor}'s organization list, got {names:?}",
            );
            ctx.last_outcome = Some(HarnessOutcome::Other(format!(
                "org_list_ok:{}",
                names.len()
            )));
        }
        Err(err) => {
            let mut dummy = tanren_testkit::ActorState::default();
            ctx.last_outcome = Some(record_failure(err, &mut dummy));
        }
    }
}

#[then(expr = "{word}'s admin permissions on {string} are empty")]
async fn then_admin_permissions_empty(world: &mut TanrenWorld, actor: String, org_name: String) {
    let ctx = world.ensure_account_ctx().await;
    let org_id = ctx
        .orgs_by_name
        .get(&org_name)
        .expect("org must have been created earlier in the scenario");
    let permissions = ctx
        .harness
        .admin_permissions_for_org(*org_id)
        .await
        .expect("admin_permissions_for_org should succeed");
    assert!(
        permissions.is_empty(),
        "expected empty admin permissions for {actor} on '{org_name}', got {permissions:?}",
    );
    ctx.last_outcome = Some(HarnessOutcome::Other(
        "admin_permissions_checked".to_owned(),
    ));
}
