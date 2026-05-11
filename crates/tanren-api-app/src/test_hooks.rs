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
    AccountId, InvitationToken, OrgId, PermissionName, PermissionScope, PrincipalRef, RoleId,
    RoleName, RoleScope,
};
use tanren_store::{ApplyRole, NewInvitation, NewRole, RoleStore, Store};
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

/// Request body for `POST /test-hooks/role-admin-grants`.
#[derive(Debug, Deserialize)]
pub(crate) struct SeedRoleAdminGrantsBody {
    /// Authenticated actor account id receiving seeded role-admin grants.
    pub actor_account_id: AccountId,
    /// Role scope used for the temporary bootstrap role.
    pub scope: RoleScope,
    /// Permission names to include in the bootstrap role.
    pub permissions: Vec<String>,
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

pub(crate) async fn seed_role_admin_grants_route(
    State(store): State<Arc<Store>>,
    Json(body): Json<SeedRoleAdminGrantsBody>,
) -> Result<StatusCode, (StatusCode, String)> {
    if body.permissions.is_empty() {
        return Ok(StatusCode::CREATED);
    }
    let permissions = body
        .permissions
        .into_iter()
        .map(|permission| PermissionName::parse(&permission).map_err(|err| err.to_string()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| (StatusCode::BAD_REQUEST, err))?;
    let now = Utc::now();
    let role = store
        .create_role(NewRole {
            id: RoleId::fresh(),
            scope: body.scope,
            name: RoleName::parse("bdd-role-admin-bootstrap")
                .expect("test-hook role name literal must parse"),
            permissions,
            created_at: now,
            updated_at: now,
        })
        .await
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?;
    let grant_scope = match role.scope {
        RoleScope::Account { account_id } => PermissionScope::Account { account_id },
        RoleScope::Organization { org_id } => PermissionScope::Organization { org_id },
        RoleScope::Project { project_id } => PermissionScope::Project { project_id },
    };
    store
        .apply_role(ApplyRole {
            role: role.scoped_role(),
            principal: PrincipalRef::Account {
                account_id: body.actor_account_id,
            },
            grant_scope,
            granted_by: PrincipalRef::Account {
                account_id: body.actor_account_id,
            },
            granted_at: now,
        })
        .await
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?;
    Ok(StatusCode::CREATED)
}

/// Build the `/test-hooks/*` router. The state is the shared
/// `Arc<Store>` already constructed by `build_app` / `build_app_with_store`.
pub(crate) fn router(store: Arc<Store>) -> Router {
    Router::new()
        .route("/test-hooks/invitations", post(seed_invitation_route))
        .route(
            "/test-hooks/role-admin-grants",
            post(seed_role_admin_grants_route),
        )
        .with_state(store)
}
