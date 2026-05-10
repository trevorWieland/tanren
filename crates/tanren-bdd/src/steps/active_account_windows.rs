//! Window-isolation active-account steps for B-0046.

use cucumber::{then, when};
use tanren_identity_policy::AccountId;
use tanren_testkit::{HarnessKind, HarnessOutcome, record_failure};

use crate::TanrenWorld;

#[when(
    expr = "{word} switches the active account to the {word} account in window {string} via the {word}"
)]
async fn when_switch_in_window(
    world: &mut TanrenWorld,
    actor: String,
    target: String,
    window_id: String,
    surface: String,
) {
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
    let target_account_id = parse_target_account(&target, first_account_id, second_account_id);

    let result = ctx
        .harness
        .switch_active_account_in_window(&window_id, target_account_id)
        .await;
    let entry = ctx.actors.entry(actor).or_default();
    match result {
        Ok(accounts) => {
            entry.last_failure = None;
            ctx.window_accounts.insert(window_id, accounts);
            ctx.last_outcome = Some(HarnessOutcome::Other("active_account_switched".to_owned()));
        }
        Err(err) => {
            ctx.last_outcome = Some(record_failure(err, entry));
        }
    }
}

#[when(
    expr = "{word} concurrently switches the active account to the {word} account in window {string} and the {word} account in window {string} via the {word}"
)]
async fn when_switch_in_windows_concurrently(
    world: &mut TanrenWorld,
    actor: String,
    first_target: String,
    first_window_id: String,
    second_target: String,
    second_window_id: String,
    surface: String,
) {
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
    let first_target_account =
        parse_target_account(&first_target, first_account_id, second_account_id);
    let second_target_account =
        parse_target_account(&second_target, first_account_id, second_account_id);

    let outcomes = ctx
        .harness
        .switch_active_accounts_in_windows_concurrent(vec![
            (first_window_id, first_target_account),
            (second_window_id, second_target_account),
        ])
        .await;

    let mut first_error = None;
    for (window_id, outcome) in outcomes {
        match outcome {
            Ok(accounts) => {
                ctx.window_accounts.insert(window_id, accounts);
            }
            Err(err) => {
                first_error = Some(err);
                break;
            }
        }
    }
    let entry = ctx.actors.entry(actor).or_default();
    if let Some(err) = first_error {
        ctx.last_outcome = Some(record_failure(err, entry));
    } else {
        entry.last_failure = None;
        ctx.last_outcome = Some(HarnessOutcome::Other("active_account_switched".to_owned()));
    }
}

#[then(
    expr = "{word} sees different active accounts between windows {string} and {string} via the {word}"
)]
async fn then_windows_differ(
    world: &mut TanrenWorld,
    actor: String,
    window_a: String,
    window_b: String,
    surface: String,
) {
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

    let accounts_a = if let Some(cached) = ctx.window_accounts.get(&window_a) {
        cached.clone()
    } else {
        let listed = ctx
            .harness
            .list_active_accounts_in_window(&window_a)
            .await
            .expect("window A account listing should succeed");
        ctx.window_accounts.insert(window_a.clone(), listed.clone());
        listed
    };
    let accounts_b = if let Some(cached) = ctx.window_accounts.get(&window_b) {
        cached.clone()
    } else {
        let listed = ctx
            .harness
            .list_active_accounts_in_window(&window_b)
            .await
            .expect("window B account listing should succeed");
        ctx.window_accounts.insert(window_b.clone(), listed.clone());
        listed
    };

    let active_a = active_account_id(&accounts_a);
    let active_b = active_account_id(&accounts_b);
    assert_ne!(
        active_a, active_b,
        "expected different active accounts for windows {window_a} and {window_b}"
    );
    assert!(
        accounts_a
            .iter()
            .any(|entry| entry.account.id == first_account_id),
        "window A should keep first account in signed-in set"
    );
    assert!(
        accounts_a
            .iter()
            .any(|entry| entry.account.id == second_account_id),
        "window A should keep second account in signed-in set"
    );
    assert!(
        accounts_b
            .iter()
            .any(|entry| entry.account.id == first_account_id),
        "window B should keep first account in signed-in set"
    );
    assert!(
        accounts_b
            .iter()
            .any(|entry| entry.account.id == second_account_id),
        "window B should keep second account in signed-in set"
    );
}

#[then(
    expr = "{word} sees window {string} stay on the {word} account after window {string} switched via the {word}"
)]
async fn then_no_window_leak(
    world: &mut TanrenWorld,
    actor: String,
    stable_window: String,
    expected_active: String,
    changed_window: String,
    surface: String,
) {
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
    let expected_id = parse_target_account(&expected_active, first_account_id, second_account_id);

    let stable_accounts = ctx
        .harness
        .list_active_accounts_in_window(&stable_window)
        .await
        .expect("stable window account listing should succeed");
    ctx.window_accounts
        .insert(stable_window.clone(), stable_accounts.clone());
    let changed_accounts = if let Some(cached) = ctx.window_accounts.get(&changed_window) {
        cached.clone()
    } else {
        let listed = ctx
            .harness
            .list_active_accounts_in_window(&changed_window)
            .await
            .expect("changed window account listing should succeed");
        ctx.window_accounts
            .insert(changed_window.clone(), listed.clone());
        listed
    };
    let stable_active = active_account_id(&stable_accounts);
    let changed_active = active_account_id(&changed_accounts);
    assert_eq!(
        stable_active, expected_id,
        "window {stable_window} should stay on {expected_active} account"
    );
    assert_ne!(
        stable_active, changed_active,
        "switch in window {changed_window} should not leak into {stable_window}"
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

fn parse_target_account(target: &str, first: AccountId, second: AccountId) -> AccountId {
    if target == "second" { second } else { first }
}

fn active_account_id(accounts: &[tanren_contract::SignedInAccountView]) -> AccountId {
    accounts
        .iter()
        .find(|entry| entry.is_active)
        .map(|entry| entry.account.id)
        .expect("exactly one active account should be present")
}
