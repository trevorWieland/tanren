//! Invalid-session active-account falsification steps for B-0046.

use cucumber::{given, then, when};
use tanren_contract::AccountFailureReason;
use tanren_testkit::{HarnessKind, HarnessOutcome, InvalidSessionKind, record_failure};

use crate::TanrenWorld;

#[given(expr = "{word} records active-account switch baseline via the {word}")]
async fn given_record_active_account_baseline(
    world: &mut TanrenWorld,
    actor: String,
    surface: String,
) {
    let expected_kind = parse_surface_kind(&surface);
    let ctx = world.ensure_account_ctx().await;
    assert_eq!(
        ctx.harness.kind(),
        expected_kind,
        "harness/surface mismatch"
    );

    let accounts = ctx
        .harness
        .list_active_accounts()
        .await
        .expect("list active accounts must succeed when recording baseline");
    assert!(
        accounts.iter().any(|entry| entry.is_active),
        "expected an active account in baseline listing"
    );

    let events = ctx
        .harness
        .recent_events(200)
        .await
        .expect("recent_events must succeed when recording baseline");
    let switched_event_count = events
        .iter()
        .filter(|event| {
            event
                .payload
                .get("kind")
                .and_then(serde_json::Value::as_str)
                == Some("active_account_switched")
        })
        .count();
    let entry = ctx.actors.entry(actor).or_default();
    entry.active_account_switched_event_count_before = Some(switched_event_count);
}

#[given(expr = "{word} invalidates the caller session as {string} via the {word}")]
#[when(expr = "{word} invalidates the caller session as {string} via the {word}")]
async fn when_invalidate_caller_session(
    world: &mut TanrenWorld,
    actor: String,
    mode: String,
    surface: String,
) {
    let expected_kind = parse_surface_kind(&surface);
    let ctx = world.ensure_account_ctx().await;
    assert_eq!(
        ctx.harness.kind(),
        expected_kind,
        "harness/surface mismatch"
    );

    let Some(mode) = parse_invalid_session_mode(&mode) else {
        let entry = ctx.actors.entry(actor).or_default();
        entry.last_failure = Some(AccountFailureReason::ValidationFailed);
        ctx.last_outcome = Some(HarnessOutcome::Failure(
            AccountFailureReason::ValidationFailed,
        ));
        return;
    };
    let result = ctx.harness.invalidate_caller_session(mode).await;
    let entry = ctx.actors.entry(actor).or_default();
    match result {
        Ok(()) => {
            entry.last_failure = None;
            ctx.last_outcome = Some(HarnessOutcome::Other(
                "caller_session_invalidated".to_owned(),
            ));
        }
        Err(err) => {
            ctx.last_outcome = Some(record_failure(err, entry));
        }
    }
}

#[then(expr = "{word} sees no active-account mutation after the rejected switch via the {word}")]
async fn then_no_active_account_mutation(world: &mut TanrenWorld, actor: String, surface: String) {
    let expected_kind = parse_surface_kind(&surface);
    let ctx = world.ensure_account_ctx().await;
    assert_eq!(
        ctx.harness.kind(),
        expected_kind,
        "harness/surface mismatch"
    );

    let expected_count = ctx
        .actors
        .get(&actor)
        .and_then(|entry| entry.active_account_switched_event_count_before)
        .expect("baseline active-account event count must be recorded");
    let events = ctx
        .harness
        .recent_events(200)
        .await
        .expect("recent_events should succeed");
    let actual_count = events
        .iter()
        .filter(|event| {
            event
                .payload
                .get("kind")
                .and_then(serde_json::Value::as_str)
                == Some("active_account_switched")
        })
        .count();
    assert_eq!(
        actual_count, expected_count,
        "expected no new active_account_switched event after rejected invalid-session switch"
    );
}

fn parse_invalid_session_mode(mode: &str) -> Option<InvalidSessionKind> {
    match mode {
        "missing" => Some(InvalidSessionKind::Missing),
        "expired" => Some(InvalidSessionKind::Expired),
        "revoked" => Some(InvalidSessionKind::Revoked),
        _ => None,
    }
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
