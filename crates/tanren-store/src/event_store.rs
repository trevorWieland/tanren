//! [`EventStore`] trait and its implementation on [`Store`].
//!
//! `append` / `append_batch` insert rows directly via the entity API;
//! `query_events` builds a filtered `Select` and runs it plus a
//! companion `count()` query for the paginated view the domain
//! exposes. Every filter dimension is backed by an index in
//! `migration::m_0001_init`, so no scan-heavy paths exist here.

use async_trait::async_trait;
use sea_orm::{
    ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder, QuerySelect,
    TransactionTrait,
};
use tanren_domain::{DomainEvent, EventCursor, EventEnvelope, EventQueryResult};

use crate::converters::events as event_converters;
use crate::entity::events;
use crate::errors::{StoreError, StoreResult};
use crate::params::{EventFilter, ReplayGuard};
use crate::store::Store;
use crate::token_replay_store::consume_replay_guard_once;

/// Append-only event log interface.
///
/// Event appends are not exposed on this trait — they are only
/// available through co-transactional store methods (e.g.
/// [`JobQueue::enqueue_step`](crate::JobQueue::enqueue_step),
/// [`StateStore::create_dispatch_projection`](crate::StateStore::create_dispatch_projection))
/// that guarantee event/projection consistency. Direct append is
/// available via `Store::append` and `Store::append_batch` (behind
/// the `test-hooks` feature) for testing and migration scenarios.
#[async_trait]
pub trait EventStore: Send + Sync {
    /// Query events by filter dimensions. Indexed; no table scans.
    async fn query_events(&self, filter: &EventFilter) -> StoreResult<EventQueryResult>;

    /// Append a typed policy-decision audit event.
    ///
    /// Only `DomainEvent::PolicyDecision` payloads are accepted.
    async fn append_policy_decision_event(&self, event: &EventEnvelope) -> StoreResult<()>;

    /// Append a typed policy-decision audit event **and** consume a
    /// caller-supplied replay guard atomically in one transaction.
    ///
    /// This is the single entry point for recording mutating-command
    /// denial decisions: the replay key must be consumed regardless of
    /// whether the mutation is carried out, so denied requests cannot
    /// be replayed repeatedly for the same signed actor token.
    ///
    /// Only `DomainEvent::PolicyDecision` payloads are accepted. If the
    /// replay key has already been consumed this call returns
    /// [`StoreError::ReplayRejected`] and does not append the event.
    async fn record_policy_decision_with_replay(
        &self,
        event: &EventEnvelope,
        replay_guard: ReplayGuard,
    ) -> StoreResult<()>;
}

/// Direct event-append methods — available for testing and migration
/// scenarios but deliberately excluded from the [`EventStore`] trait
/// to prevent event/projection divergence in normal operation.
///
/// Gated behind the `test-hooks` feature to prevent downstream
/// crates from accidentally bypassing projection consistency.
#[cfg(feature = "test-hooks")]
impl Store {
    /// Append a single event. Bypasses projection consistency — use
    /// co-transactional trait methods for operational writes.
    pub async fn append(&self, event: &EventEnvelope) -> StoreResult<()> {
        let model = event_converters::envelope_to_active_model(event)?;
        events::Entity::insert(model).exec(self.conn()).await?;
        Ok(())
    }

    /// Append a batch of events. Bypasses projection consistency —
    /// use co-transactional trait methods for operational writes.
    pub async fn append_batch(&self, envelopes: &[EventEnvelope]) -> StoreResult<()> {
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
}

#[async_trait]
impl EventStore for Store {
    async fn query_events(&self, filter: &EventFilter) -> StoreResult<EventQueryResult> {
        let conn = self.conn();
        // `total_count` describes the full filtered result set, not the
        // post-cursor remainder — pagination must not change the count.
        let total_count = if filter.include_total_count {
            Some(build_count_query(filter).count(conn).await?)
        } else {
            None
        };

        // Explicit empty-page case: a caller asking for `limit = 0`
        // gets back an empty page with `has_more = false` and
        // `next_cursor = None` regardless of how many rows match.
        if filter.limit == 0 {
            return Ok(EventQueryResult {
                events: Vec::new(),
                total_count,
                has_more: false,
                next_cursor: None,
            });
        }

        let mut rows = build_select_query(filter)
            .order_by_asc(events::Column::Timestamp)
            .order_by_asc(events::Column::Id)
            .limit(filter.limit.saturating_add(1))
            .all(conn)
            .await?;
        let page_size = usize::try_from(filter.limit).unwrap_or(usize::MAX);
        let has_more = rows.len() > page_size;
        if has_more {
            rows.truncate(page_size);
        }
        let next_cursor = if has_more {
            rows.last().map(|row| EventCursor {
                timestamp: row.timestamp,
                id: row.id,
            })
        } else {
            None
        };

        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            out.push(event_converters::model_to_envelope(row)?);
        }

        Ok(EventQueryResult {
            events: out,
            total_count,
            has_more,
            next_cursor,
        })
    }

    async fn append_policy_decision_event(&self, event: &EventEnvelope) -> StoreResult<()> {
        ensure_policy_decision_payload(event)?;
        let row = event_converters::envelope_to_active_model(event)?;
        events::Entity::insert(row).exec(self.conn()).await?;
        Ok(())
    }

    async fn record_policy_decision_with_replay(
        &self,
        event: &EventEnvelope,
        replay_guard: ReplayGuard,
    ) -> StoreResult<()> {
        ensure_policy_decision_payload(event)?;
        let row = event_converters::envelope_to_active_model(event)?;

        self.conn()
            .transaction::<_, (), StoreError>(move |txn| {
                Box::pin(async move {
                    consume_replay_guard_once(txn, replay_guard).await?;
                    events::Entity::insert(row).exec(txn).await?;
                    Ok(())
                })
            })
            .await?;
        Ok(())
    }
}

fn ensure_policy_decision_payload(event: &EventEnvelope) -> StoreResult<()> {
    if matches!(event.payload, DomainEvent::PolicyDecision { .. }) {
        return Ok(());
    }
    Err(crate::errors::StoreError::Conversion {
        context: "event_store::append_policy_decision_event",
        reason: "expected DomainEvent::PolicyDecision payload".to_owned(),
    })
}

fn build_select_query(filter: &EventFilter) -> sea_orm::Select<events::Entity> {
    apply_filters(
        events::Entity::find(),
        filter,
        /* include_cursor */ true,
    )
}

/// Count query intentionally **omits** the cursor predicate so that
/// `total_count` reports the total number of rows matching the
/// caller's filter — independent of which page the caller is on.
/// Including the cursor would make `total_count` shrink as the
/// caller paginates, which is not what callers expect from a
/// "total" field.
fn build_count_query(filter: &EventFilter) -> sea_orm::Select<events::Entity> {
    apply_filters(
        events::Entity::find(),
        filter,
        /* include_cursor */ false,
    )
}

fn apply_filters(
    mut query: sea_orm::Select<events::Entity>,
    filter: &EventFilter,
    include_cursor: bool,
) -> sea_orm::Select<events::Entity> {
    use sea_orm::sea_query::Condition;

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
    if include_cursor {
        if let Some(cursor) = filter.cursor {
            query = query.filter(
                Condition::any()
                    .add(events::Column::Timestamp.gt(cursor.timestamp))
                    .add(
                        Condition::all()
                            .add(events::Column::Timestamp.eq(cursor.timestamp))
                            .add(events::Column::Id.gt(cursor.id)),
                    ),
            );
        }
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
    fn count_query_omits_cursor_predicate_so_total_is_pagination_independent() {
        // Build a select that DOES include the cursor and a count
        // that does NOT, then compare emitted SQL: the select must
        // mention `id >` (the cursor tiebreaker) and the count must
        // not.
        let cursor = EventCursor {
            timestamp: Utc.timestamp_opt(1_700_000_000, 0).single().expect("ts"),
            id: 42,
        };
        let filter = EventFilter {
            cursor: Some(cursor),
            ..EventFilter::new()
        };
        let select_sql = build_select_query(&filter).build(DbBackend::Postgres).sql;
        let count_sql = build_count_query(&filter).build(DbBackend::Postgres).sql;
        assert!(
            select_sql.contains("\"id\" >"),
            "select must include cursor: {select_sql}"
        );
        assert!(
            !count_sql.contains("\"id\" >"),
            "count must omit cursor predicate so total is pagination independent: {count_sql}"
        );
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
