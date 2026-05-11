use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Utc};
use sea_orm::sea_query::OnConflict;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Condition, ConnectionTrait, DatabaseConnection,
    DatabaseTransaction, EntityTrait, QueryFilter, QueryOrder, QuerySelect, Set,
};
use tanren_identity_policy::{PermissionName, PermissionScope, PrincipalRef, RoleId, RoleScope};
use uuid::Uuid;

use crate::entity;
use crate::role_store_util::{
    ROLE_GRANT_INSERT_BATCH_SIZE, clamp_page_limit, dedup_permission_names,
};
use crate::{
    CursorPage, PermissionGrantListCursor, PermissionGrantRecord, RoleListCursor, RoleRecord,
    StoreError, parse_db_permission_name, permission_scope_to_parts, principal_ref_to_parts,
    role_scope_to_parts,
};

pub(crate) async fn list_roles_in_scope_page(
    conn: &DatabaseConnection,
    scope: RoleScope,
    cursor: Option<RoleListCursor>,
    limit: u64,
) -> Result<CursorPage<RoleRecord, RoleListCursor>, StoreError> {
    let (scope_kind, scope_ref) = role_scope_to_parts(scope);
    let page_limit = clamp_page_limit(limit);
    let page_limit_usize =
        usize::try_from(page_limit).map_err(|_| StoreError::PolicyDataInvariant {
            column: "page_limit",
            cause: "role list page limit exceeds usize".to_owned(),
        })?;
    let mut query = entity::roles::Entity::find()
        .filter(entity::roles::Column::ScopeKind.eq(scope_kind))
        .filter(entity::roles::Column::ScopeRef.eq(scope_ref))
        .order_by_asc(entity::roles::Column::Name)
        .order_by_asc(entity::roles::Column::Id)
        .limit(page_limit + 1);
    if let Some(cursor) = &cursor {
        query = query.filter(
            Condition::any()
                .add(entity::roles::Column::Name.gt(cursor.name.as_str()))
                .add(
                    Condition::all()
                        .add(entity::roles::Column::Name.eq(cursor.name.as_str()))
                        .add(entity::roles::Column::Id.gt(cursor.id.as_uuid())),
                ),
        );
    }
    let mut rows = query.all(conn).await?;
    if rows.is_empty() {
        return Ok(CursorPage {
            items: Vec::new(),
            next_cursor: None,
        });
    }
    let has_more = rows.len() > page_limit_usize;
    if has_more {
        rows.truncate(page_limit_usize);
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

    let records = rows
        .into_iter()
        .map(|row| {
            let permissions = by_role.remove(&row.id).unwrap_or_default();
            RoleRecord::from_parts(row, permissions)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let next_cursor = if has_more {
        records.last().map(|record| RoleListCursor {
            name: record.name.clone(),
            id: record.id,
        })
    } else {
        None
    };
    Ok(CursorPage {
        items: records,
        next_cursor,
    })
}

pub(crate) async fn role_exists_in_scope<C: ConnectionTrait>(
    conn: &C,
    role_id: RoleId,
    scope: RoleScope,
) -> Result<bool, StoreError> {
    let (scope_kind, scope_ref) = role_scope_to_parts(scope);
    let row = entity::roles::Entity::find()
        .filter(entity::roles::Column::Id.eq(role_id.as_uuid()))
        .filter(entity::roles::Column::ScopeKind.eq(scope_kind))
        .filter(entity::roles::Column::ScopeRef.eq(scope_ref))
        .limit(1)
        .one(conn)
        .await?;
    Ok(row.is_some())
}

pub(crate) async fn list_role_permission_names<C: ConnectionTrait>(
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

pub(crate) async fn insert_role_permissions_in_txn(
    txn: &DatabaseTransaction,
    role_id: RoleId,
    permissions: &[PermissionName],
    created_at: DateTime<Utc>,
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
    for chunk in inserts.chunks(ROLE_GRANT_INSERT_BATCH_SIZE) {
        entity::role_permissions::Entity::insert_many(chunk.to_vec())
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
    }
    Ok(())
}

pub(crate) async fn sync_role_permissions_in_txn(
    txn: &DatabaseTransaction,
    role_id: RoleId,
    desired_permissions: &[PermissionName],
    updated_at: DateTime<Utc>,
) -> Result<(), StoreError> {
    let existing_rows = entity::role_permissions::Entity::find()
        .filter(entity::role_permissions::Column::RoleId.eq(role_id.as_uuid()))
        .all(txn)
        .await?;
    let existing_permission_names = existing_rows
        .iter()
        .map(|row| row.permission_name.clone())
        .collect::<HashSet<_>>();
    let desired_permission_names = dedup_permission_names(desired_permissions)
        .into_iter()
        .collect::<HashSet<_>>();

    let removed_permissions = existing_permission_names
        .difference(&desired_permission_names)
        .cloned()
        .collect::<Vec<_>>();
    if !removed_permissions.is_empty() {
        entity::role_permissions::Entity::delete_many()
            .filter(entity::role_permissions::Column::RoleId.eq(role_id.as_uuid()))
            .filter(entity::role_permissions::Column::PermissionName.is_in(removed_permissions))
            .exec(txn)
            .await?;
    }

    let added_permissions = desired_permission_names
        .difference(&existing_permission_names)
        .cloned()
        .collect::<Vec<_>>();
    if !added_permissions.is_empty() {
        let permissions = added_permissions
            .into_iter()
            .map(|name| parse_db_permission_name(&name))
            .collect::<Result<Vec<_>, _>>()?;
        insert_role_permissions_in_txn(txn, role_id, &permissions, updated_at).await?;
    }
    Ok(())
}

pub(crate) async fn insert_permission_grants_chunked(
    txn: &DatabaseTransaction,
    inserts: Vec<entity::permission_grants::ActiveModel>,
) -> Result<(), StoreError> {
    for chunk in inserts.chunks(ROLE_GRANT_INSERT_BATCH_SIZE) {
        entity::permission_grants::Entity::insert_many(chunk.to_vec())
            .on_conflict(
                OnConflict::columns([
                    entity::permission_grants::Column::GranteeKind,
                    entity::permission_grants::Column::GranteeRef,
                    entity::permission_grants::Column::ScopeKind,
                    entity::permission_grants::Column::ScopeRef,
                    entity::permission_grants::Column::PermissionName,
                ])
                .do_nothing()
                .to_owned(),
            )
            .exec(txn)
            .await?;
    }
    Ok(())
}

pub(crate) async fn list_direct_grants_page(
    conn: &DatabaseConnection,
    principal: PrincipalRef,
    scope: Option<PermissionScope>,
    cursor: Option<PermissionGrantListCursor>,
    limit: u64,
) -> Result<CursorPage<PermissionGrantRecord, PermissionGrantListCursor>, StoreError> {
    let (grantee_kind, grantee_ref) = principal_ref_to_parts(principal);
    let page_limit = clamp_page_limit(limit);
    let page_limit_usize =
        usize::try_from(page_limit).map_err(|_| StoreError::PolicyDataInvariant {
            column: "page_limit",
            cause: "grant list page limit exceeds usize".to_owned(),
        })?;
    let mut query = entity::permission_grants::Entity::find()
        .filter(entity::permission_grants::Column::GranteeKind.eq(grantee_kind))
        .filter(entity::permission_grants::Column::GranteeRef.eq(grantee_ref))
        .filter(entity::permission_grants::Column::RevokedAt.is_null())
        .order_by_asc(entity::permission_grants::Column::GrantedAt)
        .order_by_asc(entity::permission_grants::Column::Id)
        .limit(page_limit + 1);
    if let Some(scope) = scope {
        let (scope_kind, scope_ref) = permission_scope_to_parts(scope);
        query = query
            .filter(entity::permission_grants::Column::ScopeKind.eq(scope_kind))
            .filter(entity::permission_grants::Column::ScopeRef.eq(scope_ref));
    }
    if let Some(cursor) = cursor {
        query = query.filter(
            Condition::any()
                .add(entity::permission_grants::Column::GrantedAt.gt(cursor.granted_at))
                .add(
                    Condition::all()
                        .add(entity::permission_grants::Column::GrantedAt.eq(cursor.granted_at))
                        .add(entity::permission_grants::Column::Id.gt(cursor.id.as_uuid())),
                ),
        );
    }
    let mut rows = query.all(conn).await?;
    let has_more = rows.len() > page_limit_usize;
    if has_more {
        rows.truncate(page_limit_usize);
    }
    let items = rows
        .into_iter()
        .map(PermissionGrantRecord::try_from)
        .collect::<Result<Vec<_>, _>>()?;
    let next_cursor = if has_more {
        items.last().map(|item| PermissionGrantListCursor {
            granted_at: item.granted_at,
            id: item.id,
        })
    } else {
        None
    };
    Ok(CursorPage { items, next_cursor })
}

pub(crate) fn missing_permissions_for_snapshot(
    snapshot_permissions: &[String],
    existing_rows: &[entity::permission_grants::Model],
) -> Vec<String> {
    let existing_permissions = existing_rows
        .iter()
        .map(|row| row.permission_name.clone())
        .collect::<HashSet<_>>();
    snapshot_permissions
        .iter()
        .filter(|permission| !existing_permissions.contains(*permission))
        .cloned()
        .collect::<Vec<_>>()
}

pub(crate) async fn load_role_record<C: ConnectionTrait>(
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

pub(crate) async fn append_event_in_txn(
    txn: &DatabaseTransaction,
    payload: serde_json::Value,
    now: DateTime<Utc>,
) -> Result<(), StoreError> {
    let model = entity::events::ActiveModel {
        id: Set(Uuid::now_v7()),
        occurred_at: Set(now),
        payload: Set(payload),
    };
    model.insert(txn).await?;
    Ok(())
}
