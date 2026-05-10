use tanren_contract::RoleFailureReason;
use tanren_identity_policy::{AccountId, PermissionName, PermissionScope, PrincipalRef, RoleScope};
use tanren_store::{RoleStore, StoreError};

use crate::RoleServiceError;

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
        self.has_any_direct_grant(PrincipalRef::Account { account_id: actor }, permission)
            .await
    }
}

pub(crate) async fn authorize_manage_role_scope<S>(
    store: &S,
    actor: AccountId,
    scope: RoleScope,
) -> Result<(), RoleServiceError>
where
    S: RoleAdminCapabilityPort + ?Sized,
{
    authorize_role_permission(
        store,
        actor,
        scope.as_permission_scope(),
        &role_manage_permission()?,
    )
    .await
}

pub(crate) async fn authorize_manage_permission_scope<S>(
    store: &S,
    actor: AccountId,
    scope: PermissionScope,
) -> Result<(), RoleServiceError>
where
    S: RoleAdminCapabilityPort + ?Sized,
{
    authorize_role_permission(store, actor, scope, &role_manage_permission()?).await
}

pub(crate) async fn authorize_read_permission_scope<S>(
    store: &S,
    actor: AccountId,
    scope: PermissionScope,
) -> Result<(), RoleServiceError>
where
    S: RoleAdminCapabilityPort + ?Sized,
{
    let can_read = store
        .actor_has_capability_in_scope(actor, scope, &role_read_permission()?)
        .await?;
    if can_read {
        return Ok(());
    }
    authorize_manage_permission_scope(store, actor, scope).await
}

pub(crate) fn role_manage_permission() -> Result<PermissionName, RoleServiceError> {
    PermissionName::parse(ROLE_MANAGE_PERMISSION)
        .map_err(|err| RoleServiceError::InvalidInput(format!("invalid static permission: {err}")))
}

pub(crate) fn role_read_permission() -> Result<PermissionName, RoleServiceError> {
    PermissionName::parse(ROLE_READ_PERMISSION)
        .map_err(|err| RoleServiceError::InvalidInput(format!("invalid static permission: {err}")))
}

async fn authorize_role_permission<S>(
    store: &S,
    actor: AccountId,
    scope: PermissionScope,
    permission: &PermissionName,
) -> Result<(), RoleServiceError>
where
    S: RoleAdminCapabilityPort + ?Sized,
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
