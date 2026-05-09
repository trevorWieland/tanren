//! Axum route handlers and `OpenAPI` annotations.

use crate::AppState;
use crate::cookies::{SessionWrite, install_cookie_session, session_account};
use crate::errors::{
    AccountFailureBody, ProjectFailureBody, ValidatedJson, auth_required, map_app_error,
    session_install_error,
};
use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use secrecy::SecretString;
use serde::{Deserialize, Serialize};
use tanren_app_services::AppServiceError;
use tanren_app_services::Handlers;
use tanren_app_services::project::{
    ActiveProjectQuery, ConnectExistingRepositoryCommand, CreateNewProjectCommand,
    ListVisibleProjectsQuery,
};
use tanren_contract::{
    AcceptInvitationRequest, AccountView, ActiveProjectCookieRequest, ActiveProjectRequest,
    ActiveProjectView, ConnectProjectRepositoryCookieRequest, ConnectProjectRepositoryRequest,
    ConnectProjectRepositoryResponse, CreateProjectCookieRequest, CreateProjectRequest,
    CreateProjectResponse, ListVisibleProjectsCookieRequest, ListVisibleProjectsRequest,
    ProjectCollectionView, SessionEnvelope, SignInRequest, SignUpRequest,
};
use tanren_identity_policy::{Email, InvitationToken, OrgId};
use tower_sessions::Session;
use utoipa::OpenApi;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
    pub contract_version: u32,
}
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct SignUpResponseCookie {
    pub account: AccountView,
    pub session: SessionEnvelope,
}
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct SignInResponseCookie {
    pub account: AccountView,
    pub session: SessionEnvelope,
}
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct AcceptInvitationResponseCookie {
    pub account: AccountView,
    pub session: SessionEnvelope,
    pub joined_org: OrgId,
}
#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct AcceptInvitationBody {
    pub email: Email,
    #[schema(value_type = String, format = Password)]
    pub password: String,
    pub display_name: String,
}
#[derive(OpenApi)]
#[openapi(
    info(
        title = "Tanren API",
        description = "Tanren control plane for agentic software delivery.",
        version = env!("CARGO_PKG_VERSION"),
    ),
    paths(
        health_route,
        sign_up_route,
        sign_in_route,
        accept_invitation_route,
        revoke_route,
        connect_project_repository_route,
        create_project_route,
        list_visible_projects_route,
        active_project_route,
    ),
    components(schemas(
        HealthResponse,
        SignUpRequest,
        SignUpResponseCookie,
        SignInRequest,
        SignInResponseCookie,
        AcceptInvitationBody,
        AcceptInvitationResponseCookie,
        AccountFailureBody,
        ProjectFailureBody,
        ConnectProjectRepositoryCookieRequest,
        ConnectProjectRepositoryResponse,
        CreateProjectCookieRequest,
        CreateProjectResponse,
        ListVisibleProjectsCookieRequest,
        ProjectCollectionView,
        ActiveProjectCookieRequest,
        ActiveProjectView,
        SessionEnvelope,
    )),
    tags(
        (name = "health", description = "Liveness probe."),
        (name = "accounts", description = "Account flow: self-signup, sign-in, accept-invitation, sign-out."),
        (name = "projects", description = "Project setup and project visibility flow."),
    )
)]
pub(crate) struct ApiDoc;

#[utoipa::path(
    get,
    path = "/health",
    responses(
        (status = 200, body = HealthResponse, description = "Service is live"),
    ),
    tag = "health",
)]
pub(crate) async fn health_route() -> Json<HealthResponse> {
    let report = Handlers::new().health(env!("CARGO_PKG_VERSION"));
    Json(HealthResponse {
        status: report.status.to_owned(),
        version: report.version.to_owned(),
        contract_version: report.contract_version.value(),
    })
}

#[utoipa::path(
    post,
    path = "/accounts",
    request_body = SignUpRequest,
    responses(
        (status = 201, body = SignUpResponseCookie, description = "Account created"),
        (status = 400, body = AccountFailureBody, description = "validation_failed"),
        (status = 401, body = AccountFailureBody, description = "invalid_credential"),
        (status = 409, body = AccountFailureBody, description = "duplicate_identifier"),
    ),
    tag = "accounts",
)]
pub(crate) async fn sign_up_route(
    State(state): State<AppState>,
    session: Session,
    ValidatedJson(request): ValidatedJson<SignUpRequest>,
) -> Response {
    match state.handlers.sign_up(state.store.as_ref(), request).await {
        Ok(response) => {
            let write = SessionWrite {
                account_id: response.session.account_id,
                expires_at: response.session.expires_at,
            };
            match install_cookie_session(&session, &write).await {
                Ok(()) => (
                    StatusCode::CREATED,
                    Json(SignUpResponseCookie {
                        account: response.account,
                        session: SessionEnvelope::cookie(&response.session),
                    }),
                )
                    .into_response(),
                Err(err) => session_install_error(&err),
            }
        }
        Err(err) => map_app_error(err),
    }
}

#[utoipa::path(
    post,
    path = "/sessions",
    request_body = SignInRequest,
    responses(
        (status = 200, body = SignInResponseCookie, description = "Sign-in succeeded"),
        (status = 400, body = AccountFailureBody, description = "validation_failed"),
        (status = 401, body = AccountFailureBody, description = "invalid_credential"),
    ),
    tag = "accounts",
)]
pub(crate) async fn sign_in_route(
    State(state): State<AppState>,
    session: Session,
    ValidatedJson(request): ValidatedJson<SignInRequest>,
) -> Response {
    match state.handlers.sign_in(state.store.as_ref(), request).await {
        Ok(response) => {
            let write = SessionWrite {
                account_id: response.session.account_id,
                expires_at: response.session.expires_at,
            };
            match install_cookie_session(&session, &write).await {
                Ok(()) => (
                    StatusCode::OK,
                    Json(SignInResponseCookie {
                        account: response.account,
                        session: SessionEnvelope::cookie(&response.session),
                    }),
                )
                    .into_response(),
                Err(err) => session_install_error(&err),
            }
        }
        Err(err) => map_app_error(err),
    }
}

#[utoipa::path(
    post,
    path = "/invitations/{token}/accept",
    request_body = AcceptInvitationBody,
    params(
        ("token" = String, Path, description = "Opaque invitation token"),
    ),
    responses(
        (status = 201, body = AcceptInvitationResponseCookie, description = "Invitation accepted"),
        (status = 400, body = AccountFailureBody, description = "validation_failed"),
        (status = 404, body = AccountFailureBody, description = "invitation_not_found"),
        (status = 410, body = AccountFailureBody, description = "invitation_expired or invitation_already_consumed"),
    ),
    tag = "accounts",
)]
pub(crate) async fn accept_invitation_route(
    State(state): State<AppState>,
    session: Session,
    Path(token): Path<String>,
    ValidatedJson(body): ValidatedJson<AcceptInvitationBody>,
) -> Response {
    let invitation_token = match InvitationToken::parse(&token) {
        Ok(t) => t,
        Err(err) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(AccountFailureBody {
                    code: "validation_failed".to_owned(),
                    summary: err.to_string(),
                }),
            )
                .into_response();
        }
    };
    let request = AcceptInvitationRequest {
        invitation_token,
        email: body.email,
        password: SecretString::from(body.password),
        display_name: body.display_name,
    };
    match state
        .handlers
        .accept_invitation(state.store.as_ref(), request)
        .await
    {
        Ok(response) => {
            let write = SessionWrite {
                account_id: response.session.account_id,
                expires_at: response.session.expires_at,
            };
            match install_cookie_session(&session, &write).await {
                Ok(()) => (
                    StatusCode::CREATED,
                    Json(AcceptInvitationResponseCookie {
                        account: response.account,
                        session: SessionEnvelope::cookie(&response.session),
                        joined_org: response.joined_org,
                    }),
                )
                    .into_response(),
                Err(err) => session_install_error(&err),
            }
        }
        Err(err) => map_app_error(err),
    }
}

#[utoipa::path(
    post,
    path = "/projects/connect-repository",
    request_body = ConnectProjectRepositoryCookieRequest,
    responses(
        (status = 201, body = ConnectProjectRepositoryResponse, description = "Repository connected as project"),
        (status = 401, body = ProjectFailureBody, description = "auth_required"),
        (status = 400, body = ProjectFailureBody, description = "validation_failed"),
        (status = 403, body = ProjectFailureBody, description = "no_access"),
        (status = 409, body = ProjectFailureBody, description = "duplicate_repository"),
        (status = 502, body = ProjectFailureBody, description = "provider_failure"),
    ),
    tag = "projects",
)]
pub(crate) async fn connect_project_repository_route(
    State(state): State<AppState>,
    session: Session,
    ValidatedJson(request): ValidatedJson<ConnectProjectRepositoryCookieRequest>,
) -> Response {
    let actor_account_id = match session_actor_account_id(&session).await {
        Ok(id) => id,
        Err(response) => return response,
    };
    if legacy_scope_mismatch(request.legacy_owning_account_id, actor_account_id) {
        return map_app_error(AppServiceError::Project(
            tanren_contract::ProjectFailureReason::NoAccess,
        ));
    }
    match state
        .handlers
        .connect_project_repository(
            state.store.as_ref(),
            state.source_control.as_ref(),
            ConnectExistingRepositoryCommand {
                actor_account_id,
                request: ConnectProjectRepositoryRequest {
                    owning_account_id: actor_account_id,
                    repository: request.repository,
                    select_as_active: request.select_as_active,
                },
            },
        )
        .await
    {
        Ok(response) => (StatusCode::CREATED, Json(response)).into_response(),
        Err(err) => map_app_error(err),
    }
}

#[utoipa::path(
    post,
    path = "/projects/create",
    request_body = CreateProjectCookieRequest,
    responses(
        (status = 201, body = CreateProjectResponse, description = "Project and repository created"),
        (status = 401, body = ProjectFailureBody, description = "auth_required"),
        (status = 400, body = ProjectFailureBody, description = "validation_failed"),
        (status = 403, body = ProjectFailureBody, description = "no_access"),
        (status = 409, body = ProjectFailureBody, description = "duplicate_repository"),
        (status = 502, body = ProjectFailureBody, description = "provider_failure"),
    ),
    tag = "projects",
)]
pub(crate) async fn create_project_route(
    State(state): State<AppState>,
    session: Session,
    ValidatedJson(request): ValidatedJson<CreateProjectCookieRequest>,
) -> Response {
    let actor_account_id = match session_actor_account_id(&session).await {
        Ok(id) => id,
        Err(response) => return response,
    };
    if legacy_scope_mismatch(request.legacy_owning_account_id, actor_account_id) {
        return map_app_error(AppServiceError::Project(
            tanren_contract::ProjectFailureReason::NoAccess,
        ));
    }
    match state
        .handlers
        .create_project(
            state.store.as_ref(),
            state.source_control.as_ref(),
            CreateNewProjectCommand {
                actor_account_id,
                request: CreateProjectRequest {
                    owning_account_id: actor_account_id,
                    repository: request.repository,
                    designated_host: request.designated_host,
                    select_as_active: request.select_as_active,
                },
            },
        )
        .await
    {
        Ok(response) => (StatusCode::CREATED, Json(response)).into_response(),
        Err(err) => map_app_error(err),
    }
}

#[utoipa::path(
    post,
    path = "/projects/list",
    request_body = ListVisibleProjectsCookieRequest,
    responses(
        (status = 200, body = ProjectCollectionView, description = "Project list"),
        (status = 401, body = ProjectFailureBody, description = "auth_required"),
        (status = 403, body = ProjectFailureBody, description = "no_access"),
    ),
    tag = "projects",
)]
pub(crate) async fn list_visible_projects_route(
    State(state): State<AppState>,
    session: Session,
    ValidatedJson(_request): ValidatedJson<ListVisibleProjectsCookieRequest>,
) -> Response {
    let actor_account_id = match session_actor_account_id(&session).await {
        Ok(id) => id,
        Err(response) => return response,
    };
    match state
        .handlers
        .list_visible_projects(
            state.store.as_ref(),
            ListVisibleProjectsQuery {
                actor_account_id,
                request: ListVisibleProjectsRequest {
                    owning_account_id: actor_account_id,
                },
            },
        )
        .await
    {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(err) => map_app_error(err),
    }
}

#[utoipa::path(
    post,
    path = "/projects/active",
    request_body = ActiveProjectCookieRequest,
    responses(
        (status = 200, body = ActiveProjectView, description = "Active-project metadata"),
        (status = 401, body = ProjectFailureBody, description = "auth_required"),
        (status = 403, body = ProjectFailureBody, description = "no_access"),
    ),
    tag = "projects",
)]
pub(crate) async fn active_project_route(
    State(state): State<AppState>,
    session: Session,
    ValidatedJson(_request): ValidatedJson<ActiveProjectCookieRequest>,
) -> Response {
    let actor_account_id = match session_actor_account_id(&session).await {
        Ok(id) => id,
        Err(response) => return response,
    };
    match state
        .handlers
        .active_project(
            state.store.as_ref(),
            ActiveProjectQuery {
                actor_account_id,
                request: ActiveProjectRequest {
                    owning_account_id: actor_account_id,
                },
            },
        )
        .await
    {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(err) => map_app_error(err),
    }
}

#[utoipa::path(
    post,
    path = "/sessions/revoke",
    responses(
        (status = 204, description = "Session revoked"),
    ),
    tag = "accounts",
)]
pub(crate) async fn revoke_route(session: Session) -> Response {
    if let Err(err) = session.flush().await {
        tracing::error!(target: "tanren_api", error = %err, "session flush");
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(AccountFailureBody {
                code: "internal_error".to_owned(),
                summary: "Tanren encountered an internal error.".to_owned(),
            }),
        )
            .into_response();
    }
    StatusCode::NO_CONTENT.into_response()
}

async fn session_actor_account_id(
    session: &Session,
) -> Result<tanren_identity_policy::AccountId, Response> {
    match session_account(session).await {
        Ok(Some(context)) => Ok(context.account_id),
        Ok(None) => Err(auth_required()),
        Err(err) => {
            tracing::error!(target: "tanren_api", error = %err, "session read");
            Err(auth_required())
        }
    }
}

fn legacy_scope_mismatch(
    legacy_owning_account_id: Option<tanren_identity_policy::AccountId>,
    actor_account_id: tanren_identity_policy::AccountId,
) -> bool {
    legacy_owning_account_id.is_some_and(|owning| owning != actor_account_id)
}

pub(crate) fn build_router(state: AppState) -> OpenApiRouter {
    OpenApiRouter::with_openapi(ApiDoc::openapi())
        .routes(routes!(health_route))
        .routes(routes!(sign_up_route))
        .routes(routes!(sign_in_route))
        .routes(routes!(accept_invitation_route))
        .routes(routes!(revoke_route))
        .routes(routes!(connect_project_repository_route))
        .routes(routes!(create_project_route))
        .routes(routes!(list_visible_projects_route))
        .routes(routes!(active_project_route))
        .with_state(state)
}
