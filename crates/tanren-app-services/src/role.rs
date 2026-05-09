//! Role-template lifecycle and permission-check handlers.
//!
//! Role templates are permission bundles, not authorization principals.
//! Applying a role snapshots its current permissions into direct grants.

use tanren_contract::{
    ApplyRoleRequest, ApplyRoleResponse, CreateRoleRequest, CreateRoleResponse, DeleteRoleRequest,
    DeleteRoleResponse, EditRoleRequest, EditRoleResponse, PermissionCheckRequest,
    PermissionCheckResponse, PermissionGrantView, RoleFailureReason, RoleTemplateView,
};
use tanren_identity_policy::{PrincipalRef, RoleId};
use tanren_store::{
    AccountStore, ApplyRole, ApplyRoleError, CreateRoleError, EditRole, EditRoleError, NewRole,
    RoleRecord, RoleStore,
};

use crate::events::{
    AuthorizationPrincipalRejected, RoleApplied, RoleCreated, RoleDeleted, RoleEdited, RoleKind,
    role_envelope,
};
use crate::{Clock, RoleServiceError};

pub(crate) async fn create_role<S>(
    store: &S,
    clock: &Clock,
    request: CreateRoleRequest,
) -> Result<CreateRoleResponse, RoleServiceError>
where
    S: RoleStore + AccountStore + ?Sized,
{
    let now = clock.now();
    let role = store
        .create_role(NewRole {
            id: RoleId::fresh(),
            scope: request.scope,
            name: request.name,
            permissions: request.permissions,
            created_at: now,
            updated_at: now,
        })
        .await
        .map_err(map_create_role_error)?;
    store
        .append_event(
            role_envelope(
                RoleKind::Created,
                &RoleCreated {
                    role: role.scoped_role(),
                    name: role.name.clone(),
                    permissions: role.permissions.clone(),
                    created_at: role.created_at,
                },
            ),
            now,
        )
        .await?;
    Ok(CreateRoleResponse {
        role: role_view(role),
    })
}

pub(crate) async fn edit_role<S>(
    store: &S,
    clock: &Clock,
    request: EditRoleRequest,
) -> Result<EditRoleResponse, RoleServiceError>
where
    S: RoleStore + AccountStore + ?Sized,
{
    let now = clock.now();
    let role = store
        .edit_role(EditRole {
            role: request.role,
            name: request.name,
            permissions: request.permissions,
            updated_at: now,
        })
        .await
        .map_err(map_edit_role_error)?;
    store
        .append_event(
            role_envelope(
                RoleKind::Edited,
                &RoleEdited {
                    role: role.scoped_role(),
                    name: role.name.clone(),
                    permissions: role.permissions.clone(),
                    edited_at: role.updated_at,
                },
            ),
            now,
        )
        .await?;
    Ok(EditRoleResponse {
        role: role_view(role),
    })
}

pub(crate) async fn delete_role<S>(
    store: &S,
    clock: &Clock,
    request: DeleteRoleRequest,
) -> Result<DeleteRoleResponse, RoleServiceError>
where
    S: RoleStore + AccountStore + ?Sized,
{
    let now = clock.now();
    let deleted = store.delete_role(request.role).await?;
    if !deleted {
        return Err(RoleServiceError::Role(RoleFailureReason::NotFound));
    }
    store
        .append_event(
            role_envelope(
                RoleKind::Deleted,
                &RoleDeleted {
                    role: request.role,
                    deleted_at: now,
                },
            ),
            now,
        )
        .await?;
    Ok(DeleteRoleResponse { role: request.role })
}

pub(crate) async fn apply_role<S>(
    store: &S,
    clock: &Clock,
    request: ApplyRoleRequest,
) -> Result<ApplyRoleResponse, RoleServiceError>
where
    S: RoleStore + AccountStore + ?Sized,
{
    if matches!(request.principal, PrincipalRef::Role { .. }) {
        return Err(RoleServiceError::Role(
            RoleFailureReason::RoleAsPrincipalRejected,
        ));
    }
    let now = clock.now();
    let grants = store
        .apply_role(ApplyRole {
            role: request.role,
            principal: request.principal,
            grant_scope: request.grant_scope,
            // Until interface auth context is threaded in, grant provenance
            // is the grantee principal that initiated self-application.
            granted_by: request.principal,
            granted_at: now,
        })
        .await
        .map_err(map_apply_role_error)?;
    let grant_views = grants.iter().cloned().map(grant_view).collect::<Vec<_>>();
    store
        .append_event(
            role_envelope(
                RoleKind::Applied,
                &RoleApplied {
                    role: request.role,
                    principal: request.principal,
                    grant_scope: request.grant_scope,
                    grant_ids: grants.iter().map(|grant| grant.id).collect::<Vec<_>>(),
                    permissions: grants
                        .iter()
                        .map(|grant| grant.permission.clone())
                        .collect::<Vec<_>>(),
                    applied_at: now,
                },
            ),
            now,
        )
        .await?;
    Ok(ApplyRoleResponse {
        role: request.role,
        grants: grant_views,
    })
}

pub(crate) async fn check_permission<S>(
    store: &S,
    clock: &Clock,
    request: PermissionCheckRequest,
) -> Result<PermissionCheckResponse, RoleServiceError>
where
    S: RoleStore + AccountStore + ?Sized,
{
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

    let grants = store
        .find_direct_grants(request.principal, request.scope, &request.permission)
        .await?;
    let matching_grant_ids = grants.iter().map(|grant| grant.id).collect::<Vec<_>>();
    Ok(PermissionCheckResponse {
        principal: request.principal,
        permission: request.permission,
        scope: request.scope,
        allowed: !matching_grant_ids.is_empty(),
        matching_grant_ids,
    })
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
        ApplyRoleError::RoleNotFound => RoleServiceError::Role(RoleFailureReason::NotFound),
        ApplyRoleError::Store(store_err) => RoleServiceError::Store(store_err),
    }
}

fn role_view(record: RoleRecord) -> RoleTemplateView {
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
        source_role_id: record.source_role_id,
        granted_at: record.granted_at,
    }
}
