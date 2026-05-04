//! Posture-flow step definitions for R-0015.
//!
//! Step bodies dispatch through the per-interface
//! [`PostureHarness`](tanren_testkit::PostureHarness) trait — never
//! `tanren_app_services::Handlers::*` directly. The active harness is
//! selected by the BDD `Before` hook from the scenario's tags
//! (`@api`, `@cli`, `@mcp`, `@tui`, `@web`, or fallback in-process).
//! `xtask check-bdd-wire-coverage` mechanically rejects any future
//! step that bypasses this seam.

use cucumber::{given, then, when};
use tanren_domain::Posture;
use tanren_identity_policy::AccountId;
use tanren_testkit::{HarnessError, HarnessOutcome, PostureHarnessActor};

use crate::TanrenWorld;

#[given(expr = "the installation has no posture configured")]
async fn given_no_posture(world: &mut TanrenWorld) {
    let ctx = world.ensure_account_ctx().await;
    let _ = ctx.harness.kind();
}

#[when(expr = "the actor lists available postures")]
async fn when_list_postures(world: &mut TanrenWorld) {
    let ctx = world.ensure_account_ctx().await;
    let result = ctx.harness.posture().list_postures().await;
    match result {
        Ok(response) => {
            ctx.last_posture_list = Some(response.postures);
        }
        Err(err) => {
            let outcome = record_posture_failure(err);
            ctx.last_outcome = Some(outcome);
        }
    }
}

#[when(expr = "the actor queries the current posture")]
async fn when_get_posture(world: &mut TanrenWorld) {
    let ctx = world.ensure_account_ctx().await;
    let result = ctx.harness.posture().get_posture().await;
    match result {
        Ok(response) => {
            ctx.last_posture = Some(response.current);
        }
        Err(err) => {
            let outcome = record_posture_failure(err);
            ctx.last_outcome = Some(outcome);
        }
    }
}

#[when(expr = "the admin sets the posture to {string}")]
async fn when_set_posture_admin(world: &mut TanrenWorld, value: String) {
    do_set_posture(world, value, true).await;
}

#[when(expr = "a non-admin sets the posture to {string}")]
async fn when_set_posture_non_admin(world: &mut TanrenWorld, value: String) {
    do_set_posture(world, value, false).await;
}

#[then(expr = "the posture list contains {int} entries")]
async fn then_list_count(world: &mut TanrenWorld, count: usize) {
    let ctx = world.ensure_account_ctx().await;
    let list = ctx
        .last_posture_list
        .as_ref()
        .expect("posture list must have been queried");
    assert_eq!(
        list.len(),
        count,
        "expected {count} posture entries, got {actual}",
        actual = list.len()
    );
}

#[then(expr = "the posture list includes {string}")]
async fn then_list_includes(world: &mut TanrenWorld, name: String) {
    let ctx = world.ensure_account_ctx().await;
    let list = ctx
        .last_posture_list
        .as_ref()
        .expect("posture list must have been queried");
    let expected = Posture::parse(&name).expect("scenario posture names must parse");
    let found = list.iter().any(|pv| pv.posture == expected);
    assert!(found, "expected {name} in posture list");
}

#[then(expr = "the current posture is {string}")]
async fn then_current_posture(world: &mut TanrenWorld, expected: String) {
    let ctx = world.ensure_account_ctx().await;
    let current = ctx
        .last_posture
        .as_ref()
        .expect("current posture must have been queried");
    let expected_posture = Posture::parse(&expected).expect("scenario posture names must parse");
    assert_eq!(
        current.posture,
        expected_posture,
        "expected posture {expected}, got {actual}",
        actual = current.posture
    );
}

#[then(expr = "the posture request fails with code {string}")]
async fn then_posture_fails_with(world: &mut TanrenWorld, code: String) {
    let ctx = world.ensure_account_ctx().await;
    let actual = match &ctx.last_outcome {
        Some(HarnessOutcome::Other(s)) => {
            let parts: Vec<&str> = s.splitn(2, ": ").collect();
            if parts.len() == 2 && parts[0] == "posture" {
                parts[1].to_owned()
            } else {
                format!("other:{s}")
            }
        }
        Some(HarnessOutcome::Failure(reason)) => reason.code().to_owned(),
        Some(
            HarnessOutcome::SignedUp(_)
            | HarnessOutcome::SignedIn(_)
            | HarnessOutcome::AcceptedInvitation(_),
        ) => "unexpected_success".to_owned(),
        None => "no_outcome".to_owned(),
    };
    assert_eq!(actual, code, "expected posture failure code");
}

async fn do_set_posture(world: &mut TanrenWorld, value: String, admin: bool) {
    let ctx = world.ensure_account_ctx().await;
    let actor = PostureHarnessActor {
        account_id: AccountId::fresh(),
        posture_admin: admin,
    };
    let result = ctx.harness.posture().set_posture_raw(actor, value).await;
    match result {
        Ok(response) => {
            ctx.last_posture = Some(response.current);
        }
        Err(err) => {
            let outcome = record_posture_failure(err);
            ctx.last_outcome = Some(outcome);
        }
    }
}

fn record_posture_failure(err: HarnessError) -> HarnessOutcome {
    match err {
        HarnessError::Posture(reason, _) => {
            HarnessOutcome::Other(format!("posture: {}", reason.code()))
        }
        HarnessError::Transport(msg) => HarnessOutcome::Other(format!("transport: {msg}")),
        HarnessError::Account(reason, _) => {
            HarnessOutcome::Other(format!("account: {}", reason.code()))
        }
    }
}
