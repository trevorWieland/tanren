//! Database access layer for Tanren.
//!
//! This crate is the **only** place in the workspace that owns SQL and
//! row-shape entities. Other crates consume typed envelopes through the
//! [`AccountStore`] port and the concrete [`Store`] adapter; the
//! underlying `SeaORM` entity types are intentionally crate-private
//! (`entity/` is a private module) so that row shape changes never leak
//! across the dependency boundary.

mod accept_invitation;
mod entity;
mod migration;
mod records;
mod traits;

#[cfg(feature = "test-hooks")]
mod test_hooks;

pub use migration::Migrator;
pub use records::{
    AccountRecord, CreateInvitation, InvitationRecord, MembershipRecord, NewAccount, NewInvitation,
    SessionRecord,
};
pub use traits::{
    AcceptInvitationAtomicOutput, AcceptInvitationAtomicRequest, AcceptInvitationError,
    AcceptInvitationEventContext, AcceptInvitationEventsBuilder, AccountStore,
    ConsumeInvitationError, ConsumedInvitation, RevokeInvitationError,
};

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Database, DatabaseConnection, DbErr, EntityTrait, QueryFilter,
    QueryOrder, QuerySelect, Set,
};
use sea_orm_migration::MigratorTrait;
use secrecy::SecretString;
use serde::{Deserialize, Serialize};
use tanren_identity_policy::{
    AccountId, Email, Identifier, InvitationToken, MembershipId, OrgId, OrganizationPermission,
    SessionToken, ValidationError,
};
use thiserror::Error;
use uuid::Uuid;

use crate::records::{granted_permissions_json, parse_permissions_json};

/// A connected handle to Tanren's canonical event store.
///
/// Construct via [`Store::connect`]; apply pending migrations via
/// [`Store::migrate`]. The handle is cheap to clone — under the hood
/// `SeaORM` pools connections.
///
/// All account-flow methods are exposed via the [`AccountStore`] trait
/// impl below; handlers depend on `&dyn AccountStore`, not on `Store`
/// directly.
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
///
/// Per architecture, payloads are JSON-serialised typed events. F-0001 ships
/// only the envelope shape; concrete event types arrive with later behavior
/// slices.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventEnvelope {
    /// UUID v7 — globally unique, time-ordered.
    pub id: Uuid,
    /// Wall-clock time the event was appended.
    pub occurred_at: DateTime<Utc>,
    /// Opaque JSON payload.
    pub payload: serde_json::Value,
}

impl Store {
    /// Connect to a database by URL (e.g. `postgres://...`).
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Database`] if the underlying `SeaORM` connect call
    /// fails.
    pub async fn connect(url: &str) -> Result<Self, StoreError> {
        let conn = Database::connect(url).await?;
        Ok(Self { conn })
    }

    /// Reference to the underlying `SeaORM` connection. Provided so app-services
    /// can run cross-cutting transactions; row-shape entity types remain
    /// crate-private.
    #[must_use]
    pub fn connection(&self) -> &DatabaseConnection {
        &self.conn
    }

    /// Apply all pending migrations.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Database`] if migration execution fails.
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
        permissions: Vec<OrganizationPermission>,
        now: DateTime<Utc>,
    ) -> Result<MembershipId, StoreError> {
        let id = MembershipId::fresh();
        let model = entity::memberships::ActiveModel {
            id: Set(id.as_uuid()),
            account_id: Set(account_id.as_uuid()),
            org_id: Set(org_id.as_uuid()),
            permissions: Set(granted_permissions_json(&permissions)),
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

    async fn create_invitation(
        &self,
        invitation: CreateInvitation,
    ) -> Result<InvitationRecord, StoreError> {
        let model = entity::invitations::ActiveModel {
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

    async fn list_invitations_by_org(
        &self,
        org_id: OrgId,
    ) -> Result<Vec<InvitationRecord>, StoreError> {
        let rows = entity::invitations::Entity::find()
            .filter(entity::invitations::Column::InvitingOrgId.eq(org_id.as_uuid()))
            .all(&self.conn)
            .await?;
        rows.into_iter().map(InvitationRecord::try_from).collect()
    }

    async fn list_invitations_by_recipient(
        &self,
        identifier: &Identifier,
    ) -> Result<Vec<InvitationRecord>, StoreError> {
        let rows = entity::invitations::Entity::find()
            .filter(entity::invitations::Column::RecipientIdentifier.eq(identifier.as_str()))
            .all(&self.conn)
            .await?;
        rows.into_iter().map(InvitationRecord::try_from).collect()
    }

    async fn revoke_invitation(
        &self,
        token: &InvitationToken,
        now: DateTime<Utc>,
    ) -> Result<InvitationRecord, RevokeInvitationError> {
        let token_owned = token.as_str().to_owned();
        let result = entity::invitations::Entity::update_many()
            .col_expr(
                entity::invitations::Column::RevokedAt,
                sea_orm::sea_query::Expr::value(Some(now)),
            )
            .filter(entity::invitations::Column::Token.eq(token_owned.clone()))
            .filter(entity::invitations::Column::ConsumedAt.is_null())
            .filter(entity::invitations::Column::RevokedAt.is_null())
            .exec(&self.conn)
            .await
            .map_err(StoreError::from)?;

        if result.rows_affected == 1 {
            let row = entity::invitations::Entity::find_by_id(token_owned)
                .one(&self.conn)
                .await
                .map_err(StoreError::from)?
                .ok_or_else(|| StoreError::DataInvariant {
                    column: "invitation_token",
                    cause: ValidationError::InvitationTokenEmpty,
                })?;
            return InvitationRecord::try_from(row).map_err(RevokeInvitationError::Store);
        }

        let existing = entity::invitations::Entity::find_by_id(token_owned)
            .one(&self.conn)
            .await
            .map_err(StoreError::from)?;
        Err(match existing {
            None => RevokeInvitationError::NotFound,
            Some(row) if row.consumed_at.is_some() => RevokeInvitationError::AlreadyConsumed,
            Some(row) if row.revoked_at.is_some() => RevokeInvitationError::AlreadyRevoked,
            Some(_) => RevokeInvitationError::AlreadyConsumed,
        })
    }

    async fn find_session_by_token(
        &self,
        token: &SessionToken,
        now: DateTime<Utc>,
    ) -> Result<Option<SessionRecord>, StoreError> {
        let row = entity::account_sessions::Entity::find()
            .filter(entity::account_sessions::Column::Token.eq(token.expose_secret()))
            .filter(entity::account_sessions::Column::ExpiresAt.gt(now))
            .one(&self.conn)
            .await?;
        Ok(row.map(SessionRecord::from))
    }

    async fn find_organization_permissions(
        &self,
        account_id: AccountId,
        org_id: OrgId,
    ) -> Result<Vec<OrganizationPermission>, StoreError> {
        let row = entity::memberships::Entity::find()
            .filter(entity::memberships::Column::AccountId.eq(account_id.as_uuid()))
            .filter(entity::memberships::Column::OrgId.eq(org_id.as_uuid()))
            .one(&self.conn)
            .await?;
        match row {
            Some(m) => parse_permissions_json(&m.permissions),
            None => Ok(Vec::new()),
        }
    }

    async fn consume_invitation(
        &self,
        token: &InvitationToken,
        now: DateTime<Utc>,
    ) -> Result<ConsumedInvitation, ConsumeInvitationError> {
        let token_owned = token.as_str().to_owned();
        let result = entity::invitations::Entity::update_many()
            .col_expr(
                entity::invitations::Column::ConsumedAt,
                sea_orm::sea_query::Expr::value(Some(now)),
            )
            .filter(entity::invitations::Column::Token.eq(token_owned.clone()))
            .filter(entity::invitations::Column::ConsumedAt.is_null())
            .filter(entity::invitations::Column::RevokedAt.is_null())
            .filter(entity::invitations::Column::ExpiresAt.gt(now))
            .exec(&self.conn)
            .await
            .map_err(StoreError::from)?;

        if result.rows_affected == 1 {
            let row = entity::invitations::Entity::find_by_id(token_owned)
                .one(&self.conn)
                .await
                .map_err(StoreError::from)?
                .ok_or_else(|| StoreError::DataInvariant {
                    column: "invitation_token",
                    cause: ValidationError::InvitationTokenEmpty,
                })?;
            let permissions = parse_permissions_json(&row.granted_permissions)
                .map_err(ConsumeInvitationError::Store)?;
            return Ok(ConsumedInvitation {
                inviting_org_id: OrgId::new(row.inviting_org_id),
                granted_permissions: permissions,
                expires_at: row.expires_at,
                consumed_at: row.consumed_at.unwrap_or(now),
            });
        }

        let existing = entity::invitations::Entity::find_by_id(token.as_str().to_owned())
            .one(&self.conn)
            .await
            .map_err(StoreError::from)?;
        Err(match existing {
            None => ConsumeInvitationError::NotFound,
            Some(row) if row.revoked_at.is_some() => ConsumeInvitationError::Revoked,
            Some(row) if row.consumed_at.is_some() => ConsumeInvitationError::AlreadyConsumed,
            Some(row) if row.expires_at <= now => ConsumeInvitationError::Expired,
            Some(_) => ConsumeInvitationError::AlreadyConsumed,
        })
    }

    async fn accept_invitation_atomic(
        &self,
        request: AcceptInvitationAtomicRequest,
    ) -> Result<AcceptInvitationAtomicOutput, AcceptInvitationError> {
        accept_invitation::run(&self.conn, request).await
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
}

/// Convert a DB-stored identifier string into an [`Identifier`]. Any
/// failure is a DB-invariant violation (we wrote the row through our
/// own validated path), so it surfaces as a distinct
/// [`StoreError::DataInvariant`] for triage rather than masquerading as
/// a query failure.
pub(crate) fn parse_db_identifier(raw: &str) -> Result<Identifier, StoreError> {
    Identifier::parse(raw).map_err(|err| StoreError::DataInvariant {
        column: "identifier",
        cause: err,
    })
}

/// Convert a DB-stored invitation token into an [`InvitationToken`].
pub(crate) fn parse_db_invitation_token(raw: &str) -> Result<InvitationToken, StoreError> {
    InvitationToken::parse(raw).map_err(|err| StoreError::DataInvariant {
        column: "invitation_token",
        cause: err,
    })
}

/// Wrap a raw string into a [`SecretString`]. Re-exported so callers
/// can build a [`SecretString`] without taking a direct `secrecy`
/// dependency.
#[must_use]
pub fn secret_from_string(value: String) -> SecretString {
    SecretString::from(value)
}

/// Errors raised by the store layer.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum StoreError {
    /// The underlying `SeaORM` call failed.
    #[error("database error: {0}")]
    Database(#[from] DbErr),
    /// A row read out of the database failed validation against a
    /// domain newtype's invariants. Indicates DB-side corruption — we
    /// only ever write rows through validated newtype constructors.
    #[error("data invariant violation in column `{column}`: {cause}")]
    DataInvariant {
        /// The column whose value failed to validate.
        column: &'static str,
        /// The underlying validation error.
        #[source]
        cause: ValidationError,
    },
}
