//! Capability-discovery steps for self-permissions navigation.

use std::collections::HashSet;

use cucumber::{then, when};
use secrecy::ExposeSecret;
use secrecy::SecretString;
use tanren_identity_policy::Email;
use tanren_testkit::HarnessOutcome;

use crate::TanrenWorld;

#[when(expr = "{word} discovers their self-permissions capability")]
async fn when_discover_self_permissions_capability(world: &mut TanrenWorld, actor: String) {
    let session_account_id = ensure_actor_signed_in(world, &actor).await;
    let ctx = world.ensure_account_ctx().await;
    let before = snapshot_event_ids(ctx).await;
    ctx.event_ids_before_permissions_query = Some(before);

    match ctx
        .harness
        .my_permissions_capability(session_account_id, None)
        .await
    {
        Ok(capability) => {
            ctx.last_permissions_capability = Some(capability);
            ctx.last_permissions = None;
            ctx.last_permissions_failure_code = None;
            ctx.last_outcome = Some(HarnessOutcome::Other(
                "permissions_capability_loaded".to_owned(),
            ));
        }
        Err(err) => {
            let code = err.code();
            ctx.last_permissions_capability = None;
            ctx.last_permissions = None;
            ctx.last_permissions_failure_code = Some(code.clone());
            ctx.last_outcome = Some(HarnessOutcome::FailureCode(code));
        }
    }
}

#[when(expr = "an unauthenticated client discovers self-permissions capability")]
async fn when_unauthenticated_discover_self_permissions_capability(world: &mut TanrenWorld) {
    let ctx = world.ensure_account_ctx().await;
    let before = snapshot_event_ids(ctx).await;
    ctx.event_ids_before_permissions_query = Some(before);

    match ctx
        .harness
        .my_permissions_capability(tanren_identity_policy::AccountId::fresh(), None)
        .await
    {
        Ok(capability) => {
            ctx.last_permissions_capability = Some(capability);
            ctx.last_permissions = None;
            ctx.last_permissions_failure_code = None;
            ctx.last_outcome = Some(HarnessOutcome::Other(
                "permissions_capability_loaded_unexpectedly".to_owned(),
            ));
        }
        Err(err) => {
            let code = err.code();
            ctx.last_permissions_capability = None;
            ctx.last_permissions = None;
            ctx.last_permissions_failure_code = Some(code.clone());
            ctx.last_outcome = Some(HarnessOutcome::FailureCode(code));
        }
    }
}

#[when(
    expr = "{word} attempts to discover {word}'s permissions capability through the self capability view"
)]
async fn when_discover_other_permissions_capability(
    world: &mut TanrenWorld,
    actor: String,
    target: String,
) {
    let session_account_id = ensure_actor_signed_in(world, &actor).await;
    let target_account_id = {
        let ctx = world.ensure_account_ctx().await;
        actor_account_id(ctx, &target)
    };
    let ctx = world.ensure_account_ctx().await;
    let before = snapshot_event_ids(ctx).await;
    ctx.event_ids_before_permissions_query = Some(before);

    match ctx
        .harness
        .my_permissions_capability(session_account_id, Some(target_account_id))
        .await
    {
        Ok(capability) => {
            ctx.last_permissions_capability = Some(capability);
            ctx.last_permissions = None;
            ctx.last_permissions_failure_code = None;
            ctx.last_outcome = Some(HarnessOutcome::Other(
                "permissions_capability_loaded_unexpectedly".to_owned(),
            ));
        }
        Err(err) => {
            let code = err.code();
            ctx.last_permissions_capability = None;
            ctx.last_permissions = None;
            ctx.last_permissions_failure_code = Some(code.clone());
            ctx.last_outcome = Some(HarnessOutcome::FailureCode(code));
        }
    }
}

#[then(expr = "the capability indicates my permissions navigation is available")]
async fn then_capability_indicates_nav_available(world: &mut TanrenWorld) {
    let ctx = world.ensure_account_ctx().await;
    let view = ctx
        .last_permissions_capability
        .as_ref()
        .expect("capability discovery should have succeeded");
    assert!(
        view.response.can_view_my_permissions,
        "expected can_view_my_permissions=true; got {:?}",
        view.response
    );
    assert!(
        view.rendered.contains("can_view_my_permissions"),
        "expected rendered output to include can_view_my_permissions; got {}",
        view.rendered
    );
}

#[then(expr = "{word} sees the My permissions navigation link on the home page")]
async fn then_sees_my_permissions_link_on_home(world: &mut TanrenWorld, actor: String) {
    drop(actor);
    let ctx = world.ensure_account_ctx().await;
    let view = ctx
        .last_permissions_capability
        .as_ref()
        .expect("capability discovery should have succeeded");
    assert!(
        view.response.can_view_my_permissions,
        "expected can_view_my_permissions=true before showing navigation link"
    );
}

async fn ensure_actor_signed_in(
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
        .expect("sign-in before capability query must succeed");
    let entry = ctx
        .actors
        .get_mut(actor)
        .expect("actor state should still be present");
    entry.sign_in = Some(response.clone());
    response.account_id
}

fn actor_account_id(ctx: &crate::AccountContext, actor: &str) -> tanren_identity_policy::AccountId {
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

async fn snapshot_event_ids(ctx: &mut crate::AccountContext) -> HashSet<String> {
    ctx.harness
        .recent_events(200)
        .await
        .expect("recent_events should succeed")
        .into_iter()
        .map(|event| event.id.to_string())
        .collect()
}
