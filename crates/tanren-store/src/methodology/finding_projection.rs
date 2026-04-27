use tanren_domain::events::DomainEvent;
use tanren_domain::methodology::events::MethodologyEvent;
use tanren_domain::methodology::finding::{
    Finding, FindingLifecycleEvidence, FindingStatus, FindingView,
};
use tanren_domain::{EntityKind, EntityRef, FindingId, SpecId, TaskId};

use crate::Store;
use crate::event_store::EventStore;
use crate::params::EventFilter;

use super::projections::{
    METHODOLOGY_PAGE_SIZE, MethodologyEventFetchError, load_methodology_events_for_kind,
};

pub async fn findings_for_spec<S: EventStore>(
    store: &S,
    spec_id: SpecId,
) -> Result<Vec<Finding>, MethodologyEventFetchError> {
    Ok(finding_views_for_spec(store, spec_id)
        .await?
        .into_iter()
        .map(|view| view.finding)
        .collect())
}

pub async fn finding_views_for_spec<S: EventStore>(
    store: &S,
    spec_id: SpecId,
) -> Result<Vec<FindingView>, MethodologyEventFetchError> {
    let events = load_methodology_events_for_kind(
        store,
        spec_id,
        METHODOLOGY_PAGE_SIZE,
        EntityKind::Finding,
    )
    .await?;
    Ok(fold_finding_views(&events))
}

#[must_use]
pub fn fold_finding_views(events: &[MethodologyEvent]) -> Vec<FindingView> {
    let mut views: std::collections::HashMap<FindingId, FindingView> =
        std::collections::HashMap::new();
    let mut out = Vec::new();
    for ev in events {
        match ev {
            MethodologyEvent::FindingAdded(e) => {
                views.insert(e.finding.id, open_view((*e.finding).clone()));
            }
            MethodologyEvent::AdherenceFindingAdded(e) => {
                views.insert(e.finding.id, open_view((*e.finding).clone()));
            }
            MethodologyEvent::FindingResolved(e) => apply_finding_status(
                &mut views,
                e.finding_id,
                FindingStatus::Resolved,
                e.evidence.clone(),
                Vec::new(),
            ),
            MethodologyEvent::FindingReopened(e) => apply_finding_status(
                &mut views,
                e.finding_id,
                FindingStatus::Reopened,
                e.evidence.clone(),
                Vec::new(),
            ),
            MethodologyEvent::FindingDeferred(e) => apply_finding_status(
                &mut views,
                e.finding_id,
                FindingStatus::Deferred,
                e.evidence.clone(),
                Vec::new(),
            ),
            MethodologyEvent::FindingSuperseded(e) => apply_finding_status(
                &mut views,
                e.finding_id,
                FindingStatus::Superseded,
                e.evidence.clone(),
                e.superseded_by.clone(),
            ),
            MethodologyEvent::FindingStillOpen(e) => apply_finding_status(
                &mut views,
                e.finding_id,
                FindingStatus::Open,
                e.evidence.clone(),
                Vec::new(),
            ),
            _ => {}
        }
    }
    out.extend(views.into_values());
    out.sort_by(|a, b| {
        a.finding
            .created_at
            .cmp(&b.finding.created_at)
            .then(a.finding.id.to_string().cmp(&b.finding.id.to_string()))
    });
    out
}

pub async fn findings_for_task(
    store: &Store,
    spec_id: SpecId,
    task_id: TaskId,
) -> Result<Vec<Finding>, MethodologyEventFetchError> {
    let finding_ids = store
        .load_methodology_finding_ids_for_task_projection(spec_id, task_id)
        .await?;
    if finding_ids.is_empty() {
        return Ok(Vec::new());
    }
    Ok(finding_views_by_ids(store, spec_id, &finding_ids)
        .await?
        .into_iter()
        .map(|view| view.finding)
        .collect())
}

pub async fn findings_by_ids(
    store: &Store,
    spec_id: SpecId,
    ids: &[FindingId],
) -> Result<Vec<Finding>, MethodologyEventFetchError> {
    Ok(finding_views_by_ids(store, spec_id, ids)
        .await?
        .into_iter()
        .map(|view| view.finding)
        .collect())
}

pub async fn finding_views_by_ids(
    store: &Store,
    spec_id: SpecId,
    ids: &[FindingId],
) -> Result<Vec<FindingView>, MethodologyEventFetchError> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let chunk_size = entity_ref_chunk_size(store);
    let mut events = Vec::new();
    for chunk in ids.chunks(chunk_size) {
        let entity_refs: Vec<EntityRef> = chunk.iter().copied().map(EntityRef::Finding).collect();
        let mut cursor = None;
        loop {
            let filter = EventFilter {
                entity_refs: Some(entity_refs.clone()),
                spec_id: Some(spec_id),
                event_type: Some("methodology".into()),
                limit: METHODOLOGY_PAGE_SIZE,
                cursor,
                ..EventFilter::new()
            };
            let page = store.query_events(&filter).await?;
            for env in page.events {
                if let DomainEvent::Methodology { event } = env.payload {
                    events.push(event);
                }
            }
            if !page.has_more {
                break;
            }
            cursor = page.next_cursor;
        }
    }
    let folded = fold_finding_views(&events)
        .into_iter()
        .map(|view| (view.finding.id, view))
        .collect::<std::collections::HashMap<_, _>>();
    Ok(ids
        .iter()
        .filter_map(|id| folded.get(id).cloned())
        .collect::<Vec<_>>())
}

fn open_view(finding: Finding) -> FindingView {
    FindingView {
        finding,
        status: FindingStatus::Open,
        lifecycle_evidence: None,
        superseded_by: Vec::new(),
        updated_at: None,
    }
}

fn apply_finding_status(
    views: &mut std::collections::HashMap<FindingId, FindingView>,
    finding_id: FindingId,
    status: FindingStatus,
    evidence: FindingLifecycleEvidence,
    superseded_by: Vec<FindingId>,
) {
    if let Some(view) = views.get_mut(&finding_id) {
        view.status = status;
        view.lifecycle_evidence = Some(evidence);
        view.superseded_by = superseded_by;
    }
}

fn entity_ref_chunk_size(store: &Store) -> usize {
    use sea_orm::ConnectionTrait;
    match store.conn().get_database_backend() {
        sea_orm::DbBackend::Sqlite => 800,
        sea_orm::DbBackend::Postgres => 10_000,
        sea_orm::DbBackend::MySql => 1_000,
    }
}
