//! Query helpers for active-organization switching and project listing.
//!
//! Free functions consumed by the `AccountStore` impl in `lib.rs`.
//! Keeping these in a separate module keeps `lib.rs` below the
//! workspace file-size budget.

use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use tanren_identity_policy::{AccountId, OrgId, ProjectId, SessionToken};
use uuid::Uuid;

use crate::entity;
use crate::{ActiveOrganizationError, ListProjectsPage, ProjectRecord, StoreError};

/// Set the `active_org_id` column on the session row identified by
/// `(token, account_id)`. Returns `SessionNotFound` when no row matches.
pub(crate) async fn set_active_organization(
    conn: &sea_orm::DatabaseConnection,
    session_token: &SessionToken,
    account_id: AccountId,
    org_id: OrgId,
) -> Result<(), StoreError> {
    let result = entity::account_sessions::Entity::update_many()
        .col_expr(
            entity::account_sessions::Column::ActiveOrgId,
            sea_orm::sea_query::Expr::value(Some(org_id.as_uuid())),
        )
        .filter(entity::account_sessions::Column::Token.eq(session_token.expose_secret()))
        .filter(entity::account_sessions::Column::AccountId.eq(account_id.as_uuid()))
        .exec(conn)
        .await?;
    if result.rows_affected == 0 {
        return Err(StoreError::SessionNotFound);
    }
    Ok(())
}

/// Clear the `active_org_id` column on the session row (set to `NULL`).
pub(crate) async fn clear_active_organization(
    conn: &sea_orm::DatabaseConnection,
    session_token: &SessionToken,
    account_id: AccountId,
) -> Result<(), StoreError> {
    let result = entity::account_sessions::Entity::update_many()
        .col_expr(
            entity::account_sessions::Column::ActiveOrgId,
            sea_orm::sea_query::Expr::value(Option::<Uuid>::None),
        )
        .filter(entity::account_sessions::Column::Token.eq(session_token.expose_secret()))
        .filter(entity::account_sessions::Column::AccountId.eq(account_id.as_uuid()))
        .exec(conn)
        .await?;
    if result.rows_affected == 0 {
        return Err(StoreError::SessionNotFound);
    }
    Ok(())
}

/// Read the `active_org_id` from the session row. Returns `None` when
/// the session has no active org or the session does not exist.
pub(crate) async fn read_active_organization(
    conn: &sea_orm::DatabaseConnection,
    session_token: &SessionToken,
) -> Result<Option<OrgId>, StoreError> {
    let row =
        entity::account_sessions::Entity::find_by_id(session_token.expose_secret().to_owned())
            .one(conn)
            .await?;
    Ok(row.and_then(|m| m.active_org_id.map(OrgId::new)))
}

/// List projects owned by an organization, paginated by `ProjectId` cursor.
pub(crate) async fn list_projects_for_org(
    conn: &sea_orm::DatabaseConnection,
    org_id: OrgId,
    limit: u64,
    cursor: Option<ProjectId>,
) -> Result<ListProjectsPage, StoreError> {
    let page_limit = limit.max(1);
    let mut query = entity::projects::Entity::find()
        .filter(entity::projects::Column::OrgId.eq(org_id.as_uuid()))
        .order_by_asc(entity::projects::Column::Id)
        .limit(page_limit.saturating_add(1));
    if let Some(after) = cursor {
        query = query.filter(entity::projects::Column::Id.gt(after.as_uuid()));
    }
    let mut rows = query.all(conn).await?;
    let mut next_cursor = None;
    if rows.len() > usize::try_from(page_limit).unwrap_or(usize::MAX) {
        rows.truncate(usize::try_from(page_limit).unwrap_or(usize::MAX));
        if let Some(last) = rows.last() {
            next_cursor = Some(ProjectId::new(last.id));
        }
    }
    let projects = rows
        .into_iter()
        .map(ProjectRecord::try_from)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(ListProjectsPage {
        projects,
        next_cursor,
    })
}

/// Check whether the account holds a membership in the organization.
pub(crate) async fn membership_exists(
    conn: &sea_orm::DatabaseConnection,
    account_id: AccountId,
    org_id: OrgId,
) -> Result<bool, StoreError> {
    let row = entity::memberships::Entity::find()
        .filter(entity::memberships::Column::AccountId.eq(account_id.as_uuid()))
        .filter(entity::memberships::Column::OrgId.eq(org_id.as_uuid()))
        .one(conn)
        .await?;
    Ok(row.is_some())
}

/// Validate membership and delegate to [`set_active_organization`].
pub(crate) async fn set_active_organization_with_membership_check(
    conn: &sea_orm::DatabaseConnection,
    session_token: &SessionToken,
    account_id: AccountId,
    org_id: OrgId,
) -> Result<(), ActiveOrganizationError> {
    let is_member = membership_exists(conn, account_id, org_id).await?;
    if !is_member {
        return Err(ActiveOrganizationError::NotAMember);
    }
    set_active_organization(conn, session_token, account_id, org_id).await?;
    Ok(())
}

/// Read active org and list projects; returns empty page when no org is set.
pub(crate) async fn list_projects_for_active_org(
    conn: &sea_orm::DatabaseConnection,
    session_token: &SessionToken,
    limit: u64,
    cursor: Option<ProjectId>,
) -> Result<ListProjectsPage, StoreError> {
    let active_org = read_active_organization(conn, session_token).await?;
    let Some(org_id) = active_org else {
        return Ok(ListProjectsPage {
            projects: Vec::new(),
            next_cursor: None,
        });
    };
    list_projects_for_org(conn, org_id, limit, cursor).await
}

/// Seed a fixture project row directly into the database.
/// Only callable behind the `test-hooks` feature gate.
#[cfg(feature = "test-hooks")]
pub(crate) async fn insert_project(
    conn: &sea_orm::DatabaseConnection,
    project_id: Uuid,
    org_id: Uuid,
    name: &str,
    created_at: chrono::DateTime<chrono::Utc>,
) -> Result<ProjectRecord, StoreError> {
    use sea_orm::{ActiveModelTrait, Set};
    let model = entity::projects::ActiveModel {
        id: Set(project_id),
        org_id: Set(org_id),
        name: Set(name.to_owned()),
        created_at: Set(created_at),
    };
    let inserted = model.insert(conn).await?;
    ProjectRecord::try_from(inserted)
}
