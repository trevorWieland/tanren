//! Axum route handlers + per-handler `#[utoipa::path(...)]` annotations
//! + the top-level `ApiDoc` struct that the `OpenApi` derive walks.
//!
//! Split out of `lib.rs` so the api-app crate stays under the workspace
//! 500-line line-budget. The wiring (router, openapi-json route,
//! tower-sessions layer) lives in `lib.rs::build_app`.

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use secrecy::SecretString;
use serde::{Deserialize, Serialize};
use tanren_app_services::Handlers;
use tanren_contract::{
    AcceptInvitationRequest, AccountView, SessionEnvelope, SignInRequest, SignUpRequest,
    SwitchActiveAccountRequest, SwitchActiveAccountResponse,
};
use tanren_identity_policy::{AccountId, Email, InvitationToken, OrgId};
use tower_sessions::Session;
use utoipa::OpenApi;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use crate::AppState;
use crate::cookies::{
    SessionWrite, install_cookie_session, read_active_account, read_signed_in_accounts,
    write_active_account,
};
use crate::errors::{AccountFailureBody, ValidatedJson, map_app_error, session_install_error};

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

/// Cookie-transport response shape for the api surface. Mirrors
/// `SignUpResponse`/`SignInResponse`/`AcceptInvitationResponse` but
/// projects the session into [`SessionEnvelope::Cookie`] (no token in
/// body — it ships in the `Set-Cookie` header).
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct SignUpResponseCookie {
    /// View of the freshly created account.
    pub account: AccountView,
    /// Cookie-projected session envelope.
    pub session: SessionEnvelope,
}

/// Cookie-transport projection of a sign-in response.
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct SignInResponseCookie {
    /// View of the signed-in account.
    pub account: AccountView,
    /// Cookie-projected session envelope.
    pub session: SessionEnvelope,
}

/// Cookie-transport projection of an invitation-acceptance response.
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct AcceptInvitationResponseCookie {
    /// View of the newly created account.
    pub account: AccountView,
    /// Cookie-projected session envelope.
    pub session: SessionEnvelope,
    /// Organization the new account joined.
    pub joined_org: OrgId,
}

/// Path body for `POST /invitations/{token}/accept`. Splits the password
/// into a `String` here (then re-wraps as `SecretString` before handing
/// off to app-services) so utoipa can document the schema; the secret
/// stays in memory only for the lifetime of this function.
#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct AcceptInvitationBody {
    /// Email the invitee chose.
    pub email: Email,
    /// Plaintext password.
    #[schema(value_type = String, format = Password)]
    pub password: String,
    /// Display name.
    pub display_name: String,
}

/// Top-level `OpenAPI` doc. Each handler is annotated with
/// `#[utoipa::path(...)]` and listed under `paths(...)` here.
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
        switch_active_account_route,
        get_active_account_route,
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
        SessionEnvelope,
        SwitchActiveAccountRequest,
        SwitchActiveAccountResponse,
        ActiveAccountResponse,
    )),
    tags(
        (name = "health", description = "Liveness probe."),
        (name = "accounts", description = "Account flow: self-signup, sign-in, accept-invitation, sign-out, switch active."),
    )
)]
pub(crate) struct ApiDoc;

/// Liveness probe.
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

/// Self-signup: create a new personal account and mint a cookie-bound
/// session.
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

/// Sign-in: mint a cookie-bound session for an existing account.
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

/// Accept an organization invitation and mint a cookie-bound session.
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

/// Revoke (sign out) the current session. Clears the cookie via
/// `Session::flush` and returns 204.
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

/// Response for GET /accounts/active — the currently active account.
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub(crate) struct ActiveAccountResponse {
    /// The currently active account id.
    pub active_account_id: Option<AccountId>,
}

/// Read the currently active account from the session.
#[utoipa::path(
    get,
    path = "/accounts/active",
    responses(
        (status = 200, body = ActiveAccountResponse, description = "Active account id"),
        (status = 401, body = AccountFailureBody, description = "invalid_credential"),
    ),
    tag = "accounts",
)]
pub(crate) async fn get_active_account_route(session: Session) -> Response {
    let active = read_active_account(&session, None).await;
    (
        StatusCode::OK,
        Json(ActiveAccountResponse {
            active_account_id: active,
        }),
    )
        .into_response()
}

/// Switch the active account within an existing session.
///
/// Validates `window_id` format (if provided), reads the signed-in
/// accounts from the session, resolves the `target` ordinal or UUID to
/// a concrete [`AccountId`], delegates to the app-service handler, and
/// updates the session with the new active account.
#[utoipa::path(
    put,
    path = "/accounts/active",
    request_body = SwitchActiveAccountRequest,
    responses(
        (status = 200, body = SwitchActiveAccountResponse, description = "Active account switched"),
        (status = 400, body = AccountFailureBody, description = "validation_failed"),
        (status = 401, body = AccountFailureBody, description = "invalid_credential"),
        (status = 422, body = AccountFailureBody, description = "target_account_not_signed_in"),
    ),
    tag = "accounts",
)]
pub(crate) async fn switch_active_account_route(
    State(state): State<AppState>,
    session: Session,
    ValidatedJson(request): ValidatedJson<SwitchActiveAccountRequest>,
) -> Response {
    if let Some(ref wid) = request.window_id {
        if let Err(summary) = validate_window_id(wid) {
            return (
                StatusCode::BAD_REQUEST,
                Json(AccountFailureBody {
                    code: "validation_failed".to_owned(),
                    summary,
                }),
            )
                .into_response();
        }
    }

    let signed_in = read_signed_in_accounts(&session).await;
    if signed_in.is_empty() {
        return (
            StatusCode::UNAUTHORIZED,
            Json(AccountFailureBody {
                code: "invalid_credential".to_owned(),
                summary: tanren_contract::AccountFailureReason::InvalidCredential
                    .summary()
                    .to_owned(),
            }),
        )
            .into_response();
    }

    let Some(target_id) = resolve_target(&request.target, &signed_in) else {
        return map_app_error(tanren_app_services::AppServiceError::Account(
            tanren_contract::AccountFailureReason::TargetAccountNotSignedIn,
        ));
    };

    match state
        .handlers
        .switch_active_account(
            state.store.as_ref(),
            &signed_in,
            target_id,
            request.window_id.clone(),
        )
        .await
    {
        Ok(response) => {
            let wid = request.window_id.as_deref();
            if let Err(err) = write_active_account(&session, target_id, wid).await {
                return session_install_error(&err);
            }
            (StatusCode::OK, Json(response)).into_response()
        }
        Err(err) => map_app_error(err),
    }
}

/// Validate a window-id string: non-empty, at most 128 chars, valid UUID.
fn validate_window_id(window_id: &str) -> Result<(), String> {
    if window_id.is_empty() {
        return Err("window_id must not be empty".to_owned());
    }
    if window_id.len() > 128 {
        return Err(format!(
            "window_id must be at most 128 characters, got {}",
            window_id.len()
        ));
    }
    if Uuid::parse_str(window_id).is_err() {
        return Err(format!("window_id must be a valid UUID, got {window_id:?}"));
    }
    Ok(())
}

/// Resolve a target string to a concrete `AccountId` from the ordered
/// signed-in list. Accepts ordinals (`"first"`, `"second"`, …) and bare
/// UUID strings that appear in the list.
fn resolve_target(target: &str, signed_in: &[AccountId]) -> Option<AccountId> {
    match target {
        "first" => signed_in.first().copied(),
        "second" => signed_in.get(1).copied(),
        "third" => signed_in.get(2).copied(),
        "fourth" => signed_in.get(3).copied(),
        "fifth" => signed_in.get(4).copied(),
        _ => {
            let uuid = Uuid::parse_str(target).ok()?;
            let id = AccountId::from(uuid);
            signed_in.contains(&id).then_some(id)
        }
    }
}

/// Build the `OpenApiRouter` carrying every account-flow route. Called
/// from `lib.rs::build_app` after the cookie/CORS layers are
/// constructed; the macros that `routes!()` expands need to live in the
/// same module as the `#[utoipa::path]`-annotated handlers, so the
/// router constructor lives here too.
pub(crate) fn build_router(state: AppState) -> OpenApiRouter {
    OpenApiRouter::with_openapi(ApiDoc::openapi())
        .routes(routes!(health_route))
        .routes(routes!(sign_up_route))
        .routes(routes!(sign_in_route))
        .routes(routes!(accept_invitation_route))
        .routes(routes!(revoke_route))
        .routes(routes!(switch_active_account_route))
        .routes(routes!(get_active_account_route))
        .with_state(state)
}
