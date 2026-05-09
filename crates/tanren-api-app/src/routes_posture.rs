//! Deployment-posture HTTP routes and `OpenAPI` schemas.

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use tanren_contract::{
    DeploymentPosture, DeploymentPostureCapabilitySummary, DeploymentPostureScope,
    SetDeploymentPostureRequest, SetDeploymentPostureResponse,
};
use tanren_identity_policy::{AccountId, InstallationId, ProjectId};
use tower_sessions::Session;
use uuid::Uuid;

use crate::AppState;
use crate::cookies::session_account_id;
use crate::errors::{AccountFailureBody, ValidatedJson, map_posture_error};

/// Supported deployment posture with canonical capability summary.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub(crate) struct SupportedDeploymentPostureResponse {
    /// Canonical posture value.
    pub posture: DeploymentPosture,
    /// Canonical capability explanation for this posture.
    pub capability_summary: DeploymentPostureCapabilitySummary,
}

/// Response body for listing supported deployment postures.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub(crate) struct DeploymentPostureListResponse {
    /// Supported posture options.
    pub supported: Vec<SupportedDeploymentPostureResponse>,
}

/// Response body for reading the current posture selection for a scope.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub(crate) struct DeploymentPostureGetResponse {
    /// Current selection for the requested scope, when recorded.
    pub current: Option<SetDeploymentPostureResponse>,
}

/// List every supported deployment posture with capability summary.
#[utoipa::path(
    get,
    path = "/deployment-postures",
    responses(
        (status = 200, body = DeploymentPostureListResponse, description = "Supported posture list"),
    ),
    tag = "posture",
)]
pub(crate) async fn list_deployment_postures_route(
    State(state): State<AppState>,
) -> Json<DeploymentPostureListResponse> {
    let supported = state
        .handlers
        .list_supported_deployment_postures()
        .into_iter()
        .map(|entry| SupportedDeploymentPostureResponse {
            posture: entry.posture,
            capability_summary: entry.capability_summary,
        })
        .collect();
    Json(DeploymentPostureListResponse { supported })
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
        (status = 200, body = DeploymentPostureGetResponse, description = "Current posture for the scope"),
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
        Ok(current) => Json(DeploymentPostureGetResponse { current }).into_response(),
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
    match session_account_id(session).await {
        Ok(Some(account_id)) => Ok(account_id),
        Ok(None) => Err((
            StatusCode::FORBIDDEN,
            Json(AccountFailureBody {
                code: "permission_denied".to_owned(),
                summary: "Sign in before changing deployment posture.".to_owned(),
            }),
        )
            .into_response()),
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

fn parse_scope(scope_kind: &str, scope_id: &str) -> Result<DeploymentPostureScope, Box<Response>> {
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

    let scope = match scope_kind {
        "account" => DeploymentPostureScope::Account {
            account_id: AccountId::from(parsed_uuid),
        },
        "project" => DeploymentPostureScope::Project {
            project_id: ProjectId::from(parsed_uuid),
        },
        "installation" => DeploymentPostureScope::Installation {
            installation_id: InstallationId::from(parsed_uuid),
        },
        other => {
            return Err(Box::new((
                StatusCode::BAD_REQUEST,
                Json(AccountFailureBody {
                    code: "validation_failed".to_owned(),
                    summary: format!(
                        "Invalid scope_kind `{other}`. Supported values: account, project, installation."
                    ),
                }),
            )
                .into_response()));
        }
    };
    Ok(scope)
}
