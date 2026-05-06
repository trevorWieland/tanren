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
mod notifications;
mod records;
mod traits;
mod user_configuration;

pub use migration::Migrator;
pub use records::{
    AccountRecord, CredentialRecord, InvitationRecord, MembershipRecord, NewAccount, NewCredential,
    NewInvitation, NewUserConfigValue, NotificationOrgOverrideRecord, NotificationPreferenceRecord,
    PendingNotificationRouteRecord, SessionRecord, UpdateCredential, UserConfigRecord,
};
pub use traits::{
    AcceptInvitationAtomicOutput, AcceptInvitationAtomicRequest, AcceptInvitationError,
    AcceptInvitationEventContext, AcceptInvitationEventsBuilder, AccountStore,
    ConsumeInvitationError, ConsumedInvitation,
};
pub(crate) use user_configuration::{
    credential_kind_from_db, credential_scope_from_db, setting_key_from_db,
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
use tanren_configuration_secrets::{
    CredentialId, NotificationChannelSet, NotificationEventType, UserSettingKey,
};
use tanren_identity_policy::{
    AccountId, Email, Identifier, InvitationToken, MembershipId, OrgId, SessionToken,
    ValidationError,
};
use thiserror::Error;
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
        accept_invitation::consume_invitation_standalone(&self.conn, token.as_str(), now).await
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

    async fn list_user_config(
        &self,
        account_id: AccountId,
    ) -> Result<Vec<UserConfigRecord>, StoreError> {
        user_configuration::list_user_config(&self.conn, account_id).await
    }

    async fn get_user_config(
        &self,
        account_id: AccountId,
        key: UserSettingKey,
    ) -> Result<Option<UserConfigRecord>, StoreError> {
        user_configuration::get_user_config(&self.conn, account_id, key).await
    }

    async fn set_user_config(
        &self,
        input: NewUserConfigValue,
    ) -> Result<UserConfigRecord, StoreError> {
        user_configuration::set_user_config(&self.conn, input).await
    }

    async fn remove_user_config(
        &self,
        account_id: AccountId,
        key: UserSettingKey,
        now: DateTime<Utc>,
    ) -> Result<bool, StoreError> {
        user_configuration::remove_user_config(&self.conn, account_id, key, now).await
    }

    async fn list_credentials(
        &self,
        account_id: AccountId,
    ) -> Result<Vec<CredentialRecord>, StoreError> {
        user_configuration::list_credentials(&self.conn, account_id).await
    }

    async fn get_credential(
        &self,
        id: CredentialId,
    ) -> Result<Option<CredentialRecord>, StoreError> {
        user_configuration::get_credential(&self.conn, id).await
    }

    async fn add_credential(&self, input: NewCredential) -> Result<CredentialRecord, StoreError> {
        user_configuration::add_credential(&self.conn, input).await
    }

    async fn update_credential(
        &self,
        input: UpdateCredential,
    ) -> Result<Option<CredentialRecord>, StoreError> {
        user_configuration::update_credential(&self.conn, input).await
    }

    async fn remove_credential(
        &self,
        id: CredentialId,
        now: DateTime<Utc>,
    ) -> Result<bool, StoreError> {
        user_configuration::remove_credential(&self.conn, id, now).await
    }

    async fn find_account_id_by_session_token(
        &self,
        token: &SessionToken,
        now: DateTime<Utc>,
    ) -> Result<Option<AccountId>, StoreError> {
        user_configuration::find_account_id_by_session_token(&self.conn, token.expose_secret(), now)
            .await
    }

    async fn upsert_notification_preference(
        &self,
        account_id: AccountId,
        event_type: NotificationEventType,
        enabled_channels: NotificationChannelSet,
        now: DateTime<Utc>,
    ) -> Result<NotificationPreferenceRecord, StoreError> {
        notifications::upsert_notification_preference(
            &self.conn,
            account_id,
            event_type,
            enabled_channels,
            now,
        )
        .await
    }

    async fn list_notification_preferences(
        &self,
        account_id: AccountId,
    ) -> Result<Vec<NotificationPreferenceRecord>, StoreError> {
        notifications::list_notification_preferences(&self.conn, account_id).await
    }

    async fn upsert_notification_org_override(
        &self,
        account_id: AccountId,
        org_id: OrgId,
        event_type: NotificationEventType,
        enabled_channels: NotificationChannelSet,
        now: DateTime<Utc>,
    ) -> Result<NotificationOrgOverrideRecord, StoreError> {
        notifications::upsert_notification_org_override(
            &self.conn,
            account_id,
            org_id,
            event_type,
            enabled_channels,
            now,
        )
        .await
    }

    async fn list_notification_org_overrides(
        &self,
        account_id: AccountId,
        org_id: OrgId,
    ) -> Result<Vec<NotificationOrgOverrideRecord>, StoreError> {
        notifications::list_notification_org_overrides(&self.conn, account_id, org_id).await
    }

    async fn upsert_pending_notification_route(
        &self,
        account_id: AccountId,
        event_type: NotificationEventType,
        channels_snapshot: NotificationChannelSet,
        overriding_org_id: Option<OrgId>,
        now: DateTime<Utc>,
    ) -> Result<PendingNotificationRouteRecord, StoreError> {
        notifications::upsert_pending_notification_route(
            &self.conn,
            account_id,
            event_type,
            channels_snapshot,
            overriding_org_id,
            now,
        )
        .await
    }

    async fn list_pending_notification_routes(
        &self,
        account_id: AccountId,
    ) -> Result<Vec<PendingNotificationRouteRecord>, StoreError> {
        notifications::list_pending_notification_routes(&self.conn, account_id).await
    }

    async fn is_account_member_of_org(
        &self,
        account_id: AccountId,
        org_id: OrgId,
    ) -> Result<bool, StoreError> {
        notifications::is_account_member_of_org(&self.conn, account_id, org_id).await
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
    /// A stored enum-string could not be deserialized back to its
    /// domain type. Indicates a row was written with a variant unknown
    /// to this build — surfaces as a distinct error for triage.
    #[error("data deserialization error in `{entity}.{column}`: {cause}")]
    Deserialization {
        entity: &'static str,
        column: &'static str,
        cause: String,
    },
}
