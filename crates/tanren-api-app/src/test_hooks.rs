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
use secrecy::SecretString;
use serde::Deserialize;
use tanren_identity_policy::{
    AccountId, Argon2idVerifier, CredentialVerifier, Email, Identifier, InvitationToken, LoopId,
    MilestoneId, OrgId, ProjectId, SpecId,
};
use tanren_store::{
    AccountStore, NewAccount, NewInvitation, NewLoop, NewMilestone, NewProject, NewSpec,
    ProjectStore, Store,
};
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

/// Request body for `POST /test-hooks/accounts`.
#[derive(Debug, Deserialize)]
pub(crate) struct SeedAccountBody {
    /// Optional stable id. Omit to let the seeder allocate a fresh
    /// `AccountId`.
    #[serde(default)]
    pub id: Option<Uuid>,
    pub email: String,
    pub password: String,
    pub display_name: String,
    #[serde(default)]
    pub org_id: Option<Uuid>,
}

pub(crate) async fn seed_account_route(
    State(store): State<Arc<Store>>,
    Json(body): Json<SeedAccountBody>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let verifier = Argon2idVerifier::fast_for_tests();
    let phc = verifier
        .hash(&SecretString::from(body.password))
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let email = Email::parse(&body.email).map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    let id = body.id.map_or_else(AccountId::fresh, AccountId::new);
    let account = store
        .insert_account(NewAccount {
            id,
            identifier: Identifier::from_email(&email),
            display_name: body.display_name,
            password_phc: phc,
            created_at: Utc::now(),
            org_id: body.org_id.map(OrgId::new),
        })
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(
        serde_json::json!({ "account_id": account.id.to_string() }),
    ))
}

/// Request body for `POST /test-hooks/projects`.
#[derive(Debug, Deserialize)]
pub(crate) struct SeedProjectBody {
    #[serde(default)]
    pub id: Option<Uuid>,
    pub account_id: Uuid,
    pub name: String,
    #[serde(default = "default_active_state")]
    pub state: String,
}

fn default_active_state() -> String {
    "active".to_owned()
}

pub(crate) async fn seed_project_route(
    State(store): State<Arc<Store>>,
    Json(body): Json<SeedProjectBody>,
) -> Result<StatusCode, (StatusCode, String)> {
    store
        .seed_project(NewProject {
            id: body.id.map_or_else(ProjectId::fresh, ProjectId::new),
            account_id: AccountId::new(body.account_id),
            name: body.name,
            state: body.state,
            created_at: Utc::now(),
        })
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(StatusCode::CREATED)
}

/// Request body for `POST /test-hooks/specs`.
#[derive(Debug, Deserialize)]
pub(crate) struct SeedSpecBody {
    #[serde(default)]
    pub id: Option<Uuid>,
    pub project_id: Uuid,
    pub name: String,
    #[serde(default)]
    pub needs_attention: bool,
    pub attention_reason: Option<String>,
}

pub(crate) async fn seed_spec_route(
    State(store): State<Arc<Store>>,
    Json(body): Json<SeedSpecBody>,
) -> Result<StatusCode, (StatusCode, String)> {
    store
        .seed_spec(NewSpec {
            id: body.id.map_or_else(SpecId::fresh, SpecId::new),
            project_id: ProjectId::new(body.project_id),
            name: body.name,
            needs_attention: body.needs_attention,
            attention_reason: body.attention_reason,
            created_at: Utc::now(),
        })
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(StatusCode::CREATED)
}

/// Request body for `POST /test-hooks/loops`.
#[derive(Debug, Deserialize)]
pub(crate) struct SeedLoopBody {
    #[serde(default)]
    pub id: Option<Uuid>,
    pub project_id: Uuid,
}

pub(crate) async fn seed_loop_route(
    State(store): State<Arc<Store>>,
    Json(body): Json<SeedLoopBody>,
) -> Result<StatusCode, (StatusCode, String)> {
    store
        .seed_loop(NewLoop {
            id: body.id.map_or_else(LoopId::fresh, LoopId::new),
            project_id: ProjectId::new(body.project_id),
            created_at: Utc::now(),
        })
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(StatusCode::CREATED)
}

/// Request body for `POST /test-hooks/milestones`.
#[derive(Debug, Deserialize)]
pub(crate) struct SeedMilestoneBody {
    #[serde(default)]
    pub id: Option<Uuid>,
    pub project_id: Uuid,
}

pub(crate) async fn seed_milestone_route(
    State(store): State<Arc<Store>>,
    Json(body): Json<SeedMilestoneBody>,
) -> Result<StatusCode, (StatusCode, String)> {
    store
        .seed_milestone(NewMilestone {
            id: body.id.map_or_else(MilestoneId::fresh, MilestoneId::new),
            project_id: ProjectId::new(body.project_id),
            created_at: Utc::now(),
        })
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(StatusCode::CREATED)
}

/// Request body for `POST /test-hooks/view-states`.
#[derive(Debug, Deserialize)]
pub(crate) struct SeedViewStateBody {
    pub account_id: Uuid,
    pub project_id: Uuid,
    pub view_state: serde_json::Value,
}

pub(crate) async fn seed_view_state_route(
    State(store): State<Arc<Store>>,
    Json(body): Json<SeedViewStateBody>,
) -> Result<StatusCode, (StatusCode, String)> {
    store
        .write_view_state(
            AccountId::new(body.account_id),
            ProjectId::new(body.project_id),
            body.view_state,
            Utc::now(),
        )
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(StatusCode::CREATED)
}

/// Build the `/test-hooks/*` router. The state is the shared
/// `Arc<Store>` already constructed by `build_app` / `build_app_with_store`.
pub(crate) fn router(store: Arc<Store>) -> Router {
    Router::new()
        .route("/test-hooks/invitations", post(seed_invitation_route))
        .route("/test-hooks/accounts", post(seed_account_route))
        .route("/test-hooks/projects", post(seed_project_route))
        .route("/test-hooks/specs", post(seed_spec_route))
        .route("/test-hooks/loops", post(seed_loop_route))
        .route("/test-hooks/milestones", post(seed_milestone_route))
        .route("/test-hooks/view-states", post(seed_view_state_route))
        .with_state(store)
}
