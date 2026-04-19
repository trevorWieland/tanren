use chrono::Utc;
use sea_orm::ActiveValue::Set;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect,
    TransactionTrait,
};
use tanren_domain::events::{DomainEvent, EventEnvelope};
use tanren_domain::{EventId, SpecId};

use crate::Store;
use crate::converters::events as event_converters;
use crate::entity::{events, methodology_phase_event_outbox};
use crate::errors::{StoreError, StoreResult};

const OUTBOX_STATUS_PENDING: &str = "pending";
const OUTBOX_STATUS_PROJECTED: &str = "projected";

/// One pending file-projection row for `phase-events.jsonl`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhaseEventOutboxEntry {
    pub event_id: EventId,
    pub spec_id: SpecId,
    pub spec_folder: String,
    pub line_json: String,
    pub attempt_count: u32,
}

/// One projected row used by strict append-only verification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhaseEventProjectedOutboxEntry {
    pub event_id: EventId,
    pub line_json: String,
}

/// Extra outbox payload written alongside one methodology event append.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppendPhaseEventOutboxParams {
    pub spec_id: SpecId,
    pub spec_folder: String,
    pub line_json: String,
}

impl Store {
    /// Append one methodology event and enqueue one outbox projection row
    /// in the same database transaction.
    ///
    /// # Errors
    /// Returns [`StoreError::Conversion`] for non-methodology payloads
    /// and a store/database error for transactional failures.
    pub async fn append_methodology_event_with_outbox(
        &self,
        event: &EventEnvelope,
        outbox: Option<AppendPhaseEventOutboxParams>,
    ) -> StoreResult<()> {
        if !matches!(event.payload, DomainEvent::Methodology { .. }) {
            return Err(StoreError::Conversion {
                context: "append_methodology_event_with_outbox",
                reason: "expected DomainEvent::Methodology payload".into(),
            });
        }
        let model = event_converters::envelope_to_active_model(event)?;
        let outbox_model = outbox.map(|item| methodology_phase_event_outbox::ActiveModel {
            event_id: Set(event.event_id.into_uuid()),
            spec_id: Set(item.spec_id.into_uuid()),
            spec_folder: Set(item.spec_folder),
            line_json: Set(item.line_json),
            status: Set(OUTBOX_STATUS_PENDING.to_owned()),
            attempt_count: Set(0),
            last_error: Set(None),
            created_at: Set(Utc::now()),
            projected_at: Set(None),
        });
        self.conn()
            .transaction::<_, (), StoreError>(move |txn| {
                Box::pin(async move {
                    events::Entity::insert(model).exec(txn).await?;
                    if let Some(outbox_model) = outbox_model {
                        methodology_phase_event_outbox::Entity::insert(outbox_model)
                            .exec(txn)
                            .await?;
                    }
                    Ok(())
                })
            })
            .await?;
        Ok(())
    }

    /// Append a methodology event without an outbox projection row.
    ///
    /// # Errors
    /// See [`Self::append_methodology_event_with_outbox`].
    pub async fn append_methodology_event(&self, event: &EventEnvelope) -> StoreResult<()> {
        self.append_methodology_event_with_outbox(event, None).await
    }

    /// Load pending outbox rows in deterministic creation order.
    ///
    /// # Errors
    /// Returns a store/database error on query failure.
    pub async fn load_pending_phase_event_outbox(
        &self,
        spec_id: Option<SpecId>,
        limit: u64,
    ) -> StoreResult<Vec<PhaseEventOutboxEntry>> {
        let mut query = methodology_phase_event_outbox::Entity::find()
            .filter(methodology_phase_event_outbox::Column::Status.eq(OUTBOX_STATUS_PENDING))
            .order_by_asc(methodology_phase_event_outbox::Column::CreatedAt);
        if let Some(spec_id) = spec_id {
            query = query
                .filter(methodology_phase_event_outbox::Column::SpecId.eq(spec_id.into_uuid()));
        }
        let rows = query.limit(limit.max(1)).all(self.conn()).await?;
        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            let attempt_count =
                u32::try_from(row.attempt_count).map_err(|_| StoreError::Conversion {
                    context: "load_pending_phase_event_outbox",
                    reason: "attempt_count is negative".into(),
                })?;
            out.push(PhaseEventOutboxEntry {
                event_id: EventId::from_uuid(row.event_id),
                spec_id: SpecId::from_uuid(row.spec_id),
                spec_folder: row.spec_folder,
                line_json: row.line_json,
                attempt_count,
            });
        }
        Ok(out)
    }

    /// Load projected outbox rows for specific event ids.
    ///
    /// # Errors
    /// Returns a store/database error on query failure.
    pub async fn load_projected_phase_event_outbox_by_event_ids(
        &self,
        spec_id: SpecId,
        spec_folder: &str,
        event_ids: &[EventId],
    ) -> StoreResult<Vec<PhaseEventProjectedOutboxEntry>> {
        if event_ids.is_empty() {
            return Ok(Vec::new());
        }
        let event_ids: Vec<uuid::Uuid> = event_ids
            .iter()
            .map(|event_id| event_id.into_uuid())
            .collect();
        let rows = methodology_phase_event_outbox::Entity::find()
            .filter(methodology_phase_event_outbox::Column::Status.eq(OUTBOX_STATUS_PROJECTED))
            .filter(methodology_phase_event_outbox::Column::SpecId.eq(spec_id.into_uuid()))
            .filter(methodology_phase_event_outbox::Column::SpecFolder.eq(spec_folder))
            .filter(methodology_phase_event_outbox::Column::EventId.is_in(event_ids))
            .order_by_asc(methodology_phase_event_outbox::Column::CreatedAt)
            .order_by_asc(methodology_phase_event_outbox::Column::EventId)
            .all(self.conn())
            .await?;
        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            out.push(PhaseEventProjectedOutboxEntry {
                event_id: EventId::from_uuid(row.event_id),
                line_json: row.line_json,
            });
        }
        Ok(out)
    }

    /// Increment attempt counter for an outbox row before file-write.
    ///
    /// # Errors
    /// Returns [`StoreError::NotFound`] when `event_id` is unknown.
    pub async fn increment_phase_event_outbox_attempt(&self, event_id: EventId) -> StoreResult<()> {
        let Some(mut row) =
            methodology_phase_event_outbox::Entity::find_by_id(event_id.into_uuid())
                .one(self.conn())
                .await?
                .map(methodology_phase_event_outbox::ActiveModel::from)
        else {
            return Err(StoreError::NotFound {
                entity_kind: tanren_domain::EntityKind::Spec,
                id: format!("phase_event_outbox:{event_id}"),
            });
        };
        let attempt = row.attempt_count.take().unwrap_or(0).saturating_add(1);
        row.attempt_count = Set(attempt);
        row.last_error = Set(None);
        row.update(self.conn()).await?;
        Ok(())
    }

    /// Mark an outbox row as projected after a successful file append.
    ///
    /// # Errors
    /// Returns [`StoreError::NotFound`] when `event_id` is unknown.
    pub async fn mark_phase_event_outbox_projected(&self, event_id: EventId) -> StoreResult<()> {
        let Some(mut row) =
            methodology_phase_event_outbox::Entity::find_by_id(event_id.into_uuid())
                .one(self.conn())
                .await?
                .map(methodology_phase_event_outbox::ActiveModel::from)
        else {
            return Err(StoreError::NotFound {
                entity_kind: tanren_domain::EntityKind::Spec,
                id: format!("phase_event_outbox:{event_id}"),
            });
        };
        row.status = Set(OUTBOX_STATUS_PROJECTED.to_owned());
        row.projected_at = Set(Some(Utc::now()));
        row.last_error = Set(None);
        row.update(self.conn()).await?;
        Ok(())
    }

    /// Reset an outbox row to pending with an error marker.
    ///
    /// # Errors
    /// Returns [`StoreError::NotFound`] when `event_id` is unknown.
    pub async fn mark_phase_event_outbox_pending_error(
        &self,
        event_id: EventId,
        error: &str,
    ) -> StoreResult<()> {
        let Some(mut row) =
            methodology_phase_event_outbox::Entity::find_by_id(event_id.into_uuid())
                .one(self.conn())
                .await?
                .map(methodology_phase_event_outbox::ActiveModel::from)
        else {
            return Err(StoreError::NotFound {
                entity_kind: tanren_domain::EntityKind::Spec,
                id: format!("phase_event_outbox:{event_id}"),
            });
        };
        row.status = Set(OUTBOX_STATUS_PENDING.to_owned());
        row.last_error = Set(Some(error.to_owned()));
        row.update(self.conn()).await?;
        Ok(())
    }
}
