//! Test-only fixture seeders. Gated behind the `test-hooks` Cargo
//! feature so production binaries cannot accidentally seed test data.

use chrono::{DateTime, Utc};
use sea_orm::{ActiveModelTrait, Set};
use tanren_identity_policy::{AccountId, MembershipId, OrgId, OrgPermissions};
use uuid::Uuid;

use crate::entity;
use crate::{
    InvitationRecord, MemberInFlightWorkRecord, MembershipRecord, NewInvitation, StoreError,
};

#[cfg(feature = "test-hooks")]
impl crate::Store {
    /// Seed a fixture invitation row directly.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Database`] if the insert fails.
    pub async fn seed_invitation(
        &self,
        new: NewInvitation,
    ) -> Result<InvitationRecord, StoreError> {
        let now = Utc::now();
        let model = entity::invitations::ActiveModel {
            token: Set(new.token.as_str().to_owned()),
            inviting_org_id: Set(new.inviting_org_id.as_uuid()),
            expires_at: Set(new.expires_at),
            consumed_at: Set(None),
            target_identifier: Set(new
                .target_identifier
                .as_ref()
                .map(|i| i.as_str().to_owned())),
            org_permissions: Set(new.org_permissions.as_ref().map(|p| p.as_str().to_owned())),
            revoked_at: Set(new.revoked.then_some(now)),
            revoked_by: Set(new.revoked.then_some(AccountId::fresh().as_uuid())),
            consumed_by: Set(None),
        };
        let inserted = model.insert(&self.conn).await?;
        InvitationRecord::try_from(inserted)
    }

    /// Seed a fixture invitation with a raw (unvalidated) `org_permissions`
    /// column value.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Database`] if the insert fails.
    pub async fn seed_invitation_raw_permissions(
        &self,
        new: NewInvitation,
        raw_org_permissions: Option<String>,
    ) -> Result<(), StoreError> {
        let now = Utc::now();
        let model = entity::invitations::ActiveModel {
            token: Set(new.token.as_str().to_owned()),
            inviting_org_id: Set(new.inviting_org_id.as_uuid()),
            expires_at: Set(new.expires_at),
            consumed_at: Set(None),
            target_identifier: Set(new
                .target_identifier
                .as_ref()
                .map(|i| i.as_str().to_owned())),
            org_permissions: Set(raw_org_permissions),
            revoked_at: Set(new.revoked.then_some(now)),
            revoked_by: Set(new.revoked.then_some(AccountId::fresh().as_uuid())),
            consumed_by: Set(None),
        };
        model.insert(&self.conn).await?;
        Ok(())
    }

    /// Seed a fixture membership with explicit org permissions. Used by
    /// BDD scenarios that need to stage members with specific permission
    /// levels (e.g. admin) without going through the invitation flow.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Database`] if the insert fails.
    pub async fn seed_membership(
        &self,
        account_id: AccountId,
        org_id: OrgId,
        org_permissions: Option<OrgPermissions>,
        now: DateTime<Utc>,
    ) -> Result<MembershipRecord, StoreError> {
        let id = MembershipId::fresh();
        let model = entity::memberships::ActiveModel {
            id: Set(id.as_uuid()),
            account_id: Set(account_id.as_uuid()),
            org_id: Set(org_id.as_uuid()),
            created_at: Set(now),
            org_permissions: Set(org_permissions.map(|p| p.as_str().to_owned())),
        };
        let inserted = model.insert(&self.conn).await?;
        MembershipRecord::try_from(inserted)
    }

    /// Seed a fixture in-flight work row. Used by BDD scenarios to
    /// represent work a departing member has outstanding.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Database`] if the insert fails.
    pub async fn seed_in_flight_work(
        &self,
        account_id: AccountId,
        org_id: OrgId,
        description: String,
        now: DateTime<Utc>,
    ) -> Result<MemberInFlightWorkRecord, StoreError> {
        let model = entity::member_in_flight_work::ActiveModel {
            id: Set(Uuid::now_v7()),
            account_id: Set(account_id.as_uuid()),
            org_id: Set(org_id.as_uuid()),
            description: Set(description),
            created_at: Set(now),
        };
        let inserted = model.insert(&self.conn).await?;
        MemberInFlightWorkRecord::try_from(inserted)
    }
}
