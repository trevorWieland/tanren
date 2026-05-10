use axum::Json;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use tanren_contract::{
    ApplyRoleRequest, ApplyRoleResponse, CreateRoleRequest, CreateRoleResponse, DeleteRoleRequest,
    DeleteRoleResponse, EditRoleRequest, EditRoleResponse, PermissionCheckRequest,
    PermissionCheckResponse, RoleActor, RoleFailureBody, RoleFailureReason, RoleReadModelRequest,
    RoleReadModelResponse,
};
use tower_sessions::Session;

use crate::AppState;
use crate::cookies::{CSRF_HEADER_NAME, session_account_id, session_csrf_token};
use crate::errors::{ValidatedJson, map_role_error};
use crate::routes::RoleCapabilitiesResponse;

/// Create a role template.
#[utoipa::path(
    post,
    path = "/roles",
    request_body = CreateRoleRequest,
    responses(
        (status = 201, body = CreateRoleResponse, description = "Role template created"),
        (status = 400, body = RoleFailureBody, description = "validation_failed"),
        (status = 403, body = RoleFailureBody, description = "permission_denied"),
        (status = 409, body = RoleFailureBody, description = "conflict"),
    ),
    tag = "roles",
)]
pub(crate) async fn create_role_route(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
    ValidatedJson(request): ValidatedJson<CreateRoleRequest>,
) -> Response {
    let actor = match role_actor_from_session(&session).await {
        Ok(actor) => actor,
        Err(response) => return response,
    };
    if let Err(response) = enforce_csrf(&session, &headers).await {
        return response;
    }
    match state
        .handlers
        .create_role(state.store.as_ref(), actor, request)
        .await
    {
        Ok(response) => (StatusCode::CREATED, Json(response)).into_response(),
        Err(err) => map_role_error(err),
    }
}

/// Edit an existing role template.
#[utoipa::path(
    post,
    path = "/roles/edit",
    request_body = EditRoleRequest,
    responses(
        (status = 200, body = EditRoleResponse, description = "Role template updated"),
        (status = 400, body = RoleFailureBody, description = "validation_failed or role_as_principal_rejected"),
        (status = 403, body = RoleFailureBody, description = "permission_denied"),
        (status = 404, body = RoleFailureBody, description = "not_found"),
        (status = 409, body = RoleFailureBody, description = "conflict"),
    ),
    tag = "roles",
)]
pub(crate) async fn edit_role_route(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
    ValidatedJson(request): ValidatedJson<EditRoleRequest>,
) -> Response {
    let actor = match role_actor_from_session(&session).await {
        Ok(actor) => actor,
        Err(response) => return response,
    };
    if let Err(response) = enforce_csrf(&session, &headers).await {
        return response;
    }
    match state
        .handlers
        .edit_role(state.store.as_ref(), actor, request)
        .await
    {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(err) => map_role_error(err),
    }
}

/// Delete an existing role template.
#[utoipa::path(
    post,
    path = "/roles/delete",
    request_body = DeleteRoleRequest,
    responses(
        (status = 200, body = DeleteRoleResponse, description = "Role template deleted"),
        (status = 400, body = RoleFailureBody, description = "validation_failed"),
        (status = 403, body = RoleFailureBody, description = "permission_denied"),
        (status = 404, body = RoleFailureBody, description = "not_found"),
    ),
    tag = "roles",
)]
pub(crate) async fn delete_role_route(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
    ValidatedJson(request): ValidatedJson<DeleteRoleRequest>,
) -> Response {
    let actor = match role_actor_from_session(&session).await {
        Ok(actor) => actor,
        Err(response) => return response,
    };
    if let Err(response) = enforce_csrf(&session, &headers).await {
        return response;
    }
    match state
        .handlers
        .delete_role(state.store.as_ref(), actor, request)
        .await
    {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(err) => map_role_error(err),
    }
}

/// Apply a role template to a principal.
#[utoipa::path(
    post,
    path = "/roles/apply",
    request_body = ApplyRoleRequest,
    responses(
        (status = 200, body = ApplyRoleResponse, description = "Role applied"),
        (status = 400, body = RoleFailureBody, description = "validation_failed or role_as_principal_rejected"),
        (status = 403, body = RoleFailureBody, description = "permission_denied"),
        (status = 404, body = RoleFailureBody, description = "not_found"),
        (status = 409, body = RoleFailureBody, description = "conflict"),
    ),
    tag = "roles",
)]
pub(crate) async fn apply_role_route(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
    ValidatedJson(request): ValidatedJson<ApplyRoleRequest>,
) -> Response {
    let actor = match role_actor_from_session(&session).await {
        Ok(actor) => actor,
        Err(response) => return response,
    };
    if let Err(response) = enforce_csrf(&session, &headers).await {
        return response;
    }
    match state
        .handlers
        .apply_role(state.store.as_ref(), actor, request)
        .await
    {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(err) => map_role_error(err),
    }
}

/// Check whether a principal has a permission in a scope.
#[utoipa::path(
    post,
    path = "/permissions/check",
    request_body = PermissionCheckRequest,
    responses(
        (status = 200, body = PermissionCheckResponse, description = "Permission check response"),
        (status = 400, body = RoleFailureBody, description = "validation_failed or role_as_principal_rejected"),
        (status = 403, body = RoleFailureBody, description = "permission_denied"),
    ),
    tag = "roles",
)]
pub(crate) async fn permission_check_route(
    State(state): State<AppState>,
    session: Session,
    ValidatedJson(request): ValidatedJson<PermissionCheckRequest>,
) -> Response {
    let actor = match role_actor_from_session(&session).await {
        Ok(actor) => actor,
        Err(response) => return response,
    };
    match state
        .handlers
        .check_permission(state.store.as_ref(), actor, request)
        .await
    {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(err) => map_role_error(err),
    }
}

/// Read paged role-template and direct-grant state.
#[utoipa::path(
    post,
    path = "/roles/read-model",
    request_body = RoleReadModelRequest,
    responses(
        (status = 200, body = RoleReadModelResponse, description = "Role read-model snapshot"),
        (status = 400, body = RoleFailureBody, description = "validation_failed or role_as_principal_rejected"),
        (status = 403, body = RoleFailureBody, description = "permission_denied"),
        (status = 404, body = RoleFailureBody, description = "not_found"),
    ),
    tag = "roles",
)]
pub(crate) async fn role_read_model_route(
    State(state): State<AppState>,
    session: Session,
    ValidatedJson(request): ValidatedJson<RoleReadModelRequest>,
) -> Response {
    let actor = match role_actor_from_session(&session).await {
        Ok(actor) => actor,
        Err(response) => return response,
    };
    match state
        .handlers
        .read_role_model(state.store.as_ref(), actor, request)
        .await
    {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(err) => map_role_error(err),
    }
}

/// Discover role-administration capabilities for the authenticated actor.
#[utoipa::path(
    get,
    path = "/roles/capabilities",
    responses(
        (status = 200, body = RoleCapabilitiesResponse, description = "Role capability metadata"),
        (status = 403, body = RoleFailureBody, description = "permission_denied"),
    ),
    tag = "roles",
)]
pub(crate) async fn role_capabilities_route(
    State(state): State<AppState>,
    session: Session,
) -> Response {
    let actor = match role_actor_from_session(&session).await {
        Ok(actor) => actor,
        Err(response) => return response,
    };
    let csrf_token = match session_csrf_token(&session).await {
        Ok(Some(token)) => token,
        Ok(None) => {
            return role_permission_denied("A CSRF token is required for role administration.");
        }
        Err(err) => {
            tracing::error!(target: "tanren_api", error = %err, "csrf token read");
            return internal_role_error();
        }
    };
    match state
        .handlers
        .role_admin_capabilities(state.store.as_ref(), actor)
        .await
    {
        Ok(capabilities) => (
            StatusCode::OK,
            Json(RoleCapabilitiesResponse {
                capabilities,
                csrf_token,
            }),
        )
            .into_response(),
        Err(err) => map_role_error(err),
    }
}

async fn role_actor_from_session(session: &Session) -> Result<RoleActor, Response> {
    match session_account_id(session).await {
        Ok(Some(account_id)) => Ok(RoleActor { account_id }),
        Ok(None) => Err(role_permission_denied(
            "Authentication is required for role administration.",
        )),
        Err(err) => {
            tracing::error!(target: "tanren_api", error = %err, "session read");
            Err(internal_role_error())
        }
    }
}

async fn enforce_csrf(session: &Session, headers: &HeaderMap) -> Result<(), Response> {
    let expected = match session_csrf_token(session).await {
        Ok(Some(token)) => token,
        Ok(None) => {
            return Err(role_permission_denied(
                "A CSRF token is required for role mutation requests.",
            ));
        }
        Err(err) => {
            tracing::error!(target: "tanren_api", error = %err, "csrf token read");
            return Err(internal_role_error());
        }
    };
    let presented = headers
        .get(CSRF_HEADER_NAME)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty());
    if presented.is_some_and(|token| token == expected) {
        return Ok(());
    }
    Err(role_permission_denied(
        "The CSRF token is missing or invalid for this role mutation request.",
    ))
}

fn role_permission_denied(summary: &str) -> Response {
    (
        StatusCode::FORBIDDEN,
        Json(RoleFailureBody {
            code: RoleFailureReason::PermissionDenied,
            summary: summary.to_owned(),
        }),
    )
        .into_response()
}

fn internal_role_error() -> Response {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(RoleFailureBody {
            code: RoleFailureReason::InternalError,
            summary: "Tanren encountered an internal error.".to_owned(),
        }),
    )
        .into_response()
}
