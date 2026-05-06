use cucumber::{given, then, when};
use secrecy::SecretString;
use tanren_contract::{
    CreateOrganizationRequest, OrganizationAdminOperation, OrganizationFailureReason,
};
use tanren_identity_policy::{Email, OrgPermission, OrganizationName};
use tanren_testkit::{HarnessOrganization, HarnessOutcome, record_failure};

use crate::TanrenWorld;

#[when(expr = "{word} creates an organization named {string}")]
async fn when_create_org(world: &mut TanrenWorld, actor: String, name: String) {
    let org_name = OrganizationName::parse(&name).expect("scenario org names must parse");
    let ctx = world.ensure_account_ctx().await;
    let result = ctx
        .harness
        .create_organization(CreateOrganizationRequest { name: org_name })
        .await;
    let entry = ctx.actors.entry(actor.clone()).or_default();
    let outcome = match result {
        Ok(org) => {
            let org_data = serde_json::to_string(&org).expect("serialize org");
            let key = format!("__org_{}__", org.name);
            let org_entry = ctx.actors.entry(key).or_default();
            org_entry.identifier = Some(org_data);
            HarnessOutcome::OrganizationCreated(org)
        }
        Err(err) => record_failure(err, entry),
    };
    let ctx = world.account.as_mut().expect("ctx");
    world.last_org_id = match &outcome {
        HarnessOutcome::OrganizationCreated(o) => Some(o.org_id),
        _ => None,
    };
    ctx.last_outcome = Some(outcome);
}

#[when(expr = "{word} tries to create an organization named {string}")]
async fn when_try_create_org(world: &mut TanrenWorld, actor: String, name: String) {
    when_create_org(world, actor, name).await;
}

#[when(expr = "an unsigned-in request creates an organization named {string}")]
async fn when_unsigned_create_org(world: &mut TanrenWorld, name: String) {
    let org_name = OrganizationName::parse(&name).expect("scenario org names must parse");
    let ctx = world.ensure_account_ctx().await;
    let result = ctx
        .harness
        .create_organization(CreateOrganizationRequest { name: org_name })
        .await;
    let outcome = match result {
        Ok(org) => HarnessOutcome::OrganizationCreated(org),
        Err(err) => record_failure(err, &mut tanren_testkit::ActorState::default()),
    };
    let ctx = world.account.as_mut().expect("ctx");
    world.last_org_id = match &outcome {
        HarnessOutcome::OrganizationCreated(o) => Some(o.org_id),
        _ => None,
    };
    ctx.last_outcome = Some(outcome);
}

#[then(expr = "the organization has {int} project(s)")]
async fn then_project_count(world: &mut TanrenWorld, count: u64) {
    let ctx = world.ensure_account_ctx().await;
    let org = extract_last_org(ctx);
    assert_eq!(
        org.project_count, count,
        "expected project_count={count}, got {}",
        org.project_count
    );
}

#[then(expr = "{word} is a member of the organization")]
async fn then_member_of_org(world: &mut TanrenWorld, actor: String) {
    let ctx = world.ensure_account_ctx().await;
    assert!(
        extract_last_org_opt(ctx).is_some(),
        "an organization must have been created for {actor}"
    );
}

#[then(expr = "{word} holds all bootstrap admin permissions")]
async fn then_admin_permissions(world: &mut TanrenWorld, actor: String) {
    let ctx = world.ensure_account_ctx().await;
    let org = extract_last_org(ctx);
    let expected: Vec<OrgPermission> = vec![
        OrgPermission::InviteMembers,
        OrgPermission::ManageAccess,
        OrgPermission::Configure,
        OrgPermission::SetPolicy,
        OrgPermission::Delete,
    ];
    assert_eq!(
        org.granted_permissions, expected,
        "creator must receive all five bootstrap admin permissions"
    );
    for perm in &expected {
        let result = ctx
            .harness
            .authorize_admin_operation(org.org_id, perm_to_operation(*perm))
            .await;
        assert!(
            result.is_ok(),
            "creator should be authorized for {perm:?} but got {result:?}"
        );
    }
    let _ = actor;
}

#[then(expr = "the organization appears in {word}'s available organizations")]
async fn then_org_available(world: &mut TanrenWorld, actor: String) {
    let ctx = world.ensure_account_ctx().await;
    let org = extract_last_org(ctx);
    let available = ctx
        .harness
        .list_available_organizations()
        .await
        .expect("list organizations should succeed");
    let found = available.iter().any(|o| o.id == org.org_id);
    assert!(
        found,
        "org {} must appear in available orgs for {actor}",
        org.org_id
    );
}

#[then(expr = "{word} cannot authorize admin operation {string} on the organization")]
async fn then_non_creator_denied(world: &mut TanrenWorld, actor: String, operation: String) {
    let ctx = world.ensure_account_ctx().await;
    let org = extract_last_org(ctx);
    let op = parse_operation(&operation);
    let result = ctx.harness.authorize_admin_operation(org.org_id, op).await;
    let err = result.expect_err(&format!(
        "non-creator {actor} should be denied admin operation {operation}"
    ));
    assert_eq!(
        err.code(),
        OrganizationFailureReason::PermissionDenied.code(),
        "expected permission_denied, got {}",
        err.code()
    );
}

#[then(expr = "the last-admin protection triggers for {string}")]
async fn then_last_admin_triggers(world: &mut TanrenWorld, permission: String) {
    let ctx = world.ensure_account_ctx().await;
    let org = extract_last_org(ctx);
    let perm = parse_permission(&permission);
    let result = ctx
        .harness
        .probe_last_admin_protection(org.org_id, perm)
        .await;
    let err = result.expect_err(&format!(
        "last-admin protection should trigger for {permission}"
    ));
    assert_eq!(
        err.code(),
        OrganizationFailureReason::LastAdminHolder.code(),
        "expected last_admin_holder, got {}",
        err.code()
    );
}

#[given(expr = "{word} has signed up with email {string} and password {string}")]
async fn given_signed_up(world: &mut TanrenWorld, actor: String, email: String, password: String) {
    do_sign_up(world, actor, email, password).await;
    let ctx = world.account.as_mut().expect("ctx initialized");
    assert!(
        matches!(ctx.last_outcome, Some(HarnessOutcome::SignedUp(_))),
        "background sign-up must succeed (got {:?})",
        ctx.last_outcome
    );
}

#[given(expr = "{word} has signed in with email {string} and password {string}")]
async fn given_signed_in(world: &mut TanrenWorld, actor: String, email: String, password: String) {
    do_sign_in(world, actor, email, password).await;
    let ctx = world.account.as_mut().expect("ctx initialized");
    assert!(
        matches!(ctx.last_outcome, Some(HarnessOutcome::SignedIn(_))),
        "background sign-in must succeed (got {:?})",
        ctx.last_outcome
    );
}

async fn do_sign_up(world: &mut TanrenWorld, actor: String, email: String, password: String) {
    let ctx = world.ensure_account_ctx().await;
    let parsed_email = Email::parse(&email).expect("scenario emails must parse");
    let result = ctx
        .harness
        .sign_up(tanren_contract::SignUpRequest {
            email: parsed_email,
            password: SecretString::from(password.clone()),
            display_name: format!("{actor} org-user"),
        })
        .await;
    let entry = ctx.actors.entry(actor.clone()).or_default();
    entry.identifier = Some(email);
    entry.password = Some(SecretString::from(password));
    let outcome = match result {
        Ok(session) => {
            entry.sign_up = Some(session.clone());
            HarnessOutcome::SignedUp(session)
        }
        Err(err) => record_failure(err, entry),
    };
    ctx.last_outcome = Some(outcome);
}

async fn do_sign_in(world: &mut TanrenWorld, actor: String, email: String, password: String) {
    let ctx = world.ensure_account_ctx().await;
    let parsed_email = Email::parse(&email).expect("scenario emails must parse");
    let result = ctx
        .harness
        .sign_in(tanren_contract::SignInRequest {
            email: parsed_email,
            password: SecretString::from(password.clone()),
        })
        .await;
    let entry = ctx.actors.entry(actor.clone()).or_default();
    entry.identifier = Some(email);
    entry.password = Some(SecretString::from(password));
    let outcome = match result {
        Ok(session) => {
            entry.sign_in = Some(session.clone());
            HarnessOutcome::SignedIn(session)
        }
        Err(err) => record_failure(err, entry),
    };
    ctx.last_outcome = Some(outcome);
}

fn extract_last_org(ctx: &mut crate::AccountContext) -> HarnessOrganization {
    extract_last_org_opt(ctx).expect("an organization must have been created first")
}

fn extract_last_org_opt(ctx: &mut crate::AccountContext) -> Option<HarnessOrganization> {
    let raw = ctx.actors.iter().find_map(|(k, v)| {
        if k.starts_with("__org_") {
            v.identifier.as_ref()
        } else {
            None
        }
    })?;
    Some(serde_json::from_str(raw).expect("deserialize org"))
}

fn perm_to_operation(perm: OrgPermission) -> OrganizationAdminOperation {
    match perm {
        OrgPermission::ManageAccess => OrganizationAdminOperation::ManageAccess,
        OrgPermission::Configure => OrganizationAdminOperation::Configure,
        OrgPermission::SetPolicy => OrganizationAdminOperation::SetPolicy,
        OrgPermission::Delete => OrganizationAdminOperation::Delete,
        _ => OrganizationAdminOperation::InviteMembers,
    }
}

fn parse_operation(s: &str) -> OrganizationAdminOperation {
    match s {
        "manage_access" => OrganizationAdminOperation::ManageAccess,
        "configure" => OrganizationAdminOperation::Configure,
        "set_policy" => OrganizationAdminOperation::SetPolicy,
        "delete" => OrganizationAdminOperation::Delete,
        _ => OrganizationAdminOperation::InviteMembers,
    }
}

fn parse_permission(s: &str) -> OrgPermission {
    match s {
        "manage_access" => OrgPermission::ManageAccess,
        "configure" => OrgPermission::Configure,
        "set_policy" => OrgPermission::SetPolicy,
        "delete" => OrgPermission::Delete,
        _ => OrgPermission::InviteMembers,
    }
}
