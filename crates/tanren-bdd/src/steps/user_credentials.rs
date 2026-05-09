//! User-credential redaction/hygiene step definitions for B-0125.

use cucumber::{then, when};
use tanren_configuration_secrets::OwnerScope;
use tanren_identity_policy::AccountId;
use tanren_testkit::{HarnessOutcome, record_failure};

use crate::{AccountContext, TanrenWorld};

#[when(expr = "{word} lists user credentials for {word}'s account")]
async fn when_list_credentials_for_actor(world: &mut TanrenWorld, actor: String, target: String) {
    let requested = {
        let ctx = world.ensure_account_ctx().await;
        actor_account_id(ctx, &target)
    };
    let ctx = world.ensure_account_ctx().await;
    let result = ctx.harness.list_user_credentials(requested).await;
    let entry = ctx.actors.entry(actor).or_default();
    let outcome = match result {
        Ok(response) => {
            entry.last_user_credentials = response.items;
            HarnessOutcome::Other("credentials_listed".to_owned())
        }
        Err(err) => record_failure(err, entry),
    };
    ctx.last_outcome = Some(outcome);
}

#[then(expr = "{word} sees user credential metadata scoped to their own account")]
async fn then_sees_credential_scope(world: &mut TanrenWorld, actor: String) {
    let ctx = world.ensure_account_ctx().await;
    let account_id = actor_account_id(ctx, &actor);
    let entry = ctx
        .actors
        .get(&actor)
        .expect("actor must have credentials snapshot");
    assert!(
        entry
            .last_user_credentials
            .iter()
            .all(|item| item.owner_scope == OwnerScope::User { account_id }),
        "expected all credential metadata rows to be user-scoped to {actor}; got {:?}",
        entry.last_user_credentials
    );
}

#[then(expr = "{word} sees user credential metadata with a last-updated timestamp")]
async fn then_sees_credential_updated_at(world: &mut TanrenWorld, actor: String) {
    let ctx = world.ensure_account_ctx().await;
    let entry = ctx
        .actors
        .get(&actor)
        .expect("actor must have credentials snapshot");
    assert!(
        entry
            .last_user_credentials
            .iter()
            .all(|item| item.updated_at >= item.created_at),
        "expected every metadata row to have updated_at >= created_at; got {:?}",
        entry.last_user_credentials
    );
}

#[then(
    expr = "{word} sees no credential plaintext value {string} in read paths, logs, audit, or event projections"
)]
async fn then_no_plaintext_in_reads_or_events(
    world: &mut TanrenWorld,
    actor: String,
    value: String,
) {
    let ctx = world.ensure_account_ctx().await;
    let mut inspected = Vec::new();
    inspected.push(format!("harness_kind={:?}", ctx.harness.kind()));
    inspected.extend(actor_snapshots(&ctx.actors));

    if let Some(outcome) = &ctx.last_outcome {
        inspected.push(format!("log.last_outcome={outcome:?}"));
    }

    let events = ctx
        .harness
        .recent_events(100)
        .await
        .expect("recent_events should succeed under BDD");
    let event_kinds = tanren_testkit::event_kinds(&events);
    inspected.push(format!("event_projection.kinds={event_kinds:?}"));
    let audit_projection: Vec<_> = events
        .iter()
        .filter(|event| {
            event
                .payload
                .get("family")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|family| family == "configuration")
                && event
                    .payload
                    .get("kind")
                    .and_then(serde_json::Value::as_str)
                    .is_some_and(|kind| {
                        kind == "user_credential_changed" || kind == "user_credential_removed"
                    })
        })
        .map(|event| &event.payload)
        .collect();
    let audit_projection =
        serde_json::to_string(&audit_projection).expect("audit projection should serialize");
    inspected.push(format!("audit_projection={audit_projection}"));
    let event_projection =
        serde_json::to_string(&events).expect("recent event projection should serialize");
    inspected.push(format!("event_projection.rows={event_projection}"));

    let combined = inspected.join("\n");
    assert!(
        !combined.contains(&value),
        "expected no plaintext credential leak for {actor}; found sentinel value in read/log/audit/event projections"
    );
}

fn actor_snapshots(
    actors: &std::collections::HashMap<String, tanren_testkit::ActorState>,
) -> Vec<String> {
    let mut inspected = Vec::new();
    for (name, state) in actors {
        let credentials = serde_json::to_string(&state.last_user_credentials)
            .expect("credential metadata snapshot should serialize");
        let settings = serde_json::to_string(&state.last_user_settings)
            .expect("setting metadata snapshot should serialize");
        inspected.push(format!(
            "actor={name} credentials={credentials} settings={settings}"
        ));
        inspected.push(format!(
            "actor={name} remembered_credential_id={:?} last_failure={:?}",
            state.remembered_credential_id, state.last_failure
        ));
    }
    inspected
}

fn actor_account_id(ctx: &AccountContext, actor: &str) -> AccountId {
    let entry = ctx
        .actors
        .get(actor)
        .expect("actor must have prior account state");
    entry
        .sign_in
        .as_ref()
        .map(|session| session.account_id)
        .or_else(|| entry.sign_up.as_ref().map(|session| session.account_id))
        .or_else(|| {
            entry
                .accept_invitation
                .as_ref()
                .map(|acceptance| acceptance.session.account_id)
        })
        .expect("actor must have a known account id")
}
