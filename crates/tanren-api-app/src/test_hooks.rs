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
use axum::extract::Query;
use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::{get, post};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use tanren_identity_policy::{AccountId, Email, InvitationToken, OrgId, ProjectId};
use tanren_store::{AccountStore, NewInvitation, NewOrganization, NewProject, Store};
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

/// Single organization entry inside [`SeedFixturesBody`].
#[derive(Debug, Deserialize)]
pub(crate) struct FixtureOrganization {
    #[serde(default)]
    pub org_id: Option<Uuid>,
    pub name: String,
}

/// Single membership entry inside [`SeedFixturesBody`].
#[derive(Debug, Deserialize)]
pub(crate) struct FixtureMembership {
    pub account_id: Uuid,
    pub org_id: Uuid,
}

/// Single project entry inside [`SeedFixturesBody`].
#[derive(Debug, Deserialize)]
pub(crate) struct FixtureProject {
    #[serde(default)]
    pub project_id: Option<Uuid>,
    pub org_id: Uuid,
    pub name: String,
}

/// Request body for `POST /test-hooks/seed-fixtures`.
///
/// Accepts organizations, memberships, and projects in one payload so
/// the Playwright BDD runner avoids per-entity HTTP round trips.
#[derive(Debug, Deserialize)]
pub(crate) struct SeedFixturesBody {
    #[serde(default)]
    pub organizations: Vec<FixtureOrganization>,
    #[serde(default)]
    pub memberships: Vec<FixtureMembership>,
    #[serde(default)]
    pub projects: Vec<FixtureProject>,
}

pub(crate) async fn seed_fixtures_route(
    State(store): State<Arc<Store>>,
    Json(body): Json<SeedFixturesBody>,
) -> Result<StatusCode, (StatusCode, String)> {
    let now = Utc::now();

    for org in &body.organizations {
        let org_id = org.org_id.map_or_else(OrgId::fresh, OrgId::new);
        store
            .seed_organization(NewOrganization {
                id: org_id,
                name: org.name.clone(),
                created_at: now,
            })
            .await
            .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?;
    }

    for membership in &body.memberships {
        store
            .seed_membership(
                AccountId::new(membership.account_id),
                OrgId::new(membership.org_id),
                now,
            )
            .await
            .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?;
    }

    for project in &body.projects {
        let project_id = project
            .project_id
            .map_or_else(ProjectId::fresh, ProjectId::new);
        store
            .seed_project(NewProject {
                id: project_id,
                org_id: OrgId::new(project.org_id),
                name: project.name.clone(),
                created_at: now,
            })
            .await
            .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?;
    }

    Ok(StatusCode::CREATED)
}

/// Query parameters for `GET /test-hooks/accounts`.
#[derive(Debug, Deserialize)]
pub(crate) struct LookupAccountQuery {
    pub email: String,
}

/// Response body for `GET /test-hooks/accounts`.
#[derive(Debug, serde::Serialize)]
pub(crate) struct AccountLookupResponse {
    pub id: Uuid,
}

pub(crate) async fn lookup_account_route(
    State(store): State<Arc<Store>>,
    Query(query): Query<LookupAccountQuery>,
) -> Result<Json<AccountLookupResponse>, (StatusCode, String)> {
    let email =
        Email::parse(&query.email).map_err(|err| (StatusCode::BAD_REQUEST, err.to_string()))?;
    let record = store
        .find_account_by_email(&email)
        .await
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?;
    let account = record.ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            format!("no account for {}", query.email),
        )
    })?;
    Ok(Json(AccountLookupResponse {
        id: account.id.as_uuid(),
    }))
}

/// Build the `/test-hooks/*` router. The state is the shared
/// `Arc<Store>` already constructed by `build_app` / `build_app_with_store`.
pub(crate) fn router(store: Arc<Store>) -> Router {
    Router::new()
        .route("/test-hooks/accounts", get(lookup_account_route))
        .route("/test-hooks/invitations", post(seed_invitation_route))
        .route("/test-hooks/seed-fixtures", post(seed_fixtures_route))
        .with_state(store)
}
