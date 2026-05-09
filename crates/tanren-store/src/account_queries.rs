//! Query helpers shared by the `AccountStore` adapter implementation.

use chrono::{DateTime, Utc};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use tanren_identity_policy::{AccountId, SessionToken};

use crate::entity;
use crate::{OrganizationRecord, SessionRecord, StoreError};

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
) -> Result<Vec<OrganizationRecord>, StoreError> {
    let memberships = entity::memberships::Entity::find()
        .filter(entity::memberships::Column::AccountId.eq(account_id.as_uuid()))
        .all(conn)
        .await?;
    let org_ids = memberships
        .into_iter()
        .map(|row| row.org_id)
        .collect::<Vec<_>>();
    if org_ids.is_empty() {
        return Ok(Vec::new());
    }

    let rows = entity::organizations::Entity::find()
        .filter(entity::organizations::Column::Id.is_in(org_ids))
        .all(conn)
        .await?;
    rows.into_iter().map(OrganizationRecord::try_from).collect()
}

pub(crate) async fn find_latest_active_session_for_account(
    conn: &sea_orm::DatabaseConnection,
    account_id: AccountId,
    expires_at: DateTime<Utc>,
    now: DateTime<Utc>,
) -> Result<Option<SessionRecord>, StoreError> {
    let row = entity::account_sessions::Entity::find()
        .filter(entity::account_sessions::Column::AccountId.eq(account_id.as_uuid()))
        .filter(entity::account_sessions::Column::ExpiresAt.eq(expires_at))
        .filter(entity::account_sessions::Column::ExpiresAt.gt(now))
        .order_by_desc(entity::account_sessions::Column::CreatedAt)
        .one(conn)
        .await?;
    Ok(row.map(SessionRecord::from))
}
