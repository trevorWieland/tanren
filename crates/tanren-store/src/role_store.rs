//! `SeaORM`-backed role-template + direct-grant persistence adapter.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseTransaction, EntityTrait, QueryFilter, QueryOrder,
    QuerySelect, Set, TransactionTrait,
};
use tanren_identity_policy::{
    PermissionGrantId, PermissionGrantSource, PermissionName, PermissionScope, PrincipalRef,
    RoleId, RoleScope,
};
use uuid::Uuid;

use crate::entity;
use crate::role_scope_lookup::{permission_scope_exists, principal_exists, role_scope_exists};
use crate::role_store_ops::{
    append_event_in_txn, insert_permission_grants_chunked, insert_role_permissions_in_txn,
    list_direct_grants_page, list_role_permission_names, list_roles_in_scope_page,
    load_role_record, missing_permissions_for_snapshot, role_exists_in_scope,
    sync_role_permissions_in_txn,
};
use crate::role_store_util::{
    dedup_permission_names, is_unique_violation, map_store_txn_error, map_wrapped_txn_error,
};
use crate::{
    ApplyRole, ApplyRoleError, CreateRoleError, CursorPage, EditRole, EditRoleError, NewRole,
    PermissionGrantListCursor, PermissionGrantRecord, RoleApplyEventBuilder,
    RoleDeleteEventBuilder, RoleListCursor, RoleRecord, RoleRecordEventBuilder, RoleStore, Store,
    StoreError, permission_grant_source_to_parts, permission_scope_to_parts,
    principal_ref_to_parts, role_scope_to_parts,
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

    async fn create_role_atomic(
        &self,
        new: NewRole,
        event_builder: RoleRecordEventBuilder,
        now: DateTime<Utc>,
    ) -> Result<RoleRecord, CreateRoleError> {
        self.conn
            .transaction::<_, RoleRecord, CreateRoleError>(|txn| {
                Box::pin(async move {
                    let role = create_role_in_txn(txn, new).await?;
                    append_event_in_txn(txn, event_builder(&role), now).await?;
                    Ok(role)
                })
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

    async fn edit_role_atomic(
        &self,
        edit: EditRole,
        event_builder: RoleRecordEventBuilder,
        now: DateTime<Utc>,
    ) -> Result<RoleRecord, EditRoleError> {
        self.conn
            .transaction::<_, RoleRecord, EditRoleError>(|txn| {
                Box::pin(async move {
                    let role = edit_role_in_txn(txn, edit).await?;
                    append_event_in_txn(txn, event_builder(&role), now).await?;
                    Ok(role)
                })
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

    async fn delete_role_atomic(
        &self,
        role: tanren_identity_policy::ScopedRole,
        event_builder: RoleDeleteEventBuilder,
        now: DateTime<Utc>,
    ) -> Result<bool, StoreError> {
        self.conn
            .transaction::<_, bool, StoreError>(|txn| {
                Box::pin(async move {
                    let deleted = delete_role_in_txn(txn, role).await?;
                    if deleted {
                        append_event_in_txn(txn, event_builder(), now).await?;
                    }
                    Ok(deleted)
                })
            })
            .await
            .map_err(map_store_txn_error)
    }

    async fn list_roles_page(
        &self,
        scope: RoleScope,
        cursor: Option<RoleListCursor>,
        limit: u64,
    ) -> Result<CursorPage<RoleRecord, RoleListCursor>, StoreError> {
        list_roles_in_scope_page(&self.conn, scope, cursor, limit).await
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

    async fn apply_role_atomic(
        &self,
        request: ApplyRole,
        event_builder: RoleApplyEventBuilder,
        now: DateTime<Utc>,
    ) -> Result<Vec<PermissionGrantRecord>, ApplyRoleError> {
        self.conn
            .transaction::<_, Vec<PermissionGrantRecord>, ApplyRoleError>(|txn| {
                Box::pin(async move {
                    let grants = apply_role_in_txn(txn, request).await?;
                    append_event_in_txn(txn, event_builder(&grants), now).await?;
                    Ok(grants)
                })
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
            .select_only()
            .column(entity::permission_grants::Column::Id)
            .filter(entity::permission_grants::Column::GranteeKind.eq(grantee_kind))
            .filter(entity::permission_grants::Column::GranteeRef.eq(grantee_ref))
            .filter(entity::permission_grants::Column::ScopeKind.eq(scope_kind))
            .filter(entity::permission_grants::Column::ScopeRef.eq(scope_ref))
            .filter(entity::permission_grants::Column::PermissionName.eq(permission.as_str()))
            .filter(entity::permission_grants::Column::RevokedAt.is_null())
            .limit(1)
            .into_tuple::<Uuid>()
            .one(&self.conn)
            .await?;
        Ok(row.is_some())
    }

    async fn has_any_direct_grant(
        &self,
        principal: PrincipalRef,
        permission: &PermissionName,
    ) -> Result<bool, StoreError> {
        let (grantee_kind, grantee_ref) = principal_ref_to_parts(principal);
        let row = entity::permission_grants::Entity::find()
            .select_only()
            .column(entity::permission_grants::Column::Id)
            .filter(entity::permission_grants::Column::GranteeKind.eq(grantee_kind))
            .filter(entity::permission_grants::Column::GranteeRef.eq(grantee_ref))
            .filter(entity::permission_grants::Column::PermissionName.eq(permission.as_str()))
            .filter(entity::permission_grants::Column::RevokedAt.is_null())
            .limit(1)
            .into_tuple::<Uuid>()
            .one(&self.conn)
            .await?;
        Ok(row.is_some())
    }

    async fn list_direct_grants_page(
        &self,
        principal: PrincipalRef,
        scope: Option<PermissionScope>,
        cursor: Option<PermissionGrantListCursor>,
        limit: u64,
    ) -> Result<CursorPage<PermissionGrantRecord, PermissionGrantListCursor>, StoreError> {
        list_direct_grants_page(&self.conn, principal, scope, cursor, limit).await
    }

    async fn find_direct_grant_ids(
        &self,
        principal: PrincipalRef,
        scope: PermissionScope,
        permission: &PermissionName,
    ) -> Result<Vec<PermissionGrantId>, StoreError> {
        let (grantee_kind, grantee_ref) = principal_ref_to_parts(principal);
        let (scope_kind, scope_ref) = permission_scope_to_parts(scope);
        let rows = entity::permission_grants::Entity::find()
            .select_only()
            .column(entity::permission_grants::Column::Id)
            .filter(entity::permission_grants::Column::GranteeKind.eq(grantee_kind))
            .filter(entity::permission_grants::Column::GranteeRef.eq(grantee_ref))
            .filter(entity::permission_grants::Column::ScopeKind.eq(scope_kind))
            .filter(entity::permission_grants::Column::ScopeRef.eq(scope_ref))
            .filter(entity::permission_grants::Column::PermissionName.eq(permission.as_str()))
            .filter(entity::permission_grants::Column::RevokedAt.is_null())
            .order_by_asc(entity::permission_grants::Column::GrantedAt)
            .order_by_asc(entity::permission_grants::Column::Id)
            .into_tuple::<Uuid>()
            .all(&self.conn)
            .await?;
        Ok(rows
            .into_iter()
            .map(PermissionGrantId::new)
            .collect::<Vec<_>>())
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

    sync_role_permissions_in_txn(txn, role.role_id, &permissions, updated_at).await?;

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
    validate_apply_role_request(txn, role, principal, grant_scope).await?;
    let permission_names = list_role_permission_names(txn, role.role_id).await?;
    if permission_names.is_empty() {
        return Ok(Vec::new());
    }
    let dedup_permission_strings = dedup_permission_names(&permission_names);
    let (grantee_kind, grantee_ref) = principal_ref_to_parts(principal);
    let (scope_kind, scope_ref) = permission_scope_to_parts(grant_scope);
    let (granted_by_kind, granted_by_ref) = principal_ref_to_parts(granted_by);
    let (source_kind, source_ref) =
        permission_grant_source_to_parts(PermissionGrantSource::RoleTemplate {
            role_id: role.role_id,
        });
    let existing_rows = entity::permission_grants::Entity::find()
        .filter(entity::permission_grants::Column::GranteeKind.eq(grantee_kind))
        .filter(entity::permission_grants::Column::GranteeRef.eq(grantee_ref))
        .filter(entity::permission_grants::Column::ScopeKind.eq(scope_kind))
        .filter(entity::permission_grants::Column::ScopeRef.eq(scope_ref))
        .filter(
            entity::permission_grants::Column::PermissionName
                .is_in(dedup_permission_strings.clone()),
        )
        .filter(entity::permission_grants::Column::RevokedAt.is_null())
        .order_by_asc(entity::permission_grants::Column::PermissionName)
        .order_by_asc(entity::permission_grants::Column::GrantedAt)
        .order_by_asc(entity::permission_grants::Column::Id)
        .all(txn)
        .await
        .map_err(StoreError::from)?;
    let missing_permissions =
        missing_permissions_for_snapshot(&dedup_permission_strings, &existing_rows);
    if !missing_permissions.is_empty() {
        let inserts = missing_permissions
            .iter()
            .map(|permission| entity::permission_grants::ActiveModel {
                id: Set(Uuid::now_v7()),
                grantee_kind: Set(grantee_kind.to_owned()),
                grantee_ref: Set(grantee_ref),
                scope_kind: Set(scope_kind.to_owned()),
                scope_ref: Set(scope_ref),
                permission_name: Set(permission.clone()),
                source_kind: Set(source_kind.to_owned()),
                source_ref: Set(source_ref),
                granted_by_kind: Set(granted_by_kind.to_owned()),
                granted_by_ref: Set(granted_by_ref),
                granted_at: Set(granted_at),
                revoked_by_kind: Set(None),
                revoked_by_ref: Set(None),
                revoked_at: Set(None),
            })
            .collect::<Vec<_>>();
        insert_permission_grants_chunked(txn, inserts).await?;
    }
    let mut rows = existing_rows;
    if !missing_permissions.is_empty() {
        let inserted_or_raced_rows = entity::permission_grants::Entity::find()
            .filter(entity::permission_grants::Column::GranteeKind.eq(grantee_kind))
            .filter(entity::permission_grants::Column::GranteeRef.eq(grantee_ref))
            .filter(entity::permission_grants::Column::ScopeKind.eq(scope_kind))
            .filter(entity::permission_grants::Column::ScopeRef.eq(scope_ref))
            .filter(entity::permission_grants::Column::PermissionName.is_in(missing_permissions))
            .filter(entity::permission_grants::Column::RevokedAt.is_null())
            .order_by_asc(entity::permission_grants::Column::PermissionName)
            .order_by_asc(entity::permission_grants::Column::GrantedAt)
            .order_by_asc(entity::permission_grants::Column::Id)
            .all(txn)
            .await
            .map_err(StoreError::from)?;
        rows.extend(inserted_or_raced_rows);
    }
    rows.sort_by(|left, right| {
        left.permission_name
            .cmp(&right.permission_name)
            .then(left.granted_at.cmp(&right.granted_at))
            .then(left.id.cmp(&right.id))
    });
    rows.into_iter()
        .map(PermissionGrantRecord::try_from)
        .collect::<Result<Vec<_>, _>>()
        .map_err(ApplyRoleError::from)
}

fn reject_role_principal(principal: PrincipalRef) -> Result<(), ApplyRoleError> {
    if matches!(principal, PrincipalRef::Role { .. }) {
        Err(ApplyRoleError::RoleAsPrincipalRejected)
    } else {
        Ok(())
    }
}

async fn validate_apply_role_request(
    txn: &DatabaseTransaction,
    role: tanren_identity_policy::ScopedRole,
    principal: PrincipalRef,
    grant_scope: PermissionScope,
) -> Result<(), ApplyRoleError> {
    reject_role_principal(principal)?;
    if !role_exists_in_scope(txn, role.role_id, role.scope).await? {
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
    Ok(())
}
