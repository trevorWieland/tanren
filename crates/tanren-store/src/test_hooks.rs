//! Test-only fixture seeders. Gated behind the `test-hooks` Cargo
//! feature so production binaries cannot accidentally seed test data;
//! the testkit (and only the testkit) enables the feature.

use sea_orm::{ActiveModelTrait, Set};
use uuid::Uuid;

use crate::records::granted_permissions_json;
use crate::{CreateInvitation, InvitationRecord, NewInvitation, Store, StoreError};

#[cfg(feature = "test-hooks")]
impl Store {
    /// Seed a fixture invitation row directly. Bypasses the
    /// invitation-creation flow so BDD scenarios can stage
    /// pending invitations without an inviting handler.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Database`] if the insert fails.
    pub async fn seed_invitation(
        &self,
        new: NewInvitation,
    ) -> Result<InvitationRecord, StoreError> {
        let model = crate::entity::invitations::ActiveModel {
            token: Set(new.token.as_str().to_owned()),
            inviting_org_id: Set(new.inviting_org_id.as_uuid()),
            recipient_identifier: Set("seed@fixture.test".to_owned()),
            granted_permissions: Set(granted_permissions_json(&[])),
            created_by_account_id: Set(Uuid::nil()),
            created_at: Set(new.expires_at),
            expires_at: Set(new.expires_at),
            consumed_at: Set(None),
            revoked_at: Set(None),
        };
        let inserted = model.insert(&self.conn).await?;
        InvitationRecord::try_from(inserted)
    }

    /// Seed a fixture organization invitation row directly. Supports
    /// recipient identifier and granted permissions for BDD scenarios
    /// that need to verify invitation metadata beyond the basic
    /// [`Store::seed_invitation`] path.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Database`] if the insert fails.
    pub async fn seed_organization_invitation(
        &self,
        invitation: CreateInvitation,
    ) -> Result<InvitationRecord, StoreError> {
        let model = crate::entity::invitations::ActiveModel {
            token: Set(invitation.token.as_str().to_owned()),
            inviting_org_id: Set(invitation.inviting_org_id.as_uuid()),
            recipient_identifier: Set(invitation.recipient_identifier.as_str().to_owned()),
            granted_permissions: Set(granted_permissions_json(&invitation.granted_permissions)),
            created_by_account_id: Set(invitation.created_by_account_id.as_uuid()),
            created_at: Set(invitation.created_at),
            expires_at: Set(invitation.expires_at),
            consumed_at: Set(None),
            revoked_at: Set(None),
        };
        let inserted = model.insert(&self.conn).await?;
        InvitationRecord::try_from(inserted)
    }
}
