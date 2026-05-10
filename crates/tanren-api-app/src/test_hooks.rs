//! Test-only HTTP routes mounted under `/test-hooks/*`.
//!
//! These exist solely to give the Playwright (`@web`) BDD runner the
//! same fixture-seeding seam that the Rust BDD harness already has via
//! direct `Arc<Store>` access. The Playwright runner cannot share a
//! process with the api binary, so it cannot reach `Store::seed_*`
//! through Rust — it has to talk over the wire.
//!
//! The whole module sits behind the `test-hooks` Cargo feature. The
//! production `tanren-api` binary does not enable that feature, so the
//! `/test-hooks/*` routes are simply absent from the production router
//! (no runtime guard, no env-var check — the routes do not compile in).
//!
//! ## Security
//!
//! All test-hook routes enforce two runtime guards:
//!
//! 1. **Loopback-only**: requests from non-loopback addresses are
//!    rejected with `403 Forbidden`.
//! 2. **Per-run secret**: requests must carry a matching
//!    `X-Test-Hook-Secret` header. The secret is generated once per API
//!    process and shared with the Playwright driver via an environment
//!    variable — never checked into files.

use std::sync::Arc;

use axum::Json;
use axum::Router;
use axum::extract::State;
use axum::http::StatusCode;
use axum::middleware;
use axum::routing::post;
use chrono::{DateTime, Utc};
use serde::Deserialize;
use tanren_identity_policy::{InvitationToken, OrgId};
use tanren_store::{NewInvitation, Store};
use uuid::Uuid;

mod guard;
mod limits;
pub(crate) mod upgrade_fixture;

#[derive(Clone)]
pub(crate) struct TestHooksState {
    store: Arc<Store>,
    upgrade_fixture: upgrade_fixture::UpgradeFixtureHarness,
}

/// Request body for `POST /test-hooks/invitations`.
#[derive(Debug, Deserialize)]
pub(crate) struct SeedInvitationBody {
    /// Opaque invitation token. Must round-trip through
    /// [`InvitationToken::parse`] (i.e. obey the same length/charset
    /// rules that production tokens do).
    pub token: String,
    /// Optional inviting org UUID. Omit to let the seeder allocate a
    /// fresh `OrgId` — most BDD scenarios don't care which org the
    /// invitee joins, only that they joined *some* org.
    #[serde(default)]
    pub inviting_org_id: Option<Uuid>,
    /// Wall-clock expiry instant in ISO 8601. May be in the past for
    /// expired-invitation falsification scenarios.
    pub expires_at: DateTime<Utc>,
}

pub(crate) async fn seed_invitation_route(
    State(state): State<TestHooksState>,
    Json(body): Json<SeedInvitationBody>,
) -> Result<StatusCode, (StatusCode, String)> {
    let token = InvitationToken::parse(&body.token)
        .map_err(|err| (StatusCode::BAD_REQUEST, err.to_string()))?;
    let inviting_org_id = body.inviting_org_id.map_or_else(OrgId::fresh, OrgId::new);
    state
        .store
        .seed_invitation(NewInvitation {
            token,
            inviting_org_id,
            expires_at: body.expires_at,
        })
        .await
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?;
    Ok(StatusCode::CREATED)
}

/// Build the `/test-hooks/*` router. The state is the shared
/// `Arc<Store>` already constructed by `build_app` / `build_app_with_store`.
///
/// The router applies the loopback + secret guard as an `axum` middleware
/// layer so every request is checked before reaching any handler.
pub(crate) fn router(store: Arc<Store>) -> Router {
    let secret = guard::generate_secret();
    tracing::info!(
        target: "tanren_api::test_hooks",
        "test-hook routes mounted with per-run secret"
    );
    let shared_state = TestHooksState {
        store,
        upgrade_fixture: upgrade_fixture::UpgradeFixtureHarness::new(),
    };
    let secret_for_mw = secret.clone();
    Router::new()
        .route("/test-hooks/invitations", post(seed_invitation_route))
        .route(
            "/test-hooks/upgrade-fixture/{action}",
            post(upgrade_fixture::upgrade_fixture_action_route),
        )
        .with_state(shared_state)
        .layer(middleware::from_fn(move |req, next| {
            let s = secret_for_mw.clone();
            guard::guard_middleware(s, req, next)
        }))
}
