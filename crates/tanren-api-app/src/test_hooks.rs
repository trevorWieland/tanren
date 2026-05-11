//! Test-only HTTP routes mounted under `/test-hooks/*`.
//!
//! These exist solely to give the Playwright (`@web`) BDD runner the
//! same fixture-seeding seam that the Rust BDD harness already has via
//! direct `Arc<Store>` access. The Playwright runner cannot share a
//! process with the api binary, so it cannot reach `Store::seed_*`
//! through Rust — it has to talk over the wire.
//!
//! The whole module sits behind the `test-hooks` Cargo feature. The
//! production `tanren-api` binary does not enable that feature, so the
//! `/test-hooks/*` routes are simply absent from the production router
//! (no runtime guard, no env-var check — the routes do not compile in).
//!
//! The endpoints here are deliberately permissive (no auth, no rate
//! limiting): the contract is that they are loopback-only, gated by a
//! test-only Cargo feature, and exercised exclusively by the BDD
//! `globalSetup` flow that just spawned the binary.

use std::sync::Arc;

use axum::Json;
use axum::Router;
use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::post;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tanren_identity_policy::{AccountId, DesignatedHost, InvitationToken, OrgId, RepositoryRef};
use tanren_store::{NewInvitation, Store};
use uuid::Uuid;

/// Request body for `POST /test-hooks/invitations`.
#[derive(Debug, Deserialize)]
pub(crate) struct SeedInvitationBody {
    /// Opaque invitation token. Must round-trip through
    /// [`InvitationToken::parse`] (i.e. obey the same length/charset
    /// rules that production tokens do).
    pub token: String,
    /// Optional inviting org UUID. Omit to let the seeder allocate a
    /// fresh `OrgId` — most BDD scenarios don't care which org the
    /// invitee joins, only that they joined *some* org.
    #[serde(default)]
    pub inviting_org_id: Option<Uuid>,
    /// Wall-clock expiry instant in ISO 8601. May be in the past for
    /// expired-invitation falsification scenarios.
    pub expires_at: DateTime<Utc>,
}

#[cfg(not(feature = "test-hooks"))]
pub(crate) async fn seed_invitation_route(
    State(store): State<Arc<Store>>,
    Json(body): Json<SeedInvitationBody>,
) -> Result<StatusCode, (StatusCode, String)> {
    let token = InvitationToken::parse(&body.token)
        .map_err(|err| (StatusCode::BAD_REQUEST, err.to_string()))?;
    let inviting_org_id = body.inviting_org_id.map_or_else(OrgId::fresh, OrgId::new);
    store
        .seed_invitation(NewInvitation {
            token,
            inviting_org_id,
            expires_at: body.expires_at,
        })
        .await
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?;
    Ok(StatusCode::CREATED)
}

/// Build the `/test-hooks/*` router. The state is the shared
/// `Arc<Store>` already constructed by `build_app` / `build_app_with_store`.
#[cfg(not(feature = "test-hooks"))]
pub(crate) fn router(store: Arc<Store>) -> Router {
    Router::new()
        .route("/test-hooks/invitations", post(seed_invitation_route))
        .with_state(store)
}

// ── Feature-gated test-hooks surface ────────────────────────────────
//
// Everything below compiles only when the `test-hooks` Cargo feature is
// enabled. The production `tanren-api` binary does not enable it, so
// none of these routes appear in the production router.

#[cfg(feature = "test-hooks")]
use crate::cookies::session_account;
#[cfg(feature = "test-hooks")]
use crate::errors::map_app_error;
#[cfg(feature = "test-hooks")]
use axum::response::IntoResponse;
#[cfg(feature = "test-hooks")]
use sea_orm::ConnectionTrait;
#[cfg(feature = "test-hooks")]
use tanren_app_services::Handlers;
#[cfg(feature = "test-hooks")]
use tanren_app_services::project::ConnectExistingRepositoryCommand;
#[cfg(feature = "test-hooks")]
use tanren_contract::{ConnectProjectRepositoryRequest, ProjectFailureBody};
#[cfg(feature = "test-hooks")]
use tanren_provider_integrations::FixtureSourceControlProvider;
#[cfg(feature = "test-hooks")]
use tower_sessions::Session;

#[cfg(feature = "test-hooks")]
#[derive(Debug, Clone)]
pub(crate) struct TestHooksState {
    pub(crate) store: Arc<Store>,
    pub(crate) source_control: FixtureSourceControlProvider,
}

#[cfg(feature = "test-hooks")]
#[derive(Debug, Deserialize)]
pub(crate) struct SetRepositoryAccessBody {
    pub account_id: Uuid,
    pub repository: String,
    pub allowed: bool,
}

#[cfg(feature = "test-hooks")]
#[derive(Debug, Deserialize)]
pub(crate) struct SetHostCreateAccessBody {
    pub account_id: Uuid,
    pub host: String,
    pub allowed: bool,
}

#[cfg(feature = "test-hooks")]
#[derive(Debug, Deserialize)]
pub(crate) struct RepositoryCreatedBody {
    pub host: String,
    pub repository: String,
}

#[cfg(feature = "test-hooks")]
#[derive(Debug, Serialize)]
pub(crate) struct RepositoryCreatedResponse {
    pub created: bool,
}

/// Serializable snapshot of the fixture source-control call counters.
/// Field names use `serde(rename)` to emit the full counter names that
/// consumers expect while keeping the Rust identifiers distinct enough
/// to satisfy `clippy::struct_field_names`.
#[cfg(feature = "test-hooks")]
#[derive(Debug, Serialize)]
pub(crate) struct CallCountersResponse {
    #[serde(rename = "preflight_connect_repository")]
    pub connect_preflight: u64,
    #[serde(rename = "preflight_create_repository")]
    pub create_preflight: u64,
    #[serde(rename = "create_repository")]
    pub created: u64,
    #[serde(rename = "delete_repository")]
    pub deleted: u64,
}

/// Request body for `POST /test-hooks/projects/connect-as-cross-account`.
#[cfg(feature = "test-hooks")]
#[derive(Debug, Deserialize)]
pub(crate) struct ConnectAsCrossAccountBody {
    pub owning_account_id: Uuid,
    pub repository: String,
    #[serde(default)]
    pub select_as_active: bool,
}

#[cfg(feature = "test-hooks")]
pub(crate) async fn set_repository_access_route(
    State(state): State<TestHooksState>,
    Json(body): Json<SetRepositoryAccessBody>,
) -> Result<StatusCode, (StatusCode, String)> {
    let repository = RepositoryRef::parse(&body.repository)
        .map_err(|err| (StatusCode::BAD_REQUEST, err.to_string()))?;
    state.source_control.set_repository_access(
        AccountId::new(body.account_id),
        repository,
        body.allowed,
    );
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(feature = "test-hooks")]
pub(crate) async fn set_host_create_access_route(
    State(state): State<TestHooksState>,
    Json(body): Json<SetHostCreateAccessBody>,
) -> Result<StatusCode, (StatusCode, String)> {
    let host = DesignatedHost::parse(&body.host)
        .map_err(|err| (StatusCode::BAD_REQUEST, err.to_string()))?;
    state.source_control.set_host_reachable(&host, true);
    state.source_control.set_host_create_access(
        AccountId::new(body.account_id),
        &host,
        body.allowed,
    );
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(feature = "test-hooks")]
pub(crate) async fn repository_created_route(
    State(state): State<TestHooksState>,
    Json(body): Json<RepositoryCreatedBody>,
) -> Result<Json<RepositoryCreatedResponse>, (StatusCode, String)> {
    let host = DesignatedHost::parse(&body.host)
        .map_err(|err| (StatusCode::BAD_REQUEST, err.to_string()))?;
    let repository = RepositoryRef::parse(&body.repository)
        .map_err(|err| (StatusCode::BAD_REQUEST, err.to_string()))?;
    Ok(Json(RepositoryCreatedResponse {
        created: state
            .source_control
            .repository_created_at_host(&host, &repository),
    }))
}

/// Return the current fixture source-control call counters.
#[cfg(feature = "test-hooks")]
pub(crate) async fn source_control_call_counters_route(
    State(state): State<TestHooksState>,
) -> Json<CallCountersResponse> {
    let counters = state.source_control.call_counters();
    Json(CallCountersResponse {
        connect_preflight: counters.preflight_connect_repository,
        create_preflight: counters.preflight_create_repository,
        created: counters.create_repository,
        deleted: counters.delete_repository,
    })
}

/// Drop the `projects` table so subsequent project operations fail with
/// an internal-error envelope. Mirrors the
/// `Store::connection().execute_unprepared("DROP TABLE IF EXISTS projects")`
/// pattern used by the in-process Rust BDD harness.
#[cfg(feature = "test-hooks")]
pub(crate) async fn break_project_store_route(
    State(state): State<TestHooksState>,
) -> Result<StatusCode, (StatusCode, String)> {
    state
        .store
        .connection()
        .execute_unprepared("DROP TABLE IF EXISTS projects")
        .await
        .map_err(|err| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("drop projects table: {err}"),
            )
        })?;
    Ok(StatusCode::NO_CONTENT)
}

/// Connect an existing repository where the session actor differs from
/// the owning account. The cookie production route intentionally
/// collapses `actor_account_id` and `owning_account_id`; this endpoint
/// lets the Playwright BDD runner exercise the cross-account path by
/// deriving `actor_account_id` from the session while accepting
/// `owning_account_id` in the request body.
#[cfg(feature = "test-hooks")]
pub(crate) async fn connect_as_cross_account_route(
    State(state): State<TestHooksState>,
    session: Session,
    Json(body): Json<ConnectAsCrossAccountBody>,
) -> axum::response::Response {
    let actor_account_id = match session_actor_account_id(&session).await {
        Ok(id) => id,
        Err(response) => return response,
    };
    let repository = match RepositoryRef::parse(&body.repository) {
        Ok(repo) => repo,
        Err(err) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ProjectFailureBody::validation_failed(err.to_string())),
            )
                .into_response();
        }
    };
    let command = ConnectExistingRepositoryCommand::new(
        actor_account_id,
        ConnectProjectRepositoryRequest {
            owning_account_id: AccountId::new(body.owning_account_id),
            repository,
            select_as_active: body.select_as_active,
        },
    );
    let handlers = Handlers::new();
    match handlers
        .connect_project_repository(state.store.as_ref(), &state.source_control, command)
        .await
    {
        Ok(response) => (StatusCode::CREATED, Json(response)).into_response(),
        Err(err) => map_app_error(err),
    }
}

#[cfg(feature = "test-hooks")]
async fn session_actor_account_id(
    session: &Session,
) -> Result<AccountId, axum::response::Response> {
    match session_account(session).await {
        Ok(Some(context)) => Ok(context.account_id),
        Ok(None) => Err(auth_required()),
        Err(err) => {
            tracing::error!(target: "tanren_api", error = %err, "session read");
            Err(auth_required())
        }
    }
}

#[cfg(feature = "test-hooks")]
fn auth_required() -> axum::response::Response {
    use tanren_contract::ProjectFailureReason;
    let reason = ProjectFailureReason::AuthRequired;
    (
        StatusCode::from_u16(reason.http_status()).unwrap_or(StatusCode::UNAUTHORIZED),
        Json(ProjectFailureBody::from_reason(reason)),
    )
        .into_response()
}

#[cfg(feature = "test-hooks")]
pub(crate) fn router_with_source_control(
    store: Arc<Store>,
    source_control: FixtureSourceControlProvider,
) -> Router {
    let state = TestHooksState {
        store,
        source_control,
    };
    Router::new()
        .route(
            "/test-hooks/invitations",
            post(seed_invitation_route_with_state),
        )
        .route(
            "/test-hooks/source-control/call-counters",
            post(source_control_call_counters_route),
        )
        .route(
            "/test-hooks/source-control/repository-access",
            post(set_repository_access_route),
        )
        .route(
            "/test-hooks/source-control/host-create-access",
            post(set_host_create_access_route),
        )
        .route(
            "/test-hooks/source-control/repository-created",
            post(repository_created_route),
        )
        .route(
            "/test-hooks/projects/break-store",
            post(break_project_store_route),
        )
        .route(
            "/test-hooks/projects/connect-as-cross-account",
            post(connect_as_cross_account_route),
        )
        .with_state(state)
}

/// Thin wrapper that extracts [`TestHooksState`] and forwards the store,
/// so all test-hooks routes share a single state type.
#[cfg(feature = "test-hooks")]
pub(crate) async fn seed_invitation_route_with_state(
    State(state): State<TestHooksState>,
    Json(body): Json<SeedInvitationBody>,
) -> Result<StatusCode, (StatusCode, String)> {
    let token = InvitationToken::parse(&body.token)
        .map_err(|err| (StatusCode::BAD_REQUEST, err.to_string()))?;
    let inviting_org_id = body.inviting_org_id.map_or_else(OrgId::fresh, OrgId::new);
    state
        .store
        .seed_invitation(NewInvitation {
            token,
            inviting_org_id,
            expires_at: body.expires_at,
        })
        .await
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?;
    Ok(StatusCode::CREATED)
}
