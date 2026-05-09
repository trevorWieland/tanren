//! Deployment-posture step definitions for B-0137.
//!
//! Every step routes through the active [`AccountHarness`](tanren_testkit::AccountHarness)
//! implementation selected by scenario tag (`@api`, `@cli`, `@mcp`, `@tui`, `@web`).

use cucumber::{given, then, when};
use secrecy::SecretString;
use tanren_contract::{
    DeploymentPosture, DeploymentPostureCapability, DeploymentPostureCapabilitySummary,
    DeploymentPostureScope, SetDeploymentPostureRequest, SignInRequest,
};
use tanren_identity_policy::{AccountId, Email};
use tanren_testkit::{HarnessError, HarnessKind, HarnessOutcome, record_failure};

use super::{poll_until, retry_on_transport, sign_up_actor_with_duplicate_sign_in_fallback};
use crate::TanrenWorld;

#[given(expr = "an {word} account actor with posture permission")]
async fn given_actor_with_permission(world: &mut TanrenWorld, interface: String) {
    seed_actor_accounts(world, interface.as_str(), false).await;
}

#[given(expr = "a {word} account actor with posture permission")]
async fn given_actor_with_permission_article(world: &mut TanrenWorld, interface: String) {
    given_actor_with_permission(world, interface).await;
}

#[given(expr = "a {word} account actor without posture permission")]
async fn given_actor_without_permission(world: &mut TanrenWorld, interface: String) {
    seed_actor_accounts(world, interface.as_str(), true).await;
}

#[given(expr = "an {word} account actor without posture permission")]
async fn given_actor_without_permission_article(world: &mut TanrenWorld, interface: String) {
    given_actor_without_permission(world, interface).await;
}

#[when(expr = "the actor lists supported deployment postures over {word}")]
async fn when_list_supported(world: &mut TanrenWorld, interface: String) {
    let ctx = world.ensure_account_ctx().await;
    assert_interface(ctx.harness.kind(), &interface);
    let supported = ctx
        .harness
        .list_supported_postures()
        .await
        .expect("supported deployment postures must be readable");
    ctx.deployment_posture.last_supported = supported;
}

#[when(expr = "the actor sets deployment posture {string} for their account scope over {word}")]
async fn when_set_for_their_scope(world: &mut TanrenWorld, posture: String, interface: String) {
    let actor_id = ensure_signed_in_actor(world, "actor").await;
    let Some(posture) = parse_requested_posture(world, "actor", &posture).await else {
        return;
    };
    let request = SetDeploymentPostureRequest {
        scope: DeploymentPostureScope::Account {
            account_id: actor_id,
        },
        posture,
    };
    execute_set(world, &interface, "actor", actor_id, request).await;
}

#[when(expr = "the actor sets deployment posture {string} for another account scope over {word}")]
async fn when_set_for_another_scope(world: &mut TanrenWorld, posture: String, interface: String) {
    let actor_id = ensure_signed_in_actor(world, "actor").await;
    let Some(posture) = parse_requested_posture(world, "actor", &posture).await else {
        return;
    };
    let other_id = account_id_for(world, "other").await;
    let scope = DeploymentPostureScope::Account {
        account_id: other_id,
    };
    let request = SetDeploymentPostureRequest { scope, posture };
    execute_set(world, &interface, "actor", actor_id, request).await;
}

#[when(expr = "the actor sets deployment posture {string} for a missing account scope over {word}")]
async fn when_set_for_missing_scope(world: &mut TanrenWorld, posture: String, interface: String) {
    let actor_id = ensure_signed_in_actor(world, "actor").await;
    let Some(posture) = parse_requested_posture(world, "actor", &posture).await else {
        return;
    };
    let request = SetDeploymentPostureRequest {
        scope: DeploymentPostureScope::Account {
            account_id: AccountId::fresh(),
        },
        posture,
    };
    execute_set(world, &interface, "actor", actor_id, request).await;
}

#[then(expr = "the {word} supported posture list includes capability summaries")]
async fn then_supported_list_has_summaries(world: &mut TanrenWorld, interface: String) {
    let ctx = world.ensure_account_ctx().await;
    assert_interface(ctx.harness.kind(), &interface);
    let supported = &ctx.deployment_posture.last_supported;
    assert_eq!(supported.len(), 3, "expected three supported postures");
    for posture in DeploymentPosture::ALL {
        assert!(
            supported.iter().any(|entry| entry.posture == posture),
            "missing supported posture {}",
            posture.as_wire_value()
        );
        let entry = supported
            .iter()
            .find(|entry| entry.posture == posture)
            .expect("supported posture presence just asserted");
        assert_eq!(
            entry.capability_summary,
            DeploymentPostureCapabilitySummary::for_posture(posture),
            "capability summary mismatch for posture {}",
            posture.as_wire_value(),
        );
    }
}

#[then(expr = "the {word} response shows posture {string}")]
#[then(expr = "the {word} output shows posture {string}")]
#[then(expr = "the {word} view shows posture {string}")]
async fn then_response_shows_posture(world: &mut TanrenWorld, interface: String, posture: String) {
    let ctx = world.ensure_account_ctx().await;
    assert_interface(ctx.harness.kind(), &interface);
    let expected = parse_posture(&posture);
    let last = ctx
        .deployment_posture
        .last_set
        .as_ref()
        .expect("a posture set response must be recorded");
    assert_eq!(last.posture, expected, "unexpected posture in response");
}

#[then(expr = "the {word} response includes available and unavailable capability summaries")]
#[then(expr = "the {word} output includes available and unavailable capability summaries")]
#[then(expr = "the {word} view shows available and unavailable capability summaries")]
async fn then_has_capability_summary(world: &mut TanrenWorld, interface: String) {
    let ctx = world.ensure_account_ctx().await;
    assert_interface(ctx.harness.kind(), &interface);
    let last = ctx
        .deployment_posture
        .last_set
        .as_ref()
        .expect("a posture set response must be recorded");
    assert!(
        !last.capability_summary.available.is_empty(),
        "expected at least one available capability"
    );
    assert!(
        !last.capability_summary.unavailable.is_empty(),
        "expected at least one unavailable capability"
    );
}

#[then(expr = "the recorded posture for the actor account over {word} is {string}")]
async fn then_recorded_posture(world: &mut TanrenWorld, interface: String, posture: String) {
    let scope = DeploymentPostureScope::Account {
        account_id: account_id_for(world, "actor").await,
    };
    let ctx = world.ensure_account_ctx().await;
    assert_interface(ctx.harness.kind(), &interface);
    let current = ctx
        .harness
        .get_deployment_posture(scope)
        .await
        .expect("deployment posture should be readable");
    ctx.deployment_posture.last_get = Some(current.clone());
    let response = current.expect("a recorded posture should exist after set");
    assert_eq!(response.posture, parse_posture(&posture));
}

#[then(expr = "recent events attribute the posture change to the actor with posture {string}")]
async fn then_event_attribution(world: &mut TanrenWorld, posture: String) {
    let actor_id = account_id_for(world, "actor").await.to_string();
    let expected_posture = posture;
    let ctx = world.ensure_account_ctx().await;
    let found = poll_until(|| async {
        let events = ctx
            .harness
            .recent_events(40)
            .await
            .expect("recent_events should succeed under BDD");
        events.iter().any(|event| {
            let family = event
                .payload
                .get("family")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default();
            let kind = event
                .payload
                .get("kind")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default();
            let payload = event
                .payload
                .get("payload")
                .cloned()
                .unwrap_or(serde_json::Value::Null);
            let changed_by = payload
                .get("changed_by")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default();
            let posture = payload
                .get("posture")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default();
            family == "deployment_posture"
                && kind == "changed"
                && changed_by == actor_id
                && posture == expected_posture
        })
    })
    .await;
    assert!(found, "expected attributed deployment posture event");
}

#[then(
    expr = "the {word} response reflects inherited runtime and credential capability availability for posture {string}"
)]
#[then(
    expr = "the {word} output reflects inherited runtime and credential capability availability for posture {string}"
)]
#[then(
    expr = "the {word} view reflects inherited runtime and credential capability availability for posture {string}"
)]
async fn then_capability_inheritance(world: &mut TanrenWorld, interface: String, posture: String) {
    let ctx = world.ensure_account_ctx().await;
    assert_interface(ctx.harness.kind(), &interface);
    let expected = parse_posture(&posture);
    let summary = &ctx
        .deployment_posture
        .last_set
        .as_ref()
        .expect("a posture set response must be recorded")
        .capability_summary;
    assert_eq!(
        summary,
        &DeploymentPostureCapabilitySummary::for_posture(expected),
        "capability summary should match the canonical posture mapping",
    );

    let provider = summary
        .available
        .contains(&DeploymentPostureCapability::ProviderIntegrations);
    let remote = summary
        .available
        .contains(&DeploymentPostureCapability::RemoteRuntimeDispatch);
    let local = summary
        .available
        .contains(&DeploymentPostureCapability::LocalRuntimeDispatch);

    match expected {
        DeploymentPosture::Hosted => {
            assert!(
                provider,
                "hosted should keep provider integrations available"
            );
            assert!(remote, "hosted should keep remote runtime available");
            assert!(!local, "hosted should not expose local runtime dispatch");
        }
        DeploymentPosture::SelfHosted => {
            assert!(
                provider,
                "self_hosted should keep provider integrations available"
            );
            assert!(remote, "self_hosted should keep remote runtime available");
            assert!(local, "self_hosted should keep local runtime available");
        }
        DeploymentPosture::LocalOnly => {
            assert!(!provider, "local_only should disable provider integrations");
            assert!(!remote, "local_only should disable remote runtime dispatch");
            assert!(local, "local_only should keep local runtime available");
        }
    }
}

#[then(expr = "the error summary is readable")]
async fn then_error_summary_readable(world: &mut TanrenWorld) {
    let ctx = world.ensure_account_ctx().await;
    let entry = ctx
        .actors
        .get("actor")
        .expect("actor state should exist before checking failures");
    let summary = entry
        .last_failure_summary
        .as_deref()
        .unwrap_or_default()
        .trim();
    assert!(
        !summary.is_empty() && summary.chars().any(char::is_alphabetic),
        "expected a non-empty readable error summary"
    );
}

async fn seed_actor_accounts(world: &mut TanrenWorld, interface: &str, include_other: bool) {
    let actor_email = format!("actor-{interface}-posture@example.com").to_lowercase();
    sign_up_actor(
        world,
        "actor",
        &actor_email,
        "actor-posture-pw",
        "Posture actor",
    )
    .await;
    if include_other {
        let other_email = format!("other-{interface}-posture@example.com").to_lowercase();
        sign_up_actor(
            world,
            "other",
            &other_email,
            "other-posture-pw",
            "Other account",
        )
        .await;
        let _ = ensure_signed_in_actor(world, "actor").await;
    }
}

async fn sign_up_actor(
    world: &mut TanrenWorld,
    actor_label: &str,
    email: &str,
    password: &str,
    display_name: &str,
) {
    sign_up_actor_with_duplicate_sign_in_fallback(
        world,
        actor_label,
        email,
        password,
        display_name,
    )
    .await;
    let ctx = world.ensure_account_ctx().await;
    assert!(
        matches!(ctx.last_outcome, Some(HarnessOutcome::SignedUp(_))),
        "{actor_label} sign-up must succeed for posture scenarios"
    );
}
async fn execute_set(
    world: &mut TanrenWorld,
    interface: &str,
    actor_label: &str,
    actor_id: AccountId,
    request: SetDeploymentPostureRequest,
) {
    let ctx = world.ensure_account_ctx().await;
    assert_interface(ctx.harness.kind(), interface);
    let result = ctx.harness.set_deployment_posture(actor_id, request).await;
    let entry = ctx.actors.entry(actor_label.to_owned()).or_default();
    match result {
        Ok(response) => {
            ctx.deployment_posture.last_set = Some(response);
            ctx.last_outcome = Some(HarnessOutcome::Other("deployment_posture_set".to_owned()));
            entry.last_failure = None;
            entry.last_failure_summary = None;
        }
        Err(err) => {
            ctx.last_outcome = Some(record_failure(err, entry));
        }
    }
}

async fn ensure_signed_in_actor(world: &mut TanrenWorld, actor_label: &str) -> AccountId {
    let (email, password, account_id) = {
        let ctx = world.ensure_account_ctx().await;
        let entry = ctx.actors.get(actor_label).expect("missing actor state");
        let email = entry
            .identifier
            .clone()
            .expect("actor email should be recorded");
        let password = entry
            .password
            .as_ref()
            .map_or("", secrecy::ExposeSecret::expose_secret)
            .to_owned();
        let account_id = entry
            .sign_up
            .as_ref()
            .map(|s| s.account_id)
            .expect("actor must have a sign-up session");
        (email, password, account_id)
    };

    let ctx = world.ensure_account_ctx().await;
    let session_result = retry_on_transport!({
        let parsed_email = Email::parse(&email).expect("recorded actor email should parse");
        let request = SignInRequest {
            email: parsed_email,
            password: SecretString::from(password.clone()),
        };
        ctx.harness.sign_in(request)
    });
    let session = session_result.expect("actor sign-in must succeed before posture mutation");
    let entry = ctx
        .actors
        .get_mut(actor_label)
        .expect("actor entry must exist");
    entry.sign_in = Some(session);
    ctx.last_outcome = Some(HarnessOutcome::Other("actor_signed_in".to_owned()));

    account_id
}

async fn account_id_for(world: &mut TanrenWorld, actor_label: &str) -> AccountId {
    let ctx = world.ensure_account_ctx().await;
    ctx.actors
        .get(actor_label)
        .and_then(|entry| entry.sign_up.as_ref().map(|session| session.account_id))
        .expect("missing account id for actor")
}

fn parse_posture(raw: &str) -> DeploymentPosture {
    DeploymentPosture::from_wire_value(raw).expect("invalid posture in scenario")
}

async fn parse_requested_posture(
    world: &mut TanrenWorld,
    actor_label: &str,
    raw: &str,
) -> Option<DeploymentPosture> {
    if let Some(posture) = DeploymentPosture::from_wire_value(raw) {
        return Some(posture);
    }
    let ctx = world.ensure_account_ctx().await;
    let entry = ctx.actors.entry(actor_label.to_owned()).or_default();
    ctx.last_outcome = Some(record_failure(
        HarnessError::FailureCode {
            code: "unsupported_posture".to_owned(),
            summary: format!(
                "Unsupported deployment posture `{raw}`. Supported values: hosted, self_hosted, local_only."
            ),
        },
        entry,
    ));
    None
}

fn assert_interface(kind: HarnessKind, interface: &str) {
    let expected = match interface.to_ascii_lowercase().as_str() {
        "api" => Some(HarnessKind::Api),
        "cli" => Some(HarnessKind::Cli),
        "mcp" => Some(HarnessKind::Mcp),
        "tui" => Some(HarnessKind::Tui),
        "web" => Some(HarnessKind::Web),
        _ => None,
    }
    .expect("unsupported interface token in scenario");
    assert_eq!(
        kind, expected,
        "scenario requested {interface} but active harness is {kind:?}"
    );
}
