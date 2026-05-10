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
use tanren_identity_policy::{InvitationToken, OrgId};
use tanren_store::{AccountStore, NewInvitation, Store};
use uuid::Uuid;

const DEFAULT_RECENT_EVENTS_LIMIT: u64 = 200;
const MAX_RECENT_EVENTS_LIMIT: u64 = 500;

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
        })
        .await
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?;
    Ok(StatusCode::CREATED)
}

/// Query parameters for `GET /test-hooks/events/recent`.
#[derive(Debug, Deserialize)]
pub(crate) struct RecentEventsQuery {
    /// Optional event limit; bounded server-side.
    #[serde(default)]
    pub limit: Option<u64>,
}

/// Redacted event metadata returned by `GET /test-hooks/events/recent`.
#[derive(Debug, Serialize)]
pub(crate) struct RedactedEventMetadata {
    /// Event id from the canonical event stream.
    pub id: Uuid,
    /// Event occurrence timestamp.
    pub occurred_at: DateTime<Utc>,
    /// Event kind lifted from payload metadata.
    pub kind: Option<String>,
}

/// Response envelope for `GET /test-hooks/events/recent`.
#[derive(Debug, Serialize)]
pub(crate) struct RecentEventsResponse {
    /// Recent events with redacted metadata only.
    pub events: Vec<RedactedEventMetadata>,
}

pub(crate) async fn recent_events_route(
    State(store): State<Arc<Store>>,
    Query(query): Query<RecentEventsQuery>,
) -> Result<Json<RecentEventsResponse>, (StatusCode, String)> {
    let limit = bounded_recent_events_limit(query.limit);
    let events = AccountStore::recent_events(store.as_ref(), limit)
        .await
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?;
    let events = events
        .into_iter()
        .map(|event| RedactedEventMetadata {
            id: event.id,
            occurred_at: event.occurred_at,
            kind: event
                .payload
                .get("kind")
                .and_then(|value| value.as_str())
                .map(str::to_owned),
        })
        .collect();
    Ok(Json(RecentEventsResponse { events }))
}

fn bounded_recent_events_limit(limit: Option<u64>) -> u64 {
    let requested = limit.unwrap_or(DEFAULT_RECENT_EVENTS_LIMIT);
    requested.clamp(1, MAX_RECENT_EVENTS_LIMIT)
}

/// Build the `/test-hooks/*` router. The state is the shared
/// `Arc<Store>` already constructed by `build_app` / `build_app_with_store`.
pub(crate) fn router(store: Arc<Store>) -> Router {
    Router::new()
        .route("/test-hooks/invitations", post(seed_invitation_route))
        .route("/test-hooks/events/recent", get(recent_events_route))
        .with_state(store)
}
