//! Deployment-posture HTTP routes and `OpenAPI` schemas.

use axum::Json;
use axum::extract::rejection::JsonRejection;
use axum::extract::{Path, State};
use axum::response::{IntoResponse, Response};
use chrono::Utc;
use tanren_contract::{
    CurrentDeploymentPostureResponse, DeploymentPostureFailureBody, DeploymentPostureFailureReason,
    DeploymentPostureScope, RawSetDeploymentPostureRequest, SetDeploymentPostureRequest,
    SetDeploymentPostureResponse, SupportedDeploymentPosturesResponse,
};
use tanren_identity_policy::{AccountId, InstallationId, ProjectId};
use tower_sessions::Session;
use uuid::Uuid;

use crate::AppState;
use crate::cookies::session_actor;
use crate::errors::{
    map_posture_error, posture_failure_body_response, posture_failure_response,
    posture_internal_error_response,
};

/// List every supported deployment posture with capability summary.
#[utoipa::path(
    get,
    path = "/deployment-postures",
    responses(
        (status = 200, body = SupportedDeploymentPosturesResponse, description = "Supported posture list"),
    ),
    tag = "posture",
)]
pub(crate) async fn list_deployment_postures_route(
    State(state): State<AppState>,
) -> Json<SupportedDeploymentPosturesResponse> {
    Json(state.handlers.list_supported_deployment_postures())
}

/// Read the currently selected posture for a scope.
#[utoipa::path(
    get,
    path = "/deployment-postures/{scope_kind}/{scope_id}",
    params(
        ("scope_kind" = String, Path, description = "Scope discriminator: account | project | installation"),
        ("scope_id" = String, Path, description = "Scope identifier (UUID)"),
    ),
    responses(
        (status = 200, body = CurrentDeploymentPostureResponse, description = "Current posture for the scope"),
        (status = 403, body = DeploymentPostureFailureBody, description = "permission_denied"),
        (status = 404, body = DeploymentPostureFailureBody, description = "scope_not_found"),
        (status = 400, body = DeploymentPostureFailureBody, description = "validation_failed"),
        (status = 500, body = DeploymentPostureFailureBody, description = "internal_error"),
    ),
    tag = "posture",
)]
pub(crate) async fn get_deployment_posture_route(
    State(state): State<AppState>,
    session: Session,
    Path((scope_kind, scope_id)): Path<(String, String)>,
) -> Response {
    let actor = match actor_from_session(&session).await {
        Ok(actor) => actor,
        Err(response) => return response,
    };
    let parsed_scope = match parse_scope(&scope_kind, &scope_id) {
        Ok(scope) => scope,
        Err(response) => return *response,
    };

    match state
        .handlers
        .deployment_posture(state.store.as_ref(), actor, parsed_scope)
        .await
    {
        Ok(current) => Json(current).into_response(),
        Err(err) => {
            if let Some(failure) = err.contract_failure() {
                tracing::warn!(
                    target: "tanren_api",
                    actor_id = %actor,
                    scope = ?parsed_scope,
                    failure_reason = failure.reason.code(),
                    "deployment posture read denied"
                );
                return map_posture_error(&err);
            }
            tracing::error!(
                target: "tanren_api",
                error = %err,
                actor_id = %actor,
                scope = ?parsed_scope,
                "deployment posture read store error"
            );
            posture_internal_error_response()
        }
    }
}

/// Set or update deployment posture for a scope.
#[utoipa::path(
    post,
    path = "/deployment-postures",
    request_body = SetDeploymentPostureRequest,
    responses(
        (status = 200, body = SetDeploymentPostureResponse, description = "Posture set"),
        (status = 400, body = DeploymentPostureFailureBody, description = "unsupported_posture or validation_failed"),
        (status = 403, body = DeploymentPostureFailureBody, description = "permission_denied"),
        (status = 404, body = DeploymentPostureFailureBody, description = "scope_not_found"),
        (status = 500, body = DeploymentPostureFailureBody, description = "internal_error"),
    ),
    tag = "posture",
)]
pub(crate) async fn set_deployment_posture_route(
    State(state): State<AppState>,
    session: Session,
    request: Result<Json<RawSetDeploymentPostureRequest>, JsonRejection>,
) -> Response {
    let actor = match actor_from_session(&session).await {
        Ok(actor) => actor,
        Err(response) => return response,
    };
    let raw_request = match request {
        Ok(Json(request)) => request,
        Err(rejection) => {
            tracing::warn!(
                target: "tanren_api",
                error = %rejection,
                "deployment posture request rejected at transport decode"
            );
            return posture_failure_response(
                DeploymentPostureFailureReason::ValidationFailed,
                None,
            );
        }
    };
    let request = match SetDeploymentPostureRequest::try_from(raw_request) {
        Ok(request) => request,
        Err(failure) => return posture_failure_body_response(failure.render()),
    };

    match state
        .handlers
        .set_deployment_posture(state.store.as_ref(), actor, request)
        .await
    {
        Ok(response) => Json(response).into_response(),
        Err(err) => map_posture_error(&err),
    }
}

async fn actor_from_session(session: &Session) -> Result<AccountId, Response> {
    match session_actor(session).await {
        Ok(Some(actor)) => {
            if actor.expires_at <= Utc::now() {
                if let Err(err) = session.flush().await {
                    tracing::error!(target: "tanren_api", error = %err, "session flush");
                }
                return Err(permission_denied_response());
            }
            Ok(actor.account_id)
        }
        Ok(None) => Err(permission_denied_response()),
        Err(err) => {
            tracing::error!(target: "tanren_api", error = %err, "session read");
            Err(posture_internal_error_response())
        }
    }
}

fn permission_denied_response() -> Response {
    posture_failure_response(DeploymentPostureFailureReason::PermissionDenied, None)
}

fn parse_scope(scope_kind: &str, scope_id: &str) -> Result<DeploymentPostureScope, Box<Response>> {
    let scope_kind = ScopeKind::parse(scope_kind)?;
    let parsed_uuid = Uuid::parse_str(scope_id).map_err(|err| {
        tracing::warn!(
            target: "tanren_api",
            raw_scope_id = scope_id,
            parse_error = %err,
            "invalid deployment posture scope id"
        );
        Box::new(posture_failure_response(
            DeploymentPostureFailureReason::ValidationFailed,
            None,
        ))
    })?;
    Ok(scope_kind.into_scope(parsed_uuid))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScopeKind {
    Account,
    Project,
    Installation,
}

impl ScopeKind {
    fn parse(raw: &str) -> Result<Self, Box<Response>> {
        match raw {
            "account" => Ok(Self::Account),
            "project" => Ok(Self::Project),
            "installation" => Ok(Self::Installation),
            other => {
                tracing::warn!(
                    target: "tanren_api",
                    raw_scope_kind = other,
                    "invalid deployment posture scope kind"
                );
                Err(Box::new(posture_failure_response(
                    DeploymentPostureFailureReason::ValidationFailed,
                    None,
                )))
            }
        }
    }

    fn into_scope(self, id: Uuid) -> DeploymentPostureScope {
        match self {
            Self::Account => DeploymentPostureScope::Account {
                account_id: AccountId::from(id),
            },
            Self::Project => DeploymentPostureScope::Project {
                project_id: ProjectId::from(id),
            },
            Self::Installation => DeploymentPostureScope::Installation {
                installation_id: InstallationId::from(id),
            },
        }
    }
}
