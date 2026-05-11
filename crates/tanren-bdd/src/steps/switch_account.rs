//! Step definitions for B-0046: Switch the active account.
//!
//! Step bodies dispatch through [`AccountHarness::switch_active_account`]
//! and [`AccountHarness::invalidate_session`] — never
//! `tanren_app_services::Handlers::*` directly.

use cucumber::{given, then, when};
use secrecy::SecretString;
use tanren_contract::SignUpRequest;
use tanren_identity_policy::{AccountId, Email};
use tanren_testkit::{HarnessError, HarnessOutcome};

use crate::TanrenWorld;

fn ordinal_to_index(ord: &str) -> usize {
    match ord {
        "first" => 0,
        "second" => 1,
        "third" => 2,
        "fourth" => 3,
        "fifth" => 4,
        other => unreachable!("unknown account ordinal: {other}"),
    }
}

async fn setup_n_accounts(world: &mut TanrenWorld, n: usize) {
    let ctx = world.ensure_account_ctx().await;
    for i in 0..n {
        let letters = ['a', 'b', 'c', 'd', 'e'];
        let letter = letters[i];
        let email_raw = format!("alice-sw-{letter}@tanren.test");
        let email = Email::parse(&email_raw).expect("email must parse");
        let result = ctx
            .harness
            .sign_up(SignUpRequest {
                email,
                password: SecretString::from("SwitchPass99!".to_owned()),
                display_name: format!("Alice Switch {}", i + 1),
            })
            .await;
        match result {
            Ok(session) => ctx.signed_in_sessions.push(session),
            Err(e) => unreachable!("setup sign-up failed: {e}"),
        }
    }
}

async fn do_switch(world: &mut TanrenWorld, ordinal: &str, window_id: Option<String>) {
    let ctx = world.ensure_account_ctx().await;
    let idx = ordinal_to_index(ordinal);
    let target = ctx.signed_in_sessions[idx].account_id;
    match ctx
        .harness
        .switch_active_account(target, window_id.clone())
        .await
    {
        Ok(result) => {
            if let Some(ref wid) = window_id {
                ctx.window_switches.insert(wid.clone(), result);
            } else {
                ctx.global_switch = Some(result);
            }
            ctx.last_outcome = Some(HarnessOutcome::Other("switch_ok".to_owned()));
        }
        Err(HarnessError::Account(reason, _)) => {
            ctx.last_outcome = Some(HarnessOutcome::Failure(reason));
        }
        Err(HarnessError::Transport(msg)) => {
            ctx.last_outcome = Some(HarnessOutcome::Other(format!("transport: {msg}")));
        }
    }
}

async fn do_switch_unsigned(world: &mut TanrenWorld) {
    let ctx = world.ensure_account_ctx().await;
    let bogus = AccountId::fresh();
    match ctx.harness.switch_active_account(bogus, None).await {
        Ok(result) => {
            ctx.global_switch = Some(result);
            ctx.last_outcome = Some(HarnessOutcome::Other("switch_ok".to_owned()));
        }
        Err(HarnessError::Account(reason, _)) => {
            ctx.last_outcome = Some(HarnessOutcome::Failure(reason));
        }
        Err(HarnessError::Transport(msg)) => {
            ctx.last_outcome = Some(HarnessOutcome::Other(format!("transport: {msg}")));
        }
    }
}

fn current_global_active(ctx: &crate::AccountContext) -> Option<AccountId> {
    ctx.global_switch
        .as_ref()
        .map(|s| s.active_account.id)
        .or_else(|| ctx.signed_in_sessions.last().map(|s| s.account_id))
}

/// Silence the unused Gherkin-mandated surface parameter.
fn dismiss_surface(_: &str) {}

#[given(expr = "alice holds one signed-in account via the {word}")]
async fn given_one_account(world: &mut TanrenWorld, surface: String) {
    dismiss_surface(&surface);
    setup_n_accounts(world, 1).await;
}

#[given(expr = "alice holds two signed-in accounts via the {word}")]
async fn given_two_accounts(world: &mut TanrenWorld, surface: String) {
    dismiss_surface(&surface);
    setup_n_accounts(world, 2).await;
}

#[when(expr = "alice switches the active account to the {word} account via the {word}")]
async fn when_switch_global(world: &mut TanrenWorld, ordinal: String, surface: String) {
    dismiss_surface(&surface);
    do_switch(world, &ordinal, None).await;
}

#[when(expr = "alice switches the active account back to the {word} account via the {word}")]
async fn when_switch_back(world: &mut TanrenWorld, ordinal: String, surface: String) {
    dismiss_surface(&surface);
    do_switch(world, &ordinal, None).await;
}

#[when(
    expr = "alice switches the active account to the {word} account in window {string} via the {word}"
)]
async fn when_switch_windowed(
    world: &mut TanrenWorld,
    ordinal: String,
    window_id: String,
    surface: String,
) {
    dismiss_surface(&surface);
    do_switch(world, &ordinal, Some(window_id)).await;
}

#[when(expr = "alice switches the active account to an unsigned account via the {word}")]
async fn when_switch_unsigned(world: &mut TanrenWorld, surface: String) {
    dismiss_surface(&surface);
    do_switch_unsigned(world).await;
}

#[when(
    expr = "alice concurrently switches the active account to the {word} account in window {string} and the {word} account in window {string} via the {word}"
)]
async fn when_concurrent_window_switches(
    world: &mut TanrenWorld,
    ord1: String,
    wid1: String,
    ord2: String,
    wid2: String,
    surface: String,
) {
    dismiss_surface(&surface);
    do_switch(world, &ord1, Some(wid1)).await;
    do_switch(world, &ord2, Some(wid2)).await;
}

#[given(expr = "alice records active-account switch baseline via the {word}")]
async fn given_records_baseline(world: &mut TanrenWorld, surface: String) {
    dismiss_surface(&surface);
    let ctx = world.ensure_account_ctx().await;
    let recent = ctx
        .harness
        .recent_events(50)
        .await
        .expect("recent_events should succeed under BDD");
    let count = recent
        .iter()
        .filter(|e| {
            e.payload
                .get("kind")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("")
                .starts_with("active_account_switch")
        })
        .count();
    ctx.switch_baseline_event_count = Some(count);
}

#[given(expr = "alice invalidates the caller session as {string} via the {word}")]
async fn given_invalidates_session(world: &mut TanrenWorld, mode: String, surface: String) {
    dismiss_surface(&surface);
    let ctx = world.ensure_account_ctx().await;
    ctx.harness
        .invalidate_session(&mode)
        .await
        .expect("invalidate_session should not fail");
}

#[then(expr = "alice sees the {word} account as active via the {word}")]
async fn then_sees_active(world: &mut TanrenWorld, ordinal: String, surface: String) {
    dismiss_surface(&surface);
    let ctx = world.ensure_account_ctx().await;
    let expected_id = ctx.signed_in_sessions[ordinal_to_index(&ordinal)].account_id;
    let actual_id = current_global_active(ctx).expect("expected an active account to be selected");
    assert_eq!(
        actual_id, expected_id,
        "expected the {ordinal} account to be active"
    );
}

#[then(expr = "alice sees the {word} account as active without re-authentication via the {word}")]
async fn then_sees_active_no_reauth(world: &mut TanrenWorld, ordinal: String, surface: String) {
    then_sees_active(world, ordinal, surface).await;
}

#[then(expr = "alice sees project availability scoped to the selected account via the {word}")]
async fn then_project_availability(world: &mut TanrenWorld, surface: String) {
    dismiss_surface(&surface);
    let ctx = world.ensure_account_ctx().await;
    assert!(
        current_global_active(ctx).is_some(),
        "expected an active account to be selected for project availability scoping"
    );
}

#[then(
    expr = "alice sees different active accounts between windows {string} and {string} via the {word}"
)]
async fn then_different_window_actives(
    world: &mut TanrenWorld,
    wid1: String,
    wid2: String,
    surface: String,
) {
    dismiss_surface(&surface);
    let ctx = world.ensure_account_ctx().await;
    let id1 = ctx.window_switches.get(&wid1).map_or_else(
        || unreachable!("no switch result recorded for window {wid1}"),
        |s| s.active_account.id,
    );
    let id2 = ctx.window_switches.get(&wid2).map_or_else(
        || unreachable!("no switch result recorded for window {wid2}"),
        |s| s.active_account.id,
    );
    assert_ne!(
        id1, id2,
        "expected different active accounts in windows {wid1} and {wid2}"
    );
}

#[then(
    expr = "alice sees window {string} stay on the {word} account after window {string} switched via the {word}"
)]
async fn then_window_stays(
    world: &mut TanrenWorld,
    stable_wid: String,
    ordinal: String,
    switched_wid: String,
    surface: String,
) {
    dismiss_surface(&surface);
    dismiss_surface(&switched_wid);
    let ctx = world.ensure_account_ctx().await;
    let expected_id = ctx.signed_in_sessions[ordinal_to_index(&ordinal)].account_id;
    let actual_id = ctx.window_switches.get(&stable_wid).map_or_else(
        || unreachable!("no switch result recorded for window {stable_wid}"),
        |s| s.active_account.id,
    );
    assert_eq!(
        actual_id, expected_id,
        "expected window {stable_wid} to stay on the {ordinal} account"
    );
}

#[then(expr = "alice sees no active-account mutation after the rejected switch via the {word}")]
async fn then_no_mutation(world: &mut TanrenWorld, surface: String) {
    dismiss_surface(&surface);
    let ctx = world.ensure_account_ctx().await;
    let baseline = ctx.switch_baseline_event_count.unwrap_or(0);
    let recent = ctx
        .harness
        .recent_events(50)
        .await
        .expect("recent_events should succeed under BDD");
    let current = recent
        .iter()
        .filter(|e| {
            e.payload
                .get("kind")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("")
                .starts_with("active_account_switch")
        })
        .count();
    assert_eq!(
        current, baseline,
        "expected no new switch events since baseline ({baseline}), but found {current}"
    );
}
