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
use tanren_identity_policy::{
    AccountId, Email, Identifier, InvitationToken, OrgId, OrganizationPermission,
};
use tanren_store::{AccountStore, CreateInvitation, NewInvitation, Store};
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

/// Request body for `POST /test-hooks/memberships`.
#[derive(Debug, Deserialize)]
pub(crate) struct SeedMembershipBody {
    /// Account to add to the organization.
    pub account_id: Uuid,
    /// Target organization.
    pub org_id: Uuid,
    /// Permission names to grant (e.g. `["admin"]` or `["member"]`).
    pub permissions: Vec<String>,
}

pub(crate) async fn seed_membership_route(
    State(store): State<Arc<Store>>,
    Json(body): Json<SeedMembershipBody>,
) -> Result<StatusCode, (StatusCode, String)> {
    let account_id = AccountId::new(body.account_id);
    let org_id = OrgId::new(body.org_id);
    let permissions = body
        .permissions
        .iter()
        .map(|s| OrganizationPermission::parse(s))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    store
        .insert_membership(account_id, org_id, permissions, Utc::now())
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(StatusCode::CREATED)
}

/// Request body for `POST /test-hooks/org-invitations`.
#[derive(Debug, Deserialize)]
pub(crate) struct SeedOrgInvitationBody {
    /// Opaque invitation token.
    pub token: String,
    /// Target organization.
    pub org_id: Uuid,
    /// Recipient identifier (email).
    pub recipient_identifier: String,
    /// Permission names to grant on acceptance.
    pub permissions: Vec<String>,
    /// Account id of the inviting admin.
    pub created_by_account_id: Uuid,
    /// Expiry instant.
    pub expires_at: DateTime<Utc>,
}

pub(crate) async fn seed_org_invitation_route(
    State(store): State<Arc<Store>>,
    Json(body): Json<SeedOrgInvitationBody>,
) -> Result<StatusCode, (StatusCode, String)> {
    let token = InvitationToken::parse(&body.token)
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    let inviting_org_id = OrgId::new(body.org_id);
    let recipient_identifier = Identifier::parse(&body.recipient_identifier)
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    let granted_permissions = body
        .permissions
        .iter()
        .map(|s| OrganizationPermission::parse(s))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    let created_by_account_id = AccountId::new(body.created_by_account_id);
    let now = Utc::now();
    store
        .seed_organization_invitation(CreateInvitation {
            token,
            inviting_org_id,
            recipient_identifier,
            granted_permissions,
            created_by_account_id,
            created_at: now,
            expires_at: body.expires_at,
        })
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(StatusCode::CREATED)
}

/// Query parameters for `GET /test-hooks/membership-permissions`.
#[derive(Debug, Deserialize)]
pub(crate) struct MembershipPermissionsQuery {
    /// Account id to look up.
    pub account_id: Uuid,
    /// Organization id to look up.
    pub org_id: Uuid,
}

/// Response body for `GET /test-hooks/membership-permissions`.
#[derive(Debug, Serialize)]
pub(crate) struct MembershipPermissionsResponse {
    /// Permission names the account holds in the organization.
    pub permissions: Vec<String>,
}

pub(crate) async fn membership_permissions_route(
    State(store): State<Arc<Store>>,
    Query(query): Query<MembershipPermissionsQuery>,
) -> Result<Json<MembershipPermissionsResponse>, (StatusCode, String)> {
    let account_id = AccountId::new(query.account_id);
    let org_id = OrgId::new(query.org_id);
    let permissions = store
        .find_organization_permissions(account_id, org_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(MembershipPermissionsResponse {
        permissions: permissions.iter().map(|p| p.as_str().to_owned()).collect(),
    }))
}

/// Query parameters for `GET /test-hooks/account-by-email`.
#[derive(Debug, Deserialize)]
pub(crate) struct AccountByEmailQuery {
    /// Email address to look up.
    pub email: String,
}

/// Response body for `GET /test-hooks/account-by-email`.
#[derive(Debug, Serialize)]
pub(crate) struct AccountByEmailResponse {
    /// Stable account id.
    pub account_id: Uuid,
    /// Owning organization id, if any.
    pub org_id: Option<Uuid>,
}

pub(crate) async fn account_by_email_route(
    State(store): State<Arc<Store>>,
    Query(query): Query<AccountByEmailQuery>,
) -> Result<Json<AccountByEmailResponse>, (StatusCode, String)> {
    let email = Email::parse(&query.email).map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    let record = store
        .find_account_by_email(&email)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or_else(|| (StatusCode::NOT_FOUND, "account not found".to_owned()))?;
    Ok(Json(AccountByEmailResponse {
        account_id: record.id.as_uuid(),
        org_id: record.org_id.map(OrgId::as_uuid),
    }))
}

/// Build the `/test-hooks/*` router. The state is the shared
/// `Arc<Store>` already constructed by `build_app` / `build_app_with_store`.
pub(crate) fn router(store: Arc<Store>) -> Router {
    Router::new()
        .route("/test-hooks/invitations", post(seed_invitation_route))
        .route(
            "/test-hooks/org-invitations",
            post(seed_org_invitation_route),
        )
        .route("/test-hooks/memberships", post(seed_membership_route))
        .route(
            "/test-hooks/membership-permissions",
            get(membership_permissions_route),
        )
        .route("/test-hooks/account-by-email", get(account_by_email_route))
        .with_state(store)
}
