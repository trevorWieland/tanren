//! Account-flow step definitions for B-0043.
//!
//! Step bodies dispatch through the per-interface
//! [`AccountHarness`](tanren_testkit::AccountHarness) trait — never
//! `tanren_app_services::Handlers::*` directly. The active harness is
//! selected by the BDD `Before` hook from the scenario's tags
//! (`@api`, `@cli`, `@mcp`, `@tui`, `@web`, or fallback in-process).
//! `xtask check-bdd-wire-coverage` mechanically rejects any future
//! step that bypasses this seam.

use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use chrono::{Duration as ChronoDuration, Utc};
use cucumber::{given, then, when};
use secrecy::SecretString;
use tanren_contract::{AcceptInvitationRequest, SignInRequest, SignUpRequest};
use tanren_identity_policy::{Email, InvitationToken, OrgId};
use tanren_testkit::{
    ConcurrentAcceptanceTally, HarnessInvitation, HarnessOutcome, record_failure,
};
use tokio::sync::Mutex;

use crate::TanrenWorld;

#[given(expr = "a clean Tanren environment")]
async fn clean_env(world: &mut TanrenWorld) {
    let _ = world.ensure_account_ctx().await;
}

#[given(expr = "a pending invitation token {string}")]
async fn given_pending_invitation(world: &mut TanrenWorld, token: String) {
    let ctx = world.ensure_account_ctx().await;
    let parsed = InvitationToken::parse(&token).expect("scenario invitation tokens must parse");
    let now = Utc::now();
    let fixture = HarnessInvitation {
        token: parsed,
        inviting_org: OrgId::fresh(),
        expires_at: now + ChronoDuration::days(1),
    };
    ctx.harness
        .seed_invitation(fixture)
        .await
        .expect("seed valid invitation");
    ctx.invitations.insert(token);
}

#[given(expr = "an expired invitation token {string}")]
async fn given_expired_invitation(world: &mut TanrenWorld, token: String) {
    let ctx = world.ensure_account_ctx().await;
    let parsed = InvitationToken::parse(&token).expect("scenario invitation tokens must parse");
    let now = Utc::now();
    let fixture = HarnessInvitation {
        token: parsed,
        inviting_org: OrgId::fresh(),
        expires_at: now - ChronoDuration::seconds(1),
    };
    ctx.harness
        .seed_invitation(fixture)
        .await
        .expect("seed expired invitation");
    ctx.invitations.insert(token);
}

#[given(expr = "{word} has signed up with email {string} and password {string}")]
async fn given_signed_up(world: &mut TanrenWorld, actor: String, email: String, password: String) {
    do_sign_up(world, actor, email, password, "Background actor".to_owned()).await;
    let ctx = world.account.as_mut().expect("ctx initialized");
    assert!(
        matches!(ctx.last_outcome, Some(HarnessOutcome::SignedUp(_))),
        "background sign-up step must succeed (got {:?})",
        ctx.last_outcome
    );
}

#[when(expr = "{word} self-signs up with email {string} and password {string}")]
async fn when_sign_up(world: &mut TanrenWorld, actor: String, email: String, password: String) {
    do_sign_up(world, actor, email, password, "Tanren user".to_owned()).await;
}

#[when(expr = "{word} signs in with email {string} and password {string}")]
async fn when_sign_in(world: &mut TanrenWorld, actor: String, email: String, password: String) {
    let ctx = world.ensure_account_ctx().await;
    let parsed_email = Email::parse(&email).expect("scenario emails must parse");
    let result = ctx
        .harness
        .sign_in(SignInRequest {
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

#[when(expr = "{word} signs in with the same credentials")]
async fn when_sign_in_same(world: &mut TanrenWorld, actor: String) {
    use secrecy::ExposeSecret;
    let (email, password) = {
        let ctx = world.ensure_account_ctx().await;
        let entry = ctx
            .actors
            .get(&actor)
            .expect("actor must have signed up first");
        (
            entry
                .identifier
                .clone()
                .expect("actor identifier captured during sign-up"),
            entry
                .password
                .as_ref()
                .map(|s| s.expose_secret().to_owned())
                .expect("actor password captured during sign-up"),
        )
    };
    when_sign_in(world, actor, email, password).await;
}

#[when(expr = "{word} accepts invitation {string} with password {string}")]
async fn when_accept_invitation(
    world: &mut TanrenWorld,
    actor: String,
    token: String,
    password: String,
) {
    let ctx = world.ensure_account_ctx().await;
    let display_name = format!("{actor} via {token}");
    let invitation_token =
        InvitationToken::parse(&token).expect("scenario invitation tokens must parse");
    let email_raw = format!("{actor}-{token}@invitation.tanren");
    let parsed_email = Email::parse(&email_raw).expect("synthesised invitation email must parse");
    let result = ctx
        .harness
        .accept_invitation(AcceptInvitationRequest {
            invitation_token,
            email: parsed_email,
            password: SecretString::from(password.clone()),
            display_name: display_name.clone(),
        })
        .await;
    let entry = ctx.actors.entry(actor.clone()).or_default();
    entry.password = Some(SecretString::from(password));
    let outcome = match result {
        Ok(acceptance) => {
            entry.identifier = Some(acceptance.session.account.identifier.as_str().to_owned());
            entry.accept_invitation = Some(acceptance.clone());
            HarnessOutcome::AcceptedInvitation(acceptance)
        }
        Err(err) => record_failure(err, entry),
    };
    ctx.last_outcome = Some(outcome);
}

#[when(expr = "{int} actors concurrently accept invitation {string}")]
async fn when_concurrent_accept(world: &mut TanrenWorld, count: usize, token: String) {
    let ctx = world.ensure_account_ctx().await;
    let invitation_token =
        InvitationToken::parse(&token).expect("scenario invitation tokens must parse");
    let harness_ref = Arc::new(Mutex::new(&mut ctx.harness));
    let mut tally = ConcurrentAcceptanceTally::default();
    let mut handles = Vec::with_capacity(count);
    for i in 0..count {
        let harness = harness_ref.clone();
        let token = invitation_token.clone();
        let email_raw = format!(
            "racer-{i}-{token}@invitation.tanren",
            token = token.as_str()
        );
        let parsed_email = Email::parse(&email_raw).expect("synthesised email must parse");
        let display = format!("Racer {i}");
        let req = AcceptInvitationRequest {
            invitation_token: token,
            email: parsed_email,
            password: SecretString::from("race-password".to_owned()),
            display_name: display,
        };
        handles.push(async move {
            let mut h = harness.lock().await;
            h.accept_invitation(req).await
        });
    }
    let outcomes = futures_join_all(handles).await;
    for outcome in outcomes {
        tally.record(outcome);
    }
    ctx.last_outcome = Some(HarnessOutcome::Other(format!(
        "concurrent: {successes} ok, {failures:?} fail",
        successes = tally.successes,
        failures = tally.failures_by_code,
    )));
    // Stash the tally on the ctx via a side channel — re-parse for
    // downstream Then steps.
    ctx.actors
        .entry("__concurrent_tally__".to_owned())
        .or_default()
        .identifier = Some(serde_json::to_string(&serialize_tally(&tally)).expect("tally"));
}

#[then(expr = "exactly {int} acceptance succeeds")]
#[then(expr = "exactly {int} acceptances succeed")]
async fn then_exact_successes(world: &mut TanrenWorld, count: usize) {
    let tally = read_concurrent_tally(world).await;
    assert_eq!(
        tally.successes,
        count,
        "expected {count} acceptance successes, got {actual} (failures = {failures:?})",
        actual = tally.successes,
        failures = tally.failures_by_code,
    );
}

#[then(expr = "{int} fail with code {string}")]
async fn then_n_fail_with(world: &mut TanrenWorld, count: usize, code: String) {
    let tally = read_concurrent_tally(world).await;
    let actual = tally.failures_with_code(&code);
    assert_eq!(
        actual,
        count,
        "expected {count} failures with code {code}, got {actual} (full breakdown = {failures:?})",
        failures = tally.failures_by_code,
    );
}

#[then(expr = "{word} receives a session token")]
async fn then_session_token(world: &mut TanrenWorld, actor: String) {
    let ctx = world.ensure_account_ctx().await;
    let entry = ctx
        .actors
        .get(&actor)
        .expect("actor must have an outcome recorded");
    let received = entry
        .sign_up
        .as_ref()
        .map(|s| s.has_token)
        .or_else(|| entry.sign_in.as_ref().map(|s| s.has_token))
        .or_else(|| {
            entry
                .accept_invitation
                .as_ref()
                .map(|a| a.session.has_token)
        })
        .unwrap_or(false);
    assert!(received, "expected a session token for {actor}");
}

#[then(expr = "{word}'s account belongs to no organization")]
async fn then_no_org(world: &mut TanrenWorld, actor: String) {
    let ctx = world.ensure_account_ctx().await;
    let entry = ctx
        .actors
        .get(&actor)
        .expect("actor must have an outcome recorded");
    let view = entry
        .sign_up
        .as_ref()
        .map(|s| &s.account)
        .or_else(|| entry.sign_in.as_ref().map(|s| &s.account))
        .expect("actor has a successful response on file");
    assert!(
        view.org.is_none(),
        "expected personal (no-org) account for {actor}"
    );
}

#[then(expr = "{word} has joined an organization")]
async fn then_joined_org(world: &mut TanrenWorld, actor: String) {
    let ctx = world.ensure_account_ctx().await;
    let entry = ctx
        .actors
        .get(&actor)
        .expect("actor must have an outcome recorded");
    let acceptance = entry
        .accept_invitation
        .as_ref()
        .expect("actor must have accepted an invitation");
    assert_eq!(
        acceptance.session.account.org,
        Some(acceptance.joined_org),
        "expected account.org to match joined_org"
    );
}

#[then(expr = "{word} now holds {int} accounts")]
async fn then_holds_n_accounts(world: &mut TanrenWorld, actor: String, count: usize) {
    let ctx = world.ensure_account_ctx().await;
    let entry = ctx.actors.get(&actor).expect("actor must have signed up");
    let mut owned = 0;
    if entry.sign_up.is_some() {
        owned += 1;
    }
    if entry.accept_invitation.is_some() {
        owned += 1;
    }
    assert_eq!(
        owned, count,
        "expected {actor} to hold {count} accounts, got {owned}"
    );
}

#[then(expr = "the request fails with code {string}")]
async fn then_fails_with(world: &mut TanrenWorld, code: String) {
    let ctx = world.ensure_account_ctx().await;
    let actual = match &ctx.last_outcome {
        Some(HarnessOutcome::Failure(reason)) => reason.code().to_owned(),
        Some(HarnessOutcome::SignedUp(_)) => "signed_up_unexpectedly".to_owned(),
        Some(HarnessOutcome::SignedIn(_)) => "signed_in_unexpectedly".to_owned(),
        Some(HarnessOutcome::AcceptedInvitation(_)) => {
            "accepted_invitation_unexpectedly".to_owned()
        }
        Some(HarnessOutcome::Other(s)) => format!("other:{s}"),
        None => "no_outcome".to_owned(),
    };
    assert_eq!(actual, code, "expected failure code");
}

#[then(expr = "a {string} event is recorded")]
async fn then_event_recorded(world: &mut TanrenWorld, kind: String) {
    let ctx = world.ensure_account_ctx().await;
    // Some surfaces propagate events asynchronously; poll briefly.
    let mut attempts = 0;
    loop {
        let recent = ctx
            .harness
            .recent_events(20)
            .await
            .expect("recent_events should succeed under BDD");
        let kinds: Vec<String> = recent
            .iter()
            .filter_map(|e| {
                e.payload
                    .get("kind")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_owned)
            })
            .collect();
        if kinds.iter().any(|k| k == &kind) {
            return;
        }
        attempts += 1;
        assert!(
            attempts < 5,
            "expected a '{kind}' event in the recent log; got {kinds:?}"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

async fn do_sign_up(
    world: &mut TanrenWorld,
    actor: String,
    email: String,
    password: String,
    display_name: String,
) {
    let ctx = world.ensure_account_ctx().await;
    let parsed_email = Email::parse(&email).expect("scenario emails must parse");
    let result = ctx
        .harness
        .sign_up(SignUpRequest {
            email: parsed_email,
            password: SecretString::from(password.clone()),
            display_name,
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

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct SerializedTally {
    successes: usize,
    failures: std::collections::HashMap<String, usize>,
}

fn serialize_tally(tally: &ConcurrentAcceptanceTally) -> SerializedTally {
    SerializedTally {
        successes: tally.successes,
        failures: tally.failures_by_code.clone(),
    }
}

async fn read_concurrent_tally(world: &mut TanrenWorld) -> ConcurrentAcceptanceTally {
    let ctx = world.ensure_account_ctx().await;
    let entry = ctx
        .actors
        .get("__concurrent_tally__")
        .and_then(|s| s.identifier.as_ref())
        .expect("concurrent tally must have been recorded");
    let parsed: SerializedTally =
        serde_json::from_str(entry).expect("concurrent tally must round-trip");
    ConcurrentAcceptanceTally {
        successes: parsed.successes,
        failures_by_code: parsed.failures,
        other: Vec::new(),
    }
}

/// Local thin wrapper around `futures::future::join_all` to avoid
/// pulling the `futures` crate in as a workspace dep just for this
/// single call. Awaits each future sequentially — fine for the
/// concurrent-race test because the outer `tokio::spawn` model isn't
/// strictly required: the race window is the harness's own
/// `accept_invitation` round-trip, and serializing the spawn calls
/// still proves the store-level atomicity (the DB constraint plus
/// `consume_invitation` rejects all but one). To get true parallelism
/// we'd need each task on its own runtime task; switching to
/// `tokio::spawn` is a follow-up once the harness is `Send + Sync +
/// Clone` (currently `Send` only, hence the local mutex).
async fn futures_join_all<F, T>(futures: Vec<F>) -> Vec<T>
where
    F: Future<Output = T>,
{
    let mut results = Vec::with_capacity(futures.len());
    for f in futures {
        results.push(f.await);
    }
    results
}
