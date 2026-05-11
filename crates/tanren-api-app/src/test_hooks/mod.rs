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
//! # Security invariants
//!
//! - **Body limit**: every route is wrapped in a
//!   [`DefaultBodyLimit`] layer set to [`MAX_JSON_BODY_BYTES`], so
//!   oversized payloads are rejected by the framework *before* any
//!   `Json` extractor allocates or parses bytes.
//! - **Loopback guard**: every route extracts [`LoopbackGuard`],
//!   which checks `ConnectInfo<SocketAddr>` for a loopback IP or
//!   accepts the request if the in-process marker extension is
//!   present. `X-Real-IP` / `X-Forwarded-For` are never trusted.
//! - **Typed payloads**: handlers receive typed request structs
//!   (`serde::Deserialize`) and return typed response structs — never
//!   raw `serde_json::Value`.

mod guard;
mod limits;

use std::sync::Arc;

use axum::Json;
use axum::Router;
use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::post;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tanren_identity_policy::{InvitationToken, OrgId};
use tanren_store::{NewInvitation, Store};
use uuid::Uuid;

pub(crate) use guard::InProcessMarker;
use guard::LoopbackGuard;
use limits::MAX_JSON_BODY_BYTES;

/// Request body for `POST /test-hooks/invitations`.
#[derive(Debug, Deserialize)]
pub(crate) struct SeedInvitationRequest {
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

/// Response body for `POST /test-hooks/invitations`.
#[derive(Debug, Serialize)]
pub(crate) struct SeedInvitationResponse {
    /// UUID of the seeded invitation row.
    pub id: Uuid,
}

pub(crate) async fn seed_invitation_route(
    _guard: LoopbackGuard,
    State(store): State<Arc<Store>>,
    Json(body): Json<SeedInvitationRequest>,
) -> Result<(StatusCode, Json<SeedInvitationResponse>), (StatusCode, String)> {
    let token = InvitationToken::parse(&body.token)
        .map_err(|err| (StatusCode::BAD_REQUEST, err.to_string()))?;
    let inviting_org_id = body.inviting_org_id.map_or_else(OrgId::fresh, OrgId::new);
    let id = Uuid::new_v4();
    store
        .seed_invitation(NewInvitation {
            token,
            inviting_org_id,
            expires_at: body.expires_at,
        })
        .await
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?;
    Ok((StatusCode::CREATED, Json(SeedInvitationResponse { id })))
}

/// Build the `/test-hooks/*` router. The state is the shared
/// `Arc<Store>` already constructed by `build_app` / `build_app_with_store`.
///
/// The router applies a body-limit layer so oversized requests are
/// rejected before any `Json` extractor runs.
pub(crate) fn router(store: Arc<Store>) -> Router {
    Router::new()
        .route("/test-hooks/invitations", post(seed_invitation_route))
        .with_state(store)
        .layer(axum::extract::DefaultBodyLimit::max(MAX_JSON_BODY_BYTES))
}
