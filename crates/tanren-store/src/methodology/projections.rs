//! Pure in-memory projections over the methodology event stream.
//!
//! Query paths are index-backed where possible:
//! - `event_type = methodology` always
//! - `entity_kind` for kind-scoped projections
//! - `entity_ref` for per-entity folds
//!
//! Then each function folds the resulting stream via pure domain-level
//! projection helpers.

use tanren_domain::events::DomainEvent;
use tanren_domain::methodology::events::{
    MethodologyEvent, TaskCreated as EvTaskCreated, fold_task_status,
};
use tanren_domain::methodology::finding::Finding;
use tanren_domain::methodology::rubric::RubricScore;
use tanren_domain::methodology::signpost::Signpost;
use tanren_domain::methodology::task::{RequiredGuard, Task, TaskStatus};
use tanren_domain::{EntityKind, EntityRef, SpecId, TaskId};

use crate::event_store::EventStore;
use crate::params::EventFilter;

const METHODOLOGY_PAGE_SIZE: u64 = 1_000;

/// Typed error from methodology projection queries.
#[derive(Debug, thiserror::Error)]
pub enum MethodologyEventFetchError {
    #[error("store error: {source}")]
    Store {
        #[from]
        source: crate::errors::StoreError,
    },
}

/// Fetch every methodology event correlated to `spec_id` (in timestamp
/// order). Because events route under per-entity roots, this is the
/// broadest read path and may still scan more than kind-scoped helpers;
/// callers should prefer `*_for_kind` / `*_for_entity` when possible.
///
/// # Errors
/// Returns [`MethodologyEventFetchError::Store`] on query failure.
pub async fn load_methodology_events<S: EventStore>(
    store: &S,
    spec_id: SpecId,
    page_size: u64,
) -> Result<Vec<MethodologyEvent>, MethodologyEventFetchError> {
    load_methodology_events_filtered(store, spec_id, page_size, None, None).await
}

/// Fetch methodology events correlated to `spec_id`, restricted to
/// one entity kind.
///
/// This keeps hot projections (tasks/findings/signposts/rubric)
/// on index-backed query paths instead of scanning all methodology
/// rows regardless of entity kind.
///
/// # Errors
/// Returns [`MethodologyEventFetchError::Store`] on query failure.
pub async fn load_methodology_events_for_kind<S: EventStore>(
    store: &S,
    spec_id: SpecId,
    page_size: u64,
    entity_kind: EntityKind,
) -> Result<Vec<MethodologyEvent>, MethodologyEventFetchError> {
    load_methodology_events_filtered(store, spec_id, page_size, Some(entity_kind), None).await
}

/// Fetch methodology events for one concrete entity root.
///
/// # Errors
/// Returns [`MethodologyEventFetchError::Store`] on query failure.
pub async fn load_methodology_events_for_entity<S: EventStore>(
    store: &S,
    entity_ref: EntityRef,
    expected_spec_id: Option<SpecId>,
    page_size: u64,
) -> Result<Vec<MethodologyEvent>, MethodologyEventFetchError> {
    let mut out = load_methodology_events_for_entity_unscoped(store, entity_ref, page_size).await?;
    if let Some(spec_id) = expected_spec_id {
        out.retain(|e| e.spec_id() == Some(spec_id));
    }
    Ok(out)
}

async fn load_methodology_events_for_entity_unscoped<S: EventStore>(
    store: &S,
    entity_ref: EntityRef,
    page_size: u64,
) -> Result<Vec<MethodologyEvent>, MethodologyEventFetchError> {
    let mut cursor = None;
    let mut out: Vec<MethodologyEvent> = Vec::new();
    loop {
        let filter = EventFilter {
            entity_ref: Some(entity_ref),
            event_type: Some("methodology".into()),
            limit: page_size.max(1),
            cursor,
            ..EventFilter::new()
        };
        let page = store.query_events(&filter).await?;
        for env in page.events {
            if let DomainEvent::Methodology { event } = env.payload {
                out.push(event);
            }
        }
        if !page.has_more {
            break;
        }
        cursor = page.next_cursor;
    }
    Ok(out)
}

async fn load_methodology_events_filtered<S: EventStore>(
    store: &S,
    spec_id: SpecId,
    page_size: u64,
    entity_kind: Option<EntityKind>,
    entity_ref: Option<EntityRef>,
) -> Result<Vec<MethodologyEvent>, MethodologyEventFetchError> {
    let mut cursor = None;
    let mut out: Vec<MethodologyEvent> = Vec::new();
    loop {
        let filter = EventFilter {
            entity_ref,
            entity_kind,
            event_type: Some("methodology".into()),
            limit: page_size.max(1),
            cursor,
            ..EventFilter::new()
        };
        let page = store.query_events(&filter).await?;
        for env in page.events {
            if let DomainEvent::Methodology { event } = env.payload
                && event.spec_id() == Some(spec_id)
            {
                out.push(event);
            }
        }
        if !page.has_more {
            break;
        }
        cursor = page.next_cursor;
    }
    Ok(out)
}

/// Fold the methodology event stream into the current set of tasks for
/// a spec.
///
/// Each `TaskCreated` seeds a task; subsequent events mutate its status
/// (respecting the monotonicity invariants proven in
/// `tanren_domain::methodology::events` proptests).
///
/// # Errors
/// See [`load_methodology_events`].
pub async fn tasks_for_spec<S: EventStore>(
    store: &S,
    spec_id: SpecId,
    required_guards: &[RequiredGuard],
) -> Result<Vec<Task>, MethodologyEventFetchError> {
    let events =
        load_methodology_events_for_kind(store, spec_id, METHODOLOGY_PAGE_SIZE, EntityKind::Task)
            .await?;
    Ok(fold_tasks(&events, required_guards))
}

/// Pure fold: event stream → `Vec<Task>` with current status.
///
/// Exposed for unit tests and for the service layer's in-memory cache.
#[must_use]
pub fn fold_tasks(events: &[MethodologyEvent], required: &[RequiredGuard]) -> Vec<Task> {
    let mut seed: std::collections::HashMap<TaskId, Task> = std::collections::HashMap::new();
    for ev in events {
        match ev {
            MethodologyEvent::TaskCreated(EvTaskCreated { task, .. }) => {
                seed.insert(task.id, (**task).clone());
            }
            MethodologyEvent::TaskRevised(e) => {
                if let Some(t) = seed.get_mut(&e.task_id) {
                    t.description.clone_from(&e.revised_description);
                    t.acceptance_criteria.clone_from(&e.revised_acceptance);
                }
            }
            _ => {}
        }
    }
    let mut out: Vec<Task> = seed.into_values().collect();
    for t in &mut out {
        t.status = fold_task_status(t.id, required, events).unwrap_or(TaskStatus::Pending);
    }
    // Deterministic order: created_at, then id (uuid-v7 tiebreaker).
    out.sort_by(|a, b| {
        a.created_at
            .cmp(&b.created_at)
            .then(a.id.into_uuid().cmp(&b.id.into_uuid()))
    });
    out
}

/// Fold the methodology event stream into the set of all findings
/// (audit + demo + investigation + triage + feedback) for a spec.
///
/// # Errors
/// See [`load_methodology_events`].
pub async fn findings_for_spec<S: EventStore>(
    store: &S,
    spec_id: SpecId,
) -> Result<Vec<Finding>, MethodologyEventFetchError> {
    let events = load_methodology_events_for_kind(
        store,
        spec_id,
        METHODOLOGY_PAGE_SIZE,
        EntityKind::Finding,
    )
    .await?;
    let mut out = Vec::new();
    for ev in events {
        match ev {
            MethodologyEvent::FindingAdded(e) => out.push(*e.finding),
            MethodologyEvent::AdherenceFindingAdded(e) => out.push(*e.finding),
            _ => {}
        }
    }
    Ok(out)
}

/// Fold to the findings attached to a specific task.
///
/// # Errors
/// See [`load_methodology_events`].
pub async fn findings_for_task<S: EventStore>(
    store: &S,
    spec_id: SpecId,
    task_id: TaskId,
) -> Result<Vec<Finding>, MethodologyEventFetchError> {
    let all = findings_for_spec(store, spec_id).await?;
    Ok(all
        .into_iter()
        .filter(|f| f.attached_task == Some(task_id))
        .collect())
}

/// Fold to adherence-only findings for a spec.
///
/// # Errors
/// See [`load_methodology_events`].
pub async fn adherence_findings_for_spec<S: EventStore>(
    store: &S,
    spec_id: SpecId,
) -> Result<Vec<Finding>, MethodologyEventFetchError> {
    let events = load_methodology_events_for_kind(
        store,
        spec_id,
        METHODOLOGY_PAGE_SIZE,
        EntityKind::Finding,
    )
    .await?;
    let mut out = Vec::new();
    for ev in events {
        if let MethodologyEvent::AdherenceFindingAdded(e) = ev {
            out.push(*e.finding);
        }
    }
    Ok(out)
}

/// Fold to the current signpost list for a spec.
///
/// `SignpostAdded` seeds entries; `SignpostStatusUpdated` mutates
/// status/resolution on the existing entry.
///
/// # Errors
/// See [`load_methodology_events`].
pub async fn signposts_for_spec<S: EventStore>(
    store: &S,
    spec_id: SpecId,
) -> Result<Vec<Signpost>, MethodologyEventFetchError> {
    let events = load_methodology_events_for_kind(
        store,
        spec_id,
        METHODOLOGY_PAGE_SIZE,
        EntityKind::Signpost,
    )
    .await?;
    let mut seed: std::collections::HashMap<tanren_domain::SignpostId, Signpost> =
        std::collections::HashMap::new();
    for ev in events {
        match ev {
            MethodologyEvent::SignpostAdded(e) => {
                seed.insert(e.signpost.id, *e.signpost);
            }
            MethodologyEvent::SignpostStatusUpdated(e) => {
                if let Some(sp) = seed.get_mut(&e.signpost_id) {
                    sp.status = e.status;
                    sp.resolution = e.resolution;
                }
            }
            _ => {}
        }
    }
    let mut out: Vec<Signpost> = seed.into_values().collect();
    out.sort_by(|a, b| {
        a.created_at
            .cmp(&b.created_at)
            .then(a.id.into_uuid().cmp(&b.id.into_uuid()))
    });
    Ok(out)
}

/// Fold to the current rubric scorecard for a spec.
///
/// The latest [`RubricScoreRecorded`](MethodologyEvent::RubricScoreRecorded)
/// event per `(scope, scope_target_id, pillar)` key wins.
///
/// # Errors
/// See [`load_methodology_events`].
pub async fn rubric_for_spec<S: EventStore>(
    store: &S,
    spec_id: SpecId,
) -> Result<Vec<RubricScore>, MethodologyEventFetchError> {
    let events =
        load_methodology_events_for_kind(store, spec_id, METHODOLOGY_PAGE_SIZE, EntityKind::Spec)
            .await?;
    let mut latest: std::collections::HashMap<
        (
            tanren_domain::methodology::pillar::PillarScope,
            Option<String>,
            String,
        ),
        RubricScore,
    > = std::collections::HashMap::new();
    for ev in events {
        if let MethodologyEvent::RubricScoreRecorded(e) = ev {
            let key = (
                e.scope,
                e.scope_target_id.clone(),
                e.score.pillar.as_str().to_owned(),
            );
            latest.insert(key, e.score);
        }
    }
    let mut out: Vec<RubricScore> = latest.into_values().collect();
    out.sort_by(|a, b| a.pillar.as_str().cmp(b.pillar.as_str()));
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use tanren_domain::NonEmptyString;
    use tanren_domain::methodology::task::{Task, TaskOrigin, TaskStatus};

    fn seed_task(spec: SpecId) -> Task {
        Task {
            id: TaskId::new(),
            spec_id: spec,
            title: NonEmptyString::try_new("t").expect("non-empty"),
            description: String::new(),
            acceptance_criteria: vec![],
            origin: TaskOrigin::ShapeSpec,
            status: TaskStatus::Pending,
            depends_on: vec![],
            parent_task_id: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn fold_tasks_returns_pending_on_creation() {
        let spec = SpecId::new();
        let t = seed_task(spec);
        let events = vec![MethodologyEvent::TaskCreated(EvTaskCreated {
            task: Box::new(t.clone()),
            origin: TaskOrigin::ShapeSpec,
            idempotency_key: None,
        })];
        let required = [
            RequiredGuard::GateChecked,
            RequiredGuard::Audited,
            RequiredGuard::Adherent,
        ];
        let tasks = fold_tasks(&events, &required);
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].id, t.id);
        assert_eq!(tasks[0].status, TaskStatus::Pending);
    }
}
