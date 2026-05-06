//! Organization invitation HTTP route handlers.

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use tanren_contract::{
    CreateOrgInvitationRequest, CreateOrgInvitationResponse, ListOrgInvitationsResponse,
    RevokeOrgInvitationRequest, RevokeOrgInvitationResponse,
};
use tanren_identity_policy::{Identifier, InvitationToken, OrgId, OrganizationPermission};
use tower_sessions::Session;
use uuid::Uuid;

use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::AppState;
use crate::cookies::read_session_account_id;
use crate::errors::{AccountFailureBody, ValidatedJson, map_app_error};

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct CreateOrgInvitationBody {
    pub recipient_identifier: Identifier,
    pub permissions: Vec<OrganizationPermission>,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct RecipientInvitationsQuery {
    pub recipient_identifier: String,
}

fn validation_error(summary: &str) -> Response {
    (
        StatusCode::BAD_REQUEST,
        Json(AccountFailureBody {
            code: "validation_failed".to_owned(),
            summary: summary.to_owned(),
        }),
    )
        .into_response()
}

#[utoipa::path(
    post,
    path = "/organizations/{org_id}/invitations",
    request_body = CreateOrgInvitationBody,
    params(("org_id" = String, Path, description = "Organization ID")),
    responses(
        (status = 201, body = CreateOrgInvitationResponse, description = "Invitation created"),
        (status = 401, body = AccountFailureBody, description = "unauthenticated"),
        (status = 403, body = AccountFailureBody, description = "permission_denied"),
        (status = 422, body = AccountFailureBody, description = "personal_context"),
    ),
    tag = "organizations",
)]
pub(crate) async fn create_org_invitation_route(
    State(state): State<AppState>,
    session: Session,
    Path(org_id): Path<String>,
    ValidatedJson(body): ValidatedJson<CreateOrgInvitationBody>,
) -> Response {
    let account_id = match read_session_account_id(&session).await {
        Ok(id) => id,
        Err(r) => return r,
    };
    let Ok(org_uuid) = Uuid::parse_str(&org_id) else {
        return validation_error("Invalid organization ID.");
    };
    let org_id = OrgId::new(org_uuid);
    let request = CreateOrgInvitationRequest {
        org_id,
        recipient_identifier: body.recipient_identifier,
        permissions: body.permissions,
        expires_at: body.expires_at,
    };
    match state
        .handlers
        .create_invitation(state.store.as_ref(), account_id, Some(org_id), request)
        .await
    {
        Ok(response) => (StatusCode::CREATED, Json(response)).into_response(),
        Err(err) => map_app_error(err),
    }
}

#[utoipa::path(
    get,
    path = "/organizations/{org_id}/invitations",
    params(("org_id" = String, Path, description = "Organization ID")),
    responses(
        (status = 200, body = ListOrgInvitationsResponse, description = "Invitation list"),
        (status = 401, body = AccountFailureBody, description = "unauthenticated"),
        (status = 403, body = AccountFailureBody, description = "permission_denied"),
    ),
    tag = "organizations",
)]
pub(crate) async fn list_org_invitations_route(
    State(state): State<AppState>,
    session: Session,
    Path(org_id): Path<String>,
) -> Response {
    let account_id = match read_session_account_id(&session).await {
        Ok(id) => id,
        Err(r) => return r,
    };
    let Ok(org_uuid) = Uuid::parse_str(&org_id) else {
        return validation_error("Invalid organization ID.");
    };
    let org_id = OrgId::new(org_uuid);
    match state
        .handlers
        .list_org_invitations(state.store.as_ref(), account_id, org_id)
        .await
    {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(err) => map_app_error(err),
    }
}

#[utoipa::path(
    get,
    path = "/invitations",
    params(("recipient_identifier" = String, Query, description = "Recipient identifier")),
    responses(
        (status = 200, body = ListOrgInvitationsResponse, description = "Pending invitations"),
        (status = 401, body = AccountFailureBody, description = "unauthenticated"),
    ),
    tag = "invitations",
)]
pub(crate) async fn list_recipient_invitations_route(
    State(state): State<AppState>,
    session: Session,
    Query(params): Query<RecipientInvitationsQuery>,
) -> Response {
    let _account_id = match read_session_account_id(&session).await {
        Ok(id) => id,
        Err(r) => return r,
    };
    let Ok(identifier) = Identifier::parse(&params.recipient_identifier) else {
        return validation_error("Invalid recipient identifier.");
    };
    match state
        .handlers
        .list_recipient_invitations(state.store.as_ref(), &identifier)
        .await
    {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(err) => map_app_error(err),
    }
}

#[utoipa::path(
    post,
    path = "/organizations/{org_id}/invitations/{token}/revoke",
    params(
        ("org_id" = String, Path, description = "Organization ID"),
        ("token" = String, Path, description = "Invitation token to revoke"),
    ),
    responses(
        (status = 200, body = RevokeOrgInvitationResponse, description = "Invitation revoked"),
        (status = 401, body = AccountFailureBody, description = "unauthenticated"),
        (status = 403, body = AccountFailureBody, description = "permission_denied"),
        (status = 410, body = AccountFailureBody, description = "invitation_revoked or invitation_already_consumed"),
    ),
    tag = "organizations",
)]
pub(crate) async fn revoke_org_invitation_route(
    State(state): State<AppState>,
    session: Session,
    Path((org_id_str, token_str)): Path<(String, String)>,
) -> Response {
    let account_id = match read_session_account_id(&session).await {
        Ok(id) => id,
        Err(r) => return r,
    };
    let Ok(org_uuid) = Uuid::parse_str(&org_id_str) else {
        return validation_error("Invalid organization ID.");
    };
    let org_id = OrgId::new(org_uuid);
    let Ok(token) = InvitationToken::parse(&token_str) else {
        return validation_error("Invalid invitation token.");
    };
    let request = RevokeOrgInvitationRequest { org_id, token };
    match state
        .handlers
        .revoke_invitation(state.store.as_ref(), account_id, Some(org_id), request)
        .await
    {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(err) => map_app_error(err),
    }
}

pub(crate) fn build_invitation_routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(create_org_invitation_route))
        .routes(routes!(list_org_invitations_route))
        .routes(routes!(list_recipient_invitations_route))
        .routes(routes!(revoke_org_invitation_route))
}
