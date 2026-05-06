//! Organization invitation step definitions for R-0005.
//!
//! Step bodies dispatch through the per-interface
//! [`AccountHarness`](tanren_testkit::AccountHarness) trait — never
//! `tanren_app_services::Handlers::*` directly. The active harness is
//! selected by the BDD `Before` hook from the scenario's tags
//! (`@api`, `@cli`, `@mcp`, `@tui`, `@web`, or fallback in-process).

use chrono::{Duration as ChronoDuration, Utc};
use cucumber::{given, then, when};
use tanren_contract::{CreateOrgInvitationRequest, InvitationStatus};
use tanren_identity_policy::{
    AccountId, Email, Identifier, InvitationToken, OrgId, OrganizationPermission,
};
use tanren_testkit::{
    HarnessMembershipSeed, HarnessOrgInvitationSeed, HarnessOutcome, record_failure,
};
use uuid::Uuid;

use crate::TanrenWorld;

fn parse_org_id(raw: &str) -> OrgId {
    OrgId::new(Uuid::parse_str(raw).expect("scenario org IDs must be valid UUIDs"))
}

fn parse_permissions(csv: &str) -> Vec<OrganizationPermission> {
    csv.split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| OrganizationPermission::parse(s).expect("scenario permissions must parse"))
        .collect()
}

fn parse_identifier(raw: &str) -> Identifier {
    if raw.contains('@') {
        let email = Email::parse(raw).expect("scenario emails must parse");
        Identifier::from_email(&email)
    } else {
        Identifier::parse(raw).expect("scenario identifiers must parse")
    }
}

fn actor_account_id(ctx: &crate::AccountContext, actor: &str) -> AccountId {
    let entry = ctx
        .actors
        .get(actor)
        .expect("actor must have signed up or signed in first");
    entry
        .sign_up
        .as_ref()
        .map(|s| s.account_id)
        .or_else(|| entry.sign_in.as_ref().map(|s| s.account_id))
        .expect("actor must have a successful session on file")
}

fn actor_org_id(ctx: &crate::AccountContext, actor: &str) -> Option<OrgId> {
    let entry = ctx
        .actors
        .get(actor)
        .expect("actor must have signed up first");
    entry
        .sign_up
        .as_ref()
        .or(entry.sign_in.as_ref())
        .and_then(|s| s.account.org)
}

fn parse_invitation_status(raw: &str) -> InvitationStatus {
    serde_json::from_value(serde_json::Value::String(raw.to_owned()))
        .expect("scenario status must be pending, revoked, or accepted")
}

#[given(expr = "{word} is an org admin of {word}")]
async fn given_org_admin(world: &mut TanrenWorld, actor: String, org_id_raw: String) {
    let ctx = world.ensure_account_ctx().await;
    let account_id = actor_account_id(ctx, &actor);
    let org_id = parse_org_id(&org_id_raw);
    ctx.harness
        .seed_membership(HarnessMembershipSeed {
            account_id,
            org_id,
            permissions: vec![
                OrganizationPermission::parse("admin").expect("admin permission must parse"),
            ],
        })
        .await
        .expect("seed admin membership");
}

#[given(expr = "{word} is a non-admin member of org {word}")]
async fn given_non_admin_member(world: &mut TanrenWorld, actor: String, org_id_raw: String) {
    let ctx = world.ensure_account_ctx().await;
    let account_id = actor_account_id(ctx, &actor);
    let org_id = parse_org_id(&org_id_raw);
    ctx.harness
        .seed_membership(HarnessMembershipSeed {
            account_id,
            org_id,
            permissions: vec![
                OrganizationPermission::parse("member").expect("member permission must parse"),
            ],
        })
        .await
        .expect("seed non-admin membership");
}

#[given(expr = "{word} has no org membership")]
async fn given_no_org_membership(world: &mut TanrenWorld, actor: String) {
    drop(actor);
    let _ = world.ensure_account_ctx().await;
}

#[given(
    expr = "a pending org invitation in org {word} to {string} with permissions {string} created by {word}"
)]
async fn given_pending_org_invitation(
    world: &mut TanrenWorld,
    org_id_raw: String,
    recipient_raw: String,
    permissions_csv: String,
    creator: String,
) {
    let ctx = world.ensure_account_ctx().await;
    let org_id = parse_org_id(&org_id_raw);
    let created_by = actor_account_id(ctx, &creator);
    let recipient = parse_identifier(&recipient_raw);
    let permissions = parse_permissions(&permissions_csv);
    let token = tanren_testkit::fresh_invitation_token();
    ctx.harness
        .seed_org_invitation(HarnessOrgInvitationSeed {
            token: token.clone(),
            org_id,
            recipient_identifier: recipient,
            permissions,
            created_by,
            expires_at: Utc::now() + ChronoDuration::days(30),
        })
        .await
        .expect("seed org invitation");
    ctx.invitations.insert(token.as_str().to_owned());
}

#[when(
    expr = "{word} creates an org invitation in org {word} to {string} with permissions {string}"
)]
async fn when_create_invitation(
    world: &mut TanrenWorld,
    actor: String,
    org_id_raw: String,
    recipient_raw: String,
    permissions_csv: String,
) {
    let ctx = world.ensure_account_ctx().await;
    let caller_account_id = actor_account_id(ctx, &actor);
    let org_id = parse_org_id(&org_id_raw);
    let caller_org_context = actor_org_id(ctx, &actor);
    let recipient = parse_identifier(&recipient_raw);
    let permissions = parse_permissions(&permissions_csv);
    let request = CreateOrgInvitationRequest {
        org_id,
        recipient_identifier: recipient,
        permissions,
        expires_at: Utc::now() + ChronoDuration::days(30),
    };
    let result = ctx
        .harness
        .create_org_invitation(caller_account_id, caller_org_context, request)
        .await;
    let outcome = match result {
        Ok(view) => {
            ctx.last_invitation_view = Some(view.clone());
            ctx.invitations.insert(view.token.as_str().to_owned());
            HarnessOutcome::InvitationCreated(view)
        }
        Err(err) => {
            let entry = ctx.actors.entry(actor).or_default();
            record_failure(err, entry)
        }
    };
    ctx.last_outcome = Some(outcome);
}

#[when(
    expr = "{word} creates an org invitation in org {word} from personal context to {string} with permissions {string}"
)]
async fn when_create_invitation_personal_context(
    world: &mut TanrenWorld,
    actor: String,
    org_id_raw: String,
    recipient_raw: String,
    permissions_csv: String,
) {
    let ctx = world.ensure_account_ctx().await;
    let caller_account_id = actor_account_id(ctx, &actor);
    let org_id = parse_org_id(&org_id_raw);
    let recipient = parse_identifier(&recipient_raw);
    let permissions = parse_permissions(&permissions_csv);
    let request = CreateOrgInvitationRequest {
        org_id,
        recipient_identifier: recipient,
        permissions,
        expires_at: Utc::now() + ChronoDuration::days(30),
    };
    let result = ctx
        .harness
        .create_org_invitation(caller_account_id, None, request)
        .await;
    let outcome = match result {
        Ok(view) => {
            ctx.last_invitation_view = Some(view.clone());
            HarnessOutcome::InvitationCreated(view)
        }
        Err(err) => {
            let entry = ctx.actors.entry(actor).or_default();
            record_failure(err, entry)
        }
    };
    ctx.last_outcome = Some(outcome);
}

#[when(expr = "{word} lists org invitations for {word}")]
async fn when_list_org_invitations(world: &mut TanrenWorld, actor: String, org_id_raw: String) {
    let ctx = world.ensure_account_ctx().await;
    let caller_account_id = actor_account_id(ctx, &actor);
    let org_id = parse_org_id(&org_id_raw);
    let result = ctx
        .harness
        .list_org_invitations(caller_account_id, org_id)
        .await;
    let outcome = match result {
        Ok(invitations) => {
            ctx.last_invitation_list.clone_from(&invitations);
            HarnessOutcome::InvitationsListed(invitations)
        }
        Err(err) => {
            let entry = ctx.actors.entry(actor).or_default();
            record_failure(err, entry)
        }
    };
    ctx.last_outcome = Some(outcome);
}

#[when(expr = "the recipient {string} lists their invitations")]
async fn when_list_recipient_invitations(world: &mut TanrenWorld, identifier_raw: String) {
    let ctx = world.ensure_account_ctx().await;
    let identifier = parse_identifier(&identifier_raw);
    let result = ctx.harness.list_recipient_invitations(&identifier).await;
    let outcome = match result {
        Ok(invitations) => {
            ctx.last_invitation_list.clone_from(&invitations);
            HarnessOutcome::InvitationsListed(invitations)
        }
        Err(err) => {
            ctx.last_outcome = Some(HarnessOutcome::Other(format!(
                "recipient list error: {err}"
            )));
            return;
        }
    };
    ctx.last_outcome = Some(outcome);
}

#[when(expr = "{word} revokes the invitation for {string} in org {word}")]
async fn when_revoke_invitation_by_recipient(
    world: &mut TanrenWorld,
    actor: String,
    recipient_raw: String,
    org_id_raw: String,
) {
    let ctx = world.ensure_account_ctx().await;
    let caller_account_id = actor_account_id(ctx, &actor);
    let org_id = parse_org_id(&org_id_raw);
    let caller_org_context = actor_org_id(ctx, &actor);
    let matching = ctx
        .last_invitation_list
        .iter()
        .find(|inv| inv.recipient_identifier.as_str() == recipient_raw && inv.org_id == org_id)
        .or_else(|| {
            ctx.last_invitation_view.as_ref().and_then(|view| {
                if view.recipient_identifier.as_str() == recipient_raw && view.org_id == org_id {
                    Some(view)
                } else {
                    None
                }
            })
        });
    let token = if let Some(inv) = matching {
        inv.token.clone()
    } else {
        let entry = ctx.actors.entry(actor).or_default();
        entry.last_failure = Some(tanren_contract::AccountFailureReason::InvitationNotFound);
        ctx.last_outcome = Some(HarnessOutcome::Failure(
            tanren_contract::AccountFailureReason::InvitationNotFound,
        ));
        return;
    };
    let result = ctx
        .harness
        .revoke_invitation(caller_account_id, caller_org_context, org_id, token)
        .await;
    let outcome = match result {
        Ok(view) => {
            ctx.last_invitation_view = Some(view.clone());
            HarnessOutcome::InvitationRevoked(view)
        }
        Err(err) => {
            let entry = ctx.actors.entry(actor).or_default();
            record_failure(err, entry)
        }
    };
    ctx.last_outcome = Some(outcome);
}

#[when(expr = "{word} revokes invitation {string} in org {word}")]
async fn when_revoke_invitation_by_token(
    world: &mut TanrenWorld,
    actor: String,
    token_raw: String,
    org_id_raw: String,
) {
    let ctx = world.ensure_account_ctx().await;
    let caller_account_id = actor_account_id(ctx, &actor);
    let org_id = parse_org_id(&org_id_raw);
    let caller_org_context = actor_org_id(ctx, &actor);
    let token = InvitationToken::parse(&token_raw).expect("scenario invitation tokens must parse");
    let result = ctx
        .harness
        .revoke_invitation(caller_account_id, caller_org_context, org_id, token)
        .await;
    let outcome = match result {
        Ok(view) => {
            ctx.last_invitation_view = Some(view.clone());
            HarnessOutcome::InvitationRevoked(view)
        }
        Err(err) => {
            let entry = ctx.actors.entry(actor).or_default();
            record_failure(err, entry)
        }
    };
    ctx.last_outcome = Some(outcome);
}

#[then(expr = "the invitation list contains {int} invitation(s)")]
async fn then_invitation_list_count(world: &mut TanrenWorld, count: usize) {
    let ctx = world.ensure_account_ctx().await;
    let actual = ctx.last_invitation_list.len();
    assert_eq!(
        actual, count,
        "expected {count} invitations, got {actual}: {:?}",
        ctx.last_invitation_list
    );
}

#[then(
    expr = "the invitation list contains a pending invitation for {string} with permission {string}"
)]
async fn then_invitation_list_contains_pending(
    world: &mut TanrenWorld,
    recipient_raw: String,
    permission: String,
) {
    let ctx = world.ensure_account_ctx().await;
    let found = ctx.last_invitation_list.iter().any(|inv| {
        inv.recipient_identifier.as_str() == recipient_raw
            && inv.status == InvitationStatus::Pending
            && inv.permissions.iter().any(|p| p.as_str() == permission)
    });
    assert!(
        found,
        "expected pending invitation for '{recipient_raw}' with permission '{permission}', got: {:?}",
        ctx.last_invitation_list
    );
}

#[then(expr = "the invitation list contains a revoked invitation for {string}")]
async fn then_invitation_list_contains_revoked(world: &mut TanrenWorld, recipient_raw: String) {
    let ctx = world.ensure_account_ctx().await;
    let found = ctx.last_invitation_list.iter().any(|inv| {
        inv.recipient_identifier.as_str() == recipient_raw
            && inv.status == InvitationStatus::Revoked
    });
    assert!(
        found,
        "expected revoked invitation for '{recipient_raw}', got: {:?}",
        ctx.last_invitation_list
    );
}

#[then(expr = "the invitation list contains no invitations")]
async fn then_invitation_list_empty(world: &mut TanrenWorld) {
    let ctx = world.ensure_account_ctx().await;
    assert!(
        ctx.last_invitation_list.is_empty(),
        "expected no invitations, got: {:?}",
        ctx.last_invitation_list
    );
}

#[then(expr = "the created invitation has status {string}")]
async fn then_created_invitation_status(world: &mut TanrenWorld, status_raw: String) {
    let ctx = world.ensure_account_ctx().await;
    let view = ctx
        .last_invitation_view
        .as_ref()
        .expect("expected a created invitation on file");
    let expected = parse_invitation_status(&status_raw);
    assert_eq!(
        view.status, expected,
        "expected invitation status {status_raw}, got {:?}",
        view.status
    );
}

#[then(expr = "the created invitation has permission {string}")]
async fn then_created_invitation_permission(world: &mut TanrenWorld, permission: String) {
    let ctx = world.ensure_account_ctx().await;
    let view = ctx
        .last_invitation_view
        .as_ref()
        .expect("expected a created invitation on file");
    let found = view.permissions.iter().any(|p| p.as_str() == permission);
    assert!(
        found,
        "expected permission '{permission}' on invitation, got: {:?}",
        view.permissions
    );
}

#[then(expr = "the recipient sees a pending invitation from org {word}")]
async fn then_recipient_sees_pending(world: &mut TanrenWorld, org_id_raw: String) {
    let ctx = world.ensure_account_ctx().await;
    let org_id = parse_org_id(&org_id_raw);
    let found = ctx
        .last_invitation_list
        .iter()
        .any(|inv| inv.org_id == org_id && inv.status == InvitationStatus::Pending);
    assert!(
        found,
        "expected pending invitation from org {org_id_raw}, got: {:?}",
        ctx.last_invitation_list
    );
}

#[then(expr = "the revoked invitation has status {string}")]
async fn then_revoked_invitation_status(world: &mut TanrenWorld, status_raw: String) {
    then_created_invitation_status(world, status_raw).await;
}

#[then(expr = "the recipient sees no invitations")]
async fn then_recipient_no_invitations(world: &mut TanrenWorld) {
    then_invitation_list_empty(world).await;
}

#[then(expr = "{word} holds permission {string} in the joined organization")]
async fn then_holds_permission_in_joined_org(
    world: &mut TanrenWorld,
    actor: String,
    permission: String,
) {
    let ctx = world.ensure_account_ctx().await;
    let entry = ctx
        .actors
        .get(&actor)
        .expect("actor must have an outcome recorded");
    let acceptance = entry
        .accept_invitation
        .as_ref()
        .expect("actor must have accepted an invitation");
    let account_id = acceptance.session.account_id;
    let org_id = acceptance.joined_org;
    let permissions = ctx
        .harness
        .find_membership_permissions(account_id, org_id)
        .await
        .expect("find_membership_permissions must succeed under BDD");
    let found = permissions.iter().any(|p| p.as_str() == permission);
    assert!(
        found,
        "expected permission '{permission}' on accepted membership, got: {permissions:?}"
    );
}
