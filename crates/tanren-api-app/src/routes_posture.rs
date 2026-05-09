//! Deployment-posture HTTP routes and `OpenAPI` schemas.

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use chrono::Utc;
use tanren_contract::{
    CurrentDeploymentPostureResponse, DeploymentPostureFailureReason, DeploymentPostureScope,
    SetDeploymentPostureRequest, SetDeploymentPostureResponse, SupportedDeploymentPosturesResponse,
};
use tanren_identity_policy::{AccountId, InstallationId, ProjectId};
use tower_sessions::Session;
use uuid::Uuid;

use crate::AppState;
use crate::cookies::session_actor;
use crate::errors::{AccountFailureBody, ValidatedJson, map_posture_error};

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
        (status = 400, body = AccountFailureBody, description = "validation_failed"),
        (status = 500, body = AccountFailureBody, description = "internal_error"),
    ),
    tag = "posture",
)]
pub(crate) async fn get_deployment_posture_route(
    State(state): State<AppState>,
    Path((scope_kind, scope_id)): Path<(String, String)>,
) -> Response {
    let parsed_scope = match parse_scope(&scope_kind, &scope_id) {
        Ok(scope) => scope,
        Err(response) => return *response,
    };

    match state
        .handlers
        .deployment_posture(state.store.as_ref(), parsed_scope)
        .await
    {
        Ok(current) => Json(current).into_response(),
        Err(err) => {
            tracing::error!(target: "tanren_api", error = %err, "store error");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(AccountFailureBody {
                    code: "internal_error".to_owned(),
                    summary: "Tanren encountered an internal error.".to_owned(),
                }),
            )
                .into_response()
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
        (status = 400, body = AccountFailureBody, description = "unsupported_posture or validation_failed"),
        (status = 403, body = AccountFailureBody, description = "permission_denied"),
        (status = 404, body = AccountFailureBody, description = "scope_not_found"),
        (status = 500, body = AccountFailureBody, description = "internal_error"),
    ),
    tag = "posture",
)]
pub(crate) async fn set_deployment_posture_route(
    State(state): State<AppState>,
    session: Session,
    ValidatedJson(request): ValidatedJson<SetDeploymentPostureRequest>,
) -> Response {
    let actor = match actor_from_session(&session).await {
        Ok(actor) => actor,
        Err(response) => return response,
    };

    match state
        .handlers
        .set_deployment_posture(state.store.as_ref(), actor, request)
        .await
    {
        Ok(response) => Json(response).into_response(),
        Err(err) => map_posture_error(err),
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
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(AccountFailureBody {
                    code: "internal_error".to_owned(),
                    summary: "Tanren encountered an internal error.".to_owned(),
                }),
            )
                .into_response())
        }
    }
}

fn permission_denied_response() -> Response {
    (
        StatusCode::FORBIDDEN,
        Json(AccountFailureBody {
            code: DeploymentPostureFailureReason::PermissionDenied
                .code()
                .to_owned(),
            summary: DeploymentPostureFailureReason::PermissionDenied
                .summary()
                .to_owned(),
        }),
    )
        .into_response()
}

fn parse_scope(scope_kind: &str, scope_id: &str) -> Result<DeploymentPostureScope, Box<Response>> {
    let scope_kind = ScopeKind::parse(scope_kind)?;
    let parsed_uuid = Uuid::parse_str(scope_id).map_err(|err| {
        Box::new(
            (
                StatusCode::BAD_REQUEST,
                Json(AccountFailureBody {
                    code: "validation_failed".to_owned(),
                    summary: format!("Invalid scope_id `{scope_id}`: {err}"),
                }),
            )
                .into_response(),
        )
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
            other => Err(Box::new(
                (
                    StatusCode::BAD_REQUEST,
                    Json(AccountFailureBody {
                        code: "validation_failed".to_owned(),
                        summary: format!(
                            "Invalid scope_kind `{other}`. Supported values: account, project, installation."
                        ),
                    }),
                )
                    .into_response(),
            )),
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
