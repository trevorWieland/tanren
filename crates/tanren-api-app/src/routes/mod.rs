//! Axum route handlers + `#[utoipa::path(...)]` annotations + `ApiDoc`; wiring lives in `lib.rs::build_app`.
mod members;
use crate::AppState;
use crate::auth::require_authoritative_auth;
use crate::cookies::{SessionWrite, install_cookie_session};
use crate::errors::{
    AccountFailureBody, OrganizationValidatedJson, ValidatedJson, map_app_error,
    map_organization_app_error, session_install_error,
};
use crate::organization_tracing::{
    emit_route_auth_denial, emit_route_failure, emit_route_success, organization_route_span,
    record_authenticated_account,
};
use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use secrecy::SecretString;
use serde::{Deserialize, Serialize};
use tanren_app_services::Handlers;
use tanren_contract::{
    AcceptInvitationRequest, AccountView, CheckOrganizationPermissionApiRequest,
    CheckOrganizationPermissionResponse, CreateOrganizationApiRequest, CreateOrganizationResponse,
    ListOrganizationMembersApiPath, ListOrganizationMembersApiQuery,
    ListOrganizationMembersResponse, ListOrganizationsApiQuery, ListOrganizationsResponse,
    OrganizationFailureBody, SessionEnvelope, SignInRequest, SignUpRequest,
};
use tanren_identity_policy::{Email, InvitationToken, OrgId};
use tower_sessions::Session;
use utoipa::OpenApi;
use utoipa_axum::{router::OpenApiRouter, routes};
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
/// `SignUpResponse`/`SignInResponse`/`AcceptInvitationResponse` and projects
/// the session into [`SessionEnvelope::Cookie`] (body has no token).
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

/// Path body for `POST /invitations/{token}/accept`.
#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct AcceptInvitationBody {
    /// Email the invitee chose.
    pub email: Email,
    /// Plaintext password.
    #[serde(deserialize_with = "tanren_identity_policy::secret_serde::deserialize_password")]
    #[schema(value_type = String, format = Password)]
    pub password: SecretString,
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
        create_organization_route,
        list_organizations_route,
        check_organization_permission_route,
        members::list_organization_members_route,
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
        CreateOrganizationApiRequest,
        CheckOrganizationPermissionApiRequest,
        ListOrganizationsApiQuery,
        CreateOrganizationResponse,
        ListOrganizationsResponse,
        ListOrganizationMembersResponse,
        ListOrganizationMembersApiPath,
        ListOrganizationMembersApiQuery,
        CheckOrganizationPermissionResponse,
        AccountFailureBody,
        OrganizationFailureBody,
        SessionEnvelope,
    )),
    tags(
        (name = "health", description = "Liveness probe."),
        (name = "accounts", description = "Account flow: self-signup, sign-in, accept-invitation, sign-out."),
        (name = "organizations", description = "Organization create/list/permission-check/member-list operations."),
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
                token: response.session.token.clone(),
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
                token: response.session.token.clone(),
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
        password: body.password,
        display_name: body.display_name,
    };
    match state
        .handlers
        .accept_invitation(state.store.as_ref(), request)
        .await
    {
        Ok(response) => {
            let write = SessionWrite {
                token: response.session.token.clone(),
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
    path = "/organizations",
    request_body = CreateOrganizationApiRequest,
    responses(
        (status = 201, body = CreateOrganizationResponse, description = "Organization created"),
        (status = 400, body = OrganizationFailureBody, description = "validation_failed"),
        (status = 401, body = OrganizationFailureBody, description = "auth_required"),
        (status = 409, body = OrganizationFailureBody, description = "conflict or idempotency_conflict"),
        (status = 500, body = OrganizationFailureBody, description = "internal_error"),
    ),
    tag = "organizations",
)]
pub(crate) async fn create_organization_route(
    State(state): State<AppState>,
    session: Session,
    OrganizationValidatedJson(body): OrganizationValidatedJson<CreateOrganizationApiRequest>,
) -> Response {
    let span = organization_route_span("create_organization", None);
    let _span_guard = span.enter();
    let auth = match require_authoritative_auth(&state, &session).await {
        Ok(auth) => auth,
        Err(response) => {
            emit_route_auth_denial("create_organization", None);
            return response;
        }
    };
    record_authenticated_account(&span, auth.0);
    let request = tanren_contract::CreateOrganizationRequest::from_api(auth.1, auth.0, body);
    match state
        .handlers
        .create_organization(state.store.as_ref(), request)
        .await
    {
        Ok(response) => {
            emit_route_success(
                "create_organization",
                auth.0,
                Some(response.organization.id),
            );
            (StatusCode::CREATED, Json(response)).into_response()
        }
        Err(err) => {
            emit_route_failure("create_organization", auth.0, None, &err);
            map_organization_app_error(err)
        }
    }
}

#[utoipa::path(
    get,
    path = "/organizations",
    params(ListOrganizationsApiQuery),
    responses(
        (status = 200, body = ListOrganizationsResponse, description = "Organization list"),
        (status = 400, body = OrganizationFailureBody, description = "validation_failed"),
        (status = 401, body = OrganizationFailureBody, description = "auth_required"),
        (status = 500, body = OrganizationFailureBody, description = "internal_error"),
    ),
    tag = "organizations",
)]
pub(crate) async fn list_organizations_route(
    State(state): State<AppState>,
    session: Session,
    Query(query): Query<ListOrganizationsApiQuery>,
) -> Response {
    let span = organization_route_span("list_organizations", None);
    let _span_guard = span.enter();
    let auth = match require_authoritative_auth(&state, &session).await {
        Ok(auth) => auth,
        Err(response) => {
            emit_route_auth_denial("list_organizations", None);
            return response;
        }
    };
    record_authenticated_account(&span, auth.0);
    let request = tanren_contract::ListOrganizationsRequest::from_api_query(auth.1, auth.0, &query);
    match state
        .handlers
        .list_organizations(state.store.as_ref(), request)
        .await
    {
        Ok(response) => {
            emit_route_success("list_organizations", auth.0, None);
            (StatusCode::OK, Json(response)).into_response()
        }
        Err(err) => {
            emit_route_failure("list_organizations", auth.0, None, &err);
            map_organization_app_error(err)
        }
    }
}

#[utoipa::path(
    post,
    path = "/organizations/permissions/check",
    request_body = CheckOrganizationPermissionApiRequest,
    responses(
        (status = 200, body = CheckOrganizationPermissionResponse, description = "Permission present"),
        (status = 400, body = OrganizationFailureBody, description = "validation_failed"),
        (status = 401, body = OrganizationFailureBody, description = "auth_required"),
        (status = 403, body = OrganizationFailureBody, description = "permission_denied"),
        (status = 500, body = OrganizationFailureBody, description = "internal_error"),
    ),
    tag = "organizations",
)]
pub(crate) async fn check_organization_permission_route(
    State(state): State<AppState>,
    session: Session,
    OrganizationValidatedJson(body): OrganizationValidatedJson<
        CheckOrganizationPermissionApiRequest,
    >,
) -> Response {
    let org_id = body.org_id;
    let span = organization_route_span("check_organization_permission", Some(org_id));
    let _span_guard = span.enter();
    let auth = match require_authoritative_auth(&state, &session).await {
        Ok(auth) => auth,
        Err(response) => {
            emit_route_auth_denial("check_organization_permission", Some(org_id));
            return response;
        }
    };
    record_authenticated_account(&span, auth.0);
    let request =
        tanren_contract::CheckOrganizationPermissionRequest::from_api(auth.1, auth.0, &body);
    match state
        .handlers
        .check_organization_permission(state.store.as_ref(), request)
        .await
    {
        Ok(response) => {
            emit_route_success("check_organization_permission", auth.0, Some(org_id));
            (StatusCode::OK, Json(response)).into_response()
        }
        Err(err) => {
            emit_route_failure("check_organization_permission", auth.0, Some(org_id), &err);
            map_organization_app_error(err)
        }
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

/// Build the `OpenApiRouter` carrying every account-flow route. Called
/// from `lib.rs::build_app` after the cookie/CORS layers are
/// constructed; the macros that `routes!()` expands need to live in the
/// same module as the `#[utoipa::path]`-annotated handlers, so the
/// router constructor lives here too.
pub(crate) fn build_router(state: AppState) -> OpenApiRouter {
    OpenApiRouter::with_openapi(openapi_document())
        .routes(routes!(health_route))
        .routes(routes!(sign_up_route))
        .routes(routes!(sign_in_route))
        .routes(routes!(accept_invitation_route))
        .routes(routes!(create_organization_route))
        .routes(routes!(list_organizations_route))
        .routes(routes!(check_organization_permission_route))
        .routes(routes!(members::list_organization_members_route))
        .routes(routes!(revoke_route))
        .with_state(state)
}

/// Materialize the canonical `OpenAPI` document from route metadata and
/// contract schemas.
pub(crate) fn openapi_document() -> utoipa::openapi::OpenApi {
    ApiDoc::openapi()
}
