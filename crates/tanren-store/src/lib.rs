//! Database access layer for Tanren.
//!
//! The only place in the workspace that owns SQL and row-shape entities.
//! Other crates consume typed envelopes through the [`AccountStore`] port
//! and the concrete [`Store`] adapter.

mod accept_invitation;
mod account_queries;
mod active_organization;
mod create_organization;
mod entity;
mod migration;
mod organization_constraints;
mod records;
mod traits;

pub use migration::Migrator;
pub use records::{
    AccountRecord, InvitationRecord, MembershipRecord, NewAccount, NewInvitation,
    OrganizationCreateIdempotencyRecord, OrganizationPermissionGrantRecord, OrganizationRecord,
    ProjectRecord, SessionRecord,
};
pub use traits::{
    AcceptInvitationAtomicOutput, AcceptInvitationAtomicRequest, AcceptInvitationError,
    AcceptInvitationEventContext, AcceptInvitationEventsBuilder, AccountStore,
    ActiveOrganizationError, ConsumeInvitationError, ConsumedInvitation,
    CreateOrganizationAtomicOutput, CreateOrganizationAtomicRequest, CreateOrganizationError,
    CreateOrganizationEventContext, CreateOrganizationEventsBuilder, EventReference,
    LastOrganizationAdminGuardError, ListOrganizationsPage, ListProjectsPage,
    ListedOrganizationRecord,
};

use async_trait::async_trait;
use chrono::{DateTime, Utc};
pub(crate) use organization_constraints::{
    OrganizationCreateConstraint, classify_organization_create_constraint,
};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Database, DatabaseConnection, DbErr, EntityTrait, QueryFilter,
    QueryOrder, QuerySelect, Set,
};
use sea_orm_migration::MigratorTrait;
use secrecy::SecretString;
use serde::{Deserialize, Serialize};
use tanren_identity_policy::{
    AccountId, Email, IdempotencyKey, Identifier, InvitationToken, MembershipId, OrgId,
    OrganizationName, OrganizationPermission, ProjectId, ProjectName, SessionToken,
    ValidationError,
};
use thiserror::Error;
use uuid::Uuid;

/// Connected handle to Tanren's event store. Cheap to clone.
///
/// Construct via [`Store::connect`]; apply migrations via [`Store::migrate`].
/// Account-flow methods live on the [`AccountStore`] trait impl.
pub struct Store {
    conn: DatabaseConnection,
}

impl std::fmt::Debug for Store {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Store").finish_non_exhaustive()
    }
}

impl Clone for Store {
    fn clone(&self) -> Self {
        Self {
            conn: self.conn.clone(),
        }
    }
}

/// A row in Tanren's canonical event log.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventEnvelope {
    pub id: Uuid,
    pub occurred_at: DateTime<Utc>,
    pub payload: serde_json::Value,
}

impl Store {
    /// Connect to a database by URL.
    pub async fn connect(url: &str) -> Result<Self, StoreError> {
        let conn = Database::connect(url).await?;
        Ok(Self { conn })
    }

    /// Underlying `SeaORM` connection reference.
    #[must_use]
    pub fn connection(&self) -> &DatabaseConnection {
        &self.conn
    }

    /// Apply all pending migrations.
    pub async fn migrate(&self) -> Result<(), StoreError> {
        Migrator::up(&self.conn, None).await?;
        Ok(())
    }
}

impl From<entity::events::Model> for EventEnvelope {
    fn from(model: entity::events::Model) -> Self {
        Self {
            id: model.id,
            occurred_at: model.occurred_at,
            payload: model.payload,
        }
    }
}

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
            active_org_id: Set(None),
        };
        model.insert(&self.conn).await?;
        Ok(SessionRecord {
            token,
            account_id,
            created_at: now,
            expires_at,
            active_org_id: None,
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
        let rows = entity::events::Entity::find()
            .order_by_desc(entity::events::Column::OccurredAt)
            .order_by_desc(entity::events::Column::Id)
            .limit(limit)
            .all(&self.conn)
            .await?;
        Ok(rows.into_iter().map(EventEnvelope::from).collect())
    }

    async fn set_active_organization(
        &self,
        session_token: &SessionToken,
        account_id: AccountId,
        org_id: OrgId,
    ) -> Result<(), ActiveOrganizationError> {
        active_organization::set_active_organization_with_membership_check(
            &self.conn,
            session_token,
            account_id,
            org_id,
        )
        .await
    }

    async fn clear_active_organization(
        &self,
        session_token: &SessionToken,
        account_id: AccountId,
    ) -> Result<(), ActiveOrganizationError> {
        active_organization::clear_active_organization(&self.conn, session_token, account_id)
            .await
            .map_err(ActiveOrganizationError::from)
    }

    async fn read_active_organization(
        &self,
        session_token: &SessionToken,
    ) -> Result<Option<OrgId>, StoreError> {
        active_organization::read_active_organization(&self.conn, session_token).await
    }

    async fn list_projects_for_active_organization(
        &self,
        session_token: &SessionToken,
        limit: u64,
        cursor: Option<ProjectId>,
    ) -> Result<ListProjectsPage, StoreError> {
        active_organization::list_projects_for_active_org(&self.conn, session_token, limit, cursor)
            .await
    }

    async fn list_projects_for_organization(
        &self,
        org_id: OrgId,
        limit: u64,
        cursor: Option<ProjectId>,
    ) -> Result<ListProjectsPage, StoreError> {
        active_organization::list_projects_for_org(&self.conn, org_id, limit, cursor).await
    }
}

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

    pub async fn seed_project(
        &self,
        project_id: ProjectId,
        org_id: OrgId,
        name: &ProjectName,
        created_at: DateTime<Utc>,
    ) -> Result<ProjectRecord, StoreError> {
        active_organization::insert_project(
            &self.conn,
            project_id.as_uuid(),
            org_id.as_uuid(),
            name.as_str(),
            created_at,
        )
        .await
    }
}

pub(crate) fn parse_db_identifier(raw: &str) -> Result<Identifier, StoreError> {
    Identifier::parse(raw).map_err(|err| StoreError::DataInvariant {
        column: "identifier",
        cause: err,
    })
}

pub(crate) fn parse_db_invitation_token(raw: &str) -> Result<InvitationToken, StoreError> {
    InvitationToken::parse(raw).map_err(|err| StoreError::DataInvariant {
        column: "invitation_token",
        cause: err,
    })
}

pub(crate) fn parse_db_organization_name(raw: &str) -> Result<OrganizationName, StoreError> {
    OrganizationName::parse(raw).map_err(|err| StoreError::DataInvariant {
        column: "organization_name",
        cause: err,
    })
}

pub(crate) fn parse_db_idempotency_key(raw: &str) -> Result<IdempotencyKey, StoreError> {
    IdempotencyKey::parse(raw).map_err(|err| StoreError::DataInvariant {
        column: "idempotency_key",
        cause: err,
    })
}

pub(crate) fn parse_db_organization_permission(
    raw: &str,
) -> Result<OrganizationPermission, StoreError> {
    raw.parse::<OrganizationPermission>()
        .map_err(|_| StoreError::InvalidPermissionKey {
            column: "permission",
            value: raw.to_owned(),
        })
}

pub(crate) fn parse_db_project_name(raw: &str) -> Result<ProjectName, StoreError> {
    ProjectName::parse(raw).map_err(|err| StoreError::DataInvariant {
        column: "project_name",
        cause: err,
    })
}

#[must_use]
pub fn secret_from_string(value: String) -> SecretString {
    SecretString::from(value)
}

/// Errors raised by the store layer.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum StoreError {
    #[error("database error: {0}")]
    Database(#[from] DbErr),
    #[error("data invariant violation in column `{column}`: {cause}")]
    DataInvariant {
        /// The column whose value failed to validate.
        column: &'static str,
        /// The underlying validation error.
        #[source]
        cause: ValidationError,
    },
    #[error("unknown permission key in column `{column}`: {value}")]
    InvalidPermissionKey {
        /// Column that contained the unknown value.
        column: &'static str,
        /// Raw unknown key.
        value: String,
    },
    /// A session was not found or has expired.
    #[error("session not found")]
    SessionNotFound,
}
