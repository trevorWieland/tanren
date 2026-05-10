//! Shared helper utilities for B-0039 permissions step modules.

use std::collections::HashSet;

use secrecy::ExposeSecret;
use secrecy::SecretString;
use tanren_identity_policy::Email;

use crate::TanrenWorld;

pub(super) async fn ensure_actor_signed_in(
    world: &mut TanrenWorld,
    actor: &str,
) -> tanren_identity_policy::AccountId {
    let (email_raw, password_raw) = {
        let ctx = world.ensure_account_ctx().await;
        let entry = ctx
            .actors
            .get(actor)
            .expect("actor must be registered first");
        (
            entry
                .identifier
                .clone()
                .expect("actor identifier should be recorded"),
            entry
                .password
                .as_ref()
                .map(|secret| secret.expose_secret().to_owned())
                .expect("actor password should be recorded"),
        )
    };
    let ctx = world.ensure_account_ctx().await;
    let email = Email::parse(&email_raw).expect("scenario email must parse");
    let response = ctx
        .harness
        .sign_in(tanren_contract::SignInRequest {
            email,
            password: SecretString::from(password_raw),
        })
        .await
        .expect("sign-in before permissions query must succeed");
    let entry = ctx
        .actors
        .get_mut(actor)
        .expect("actor state should still be present");
    entry.sign_in = Some(response.clone());
    response.account_id
}

pub(super) fn actor_account_id(
    ctx: &crate::AccountContext,
    actor: &str,
) -> tanren_identity_policy::AccountId {
    let entry = ctx
        .actors
        .get(actor)
        .expect("actor must have prior sign-up/sign-in");
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
        .expect("actor must have a session/account id")
}

pub(super) async fn snapshot_event_ids(ctx: &mut crate::AccountContext) -> HashSet<String> {
    ctx.harness
        .recent_events(200)
        .await
        .expect("recent_events should succeed")
        .into_iter()
        .map(|event| event.id.to_string())
        .collect()
}
