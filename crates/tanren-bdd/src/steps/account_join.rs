//! B-0045 step definitions — existing-account join-organization.
//!
//! Steps dispatch through the per-interface
//! [`AccountHarness`](tanren_testkit::AccountHarness) trait. The
//! `xtask check-bdd-wire-coverage` guard rejects any step that bypasses
//! this seam.

use chrono::{Duration as ChronoDuration, Utc};
use cucumber::{given, then, when};
use tanren_contract::JoinOrganizationRequest;
use tanren_identity_policy::{Email, Identifier, InvitationToken, OrgId, OrgPermissions};
use tanren_testkit::{HarnessInvitation, HarnessOutcome, record_failure};

use crate::TanrenWorld;

#[given(expr = "a pending invitation for {string} with token {string}")]
async fn given_addressed_pending_invitation(world: &mut TanrenWorld, email: String, token: String) {
    let ctx = world.ensure_account_ctx().await;
    let fixture = build_invitation_fixture(&token, Some(&email), false, false, None);
    ctx.harness
        .seed_invitation(fixture)
        .await
        .expect("seed addressed invitation");
    ctx.invitations.insert(token);
}

#[given(expr = "an expired invitation for {string} with token {string}")]
async fn given_addressed_expired_invitation(world: &mut TanrenWorld, email: String, token: String) {
    let ctx = world.ensure_account_ctx().await;
    let fixture = build_invitation_fixture(&token, Some(&email), true, false, None);
    ctx.harness
        .seed_invitation(fixture)
        .await
        .expect("seed addressed expired invitation");
    ctx.invitations.insert(token);
}

#[given(expr = "a pending invitation for {string} with token {string} and {string} permissions")]
async fn given_addressed_pending_invitation_with_perms(
    world: &mut TanrenWorld,
    email: String,
    token: String,
    permissions: String,
) {
    let ctx = world.ensure_account_ctx().await;
    let perms = OrgPermissions::parse(&permissions).expect("scenario permissions must parse");
    let fixture = build_invitation_fixture(&token, Some(&email), false, false, Some(perms));
    ctx.harness
        .seed_invitation(fixture)
        .await
        .expect("seed addressed invitation with permissions");
    ctx.invitations.insert(token);
}

#[given(expr = "a revoked invitation for {string} with token {string}")]
async fn given_revoked_addressed_invitation(world: &mut TanrenWorld, email: String, token: String) {
    let ctx = world.ensure_account_ctx().await;
    let fixture = build_invitation_fixture(&token, Some(&email), false, true, None);
    ctx.harness
        .seed_invitation(fixture)
        .await
        .expect("seed revoked addressed invitation");
    ctx.invitations.insert(token);
}

#[given(expr = "{word} is already a member of organization {string}")]
async fn given_already_member(world: &mut TanrenWorld, actor: String, org_label: String) {
    let _ = org_label;
    let ctx = world.ensure_account_ctx().await;
    let entry = ctx
        .actors
        .get(&actor)
        .expect("actor must have signed up first");
    let account_id = entry
        .sign_up
        .as_ref()
        .map(|s| s.account_id)
        .or_else(|| entry.sign_in.as_ref().map(|s| s.account_id))
        .expect("actor must have a session");
    let org_id = OrgId::fresh();
    ctx.harness
        .seed_membership(account_id, org_id)
        .await
        .expect("seed membership");
}

#[when(expr = "{word} joins organization with invitation {string}")]
async fn when_join_organization(world: &mut TanrenWorld, actor: String, token: String) {
    let ctx = world.ensure_account_ctx().await;
    let invitation_token =
        InvitationToken::parse(&token).expect("scenario invitation tokens must parse");
    let entry = ctx
        .actors
        .get(&actor)
        .expect("actor must have signed up first");
    let account_id = entry
        .sign_up
        .as_ref()
        .map(|s| s.account_id)
        .or_else(|| entry.sign_in.as_ref().map(|s| s.account_id))
        .expect("actor must have a session");
    let result = ctx
        .harness
        .join_organization(account_id, JoinOrganizationRequest { invitation_token })
        .await;
    let entry = ctx.actors.entry(actor).or_default();
    let outcome = match result {
        Ok(join_result) => {
            entry.join_organization = Some(join_result.clone());
            HarnessOutcome::JoinedOrganization(join_result)
        }
        Err(err) => record_failure(err, entry),
    };
    ctx.last_outcome = Some(outcome);
}

#[then(expr = "{word} is a member of the inviting organization")]
async fn then_member_of_inviting_org(world: &mut TanrenWorld, actor: String) {
    let ctx = world.ensure_account_ctx().await;
    let entry = ctx
        .actors
        .get(&actor)
        .expect("actor must have an outcome recorded");
    let join_result = entry
        .join_organization
        .as_ref()
        .expect("actor must have joined an organization");
    let selectable = &join_result.selectable_organizations;
    let found = selectable
        .iter()
        .any(|m| m.org_id == join_result.joined_org);
    assert!(
        found,
        "expected joined org {} in selectable organizations: {:?}",
        join_result.joined_org, selectable
    );
}

#[then(expr = "{word} has been granted {string} organization permissions")]
async fn then_granted_org_permissions(world: &mut TanrenWorld, actor: String, permissions: String) {
    let ctx = world.ensure_account_ctx().await;
    let entry = ctx
        .actors
        .get(&actor)
        .expect("actor must have an outcome recorded");
    let join_result = entry
        .join_organization
        .as_ref()
        .expect("actor must have joined an organization");
    let expected = OrgPermissions::parse(&permissions).expect("scenario permissions must parse");
    assert_eq!(
        join_result.membership_permissions, expected,
        "expected organization permissions '{expected}', got '{}'",
        join_result.membership_permissions
    );
}

#[then(expr = "{word} has no project access grants")]
async fn then_no_project_access(world: &mut TanrenWorld, actor: String) {
    let ctx = world.ensure_account_ctx().await;
    let entry = ctx
        .actors
        .get(&actor)
        .expect("actor must have an outcome recorded");
    let join_result = entry
        .join_organization
        .as_ref()
        .expect("actor must have joined an organization");
    assert!(
        join_result.project_access_grants.is_empty(),
        "expected no project access grants after joining"
    );
}

#[then(expr = "{word} is a member of {int} organizations")]
async fn then_member_of_n_orgs(world: &mut TanrenWorld, actor: String, count: usize) {
    let ctx = world.ensure_account_ctx().await;
    let entry = ctx
        .actors
        .get(&actor)
        .expect("actor must have an outcome recorded");
    let join_result = entry
        .join_organization
        .as_ref()
        .expect("actor must have joined an organization");
    assert_eq!(
        join_result.selectable_organizations.len(),
        count,
        "expected {count} selectable organizations, got {actual}",
        actual = join_result.selectable_organizations.len()
    );
}

fn build_invitation_fixture(
    token: &str,
    email: Option<&str>,
    expired: bool,
    revoked: bool,
    org_permissions: Option<OrgPermissions>,
) -> HarnessInvitation {
    let parsed = InvitationToken::parse(token).expect("scenario invitation tokens must parse");
    let now = Utc::now();
    let expires_at = if expired {
        now - ChronoDuration::seconds(1)
    } else {
        now + ChronoDuration::days(1)
    };
    let target_identifier = email
        .map(|e| Identifier::from_email(&Email::parse(e).expect("scenario emails must parse")));
    HarnessInvitation {
        token: parsed,
        inviting_org: OrgId::fresh(),
        expires_at,
        target_identifier,
        org_permissions,
        revoked,
    }
}
