//! Role administration read-model query handlers.

use tanren_contract::{
    PermissionGrantCursorView, ROLE_READ_MODEL_PAGE_DEFAULT, ROLE_READ_MODEL_PAGE_MAX, RoleActor,
    RoleFailureReason, RoleReadModelFreshness, RoleReadModelRequest, RoleReadModelResponse,
    RoleTemplateCursorView,
};
use tanren_identity_policy::{PermissionScope, PrincipalRef, RoleScope};
use tanren_store::{
    AccountStore, PermissionGrantListCursor, ROLE_GRANT_LIST_PAGE_MAX, RoleListCursor, RoleStore,
};

use crate::role_authorization::authorize_read_permission_scope;
use crate::role_view_mapper::{permission_grant_view, role_template_view};
use crate::{Clock, RoleServiceError};

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
            .map(role_template_view)
            .collect::<Vec<_>>(),
        role_next_cursor: role_page.next_cursor.map(role_cursor_to_view),
        direct_grants: grant_page
            .items
            .into_iter()
            .map(permission_grant_view)
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
