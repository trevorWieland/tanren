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
use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::post;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tanren_identity_policy::{AccountId, DesignatedHost, InvitationToken, OrgId, RepositoryRef};
#[cfg(feature = "test-hooks")]
use tanren_provider_integrations::FixtureSourceControlProvider;
use tanren_store::{NewInvitation, Store};
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

/// Build the `/test-hooks/*` router. The state is the shared
/// `Arc<Store>` already constructed by `build_app` / `build_app_with_store`.
pub(crate) fn router(store: Arc<Store>) -> Router {
    Router::new()
        .route("/test-hooks/invitations", post(seed_invitation_route))
        .with_state(store)
}

#[cfg(feature = "test-hooks")]
#[derive(Debug, Deserialize)]
pub(crate) struct SetRepositoryAccessBody {
    pub account_id: Uuid,
    pub repository: String,
    pub allowed: bool,
}

#[cfg(feature = "test-hooks")]
#[derive(Debug, Deserialize)]
pub(crate) struct SetHostCreateAccessBody {
    pub account_id: Uuid,
    pub host: String,
    pub allowed: bool,
}

#[cfg(feature = "test-hooks")]
#[derive(Debug, Deserialize)]
pub(crate) struct RepositoryCreatedBody {
    pub host: String,
    pub repository: String,
}

#[cfg(feature = "test-hooks")]
#[derive(Debug, Serialize)]
pub(crate) struct RepositoryCreatedResponse {
    pub created: bool,
}

#[cfg(feature = "test-hooks")]
pub(crate) async fn set_repository_access_route(
    State(source_control): State<FixtureSourceControlProvider>,
    Json(body): Json<SetRepositoryAccessBody>,
) -> Result<StatusCode, (StatusCode, String)> {
    let repository = RepositoryRef::parse(&body.repository)
        .map_err(|err| (StatusCode::BAD_REQUEST, err.to_string()))?;
    source_control.set_repository_access(AccountId::new(body.account_id), repository, body.allowed);
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(feature = "test-hooks")]
pub(crate) async fn set_host_create_access_route(
    State(source_control): State<FixtureSourceControlProvider>,
    Json(body): Json<SetHostCreateAccessBody>,
) -> Result<StatusCode, (StatusCode, String)> {
    let host = DesignatedHost::parse(&body.host)
        .map_err(|err| (StatusCode::BAD_REQUEST, err.to_string()))?;
    source_control.set_host_reachable(&host, true);
    source_control.set_host_create_access(AccountId::new(body.account_id), &host, body.allowed);
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(feature = "test-hooks")]
pub(crate) async fn repository_created_route(
    State(source_control): State<FixtureSourceControlProvider>,
    Json(body): Json<RepositoryCreatedBody>,
) -> Result<Json<RepositoryCreatedResponse>, (StatusCode, String)> {
    let host = DesignatedHost::parse(&body.host)
        .map_err(|err| (StatusCode::BAD_REQUEST, err.to_string()))?;
    let repository = RepositoryRef::parse(&body.repository)
        .map_err(|err| (StatusCode::BAD_REQUEST, err.to_string()))?;
    Ok(Json(RepositoryCreatedResponse {
        created: source_control.repository_created_at_host(&host, &repository),
    }))
}

#[cfg(feature = "test-hooks")]
pub(crate) fn router_with_source_control(
    store: Arc<Store>,
    source_control: FixtureSourceControlProvider,
) -> Router {
    router(store).merge(
        Router::new()
            .route(
                "/test-hooks/source-control/repository-access",
                post(set_repository_access_route),
            )
            .route(
                "/test-hooks/source-control/host-create-access",
                post(set_host_create_access_route),
            )
            .route(
                "/test-hooks/source-control/repository-created",
                post(repository_created_route),
            )
            .with_state(source_control),
    )
}
