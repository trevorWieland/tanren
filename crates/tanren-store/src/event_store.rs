//! [`EventStore`] trait and its implementation on [`Store`].
//!
//! `append` / `append_batch` insert rows directly via the entity API;
//! `query_events` builds a filtered `Select` and runs it plus a
//! companion `count()` query for the paginated view the domain
//! exposes. Every filter dimension is backed by an index in
//! `migration::m_0001_init`, so no scan-heavy paths exist here.

use async_trait::async_trait;
use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder, QuerySelect};
use tanren_domain::{EventEnvelope, EventQueryResult};

use crate::converters::events as event_converters;
use crate::entity::events;
use crate::errors::StoreResult;
use crate::params::EventFilter;
use crate::store::Store;

/// Append-only event log interface.
#[async_trait]
pub trait EventStore: Send + Sync {
    /// Append a single event.
    async fn append(&self, event: &EventEnvelope) -> StoreResult<()>;

    /// Append a batch of events in one transaction.
    async fn append_batch(&self, events: &[EventEnvelope]) -> StoreResult<()>;

    /// Query events by filter dimensions. Indexed; no table scans.
    async fn query_events(&self, filter: &EventFilter) -> StoreResult<EventQueryResult>;
}

#[async_trait]
impl EventStore for Store {
    async fn append(&self, event: &EventEnvelope) -> StoreResult<()> {
        let model = event_converters::envelope_to_active_model(event)?;
        events::Entity::insert(model).exec(self.conn()).await?;
        Ok(())
    }

    async fn append_batch(&self, envelopes: &[EventEnvelope]) -> StoreResult<()> {
        if envelopes.is_empty() {
            return Ok(());
        }
        let mut rows = Vec::with_capacity(envelopes.len());
        for envelope in envelopes {
            rows.push(event_converters::envelope_to_active_model(envelope)?);
        }
        events::Entity::insert_many(rows).exec(self.conn()).await?;
        Ok(())
    }

    async fn query_events(&self, filter: &EventFilter) -> StoreResult<EventQueryResult> {
        let conn = self.conn();
        let total_count = build_count_query(filter).count(conn).await?;

        let rows = build_select_query(filter)
            .limit(filter.limit)
            .offset(filter.offset)
            .order_by_asc(events::Column::Timestamp)
            .order_by_asc(events::Column::Id)
            .all(conn)
            .await?;

        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            out.push(event_converters::model_to_envelope(row)?);
        }

        let has_more = filter
            .offset
            .saturating_add(u64::try_from(out.len()).unwrap_or(u64::MAX))
            < total_count;

        Ok(EventQueryResult {
            events: out,
            total_count,
            has_more,
        })
    }
}

fn build_select_query(filter: &EventFilter) -> sea_orm::Select<events::Entity> {
    apply_filters(events::Entity::find(), filter)
}

fn build_count_query(filter: &EventFilter) -> sea_orm::Select<events::Entity> {
    apply_filters(events::Entity::find(), filter)
}

fn apply_filters(
    mut query: sea_orm::Select<events::Entity>,
    filter: &EventFilter,
) -> sea_orm::Select<events::Entity> {
    if let Some(ref entity_ref) = filter.entity_ref {
        let entity_id = entity_ref_to_id_string(entity_ref);
        query = query.filter(events::Column::EntityId.eq(entity_id));
    }
    if let Some(ref entity_refs) = filter.entity_refs {
        let ids: Vec<String> = entity_refs.iter().map(entity_ref_to_id_string).collect();
        query = query.filter(events::Column::EntityId.is_in(ids));
    }
    if let Some(entity_kind) = filter.entity_kind {
        query = query.filter(events::Column::EntityKind.eq(entity_kind.to_string()));
    }
    if let Some(ref event_type) = filter.event_type {
        query = query.filter(events::Column::EventType.eq(event_type.as_str()));
    }
    if let Some(since) = filter.since {
        query = query.filter(events::Column::Timestamp.gte(since));
    }
    if let Some(until) = filter.until {
        query = query.filter(events::Column::Timestamp.lt(until));
    }
    query
}

fn entity_ref_to_id_string(entity_ref: &tanren_domain::EntityRef) -> String {
    match *entity_ref {
        tanren_domain::EntityRef::Dispatch(id) => id.into_uuid().to_string(),
        tanren_domain::EntityRef::Step(id) => id.into_uuid().to_string(),
        tanren_domain::EntityRef::Lease(id) => id.into_uuid().to_string(),
        tanren_domain::EntityRef::User(id) => id.into_uuid().to_string(),
        tanren_domain::EntityRef::Org(id) => id.into_uuid().to_string(),
        tanren_domain::EntityRef::Team(id) => id.into_uuid().to_string(),
        tanren_domain::EntityRef::Project(id) => id.into_uuid().to_string(),
        tanren_domain::EntityRef::ApiKey(id) => id.into_uuid().to_string(),
    }
}
