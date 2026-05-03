//! Account-flow step definitions for B-0043.
//!
//! Drives the same `tanren_app_services::Handlers` facade every
//! interface binary delegates to. Per the equivalent-operations rule
//! in `docs/architecture/subsystems/interfaces.md`, the api / mcp /
//! cli / tui / web surfaces all resolve to these handlers, so the
//! interface tag on a scenario is a witness label rather than a
//! transport switch — the mechanism under proof is identical.

use chrono::Duration;
use cucumber::{given, then, when};
use secrecy::SecretString;
use tanren_contract::{AcceptInvitationRequest, SignInRequest, SignUpRequest};
use tanren_identity_policy::{Email, InvitationToken};
use tanren_testkit::InvitationFixture;

use crate::{ActorState, Outcome, TanrenWorld};

#[given(expr = "a clean Tanren environment")]
async fn clean_env(world: &mut TanrenWorld) {
    let _ = world.ensure_account_ctx().await;
}

#[given(expr = "a pending invitation token {string}")]
async fn given_pending_invitation(world: &mut TanrenWorld, token: String) {
    let ctx = world.ensure_account_ctx().await;
    let now = ctx.clock.read();
    let parsed = InvitationToken::parse(&token).expect("scenario invitation tokens must parse");
    let mut fixture = InvitationFixture::valid(now);
    fixture.token = parsed;
    tanren_testkit::seed_invitation(&ctx.store, &fixture)
        .await
        .expect("seed valid invitation");
    ctx.invitations.insert(token, fixture);
}

#[given(expr = "an expired invitation token {string}")]
async fn given_expired_invitation(world: &mut TanrenWorld, token: String) {
    let ctx = world.ensure_account_ctx().await;
    let now = ctx.clock.read();
    let parsed = InvitationToken::parse(&token).expect("scenario invitation tokens must parse");
    let mut fixture = InvitationFixture::expired(now);
    fixture.token = parsed;
    tanren_testkit::seed_invitation(&ctx.store, &fixture)
        .await
        .expect("seed expired invitation");
    ctx.invitations.insert(token, fixture);
}

#[given(expr = "{word} has signed up with email {string} and password {string}")]
async fn given_signed_up(world: &mut TanrenWorld, actor: String, email: String, password: String) {
    do_sign_up(world, actor, email, password, "Background actor".to_owned()).await;
    let ctx = world.account.as_mut().expect("ctx initialized");
    assert!(
        matches!(ctx.last_outcome, Some(Outcome::SignedUp(_))),
        "background sign-up step must succeed"
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
        .handlers
        .sign_in(
            &ctx.store,
            SignInRequest {
                email: parsed_email,
                password: SecretString::from(password.clone()),
            },
        )
        .await;
    let entry = ctx.actors.entry(actor.clone()).or_default();
    entry.identifier = Some(email);
    entry.password = Some(password);
    let outcome = match result {
        Ok(response) => {
            entry.sign_in = Some(response.clone());
            Outcome::SignedIn(response)
        }
        Err(err) => failure_outcome(err, entry),
    };
    ctx.last_outcome = Some(outcome);
}

#[when(expr = "{word} signs in with the same credentials")]
async fn when_sign_in_same(world: &mut TanrenWorld, actor: String) {
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
                .clone()
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
    // PR 7 sources the invitee email from the invitation row; for PR 3 the
    // step synthesises a deterministic email from the actor name + token so
    // every invitation acceptance lands on a unique identifier.
    let email_raw = format!("{actor}-{token}@invitation.tanren");
    let parsed_email = Email::parse(&email_raw).expect("synthesised invitation email must parse");
    let result = ctx
        .handlers
        .accept_invitation(
            &ctx.store,
            AcceptInvitationRequest {
                invitation_token,
                email: parsed_email,
                password: SecretString::from(password.clone()),
                display_name: display_name.clone(),
            },
        )
        .await;
    let entry = ctx.actors.entry(actor.clone()).or_default();
    entry.password = Some(password);
    let outcome = match result {
        Ok(response) => {
            entry.identifier = Some(response.account.identifier.as_str().to_owned());
            entry.accept_invitation = Some(response.clone());
            Outcome::AcceptedInvitation(response)
        }
        Err(err) => failure_outcome(err, entry),
    };
    ctx.last_outcome = Some(outcome);
}

#[when(expr = "the clock advances past the invitation expiry")]
async fn when_clock_advances(world: &mut TanrenWorld) {
    let ctx = world.ensure_account_ctx().await;
    let now = ctx.clock.read();
    ctx.clock.set(now + Duration::days(2));
}

#[then(expr = "{word} receives a session token")]
async fn then_session_token(world: &mut TanrenWorld, actor: String) {
    let ctx = world.ensure_account_ctx().await;
    let entry = ctx
        .actors
        .get(&actor)
        .expect("actor must have an outcome recorded");
    let token = match entry.sign_up.as_ref() {
        Some(r) => Some(r.session.token.expose_secret()),
        None => entry
            .sign_in
            .as_ref()
            .map(|r| r.session.token.expose_secret())
            .or_else(|| {
                entry
                    .accept_invitation
                    .as_ref()
                    .map(|r| r.session.token.expose_secret())
            }),
    };
    assert!(
        token.is_some_and(|t| !t.is_empty()),
        "expected non-empty session token for {actor}"
    );
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
        .map(|r| &r.account)
        .or_else(|| entry.sign_in.as_ref().map(|r| &r.account))
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
    let response = entry
        .accept_invitation
        .as_ref()
        .expect("actor must have accepted an invitation");
    assert_eq!(response.account.org, Some(response.joined_org));
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
        Some(Outcome::Failure(reason)) => reason.code().to_owned(),
        Some(Outcome::SignedUp(_)) => "signed_up_unexpectedly".to_owned(),
        Some(Outcome::SignedIn(_)) => "signed_in_unexpectedly".to_owned(),
        Some(Outcome::AcceptedInvitation(_)) => "accepted_invitation_unexpectedly".to_owned(),
        Some(Outcome::Other(s)) => format!("other:{s}"),
        None => "no_outcome".to_owned(),
    };
    assert_eq!(actual, code, "expected failure code");
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
        .handlers
        .sign_up(
            &ctx.store,
            SignUpRequest {
                email: parsed_email,
                password: SecretString::from(password.clone()),
                display_name,
            },
        )
        .await;
    let entry = ctx.actors.entry(actor.clone()).or_default();
    entry.identifier = Some(email);
    entry.password = Some(password);
    let outcome = match result {
        Ok(response) => {
            entry.sign_up = Some(response.clone());
            Outcome::SignedUp(response)
        }
        Err(err) => failure_outcome(err, entry),
    };
    ctx.last_outcome = Some(outcome);
}

fn failure_outcome(err: tanren_app_services::AppServiceError, entry: &mut ActorState) -> Outcome {
    match err {
        tanren_app_services::AppServiceError::Account(reason) => {
            entry.last_failure = Some(reason);
            Outcome::Failure(reason)
        }
        tanren_app_services::AppServiceError::InvalidInput(message) => {
            Outcome::Other(format!("invalid_input: {message}"))
        }
        tanren_app_services::AppServiceError::Store(err) => Outcome::Other(format!("store: {err}")),
        _ => Outcome::Other("unknown".to_owned()),
    }
}
