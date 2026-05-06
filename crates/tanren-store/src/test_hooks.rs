//! Test-only fixture seeders. Gated behind the `test-hooks` Cargo feature
//! so production binaries cannot accidentally seed test data.

use chrono::{DateTime, Utc};
use sea_orm::{ActiveModelTrait, Set};
use tanren_identity_policy::{AccountId, MembershipId, OrgId};

use crate::{
    InvitationRecord, NewInvitation, NewOrganization, NewProject, OrganizationRecord,
    ProjectRecord, Store, StoreError, entity,
};

#[cfg(feature = "test-hooks")]
impl Store {
    pub async fn seed_invitation(
        &self,
        new: NewInvitation,
    ) -> Result<InvitationRecord, StoreError> {
        let model = entity::invitations::ActiveModel {
            token: Set(new.token.as_str().to_owned()),
            inviting_org_id: Set(new.inviting_org_id.as_uuid()),
            expires_at: Set(new.expires_at),
            consumed_at: Set(None),
        };
        let inserted = model.insert(&self.conn).await?;
        InvitationRecord::try_from(inserted)
    }

    pub async fn seed_organization(
        &self,
        new: NewOrganization,
    ) -> Result<OrganizationRecord, StoreError> {
        let model = entity::organizations::ActiveModel {
            id: Set(new.id.as_uuid()),
            name: Set(new.name),
            created_at: Set(new.created_at),
        };
        let inserted = model.insert(&self.conn).await?;
        Ok(OrganizationRecord::from(inserted))
    }

    pub async fn seed_membership(
        &self,
        account_id: AccountId,
        org_id: OrgId,
        now: DateTime<Utc>,
    ) -> Result<MembershipId, StoreError> {
        let id = MembershipId::fresh();
        let model = entity::memberships::ActiveModel {
            id: Set(id.as_uuid()),
            account_id: Set(account_id.as_uuid()),
            org_id: Set(org_id.as_uuid()),
            created_at: Set(now),
        };
        model.insert(&self.conn).await?;
        Ok(id)
    }

    pub async fn seed_project(&self, new: NewProject) -> Result<ProjectRecord, StoreError> {
        let model = entity::projects::ActiveModel {
            id: Set(new.id.as_uuid()),
            org_id: Set(new.org_id.as_uuid()),
            name: Set(new.name),
            created_at: Set(new.created_at),
        };
        let inserted = model.insert(&self.conn).await?;
        Ok(ProjectRecord::from(inserted))
    }
}
