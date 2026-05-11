//! `AccountStore` trait implementation for `Store`.
//!
//! Split from `lib.rs` to stay under the workspace per-file line budget.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect, Set,
};
use tanren_identity_policy::{
    AccountId, ApprovalPolicyId, ApprovalRule, Email, GatedAction, Identifier, InvitationToken,
    MembershipId, OrgId, OrganizationPermission, SessionToken,
};
use uuid::Uuid;

use crate::accept_invitation;
use crate::account_queries;
use crate::approval_policy;
use crate::create_organization;
use crate::entity;
use crate::records::ApprovalPolicyRecord;
use crate::traits::{
    AcceptInvitationAtomicOutput, AcceptInvitationAtomicRequest, AcceptInvitationError,
    ApprovalPolicyError, ApprovalPolicyPage, ConsumeInvitationError, ConsumedInvitation,
    CreateOrganizationAtomicOutput, CreateOrganizationAtomicRequest, CreateOrganizationError,
    LastOrganizationAdminGuardError, ListApprovalPoliciesRequest, ListOrganizationsPage,
};
use crate::{
    AccountRecord, AccountStore, EventEnvelope, InvitationRecord, NewAccount, SessionRecord, Store,
    StoreError,
};
use tanren_identity_policy::ValidationError;

#[async_trait]
impl AccountStore for Store {
    async fn find_account_by_identifier(
        &self,
        identifier: &Identifier,
    ) -> Result<Option<AccountRecord>, StoreError> {
        let row = entity::accounts::Entity::find()
            .filter(entity::accounts::Column::Identifier.eq(identifier.as_str()))
            .one(&self.conn)
            .await?;
        row.map(AccountRecord::try_from).transpose()
    }
    async fn find_account_by_email(
        &self,
        email: &Email,
    ) -> Result<Option<AccountRecord>, StoreError> {
        let identifier = Identifier::from_email(email);
        AccountStore::find_account_by_identifier(self, &identifier).await
    }
    async fn insert_account(&self, new: NewAccount) -> Result<AccountRecord, StoreError> {
        let model = entity::accounts::ActiveModel {
            id: Set(new.id.as_uuid()),
            identifier: Set(new.identifier.as_str().to_owned()),
            display_name: Set(new.display_name),
            password_phc: Set(new.password_phc),
            created_at: Set(new.created_at),
            org_id: Set(new.org_id.map(OrgId::as_uuid)),
        };
        let inserted = model.insert(&self.conn).await?;
        AccountRecord::try_from(inserted)
    }
    async fn insert_membership(
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
    async fn find_invitation_by_token(
        &self,
        token: &InvitationToken,
    ) -> Result<Option<InvitationRecord>, StoreError> {
        let row = entity::invitations::Entity::find_by_id(token.as_str().to_owned())
            .one(&self.conn)
            .await?;
        row.map(InvitationRecord::try_from).transpose()
    }
    async fn consume_invitation(
        &self,
        token: &InvitationToken,
        now: DateTime<Utc>,
    ) -> Result<ConsumedInvitation, ConsumeInvitationError> {
        // Single round-trip conditional UPDATE: only flip rows that are
        // still pending and not yet expired. SQLite serialises writes,
        // and a Postgres deployment relies on the partial-unique index
        // `idx_invitations_active_token` (see the
        // m20260503_000002_account_sessions_expires_at migration) to
        // belt-and-brace the same invariant.
        let token_owned = token.as_str().to_owned();
        let result = entity::invitations::Entity::update_many()
            .col_expr(
                entity::invitations::Column::ConsumedAt,
                sea_orm::sea_query::Expr::value(Some(now)),
            )
            .filter(entity::invitations::Column::Token.eq(token_owned.clone()))
            .filter(entity::invitations::Column::ConsumedAt.is_null())
            .filter(entity::invitations::Column::ExpiresAt.gt(now))
            .exec(&self.conn)
            .await
            .map_err(StoreError::from)?;
        if result.rows_affected == 1 {
            // Re-read the row to populate the success shape. The row is
            // already pinned to `consumed_at = now` so any concurrent
            // acceptance has lost the race and will see the same row in
            // its disambiguation read below.
            let row = entity::invitations::Entity::find_by_id(token_owned)
                .one(&self.conn)
                .await
                .map_err(StoreError::from)?
                .ok_or_else(|| StoreError::DataInvariant {
                    column: "invitation_token",
                    cause: ValidationError::InvitationTokenEmpty,
                })?;
            return Ok(ConsumedInvitation {
                inviting_org_id: OrgId::new(row.inviting_org_id),
                expires_at: row.expires_at,
                consumed_at: row.consumed_at.unwrap_or(now),
            });
        }
        // No row was transitioned. Disambiguate why.
        let existing = entity::invitations::Entity::find_by_id(token.as_str().to_owned())
            .one(&self.conn)
            .await
            .map_err(StoreError::from)?;
        match existing {
            None => Err(ConsumeInvitationError::NotFound),
            Some(row) if row.consumed_at.is_some() => Err(ConsumeInvitationError::AlreadyConsumed),
            Some(row) if row.expires_at <= now => Err(ConsumeInvitationError::Expired),
            // The row matched the WHERE clause when we read it but the
            // UPDATE reported zero rows-affected — this can only happen
            // if a concurrent caller transitioned-then-reset the row,
            // which the schema does not permit. Surface as
            // `AlreadyConsumed` because that's the racier-than-expected
            // failure shape the user-facing API exposes for any
            // already-locked invitation.
            Some(_) => Err(ConsumeInvitationError::AlreadyConsumed),
        }
    }
    async fn accept_invitation_atomic(
        &self,
        request: AcceptInvitationAtomicRequest,
    ) -> Result<AcceptInvitationAtomicOutput, AcceptInvitationError> {
        accept_invitation::run(&self.conn, request).await
    }
    async fn create_organization_atomic(
        &self,
        request: CreateOrganizationAtomicRequest,
    ) -> Result<CreateOrganizationAtomicOutput, CreateOrganizationError> {
        create_organization::run(&self.conn, request).await
    }
    async fn has_organization_permission(
        &self,
        account_id: AccountId,
        org_id: OrgId,
        permission: OrganizationPermission,
    ) -> Result<bool, StoreError> {
        create_organization::has_permission(&self.conn, account_id, org_id, permission).await
    }
    async fn enforce_not_last_organization_admin_holder(
        &self,
        account_id: AccountId,
        org_id: OrgId,
    ) -> Result<(), LastOrganizationAdminGuardError> {
        create_organization::enforce_not_last_admin_holder(&self.conn, account_id, org_id).await
    }
    async fn insert_session(
        &self,
        token: SessionToken,
        account_id: AccountId,
        now: DateTime<Utc>,
        expires_at: DateTime<Utc>,
    ) -> Result<SessionRecord, StoreError> {
        let model = entity::account_sessions::ActiveModel {
            token: Set(token.expose_secret().to_owned()),
            account_id: Set(account_id.as_uuid()),
            created_at: Set(now),
            expires_at: Set(expires_at),
        };
        model.insert(&self.conn).await?;
        Ok(SessionRecord {
            token,
            account_id,
            created_at: now,
            expires_at,
        })
    }
    async fn find_session_by_token(
        &self,
        token: &SessionToken,
    ) -> Result<Option<SessionRecord>, StoreError> {
        account_queries::find_session_by_token(&self.conn, token).await
    }
    async fn list_organizations_for_account(
        &self,
        account_id: AccountId,
        limit: u64,
        cursor: Option<MembershipId>,
        now: DateTime<Utc>,
    ) -> Result<ListOrganizationsPage, StoreError> {
        account_queries::list_organizations_for_account(&self.conn, account_id, limit, cursor, now)
            .await
    }
    async fn append_event(
        &self,
        payload: serde_json::Value,
        now: DateTime<Utc>,
    ) -> Result<EventEnvelope, StoreError> {
        let envelope = EventEnvelope {
            id: Uuid::now_v7(),
            occurred_at: now,
            payload,
        };
        let model = entity::events::ActiveModel {
            id: Set(envelope.id),
            occurred_at: Set(envelope.occurred_at),
            payload: Set(envelope.payload.clone()),
        };
        model.insert(&self.conn).await?;
        Ok(envelope)
    }
    async fn recent_events(&self, limit: u64) -> Result<Vec<EventEnvelope>, StoreError> {
        // Order by `occurred_at` first, then by `id` (UUIDv7) as a stable
        // tie-breaker. Without the secondary key, events landing inside the
        // same timestamp bucket can come back in different orders across
        // reads — replay correctness demands a total order.
        let rows = entity::events::Entity::find()
            .order_by_desc(entity::events::Column::OccurredAt)
            .order_by_desc(entity::events::Column::Id)
            .limit(limit)
            .all(&self.conn)
            .await?;
        Ok(rows.into_iter().map(EventEnvelope::from).collect())
    }
    async fn create_approval_policy_atomic(
        &self,
        policy_id: ApprovalPolicyId,
        org_id: OrgId,
        gated_action: &GatedAction,
        required_approvals: u16,
        permitted_approver_permission: &str,
        now: DateTime<Utc>,
    ) -> Result<ApprovalPolicyRecord, ApprovalPolicyError> {
        approval_policy::create_approval_policy_atomic(
            &self.conn,
            policy_id,
            org_id,
            gated_action,
            required_approvals,
            permitted_approver_permission,
            now,
        )
        .await
    }
    async fn update_approval_policy_atomic(
        &self,
        policy_id: ApprovalPolicyId,
        org_id: OrgId,
        required_approvals: u16,
        permitted_approver_permission: &str,
        expected_version: u16,
        now: DateTime<Utc>,
    ) -> Result<ApprovalPolicyRecord, ApprovalPolicyError> {
        approval_policy::update_approval_policy_atomic(
            &self.conn,
            policy_id,
            org_id,
            required_approvals,
            permitted_approver_permission,
            expected_version,
            now,
        )
        .await
    }
    async fn delete_approval_policy(
        &self,
        policy_id: ApprovalPolicyId,
        org_id: OrgId,
        expected_version: u16,
        now: DateTime<Utc>,
    ) -> Result<(), ApprovalPolicyError> {
        approval_policy::delete_approval_policy(
            &self.conn,
            policy_id,
            org_id,
            expected_version,
            now,
        )
        .await
    }
    async fn find_required_approval_for_action(
        &self,
        org_id: OrgId,
        action: &GatedAction,
    ) -> Result<Option<ApprovalRule>, StoreError> {
        Ok(
            approval_policy::find_required_approval_for_action(&self.conn, org_id, action)
                .await?
                .map(|r| ApprovalRule {
                    id: r.id,
                    org_id: r.org_id,
                    gated_action: r.gated_action,
                    required_approvals: r.required_approvals,
                    permitted_approver_permission: r.permitted_approver_permission,
                    version: r.version,
                }),
        )
    }
    async fn list_approval_policies(
        &self,
        request: ListApprovalPoliciesRequest,
    ) -> Result<ApprovalPolicyPage, StoreError> {
        approval_policy::list_approval_policies(&self.conn, request).await
    }
}
