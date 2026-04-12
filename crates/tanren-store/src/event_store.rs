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
    // Typed entity references must constrain both `entity_kind` and
    // `entity_id`. A bare `entity_id` filter would return events
    // from a different entity kind that happens to share the same
    // UUID — pathological, but not impossible.
    if let Some(ref entity_ref) = filter.entity_ref {
        query = query
            .filter(events::Column::EntityId.eq(entity_ref_to_id_string(entity_ref)))
            .filter(events::Column::EntityKind.eq(entity_ref.kind().to_string()));
    }
    if let Some(ref entity_refs) = filter.entity_refs {
        query = apply_entity_refs_filter(query, entity_refs);
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

/// Build the correct filter for a list of typed entity refs.
///
/// When all refs share the same kind (common case), we can emit a
/// single `entity_kind = ? AND entity_id IN (...)` which is index-
/// friendly. When they span multiple kinds, we emit an OR of
/// `(entity_kind = ? AND entity_id = ?)` pairs.
fn apply_entity_refs_filter(
    query: sea_orm::Select<events::Entity>,
    refs: &[tanren_domain::EntityRef],
) -> sea_orm::Select<events::Entity> {
    use sea_orm::sea_query::Condition;

    if refs.is_empty() {
        return query;
    }

    // Fast path: all refs share the same kind.
    let first_kind = refs[0].kind();
    let all_same_kind = refs.iter().all(|r| r.kind() == first_kind);
    if all_same_kind {
        let ids: Vec<String> = refs.iter().map(entity_ref_to_id_string).collect();
        return query
            .filter(events::Column::EntityKind.eq(first_kind.to_string()))
            .filter(events::Column::EntityId.is_in(ids));
    }

    // Slow path: mixed kinds — OR of (kind, id) pairs.
    let mut cond = Condition::any();
    for r in refs {
        cond = cond.add(
            Condition::all()
                .add(events::Column::EntityKind.eq(r.kind().to_string()))
                .add(events::Column::EntityId.eq(entity_ref_to_id_string(r))),
        );
    }
    query.filter(cond)
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

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};
    use sea_orm::{DbBackend, QueryTrait};
    use tanren_domain::{
        ApiKeyId, DispatchId, EntityKind, EntityRef, EventId, LeaseId, OrgId, ProjectId, StepId,
        TeamId, UserId,
    };

    use super::*;

    fn filter_sql(filter: &EventFilter) -> String {
        build_select_query(filter).build(DbBackend::Postgres).sql
    }

    #[test]
    fn empty_filter_has_no_where_clause() {
        let sql = filter_sql(&EventFilter::new());
        assert!(!sql.contains("WHERE"), "unexpected WHERE: {sql}");
    }

    #[test]
    fn entity_ref_filter_constrains_both_kind_and_id() {
        let id = DispatchId::new();
        let filter = EventFilter {
            entity_ref: Some(EntityRef::Dispatch(id)),
            ..EventFilter::new()
        };
        let sql = filter_sql(&filter);
        assert!(
            sql.contains("\"entity_id\" ="),
            "missing entity_id eq: {sql}"
        );
        assert!(
            sql.contains("\"entity_kind\" ="),
            "entity_ref must also constrain entity_kind: {sql}"
        );
    }

    #[test]
    fn entity_refs_same_kind_emits_kind_plus_in() {
        let filter = EventFilter {
            entity_refs: Some(vec![
                EntityRef::Dispatch(DispatchId::new()),
                EntityRef::Dispatch(DispatchId::new()),
            ]),
            ..EventFilter::new()
        };
        let sql = filter_sql(&filter);
        assert!(
            sql.contains("\"entity_kind\" ="),
            "same-kind refs must constrain kind: {sql}"
        );
        assert!(sql.contains("\"entity_id\" IN"), "missing IN: {sql}");
    }

    #[test]
    fn entity_refs_mixed_kinds_emits_or_of_kind_id_pairs() {
        let filter = EventFilter {
            entity_refs: Some(vec![
                EntityRef::Dispatch(DispatchId::new()),
                EntityRef::Step(StepId::new()),
            ]),
            ..EventFilter::new()
        };
        let sql = filter_sql(&filter);
        // Mixed kinds: should have OR conditions with entity_kind
        assert!(
            sql.contains("\"entity_kind\" ="),
            "mixed-kind refs must include kind constraints: {sql}"
        );
        assert!(sql.contains("OR"), "mixed-kind refs must use OR: {sql}");
    }

    #[test]
    fn entity_kind_filter_emits_entity_kind_equality() {
        let filter = EventFilter {
            entity_kind: Some(EntityKind::Dispatch),
            ..EventFilter::new()
        };
        let sql = filter_sql(&filter);
        assert!(
            sql.contains("\"entity_kind\" ="),
            "missing entity_kind: {sql}"
        );
    }

    #[test]
    fn event_type_filter_emits_event_type_equality() {
        let filter = EventFilter {
            event_type: Some("dispatch_created".to_owned()),
            ..EventFilter::new()
        };
        let sql = filter_sql(&filter);
        assert!(
            sql.contains("\"event_type\" ="),
            "missing event_type: {sql}"
        );
    }

    #[test]
    fn since_filter_emits_timestamp_gte() {
        let filter = EventFilter {
            since: Some(Utc.timestamp_opt(1_700_000_000, 0).single().expect("ts")),
            ..EventFilter::new()
        };
        let sql = filter_sql(&filter);
        assert!(sql.contains("\"timestamp\" >="), "missing since: {sql}");
    }

    #[test]
    fn until_filter_emits_timestamp_lt() {
        let filter = EventFilter {
            until: Some(Utc.timestamp_opt(1_700_000_000, 0).single().expect("ts")),
            ..EventFilter::new()
        };
        let sql = filter_sql(&filter);
        assert!(sql.contains("\"timestamp\" <"), "missing until: {sql}");
    }

    #[test]
    fn entity_ref_to_id_string_covers_every_variant() {
        let event_id = EventId::from_uuid(uuid::Uuid::now_v7());
        // Touch the unused EventId import so it is reachable.
        let _ = event_id;
        let refs = [
            EntityRef::Dispatch(DispatchId::new()),
            EntityRef::Step(StepId::new()),
            EntityRef::Lease(LeaseId::new()),
            EntityRef::User(UserId::new()),
            EntityRef::Org(OrgId::new()),
            EntityRef::Team(TeamId::new()),
            EntityRef::Project(ProjectId::new()),
            EntityRef::ApiKey(ApiKeyId::new()),
        ];
        for r in refs {
            // Every variant must produce a 36-char hyphenated UUID.
            let id = entity_ref_to_id_string(&r);
            assert_eq!(id.len(), 36);
            assert_eq!(id.chars().filter(|c| *c == '-').count(), 4);
        }
    }
}
