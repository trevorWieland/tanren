//! Role administration read-model query handlers.

use tanren_contract::{
    PermissionGrantCursorView, PermissionGrantView, ROLE_READ_MODEL_PAGE_DEFAULT,
    ROLE_READ_MODEL_PAGE_MAX, RoleActor, RoleFailureReason, RoleReadModelFreshness,
    RoleReadModelRequest, RoleReadModelResponse, RoleTemplateCursorView, RoleTemplateView,
};
use tanren_identity_policy::{AccountId, PermissionName, PermissionScope, PrincipalRef, RoleScope};
use tanren_store::{
    AccountStore, PermissionGrantListCursor, ROLE_GRANT_LIST_PAGE_MAX, RoleListCursor, RoleStore,
};

use crate::{Clock, RoleServiceError};

const ROLE_MANAGE_PERMISSION: &str = "roles.manage";
const ROLE_READ_PERMISSION: &str = "roles.read";

pub(crate) async fn read_role_model<S>(
    store: &S,
    clock: &Clock,
    actor: RoleActor,
    request: RoleReadModelRequest,
) -> Result<RoleReadModelResponse, RoleServiceError>
where
    S: RoleStore + AccountStore + ?Sized,
{
    authorize_read_permission_scope(
        store,
        actor.account_id,
        request.role_scope.as_permission_scope(),
    )
    .await?;
    authorize_read_permission_scope(store, actor.account_id, request.grant_scope).await?;
    ensure_role_scope_exists(store, request.role_scope).await?;
    ensure_permission_scope_exists(store, request.grant_scope).await?;
    if matches!(request.grant_principal, PrincipalRef::Role { .. }) {
        return Err(RoleServiceError::Role(
            RoleFailureReason::RoleAsPrincipalRejected,
        ));
    }
    ensure_principal_exists(store, request.grant_principal).await?;

    let role_limit = normalize_page_limit(request.role_limit);
    let grant_limit = normalize_page_limit(request.grant_limit);

    let role_page = store
        .list_roles_page(
            request.role_scope,
            request.role_cursor.map(role_cursor_from_view),
            role_limit,
        )
        .await?;
    let grant_page = store
        .list_direct_grants_page(
            request.grant_principal,
            Some(request.grant_scope),
            request.grant_cursor.as_ref().map(grant_cursor_from_view),
            grant_limit,
        )
        .await?;

    Ok(RoleReadModelResponse {
        role_scope: request.role_scope,
        grant_principal: request.grant_principal,
        grant_scope: request.grant_scope,
        role_templates: role_page
            .items
            .into_iter()
            .map(role_view)
            .collect::<Vec<_>>(),
        role_next_cursor: role_page.next_cursor.map(role_cursor_to_view),
        direct_grants: grant_page
            .items
            .into_iter()
            .map(grant_view)
            .collect::<Vec<_>>(),
        grant_next_cursor: grant_page.next_cursor.as_ref().map(grant_cursor_to_view),
        freshness: RoleReadModelFreshness {
            observed_at: clock.now(),
        },
    })
}

fn normalize_page_limit(requested: Option<u64>) -> u64 {
    let max = ROLE_READ_MODEL_PAGE_MAX.min(ROLE_GRANT_LIST_PAGE_MAX);
    requested
        .filter(|limit| *limit > 0)
        .unwrap_or(ROLE_READ_MODEL_PAGE_DEFAULT)
        .min(max)
}

async fn authorize_read_permission_scope<S>(
    store: &S,
    actor: AccountId,
    scope: PermissionScope,
) -> Result<(), RoleServiceError>
where
    S: RoleStore + ?Sized,
{
    let can_read = store
        .has_direct_grant(
            PrincipalRef::Account { account_id: actor },
            scope,
            &role_read_permission()?,
        )
        .await?;
    if can_read {
        return Ok(());
    }
    let can_manage = store
        .has_direct_grant(
            PrincipalRef::Account { account_id: actor },
            scope,
            &role_manage_permission()?,
        )
        .await?;
    if can_manage {
        Ok(())
    } else {
        Err(RoleServiceError::Role(RoleFailureReason::PermissionDenied))
    }
}

fn role_manage_permission() -> Result<PermissionName, RoleServiceError> {
    PermissionName::parse(ROLE_MANAGE_PERMISSION)
        .map_err(|err| RoleServiceError::InvalidInput(format!("invalid static permission: {err}")))
}

fn role_read_permission() -> Result<PermissionName, RoleServiceError> {
    PermissionName::parse(ROLE_READ_PERMISSION)
        .map_err(|err| RoleServiceError::InvalidInput(format!("invalid static permission: {err}")))
}

async fn ensure_role_scope_exists<S>(store: &S, scope: RoleScope) -> Result<(), RoleServiceError>
where
    S: RoleStore + ?Sized,
{
    if store.role_scope_exists(scope).await? {
        Ok(())
    } else {
        Err(RoleServiceError::Role(RoleFailureReason::NotFound))
    }
}

async fn ensure_permission_scope_exists<S>(
    store: &S,
    scope: PermissionScope,
) -> Result<(), RoleServiceError>
where
    S: RoleStore + ?Sized,
{
    if store.permission_scope_exists(scope).await? {
        Ok(())
    } else {
        Err(RoleServiceError::Role(RoleFailureReason::NotFound))
    }
}

async fn ensure_principal_exists<S>(
    store: &S,
    principal: PrincipalRef,
) -> Result<(), RoleServiceError>
where
    S: RoleStore + ?Sized,
{
    if store.principal_exists(principal).await? {
        Ok(())
    } else {
        Err(RoleServiceError::Role(RoleFailureReason::NotFound))
    }
}

fn role_cursor_from_view(cursor: RoleTemplateCursorView) -> RoleListCursor {
    RoleListCursor {
        name: cursor.name,
        id: cursor.id,
    }
}

fn role_cursor_to_view(cursor: RoleListCursor) -> RoleTemplateCursorView {
    RoleTemplateCursorView {
        name: cursor.name,
        id: cursor.id,
    }
}

fn grant_cursor_from_view(cursor: &PermissionGrantCursorView) -> PermissionGrantListCursor {
    PermissionGrantListCursor {
        granted_at: cursor.granted_at,
        id: cursor.id,
    }
}

fn grant_cursor_to_view(cursor: &PermissionGrantListCursor) -> PermissionGrantCursorView {
    PermissionGrantCursorView {
        granted_at: cursor.granted_at,
        id: cursor.id,
    }
}

fn role_view(record: tanren_store::RoleRecord) -> RoleTemplateView {
    RoleTemplateView {
        id: record.id,
        scope: record.scope,
        name: record.name,
        permissions: record.permissions,
        created_at: record.created_at,
        updated_at: record.updated_at,
    }
}

fn grant_view(record: tanren_store::PermissionGrantRecord) -> PermissionGrantView {
    PermissionGrantView {
        id: record.id,
        principal: record.principal,
        scope: record.scope,
        permission: record.permission,
        source: record.source,
        revocation: record.revocation,
        granted_at: record.granted_at,
    }
}
