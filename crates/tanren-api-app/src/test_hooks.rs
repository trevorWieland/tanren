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
use serde::Deserialize;
use tanren_identity_policy::{
    AccountId, Email, InvitationToken, OrgId, PermissionGrantSource, PermissionName,
    PolicyConstraintReason, PolicyConstraintSource, ProjectId, RoleTemplateName,
};
use tanren_store::{
    AccountStore, NewInvitation, NewPermissionConstraint, NewPermissionGrant, PermissionGrantScope,
    Store,
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

/// Request body for `POST /test-hooks/permission-grants`.
#[derive(Debug, Deserialize)]
pub(crate) struct SeedPermissionGrantBody {
    #[serde(default)]
    pub account_id: Option<AccountId>,
    #[serde(default)]
    pub account_email: Option<Email>,
    pub scope_kind: SeedPermissionScopeKind,
    pub scope_id: Uuid,
    pub permission: PermissionName,
    pub grant_source_kind: SeedPermissionGrantSourceKind,
    #[serde(default)]
    pub role_template_name: Option<RoleTemplateName>,
    #[serde(default)]
    pub policy_constraint: Option<SeedPolicyConstraintBody>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SeedPolicyConstraintBody {
    pub reason: PolicyConstraintReason,
    pub source: PolicyConstraintSource,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SeedPermissionScopeKind {
    Organization,
    Project,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SeedPermissionGrantSourceKind {
    Direct,
    RoleTemplate,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ResolveAccountBody {
    pub account_email: Email,
}

#[derive(Debug, serde::Serialize)]
pub(crate) struct ResolveAccountResponse {
    pub account_id: AccountId,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RecentEventsBody {
    #[serde(default)]
    pub limit: Option<u64>,
}

#[derive(Debug, serde::Serialize)]
pub(crate) struct RecentEventView {
    pub id: Uuid,
    pub kind: Option<String>,
}

#[derive(Debug, serde::Serialize)]
pub(crate) struct RecentEventsResponse {
    pub events: Vec<RecentEventView>,
}

pub(crate) async fn seed_permission_grant_route(
    State(store): State<Arc<Store>>,
    Json(body): Json<SeedPermissionGrantBody>,
) -> Result<StatusCode, (StatusCode, String)> {
    let account_id = if let Some(id) = body.account_id {
        id
    } else if let Some(email_raw) = body.account_email {
        let account = AccountStore::find_account_by_email(store.as_ref(), &email_raw)
            .await
            .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?
            .ok_or_else(|| {
                (
                    StatusCode::BAD_REQUEST,
                    format!("no account found for email '{email_raw}'"),
                )
            })?;
        account.id
    } else {
        return Err((
            StatusCode::BAD_REQUEST,
            "either account_id or account_email is required".to_owned(),
        ));
    };
    let scope = match body.scope_kind {
        SeedPermissionScopeKind::Organization => {
            PermissionGrantScope::Organization(OrgId::from(body.scope_id))
        }
        SeedPermissionScopeKind::Project => {
            PermissionGrantScope::Project(ProjectId::from(body.scope_id))
        }
    };
    let grant_source = match body.grant_source_kind {
        SeedPermissionGrantSourceKind::Direct => PermissionGrantSource::Direct,
        SeedPermissionGrantSourceKind::RoleTemplate => {
            let role_template = body.role_template_name.ok_or_else(|| {
                (
                    StatusCode::BAD_REQUEST,
                    "role_template_name is required for role_template grants".to_owned(),
                )
            })?;
            PermissionGrantSource::RoleTemplate { role_template }
        }
    };

    let grant = store
        .seed_permission_grant(NewPermissionGrant {
            account_id,
            scope,
            permission: body.permission,
            grant_source,
            created_at: Utc::now(),
        })
        .await
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?;

    if let Some(constraint) = body.policy_constraint {
        store
            .seed_permission_constraint(NewPermissionConstraint {
                grant_id: grant.id,
                reason: constraint.reason,
                source: constraint.source,
                created_at: Utc::now(),
            })
            .await
            .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?;
    }

    Ok(StatusCode::CREATED)
}

pub(crate) async fn resolve_account_route(
    State(store): State<Arc<Store>>,
    Json(body): Json<ResolveAccountBody>,
) -> Result<Json<ResolveAccountResponse>, (StatusCode, String)> {
    let account = AccountStore::find_account_by_email(store.as_ref(), &body.account_email)
        .await
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                format!("no account found for email '{}'", body.account_email),
            )
        })?;
    Ok(Json(ResolveAccountResponse {
        account_id: account.id,
    }))
}

pub(crate) async fn recent_events_route(
    State(store): State<Arc<Store>>,
    Json(body): Json<RecentEventsBody>,
) -> Result<Json<RecentEventsResponse>, (StatusCode, String)> {
    let limit = body.limit.unwrap_or(200);
    let events = AccountStore::recent_events(store.as_ref(), limit)
        .await
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?
        .into_iter()
        .map(|event| RecentEventView {
            id: event.id,
            kind: event
                .payload
                .get("kind")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned),
        })
        .collect();
    Ok(Json(RecentEventsResponse { events }))
}

/// Build the `/test-hooks/*` router. The state is the shared
/// `Arc<Store>` already constructed by `build_app` / `build_app_with_store`.
pub(crate) fn router(store: Arc<Store>) -> Router {
    Router::new()
        .route("/test-hooks/invitations", post(seed_invitation_route))
        .route("/test-hooks/accounts/resolve", post(resolve_account_route))
        .route("/test-hooks/events/recent", post(recent_events_route))
        .route(
            "/test-hooks/permission-grants",
            post(seed_permission_grant_route),
        )
        .with_state(store)
}
