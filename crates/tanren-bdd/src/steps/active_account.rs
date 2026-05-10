//! Active-account switch step definitions for B-0046.

use chrono::{Duration as ChronoDuration, Utc};
use cucumber::{given, then, when};
use secrecy::SecretString;
use std::sync::atomic::{AtomicU64, Ordering};
use tanren_contract::{AcceptInvitationRequest, SignUpRequest};
use tanren_identity_policy::{AccountId, Email, InvitationToken, OrgId};
use tanren_testkit::{HarnessInvitation, HarnessKind, HarnessOutcome, record_failure};

use crate::TanrenWorld;

#[given(expr = "{word} holds two signed-in accounts via the {word}")]
async fn given_two_signed_in_accounts(world: &mut TanrenWorld, actor: String, surface: String) {
    let expected_kind = parse_surface_kind(&surface);
    let ctx = world
        .ensure_account_ctx()
        .await
        .expect("account context must initialize");
    assert_eq!(
        ctx.harness.kind(),
        expected_kind,
        "harness/surface mismatch"
    );

    let suffix = unique_suffix();
    let first_email = format!("{actor}-{surface}-a-{suffix}@example.com");
    ctx.window_accounts.clear();
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

    let token_raw = format!("{actor}-{surface}-{suffix}-switch-padpad");
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

    let second_email = format!("{actor}-{surface}-b-{suffix}@example.com");
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
    let ctx = world
        .ensure_account_ctx()
        .await
        .expect("account context must initialize");
    assert_eq!(
        ctx.harness.kind(),
        expected_kind,
        "harness/surface mismatch"
    );

    let email = format!("{actor}-{surface}-single-{}@example.com", unique_suffix());
    ctx.window_accounts.clear();
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
    let ctx = world
        .ensure_account_ctx()
        .await
        .expect("account context must initialize");
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
        Ok(accounts) => {
            entry.last_failure = None;
            ctx.window_accounts.insert("_default".to_owned(), accounts);
            ctx.last_outcome = Some(HarnessOutcome::Other("active_account_switched".to_owned()));
        }
        Err(err) => {
            ctx.last_outcome = Some(record_failure(err, entry));
        }
    }
}

#[when(expr = "{word} switches the active account back to the first account via the {word}")]
async fn when_switch_back_to_first(world: &mut TanrenWorld, actor: String, surface: String) {
    let expected_kind = parse_surface_kind(&surface);
    let ctx = world
        .ensure_account_ctx()
        .await
        .expect("account context must initialize");
    assert_eq!(
        ctx.harness.kind(),
        expected_kind,
        "harness/surface mismatch"
    );
    let target_account_id = ctx
        .actors
        .get(&actor)
        .and_then(|entry| entry.sign_up.as_ref())
        .map(|session| session.account_id)
        .expect("actor must hold a first account");

    let result = ctx.harness.switch_active_account(target_account_id).await;
    let entry = ctx.actors.entry(actor).or_default();
    match result {
        Ok(accounts) => {
            entry.last_failure = None;
            ctx.window_accounts.insert("_default".to_owned(), accounts);
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
    let ctx = world
        .ensure_account_ctx()
        .await
        .expect("account context must initialize");
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
    let ctx = world
        .ensure_account_ctx()
        .await
        .expect("account context must initialize");
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

#[then(expr = "{word} sees project availability scoped to the selected account via the {word}")]
async fn then_org_visibility_changes(world: &mut TanrenWorld, actor: String, surface: String) {
    let expected_kind = parse_surface_kind(&surface);
    let ctx = world
        .ensure_account_ctx()
        .await
        .expect("account context must initialize");
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

    let first = accounts
        .iter()
        .find(|entry| entry.account.id == first_account_id)
        .expect("first account must be visible");
    let second = accounts
        .iter()
        .find(|entry| entry.account.id == second_account_id)
        .expect("second account must be visible");
    assert!(
        second.is_active,
        "second account should own the active availability scope after switching"
    );
    assert!(
        !first.is_active,
        "first account should not stay active after switching to second"
    );
    assert!(
        first.account.org.is_none(),
        "first account should be personal/no-org"
    );
    assert!(
        second.account.org.is_some(),
        "second account should be org-backed"
    );
}

#[then(expr = "{word} sees the first account as active without re-authentication via the {word}")]
async fn then_first_active_without_reauth(world: &mut TanrenWorld, actor: String, surface: String) {
    let expected_kind = parse_surface_kind(&surface);
    let ctx = world
        .ensure_account_ctx()
        .await
        .expect("account context must initialize");
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
    assert!(
        accounts
            .iter()
            .any(|entry| entry.account.id == first_account_id && entry.is_active),
        "first account should be active after switching back"
    );
    assert!(
        accounts
            .iter()
            .any(|entry| entry.account.id == second_account_id),
        "second account should remain signed in after switching back"
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

fn unique_suffix() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let counter = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{}-{counter}", Utc::now().timestamp_micros())
}
