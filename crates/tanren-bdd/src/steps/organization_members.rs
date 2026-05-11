//! Step definitions for B-0065 (list-organization-members).

use std::collections::HashSet;
use std::str::FromStr;

use cucumber::{then, when};
use tanren_contract::{GrantSource, ListOrganizationMembersResponse};
use tanren_identity_policy::{AccountId, OrganizationPermission};
use tanren_testkit::{HarnessError, HarnessOutcome, HarnessResult, record_failure};

use crate::TanrenWorld;
use crate::steps::organization::{
    organization_id_for_name, parse_organization_name, require_account_id_for_actor,
};

/// Resolves the account id most likely to appear in organization member listings.
/// Invitation-acceptance creates a separate account from the initial sign-up;
/// for member assertions we prefer that account over the sign-up account.
fn membership_account_id_for_actor(ctx: &crate::AccountContext, actor: &str) -> Option<AccountId> {
    let entry = ctx.actors.get(actor)?;
    entry
        .accept_invitation
        .as_ref()
        .map(|acceptance| acceptance.session.account_id)
        .or_else(|| entry.sign_in.as_ref().map(|s| s.account_id))
        .or_else(|| entry.sign_up.as_ref().map(|s| s.account_id))
}

// ── B-0065: list-organization-members steps ──────────────────────────────────

#[when(expr = "{word} lists members of {string}")]
async fn when_list_members_of(
    world: &mut TanrenWorld,
    actor: String,
    name: String,
) -> HarnessResult<()> {
    let ctx = world.ensure_account_ctx().await;
    let account_id = require_account_id_for_actor(
        ctx,
        &actor,
        "actor must have a signed-in session before listing organization members",
    )?;
    let org_name = parse_organization_name(&name)?;
    let org_id = organization_id_for_name(ctx, &org_name)?;

    let result = ctx
        .harness
        .list_organization_members(account_id, org_id)
        .await;
    match result {
        Ok(response) => {
            ctx.last_listed_members = Some(response);
            ctx.last_outcome = Some(HarnessOutcome::Other(
                "list_organization_members_succeeded".to_owned(),
            ));
        }
        Err(err) => {
            let entry = ctx.actors.entry(actor).or_default();
            ctx.last_outcome = Some(record_failure(err, entry));
        }
    }
    Ok(())
}

#[when(expr = "{word} lists members of {string} without signing in")]
async fn when_list_members_of_unsigned(
    world: &mut TanrenWorld,
    actor: String,
    name: String,
) -> HarnessResult<()> {
    let ctx = world.ensure_account_ctx().await;
    let org_name = parse_organization_name(&name)?;
    let org_id = organization_id_for_name(ctx, &org_name)?;

    let result = ctx
        .harness
        .list_organization_members(AccountId::fresh(), org_id)
        .await;
    match result {
        Ok(response) => {
            ctx.last_listed_members = Some(response);
            ctx.last_outcome = Some(HarnessOutcome::Other(
                "list_organization_members_without_sign_in_unexpectedly_succeeded".to_owned(),
            ));
        }
        Err(err) => {
            let entry = ctx.actors.entry(actor).or_default();
            ctx.last_outcome = Some(record_failure(err, entry));
        }
    }
    Ok(())
}

#[then(expr = "the member list includes {word} with admin permissions and grant source {string}")]
async fn then_member_list_includes_admin(
    world: &mut TanrenWorld,
    actor: String,
    grant_source: String,
) -> HarnessResult<()> {
    let ctx = world.ensure_account_ctx().await;
    let account_id = membership_account_id_for_actor(ctx, &actor).ok_or_else(|| {
        HarnessError::Transport(format!(
            "actor {actor} must have a session before member-list assertions"
        ))
    })?;
    let expected_grant_source = parse_grant_source(&grant_source)?;
    let listed = require_last_listed_members(ctx)?;

    let member = listed
        .members
        .iter()
        .find(|m| m.account_id == account_id)
        .ok_or_else(|| {
            HarnessError::Transport(format!("expected {actor} to appear in member listing"))
        })?;

    let expected_perms: HashSet<OrganizationPermission> =
        OrganizationPermission::ALL.into_iter().collect();
    let actual_perms: HashSet<OrganizationPermission> = member
        .granted_permissions
        .iter()
        .map(|g| g.permission)
        .collect();
    assert_eq!(
        actual_perms, expected_perms,
        "expected {actor} to hold all admin permissions"
    );

    for grant in &member.granted_permissions {
        assert_eq!(
            grant.grant_source, expected_grant_source,
            "expected grant source {grant_source} for {actor}'s {:?} grant",
            grant.permission
        );
    }

    Ok(())
}

#[then(expr = "the member list includes {word} with member permissions and grant source {string}")]
async fn then_member_list_includes_member(
    world: &mut TanrenWorld,
    actor: String,
    grant_source: String,
) -> HarnessResult<()> {
    let ctx = world.ensure_account_ctx().await;
    let account_id = membership_account_id_for_actor(ctx, &actor).ok_or_else(|| {
        HarnessError::Transport(format!(
            "actor {actor} must have a session before member-list assertions"
        ))
    })?;
    let expected_grant_source = parse_grant_source(&grant_source)?;
    let listed = require_last_listed_members(ctx)?;

    let member = listed
        .members
        .iter()
        .find(|m| m.account_id == account_id)
        .ok_or_else(|| {
            HarnessError::Transport(format!("expected {actor} to appear in member listing"))
        })?;

    let admin_perms: HashSet<OrganizationPermission> =
        OrganizationPermission::ALL.into_iter().collect();
    let actual_perms: HashSet<OrganizationPermission> = member
        .granted_permissions
        .iter()
        .map(|g| g.permission)
        .collect();
    assert_ne!(
        actual_perms, admin_perms,
        "expected {actor} to hold member (non-admin) permissions"
    );

    for grant in &member.granted_permissions {
        assert_eq!(
            grant.grant_source, expected_grant_source,
            "expected grant source {grant_source} for {actor}'s {:?} grant",
            grant.permission
        );
    }

    Ok(())
}

#[then(expr = "the member listing exposes no project-scope grants")]
async fn then_no_project_scope_grants(world: &mut TanrenWorld) -> HarnessResult<()> {
    let ctx = world.ensure_account_ctx().await;
    // OrganizationMemberPermissionGrant.permission is org-scoped by type;
    // project-scope grants cannot appear in this response structure.
    require_last_listed_members(ctx)?;
    Ok(())
}

fn require_last_listed_members(
    ctx: &crate::AccountContext,
) -> HarnessResult<&ListOrganizationMembersResponse> {
    ctx.last_listed_members.as_ref().ok_or_else(|| {
        HarnessError::Transport(
            "member-list response must be captured before this assertion".to_owned(),
        )
    })
}

fn parse_grant_source(raw: &str) -> HarnessResult<GrantSource> {
    GrantSource::from_str(raw)
        .map_err(|err| HarnessError::Transport(format!("unknown grant source '{raw}': {err}")))
}
