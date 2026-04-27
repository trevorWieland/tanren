use tanren_domain::methodology::events::MethodologyEvent;
use tanren_domain::methodology::investigation::InvestigationAttempt;
use tanren_domain::{InvestigationAttemptId, SpecId};

use crate::event_store::EventStore;

use super::projections::{
    METHODOLOGY_PAGE_SIZE, MethodologyEventFetchError, load_methodology_events,
};

pub async fn investigation_attempts_for_spec<S: EventStore>(
    store: &S,
    spec_id: SpecId,
) -> Result<Vec<InvestigationAttempt>, MethodologyEventFetchError> {
    let events = load_methodology_events(store, spec_id, METHODOLOGY_PAGE_SIZE).await?;
    Ok(fold_investigation_attempts(&events))
}

pub async fn investigation_attempt_by_id<S: EventStore>(
    store: &S,
    spec_id: SpecId,
    attempt_id: InvestigationAttemptId,
) -> Result<Option<InvestigationAttempt>, MethodologyEventFetchError> {
    Ok(investigation_attempts_for_spec(store, spec_id)
        .await?
        .into_iter()
        .find(|attempt| attempt.id == attempt_id))
}

#[must_use]
pub fn fold_investigation_attempts(events: &[MethodologyEvent]) -> Vec<InvestigationAttempt> {
    let mut attempts = events
        .iter()
        .filter_map(|event| match event {
            MethodologyEvent::InvestigationAttemptRecorded(e) => Some(e.attempt.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();
    attempts.sort_by(|a, b| {
        a.recorded_at
            .cmp(&b.recorded_at)
            .then(a.id.to_string().cmp(&b.id.to_string()))
    });
    attempts
}
