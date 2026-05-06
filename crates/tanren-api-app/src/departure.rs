//! Membership departure route handlers: voluntary leave and
//! admin-initiated removal.
//!
//! Split out of `routes.rs` so that file stays under the workspace
//! 500-line line-budget after the formatter expands the utoipa
//! annotations.

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use tanren_app_services::AppServiceError;
use tanren_contract::{
    AccountFailureReason, LeaveOrganizationRequest, MembershipDepartureResponse,
    RemoveMemberRequest,
};
use tanren_identity_policy::{AccountId, OrgId};
use tower_sessions::Session;
use uuid::Uuid;

use crate::AppState;
use crate::cookies::read_cookie_session;
use crate::errors::{AccountFailureBody, ValidatedJson, map_app_error};

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub(crate) struct DepartureBody {
    #[serde(default)]
    pub acknowledge_in_flight_work: bool,
}

#[utoipa::path(
    post,
    path = "/organizations/{org_id}/leave",
    params(("org_id" = String, Path, description = "Organization to leave")),
    request_body = DepartureBody,
    responses(
        (status = 200, body = MembershipDepartureResponse, description = "Preview or completed"),
        (status = 401, body = AccountFailureBody, description = "unauthenticated"),
        (status = 403, body = AccountFailureBody, description = "last_admin_permission_holder"),
        (status = 404, body = AccountFailureBody, description = "not_org_member"),
    ),
    tag = "accounts",
)]
pub(crate) async fn leave_organization_route(
    State(state): State<AppState>,
    session: Session,
    Path(org_id_str): Path<String>,
    ValidatedJson(body): ValidatedJson<DepartureBody>,
) -> Response {
    let Ok(account_id) = read_cookie_session(&session).await else {
        return map_app_error(AppServiceError::Account(
            AccountFailureReason::Unauthenticated,
        ));
    };
    let org_id = match parse_path_uuid(&org_id_str) {
        Ok(id) => OrgId::new(id),
        Err(r) => return *r,
    };
    match state
        .handlers
        .leave_organization(
            state.store.as_ref(),
            account_id,
            LeaveOrganizationRequest { org_id },
            body.acknowledge_in_flight_work,
        )
        .await
    {
        Ok(r) => (StatusCode::OK, Json(r)).into_response(),
        Err(e) => map_app_error(e),
    }
}

#[utoipa::path(
    post,
    path = "/organizations/{org_id}/members/{member_account_id}/remove",
    params(
        ("org_id" = String, Path, description = "Organization"),
        ("member_account_id" = String, Path, description = "Account to remove"),
    ),
    request_body = DepartureBody,
    responses(
        (status = 200, body = MembershipDepartureResponse, description = "Preview or completed"),
        (status = 401, body = AccountFailureBody, description = "unauthenticated"),
        (status = 403, body = AccountFailureBody, description = "permission_denied or last_admin"),
        (status = 404, body = AccountFailureBody, description = "not_org_member"),
    ),
    tag = "accounts",
)]
pub(crate) async fn remove_member_route(
    State(state): State<AppState>,
    session: Session,
    Path((org_id_str, member_str)): Path<(String, String)>,
    ValidatedJson(body): ValidatedJson<DepartureBody>,
) -> Response {
    let Ok(actor) = read_cookie_session(&session).await else {
        return map_app_error(AppServiceError::Account(
            AccountFailureReason::Unauthenticated,
        ));
    };
    let org_id = match parse_path_uuid(&org_id_str) {
        Ok(id) => OrgId::new(id),
        Err(r) => return *r,
    };
    let member_account_id = match parse_path_uuid(&member_str) {
        Ok(id) => AccountId::new(id),
        Err(r) => return *r,
    };
    match state
        .handlers
        .remove_member(
            state.store.as_ref(),
            actor,
            RemoveMemberRequest {
                org_id,
                member_account_id,
            },
            body.acknowledge_in_flight_work,
        )
        .await
    {
        Ok(r) => (StatusCode::OK, Json(r)).into_response(),
        Err(e) => map_app_error(e),
    }
}

fn parse_path_uuid(raw: &str) -> Result<Uuid, Box<Response>> {
    Uuid::parse_str(raw).map_err(|e| {
        Box::new(
            (
                StatusCode::BAD_REQUEST,
                Json(AccountFailureBody {
                    code: "validation_failed".to_owned(),
                    summary: e.to_string(),
                }),
            )
                .into_response(),
        )
    })
}
