//! `SeaORM`-backed role-template + direct-grant persistence adapter.

use std::collections::HashMap;

use async_trait::async_trait;
use sea_orm::sea_query::OnConflict;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, DatabaseTransaction,
    EntityTrait, QueryFilter, QueryOrder, Set, TransactionTrait,
};
use tanren_identity_policy::{PermissionName, PermissionScope, PrincipalRef, RoleId, RoleScope};
use uuid::Uuid;

use crate::entity;
use crate::role_scope_lookup::{permission_scope_exists, principal_exists, role_scope_exists};
use crate::role_store_util::{
    dedup_permission_names, is_unique_violation, map_store_txn_error, map_wrapped_txn_error,
};
use crate::{
    ApplyRole, ApplyRoleError, CreateRoleError, EditRole, EditRoleError, NewRole,
    PermissionGrantRecord, RoleRecord, RoleStore, Store, StoreError, parse_db_permission_name,
    permission_scope_to_parts, principal_ref_to_parts, role_scope_to_parts,
};

#[async_trait]
impl RoleStore for Store {
    async fn create_role(&self, new: NewRole) -> Result<RoleRecord, CreateRoleError> {
        self.conn
            .transaction::<_, RoleRecord, CreateRoleError>(|txn| {
                Box::pin(async move { create_role_in_txn(txn, new).await })
            })
            .await
            .map_err(map_wrapped_txn_error)
    }

    async fn edit_role(&self, edit: EditRole) -> Result<RoleRecord, EditRoleError> {
        self.conn
            .transaction::<_, RoleRecord, EditRoleError>(|txn| {
                Box::pin(async move { edit_role_in_txn(txn, edit).await })
            })
            .await
            .map_err(map_wrapped_txn_error)
    }

    async fn delete_role(
        &self,
        role: tanren_identity_policy::ScopedRole,
    ) -> Result<bool, StoreError> {
        self.conn
            .transaction::<_, bool, StoreError>(|txn| {
                Box::pin(async move { delete_role_in_txn(txn, role).await })
            })
            .await
            .map_err(map_store_txn_error)
    }

    async fn list_roles(&self, scope: RoleScope) -> Result<Vec<RoleRecord>, StoreError> {
        list_roles_in_scope(&self.conn, scope).await
    }

    async fn find_role(
        &self,
        role: tanren_identity_policy::ScopedRole,
    ) -> Result<Option<RoleRecord>, StoreError> {
        load_role_record(&self.conn, role.role_id, role.scope).await
    }

    async fn role_scope_exists(&self, scope: RoleScope) -> Result<bool, StoreError> {
        role_scope_exists(&self.conn, scope).await
    }

    async fn permission_scope_exists(&self, scope: PermissionScope) -> Result<bool, StoreError> {
        permission_scope_exists(&self.conn, scope).await
    }

    async fn principal_exists(&self, principal: PrincipalRef) -> Result<bool, StoreError> {
        principal_exists(&self.conn, principal).await
    }

    async fn apply_role(
        &self,
        request: ApplyRole,
    ) -> Result<Vec<PermissionGrantRecord>, ApplyRoleError> {
        self.conn
            .transaction::<_, Vec<PermissionGrantRecord>, ApplyRoleError>(|txn| {
                Box::pin(async move { apply_role_in_txn(txn, request).await })
            })
            .await
            .map_err(map_wrapped_txn_error)
    }

    async fn has_direct_grant(
        &self,
        principal: PrincipalRef,
        scope: PermissionScope,
        permission: &PermissionName,
    ) -> Result<bool, StoreError> {
        let (grantee_kind, grantee_ref) = principal_ref_to_parts(principal);
        let (scope_kind, scope_ref) = permission_scope_to_parts(scope);
        let row = entity::permission_grants::Entity::find()
            .filter(entity::permission_grants::Column::GranteeKind.eq(grantee_kind))
            .filter(entity::permission_grants::Column::GranteeRef.eq(grantee_ref))
            .filter(entity::permission_grants::Column::ScopeKind.eq(scope_kind))
            .filter(entity::permission_grants::Column::ScopeRef.eq(scope_ref))
            .filter(entity::permission_grants::Column::PermissionName.eq(permission.as_str()))
            .one(&self.conn)
            .await?;
        Ok(row.is_some())
    }

    async fn list_direct_grants(
        &self,
        principal: PrincipalRef,
        scope: PermissionScope,
    ) -> Result<Vec<PermissionGrantRecord>, StoreError> {
        let (grantee_kind, grantee_ref) = principal_ref_to_parts(principal);
        let (scope_kind, scope_ref) = permission_scope_to_parts(scope);
        let rows = entity::permission_grants::Entity::find()
            .filter(entity::permission_grants::Column::GranteeKind.eq(grantee_kind))
            .filter(entity::permission_grants::Column::GranteeRef.eq(grantee_ref))
            .filter(entity::permission_grants::Column::ScopeKind.eq(scope_kind))
            .filter(entity::permission_grants::Column::ScopeRef.eq(scope_ref))
            .order_by_asc(entity::permission_grants::Column::PermissionName)
            .order_by_asc(entity::permission_grants::Column::GrantedAt)
            .all(&self.conn)
            .await?;
        rows.into_iter()
            .map(PermissionGrantRecord::try_from)
            .collect::<Result<Vec<_>, _>>()
    }

    async fn list_all_direct_grants(
        &self,
        principal: PrincipalRef,
    ) -> Result<Vec<PermissionGrantRecord>, StoreError> {
        let (grantee_kind, grantee_ref) = principal_ref_to_parts(principal);
        let rows = entity::permission_grants::Entity::find()
            .filter(entity::permission_grants::Column::GranteeKind.eq(grantee_kind))
            .filter(entity::permission_grants::Column::GranteeRef.eq(grantee_ref))
            .order_by_asc(entity::permission_grants::Column::ScopeKind)
            .order_by_asc(entity::permission_grants::Column::ScopeRef)
            .order_by_asc(entity::permission_grants::Column::PermissionName)
            .order_by_asc(entity::permission_grants::Column::GrantedAt)
            .all(&self.conn)
            .await?;
        rows.into_iter()
            .map(PermissionGrantRecord::try_from)
            .collect::<Result<Vec<_>, _>>()
    }

    async fn find_direct_grants(
        &self,
        principal: PrincipalRef,
        scope: PermissionScope,
        permission: &PermissionName,
    ) -> Result<Vec<PermissionGrantRecord>, StoreError> {
        let (grantee_kind, grantee_ref) = principal_ref_to_parts(principal);
        let (scope_kind, scope_ref) = permission_scope_to_parts(scope);
        let rows = entity::permission_grants::Entity::find()
            .filter(entity::permission_grants::Column::GranteeKind.eq(grantee_kind))
            .filter(entity::permission_grants::Column::GranteeRef.eq(grantee_ref))
            .filter(entity::permission_grants::Column::ScopeKind.eq(scope_kind))
            .filter(entity::permission_grants::Column::ScopeRef.eq(scope_ref))
            .filter(entity::permission_grants::Column::PermissionName.eq(permission.as_str()))
            .order_by_asc(entity::permission_grants::Column::GrantedAt)
            .all(&self.conn)
            .await?;
        rows.into_iter()
            .map(PermissionGrantRecord::try_from)
            .collect::<Result<Vec<_>, _>>()
    }

    async fn list_role_permissions(
        &self,
        role_id: RoleId,
    ) -> Result<Vec<PermissionName>, StoreError> {
        list_role_permission_names(&self.conn, role_id).await
    }
}

async fn create_role_in_txn(
    txn: &DatabaseTransaction,
    new: NewRole,
) -> Result<RoleRecord, CreateRoleError> {
    let NewRole {
        id,
        scope,
        name,
        permissions,
        created_at,
        updated_at,
    } = new;
    let (scope_kind, scope_ref) = role_scope_to_parts(scope);
    let role_insert = entity::roles::ActiveModel {
        id: Set(id.as_uuid()),
        scope_kind: Set(scope_kind.to_owned()),
        scope_ref: Set(scope_ref),
        name: Set(name.as_str().to_owned()),
        created_at: Set(created_at),
        updated_at: Set(updated_at),
    };
    if let Err(err) = role_insert.insert(txn).await {
        if is_unique_violation(&err) {
            return Err(CreateRoleError::DuplicateRoleName);
        }
        return Err(StoreError::from(err).into());
    }
    insert_role_permissions_in_txn(txn, id, &permissions, created_at).await?;
    let role =
        load_role_record(txn, id, scope)
            .await?
            .ok_or_else(|| StoreError::EnumDataInvariant {
                column: "role_id",
                value: id.as_uuid().to_string(),
            })?;
    Ok(role)
}

async fn edit_role_in_txn(
    txn: &DatabaseTransaction,
    edit: EditRole,
) -> Result<RoleRecord, EditRoleError> {
    let EditRole {
        role,
        name,
        permissions,
        updated_at,
    } = edit;
    let (scope_kind, scope_ref) = role_scope_to_parts(role.scope);
    let update_result = entity::roles::Entity::update_many()
        .col_expr(
            entity::roles::Column::Name,
            sea_orm::sea_query::Expr::value(name.as_str().to_owned()),
        )
        .col_expr(
            entity::roles::Column::UpdatedAt,
            sea_orm::sea_query::Expr::value(updated_at),
        )
        .filter(entity::roles::Column::Id.eq(role.role_id.as_uuid()))
        .filter(entity::roles::Column::ScopeKind.eq(scope_kind))
        .filter(entity::roles::Column::ScopeRef.eq(scope_ref))
        .exec(txn)
        .await;
    let update_result = match update_result {
        Ok(result) => result,
        Err(err) => {
            if is_unique_violation(&err) {
                return Err(EditRoleError::DuplicateRoleName);
            }
            return Err(StoreError::from(err).into());
        }
    };
    if update_result.rows_affected != 1 {
        return Err(EditRoleError::RoleNotFound);
    }

    entity::role_permissions::Entity::delete_many()
        .filter(entity::role_permissions::Column::RoleId.eq(role.role_id.as_uuid()))
        .exec(txn)
        .await
        .map_err(StoreError::from)?;
    insert_role_permissions_in_txn(txn, role.role_id, &permissions, updated_at).await?;

    let row = load_role_record(txn, role.role_id, role.scope)
        .await?
        .ok_or(EditRoleError::RoleNotFound)?;
    Ok(row)
}

async fn delete_role_in_txn(
    txn: &DatabaseTransaction,
    role: tanren_identity_policy::ScopedRole,
) -> Result<bool, StoreError> {
    entity::role_permissions::Entity::delete_many()
        .filter(entity::role_permissions::Column::RoleId.eq(role.role_id.as_uuid()))
        .exec(txn)
        .await?;

    let (scope_kind, scope_ref) = role_scope_to_parts(role.scope);
    let delete_result = entity::roles::Entity::delete_many()
        .filter(entity::roles::Column::Id.eq(role.role_id.as_uuid()))
        .filter(entity::roles::Column::ScopeKind.eq(scope_kind))
        .filter(entity::roles::Column::ScopeRef.eq(scope_ref))
        .exec(txn)
        .await?;
    Ok(delete_result.rows_affected == 1)
}

async fn apply_role_in_txn(
    txn: &DatabaseTransaction,
    request: ApplyRole,
) -> Result<Vec<PermissionGrantRecord>, ApplyRoleError> {
    let ApplyRole {
        role,
        principal,
        grant_scope,
        granted_by,
        granted_at,
    } = request;
    let role_row = load_role_record(txn, role.role_id, role.scope).await?;
    if role_row.is_none() {
        return Err(ApplyRoleError::RoleNotFound);
    }
    if !principal_exists(txn, principal).await? {
        return Err(ApplyRoleError::PrincipalNotFound);
    }
    if !permission_scope_exists(txn, grant_scope).await? {
        return Err(ApplyRoleError::GrantScopeNotFound);
    }
    if !role.scope.allows_grant_scope(grant_scope) {
        return Err(ApplyRoleError::IncompatibleGrantScope);
    }
    let permission_names = list_role_permission_names(txn, role.role_id).await?;
    if permission_names.is_empty() {
        return Ok(Vec::new());
    }

    let dedup_permission_strings = dedup_permission_names(&permission_names);
    let (grantee_kind, grantee_ref) = principal_ref_to_parts(principal);
    let (scope_kind, scope_ref) = permission_scope_to_parts(grant_scope);
    let (granted_by_kind, granted_by_ref) = principal_ref_to_parts(granted_by);
    let inserts = dedup_permission_strings
        .iter()
        .map(|permission| entity::permission_grants::ActiveModel {
            id: Set(Uuid::now_v7()),
            grantee_kind: Set(grantee_kind.to_owned()),
            grantee_ref: Set(grantee_ref),
            scope_kind: Set(scope_kind.to_owned()),
            scope_ref: Set(scope_ref),
            permission_name: Set(permission.clone()),
            source_role_id: Set(role.role_id.as_uuid()),
            granted_by_kind: Set(granted_by_kind.to_owned()),
            granted_by_ref: Set(granted_by_ref),
            granted_at: Set(granted_at),
        })
        .collect::<Vec<_>>();

    entity::permission_grants::Entity::insert_many(inserts)
        .on_conflict(
            OnConflict::columns([
                entity::permission_grants::Column::GranteeKind,
                entity::permission_grants::Column::GranteeRef,
                entity::permission_grants::Column::ScopeKind,
                entity::permission_grants::Column::ScopeRef,
                entity::permission_grants::Column::PermissionName,
                entity::permission_grants::Column::SourceRoleId,
            ])
            .do_nothing()
            .to_owned(),
        )
        .exec(txn)
        .await
        .map_err(StoreError::from)?;

    let rows = entity::permission_grants::Entity::find()
        .filter(entity::permission_grants::Column::GranteeKind.eq(grantee_kind))
        .filter(entity::permission_grants::Column::GranteeRef.eq(grantee_ref))
        .filter(entity::permission_grants::Column::ScopeKind.eq(scope_kind))
        .filter(entity::permission_grants::Column::ScopeRef.eq(scope_ref))
        .filter(entity::permission_grants::Column::SourceRoleId.eq(role.role_id.as_uuid()))
        .filter(entity::permission_grants::Column::PermissionName.is_in(dedup_permission_strings))
        .order_by_asc(entity::permission_grants::Column::PermissionName)
        .order_by_asc(entity::permission_grants::Column::GrantedAt)
        .all(txn)
        .await
        .map_err(StoreError::from)?;
    rows.into_iter()
        .map(PermissionGrantRecord::try_from)
        .collect::<Result<Vec<_>, _>>()
        .map_err(ApplyRoleError::from)
}

async fn list_roles_in_scope(
    conn: &DatabaseConnection,
    scope: RoleScope,
) -> Result<Vec<RoleRecord>, StoreError> {
    let (scope_kind, scope_ref) = role_scope_to_parts(scope);
    let rows = entity::roles::Entity::find()
        .filter(entity::roles::Column::ScopeKind.eq(scope_kind))
        .filter(entity::roles::Column::ScopeRef.eq(scope_ref))
        .order_by_asc(entity::roles::Column::Name)
        .order_by_asc(entity::roles::Column::Id)
        .all(conn)
        .await?;
    if rows.is_empty() {
        return Ok(Vec::new());
    }

    let role_ids = rows.iter().map(|row| row.id).collect::<Vec<_>>();
    let permission_rows = entity::role_permissions::Entity::find()
        .filter(entity::role_permissions::Column::RoleId.is_in(role_ids))
        .order_by_asc(entity::role_permissions::Column::RoleId)
        .order_by_asc(entity::role_permissions::Column::PermissionName)
        .all(conn)
        .await?;
    let mut by_role: HashMap<Uuid, Vec<PermissionName>> = HashMap::new();
    for row in permission_rows {
        by_role
            .entry(row.role_id)
            .or_default()
            .push(parse_db_permission_name(&row.permission_name)?);
    }

    rows.into_iter()
        .map(|row| {
            let permissions = by_role.remove(&row.id).unwrap_or_default();
            RoleRecord::from_parts(row, permissions)
        })
        .collect::<Result<Vec<_>, _>>()
}

async fn load_role_record<C: ConnectionTrait>(
    conn: &C,
    role_id: RoleId,
    scope: RoleScope,
) -> Result<Option<RoleRecord>, StoreError> {
    let (scope_kind, scope_ref) = role_scope_to_parts(scope);
    let row = entity::roles::Entity::find()
        .filter(entity::roles::Column::Id.eq(role_id.as_uuid()))
        .filter(entity::roles::Column::ScopeKind.eq(scope_kind))
        .filter(entity::roles::Column::ScopeRef.eq(scope_ref))
        .one(conn)
        .await?;
    let Some(role_row) = row else {
        return Ok(None);
    };

    let permissions = list_role_permission_names(conn, role_id).await?;
    RoleRecord::from_parts(role_row, permissions).map(Some)
}

async fn list_role_permission_names<C: ConnectionTrait>(
    conn: &C,
    role_id: RoleId,
) -> Result<Vec<PermissionName>, StoreError> {
    let rows = entity::role_permissions::Entity::find()
        .filter(entity::role_permissions::Column::RoleId.eq(role_id.as_uuid()))
        .order_by_asc(entity::role_permissions::Column::PermissionName)
        .all(conn)
        .await?;
    rows.into_iter()
        .map(|row| parse_db_permission_name(&row.permission_name))
        .collect::<Result<Vec<_>, _>>()
}

async fn insert_role_permissions_in_txn(
    txn: &DatabaseTransaction,
    role_id: RoleId,
    permissions: &[PermissionName],
    created_at: chrono::DateTime<chrono::Utc>,
) -> Result<(), StoreError> {
    let dedup_permissions = dedup_permission_names(permissions);
    if dedup_permissions.is_empty() {
        return Ok(());
    }
    let inserts = dedup_permissions
        .into_iter()
        .map(|permission_name| entity::role_permissions::ActiveModel {
            role_id: Set(role_id.as_uuid()),
            permission_name: Set(permission_name),
            created_at: Set(created_at),
        })
        .collect::<Vec<_>>();
    entity::role_permissions::Entity::insert_many(inserts)
        .on_conflict(
            OnConflict::columns([
                entity::role_permissions::Column::RoleId,
                entity::role_permissions::Column::PermissionName,
            ])
            .do_nothing()
            .to_owned(),
        )
        .exec(txn)
        .await?;
    Ok(())
}
