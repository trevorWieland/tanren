//! Organization-member listing route.

use crate::AppState;
use crate::auth::require_authoritative_auth;
use crate::errors::map_organization_app_error;
use crate::organization_tracing::{
    emit_route_auth_denial, emit_route_failure, emit_route_success, organization_route_span,
    record_authenticated_account,
};
use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use tanren_contract::{
    ListOrganizationMembersApiPath, ListOrganizationMembersApiQuery,
    ListOrganizationMembersRequest, ListOrganizationMembersResponse, OrganizationFailureBody,
};
use tower_sessions::Session;

#[utoipa::path(
    get,
    path = "/organizations/{org_id}/members",
    responses(
        (status = 200, body = ListOrganizationMembersResponse, description = "Member list"),
        (status = 401, body = OrganizationFailureBody, description = "auth_required"),
        (status = 403, body = OrganizationFailureBody, description = "permission_denied"),
        (status = 500, body = OrganizationFailureBody, description = "internal_error"),
    ),
    params(
        ListOrganizationMembersApiPath,
        ListOrganizationMembersApiQuery,
    ),
    tag = "organizations",
)]
pub(crate) async fn list_organization_members_route(
    State(state): State<AppState>,
    session: Session,
    Path(path): Path<ListOrganizationMembersApiPath>,
    Query(query): Query<ListOrganizationMembersApiQuery>,
) -> Response {
    let org_id = path.org_id;
    let span = organization_route_span("list_organization_members", Some(org_id));
    let _span_guard = span.enter();
    let auth = match require_authoritative_auth(&state, &session).await {
        Ok(auth) => auth,
        Err(response) => {
            emit_route_auth_denial("list_organization_members", Some(org_id));
            return response;
        }
    };
    record_authenticated_account(&span, auth.0);
    let request = ListOrganizationMembersRequest::from_api_query(auth.1, auth.0, &path, &query);
    match state
        .handlers
        .list_organization_members(state.store.as_ref(), request)
        .await
    {
        Ok(response) => {
            emit_route_success("list_organization_members", auth.0, Some(org_id));
            (StatusCode::OK, Json(response)).into_response()
        }
        Err(err) => {
            emit_route_failure("list_organization_members", auth.0, Some(org_id), &err);
            map_organization_app_error(err)
        }
    }
}
