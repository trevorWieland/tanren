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
mod event_ops;
mod migration;
mod provider_connection;
mod records;
mod traits;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
pub use event_ops::EventEnvelope;
pub use migration::Migrator;
pub use provider_connection::{
    NewProviderConnection, NewReachableRepository, PaginatedReachableRepositories,
    ProviderConnectionEstablished, ProviderConnectionFailed, ProviderConnectionInitiated,
    ProviderConnectionMetadata, ProviderConnectionStatus, ProviderConnectionStore,
    ReachableRepository,
};
pub use records::{
    AccountRecord, DeploymentPosture, DeploymentPostureRecord, DeploymentPostureScope,
    InvitationRecord, MembershipRecord, NewAccount, NewDeploymentPosture, NewInvitation,
    SessionRecord,
};
use sea_orm::sea_query::OnConflict;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Database, DatabaseConnection, DbErr, EntityTrait, QueryFilter,
    QueryOrder, QuerySelect, Set, TransactionTrait,
};
use sea_orm_migration::MigratorTrait;
use secrecy::SecretString;
use tanren_identity_policy::{
    AccountId, Email, Identifier, InvitationToken, MembershipId, OrgId, SessionToken,
    ValidationError,
};
use thiserror::Error;
pub use traits::{
    AcceptInvitationAtomicOutput, AcceptInvitationAtomicRequest, AcceptInvitationError,
    AcceptInvitationEventContext, AcceptInvitationEventsBuilder, AccountStore,
    ConsumeInvitationError, ConsumedInvitation, DeploymentPostureMutationRecord,
    DeploymentPostureStore, ResolvedDeploymentPostureScope,
};
use uuid::Uuid;

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

    async fn find_active_session_by_token(
        &self,
        token: &SessionToken,
        now: DateTime<Utc>,
    ) -> Result<Option<SessionRecord>, StoreError> {
        let row = entity::account_sessions::Entity::find_by_id(token.expose_secret().to_owned())
            .filter(entity::account_sessions::Column::ExpiresAt.gt(now))
            .one(&self.conn)
            .await?;
        Ok(row.map(SessionRecord::from))
    }

    async fn append_event(
        &self,
        payload: serde_json::Value,
        now: DateTime<Utc>,
    ) -> Result<EventEnvelope, StoreError> {
        let id = Uuid::now_v7();
        let model = event_ops::event_active_model(id, now, payload);
        let inserted = model.insert(&self.conn).await?;
        // `model.insert()` returns the row with database-assigned values.
        // The `position` column is populated by the `events_position_seq`
        // sequence default — the canonical global ordering authority.
        Ok(EventEnvelope::from(inserted))
    }

    async fn recent_events(&self, limit: u64) -> Result<Vec<EventEnvelope>, StoreError> {
        // The global `position` is the canonical ordering authority for
        // replay, cursors, and subscriptions per state.md § Event Log.
        // The column is NOT NULL — every row has a database-assigned
        // position value.
        let rows = entity::events::Entity::find()
            .order_by_desc(entity::events::Column::Position)
            .limit(limit)
            .all(&self.conn)
            .await?;
        Ok(rows.into_iter().map(EventEnvelope::from).collect())
    }
}

#[async_trait]
impl DeploymentPostureStore for Store {
    async fn resolve_deployment_posture_scope(
        &self,
        scope: DeploymentPostureScope,
    ) -> Result<Option<ResolvedDeploymentPostureScope>, StoreError> {
        match scope {
            DeploymentPostureScope::Account { account_id } => {
                let account_exists = entity::accounts::Entity::find_by_id(account_id.as_uuid())
                    .one(&self.conn)
                    .await?
                    .is_some();
                if account_exists {
                    Ok(Some(ResolvedDeploymentPostureScope::Account { account_id }))
                } else {
                    Ok(None)
                }
            }
            // Project and installation scope resolvers are intentionally
            // typed but unresolved until their owning read models land.
            DeploymentPostureScope::Project { .. }
            | DeploymentPostureScope::Installation { .. } => Ok(None),
        }
    }

    async fn get_deployment_posture(
        &self,
        scope: DeploymentPostureScope,
    ) -> Result<Option<DeploymentPostureRecord>, StoreError> {
        let (scope_kind, scope_id) = records::DeploymentPostureScopeKind::from_scope(scope);
        let row = entity::deployment_postures::Entity::find_by_id((
            scope_kind.as_stored_value().to_owned(),
            scope_id,
        ))
        .one(&self.conn)
        .await?;
        row.map(DeploymentPostureRecord::try_from).transpose()
    }

    async fn upsert_deployment_posture_with_event(
        &self,
        new: NewDeploymentPosture,
        event_payload: serde_json::Value,
    ) -> Result<DeploymentPostureMutationRecord, StoreError> {
        let (scope_kind, scope_id) = records::DeploymentPostureScopeKind::from_scope(new.scope);
        let tx = self.conn.begin().await?;

        entity::deployment_postures::Entity::insert(entity::deployment_postures::ActiveModel {
            scope_kind: Set(scope_kind.as_stored_value().to_owned()),
            scope_id: Set(scope_id),
            posture: Set(new.posture.as_stored_value().to_owned()),
            changed_by: Set(new.changed_by.as_uuid()),
            changed_at: Set(new.changed_at),
        })
        .on_conflict(
            OnConflict::columns([
                entity::deployment_postures::Column::ScopeKind,
                entity::deployment_postures::Column::ScopeId,
            ])
            .update_columns([
                entity::deployment_postures::Column::Posture,
                entity::deployment_postures::Column::ChangedBy,
                entity::deployment_postures::Column::ChangedAt,
            ])
            .to_owned(),
        )
        .exec(&tx)
        .await?;

        let event_id = Uuid::now_v7();
        let mut evt = event_ops::event_active_model(event_id, new.changed_at, event_payload);
        evt.event_type = Set(Some("deployment_posture.changed".to_owned()));
        evt.insert(&tx).await?;
        tx.commit().await?;
        Ok(DeploymentPostureMutationRecord {
            record: DeploymentPostureRecord {
                scope: new.scope,
                posture: new.posture,
                changed_by: new.changed_by,
                changed_at: new.changed_at,
            },
            audit_reference: event_id.to_string(),
        })
    }
}

/// Test-only fixture seeders. Gated behind the `test-hooks` Cargo feature
/// so production binaries cannot accidentally seed test data; the testkit
/// (and only the testkit) enables the feature.
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
    /// A row read out of the database failed a closed-value invariant
    /// that does not map to a `tanren-identity-policy` validation
    /// error (for example an unexpected enum literal).
    #[error("data invariant violation in column `{column}`: {detail}")]
    DataInvariantDetail {
        /// The column whose value failed to validate.
        column: &'static str,
        /// Human-readable detail about the violated invariant.
        detail: String,
    },
}
