//! Scope/principal existence lookups used by role operations.

use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter};
use tanren_identity_policy::{PermissionScope, PrincipalRef, RoleScope};
use uuid::Uuid;

use crate::{StoreError, entity};

pub(crate) async fn role_scope_exists<C: ConnectionTrait>(
    conn: &C,
    scope: RoleScope,
) -> Result<bool, StoreError> {
    match scope {
        RoleScope::Account { account_id } => account_exists(conn, account_id.as_uuid()).await,
        RoleScope::Organization { org_id } => org_exists(conn, org_id.as_uuid()).await,
        RoleScope::Project { project_id } => project_exists(conn, project_id.as_uuid()).await,
    }
}

pub(crate) async fn permission_scope_exists<C: ConnectionTrait>(
    conn: &C,
    scope: PermissionScope,
) -> Result<bool, StoreError> {
    match scope {
        PermissionScope::Account { account_id } => account_exists(conn, account_id.as_uuid()).await,
        PermissionScope::Organization { org_id } => org_exists(conn, org_id.as_uuid()).await,
        PermissionScope::Project { project_id } => project_exists(conn, project_id.as_uuid()).await,
    }
}

pub(crate) async fn principal_exists<C: ConnectionTrait>(
    conn: &C,
    principal: PrincipalRef,
) -> Result<bool, StoreError> {
    match principal {
        PrincipalRef::Account { account_id } => account_exists(conn, account_id.as_uuid()).await,
        PrincipalRef::Role { role_id } => {
            let row = entity::roles::Entity::find_by_id(role_id.as_uuid())
                .one(conn)
                .await?;
            Ok(row.is_some())
        }
    }
}

async fn account_exists<C: ConnectionTrait>(
    conn: &C,
    account_id: Uuid,
) -> Result<bool, StoreError> {
    let row = entity::accounts::Entity::find_by_id(account_id)
        .one(conn)
        .await?;
    Ok(row.is_some())
}

async fn org_exists<C: ConnectionTrait>(conn: &C, org_id: Uuid) -> Result<bool, StoreError> {
    if entity::accounts::Entity::find()
        .filter(entity::accounts::Column::OrgId.eq(org_id))
        .one(conn)
        .await?
        .is_some()
    {
        return Ok(true);
    }
    if entity::memberships::Entity::find()
        .filter(entity::memberships::Column::OrgId.eq(org_id))
        .one(conn)
        .await?
        .is_some()
    {
        return Ok(true);
    }
    if entity::invitations::Entity::find()
        .filter(entity::invitations::Column::InvitingOrgId.eq(org_id))
        .one(conn)
        .await?
        .is_some()
    {
        return Ok(true);
    }
    if entity::roles::Entity::find()
        .filter(entity::roles::Column::ScopeKind.eq("organization"))
        .filter(entity::roles::Column::ScopeRef.eq(org_id))
        .one(conn)
        .await?
        .is_some()
    {
        return Ok(true);
    }
    if entity::permission_grants::Entity::find()
        .filter(entity::permission_grants::Column::ScopeKind.eq("organization"))
        .filter(entity::permission_grants::Column::ScopeRef.eq(org_id))
        .one(conn)
        .await?
        .is_some()
    {
        return Ok(true);
    }
    Ok(false)
}

async fn project_exists<C: ConnectionTrait>(
    conn: &C,
    project_id: Uuid,
) -> Result<bool, StoreError> {
    if entity::roles::Entity::find()
        .filter(entity::roles::Column::ScopeKind.eq("project"))
        .filter(entity::roles::Column::ScopeRef.eq(project_id))
        .one(conn)
        .await?
        .is_some()
    {
        return Ok(true);
    }
    if entity::permission_grants::Entity::find()
        .filter(entity::permission_grants::Column::ScopeKind.eq("project"))
        .filter(entity::permission_grants::Column::ScopeRef.eq(project_id))
        .one(conn)
        .await?
        .is_some()
    {
        return Ok(true);
    }
    Ok(false)
}
