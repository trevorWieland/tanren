//! In-memory recording store used by orchestrator unit tests.
//!
//! Records every mutating call (create/cancel/status-update/policy
//! decision with replay) and enforces JTI uniqueness across mutating
//! paths so tests can assert that replay-guard consumption actually
//! happens before policy-denied or successful mutations return.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use tanren_domain::{
    ActorContext, DispatchId, DispatchStatus, DispatchView, EntityKind, EventEnvelope,
    EventQueryResult, Lane, read_scope_allows_dispatch_actor,
};
use tanren_store::{
    AckAndEnqueueParams, AckParams, CancelDispatchParams, CancelPendingStepsParams,
    CreateDispatchParams, CreateDispatchWithInitialStepParams, DequeueParams, DispatchCursor,
    DispatchFilter, DispatchQueryPage, EnqueueStepParams, EventFilter, EventStore, JobQueue,
    NackParams, QueuedStep, ReplayGuard, StateStore, StoreConflictClass, StoreError,
    StoreOperation, UpdateDispatchStatusParams,
};
use tokio::sync::Mutex;

#[derive(Debug, Default)]
pub(crate) struct RecordingState {
    pub(crate) created_dispatches: Vec<CreateDispatchWithInitialStepParams>,
    pub(crate) cancelled_dispatches: Vec<CancelDispatchParams>,
    pub(crate) dispatch_status_updates: Vec<UpdateDispatchStatusParams>,
    pub(crate) policy_decision_events: Vec<EventEnvelope>,
    pub(crate) consumed_replay_jtis: HashSet<String>,
    pub(crate) dispatches: HashMap<DispatchId, DispatchView>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct RecordingStore {
    pub(crate) state: Arc<Mutex<RecordingState>>,
}

impl RecordingStore {
    pub(crate) async fn snapshot(&self) -> RecordingStateSnapshot {
        let state = self.state.lock().await;
        RecordingStateSnapshot {
            created_dispatches: state.created_dispatches.clone(),
            cancelled_dispatches: state.cancelled_dispatches.clone(),
            dispatch_status_updates: state.dispatch_status_updates.clone(),
            policy_decision_events: state.policy_decision_events.clone(),
            consumed_replay_jtis: state.consumed_replay_jtis.clone(),
            dispatches: state.dispatches.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct RecordingStateSnapshot {
    pub(crate) created_dispatches: Vec<CreateDispatchWithInitialStepParams>,
    pub(crate) cancelled_dispatches: Vec<CancelDispatchParams>,
    pub(crate) dispatch_status_updates: Vec<UpdateDispatchStatusParams>,
    pub(crate) policy_decision_events: Vec<EventEnvelope>,
    pub(crate) consumed_replay_jtis: HashSet<String>,
    pub(crate) dispatches: HashMap<DispatchId, DispatchView>,
}

#[async_trait]
impl EventStore for RecordingStore {
    async fn query_events(&self, _filter: &EventFilter) -> Result<EventQueryResult, StoreError> {
        Ok(EventQueryResult {
            events: vec![],
            total_count: None,
            has_more: false,
            next_cursor: None,
        })
    }

    async fn append_policy_decision_event(&self, event: &EventEnvelope) -> Result<(), StoreError> {
        let mut state = self.state.lock().await;
        state.policy_decision_events.push(event.clone());
        Ok(())
    }

    async fn record_policy_decision_with_replay(
        &self,
        event: &EventEnvelope,
        replay_guard: ReplayGuard,
    ) -> Result<(), StoreError> {
        let mut state = self.state.lock().await;
        if !state.consumed_replay_jtis.insert(replay_guard.jti.clone()) {
            return Err(StoreError::ReplayRejected);
        }
        state.policy_decision_events.push(event.clone());
        Ok(())
    }
}

#[async_trait]
impl JobQueue for RecordingStore {
    async fn enqueue_step(&self, _params: EnqueueStepParams) -> Result<(), StoreError> {
        Ok(())
    }

    async fn dequeue(&self, _params: DequeueParams) -> Result<Option<QueuedStep>, StoreError> {
        Ok(None)
    }

    async fn ack(&self, _params: AckParams) -> Result<(), StoreError> {
        Ok(())
    }

    async fn ack_and_enqueue(&self, _params: AckAndEnqueueParams) -> Result<(), StoreError> {
        Ok(())
    }

    async fn cancel_pending_steps(
        &self,
        _params: CancelPendingStepsParams,
    ) -> Result<u64, StoreError> {
        Ok(0)
    }

    async fn nack(&self, _params: NackParams) -> Result<(), StoreError> {
        Ok(())
    }

    async fn heartbeat_step(&self, _step_id: &tanren_domain::StepId) -> Result<(), StoreError> {
        Ok(())
    }

    async fn recover_stale_steps(&self, _timeout_secs: u64) -> Result<u64, StoreError> {
        Ok(0)
    }
}

#[async_trait]
impl StateStore for RecordingStore {
    async fn get_dispatch(&self, id: &DispatchId) -> Result<Option<DispatchView>, StoreError> {
        let state = self.state.lock().await;
        Ok(state.dispatches.get(id).cloned())
    }

    async fn get_dispatch_actor_context_for_cancel_auth(
        &self,
        id: &DispatchId,
    ) -> Result<Option<ActorContext>, StoreError> {
        let state = self.state.lock().await;
        Ok(state
            .dispatches
            .get(id)
            .map(|dispatch| dispatch.actor.clone()))
    }

    async fn get_dispatch_scoped(
        &self,
        id: &DispatchId,
        scope: tanren_domain::DispatchReadScope,
    ) -> Result<Option<DispatchView>, StoreError> {
        let state = self.state.lock().await;
        Ok(state
            .dispatches
            .get(id)
            .cloned()
            .filter(|view| read_scope_allows_dispatch_actor(scope, &view.actor)))
    }

    async fn query_dispatches(
        &self,
        filter: &DispatchFilter,
    ) -> Result<DispatchQueryPage, StoreError> {
        let state = self.state.lock().await;
        let mut dispatches: Vec<DispatchView> = state.dispatches.values().cloned().collect();
        if let Some(status) = filter.status {
            dispatches.retain(|view| view.status == status);
        }
        if let Some(lane) = filter.lane {
            dispatches.retain(|view| view.lane == lane);
        }
        if let Some(ref project) = filter.project {
            dispatches.retain(|view| view.dispatch.project.as_str() == project.as_str());
        }
        if let Some(user_id) = filter.user_id {
            dispatches.retain(|view| view.actor.user_id == user_id);
        }
        if let Some(scope) = filter.read_scope {
            dispatches.retain(|view| read_scope_allows_dispatch_actor(scope, &view.actor));
        }
        dispatches.sort_by(|a, b| {
            b.created_at
                .cmp(&a.created_at)
                .then_with(|| b.dispatch_id.into_uuid().cmp(&a.dispatch_id.into_uuid()))
        });

        if let Some(cursor) = filter.cursor {
            dispatches.retain(|view| {
                view.created_at < cursor.created_at
                    || (view.created_at == cursor.created_at
                        && view.dispatch_id.into_uuid() < cursor.dispatch_id.into_uuid())
            });
        }

        let limit = usize::try_from(filter.limit).unwrap_or(usize::MAX);
        let mut next_cursor = None;
        if dispatches.len() > limit {
            let cursor_row = dispatches
                .get(limit.saturating_sub(1))
                .expect("limit > 0 guarded by default filter");
            next_cursor = Some(DispatchCursor {
                created_at: cursor_row.created_at,
                dispatch_id: cursor_row.dispatch_id,
            });
            dispatches.truncate(limit);
        }

        Ok(DispatchQueryPage {
            dispatches,
            next_cursor,
        })
    }

    async fn get_step(
        &self,
        _id: &tanren_domain::StepId,
    ) -> Result<Option<tanren_domain::StepView>, StoreError> {
        Ok(None)
    }

    async fn get_steps_for_dispatch(
        &self,
        _dispatch_id: &DispatchId,
    ) -> Result<Vec<tanren_domain::StepView>, StoreError> {
        Ok(vec![])
    }

    async fn count_running_steps(&self, _lane: Option<&Lane>) -> Result<u64, StoreError> {
        Ok(0)
    }

    async fn create_dispatch_projection(
        &self,
        _params: CreateDispatchParams,
    ) -> Result<(), StoreError> {
        Err(StoreError::Conflict {
            class: StoreConflictClass::Contention,
            operation: StoreOperation::UpdateDispatchStatus,
            reason: "unexpected path: create_dispatch_projection".to_owned(),
        })
    }

    async fn create_dispatch_with_initial_step(
        &self,
        params: CreateDispatchWithInitialStepParams,
    ) -> Result<(), StoreError> {
        let mut state = self.state.lock().await;
        if !state
            .consumed_replay_jtis
            .insert(params.replay_guard.jti.clone())
        {
            return Err(StoreError::ReplayRejected);
        }
        state.created_dispatches.push(params.clone());
        let dispatch = &params.dispatch;
        let view = DispatchView {
            dispatch_id: dispatch.dispatch_id,
            mode: dispatch.mode,
            status: DispatchStatus::Pending,
            outcome: None,
            lane: dispatch.lane,
            dispatch: Box::new(dispatch.dispatch.clone()),
            actor: dispatch.actor.clone(),
            graph_revision: dispatch.graph_revision,
            created_at: dispatch.created_at,
            updated_at: dispatch.created_at,
        };
        state.dispatches.insert(dispatch.dispatch_id, view);
        Ok(())
    }

    async fn cancel_dispatch(&self, params: CancelDispatchParams) -> Result<u64, StoreError> {
        let mut state = self.state.lock().await;
        if !state
            .consumed_replay_jtis
            .insert(params.replay_guard.jti.clone())
        {
            return Err(StoreError::ReplayRejected);
        }
        let view = state
            .dispatches
            .get_mut(&params.dispatch_id)
            .ok_or(StoreError::NotFound {
                entity_kind: EntityKind::Dispatch,
                id: params.dispatch_id.to_string(),
            })?;
        view.status = DispatchStatus::Cancelled;
        view.updated_at = Utc::now();
        state.cancelled_dispatches.push(params);
        Ok(1)
    }

    async fn update_dispatch_status(
        &self,
        params: UpdateDispatchStatusParams,
    ) -> Result<(), StoreError> {
        let mut state = self.state.lock().await;
        let view = state
            .dispatches
            .get_mut(&params.dispatch_id)
            .ok_or(StoreError::NotFound {
                entity_kind: EntityKind::Dispatch,
                id: params.dispatch_id.to_string(),
            })?;
        if !view.status.can_transition_to(params.status) {
            return Err(StoreError::InvalidTransition {
                entity: format!("dispatch {}", params.dispatch_id),
                from: view.status.to_string(),
                to: params.status.to_string(),
            });
        }
        view.status = params.status;
        view.outcome = params.outcome;
        view.updated_at = Utc::now();
        state.dispatch_status_updates.push(params);
        Ok(())
    }
}
