//! Organization-flow step definitions for B-0066.
//!
//! Like account steps, these dispatch exclusively through
//! [`tanren_testkit::AccountHarness`] so each interface tag proves the
//! real surface under test.

use std::collections::HashSet;

use cucumber::{then, when};
use tanren_identity_policy::{AccountId, OrganizationName, OrganizationPermission};
use tanren_testkit::{HarnessOutcome, record_failure};

use crate::TanrenWorld;

const ALL_ADMIN_PERMISSIONS: [OrganizationPermission; 5] = [
    OrganizationPermission::Invite,
    OrganizationPermission::ManageAccess,
    OrganizationPermission::Configure,
    OrganizationPermission::SetPolicy,
    OrganizationPermission::Delete,
];

#[when(expr = "{word} creates organization {string}")]
async fn when_create_organization(world: &mut TanrenWorld, actor: String, name: String) {
    let ctx = world.ensure_account_ctx().await;
    let account_id = account_id_for_actor(ctx, &actor)
        .expect("actor must have a signed-in session before creating an organization");
    let org_name = OrganizationName::parse(&name).expect("scenario organization names must parse");

    let result = ctx.harness.create_organization(account_id, org_name).await;
    match result {
        Ok(response) => {
            ctx.organizations_by_name.insert(
                response.organization.name.as_str().to_owned(),
                response.organization.id,
            );
            ctx.last_created_organization = Some(response);
            ctx.last_listed_organizations = None;
            ctx.last_checked_organization_permission = None;
            ctx.last_outcome = Some(HarnessOutcome::Other(
                "create_organization_succeeded".to_owned(),
            ));
        }
        Err(err) => {
            let entry = ctx.actors.entry(actor).or_default();
            ctx.last_outcome = Some(record_failure(err, entry));
        }
    }
}

#[when(expr = "{word} creates organization {string} without signing in")]
async fn when_create_organization_unsigned(world: &mut TanrenWorld, actor: String, name: String) {
    let ctx = world.ensure_account_ctx().await;
    let org_name = OrganizationName::parse(&name).expect("scenario organization names must parse");

    // Unsigned calls intentionally use a fresh account id with no session.
    let result = ctx
        .harness
        .create_organization(AccountId::fresh(), org_name)
        .await;

    match result {
        Ok(response) => {
            ctx.organizations_by_name.insert(
                response.organization.name.as_str().to_owned(),
                response.organization.id,
            );
            ctx.last_created_organization = Some(response);
            ctx.last_listed_organizations = None;
            ctx.last_checked_organization_permission = None;
            ctx.last_outcome = Some(HarnessOutcome::Other(
                "create_organization_without_sign_in_unexpectedly_succeeded".to_owned(),
            ));
        }
        Err(err) => {
            let entry = ctx.actors.entry(actor).or_default();
            ctx.last_outcome = Some(record_failure(err, entry));
        }
    }
}

#[when(expr = "{word} lists available organizations")]
async fn when_list_organizations(world: &mut TanrenWorld, actor: String) {
    let ctx = world.ensure_account_ctx().await;
    let account_id = account_id_for_actor(ctx, &actor)
        .expect("actor must have a signed-in session before listing organizations");

    let result = ctx.harness.list_organizations(account_id).await;
    match result {
        Ok(response) => {
            for org in &response.organizations {
                ctx.organizations_by_name
                    .insert(org.name.as_str().to_owned(), org.id);
            }
            ctx.last_listed_organizations = Some(response);
            ctx.last_checked_organization_permission = None;
            ctx.last_outcome = Some(HarnessOutcome::Other(
                "list_organizations_succeeded".to_owned(),
            ));
        }
        Err(err) => {
            let entry = ctx.actors.entry(actor).or_default();
            ctx.last_outcome = Some(record_failure(err, entry));
        }
    }
}

#[when(expr = "{word} checks organization permission {string} in {string}")]
async fn when_check_org_permission(
    world: &mut TanrenWorld,
    actor: String,
    permission: String,
    name: String,
) {
    let ctx = world.ensure_account_ctx().await;
    let account_id = account_id_for_actor(ctx, &actor)
        .expect("actor must have a signed-in session before checking permissions");
    let org_name = OrganizationName::parse(&name).expect("scenario organization names must parse");
    let org_id = *ctx
        .organizations_by_name
        .get(org_name.as_str())
        .expect("organization must have been created/listed earlier in the scenario");
    let permission = parse_permission(&permission);

    let result = ctx
        .harness
        .check_organization_admin_permission(account_id, org_id, permission)
        .await;
    match result {
        Ok(response) => {
            ctx.last_checked_organization_permission = Some(response);
            ctx.last_outcome = Some(HarnessOutcome::Other(
                "check_organization_permission_succeeded".to_owned(),
            ));
        }
        Err(err) => {
            let entry = ctx.actors.entry(actor).or_default();
            ctx.last_outcome = Some(record_failure(err, entry));
        }
    }
}

#[then(expr = "the operation succeeds")]
async fn then_operation_succeeds(world: &mut TanrenWorld) {
    let ctx = world.ensure_account_ctx().await;
    let ok = matches!(
        &ctx.last_outcome,
        Some(HarnessOutcome::Other(marker))
            if marker == "create_organization_succeeded"
                || marker == "list_organizations_succeeded"
                || marker == "check_organization_permission_succeeded"
    );
    assert!(ok, "expected operation success, got {:?}", ctx.last_outcome);
}

#[then(expr = "organization {string} is listed for {word}")]
async fn then_org_is_listed(world: &mut TanrenWorld, name: String, actor: String) {
    let ctx = world.ensure_account_ctx().await;
    let org_name = OrganizationName::parse(&name).expect("scenario organization names must parse");
    let listed = ctx
        .last_listed_organizations
        .as_ref()
        .expect("organization-list response must be captured before this assertion");

    let found = listed
        .organizations
        .iter()
        .any(|org| org.name.as_str() == org_name.as_str());
    assert!(
        found,
        "expected organization {org_name} to be listed for {actor}"
    );
}

#[then(expr = "{word} holds all organization admin permissions in {string}")]
async fn then_creator_has_admin_permissions(world: &mut TanrenWorld, actor: String, name: String) {
    let ctx = world.ensure_account_ctx().await;
    let org_name = OrganizationName::parse(&name).expect("scenario organization names must parse");

    let created = ctx
        .last_created_organization
        .as_ref()
        .expect("create-organization response must be captured before this assertion");
    assert_eq!(
        created.organization.name.as_str(),
        org_name.as_str(),
        "admin-permission assertion for {actor} must target the just-created organization"
    );

    let expected: HashSet<OrganizationPermission> = ALL_ADMIN_PERMISSIONS.into_iter().collect();
    let actual: HashSet<OrganizationPermission> =
        created.granted_permissions.iter().copied().collect();
    assert_eq!(
        actual, expected,
        "creator should receive the full admin permission set for the new organization"
    );
}

#[then(expr = "organization {string} has zero initial projects")]
async fn then_org_has_zero_initial_projects(world: &mut TanrenWorld, name: String) {
    let ctx = world.ensure_account_ctx().await;
    let org_name = OrganizationName::parse(&name).expect("scenario organization names must parse");

    let events = ctx
        .harness
        .recent_events(50)
        .await
        .expect("recent_events should succeed under BDD");
    let payload = events
        .iter()
        .rev()
        .find_map(|event| {
            let kind = event
                .payload
                .get("kind")
                .and_then(serde_json::Value::as_str)?;
            if kind != "organization_created" {
                return None;
            }
            let payload = event.payload.get("payload")?;
            let event_name = payload.get("name").and_then(serde_json::Value::as_str)?;
            if event_name == org_name.as_str() {
                Some(payload)
            } else {
                None
            }
        })
        .expect("expected an organization_created event for the organization");

    let initial_count = payload
        .get("initial_project_count")
        .and_then(serde_json::Value::as_u64)
        .expect("organization_created payload must include numeric initial_project_count");
    assert_eq!(
        initial_count, 0,
        "new organizations must start with zero projects"
    );
}

fn parse_permission(raw: &str) -> OrganizationPermission {
    let parsed = match raw {
        "invite" => Some(OrganizationPermission::Invite),
        "manage_access" => Some(OrganizationPermission::ManageAccess),
        "configure" => Some(OrganizationPermission::Configure),
        "set_policy" => Some(OrganizationPermission::SetPolicy),
        "delete" => Some(OrganizationPermission::Delete),
        _ => None,
    };
    parsed.expect("unsupported organization permission in scenario")
}

fn account_id_for_actor(ctx: &crate::AccountContext, actor: &str) -> Option<AccountId> {
    let entry = ctx.actors.get(actor)?;
    entry
        .sign_in
        .as_ref()
        .map(|session| session.account_id)
        .or_else(|| entry.sign_up.as_ref().map(|session| session.account_id))
        .or_else(|| {
            entry
                .accept_invitation
                .as_ref()
                .map(|acceptance| acceptance.session.account_id)
        })
}
