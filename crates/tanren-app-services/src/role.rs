//! Role-template lifecycle and permission-check handlers.
//! Role templates are permission bundles, not authorization principals.

use tanren_contract::{
    ApplyRoleRequest, ApplyRoleResponse, CreateRoleRequest, CreateRoleResponse, DeleteRoleRequest,
    DeleteRoleResponse, EditRoleRequest, EditRoleResponse, MAX_ROLE_TEMPLATE_PERMISSIONS,
    PermissionCheckRequest, PermissionCheckResponse, ROLE_TEMPLATE_ALLOW_EMPTY_BUNDLE, RoleActor,
    RoleAdminAction, RoleAdminCapabilities, RoleFailureReason,
};
use tanren_identity_policy::{PermissionName, PermissionScope, PrincipalRef, RoleId, RoleScope};
use tanren_store::{
    AccountStore, ApplyRole, ApplyRoleError, CreateRoleError, EditRole, EditRoleError, NewRole,
    RoleStore,
};

use crate::events::{
    AuthorizationPrincipalRejected, RoleKind, role_applied_event_builder,
    role_created_event_builder, role_deleted_event_builder, role_edited_event_builder,
    role_envelope,
};
use crate::role_authorization::{
    RoleAdminCapabilityPort, authorize_manage_permission_scope, authorize_manage_role_scope,
    authorize_read_permission_scope, role_manage_permission, role_read_permission,
};
use crate::role_view_mapper::{permission_grant_view, role_template_view};
use crate::{Clock, RoleServiceError};

pub(crate) async fn create_role<S>(
    store: &S,
    clock: &Clock,
    actor: RoleActor,
    request: CreateRoleRequest,
) -> Result<CreateRoleResponse, RoleServiceError>
where
    S: RoleStore + AccountStore + ?Sized,
{
    authorize_manage_role_scope(store, actor.account_id, request.scope).await?;
    ensure_role_scope_exists(store, request.scope).await?;
    validate_role_permissions_bundle(&request.permissions)?;
    let now = clock.now();
    let role = store
        .create_role_atomic(
            NewRole {
                id: RoleId::fresh(),
                scope: request.scope,
                name: request.name,
                permissions: request.permissions,
                created_at: now,
                updated_at: now,
            },
            role_created_event_builder(),
            now,
        )
        .await
        .map_err(map_create_role_error)?;
    Ok(CreateRoleResponse {
        role: role_template_view(role),
    })
}

pub(crate) async fn edit_role<S>(
    store: &S,
    clock: &Clock,
    actor: RoleActor,
    request: EditRoleRequest,
) -> Result<EditRoleResponse, RoleServiceError>
where
    S: RoleStore + AccountStore + ?Sized,
{
    authorize_manage_role_scope(store, actor.account_id, request.role.scope).await?;
    validate_role_permissions_bundle(&request.permissions)?;
    let now = clock.now();
    let role = store
        .edit_role_atomic(
            EditRole {
                role: request.role,
                name: request.name,
                permissions: request.permissions,
                updated_at: now,
            },
            role_edited_event_builder(),
            now,
        )
        .await
        .map_err(map_edit_role_error)?;
    Ok(EditRoleResponse {
        role: role_template_view(role),
    })
}

pub(crate) async fn delete_role<S>(
    store: &S,
    clock: &Clock,
    actor: RoleActor,
    request: DeleteRoleRequest,
) -> Result<DeleteRoleResponse, RoleServiceError>
where
    S: RoleStore + AccountStore + ?Sized,
{
    authorize_manage_role_scope(store, actor.account_id, request.role.scope).await?;
    let now = clock.now();
    let deleted = store
        .delete_role_atomic(
            request.role,
            role_deleted_event_builder(request.role, now),
            now,
        )
        .await?;
    if !deleted {
        return Err(RoleServiceError::Role(RoleFailureReason::NotFound));
    }
    Ok(DeleteRoleResponse { role: request.role })
}

pub(crate) async fn apply_role<S>(
    store: &S,
    clock: &Clock,
    actor: RoleActor,
    request: ApplyRoleRequest,
) -> Result<ApplyRoleResponse, RoleServiceError>
where
    S: RoleStore + AccountStore + ?Sized,
{
    authorize_manage_permission_scope(store, actor.account_id, request.grant_scope).await?;
    ensure_role_scope_exists(store, request.role.scope).await?;
    ensure_permission_scope_exists(store, request.grant_scope).await?;
    if matches!(request.principal, PrincipalRef::Role { .. }) {
        return Err(RoleServiceError::Role(
            RoleFailureReason::RoleAsPrincipalRejected,
        ));
    }
    ensure_principal_exists(store, request.principal).await?;
    if !request.role.scope.allows_grant_scope(request.grant_scope) {
        return Err(RoleServiceError::Role(RoleFailureReason::ValidationFailed));
    }
    let now = clock.now();
    let grants = store
        .apply_role_atomic(
            ApplyRole {
                role: request.role,
                principal: request.principal,
                grant_scope: request.grant_scope,
                granted_by: PrincipalRef::Account {
                    account_id: actor.account_id,
                },
                granted_at: now,
            },
            role_applied_event_builder(request.role, request.principal, request.grant_scope, now),
            now,
        )
        .await
        .map_err(map_apply_role_error)?;
    let grant_views = grants
        .iter()
        .cloned()
        .map(permission_grant_view)
        .collect::<Vec<_>>();
    Ok(ApplyRoleResponse {
        role: request.role,
        grants: grant_views,
    })
}

pub(crate) async fn check_permission<S>(
    store: &S,
    clock: &Clock,
    actor: RoleActor,
    request: PermissionCheckRequest,
) -> Result<PermissionCheckResponse, RoleServiceError>
where
    S: RoleStore + AccountStore + ?Sized,
{
    authorize_read_permission_scope(store, actor.account_id, request.scope).await?;
    ensure_permission_scope_exists(store, request.scope).await?;
    if matches!(request.principal, PrincipalRef::Role { .. }) {
        let now = clock.now();
        store
            .append_event(
                role_envelope(
                    RoleKind::AuthorizationPrincipalRejected,
                    &AuthorizationPrincipalRejected {
                        principal: request.principal,
                        permission: request.permission,
                        scope: request.scope,
                        reason: RoleFailureReason::RoleAsPrincipalRejected,
                        at: now,
                    },
                ),
                now,
            )
            .await?;
        return Err(RoleServiceError::Role(
            RoleFailureReason::RoleAsPrincipalRejected,
        ));
    }
    ensure_principal_exists(store, request.principal).await?;

    let matching_grant_ids = store
        .find_direct_grant_ids(request.principal, request.scope, &request.permission)
        .await?;
    Ok(PermissionCheckResponse {
        principal: request.principal,
        permission: request.permission,
        scope: request.scope,
        allowed: !matching_grant_ids.is_empty(),
        matching_grant_ids,
    })
}

pub(crate) async fn role_admin_capabilities<S>(
    store: &S,
    actor: RoleActor,
) -> Result<RoleAdminCapabilities, RoleServiceError>
where
    S: RoleStore + ?Sized,
{
    let manage_permission = role_manage_permission()?;
    let read_permission = role_read_permission()?;
    let can_manage = store
        .actor_has_capability_any_scope(actor.account_id, &manage_permission)
        .await?;
    let can_read = store
        .actor_has_capability_any_scope(actor.account_id, &read_permission)
        .await?;
    let mut actions = Vec::new();
    if can_manage {
        actions.extend_from_slice(&[
            RoleAdminAction::CreateRole,
            RoleAdminAction::EditRole,
            RoleAdminAction::DeleteRole,
            RoleAdminAction::ApplyRole,
        ]);
    }
    if can_read || can_manage {
        actions.extend_from_slice(&[RoleAdminAction::ReadRoles, RoleAdminAction::CheckPermission]);
    }
    Ok(RoleAdminCapabilities { actor, actions })
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

fn validate_role_permissions_bundle(
    permissions: &[PermissionName],
) -> Result<(), RoleServiceError> {
    if permissions.is_empty() && !ROLE_TEMPLATE_ALLOW_EMPTY_BUNDLE {
        return Err(RoleServiceError::InvalidInput(
            "role templates must include at least one permission".to_owned(),
        ));
    }
    if permissions.len() > MAX_ROLE_TEMPLATE_PERMISSIONS {
        return Err(RoleServiceError::InvalidInput(format!(
            "role templates can include at most {MAX_ROLE_TEMPLATE_PERMISSIONS} permissions"
        )));
    }
    Ok(())
}

fn map_create_role_error(err: CreateRoleError) -> RoleServiceError {
    match err {
        CreateRoleError::DuplicateRoleName => RoleServiceError::Role(RoleFailureReason::Conflict),
        CreateRoleError::Store(store_err) => RoleServiceError::Store(store_err),
    }
}

fn map_edit_role_error(err: EditRoleError) -> RoleServiceError {
    match err {
        EditRoleError::RoleNotFound => RoleServiceError::Role(RoleFailureReason::NotFound),
        EditRoleError::DuplicateRoleName => RoleServiceError::Role(RoleFailureReason::Conflict),
        EditRoleError::Store(store_err) => RoleServiceError::Store(store_err),
    }
}

fn map_apply_role_error(err: ApplyRoleError) -> RoleServiceError {
    match err {
        ApplyRoleError::RoleNotFound
        | ApplyRoleError::PrincipalNotFound
        | ApplyRoleError::GrantScopeNotFound => RoleServiceError::Role(RoleFailureReason::NotFound),
        ApplyRoleError::RoleAsPrincipalRejected => {
            RoleServiceError::Role(RoleFailureReason::RoleAsPrincipalRejected)
        }
        ApplyRoleError::IncompatibleGrantScope => {
            RoleServiceError::Role(RoleFailureReason::ValidationFailed)
        }
        ApplyRoleError::Store(store_err) => RoleServiceError::Store(store_err),
    }
}
