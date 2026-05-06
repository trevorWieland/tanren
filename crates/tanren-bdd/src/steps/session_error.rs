//! Session-error taxonomy step definitions for B-0289.

use cucumber::{then, when};

use crate::TanrenWorld;
use tanren_testkit::{HarnessOutcome, record_failure};

#[when(expr = "an unauthenticated request lists organizations")]
async fn when_unauthenticated_lists_orgs(world: &mut TanrenWorld) {
    let ctx = world.ensure_account_ctx().await;
    let result = ctx
        .harness
        .unauthenticated_request("GET", "/account/organizations")
        .await;
    match result {
        Err(err) => {
            let entry = ctx.actors.entry("anonymous".to_owned()).or_default();
            let outcome = record_failure(err, entry);
            ctx.last_outcome = Some(outcome);
        }
        Ok(_) => {
            ctx.last_outcome = Some(HarnessOutcome::Other("unexpected success".to_owned()));
        }
    }
}

#[when(expr = "an unauthenticated request switches active organization to {string}")]
async fn when_unauthenticated_switches_org(world: &mut TanrenWorld, org_id: String) {
    let ctx = world.ensure_account_ctx().await;
    let result = ctx
        .harness
        .unauthenticated_request_with_body(
            "POST",
            "/account/organizations/active",
            serde_json::json!({ "org_id": org_id }),
        )
        .await;
    match result {
        Err(err) => {
            let _ = org_id;
            let entry = ctx.actors.entry("anonymous".to_owned()).or_default();
            let outcome = record_failure(err, entry);
            ctx.last_outcome = Some(outcome);
        }
        Ok(_) => {
            ctx.last_outcome = Some(HarnessOutcome::Other("unexpected success".to_owned()));
        }
    }
}

#[then(expr = "the error code is {string}")]
async fn then_error_code_is(world: &mut TanrenWorld, expected_code: String) {
    let ctx = world.ensure_account_ctx().await;
    let actual = match &ctx.last_outcome {
        Some(HarnessOutcome::Failure(reason)) => reason.code().to_owned(),
        Some(HarnessOutcome::Other(s)) => s.clone(),
        Some(other) => format!("{other:?}"),
        None => "no_outcome".to_owned(),
    };
    assert_eq!(actual, expected_code, "error code mismatch");
}
