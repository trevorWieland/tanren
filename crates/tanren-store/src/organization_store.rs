use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder,
    QuerySelect, Set,
};
use tanren_identity_policy::{AccountId, OrgId, OrgPermission};
use uuid::Uuid;

use crate::create_organization;
use crate::entity;
use crate::records::{self, MembershipRecord, OrganizationRecord, SessionRecord};
use crate::traits::{
    CreateOrganizationAtomicOutput, CreateOrganizationAtomicRequest, CreateOrganizationError,
    OrganizationStore,
};
use crate::{Store, StoreError};

#[async_trait]
impl OrganizationStore for Store {
    async fn create_organization(
        &self,
        request: CreateOrganizationAtomicRequest,
    ) -> Result<CreateOrganizationAtomicOutput, CreateOrganizationError> {
        create_organization::run(&self.conn, request).await
    }

    async fn list_account_organizations(
        &self,
        account_id: AccountId,
        limit: u32,
        after: Option<OrgId>,
    ) -> Result<Vec<OrganizationRecord>, StoreError> {
        let membership_rows = entity::memberships::Entity::find()
            .filter(entity::memberships::Column::AccountId.eq(account_id.as_uuid()))
            .all(&self.conn)
            .await?;
        let org_ids: Vec<Uuid> = membership_rows.iter().map(|m| m.org_id).collect();
        if org_ids.is_empty() {
            return Ok(Vec::new());
        }
        let mut query = entity::organizations::Entity::find()
            .filter(entity::organizations::Column::Id.is_in(org_ids));
        if let Some(after_id) = after {
            query = query.filter(entity::organizations::Column::Id.lt(after_id.as_uuid()));
        }
        let org_rows = query
            .order_by_desc(entity::organizations::Column::CreatedAt)
            .order_by_desc(entity::organizations::Column::Id)
            .limit(u64::from(limit) + 1)
            .all(&self.conn)
            .await?;
        org_rows
            .into_iter()
            .map(OrganizationRecord::try_from)
            .collect()
    }

    async fn find_membership(
        &self,
        account_id: AccountId,
        org_id: OrgId,
    ) -> Result<Option<MembershipRecord>, StoreError> {
        let row = entity::memberships::Entity::find()
            .filter(entity::memberships::Column::AccountId.eq(account_id.as_uuid()))
            .filter(entity::memberships::Column::OrgId.eq(org_id.as_uuid()))
            .one(&self.conn)
            .await?;
        Ok(row.map(MembershipRecord::from))
    }

    async fn grant_organization_permission(
        &self,
        org_id: OrgId,
        account_id: AccountId,
        permission: OrgPermission,
        now: DateTime<Utc>,
    ) -> Result<(), StoreError> {
        let model = entity::organization_permission_grants::ActiveModel {
            id: Set(Uuid::now_v7()),
            org_id: Set(org_id.as_uuid()),
            account_id: Set(account_id.as_uuid()),
            permission: Set(records::org_permission_to_str(permission).to_owned()),
            granted_at: Set(now),
        };
        model.insert(&self.conn).await?;
        Ok(())
    }

    async fn has_organization_permission(
        &self,
        org_id: OrgId,
        account_id: AccountId,
        permission: OrgPermission,
    ) -> Result<bool, StoreError> {
        let perm_str = records::org_permission_to_str(permission).to_owned();
        let count = entity::organization_permission_grants::Entity::find()
            .filter(entity::organization_permission_grants::Column::OrgId.eq(org_id.as_uuid()))
            .filter(
                entity::organization_permission_grants::Column::AccountId.eq(account_id.as_uuid()),
            )
            .filter(entity::organization_permission_grants::Column::Permission.eq(perm_str))
            .count(&self.conn)
            .await?;
        Ok(count > 0)
    }

    async fn count_permission_holders(
        &self,
        org_id: OrgId,
        permission: OrgPermission,
    ) -> Result<u64, StoreError> {
        let perm_str = records::org_permission_to_str(permission).to_owned();
        let count = entity::organization_permission_grants::Entity::find()
            .filter(entity::organization_permission_grants::Column::OrgId.eq(org_id.as_uuid()))
            .filter(entity::organization_permission_grants::Column::Permission.eq(perm_str))
            .count(&self.conn)
            .await?;
        Ok(count)
    }

    async fn count_organization_projects(&self, _org_id: OrgId) -> Result<u64, StoreError> {
        Ok(0)
    }

    async fn resolve_bearer_session(
        &self,
        token: &str,
        now: DateTime<Utc>,
    ) -> Result<Option<SessionRecord>, StoreError> {
        let row = entity::account_sessions::Entity::find()
            .filter(entity::account_sessions::Column::Token.eq(token))
            .filter(entity::account_sessions::Column::ExpiresAt.gt(now))
            .one(&self.conn)
            .await?;
        Ok(row.map(SessionRecord::from))
    }
}
