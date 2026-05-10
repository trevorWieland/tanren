//! Test-only fixture seeders.
//!
//! Gated behind `test-hooks` so production binaries cannot seed fixtures.

use sea_orm::{ActiveModelTrait, Set};
use tanren_identity_policy::{PermissionGrantSource, PolicyConstraintSource};
use uuid::Uuid;

use crate::entity;
use crate::{
    InvitationRecord, NewInvitation, NewPermissionConstraint, NewPermissionGrant,
    PermissionConstraintRecord, PermissionGrantId, PermissionGrantRecord, PermissionGrantScope,
    Store, StoreError,
};

#[cfg(feature = "test-hooks")]
impl Store {
    /// Seed a fixture invitation row directly. Bypasses the (currently
    /// non-existent) invitation-creation flow so BDD scenarios can stage
    /// pending invitations without an inviting handler.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Database`] if the insert fails.
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

    /// Seed a fixture permission grant row directly for BDD scenarios.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Database`] if the insert fails.
    pub async fn seed_permission_grant(
        &self,
        new: NewPermissionGrant,
    ) -> Result<PermissionGrantRecord, StoreError> {
        let (org_id, project_id) = match new.scope {
            PermissionGrantScope::Organization(org_id) => (Some(org_id.as_uuid()), None),
            PermissionGrantScope::Project(project_id) => (None, Some(project_id.as_uuid())),
        };
        let role_template_name = match new.grant_source {
            PermissionGrantSource::Direct => None,
            PermissionGrantSource::RoleTemplate { role_template } => {
                Some(role_template.as_str().to_owned())
            }
        };
        let model = entity::permission_grants::ActiveModel {
            id: Set(PermissionGrantId::new(Uuid::now_v7()).as_uuid()),
            account_id: Set(new.account_id.as_uuid()),
            org_id: Set(org_id),
            project_id: Set(project_id),
            permission_name: Set(new.permission.as_str().to_owned()),
            role_template_name: Set(role_template_name),
            created_at: Set(new.created_at),
        };
        let inserted = model.insert(&self.conn).await?;
        Ok(PermissionGrantRecord::from(inserted))
    }

    /// Seed a fixture policy constraint row directly for BDD scenarios.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Database`] if the insert fails.
    pub async fn seed_permission_constraint(
        &self,
        new: NewPermissionConstraint,
    ) -> Result<PermissionConstraintRecord, StoreError> {
        let is_project_policy = match new.source {
            PolicyConstraintSource::OrganizationPolicy => false,
            PolicyConstraintSource::ProjectPolicy => true,
        };
        let model = entity::permission_constraints::ActiveModel {
            id: Set(Uuid::now_v7()),
            grant_id: Set(new.grant_id.as_uuid()),
            reason: Set(new.reason.as_str().to_owned()),
            is_project_policy: Set(is_project_policy),
            created_at: Set(new.created_at),
        };
        let inserted = model.insert(&self.conn).await?;
        Ok(PermissionConstraintRecord::from(inserted))
    }
}
