//! Bounded active-account read-model step definitions for B-0041.

use cucumber::{then, when};
use tanren_testkit::HarnessKind;

use crate::TanrenWorld;

/// Maximum number of signed-in accounts the server and validators accept.
/// Must match `tanren_app_services::ACTIVE_ACCOUNT_REGISTRY_LIMIT`.
const ACTIVE_ACCOUNT_LIMIT: usize = 16;

#[when(expr = "{word} lists active accounts via the {word}")]
async fn when_list_active_accounts(world: &mut TanrenWorld, actor: String, surface: String) {
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

    let entry = ctx.actors.entry(actor).or_default();
    let result = ctx.harness.list_active_accounts().await;
    match result {
        Ok(accounts) => {
            entry.last_failure = None;
            ctx.bounded_list_result = Some(accounts);
        }
        Err(_err) => {
            ctx.bounded_list_result = None;
        }
    }
}

#[then(expr = "{word} receives at most 16 signed-in accounts via the {word}")]
async fn then_at_most_16_accounts(world: &mut TanrenWorld, actor: String, surface: String) {
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
    let _ = ctx.actors.entry(actor).or_default();
    let accounts = ctx
        .bounded_list_result
        .as_ref()
        .expect("list active accounts must have succeeded");
    assert!(
        accounts.len() <= ACTIVE_ACCOUNT_LIMIT,
        "active-account list has {} entries, exceeding limit of {}",
        accounts.len(),
        ACTIVE_ACCOUNT_LIMIT,
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
