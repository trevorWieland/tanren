//! Provider connection store — event types, projection records, port trait,
//! and `SeaORM` adapter.
//!
//! Read paths return [`ProviderConnectionMetadata`] which contains no token
//! fields. Test-hooks gates fixture seeding; production uses the event-sourced
//! append path.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sea_orm::sea_query::OnConflict;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder, Set,
    TransactionTrait,
};
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use tanren_identity_policy::{AccountId, ProviderConnectionId, ProviderKind};
use uuid::Uuid;

use crate::StoreError;
use crate::entity;

// Event types

/// Connection initiated — pending until [`ProviderConnectionEstablished`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConnectionInitiated {
    /// The connection this event belongs to.
    pub connection_id: ProviderConnectionId,
    /// Account that initiated the connection.
    pub account_id: AccountId,
    /// Kind of provider being connected.
    pub provider_kind: ProviderKind,
    pub provider_name: String,
    pub external_account_id: String,
}

/// Connection successfully established — credential is valid.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConnectionEstablished {
    /// The connection this event belongs to.
    pub connection_id: ProviderConnectionId,
    /// Account that owns the connection.
    pub account_id: AccountId,
    /// Kind of provider.
    pub provider_kind: ProviderKind,
}

/// Connection failed during handshake.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConnectionFailed {
    /// The connection this event belongs to.
    pub connection_id: ProviderConnectionId,
    /// Account that owns the connection.
    pub account_id: AccountId,
    pub error_message: String,
}

// Projection read models (no secret fields)

/// Redacted read model — omits the opaque access credential.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderConnectionMetadata {
    /// Stable connection id.
    pub id: ProviderConnectionId,
    /// Owning account.
    pub account_id: AccountId,
    /// Provider kind (`source_control`, `identity`, ...).
    pub provider_kind: ProviderKind,
    /// Human-readable provider name.
    pub provider_name: String,
    /// Connection lifecycle status.
    pub status: ProviderConnectionStatus,
    /// Provider-specific user identifier.
    pub external_account_id: String,
    /// When the connection record was created.
    pub created_at: DateTime<Utc>,
    /// When the connection record was last updated.
    pub updated_at: DateTime<Utc>,
}

/// Closed set of provider connection lifecycle states.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ProviderConnectionStatus {
    /// Connection initiated but not yet confirmed.
    Initiated,
    /// Connection is active and usable.
    Established,
    /// Connection failed during handshake.
    Failed,
}

impl ProviderConnectionStatus {
    /// Stored string representation.
    #[must_use]
    pub const fn as_stored_value(self) -> &'static str {
        match self {
            Self::Initiated => "initiated",
            Self::Established => "established",
            Self::Failed => "failed",
        }
    }

    /// Parse a stored value back.
    #[must_use]
    pub fn from_stored_value(value: &str) -> Option<Self> {
        match value {
            "initiated" => Some(Self::Initiated),
            "established" => Some(Self::Established),
            "failed" => Some(Self::Failed),
            _ => None,
        }
    }
}

/// A reachable repository discovered via a source-control provider connection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReachableRepository {
    /// Stable row id.
    pub id: Uuid,
    /// The connection this repo was discovered through.
    pub connection_id: ProviderConnectionId,
    /// Full repository name (e.g. `"owner/repo"`).
    pub repository_full_name: String,
    /// Clone/browse URL.
    pub repository_url: String,
    /// When the repository was discovered.
    pub discovered_at: DateTime<Utc>,
}

// Input types

/// Input shape for creating a new provider connection with an initial event.
#[derive(Debug, Clone)]
pub struct NewProviderConnection {
    /// Stable id allocated by the caller.
    pub id: ProviderConnectionId,
    /// Owning account.
    pub account_id: AccountId,
    /// Provider kind.
    pub provider_kind: ProviderKind,
    /// Human-readable provider name.
    pub provider_name: String,
    /// Opaque encrypted access credential — stored as ciphertext.
    pub opaque_access_token: SecretString,
    /// Provider-specific user identifier.
    pub external_account_id: String,
    /// Wall-clock creation time.
    pub now: DateTime<Utc>,
}

/// Input shape for upserting reachable repositories.
#[derive(Debug, Clone)]
pub struct NewReachableRepository {
    /// Stable row id.
    pub id: Uuid,
    /// Connection this repo belongs to.
    pub connection_id: ProviderConnectionId,
    /// Full repository name.
    pub repository_full_name: String,
    /// Clone/browse URL.
    pub repository_url: String,
    /// When this repo was discovered.
    pub discovered_at: DateTime<Utc>,
}

/// Paginated result for reachable repository listing.
#[derive(Debug, Clone)]
pub struct PaginatedReachableRepositories {
    /// The page of repositories.
    pub items: Vec<ReachableRepository>,
    /// Total count across all pages.
    pub total: u64,
    /// Whether there are more pages.
    pub has_more: bool,
}

// Port trait

/// Port trait for provider connection persistence. Read paths return no
/// secret token columns.
#[async_trait]
pub trait ProviderConnectionStore: Send + Sync + std::fmt::Debug {
    /// Append a provider-connection event to the canonical event log and
    /// update the projection in one transaction. The event payload must
    /// be a serialised [`ProviderConnectionInitiated`],
    /// [`ProviderConnectionEstablished`], or [`ProviderConnectionFailed`].
    async fn append_provider_connection_event(
        &self,
        connection: NewProviderConnection,
        event_payload: serde_json::Value,
    ) -> Result<ProviderConnectionMetadata, StoreError>;

    /// Get connection metadata (no secret token) by connection id.
    async fn get_connection_metadata(
        &self,
        connection_id: ProviderConnectionId,
    ) -> Result<Option<ProviderConnectionMetadata>, StoreError>;

    /// List all connections for an account. Returns metadata only — no
    /// secret tokens.
    async fn list_connections(
        &self,
        account_id: AccountId,
    ) -> Result<Vec<ProviderConnectionMetadata>, StoreError>;

    /// List reachable repositories for a connection, paginated with a hard
    /// cap to bound first-run fetch cost. Returns results ordered by
    /// discovery time.
    async fn list_reachable_repositories(
        &self,
        connection_id: ProviderConnectionId,
        limit: u64,
        offset: u64,
    ) -> Result<PaginatedReachableRepositories, StoreError>;

    /// Replace the reachable repositories for a connection (upsert).
    /// Deletes existing entries and inserts the new set. Hard cap enforced
    /// by the caller — this method does not impose its own limit.
    async fn replace_reachable_repositories(
        &self,
        connection_id: ProviderConnectionId,
        repos: Vec<NewReachableRepository>,
    ) -> Result<(), StoreError>;

    /// Read the opaque access credential for a connection. Only used at
    /// the point where a provider adapter needs to make an outbound call.
    /// The secret is never logged, serialized, or returned in a read-model.
    async fn get_opaque_access_token(
        &self,
        connection_id: ProviderConnectionId,
    ) -> Result<Option<SecretString>, StoreError>;
}

// Entity → record conversions (private to crate)

impl TryFrom<entity::provider_connections::Model> for ProviderConnectionMetadata {
    type Error = StoreError;

    fn try_from(model: entity::provider_connections::Model) -> Result<Self, Self::Error> {
        let status =
            ProviderConnectionStatus::from_stored_value(&model.status).ok_or_else(|| {
                StoreError::DataInvariantDetail {
                    column: "status",
                    detail: format!(
                        "unsupported status `{}` in provider_connections",
                        model.status
                    ),
                }
            })?;
        let provider_kind =
            ProviderKind::from_stored_value(&model.provider_kind).ok_or_else(|| {
                StoreError::DataInvariantDetail {
                    column: "provider_kind",
                    detail: format!(
                        "unsupported provider_kind `{}` in provider_connections",
                        model.provider_kind
                    ),
                }
            })?;
        Ok(Self {
            id: ProviderConnectionId::new(model.id),
            account_id: AccountId::new(model.account_id),
            provider_kind,
            provider_name: model.provider_name,
            status,
            external_account_id: model.external_account_id,
            created_at: model.created_at,
            updated_at: model.updated_at,
        })
    }
}

impl From<entity::provider_connection_reachable_repos::Model> for ReachableRepository {
    fn from(model: entity::provider_connection_reachable_repos::Model) -> Self {
        Self {
            id: model.id,
            connection_id: ProviderConnectionId::new(model.connection_id),
            repository_full_name: model.repository_full_name,
            repository_url: model.repository_url,
            discovered_at: model.discovered_at,
        }
    }
}

// SeaORM adapter implementation

#[async_trait]
impl ProviderConnectionStore for crate::Store {
    async fn append_provider_connection_event(
        &self,
        connection: NewProviderConnection,
        event_payload: serde_json::Value,
    ) -> Result<ProviderConnectionMetadata, StoreError> {
        let tx = self.connection().begin().await?;

        let status_str = match event_payload.get("type").and_then(|v| v.as_str()) {
            Some("ProviderConnectionFailed") => "failed",
            Some("ProviderConnectionEstablished") => "established",
            _ => "initiated",
        };

        entity::provider_connections::Entity::insert(entity::provider_connections::ActiveModel {
            id: Set(connection.id.as_uuid()),
            account_id: Set(connection.account_id.as_uuid()),
            provider_kind: Set(connection.provider_kind.as_stored_value().to_owned()),
            provider_name: Set(connection.provider_name.clone()),
            status: Set(status_str.to_owned()),
            opaque_access_token: Set(connection.opaque_access_token.expose_secret().to_owned()),
            external_account_id: Set(connection.external_account_id.clone()),
            created_at: Set(connection.now),
            updated_at: Set(connection.now),
        })
        .on_conflict(
            OnConflict::column(entity::provider_connections::Column::Id)
                .update_columns([
                    entity::provider_connections::Column::Status,
                    entity::provider_connections::Column::OpaqueAccessToken,
                    entity::provider_connections::Column::UpdatedAt,
                ])
                .to_owned(),
        )
        .exec(&tx)
        .await?;

        let event_type_suffix = event_payload
            .get("type")
            .and_then(serde_json::Value::as_str)
            .map_or_else(|| "unknown".to_owned(), str::to_owned);
        let event_id = Uuid::now_v7();
        let mut event_model =
            crate::event_ops::event_active_model(event_id, connection.now, event_payload);
        event_model.event_type = Set(Some(format!("provider_connection.{event_type_suffix}")));
        event_model.actor = Set(Some(connection.account_id.to_string()));
        event_model.scope = Set(Some("account".to_owned()));
        event_model.resource_id = Set(Some(connection.id.as_uuid()));
        event_model.insert(&tx).await?;

        tx.commit().await?;

        let row = entity::provider_connections::Entity::find_by_id(connection.id.as_uuid())
            .one(self.connection())
            .await?
            .ok_or_else(|| StoreError::DataInvariantDetail {
                column: "id",
                detail: "provider connection row not found after insert".to_owned(),
            })?;
        ProviderConnectionMetadata::try_from(row)
    }

    async fn get_connection_metadata(
        &self,
        connection_id: ProviderConnectionId,
    ) -> Result<Option<ProviderConnectionMetadata>, StoreError> {
        let row = entity::provider_connections::Entity::find_by_id(connection_id.as_uuid())
            .one(self.connection())
            .await?;
        row.map(ProviderConnectionMetadata::try_from).transpose()
    }

    async fn list_connections(
        &self,
        account_id: AccountId,
    ) -> Result<Vec<ProviderConnectionMetadata>, StoreError> {
        let rows = entity::provider_connections::Entity::find()
            .filter(entity::provider_connections::Column::AccountId.eq(account_id.as_uuid()))
            .order_by_asc(entity::provider_connections::Column::CreatedAt)
            .all(self.connection())
            .await?;
        rows.into_iter()
            .map(ProviderConnectionMetadata::try_from)
            .collect()
    }

    async fn list_reachable_repositories(
        &self,
        connection_id: ProviderConnectionId,
        limit: u64,
        offset: u64,
    ) -> Result<PaginatedReachableRepositories, StoreError> {
        let query = entity::provider_connection_reachable_repos::Entity::find()
            .filter(
                entity::provider_connection_reachable_repos::Column::ConnectionId
                    .eq(connection_id.as_uuid()),
            )
            .order_by_asc(entity::provider_connection_reachable_repos::Column::DiscoveredAt);

        let page_size: u64 = limit.max(1);
        let paginator = query.paginate(self.connection(), page_size);
        let total = paginator.num_items().await?;
        let page = paginator.fetch_page(offset / page_size).await?;

        let items: Vec<ReachableRepository> =
            page.into_iter().map(ReachableRepository::from).collect();
        let has_more = (offset + page_size) < total;

        Ok(PaginatedReachableRepositories {
            items,
            total,
            has_more,
        })
    }

    async fn replace_reachable_repositories(
        &self,
        connection_id: ProviderConnectionId,
        repos: Vec<NewReachableRepository>,
    ) -> Result<(), StoreError> {
        let tx = self.connection().begin().await?;

        entity::provider_connection_reachable_repos::Entity::delete_many()
            .filter(
                entity::provider_connection_reachable_repos::Column::ConnectionId
                    .eq(connection_id.as_uuid()),
            )
            .exec(&tx)
            .await?;

        for repo in repos {
            entity::provider_connection_reachable_repos::ActiveModel {
                id: Set(repo.id),
                connection_id: Set(repo.connection_id.as_uuid()),
                repository_full_name: Set(repo.repository_full_name),
                repository_url: Set(repo.repository_url),
                discovered_at: Set(repo.discovered_at),
            }
            .insert(&tx)
            .await?;
        }

        tx.commit().await?;
        Ok(())
    }

    async fn get_opaque_access_token(
        &self,
        connection_id: ProviderConnectionId,
    ) -> Result<Option<SecretString>, StoreError> {
        let row = entity::provider_connections::Entity::find_by_id(connection_id.as_uuid())
            .one(self.connection())
            .await?;
        Ok(row.map(|r| SecretString::from(r.opaque_access_token)))
    }
}

/// Test-only fixture seeders gated behind `test-hooks`.
#[cfg(feature = "test-hooks")]
impl crate::Store {
    /// Seed a provider connection row directly. Bypasses the event-sourced
    /// append path so BDD scenarios can stage connection state without
    /// driving the full lifecycle.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Database`] if the insert fails.
    pub async fn seed_provider_connection(
        &self,
        new: NewProviderConnection,
        status: ProviderConnectionStatus,
    ) -> Result<ProviderConnectionMetadata, StoreError> {
        let model = entity::provider_connections::ActiveModel {
            id: Set(new.id.as_uuid()),
            account_id: Set(new.account_id.as_uuid()),
            provider_kind: Set(new.provider_kind.as_stored_value().to_owned()),
            provider_name: Set(new.provider_name.clone()),
            status: Set(status.as_stored_value().to_owned()),
            opaque_access_token: Set(new.opaque_access_token.expose_secret().to_owned()),
            external_account_id: Set(new.external_account_id.clone()),
            created_at: Set(new.now),
            updated_at: Set(new.now),
        };
        let inserted = model.insert(self.connection()).await?;
        ProviderConnectionMetadata::try_from(inserted)
    }

    /// Seed reachable repository rows directly. Bypasses the replace flow.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Database`] if any insert fails.
    pub async fn seed_reachable_repositories(
        &self,
        repos: Vec<NewReachableRepository>,
    ) -> Result<(), StoreError> {
        for repo in repos {
            entity::provider_connection_reachable_repos::ActiveModel {
                id: Set(repo.id),
                connection_id: Set(repo.connection_id.as_uuid()),
                repository_full_name: Set(repo.repository_full_name),
                repository_url: Set(repo.repository_url),
                discovered_at: Set(repo.discovered_at),
            }
            .insert(self.connection())
            .await?;
        }
        Ok(())
    }
}
