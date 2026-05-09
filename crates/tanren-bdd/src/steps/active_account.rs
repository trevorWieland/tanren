//! Active-account switch step definitions for B-0046.

use chrono::{Duration as ChronoDuration, Utc};
use cucumber::{given, then, when};
use secrecy::SecretString;
use tanren_contract::{AcceptInvitationRequest, SignUpRequest};
use tanren_identity_policy::{AccountId, Email, InvitationToken, OrgId};
use tanren_testkit::{HarnessInvitation, HarnessKind, HarnessOutcome, record_failure};

use crate::TanrenWorld;

#[given(expr = "{word} holds two signed-in accounts via the {word}")]
async fn given_two_signed_in_accounts(world: &mut TanrenWorld, actor: String, surface: String) {
    let expected_kind = parse_surface_kind(&surface);
    let ctx = world.ensure_account_ctx().await;
    assert_eq!(
        ctx.harness.kind(),
        expected_kind,
        "harness/surface mismatch"
    );

    let first_email = format!("{actor}-{surface}-a@example.com");
    let first_request = SignUpRequest {
        email: Email::parse(&first_email).expect("scenario email must parse"),
        password: SecretString::from("switch-pw-1".to_owned()),
        display_name: format!("{actor} first"),
    };
    let first = ctx.harness.sign_up(first_request).await;
    let entry = ctx.actors.entry(actor.clone()).or_default();
    match first {
        Ok(session) => {
            entry.sign_up = Some(session.clone());
            entry.identifier = Some(first_email);
            entry.password = Some(SecretString::from("switch-pw-1".to_owned()));
            entry.last_failure = None;
            ctx.last_outcome = Some(HarnessOutcome::SignedUp(session));
        }
        Err(err) => {
            ctx.last_outcome = Some(record_failure(err, entry));
            return;
        }
    }

    let token_raw = format!("{actor}-{surface}-switch-padpad");
    let invitation_token =
        InvitationToken::parse(&token_raw).expect("scenario invitation token must parse");
    let invitation = HarnessInvitation {
        token: invitation_token.clone(),
        inviting_org: OrgId::fresh(),
        expires_at: Utc::now() + ChronoDuration::days(1),
    };
    ctx.harness
        .seed_invitation(invitation)
        .await
        .expect("seed invitation for second account");

    let second_email = format!("{actor}-{surface}-b@example.com");
    let accept = ctx
        .harness
        .accept_invitation(AcceptInvitationRequest {
            invitation_token,
            email: Email::parse(&second_email).expect("scenario email must parse"),
            password: SecretString::from("switch-pw-2".to_owned()),
            display_name: format!("{actor} second"),
        })
        .await;
    let entry = ctx.actors.entry(actor).or_default();
    match accept {
        Ok(acceptance) => {
            entry.accept_invitation = Some(acceptance.clone());
            entry.last_failure = None;
            ctx.last_outcome = Some(HarnessOutcome::AcceptedInvitation(acceptance));
        }
        Err(err) => {
            ctx.last_outcome = Some(record_failure(err, entry));
        }
    }
}

#[given(expr = "{word} holds one signed-in account via the {word}")]
async fn given_one_signed_in_account(world: &mut TanrenWorld, actor: String, surface: String) {
    let expected_kind = parse_surface_kind(&surface);
    let ctx = world.ensure_account_ctx().await;
    assert_eq!(
        ctx.harness.kind(),
        expected_kind,
        "harness/surface mismatch"
    );

    let email = format!("{actor}-{surface}-single@example.com");
    let result = ctx
        .harness
        .sign_up(SignUpRequest {
            email: Email::parse(&email).expect("scenario email must parse"),
            password: SecretString::from("switch-pw-1".to_owned()),
            display_name: format!("{actor} single"),
        })
        .await;
    let entry = ctx.actors.entry(actor).or_default();
    match result {
        Ok(session) => {
            entry.sign_up = Some(session.clone());
            entry.accept_invitation = None;
            entry.last_failure = None;
            ctx.last_outcome = Some(HarnessOutcome::SignedUp(session));
        }
        Err(err) => {
            ctx.last_outcome = Some(record_failure(err, entry));
        }
    }
}

#[when(expr = "{word} switches the active account to the second account via the {word}")]
async fn when_switch_to_second(world: &mut TanrenWorld, actor: String, surface: String) {
    let expected_kind = parse_surface_kind(&surface);
    let ctx = world.ensure_account_ctx().await;
    assert_eq!(
        ctx.harness.kind(),
        expected_kind,
        "harness/surface mismatch"
    );
    let target_account_id = ctx
        .actors
        .get(&actor)
        .and_then(|entry| entry.accept_invitation.as_ref())
        .map(|accept| accept.session.account_id)
        .expect("actor must hold a second account first");

    let result = ctx.harness.switch_active_account(target_account_id).await;
    let entry = ctx.actors.entry(actor).or_default();
    match result {
        Ok(_) => {
            entry.last_failure = None;
            ctx.last_outcome = Some(HarnessOutcome::Other("active_account_switched".to_owned()));
        }
        Err(err) => {
            ctx.last_outcome = Some(record_failure(err, entry));
        }
    }
}

#[when(expr = "{word} switches the active account to an unsigned account via the {word}")]
async fn when_switch_to_unsigned(world: &mut TanrenWorld, actor: String, surface: String) {
    let expected_kind = parse_surface_kind(&surface);
    let ctx = world.ensure_account_ctx().await;
    assert_eq!(
        ctx.harness.kind(),
        expected_kind,
        "harness/surface mismatch"
    );

    let result = ctx.harness.switch_active_account(AccountId::fresh()).await;
    let entry = ctx.actors.entry(actor).or_default();
    match result {
        Ok(_) => {
            entry.last_failure = None;
            ctx.last_outcome = Some(HarnessOutcome::Other(
                "active_account_switch_unexpected_success".to_owned(),
            ));
        }
        Err(err) => {
            ctx.last_outcome = Some(record_failure(err, entry));
        }
    }
}

#[then(expr = "{word} sees the second account as active via the {word}")]
async fn then_second_account_active(world: &mut TanrenWorld, actor: String, surface: String) {
    let expected_kind = parse_surface_kind(&surface);
    let ctx = world.ensure_account_ctx().await;
    assert_eq!(
        ctx.harness.kind(),
        expected_kind,
        "harness/surface mismatch"
    );

    let first_account_id = ctx
        .actors
        .get(&actor)
        .and_then(|entry| entry.sign_up.as_ref())
        .map(|session| session.account_id)
        .expect("first account must exist");
    let second_account_id = ctx
        .actors
        .get(&actor)
        .and_then(|entry| entry.accept_invitation.as_ref())
        .map(|accept| accept.session.account_id)
        .expect("second account must exist");

    let accounts = ctx
        .harness
        .list_active_accounts()
        .await
        .expect("list active accounts must succeed");
    assert_eq!(accounts.len(), 2, "expected two signed-in accounts");

    let active = accounts
        .iter()
        .filter(|entry| entry.is_active)
        .collect::<Vec<_>>();
    assert_eq!(active.len(), 1, "expected exactly one active account");
    assert_eq!(
        active[0].account.id, second_account_id,
        "wrong active account"
    );
    assert!(
        accounts
            .iter()
            .any(|entry| entry.account.id == first_account_id),
        "first account missing from signed-in list"
    );
}

fn parse_surface_kind(surface: &str) -> HarnessKind {
    match surface {
        "api" => HarnessKind::Api,
        "web" => HarnessKind::Web,
        "cli" => HarnessKind::Cli,
        "mcp" => HarnessKind::Mcp,
        "tui" => HarnessKind::Tui,
        _ => HarnessKind::InProcess,
    }
}
