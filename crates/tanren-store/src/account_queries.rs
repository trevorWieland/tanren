//! Query helpers shared by the `AccountStore` adapter implementation.

use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use tanren_identity_policy::{AccountId, MembershipId, SessionToken};

use crate::entity;
use crate::{
    ListOrganizationsPage, ListedOrganizationRecord, OrganizationRecord, SessionRecord, StoreError,
    parse_db_organization_permission,
};

pub(crate) async fn find_session_by_token(
    conn: &sea_orm::DatabaseConnection,
    token: &SessionToken,
) -> Result<Option<SessionRecord>, StoreError> {
    let row = entity::account_sessions::Entity::find_by_id(token.expose_secret().to_owned())
        .one(conn)
        .await?;
    Ok(row.map(SessionRecord::from))
}

pub(crate) async fn list_organizations_for_account(
    conn: &sea_orm::DatabaseConnection,
    account_id: AccountId,
    limit: u64,
    cursor: Option<MembershipId>,
    now: chrono::DateTime<chrono::Utc>,
) -> Result<ListOrganizationsPage, StoreError> {
    let page_limit = limit.max(1);
    let mut query = entity::memberships::Entity::find()
        .filter(entity::memberships::Column::AccountId.eq(account_id.as_uuid()))
        .order_by_asc(entity::memberships::Column::Id)
        .find_also_related(entity::organizations::Entity)
        .limit(page_limit.saturating_add(1));
    if let Some(after) = cursor {
        query = query.filter(entity::memberships::Column::Id.gt(after.as_uuid()));
    }
    let mut rows = query.all(conn).await?;

    let mut next_cursor = None;
    if rows.len() > usize::try_from(page_limit).unwrap_or(usize::MAX) {
        rows.truncate(usize::try_from(page_limit).unwrap_or(usize::MAX));
        if let Some((membership, _)) = rows.last() {
            next_cursor = Some(MembershipId::new(membership.id));
        }
    }

    let mut organizations = Vec::with_capacity(rows.len());
    let org_ids: Vec<_> = rows
        .iter()
        .filter_map(|(_, maybe_org)| maybe_org.as_ref().map(|org| org.id))
        .collect();
    let granted_permissions = if org_ids.is_empty() {
        std::collections::HashMap::new()
    } else {
        let rows = entity::organization_permission_grants::Entity::find()
            .filter(
                entity::organization_permission_grants::Column::AccountId.eq(account_id.as_uuid()),
            )
            .filter(entity::organization_permission_grants::Column::OrgId.is_in(org_ids))
            .all(conn)
            .await?;
        let mut by_org = std::collections::HashMap::new();
        for row in rows {
            let permission = parse_db_organization_permission(&row.permission)?;
            by_org
                .entry(row.org_id)
                .or_insert_with(Vec::new)
                .push(permission);
        }
        by_org
    };

    for (membership, maybe_org) in rows {
        if let Some(org) = maybe_org {
            let org_record = OrganizationRecord::try_from(org)?;
            let org_permissions = granted_permissions
                .get(&org_record.id.as_uuid())
                .cloned()
                .unwrap_or_default();
            organizations.push(ListedOrganizationRecord {
                organization: org_record,
                membership_id: MembershipId::new(membership.id),
                granted_permissions: org_permissions,
            });
        }
    }

    Ok(ListOrganizationsPage {
        organizations,
        next_cursor,
        generated_at: now,
        checkpoint: None,
        cursor: next_cursor.map(|value| value.to_string()),
    })
}
