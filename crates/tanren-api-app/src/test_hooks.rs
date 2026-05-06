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
//! The endpoints here are deliberately permissive (no auth, no rate
//! limiting): the contract is that they are loopback-only, gated by a
//! test-only Cargo feature, and exercised exclusively by the BDD
//! `globalSetup` flow that just spawned the binary.

use std::sync::Arc;

use axum::Json;
use axum::Router;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tanren_identity_policy::{AccountId, Identifier, InvitationToken, OrgId, OrgPermissions};
use tanren_store::{AccountStore, NewInvitation, Store};
use uuid::Uuid;

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
    /// Target identifier (email) for addressed invitations. Omit for
    /// open (new-account) invitations.
    #[serde(default)]
    pub target_identifier: Option<Identifier>,
    /// Organization-level permissions granted on acceptance. Omit to
    /// default to member permissions at the service layer.
    #[serde(default)]
    pub org_permissions: Option<OrgPermissions>,
    /// When set, seeds the invitation in a revoked state so BDD
    /// scenarios can exercise the revoked-invitation rejection path.
    #[serde(default)]
    pub revoked_at: Option<DateTime<Utc>>,
}

pub(crate) async fn seed_invitation_route(
    State(store): State<Arc<Store>>,
    Json(body): Json<SeedInvitationBody>,
) -> Result<StatusCode, (StatusCode, String)> {
    let token = InvitationToken::parse(&body.token)
        .map_err(|err| (StatusCode::BAD_REQUEST, err.to_string()))?;
    let inviting_org_id = body.inviting_org_id.map_or_else(OrgId::fresh, OrgId::new);
    store
        .seed_invitation(NewInvitation {
            token,
            inviting_org_id,
            expires_at: body.expires_at,
            target_identifier: body.target_identifier,
            org_permissions: body.org_permissions,
            revoked: body.revoked_at.is_some(),
        })
        .await
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?;

    Ok(StatusCode::CREATED)
}

/// Request body for `POST /test-hooks/memberships`.
#[derive(Debug, Deserialize)]
pub(crate) struct SeedMembershipBody {
    pub account_id: AccountId,
    #[serde(default)]
    pub org_id: Option<OrgId>,
    #[serde(default)]
    pub org_permissions: Option<OrgPermissions>,
}

pub(crate) async fn seed_membership_route(
    State(store): State<Arc<Store>>,
    Json(body): Json<SeedMembershipBody>,
) -> Result<StatusCode, (StatusCode, String)> {
    let org_id = body.org_id.unwrap_or_else(OrgId::fresh);
    store
        .seed_membership(body.account_id, org_id, body.org_permissions, Utc::now())
        .await
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?;
    Ok(StatusCode::CREATED)
}

#[derive(Debug, Deserialize)]
pub(crate) struct SeedInFlightWorkBody {
    pub account_id: AccountId,
    pub org_id: OrgId,
    pub description: String,
}

pub(crate) async fn seed_in_flight_work_route(
    State(store): State<Arc<Store>>,
    Json(body): Json<SeedInFlightWorkBody>,
) -> Result<StatusCode, (StatusCode, String)> {
    store
        .seed_in_flight_work(body.account_id, body.org_id, body.description, Utc::now())
        .await
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?;
    Ok(StatusCode::CREATED)
}

#[derive(Debug, Deserialize)]
pub(crate) struct RecentEventsQuery {
    #[serde(default = "default_limit")]
    pub limit: u64,
}

fn default_limit() -> u64 {
    20
}

#[derive(Debug, Serialize)]
pub(crate) struct EventEntry {
    pub kind: String,
    pub payload: serde_json::Value,
}

pub(crate) async fn recent_events_route(
    State(store): State<Arc<Store>>,
    Query(query): Query<RecentEventsQuery>,
) -> Result<Json<Vec<EventEntry>>, (StatusCode, String)> {
    let events = AccountStore::recent_events(store.as_ref(), query.limit)
        .await
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?;
    let entries: Vec<EventEntry> = events
        .into_iter()
        .map(|e| {
            let kind = e
                .payload
                .get("kind")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_owned();
            EventEntry {
                kind,
                payload: e.payload,
            }
        })
        .collect();
    Ok(Json(entries))
}

/// Build the `/test-hooks/*` router. The state is the shared
/// `Arc<Store>` already constructed by `build_app` / `build_app_with_store`.
pub(crate) fn router(store: Arc<Store>) -> Router {
    Router::new()
        .route("/test-hooks/invitations", post(seed_invitation_route))
        .route("/test-hooks/memberships", post(seed_membership_route))
        .route(
            "/test-hooks/in-flight-work",
            post(seed_in_flight_work_route),
        )
        .route("/test-hooks/events", get(recent_events_route))
        .with_state(store)
}
