//! Role-template lifecycle and permission-check handlers.
//! Role templates are permission bundles, not authorization principals.

use tanren_contract::{
    ApplyRoleRequest, ApplyRoleResponse, CreateRoleRequest, CreateRoleResponse, DeleteRoleRequest,
    DeleteRoleResponse, EditRoleRequest, EditRoleResponse, MAX_ROLE_TEMPLATE_PERMISSIONS,
    PermissionCheckRequest, PermissionCheckResponse, PermissionGrantView,
    ROLE_TEMPLATE_ALLOW_EMPTY_BUNDLE, RoleActor, RoleAdminAction, RoleAdminCapabilities,
    RoleFailureReason, RoleTemplateView,
};
use tanren_identity_policy::{
    AccountId, PermissionName, PermissionScope, PrincipalRef, RoleId, RoleScope,
};
use tanren_store::{
    AccountStore, ApplyRole, ApplyRoleError, CreateRoleError, EditRole, EditRoleError, NewRole,
    RoleRecord, RoleStore, StoreError,
};

use crate::events::{
    AuthorizationPrincipalRejected, RoleApplied, RoleCreated, RoleDeleted, RoleEdited, RoleKind,
    role_envelope,
};
use crate::{Clock, RoleServiceError};

const ROLE_MANAGE_PERMISSION: &str = "roles.manage";
const ROLE_READ_PERMISSION: &str = "roles.read";

pub(crate) trait RoleAdminCapabilityPort {
    async fn actor_has_capability_in_scope(
        &self,
        actor: AccountId,
        scope: PermissionScope,
        permission: &PermissionName,
    ) -> Result<bool, StoreError>;

    async fn actor_has_capability_any_scope(
        &self,
        actor: AccountId,
        permission: &PermissionName,
    ) -> Result<bool, StoreError>;
}

impl<T> RoleAdminCapabilityPort for T
where
    T: RoleStore + ?Sized,
{
    async fn actor_has_capability_in_scope(
        &self,
        actor: AccountId,
        scope: PermissionScope,
        permission: &PermissionName,
    ) -> Result<bool, StoreError> {
        self.has_direct_grant(
            PrincipalRef::Account { account_id: actor },
            scope,
            permission,
        )
        .await
    }

    async fn actor_has_capability_any_scope(
        &self,
        actor: AccountId,
        permission: &PermissionName,
    ) -> Result<bool, StoreError> {
        let grants = self
            .list_all_direct_grants(PrincipalRef::Account { account_id: actor })
            .await?;
        Ok(grants.iter().any(|grant| grant.permission == *permission))
    }
}

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
    actor: RoleActor,
    request: DeleteRoleRequest,
) -> Result<DeleteRoleResponse, RoleServiceError>
where
    S: RoleStore + AccountStore + ?Sized,
{
    authorize_manage_role_scope(store, actor.account_id, request.role.scope).await?;
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
        .apply_role(ApplyRole {
            role: request.role,
            principal: request.principal,
            grant_scope: request.grant_scope,
            granted_by: PrincipalRef::Account {
                account_id: actor.account_id,
            },
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

async fn authorize_manage_role_scope<S>(
    store: &S,
    actor: AccountId,
    scope: RoleScope,
) -> Result<(), RoleServiceError>
where
    S: RoleStore + ?Sized,
{
    authorize_role_permission(
        store,
        actor,
        scope.as_permission_scope(),
        &role_manage_permission()?,
    )
    .await
}

async fn authorize_manage_permission_scope<S>(
    store: &S,
    actor: AccountId,
    scope: PermissionScope,
) -> Result<(), RoleServiceError>
where
    S: RoleStore + ?Sized,
{
    authorize_role_permission(store, actor, scope, &role_manage_permission()?).await
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
        .actor_has_capability_in_scope(actor, scope, &role_read_permission()?)
        .await?;
    if can_read {
        return Ok(());
    }
    authorize_manage_permission_scope(store, actor, scope).await
}

async fn authorize_role_permission<S>(
    store: &S,
    actor: AccountId,
    scope: PermissionScope,
    permission: &PermissionName,
) -> Result<(), RoleServiceError>
where
    S: RoleStore + ?Sized,
{
    let allowed = store
        .actor_has_capability_in_scope(actor, scope, permission)
        .await?;
    if allowed {
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
        ApplyRoleError::IncompatibleGrantScope => {
            RoleServiceError::Role(RoleFailureReason::ValidationFailed)
        }
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
        source: record.source,
        revocation: record.revocation,
        granted_at: record.granted_at,
    }
}
