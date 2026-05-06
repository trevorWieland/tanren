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
    AcceptInvitationRequest, AccountView, ListOrganizationProjectsResponse, OrganizationSwitcher,
    SessionEnvelope, SignInRequest, SignUpRequest, SwitchActiveOrganizationRequest,
    SwitchActiveOrganizationResponse,
};
use tanren_identity_policy::{AccountId, Email, InvitationToken, OrgId};
use tower_sessions::Session;
use utoipa::OpenApi;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::AppState;
use crate::cookies::{SessionWrite, install_cookie_session};
use crate::errors::{AccountFailureBody, ValidatedJson, map_app_error, session_install_error};

const SESSION_KEY_ACCOUNT: &str = "account_id";

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

/// Extract the `AccountId` stored in the cookie session. Returns a 401
/// `unauthenticated` error body when the session is absent or does not
/// contain an account id. All organization-scoped routes use this helper
/// instead of trusting an account id supplied in the request body.
async fn require_account_id(session: &Session) -> Result<AccountId, Response> {
    session
        .get::<AccountId>(SESSION_KEY_ACCOUNT)
        .await
        .map_err(|err| {
            tracing::error!(target: "tanren_api", error = %err, "session read");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(AccountFailureBody {
                    code: "internal_error".to_owned(),
                    summary: "Tanren encountered an internal error.".to_owned(),
                }),
            )
                .into_response()
        })?
        .ok_or_else(|| {
            (
                StatusCode::UNAUTHORIZED,
                Json(AccountFailureBody {
                    code: "unauthenticated".to_owned(),
                    summary: "Authentication required.".to_owned(),
                }),
            )
                .into_response()
        })
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
        list_organizations_route,
        switch_active_org_route,
        list_active_org_projects_route,
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
        OrganizationSwitcher,
        SwitchActiveOrganizationRequest,
        SwitchActiveOrganizationResponse,
        ListOrganizationProjectsResponse,
    )),
    tags(
        (name = "health", description = "Liveness probe."),
        (name = "accounts", description = "Account flow: self-signup, sign-in, accept-invitation, sign-out."),
        (name = "organizations", description = "Organization-scoped operations: list memberships, switch active org, list projects."),
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

/// List organizations the authenticated account belongs to, including
/// which one is currently active. Personal accounts with zero org
/// memberships receive an empty memberships vector.
#[utoipa::path(
    get,
    path = "/account/organizations",
    responses(
        (status = 200, body = OrganizationSwitcher, description = "Organization memberships listed"),
        (status = 401, body = AccountFailureBody, description = "unauthenticated"),
    ),
    tag = "organizations",
)]
pub(crate) async fn list_organizations_route(
    State(state): State<AppState>,
    session: Session,
) -> Response {
    let account_id = match require_account_id(&session).await {
        Ok(id) => id,
        Err(resp) => return resp,
    };
    match state
        .handlers
        .list_organizations(state.store.as_ref(), account_id)
        .await
    {
        Ok(switcher) => (StatusCode::OK, Json(switcher)).into_response(),
        Err(err) => map_app_error(err),
    }
}

/// Switch the active organization for the authenticated account. The
/// target `org_id` comes from the request body; the `account_id` is
/// derived from the cookie session (never from the body).
#[utoipa::path(
    post,
    path = "/account/organizations/active",
    request_body = SwitchActiveOrganizationRequest,
    responses(
        (status = 200, body = SwitchActiveOrganizationResponse, description = "Active organization switched"),
        (status = 401, body = AccountFailureBody, description = "unauthenticated"),
        (status = 403, body = AccountFailureBody, description = "organization_not_member"),
    ),
    tag = "organizations",
)]
pub(crate) async fn switch_active_org_route(
    State(state): State<AppState>,
    session: Session,
    ValidatedJson(request): ValidatedJson<SwitchActiveOrganizationRequest>,
) -> Response {
    let account_id = match require_account_id(&session).await {
        Ok(id) => id,
        Err(resp) => return resp,
    };
    match state
        .handlers
        .switch_active_org(state.store.as_ref(), account_id, request)
        .await
    {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(err) => map_app_error(err),
    }
}

/// List projects scoped to the authenticated account's currently active
/// organization. Returns an empty list when the account has no active
/// organization (personal accounts with zero memberships).
#[utoipa::path(
    get,
    path = "/account/organizations/active/projects",
    responses(
        (status = 200, body = ListOrganizationProjectsResponse, description = "Projects listed"),
        (status = 401, body = AccountFailureBody, description = "unauthenticated"),
    ),
    tag = "organizations",
)]
pub(crate) async fn list_active_org_projects_route(
    State(state): State<AppState>,
    session: Session,
) -> Response {
    let account_id = match require_account_id(&session).await {
        Ok(id) => id,
        Err(resp) => return resp,
    };
    match state
        .handlers
        .list_active_org_projects(state.store.as_ref(), account_id)
        .await
    {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(err) => map_app_error(err),
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
        .routes(routes!(list_organizations_route))
        .routes(routes!(switch_active_org_route))
        .routes(routes!(list_active_org_projects_route))
        .with_state(state)
}
