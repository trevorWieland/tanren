//! Axum route handlers, `#[utoipa::path(...)]` annotations, and the
//! top-level `ApiDoc` struct. Split from `lib.rs` for line-budget.

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use secrecy::SecretString;
use serde::{Deserialize, Serialize};
use tanren_app_services::Handlers;
use tanren_contract::{
    AcceptInvitationRequest, AccountView, CreateOrganizationRequest, CreateOrganizationResponse,
    ListOrganizationsResponse, OrganizationAdminOperation, OrganizationMembershipView,
    OrganizationView, SessionEnvelope, SignInRequest, SignUpRequest,
};
use tanren_identity_policy::{Email, InvitationToken, OrgId};
use tower_sessions::Session;
use utoipa::OpenApi;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::AppState;
use crate::cookies::{SessionWrite, extract_account_id, install_cookie_session};
use crate::errors::{
    AccountFailureBody, ValidatedJson, auth_required_response, map_app_error, session_install_error,
};

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

/// Cookie-transport response for sign-up. Session ships via `Set-Cookie`.
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

/// Body for `POST /invitations/{token}/accept`. Password is re-wrapped as
/// `SecretString` before reaching app-services so utoipa can document it.
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

#[derive(Debug, Deserialize)]
pub(crate) struct AuthorizePath {
    pub(crate) org_id: OrgId,
    pub(crate) operation: OrganizationAdminOperation,
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
        create_organization_route,
        list_account_organizations_route,
        authorize_admin_operation_route,
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
        CreateOrganizationRequest,
        CreateOrganizationResponse,
        OrganizationView,
        OrganizationMembershipView,
        OrganizationAdminOperation,
        ListOrganizationsResponse,
    )),
    tags(
        (name = "health", description = "Liveness probe."),
        (name = "accounts", description = "Account flow: self-signup, sign-in, accept-invitation, sign-out."),
        (name = "organizations", description = "Organization creation, listing, and admin authorization."),
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

/// Create a new organization. The cookie-authenticated account becomes the
/// creator and receives all five bootstrap admin permissions.
#[utoipa::path(
    post,
    path = "/organizations",
    request_body = CreateOrganizationRequest,
    responses(
        (status = 201, body = CreateOrganizationResponse, description = "Organization created"),
        (status = 400, body = AccountFailureBody, description = "validation_failed"),
        (status = 401, body = AccountFailureBody, description = "auth_required"),
        (status = 409, body = AccountFailureBody, description = "duplicate_organization_name"),
    ),
    tag = "organizations",
)]
pub(crate) async fn create_organization_route(
    State(state): State<AppState>,
    session: Session,
    ValidatedJson(request): ValidatedJson<CreateOrganizationRequest>,
) -> Response {
    let Some(account_id) = extract_account_id(&session).await else {
        return auth_required_response();
    };
    match state
        .handlers
        .create_organization_for_account(state.store.as_ref(), account_id, request)
        .await
    {
        Ok(output) => (
            StatusCode::CREATED,
            Json(CreateOrganizationResponse {
                organization: output.organization,
                membership: output.membership,
            }),
        )
            .into_response(),
        Err(err) => map_app_error(err),
    }
}

/// List organizations the cookie-authenticated account belongs to.
#[utoipa::path(
    get,
    path = "/account/organizations",
    responses(
        (status = 200, body = ListOrganizationsResponse, description = "Organizations listed"),
        (status = 401, body = AccountFailureBody, description = "auth_required"),
    ),
    tag = "organizations",
)]
pub(crate) async fn list_account_organizations_route(
    State(state): State<AppState>,
    session: Session,
) -> Response {
    let Some(account_id) = extract_account_id(&session).await else {
        return auth_required_response();
    };
    match state
        .handlers
        .list_account_organizations(state.store.as_ref(), account_id)
        .await
    {
        Ok(organizations) => (
            StatusCode::OK,
            Json(ListOrganizationsResponse { organizations }),
        )
            .into_response(),
        Err(err) => map_app_error(err),
    }
}

/// No-op authorization probe: returns 204 when the authenticated account
/// holds the permission matching the requested admin operation on the
/// specified organization.
#[utoipa::path(
    post,
    path = "/organizations/{org_id}/admin-operations/{operation}/authorize",
    params(
        ("org_id" = OrgId, Path, description = "Organization id"),
        ("operation" = OrganizationAdminOperation, Path, description = "Admin operation to probe"),
    ),
    responses(
        (status = 204, description = "Operation authorized"),
        (status = 401, body = AccountFailureBody, description = "auth_required"),
        (status = 403, body = AccountFailureBody, description = "permission_denied"),
    ),
    tag = "organizations",
)]
pub(crate) async fn authorize_admin_operation_route(
    State(state): State<AppState>,
    session: Session,
    Path(params): Path<AuthorizePath>,
) -> Response {
    let Some(account_id) = extract_account_id(&session).await else {
        return auth_required_response();
    };
    match state
        .handlers
        .authorize_org_admin_operation(
            state.store.as_ref(),
            account_id,
            params.org_id,
            params.operation,
        )
        .await
    {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(err) => map_app_error(err),
    }
}

/// Build the `OpenApiRouter` carrying every route. Called from
/// `lib.rs::build_app`; must live here for `routes!()` macro expansion.
pub(crate) fn build_router(state: AppState) -> OpenApiRouter {
    OpenApiRouter::with_openapi(ApiDoc::openapi())
        .routes(routes!(health_route))
        .routes(routes!(sign_up_route))
        .routes(routes!(sign_in_route))
        .routes(routes!(accept_invitation_route))
        .routes(routes!(revoke_route))
        .routes(routes!(create_organization_route))
        .routes(routes!(list_account_organizations_route))
        .routes(routes!(authorize_admin_operation_route))
        .with_state(state)
}
