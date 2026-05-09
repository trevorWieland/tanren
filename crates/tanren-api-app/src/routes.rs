//! Axum route handlers + utoipa path annotations for the API surface.

use axum::Json;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use secrecy::SecretString;
use serde::{Deserialize, Serialize};
use tanren_app_services::{
    AccountErrorProjection, ActiveAccountContext, ActiveAccountContextError, Handlers,
};
use tanren_contract::{
    AcceptInvitationRequest, AccountFailureReason, AccountView, ListActiveAccountsRequest,
    ListActiveAccountsResponse, SessionEnvelope, SignInRequest, SignUpRequest,
    SwitchActiveAccountRequest, SwitchActiveAccountResponse,
};
use tanren_identity_policy::{Email, InvitationToken, OrgId};
use tower_sessions::Session;
use utoipa::OpenApi;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::AppState;
use crate::cookies::{
    SessionWrite, install_cookie_session, read_session_account_context,
    write_active_account_for_window,
};
use crate::errors::{AccountFailureBody, ValidatedJson, map_app_error, session_install_error};
use crate::window_context::resolve_window_context;

/// Liveness response.
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct HealthResponse {
    /// Static "ok" string.
    pub status: String,
    /// Build-time package version.
    pub version: String,
    /// Wire-contract version.
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
        list_active_accounts_route,
        switch_active_account_route,
        revoke_route,
    ),
    components(schemas(
        HealthResponse,
        SignUpRequest,
        SignUpResponseCookie,
        SignInRequest,
        SignInResponseCookie,
        AcceptInvitationBody,
        AcceptInvitationResponseCookie,
        ListActiveAccountsRequest,
        ListActiveAccountsResponse,
        SwitchActiveAccountRequest,
        SwitchActiveAccountResponse,
        AccountFailureBody,
        SessionEnvelope,
    )),
    tags(
        (name = "health", description = "Liveness probe."),
        (name = "accounts", description = "Account flow: self-signup, sign-in, accept-invitation, active-account switch, sign-out."),
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
    headers: HeaderMap,
    ValidatedJson(request): ValidatedJson<SignUpRequest>,
) -> Response {
    let window_context = match resolve_window_context(&headers) {
        Ok(value) => value,
        Err(err) => return window_id_validation_error(err.summary()),
    };
    match state.handlers.sign_up(state.store.as_ref(), request).await {
        Ok(response) => {
            let write = SessionWrite {
                account_id: response.session.account_id,
                expires_at: response.session.expires_at,
            };
            match install_cookie_session(&session, &write, window_context).await {
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
        Err(err) => map_app_error(&err),
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
    headers: HeaderMap,
    ValidatedJson(request): ValidatedJson<SignInRequest>,
) -> Response {
    let window_context = match resolve_window_context(&headers) {
        Ok(value) => value,
        Err(err) => return window_id_validation_error(err.summary()),
    };
    match state.handlers.sign_in(state.store.as_ref(), request).await {
        Ok(response) => {
            let write = SessionWrite {
                account_id: response.session.account_id,
                expires_at: response.session.expires_at,
            };
            match install_cookie_session(&session, &write, window_context).await {
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
        Err(err) => map_app_error(&err),
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
    headers: HeaderMap,
    Path(token): Path<String>,
    ValidatedJson(body): ValidatedJson<AcceptInvitationBody>,
) -> Response {
    let window_context = match resolve_window_context(&headers) {
        Ok(value) => value,
        Err(err) => return window_id_validation_error(err.summary()),
    };
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
            match install_cookie_session(&session, &write, window_context).await {
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
        Err(err) => map_app_error(&err),
    }
}

#[utoipa::path(
    get,
    path = "/accounts/active",
    responses(
        (status = 200, body = ListActiveAccountsResponse, description = "Signed-in accounts returned"),
        (status = 400, body = AccountFailureBody, description = "validation_failed"),
        (status = 401, body = AccountFailureBody, description = "invalid_credential"),
    ),
    tag = "accounts",
)]
pub(crate) async fn list_active_accounts_route(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
) -> Response {
    let window_context = match resolve_window_context(&headers) {
        Ok(value) => value,
        Err(err) => return window_id_validation_error(err.summary()),
    };
    let session_context = match read_session_account_context(&session, window_context).await {
        Ok(Some(context)) => context,
        Ok(None) => return missing_session_response(),
        Err(err) => return session_read_error(&err),
    };

    let context = match ActiveAccountContext::from_account_ids(
        session_context.active_account_id,
        session_context.signed_in_account_ids,
    ) {
        Ok(context) => context,
        Err(err) => return active_account_context_validation_error(&err),
    };

    match state
        .handlers
        .list_active_accounts(
            state.store.as_ref(),
            &context,
            ListActiveAccountsRequest::default(),
        )
        .await
    {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(err) => map_app_error(&err),
    }
}

#[utoipa::path(
    post,
    path = "/accounts/active/switch",
    request_body = SwitchActiveAccountRequest,
    responses(
        (status = 200, body = SwitchActiveAccountResponse, description = "Active account switched"),
        (status = 400, body = AccountFailureBody, description = "validation_failed"),
        (status = 401, body = AccountFailureBody, description = "invalid_credential"),
        (status = 403, body = AccountFailureBody, description = "target_account_not_signed_in"),
    ),
    tag = "accounts",
)]
pub(crate) async fn switch_active_account_route(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
    ValidatedJson(request): ValidatedJson<SwitchActiveAccountRequest>,
) -> Response {
    let window_context = match resolve_window_context(&headers) {
        Ok(value) => value,
        Err(err) => return window_id_validation_error(err.summary()),
    };
    let session_context = match read_session_account_context(&session, window_context).await {
        Ok(Some(context)) => context,
        Ok(None) => return missing_session_response(),
        Err(err) => return session_read_error(&err),
    };

    let context = match ActiveAccountContext::from_account_ids(
        session_context.active_account_id,
        session_context.signed_in_account_ids,
    ) {
        Ok(context) => context,
        Err(err) => return active_account_context_validation_error(&err),
    };

    match state
        .handlers
        .switch_active_account(state.store.as_ref(), &context, request)
        .await
    {
        Ok(response) => {
            if let Err(err) = write_active_account_for_window(
                &session,
                window_context,
                response.active_account_id,
            )
            .await
            {
                return session_install_error(&err);
            }
            (StatusCode::OK, Json(response)).into_response()
        }
        Err(err) => map_app_error(&err),
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
        let projected = AccountErrorProjection::internal();
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(AccountFailureBody {
                code: projected.code,
                summary: projected.summary,
            }),
        )
            .into_response();
    }
    StatusCode::NO_CONTENT.into_response()
}

pub(crate) fn build_router(state: AppState) -> OpenApiRouter {
    OpenApiRouter::with_openapi(ApiDoc::openapi())
        .routes(routes!(health_route))
        .routes(routes!(sign_up_route))
        .routes(routes!(sign_in_route))
        .routes(routes!(accept_invitation_route))
        .routes(routes!(list_active_accounts_route))
        .routes(routes!(switch_active_account_route))
        .routes(routes!(revoke_route))
        .with_state(state)
}

fn missing_session_response() -> Response {
    let reason = AccountFailureReason::InvalidCredential;
    (
        StatusCode::UNAUTHORIZED,
        Json(AccountFailureBody {
            code: reason.code().to_owned(),
            summary: reason.summary().to_owned(),
        }),
    )
        .into_response()
}

fn window_id_validation_error(summary: &str) -> Response {
    (
        StatusCode::BAD_REQUEST,
        Json(AccountFailureBody {
            code: AccountFailureReason::ValidationFailed.code().to_owned(),
            summary: summary.to_owned(),
        }),
    )
        .into_response()
}

fn active_account_context_validation_error(err: &ActiveAccountContextError) -> Response {
    (
        StatusCode::BAD_REQUEST,
        Json(AccountFailureBody {
            code: AccountFailureReason::ValidationFailed.code().to_owned(),
            summary: err.to_string(),
        }),
    )
        .into_response()
}

fn session_read_error(err: &anyhow::Error) -> Response {
    tracing::error!(target: "tanren_api", error = %err, "session read");
    let projected = AccountErrorProjection::internal();
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(AccountFailureBody {
            code: projected.code,
            summary: projected.summary,
        }),
    )
        .into_response()
}
