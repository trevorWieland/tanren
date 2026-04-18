use chrono::{DateTime, Utc};
use sea_orm::ActiveValue::Set;
use sea_orm::{ActiveModelTrait, EntityTrait};
use tanren_domain::EventId;

use crate::Store;
use crate::db_error_codes::{
    extract_db_error_code, is_postgres_unique_violation_code, is_sqlite_unique_violation_code,
};
use crate::entity::methodology_idempotency;
use crate::errors::{StoreError, StoreResult};

/// Stored idempotency ledger row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MethodologyIdempotencyEntry {
    pub tool: String,
    pub scope_key: String,
    pub idempotency_key: String,
    pub request_hash: String,
    pub response_json: Option<String>,
    pub first_event_id: Option<EventId>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Input for creating one reservation row in the idempotency ledger.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InsertMethodologyIdempotencyParams {
    pub tool: String,
    pub scope_key: String,
    pub idempotency_key: String,
    pub request_hash: String,
}

impl Store {
    /// Load one idempotency ledger row by `(tool, scope_key, idempotency_key)`.
    ///
    /// # Errors
    /// Returns a store/database error on query failure.
    pub async fn get_methodology_idempotency(
        &self,
        tool: &str,
        scope_key: &str,
        idempotency_key: &str,
    ) -> StoreResult<Option<MethodologyIdempotencyEntry>> {
        let row = methodology_idempotency::Entity::find_by_id((
            tool.to_owned(),
            scope_key.to_owned(),
            idempotency_key.to_owned(),
        ))
        .one(self.conn())
        .await?;
        Ok(row.map(|row| MethodologyIdempotencyEntry {
            tool: row.tool,
            scope_key: row.scope_key,
            idempotency_key: row.idempotency_key,
            request_hash: row.request_hash,
            response_json: row.response_json,
            first_event_id: row.first_event_id.map(EventId::from_uuid),
            created_at: row.created_at,
            updated_at: row.updated_at,
        }))
    }

    /// Insert a reservation row. Returns `Ok(false)` on unique collision.
    ///
    /// # Errors
    /// Returns database errors other than uniqueness collisions.
    pub async fn insert_methodology_idempotency_reservation(
        &self,
        params: InsertMethodologyIdempotencyParams,
    ) -> StoreResult<bool> {
        let now = Utc::now();
        let row = methodology_idempotency::ActiveModel {
            tool: Set(params.tool),
            scope_key: Set(params.scope_key),
            idempotency_key: Set(params.idempotency_key),
            request_hash: Set(params.request_hash),
            response_json: Set(None),
            first_event_id: Set(None),
            created_at: Set(now),
            updated_at: Set(now),
        };
        match methodology_idempotency::Entity::insert(row)
            .exec(self.conn())
            .await
        {
            Ok(_) => Ok(true),
            Err(err) => {
                let code = extract_db_error_code(&err);
                if code.as_deref().is_some_and(|c| {
                    is_sqlite_unique_violation_code(c) || is_postgres_unique_violation_code(c)
                }) {
                    Ok(false)
                } else {
                    Err(StoreError::from(err))
                }
            }
        }
    }

    /// Finalize a reservation row with a replay-safe response payload.
    ///
    /// # Errors
    /// Returns [`StoreError::NotFound`] if no reservation row exists.
    pub async fn finalize_methodology_idempotency(
        &self,
        tool: &str,
        scope_key: &str,
        idempotency_key: &str,
        response_json: String,
        first_event_id: Option<EventId>,
    ) -> StoreResult<()> {
        let Some(mut row) = methodology_idempotency::Entity::find_by_id((
            tool.to_owned(),
            scope_key.to_owned(),
            idempotency_key.to_owned(),
        ))
        .one(self.conn())
        .await?
        .map(methodology_idempotency::ActiveModel::from) else {
            return Err(StoreError::NotFound {
                entity_kind: tanren_domain::EntityKind::Spec,
                id: format!("methodology_idempotency:{tool}:{scope_key}:{idempotency_key}"),
            });
        };
        row.response_json = Set(Some(response_json));
        row.first_event_id = Set(first_event_id.map(EventId::into_uuid));
        row.updated_at = Set(Utc::now());
        row.update(self.conn()).await?;
        Ok(())
    }

    /// Delete a reservation row, used when a mutation fails before finalization.
    ///
    /// # Errors
    /// Returns a store/database error on delete failure.
    pub async fn delete_methodology_idempotency(
        &self,
        tool: &str,
        scope_key: &str,
        idempotency_key: &str,
    ) -> StoreResult<()> {
        let _ = methodology_idempotency::Entity::delete_by_id((
            tool.to_owned(),
            scope_key.to_owned(),
            idempotency_key.to_owned(),
        ))
        .exec(self.conn())
        .await?;
        Ok(())
    }
}
