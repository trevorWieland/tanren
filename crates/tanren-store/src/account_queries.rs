//! Query helpers shared by the `AccountStore` adapter implementation.

use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use tanren_identity_policy::{AccountId, MembershipId, SessionToken};

use crate::entity;
use crate::{ListOrganizationsPage, OrganizationRecord, SessionRecord, StoreError};

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
    for (_, maybe_org) in rows {
        if let Some(org) = maybe_org {
            organizations.push(OrganizationRecord::try_from(org)?);
        }
    }

    Ok(ListOrganizationsPage {
        organizations,
        next_cursor,
    })
}
