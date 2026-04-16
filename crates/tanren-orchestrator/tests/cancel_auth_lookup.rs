//! Regression tests for cancel auth lookup behavior.

#[path = "support/dispatch_fixtures.rs"]
mod dispatch_fixtures;

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use dispatch_fixtures::{sample_actor, sample_command, sample_replay_guard};
use tanren_domain::{
    CancelDispatch, DispatchId, DispatchStatus, DispatchView, EntityKind, EventQueryResult, Lane,
    read_scope_allows_dispatch_actor,
};
use tanren_orchestrator::Orchestrator;
use tanren_policy::PolicyEngine;
use tanren_store::{
    AckAndEnqueueParams, AckParams, CancelDispatchParams, CancelPendingStepsParams,
    CreateDispatchParams, CreateDispatchWithInitialStepParams, DequeueParams, DispatchFilter,
    DispatchQueryPage, EnqueueStepParams, EventFilter, EventStore, JobQueue, NackParams,
    QueuedStep, StateStore, StoreConflictClass, StoreError, StoreOperation,
    UpdateDispatchStatusParams,
};
use tokio::sync::Mutex;

#[derive(Debug, Default)]
struct LookupState {
    dispatches: HashMap<DispatchId, DispatchView>,
    cancel_invocations: usize,
}

#[derive(Debug, Clone, Default)]
struct PanicOnGetDispatchStore {
    state: Arc<Mutex<LookupState>>,
}

impl PanicOnGetDispatchStore {
    async fn cancel_invocations(&self) -> usize {
        self.state.lock().await.cancel_invocations
    }
}

#[async_trait]
impl EventStore for PanicOnGetDispatchStore {
    async fn query_events(&self, _filter: &EventFilter) -> Result<EventQueryResult, StoreError> {
        Ok(EventQueryResult {
            events: vec![],
            total_count: None,
            has_more: false,
            next_cursor: None,
        })
    }

    async fn append_policy_decision_event(
        &self,
        _event: &tanren_domain::EventEnvelope,
    ) -> Result<(), StoreError> {
        Ok(())
    }

    async fn record_policy_decision_with_replay(
        &self,
        _event: &tanren_domain::EventEnvelope,
        _replay_guard: tanren_store::ReplayGuard,
    ) -> Result<(), StoreError> {
        Ok(())
    }
}

#[async_trait]
impl JobQueue for PanicOnGetDispatchStore {
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
impl StateStore for PanicOnGetDispatchStore {
    async fn get_dispatch(&self, _id: &DispatchId) -> Result<Option<DispatchView>, StoreError> {
        Err(StoreError::Conflict {
            class: StoreConflictClass::Other,
            operation: StoreOperation::CancelDispatch,
            reason: "cancel auth path must not call get_dispatch".to_owned(),
        })
    }

    async fn get_dispatch_actor_context_for_cancel_auth(
        &self,
        id: &DispatchId,
    ) -> Result<Option<tanren_domain::ActorContext>, StoreError> {
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
        _filter: &DispatchFilter,
    ) -> Result<DispatchQueryPage, StoreError> {
        Ok(DispatchQueryPage {
            dispatches: vec![],
            next_cursor: None,
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
        unreachable!("orchestrator must use create_dispatch_with_initial_step");
    }

    async fn create_dispatch_with_initial_step(
        &self,
        params: CreateDispatchWithInitialStepParams,
    ) -> Result<(), StoreError> {
        let mut state = self.state.lock().await;
        let dispatch = &params.dispatch;
        state.dispatches.insert(
            dispatch.dispatch_id,
            DispatchView {
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
            },
        );
        Ok(())
    }

    async fn cancel_dispatch(&self, params: CancelDispatchParams) -> Result<u64, StoreError> {
        let mut state = self.state.lock().await;
        let dispatch =
            state
                .dispatches
                .get_mut(&params.dispatch_id)
                .ok_or(StoreError::NotFound {
                    entity_kind: EntityKind::Dispatch,
                    id: params.dispatch_id.to_string(),
                })?;
        dispatch.status = DispatchStatus::Cancelled;
        dispatch.updated_at = Utc::now();
        state.cancel_invocations += 1;
        Ok(1)
    }

    async fn update_dispatch_status(
        &self,
        _params: UpdateDispatchStatusParams,
    ) -> Result<(), StoreError> {
        Ok(())
    }
}

#[tokio::test]
async fn cancel_dispatch_uses_minimal_cancel_auth_lookup_instead_of_full_dispatch_read() {
    let store = PanicOnGetDispatchStore::default();
    let orch = Orchestrator::new(store.clone(), PolicyEngine::new());
    let actor = sample_actor();

    let created = orch
        .create_dispatch(sample_command(actor.clone()), sample_replay_guard())
        .await
        .expect("create");

    orch.cancel_dispatch(
        CancelDispatch {
            actor,
            dispatch_id: created.dispatch_id,
            reason: Some("cancel".to_owned()),
        },
        sample_replay_guard(),
    )
    .await
    .expect("cancel should succeed");

    assert_eq!(store.cancel_invocations().await, 1);
}
